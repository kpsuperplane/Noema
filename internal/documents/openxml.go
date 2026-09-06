package documents

import (
	"bytes"
	"encoding/xml"
	"io"
	"path"
	"sort"
	"strconv"
	"strings"

	"golang.org/x/net/html/charset"
)

const (
	wordNamespace               = "http://schemas.openxmlformats.org/wordprocessingml/2006/main"
	presentationNamespace       = "http://schemas.openxmlformats.org/presentationml/2006/main"
	drawingNamespace            = "http://schemas.openxmlformats.org/drawingml/2006/main"
	relationshipNamespace       = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
	strictWordNamespace         = "http://purl.oclc.org/ooxml/wordprocessingml/main"
	strictPresentationNamespace = "http://purl.oclc.org/ooxml/presentationml/main"
	strictDrawingNamespace      = "http://purl.oclc.org/ooxml/drawingml/main"
	strictRelationshipNamespace = "http://purl.oclc.org/ooxml/officeDocument/relationships"
)

func officeName(name xml.Name, local string, namespaces ...string) bool {
	if name.Local != local {
		return false
	}
	for _, namespace := range namespaces {
		if name.Space == namespace {
			return true
		}
	}
	return false
}

type guardedXML struct {
	decoder *xml.Decoder
	depth   int
	tokens  int
}

func newGuardedXML(content []byte) *guardedXML {
	decoder := xml.NewDecoder(bytes.NewReader(content))
	decoder.CharsetReader = charset.NewReaderLabel
	return &guardedXML{decoder: decoder}
}

func (reader *guardedXML) token() (xml.Token, error) {
	token, err := reader.decoder.Token()
	if err != nil {
		return nil, err
	}
	reader.tokens++
	if reader.tokens > maxDocumentXMLTokens {
		return nil, errInvalidDocument
	}
	switch token.(type) {
	case xml.StartElement:
		reader.depth++
		if reader.depth > maxDocumentXMLDepth {
			return nil, errInvalidDocument
		}
	case xml.EndElement:
		reader.depth--
	}
	return token, nil
}

type packageRelation struct {
	target string
	kind   string
}

func packageRelations(content []byte, base string) (map[string]packageRelation, error) {
	reader := newGuardedXML(content)
	result := make(map[string]packageRelation)
	for {
		token, err := reader.token()
		if err == io.EOF {
			return result, nil
		}
		if err != nil {
			return nil, errInvalidDocument
		}
		start, ok := token.(xml.StartElement)
		if !ok || start.Name.Local != "Relationship" || strings.EqualFold(attribute(start, "TargetMode"), "External") {
			continue
		}
		target, ok := resolveDocumentPart(base, attribute(start, "Target"))
		if !ok {
			continue
		}
		id, kind := attribute(start, "Id"), attribute(start, "Type")
		if id == "" || kind == "" {
			continue
		}
		if _, duplicate := result[id]; duplicate {
			return nil, errInvalidDocument
		}
		result[id] = packageRelation{target: target, kind: kind}
	}
}

func firstRelation(relations map[string]packageRelation, suffix string) (packageRelation, bool) {
	ids := make([]string, 0, len(relations))
	for id := range relations {
		ids = append(ids, id)
	}
	sort.Strings(ids)
	for _, id := range ids {
		if strings.HasSuffix(relations[id].kind, suffix) {
			return relations[id], true
		}
	}
	return packageRelation{}, false
}

func officeDocumentPart(archive *documentArchive, fallback string) (string, error) {
	root, err := archive.part("_rels/.rels", false)
	if err != nil || root == nil {
		return fallback, err
	}
	relations, err := packageRelations(root, "")
	if err != nil {
		return "", err
	}
	if relation, ok := firstRelation(relations, "/officeDocument"); ok {
		return relation.target, nil
	}
	return fallback, nil
}

func relationsPart(part string) string {
	return path.Join(path.Dir(part), "_rels", path.Base(part)+".rels")
}

func parseDOCX(data []byte) (string, error) {
	archive, err := openDocumentArchive(data)
	if err != nil {
		return "", err
	}
	mainPart, err := officeDocumentPart(archive, "word/document.xml")
	if err != nil {
		return "", err
	}
	content, err := archive.part(mainPart, true)
	if err != nil {
		return "", err
	}
	styles := map[string]int{}
	if stylePart, partErr := archive.part(path.Join(path.Dir(mainPart), "styles.xml"), false); partErr != nil {
		return "", partErr
	} else if stylePart != nil {
		styles = wordHeadingStyles(stylePart)
	}
	return wordMarkdown(content, styles)
}

func wordHeadingStyles(content []byte) map[string]int {
	reader := newGuardedXML(content)
	result := make(map[string]int)
	var styleID string
	level := 0
	for {
		token, err := reader.token()
		if err != nil {
			return result
		}
		switch typed := token.(type) {
		case xml.StartElement:
			if officeName(typed.Name, "style", wordNamespace, strictWordNamespace) {
				styleID, level = attribute(typed, "styleId"), 0
			} else if styleID != "" && officeName(typed.Name, "name", wordNamespace, strictWordNamespace) {
				level = headingLevel(attribute(typed, "val"))
			} else if styleID != "" && officeName(typed.Name, "outlineLvl", wordNamespace, strictWordNamespace) {
				if value, parseErr := strconv.Atoi(attribute(typed, "val")); parseErr == nil && value >= 0 && value < 6 {
					level = value + 1
				}
			}
		case xml.EndElement:
			if officeName(typed.Name, "style", wordNamespace, strictWordNamespace) {
				if styleID != "" && level > 0 {
					result[styleID] = level
				}
				styleID = ""
			}
		}
	}
}

func headingLevel(value string) int {
	compact := strings.ToLower(strings.ReplaceAll(strings.TrimSpace(value), " ", ""))
	if compact == "title" {
		return 1
	}
	for _, prefix := range []string{"heading", "head"} {
		if strings.HasPrefix(compact, prefix) {
			level, _ := strconv.Atoi(strings.TrimPrefix(compact, prefix))
			if level >= 1 && level <= 6 {
				return level
			}
		}
	}
	return 0
}

func wordMarkdown(content []byte, styles map[string]int) (string, error) {
	reader := newGuardedXML(content)
	var output markdownDocument
	var paragraph strings.Builder
	heading, list := 0, false
	inParagraph, inText, preserve := false, false, false
	tableDepth := 0
	recognizedBody := false
	var rows [][]string
	var row []string
	var cell []string
	for {
		token, err := reader.token()
		if err == io.EOF {
			if !recognizedBody {
				return "", errInvalidDocument
			}
			return output.String(), nil
		}
		if err != nil {
			return "", errInvalidDocument
		}
		switch typed := token.(type) {
		case xml.StartElement:
			if typed.Name.Space != wordNamespace && typed.Name.Space != strictWordNamespace {
				continue
			}
			switch typed.Name.Local {
			case "body":
				recognizedBody = true
			case "tbl":
				tableDepth++
				if tableDepth == 1 {
					rows = nil
				}
			case "tr":
				if tableDepth == 1 {
					row = nil
				}
			case "tc":
				if tableDepth == 1 {
					cell = nil
				}
			case "p":
				inParagraph, heading, list = true, 0, false
				paragraph.Reset()
			case "pStyle":
				if inParagraph {
					style := attribute(typed, "val")
					heading = styles[style]
					if heading == 0 {
						heading = headingLevel(style)
					}
				}
			case "numId":
				list = inParagraph && attribute(typed, "val") != "0"
			case "t":
				inText = inParagraph
				preserve = attribute(typed, "space") == "preserve"
			case "tab":
				if inParagraph {
					appendDocumentRune(&paragraph, '\t')
				}
			case "br", "cr":
				if inParagraph {
					appendDocumentRune(&paragraph, '\n')
				}
			}
		case xml.CharData:
			if inText && preserve {
				appendDocumentText(&paragraph, string(typed))
			} else if inText && strings.TrimSpace(string(typed)) != "" {
				appendDocumentText(&paragraph, strings.TrimSpace(string(typed)))
			}
		case xml.EndElement:
			if typed.Name.Space != wordNamespace && typed.Name.Space != strictWordNamespace {
				continue
			}
			switch typed.Name.Local {
			case "t":
				inText = false
				preserve = false
			case "p":
				value := paragraph.String()
				if tableDepth > 0 {
					if strings.TrimSpace(value) != "" {
						cell = append(cell, value)
					}
				} else if heading > 0 {
					output.block(strings.Repeat("#", heading)+" ", value)
				} else if list {
					output.block("- ", value)
				} else {
					output.block("", value)
				}
				inParagraph = false
			case "tc":
				if tableDepth == 1 {
					row = append(row, strings.Join(cell, "\n"))
				}
			case "tr":
				if tableDepth == 1 {
					rows = append(rows, row)
				}
			case "tbl":
				if tableDepth == 1 {
					output.table(rows)
				}
				tableDepth--
			}
		}
	}
}

func (document *markdownDocument) table(rows [][]string) {
	if len(rows) == 0 || document.blocks >= maxDocumentBlocks {
		return
	}
	width := 0
	for _, row := range rows {
		width = max(width, len(row))
	}
	if width == 0 {
		return
	}
	if document.blocks > 0 {
		document.append("\n\n")
	}
	for index, row := range rows {
		values := make([]string, width)
		for column := range width {
			if column < len(row) {
				values[column] = escapeCell(row[column])
			}
		}
		if index > 0 {
			document.append("\n")
		}
		document.append("| " + strings.Join(values, " | ") + " |")
		if index == 0 {
			document.append("\n| " + strings.TrimSuffix(strings.Repeat("--- | ", width), " "))
		}
		if document.count >= maxPreviewCharacters {
			break
		}
	}
	document.blocks++
}

func parsePPTX(data []byte) (string, error) {
	archive, err := openDocumentArchive(data)
	if err != nil {
		return "", err
	}
	presentationPart, err := officeDocumentPart(archive, "ppt/presentation.xml")
	if err != nil {
		return "", err
	}
	presentation, err := archive.part(presentationPart, true)
	if err != nil {
		return "", err
	}
	relationBytes, err := archive.part(relationsPart(presentationPart), true)
	if err != nil {
		return "", err
	}
	relations, err := packageRelations(relationBytes, presentationPart)
	if err != nil {
		return "", err
	}
	ids, err := presentationSlideIDs(presentation)
	if err != nil || len(ids) == 0 {
		return "", errInvalidDocument
	}
	var output markdownDocument
	for _, id := range ids {
		relation, ok := relations[id]
		if !ok || !strings.HasSuffix(relation.kind, "/slide") {
			return "", errInvalidDocument
		}
		slide, readErr := archive.part(relation.target, true)
		if readErr != nil {
			return "", readErr
		}
		if parseErr := presentationPage(slide, false, &output); parseErr != nil {
			return "", parseErr
		}
		slideRelationsBytes, readErr := archive.part(relationsPart(relation.target), false)
		if readErr != nil || slideRelationsBytes == nil {
			continue
		}
		slideRelations, parseErr := packageRelations(slideRelationsBytes, relation.target)
		if parseErr != nil {
			return "", parseErr
		}
		if related, ok := firstRelation(slideRelations, "/notesSlide"); ok {
			notes, notesErr := archive.part(related.target, false)
			if notesErr != nil {
				return "", notesErr
			}
			if notes != nil {
				if parseErr := presentationPage(notes, true, &output); parseErr != nil {
					return "", parseErr
				}
			}
		}
	}
	return output.String(), nil
}

func presentationSlideIDs(content []byte) ([]string, error) {
	reader := newGuardedXML(content)
	var result []string
	for {
		token, err := reader.token()
		if err == io.EOF {
			return result, nil
		}
		if err != nil {
			return nil, errInvalidDocument
		}
		start, ok := token.(xml.StartElement)
		if ok && officeName(start.Name, "sldId", presentationNamespace, strictPresentationNamespace) {
			for _, value := range start.Attr {
				if officeName(value.Name, "id", relationshipNamespace, strictRelationshipNamespace) {
					result = append(result, value.Value)
				}
			}
		}
	}
}

func presentationPage(content []byte, notes bool, output *markdownDocument) error {
	reader := newGuardedXML(content)
	var paragraph strings.Builder
	inParagraph, inText, title, skipShape := false, false, false, false
	shapeDepth := 0
	for {
		token, err := reader.token()
		if err == io.EOF {
			return nil
		}
		if err != nil {
			return errInvalidDocument
		}
		switch typed := token.(type) {
		case xml.StartElement:
			if officeName(typed.Name, "sp", presentationNamespace, strictPresentationNamespace) {
				shapeDepth, title, skipShape = reader.depth, false, false
			} else if shapeDepth > 0 && officeName(typed.Name, "ph", presentationNamespace, strictPresentationNamespace) {
				kind := attribute(typed, "type")
				title = kind == "title" || kind == "ctrTitle"
				skipShape = notes && (kind == "sldImg" || kind == "sldNum" || kind == "hdr" || kind == "ftr" || kind == "dt")
			} else if officeName(typed.Name, "p", drawingNamespace, strictDrawingNamespace) {
				inParagraph = true
				paragraph.Reset()
			} else if inParagraph && officeName(typed.Name, "t", drawingNamespace, strictDrawingNamespace) {
				inText = true
			} else if inParagraph && (officeName(typed.Name, "br", drawingNamespace, strictDrawingNamespace) || officeName(typed.Name, "tab", drawingNamespace, strictDrawingNamespace)) {
				appendDocumentRune(&paragraph, '\n')
			}
		case xml.CharData:
			if inText && !skipShape {
				appendDocumentText(&paragraph, string(typed))
			}
		case xml.EndElement:
			if officeName(typed.Name, "t", drawingNamespace, strictDrawingNamespace) {
				inText = false
			} else if officeName(typed.Name, "p", drawingNamespace, strictDrawingNamespace) {
				if notes {
					output.block("> ", paragraph.String())
				} else if title {
					output.block("## ", paragraph.String())
				} else {
					output.block("", paragraph.String())
				}
				inParagraph = false
			} else if officeName(typed.Name, "sp", presentationNamespace, strictPresentationNamespace) && reader.depth < shapeDepth {
				shapeDepth = 0
			}
		}
	}
}
