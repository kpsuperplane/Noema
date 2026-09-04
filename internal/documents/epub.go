package documents

import (
	"encoding/xml"
	"io"
	"net/url"
	"strconv"
	"strings"
)

type epubPackage struct {
	title    string
	manifest map[string]string
	spine    []string
}

func parseEPUB(data []byte) (string, error) {
	archive, err := openDocumentArchive(data)
	if err != nil {
		return "", err
	}
	container, err := archive.part("META-INF/container.xml", true)
	if err != nil {
		return "", err
	}
	root, err := epubRoot(container)
	if err != nil {
		return "", err
	}
	packagePath, ok := resolveEPUBPart("", root)
	if !ok {
		return "", errInvalidDocument
	}
	packageData, err := archive.part(packagePath, true)
	if err != nil {
		return "", err
	}
	book, err := readEPUBPackage(packageData)
	if err != nil {
		return "", err
	}

	var output markdownDocument
	output.block("# ", book.title)
	if output.count >= maxPreviewCharacters {
		return output.String(), nil
	}
	chapters, failed := 0, 0
	for _, id := range book.spine {
		href, exists := book.manifest[id]
		if !exists {
			continue
		}
		chapters++
		chapterPath, valid := resolveEPUBPart(packagePath, href)
		if !valid {
			failed++
			continue
		}
		chapter, partErr := archive.part(chapterPath, false)
		if partErr != nil {
			return "", partErr
		}
		if chapter == nil || appendEPUBChapter(&output, chapter) != nil {
			failed++
		}
		if output.count >= maxPreviewCharacters {
			break
		}
	}
	if chapters > 0 && failed == chapters {
		return "", errInvalidDocument
	}
	return output.String(), nil
}

func resolveEPUBPart(base, target string) (string, bool) {
	decoded, err := url.PathUnescape(target)
	if err != nil || strings.ContainsRune(decoded, 0) {
		return "", false
	}
	return resolveDocumentPart(base, decoded)
}

func epubRoot(content []byte) (string, error) {
	reader := newGuardedXML(content)
	for {
		token, err := reader.token()
		if err != nil {
			return "", errInvalidDocument
		}
		start, ok := token.(xml.StartElement)
		if !ok || start.Name.Local != "rootfile" {
			continue
		}
		if root := attribute(start, "full-path"); root != "" {
			return root, nil
		}
	}
}

func readEPUBPackage(content []byte) (epubPackage, error) {
	reader := newGuardedXML(content)
	book := epubPackage{manifest: make(map[string]string)}
	inTitle := false
	var title strings.Builder
	for {
		token, err := reader.token()
		if err == io.EOF {
			book.title = strings.Join(strings.Fields(title.String()), " ")
			return book, nil
		}
		if err != nil {
			return epubPackage{}, errInvalidDocument
		}
		switch typed := token.(type) {
		case xml.StartElement:
			switch typed.Name.Local {
			case "title":
				if title.Len() == 0 {
					inTitle = true
				}
			case "item":
				id, href := attribute(typed, "id"), attribute(typed, "href")
				if id != "" && href != "" {
					book.manifest[id] = href
				}
			case "itemref":
				if id := attribute(typed, "idref"); id != "" {
					book.spine = append(book.spine, id)
				}
			}
		case xml.CharData:
			if inTitle {
				appendDocumentText(&title, string(typed))
			}
		case xml.EndElement:
			if typed.Name.Local == "title" {
				inTitle = false
			}
		}
	}
}

type epubChapter struct {
	output     *markdownDocument
	text       strings.Builder
	prefix     string
	block      string
	list       []bool
	ignored    int
	inBody     bool
	sawBody    bool
	tableDepth int
	rows       [][]string
	row        []string
	cell       strings.Builder
	inCell     bool
}

func appendEPUBChapter(output *markdownDocument, content []byte) error {
	chapter := epubChapter{output: output}
	reader := newGuardedXML(content)
	for {
		token, err := reader.token()
		if err == io.EOF {
			if !chapter.sawBody {
				return errInvalidDocument
			}
			chapter.flush()
			return nil
		}
		if err != nil {
			return errInvalidDocument
		}
		switch typed := token.(type) {
		case xml.StartElement:
			chapter.start(typed)
		case xml.CharData:
			chapter.characters(string(typed))
		case xml.EndElement:
			chapter.end(typed)
		}
	}
}

func (chapter *epubChapter) start(element xml.StartElement) {
	name := strings.ToLower(element.Name.Local)
	if chapter.ignored > 0 {
		chapter.ignored++
		return
	}
	if name == "head" || name == "script" || name == "style" || hiddenEPUBElement(element) {
		chapter.ignored = 1
		return
	}
	if name == "body" {
		chapter.inBody, chapter.sawBody = true, true
		return
	}
	if !chapter.inBody {
		return
	}
	if chapter.tableDepth > 0 {
		chapter.tableStart(name)
		return
	}
	switch name {
	case "table":
		chapter.flush()
		chapter.tableDepth = 1
		chapter.rows = nil
	case "ol":
		chapter.list = append(chapter.list, true)
	case "ul":
		chapter.list = append(chapter.list, false)
	case "p", "div", "section", "article", "aside", "header", "footer", "figure", "figcaption", "pre":
		if chapter.block != "li" && chapter.block != "blockquote" {
			chapter.beginBlock(name, "")
		}
	case "blockquote":
		chapter.beginBlock(name, "> ")
	case "li":
		prefix := "- "
		if len(chapter.list) > 0 && chapter.list[len(chapter.list)-1] {
			prefix = "1. "
		}
		chapter.beginBlock(name, prefix)
	case "h1", "h2", "h3", "h4", "h5", "h6":
		level, _ := strconv.Atoi(strings.TrimPrefix(name, "h"))
		chapter.beginBlock(name, strings.Repeat("#", level)+" ")
	case "br":
		appendDocumentRune(&chapter.text, '\n')
	case "img":
		if alt := attribute(element, "alt"); alt != "" {
			appendDocumentText(&chapter.text, alt)
		}
	}
}

func hiddenEPUBElement(element xml.StartElement) bool {
	if _, hidden := xmlAttribute(element, "hidden"); hidden {
		return true
	}
	style := strings.ToLower(attribute(element, "style"))
	style = strings.ReplaceAll(strings.ReplaceAll(style, " ", ""), "\t", "")
	return strings.Contains(style, "display:none")
}

func xmlAttribute(element xml.StartElement, local string) (string, bool) {
	for _, attr := range element.Attr {
		if attr.Name.Local == local {
			return attr.Value, true
		}
	}
	return "", false
}

func (chapter *epubChapter) beginBlock(name, prefix string) {
	chapter.flush()
	chapter.block, chapter.prefix = name, prefix
}

func (chapter *epubChapter) characters(value string) {
	if chapter.ignored > 0 || !chapter.inBody {
		return
	}
	if chapter.inCell {
		appendDocumentText(&chapter.cell, value)
		return
	}
	appendDocumentText(&chapter.text, value)
}

func (chapter *epubChapter) end(element xml.EndElement) {
	name := strings.ToLower(element.Name.Local)
	if chapter.ignored > 0 {
		chapter.ignored--
		return
	}
	if name == "body" {
		chapter.flush()
		chapter.inBody = false
		return
	}
	if !chapter.inBody {
		return
	}
	if chapter.tableDepth > 0 {
		chapter.tableEnd(name)
		return
	}
	if name == "ol" || name == "ul" {
		if len(chapter.list) > 0 {
			chapter.list = chapter.list[:len(chapter.list)-1]
		}
		return
	}
	if name == chapter.block {
		chapter.flush()
	}
}

func (chapter *epubChapter) flush() {
	value := strings.Join(strings.Fields(chapter.text.String()), " ")
	chapter.output.block(chapter.prefix, value)
	chapter.text.Reset()
	chapter.block, chapter.prefix = "", ""
}

func (chapter *epubChapter) tableStart(name string) {
	switch name {
	case "table":
		chapter.tableDepth++
	case "tr":
		if chapter.tableDepth == 1 {
			chapter.row = nil
		}
	case "th", "td":
		if chapter.tableDepth == 1 {
			chapter.cell.Reset()
			chapter.inCell = true
		}
	}
}

func (chapter *epubChapter) tableEnd(name string) {
	switch name {
	case "th", "td":
		if chapter.tableDepth == 1 && chapter.inCell {
			chapter.row = append(chapter.row, strings.Join(strings.Fields(chapter.cell.String()), " "))
			chapter.inCell = false
		}
	case "tr":
		if chapter.tableDepth == 1 {
			chapter.rows = append(chapter.rows, chapter.row)
		}
	case "table":
		chapter.tableDepth--
		if chapter.tableDepth == 0 {
			chapter.output.table(chapter.rows)
			chapter.rows = nil
		}
	}
}
