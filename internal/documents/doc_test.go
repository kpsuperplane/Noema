package documents

import (
	"encoding/binary"
	"os"
	"strings"
	"testing"
)

func docFixture(t *testing.T, name string) []byte {
	t.Helper()
	content, err := os.ReadFile("testdata/doc/" + name)
	if err != nil {
		t.Fatal(err)
	}
	return content
}

func TestDocumentMarkdownDOCKeepsProductionStructure(t *testing.T) {
	preview, converted := DocumentMarkdown(docFixture(t, "text.doc"), MediaDOC)
	want := `# Fixture Document

Plain paragraph with **bold**, *italic*, and ~~struck~~ runs.

**Style-bold paragraph with a** NotBold-styled span **inside.**

## Lists

1. First numbered

2. Second numbered

   - a) Alpha sub one

   - b) Alpha sub two

        - i. Roman sub sub

3. Third numbered

Interrupting paragraph between lists.

4. Fourth, continuing the count

- IV. Roman starting at four

- I. Roman five

- Bullet one

- Bullet two

  - Nested bullet

## Table

|  |  |  |
| --- | --- | --- |
| Wide head |  | End |
| Tall | B2 | C2 |
|  | B3 | C3 |

## Notes and special text

Music clef 𝄞 appears before this footnote[^1] reference.

An endnote follows here[^2].

Persian with ZWNJ: می‌خواهم. Family emoji: 👨‍👩‍👧.

Markdown specials: \*stars* \_under_ \[bracket] \` + "`tick`" + ` #hash 1. dotted | pipe.

## Links and anchors

External link to [example](https://example.com/page).

Relative link to [a sibling file](../../fixture-src/sibling.odt).

This plain paragraph carries a bookmark.

Jump to the bookmarked paragraph.

## Objects

Inline image:  done.

Text box:  after the box.

## Quote and code

Value below one millionth: 0.0000004 should survive.

[^1]: Footnote after an astral character.

[^2]: Endnote body text.`
	if !converted || preview != want {
		t.Fatalf("DOC converted = %v\n--- got ---\n%s\n--- want ---\n%s", converted, preview, want)
	}
}

func TestDocumentMarkdownDOCCodePages(t *testing.T) {
	for _, test := range []struct{ name, want string }{
		{"cyrillic.doc", "Привет, мир!\n\nВторой абзац по-русски."},
		{"shiftjis.doc", "こんにちは世界。\n\n日本語の段落です。"},
	} {
		t.Run(test.name, func(t *testing.T) {
			got, ok := DocumentMarkdown(docFixture(t, test.name), MediaDOC)
			if !ok || got != test.want {
				t.Fatalf("preview = %q, converted = %v", got, ok)
			}
		})
	}
}

func TestDocumentMarkdownDOCBlockStylesAndRTFSignature(t *testing.T) {
	want := "Body before.\n\n```\nfn main() {\n}\n```\n\n> A quotation.\n\nBody after."
	if got, ok := DocumentMarkdown(docFixture(t, "blockstyle.doc"), MediaDOC); !ok || got != want {
		t.Fatalf("preview = %q, converted = %v", got, ok)
	}
	if got, ok := DocumentMarkdown([]byte("{\\rtf1 legacy}"), MediaDOC); !ok || got != "legacy" {
		t.Fatalf("RTF-backed DOC = %q, converted = %v", got, ok)
	}
}

func TestDocumentMarkdownDOCRejectsMalformedAndExcessivePieces(t *testing.T) {
	if got, ok := DocumentMarkdown(docFixture(t, "truncated.doc"), MediaDOC); ok || got != "" {
		t.Fatalf("truncated DOC converted to %q", got)
	}
	pieces := 200_001
	plc := make([]byte, 4+pieces*12)
	clx := make([]byte, 5+len(plc))
	clx[0] = 2
	binary.LittleEndian.PutUint32(clx[1:], uint32(len(plc)))
	if _, _, err := parseDOCClx(clx, 0, len(clx)); err == nil {
		t.Fatal("excessive piece table was accepted")
	}
	large := []byte("{\\rtf1 " + strings.Repeat("a", maxPreviewCharacters+100) + "}")
	if got, ok := DocumentMarkdown(large, MediaDOC); !ok || len(got) != maxPreviewCharacters {
		t.Fatalf("bounded DOC has %d bytes, converted = %v", len(got), ok)
	}
}
