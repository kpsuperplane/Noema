package documents

import (
	"encoding/xml"
	"io"
	"strconv"
	"strings"
)

const (
	officeNamespace      = "urn:oasis:names:tc:opendocument:xmlns:office:1.0"
	textNamespace        = "urn:oasis:names:tc:opendocument:xmlns:text:1.0"
	tableNamespace       = "urn:oasis:names:tc:opendocument:xmlns:table:1.0"
	drawingODFNamespace  = "urn:oasis:names:tc:opendocument:xmlns:drawing:1.0"
	presentationODFSpace = "urn:oasis:names:tc:opendocument:xmlns:presentation:1.0"
	manifestODFNamespace = "urn:oasis:names:tc:opendocument:xmlns:manifest:1.0"
)

func parseODF(data []byte, presentation bool) (string, error) {
	archive, err := openDocumentArchive(data)
	if err != nil {
		return "", err
	}
	if manifest, readErr := archive.part("META-INF/manifest.xml", false); readErr != nil {
		return "", readErr
	} else if manifest != nil && odfEncrypted(manifest) {
		return "", errInvalidDocument
	}
	content, err := archive.part("content.xml", true)
	if err != nil {
		return "", err
	}
	return odfMarkdown(content, presentation)
}

func odfEncrypted(content []byte) bool {
	reader := newGuardedXML(content)
	for {
		token, err := reader.token()
		if err != nil {
			return false
		}
		start, ok := token.(xml.StartElement)
		if ok && start.Name.Space == manifestODFNamespace && start.Name.Local == "encryption-data" {
			return true
		}
	}
}

func odfMarkdown(content []byte, presentation bool) (string, error) {
	reader := newGuardedXML(content)
	var output markdownDocument
	var paragraph strings.Builder
	inParagraph, recognized := false, false
	heading, listDepth := 0, 0
	pageDepth, frameDepth, notesDepth := 0, 0, 0
	frameTitle, skipFrame := false, false
	tableDepth := 0
	var rows [][]string
	var row []string
	var cell []string
	rowRepeat, cellRepeat := 1, 1
	for {
		token, err := reader.token()
		if err == io.EOF {
			if !recognized {
				return "", errInvalidDocument
			}
			return output.String(), nil
		}
		if err != nil {
			return "", errInvalidDocument
		}
		switch typed := token.(type) {
		case xml.StartElement:
			switch {
			case typed.Name.Space == officeNamespace && typed.Name.Local == "text":
				if presentation {
					return "", errInvalidDocument
				}
				recognized = true
			case typed.Name.Space == officeNamespace && typed.Name.Local == "presentation":
				if !presentation {
					return "", errInvalidDocument
				}
				recognized = true
			case typed.Name.Space == drawingODFNamespace && typed.Name.Local == "page":
				pageDepth = reader.depth
			case typed.Name.Space == drawingODFNamespace && typed.Name.Local == "frame":
				frameDepth = reader.depth
				class := attribute(typed, "class")
				frameTitle = class == "title"
				skipFrame = class == "page-number" || class == "date-time" || class == "footer" || class == "header"
			case typed.Name.Space == presentationODFSpace && typed.Name.Local == "notes":
				notesDepth = reader.depth
			case typed.Name.Space == textNamespace && typed.Name.Local == "list":
				listDepth++
			case typed.Name.Space == tableNamespace && typed.Name.Local == "table":
				tableDepth++
				if tableDepth == 1 {
					rows = nil
				}
			case typed.Name.Space == tableNamespace && typed.Name.Local == "table-row":
				if tableDepth == 1 {
					row = nil
					rowRepeat = boundedRepeat(typed, "number-rows-repeated", maxRows)
				}
			case typed.Name.Space == tableNamespace && (typed.Name.Local == "table-cell" || typed.Name.Local == "covered-table-cell"):
				if tableDepth == 1 {
					cell = nil
					cellRepeat = boundedRepeat(typed, "number-columns-repeated", maxColumns)
				}
			case typed.Name.Space == textNamespace && (typed.Name.Local == "p" || typed.Name.Local == "h"):
				inParagraph, heading = true, 0
				paragraph.Reset()
				if typed.Name.Local == "h" {
					heading = boundedRepeat(typed, "outline-level", 6)
				}
			case inParagraph && typed.Name.Space == textNamespace && typed.Name.Local == "s":
				appendDocumentSpaces(&paragraph, boundedRepeat(typed, "c", maxPreviewCharacters))
			case inParagraph && typed.Name.Space == textNamespace && typed.Name.Local == "tab":
				appendDocumentRune(&paragraph, '\t')
			case inParagraph && typed.Name.Space == textNamespace && typed.Name.Local == "line-break":
				appendDocumentRune(&paragraph, '\n')
			}
		case xml.CharData:
			if inParagraph {
				appendDocumentText(&paragraph, string(typed))
			}
		case xml.EndElement:
			switch {
			case typed.Name.Space == textNamespace && (typed.Name.Local == "p" || typed.Name.Local == "h"):
				value := paragraph.String()
				if tableDepth == 1 {
					if strings.TrimSpace(value) != "" {
						cell = append(cell, value)
					}
				} else if presentation && skipFrame {
				} else if presentation && notesDepth > 0 {
					output.block("> ", value)
				} else if presentation && frameTitle {
					output.block("## ", value)
				} else if heading > 0 {
					output.block(strings.Repeat("#", min(heading, 6))+" ", value)
				} else if listDepth > 0 {
					output.block(strings.Repeat("  ", min(listDepth-1, 8))+"- ", value)
				} else {
					output.block("", value)
				}
				inParagraph = false
			case typed.Name.Space == tableNamespace && (typed.Name.Local == "table-cell" || typed.Name.Local == "covered-table-cell"):
				if tableDepth == 1 {
					for range min(cellRepeat, maxColumns-len(row)) {
						row = append(row, strings.Join(cell, "\n"))
					}
				}
			case typed.Name.Space == tableNamespace && typed.Name.Local == "table-row":
				if tableDepth == 1 {
					for range min(rowRepeat, maxRows-len(rows)) {
						rows = append(rows, append([]string(nil), row...))
					}
				}
			case typed.Name.Space == tableNamespace && typed.Name.Local == "table":
				if tableDepth == 1 {
					output.table(rows)
				}
				tableDepth--
			case typed.Name.Space == textNamespace && typed.Name.Local == "list":
				listDepth--
			case typed.Name.Space == presentationODFSpace && typed.Name.Local == "notes" && reader.depth < notesDepth:
				notesDepth = 0
			case typed.Name.Space == drawingODFNamespace && typed.Name.Local == "frame" && reader.depth < frameDepth:
				frameDepth, frameTitle, skipFrame = 0, false, false
			case typed.Name.Space == drawingODFNamespace && typed.Name.Local == "page" && reader.depth < pageDepth:
				pageDepth = 0
			}
		}
	}
}

func boundedRepeat(start xml.StartElement, name string, limit int) int {
	value, err := strconv.Atoi(attribute(start, name))
	if err != nil || value < 1 {
		return 1
	}
	return min(value, limit)
}
