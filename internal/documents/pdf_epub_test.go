package documents

import (
	"bytes"
	"fmt"
	"strings"
	"testing"
	"unicode/utf8"
)

func TestDocumentMarkdownPDFKeepsPageOrderAndEscapesMarkdown(t *testing.T) {
	content := testPDF(t, "First *page*", "Second page")
	preview, converted := DocumentMarkdown(content, MediaPDF+"; charset=binary")
	if !converted || preview != "First \\*page\\*\n\nSecond page" {
		t.Fatalf("PDF preview = %q, converted = %v", preview, converted)
	}
	if !IsDocument(MediaPDF) {
		t.Fatal("PDF is not recognized")
	}
}

func TestDocumentMarkdownPDFRejectsMalformedAndTextlessFiles(t *testing.T) {
	for _, content := range [][]byte{[]byte("%PDF-1.7\nnot a PDF"), testPDF(t, "")} {
		if preview, converted := DocumentMarkdown(content, MediaPDF); converted || preview != "" {
			t.Fatalf("invalid PDF converted to %q", preview)
		}
	}
}

func TestDocumentMarkdownEPUBUsesTitleAndSpineOrder(t *testing.T) {
	content := testEPUB(t, `
		<metadata><dc:title xmlns:dc="http://purl.org/dc/elements/1.1/">Book *title*</dc:title></metadata>
		<manifest>
			<item id="one" href="text/chapter%201.xhtml" media-type="application/xhtml+xml"/>
			<item id="two" href="text/chapter2.xhtml" media-type="application/xhtml+xml"/>
		</manifest>
		<spine><itemref idref="two"/><itemref idref="one"/></spine>`, map[string]string{
		"OPS/text/chapter%201.xhtml": "unused encoded path",
		"OPS/text/chapter 1.xhtml":   `<html xmlns="http://www.w3.org/1999/xhtml"><body><ul><li><p>First *item*</p></li></ul></body></html>`,
		"OPS/text/chapter2.xhtml": `<html xmlns="http://www.w3.org/1999/xhtml"><head><style>.hidden { display:none }</style></head><body>
			<h2>Second</h2><p>Visible <em>text</em></p><script>secret()</script><p style="display: none">hidden</p>
			<table><tr><th>Name</th><th>Count</th></tr><tr><td>A|B</td><td>2</td></tr></table>
		</body></html>`,
	})
	preview, converted := DocumentMarkdown(content, MediaEPUB)
	want := "# Book \\*title\\*\n\n## Second\n\nVisible text\n\n| Name | Count |\n| --- | --- |\n| A\\|B | 2 |\n\n- First \\*item\\*"
	if !converted || preview != want {
		t.Fatalf("EPUB preview = %q, converted = %v", preview, converted)
	}
	if !IsDocument(MediaEPUB) {
		t.Fatal("EPUB is not recognized")
	}
}

func TestDocumentMarkdownEPUBSkipsOneUnusableChapter(t *testing.T) {
	content := testEPUB(t, `
		<manifest><item id="bad" href="bad.xhtml"/><item id="good" href="good.xhtml"/></manifest>
		<spine><itemref idref="bad"/><itemref idref="good"/></spine>`, map[string]string{
		"OPS/bad.xhtml":  `<html><body><p>broken`,
		"OPS/good.xhtml": `<html><body><p>Kept</p></body></html>`,
	})
	preview, converted := DocumentMarkdown(content, MediaEPUB)
	if !converted || preview != "Kept" {
		t.Fatalf("EPUB preview = %q, converted = %v", preview, converted)
	}

	allBad := testEPUB(t, `
		<manifest><item id="bad" href="%2e%2e/bad.xhtml"/></manifest>
		<spine><itemref idref="bad"/></spine>`, nil)
	if preview, converted = DocumentMarkdown(allBad, MediaEPUB); converted || preview != "" {
		t.Fatalf("unusable EPUB converted to %q", preview)
	}
}

func TestDocumentMarkdownEPUBBoundsUnicodeOutput(t *testing.T) {
	content := testEPUB(t, `<manifest><item id="chapter" href="chapter.xhtml"/></manifest><spine><itemref idref="chapter"/></spine>`, map[string]string{
		"OPS/chapter.xhtml": `<html><body><p>` + strings.Repeat("é", maxPreviewCharacters+100) + `</p></body></html>`,
	})
	preview, converted := DocumentMarkdown(content, MediaEPUB)
	if !converted || utf8.RuneCountInString(preview) != maxPreviewCharacters || !utf8.ValidString(preview) {
		t.Fatalf("bounded EPUB has %d characters, converted = %v", utf8.RuneCountInString(preview), converted)
	}
}

func testEPUB(t *testing.T, opfBody string, parts map[string]string) []byte {
	t.Helper()
	content := map[string]string{
		"META-INF/container.xml": `<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><rootfiles><rootfile full-path="OPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles></container>`,
		"OPS/content.opf":        `<package xmlns="http://www.idpf.org/2007/opf">` + opfBody + `</package>`,
	}
	for name, value := range parts {
		content[name] = value
	}
	return zipContent(t, content)
}

func testPDF(t *testing.T, pages ...string) []byte {
	t.Helper()
	if len(pages) == 0 {
		t.Fatal("test PDF needs one page")
	}
	objects := []string{
		`<< /Type /Catalog /Pages 2 0 R >>`,
		"",
		`<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>`,
	}
	kids := make([]string, 0, len(pages))
	for _, page := range pages {
		pageObject := len(objects) + 1
		contentObject := pageObject + 1
		kids = append(kids, fmt.Sprintf("%d 0 R", pageObject))
		stream := "BT /F1 12 Tf 72 720 Td (" + escapePDFText(page) + ") Tj ET"
		objects = append(objects,
			fmt.Sprintf(`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents %d 0 R >>`, contentObject),
			fmt.Sprintf("<< /Length %d >>\nstream\n%s\nendstream", len(stream), stream),
		)
	}
	objects[1] = fmt.Sprintf("<< /Type /Pages /Kids [%s] /Count %d >>", strings.Join(kids, " "), len(pages))

	var result bytes.Buffer
	result.WriteString("%PDF-1.4\n")
	offsets := make([]int, len(objects)+1)
	for index, object := range objects {
		offsets[index+1] = result.Len()
		fmt.Fprintf(&result, "%d 0 obj\n%s\nendobj\n", index+1, object)
	}
	xref := result.Len()
	fmt.Fprintf(&result, "xref\n0 %d\n0000000000 65535 f \n", len(objects)+1)
	for _, offset := range offsets[1:] {
		fmt.Fprintf(&result, "%010d 00000 n \n", offset)
	}
	fmt.Fprintf(&result, "trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n", len(objects)+1, xref)
	return result.Bytes()
}

func escapePDFText(value string) string {
	value = strings.ReplaceAll(value, `\`, `\\`)
	value = strings.ReplaceAll(value, `(`, `\(`)
	return strings.ReplaceAll(value, `)`, `\)`)
}
