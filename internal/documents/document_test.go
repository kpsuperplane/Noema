package documents

import (
	"strings"
	"testing"
	"unicode/utf8"
)

func TestDocumentMarkdownDOCXKeepsStructure(t *testing.T) {
	content := zipContent(t, map[string]string{
		"word/document.xml": `<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>
			<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Intro</w:t></w:r></w:p>
			<w:p><w:pPr><w:numPr><w:numId w:val="4"/></w:numPr></w:pPr><w:r><w:t>*first*</w:t></w:r></w:p>
			<w:tbl><w:tr><w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Count</w:t></w:r></w:p></w:tc></w:tr>
			<w:tr><w:tc><w:p><w:r><w:t>A|B</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>2</w:t></w:r></w:p></w:tc></w:tr></w:tbl>
		</w:body></w:document>`,
	})
	preview, converted := DocumentMarkdown(content, MediaDOCX+"; charset=binary")
	want := "# Intro\n\n- \\*first\\*\n\n| Name | Count |\n| --- | --- |\n| A\\|B | 2 |"
	if !converted || preview != want {
		t.Fatalf("DOCX preview = %q, converted = %v", preview, converted)
	}
	strict := zipContent(t, map[string]string{
		"word/document.xml": strings.ReplaceAll(
			`<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Strict</w:t></w:r></w:p></w:body></w:document>`,
			wordNamespace, strictWordNamespace,
		),
	})
	if preview, converted = DocumentMarkdown(strict, MediaDOCX); !converted || preview != "Strict" {
		t.Fatalf("strict DOCX preview = %q, converted = %v", preview, converted)
	}
}

func TestDocumentMarkdownODTKeepsListsAndTables(t *testing.T) {
	content := zipContent(t, map[string]string{
		"content.xml": `<?xml version="1.0"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:table="urn:oasis:names:tc:opendocument:xmlns:table:1.0"><office:body><office:text>
			<text:h text:outline-level="2">Plan</text:h><text:list><text:list-item><text:p>One<text:s text:c="2"/>item</text:p></text:list-item></text:list>
			<table:table><table:table-row><table:table-cell><text:p>A</text:p></table:table-cell><table:table-cell><text:p>B</text:p></table:table-cell></table:table-row></table:table>
		</office:text></office:body></office:document-content>`,
	})
	preview, converted := DocumentMarkdown(content, MediaODT)
	if !converted || preview != "## Plan\n\n- One  item\n\n| A | B |\n| --- | --- |" {
		t.Fatalf("ODT preview = %q, converted = %v", preview, converted)
	}
}

func TestDocumentMarkdownRejectsEncryptedODF(t *testing.T) {
	content := zipContent(t, map[string]string{
		"META-INF/manifest.xml": `<manifest:manifest xmlns:manifest="urn:oasis:names:tc:opendocument:xmlns:manifest:1.0"><manifest:file-entry><manifest:encryption-data/></manifest:file-entry></manifest:manifest>`,
		"content.xml":           `<office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0"><office:body/></office:document-content>`,
	})
	if preview, converted := DocumentMarkdown(content, MediaODT); converted || preview != "" {
		t.Fatalf("encrypted ODT converted to %q", preview)
	}
}

func TestDocumentMarkdownPPTXUsesDeclaredOrderAndNotes(t *testing.T) {
	presentation := `<?xml version="1.0"?><p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst><p:sldId r:id="rId2"/><p:sldId r:id="rId1"/></p:sldIdLst></p:presentation>`
	relations := `<?xml version="1.0"?><Relationships><Relationship Id="rId1" Type="x/slide" Target="slides/slide1.xml"/><Relationship Id="rId2" Type="x/slide" Target="slides/slide2.xml"/></Relationships>`
	slide := func(title string) string {
		return `<p:sld xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>` + title + `</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:sld>`
	}
	notes := `<p:notes xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><p:cSld><p:spTree><p:sp><p:nvSpPr><p:nvPr><p:ph type="body"/></p:nvPr></p:nvSpPr><p:txBody><a:p><a:r><a:t>Speaker note</a:t></a:r></a:p></p:txBody></p:sp></p:spTree></p:cSld></p:notes>`
	content := zipContent(t, map[string]string{
		"ppt/presentation.xml":             presentation,
		"ppt/_rels/presentation.xml.rels":  relations,
		"ppt/slides/slide1.xml":            slide("First"),
		"ppt/slides/slide2.xml":            slide("Second"),
		"ppt/slides/_rels/slide2.xml.rels": `<Relationships><Relationship Id="n1" Type="x/notesSlide" Target="../notesSlides/notesSlide2.xml"/></Relationships>`,
		"ppt/notesSlides/notesSlide2.xml":  notes,
	})
	preview, converted := DocumentMarkdown(content, MediaPPTX)
	if !converted || preview != "## Second\n\n> Speaker note\n\n## First" {
		t.Fatalf("PPTX preview = %q, converted = %v", preview, converted)
	}
}

func TestDocumentMarkdownODPKeepsTitlesAndNotes(t *testing.T) {
	content := zipContent(t, map[string]string{
		"content.xml": `<?xml version="1.0"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0" xmlns:draw="urn:oasis:names:tc:opendocument:xmlns:drawing:1.0" xmlns:presentation="urn:oasis:names:tc:opendocument:xmlns:presentation:1.0"><office:body><office:presentation><draw:page><draw:frame presentation:class="title"><draw:text-box><text:p>Deck title</text:p></draw:text-box></draw:frame><draw:frame><draw:text-box><text:p>Body</text:p></draw:text-box></draw:frame><presentation:notes><draw:frame><draw:text-box><text:p>Remember this</text:p></draw:text-box></draw:frame></presentation:notes></draw:page></office:presentation></office:body></office:document-content>`,
	})
	preview, converted := DocumentMarkdown(content, MediaODP)
	if !converted || preview != "## Deck title\n\nBody\n\n> Remember this" {
		t.Fatalf("ODP preview = %q, converted = %v", preview, converted)
	}
}

func TestDocumentMarkdownRTFHandlesDestinationsEncodingAndBounds(t *testing.T) {
	input := []byte("{\\rtf1\\ansi\\ansicpg1252{\\fonttbl{\\f0 Arial;}}Hello \\'80 \\u8212? {\\*\\comment hidden}\\par Next}")
	preview, converted := DocumentMarkdown(input, "text/rtf")
	if !converted || preview != "Hello € —\n\nNext" {
		t.Fatalf("RTF preview = %q, converted = %v", preview, converted)
	}
	large := []byte("{\\rtf1 " + strings.Repeat("é", maxPreviewCharacters+100) + "}")
	preview, converted = DocumentMarkdown(large, MediaRTF)
	if !converted || utf8.RuneCountInString(preview) != maxPreviewCharacters || !utf8.ValidString(preview) {
		t.Fatalf("bounded RTF has %d characters, converted = %v", utf8.RuneCountInString(preview), converted)
	}
}

func TestDocumentMarkdownRejectsExcessiveXMLDepth(t *testing.T) {
	for _, mediaType := range []string{MediaDOCX, MediaODT, MediaPPTX, MediaODP, MediaRTF} {
		if !IsDocument(mediaType) {
			t.Fatalf("%s is not recognized", mediaType)
		}
		if preview, converted := DocumentMarkdown([]byte("not a document"), mediaType); converted || preview != "" {
			t.Fatalf("malformed %s converted to %q", mediaType, preview)
		}
	}
	if IsDocument("application/octet-stream") {
		t.Fatal("binary content is recognized as a document")
	}
	body := strings.Repeat("<w:s>", maxDocumentXMLDepth+1) + strings.Repeat("</w:s>", maxDocumentXMLDepth+1)
	content := zipContent(t, map[string]string{
		"word/document.xml": `<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>` + body + `</w:body></w:document>`,
	})
	if preview, converted := DocumentMarkdown(content, MediaDOCX); converted || preview != "" {
		t.Fatalf("deep DOCX converted to %q", preview)
	}
}
