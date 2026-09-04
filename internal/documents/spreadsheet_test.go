package documents

import (
	"archive/zip"
	"bytes"
	"encoding/base64"
	"fmt"
	"strings"
	"testing"
	"unicode/utf8"
)

func TestSpreadsheetMarkdownXLS(t *testing.T) {
	content, err := base64.StdEncoding.DecodeString(xlsFixture)
	if err != nil {
		t.Fatal(err)
	}
	preview, converted := SpreadsheetMarkdown(content, MediaXLS)
	if !converted || preview != "|  |\n| --- |\n| 0.3402777777777778 |\n| 0.3402777777777778 |" {
		t.Fatalf("XLS preview = %q, converted = %v", preview, converted)
	}
}

func TestSpreadsheetMarkdownOpenFormats(t *testing.T) {
	xlsx := zipContent(t, map[string]string{
		"xl/workbook.xml":            `<?xml version="1.0"?><workbook xmlns:r="relationships"><sheets><sheet name="Data" r:id="rId1"/><sheet name="Archive" r:id="rId2"/></sheets></workbook>`,
		"xl/_rels/workbook.xml.rels": `<?xml version="1.0"?><Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Target="worksheets/sheet2.xml"/></Relationships>`,
		"xl/sharedStrings.xml":       `<?xml version="1.0"?><sst><si><t>Name</t></si><si><r><t>A</t></r><r><t>lpha</t></r></si></sst>`,
		"xl/worksheets/sheet1.xml":   `<?xml version="1.0"?><worksheet><sheetData><row><c r="A1" t="s"><v>0</v></c><c r="B1" t="inlineStr"><is><t>Count</t></is></c></row><row><c r="A2" t="s"><v>1</v></c><c r="B2"><v>2</v></c></row></sheetData></worksheet>`,
		"xl/worksheets/sheet2.xml":   `<?xml version="1.0"?><worksheet><sheetData><row><c r="A1" t="inlineStr"><is><t>A|B&#10;C ![x](https://example.com/a.png)&lt;img&gt;</t></is></c><c r="B1" t="b"><v>1</v></c></row></sheetData></worksheet>`,
	})
	ods := zipContent(t, map[string]string{
		"content.xml": `<?xml version="1.0"?><office:document-content xmlns:office="office" xmlns:table="table" xmlns:text="text"><office:body><office:spreadsheet><table:table table:name="Data"><table:table-header-rows><table:table-row><table:table-cell office:value-type="string"><text:p>Name</text:p></table:table-cell><table:table-cell office:value-type="string" office:string-value="Count"/></table:table-row></table:table-header-rows><table:table-row><table:table-cell office:value-type="string"><text:p>Alpha</text:p></table:table-cell><table:table-cell office:value-type="float" office:value="2"/></table:table-row></table:table></office:spreadsheet></office:body></office:document-content>`,
	})

	tests := []struct {
		name      string
		mediaType string
		content   []byte
		want      string
	}{
		{
			name: "xlsx", mediaType: MediaXLSX + "; charset=binary", content: xlsx,
			want: "## Data\n\n| Name | Count |\n| --- | --- |\n| Alpha | 2 |\n## Archive\n\n|  |  |\n| --- | --- |\n| A\\|B<br>C \\!\\[x\\](https://example.com/a.png)\\<img> | TRUE |",
		},
		{
			name: "ods", mediaType: MediaODS, content: ods,
			want: "| Name | Count |\n| --- | --- |\n| Alpha | 2 |",
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			preview, converted := SpreadsheetMarkdown(test.content, test.mediaType)
			if !converted || preview != test.want {
				t.Fatalf("preview = %q, converted = %v", preview, converted)
			}
		})
	}
}

func TestSpreadsheetMarkdownRejectsInvalidAndBoundsOutput(t *testing.T) {
	for _, mediaType := range []string{MediaXLSX, MediaXLS, MediaODS, "application/octet-stream"} {
		if preview, converted := SpreadsheetMarkdown([]byte("not a workbook"), mediaType); converted || preview != "" {
			t.Fatalf("malformed %s converted to %q", mediaType, preview)
		}
	}
	bomb := zipContent(t, map[string]string{"content.xml": strings.Repeat("x", maxArchivePartBytes+1)})
	if _, converted := SpreadsheetMarkdown(bomb, MediaODS); converted {
		t.Fatal("oversized archive part converted")
	}
	large := zipContent(t, map[string]string{
		"xl/workbook.xml":            `<?xml version="1.0"?><workbook xmlns:r="relationships"><sheets><sheet name="Data" r:id="rId1"/></sheets></workbook>`,
		"xl/_rels/workbook.xml.rels": `<?xml version="1.0"?><Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>`,
		"xl/worksheets/sheet1.xml":   `<?xml version="1.0"?><worksheet><sheetData><row><c r="A1" t="inlineStr"><is><t>` + strings.Repeat("é", maxPreviewCharacters) + `</t></is></c></row></sheetData></worksheet>`,
	})
	preview, converted := SpreadsheetMarkdown(large, MediaXLSX)
	if !converted || utf8.RuneCountInString(preview) != maxPreviewCharacters || !utf8.ValidString(preview) {
		t.Fatalf("bounded preview has %d characters, converted = %v", utf8.RuneCountInString(preview), converted)
	}
}

func TestSpreadsheetMarkdownAcceptsCurrentDocumentLimit(t *testing.T) {
	parts := map[string]string{
		"xl/workbook.xml":            `<workbook xmlns:r="relationships"><sheets><sheet name="Data" r:id="rId1"/></sheets></workbook>`,
		"xl/_rels/workbook.xml.rels": `<Relationships><Relationship Id="rId1" Target="worksheets/sheet1.xml"/></Relationships>`,
		"xl/worksheets/sheet1.xml":   `<worksheet><sheetData><row><c r="A1" t="inlineStr"><is><t>Name</t></is></c></row><row><c r="A2" t="inlineStr"><is><t>kept</t></is></c></row></sheetData></worksheet>`,
	}
	var content bytes.Buffer
	archive := zip.NewWriter(&content)
	for name, body := range parts {
		file, err := archive.Create(name)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := file.Write([]byte(body)); err != nil {
			t.Fatal(err)
		}
	}
	for index := range 2 {
		header := &zip.FileHeader{Name: fmt.Sprintf("padding-%d.bin", index), Method: zip.Store}
		padding, err := archive.CreateHeader(header)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := padding.Write(make([]byte, 5*1024*1024)); err != nil {
			t.Fatal(err)
		}
	}
	if err := archive.Close(); err != nil {
		t.Fatal(err)
	}
	preview, converted := SpreadsheetMarkdown(content.Bytes(), MediaXLSX)
	if !converted || preview != "| Name |\n| --- |\n| kept |" {
		t.Fatalf("preview = %q, converted = %v", preview, converted)
	}
}

func zipContent(t *testing.T, parts map[string]string) []byte {
	t.Helper()
	var content bytes.Buffer
	archive := zip.NewWriter(&content)
	for name, body := range parts {
		file, err := archive.Create(name)
		if err != nil {
			t.Fatal(err)
		}
		if _, err := file.Write([]byte(body)); err != nil {
			t.Fatal(err)
		}
	}
	if err := archive.Close(); err != nil {
		t.Fatal(err)
	}
	return content.Bytes()
}

const xlsFixture = "0M8R4KGxGuEAAAAAAAAAAAAAAAAAAAAAOwADAP7/CQAGAAAAAAAAAAAAAAABAAAACAAAAAAAAAAAEAAAAgAAAAEAAAD+////AAAAAAAAAAD////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////9//////////7///8EAAAABQAAAAYAAAAHAAAA/v///wkAAAD+/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////1IAbwBvAHQAIABFAG4AdAByAHkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWAAUA////////////////AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA/v///wAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD///////////////8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAD+////AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAP///////////////wAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAP7///8AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA////////////////AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA/v///wAAAAAAAAAAAQAAAAIAAAADAAAABAAAAAUAAAAGAAAABwAAAAgAAAAJAAAACgAAAAsAAAAMAAAADQAAAA4AAAAPAAAAEAAAABEAAAASAAAAEwAAABQAAAAVAAAAFgAAABcAAAAYAAAAGQAAABoAAAAbAAAAHAAAAP7///8eAAAA/v////7///8hAAAAIgAAAP7///8kAAAA/v////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////8JCBAAAAYFALsNzAcAAAAABgAAAOEAAgCwBMEAAgAAAOIAAABcAHAABAAAQ2FsYyAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIEIAAgCwBGEBAgAAAMABAAA9AQIAAQCcAAIADgCvAQIAAAC8AQIAAAA9ABIAAAAAAABAACA4AAAAAAABAPQBQAACAAAAjQACAAAAIgACAAAADgACAAEAtwECAAAA2gACAAAAMQAaAMgAAAD/f5ABAAAAAgAABQFBAHIAaQBhAGwAMQAaAMgAAAD/f5ABAAAAAAAABQFBAHIAaQBhAGwAMQAaAMgAAAD/f5ABAAAAAAAABQFBAHIAaQBhAGwAMQAaAMgAAAD/f5ABAAAAAAAABQFBAHIAaQBhAGwAHgQMAKQABwAAR2VuZXJhbB4EDQClAAgAAEhIOk1NOlNT4AAUAAAApAD1/yAAAAAAAAAAAAAAAMAg4AAUAAEAAAD1/yAAAPQAAAAAAAAAAMAg4AAUAAEAAAD1/yAAAPQAAAAAAAAAAMAg4AAUAAIAAAD1/yAAAPQAAAAAAAAAAMAg4AAUAAIAAAD1/yAAAPQAAAAAAAAAAMAg4AAUAAAAAAD1/yAAAPQAAAAAAAAAAMAg4AAUAAAAAAD1/yAAAPQAAAAAAAAAAMAg4AAUAAAAAAD1/yAAAPQAAAAAAAAAAMAg4AAUAAAAAAD1/yAAAPQAAAAAAAAAAMAg4AAUAAAAAAD1/yAAAPQAAAAAAAAAAMAg4AAUAAAAAAD1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"
