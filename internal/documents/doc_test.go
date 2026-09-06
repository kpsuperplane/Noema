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

func TestDOCNoteWorkBudgetBoundsAllRanges(t *testing.T) {
	budget := docWorkBudget(maxDocumentBlocks - 2)
	if !budget.add(2) {
		t.Fatal("valid note work was rejected")
	}
	if budget.add(1) {
		t.Fatal("aggregate note work exceeded its bound")
	}
}

func TestDOCTableBoundsFlattensContentAndMerges(t *testing.T) {
	t.Run("aggregate slots", func(t *testing.T) {
		rows := make([]docRow, 257)
		for i := range rows {
			rows[i] = docRow{cells: [][]docBlock{{{kind: 'p', inlines: []docInline{{text: "x"}}}}}, tap: &docTap{boundaries: []int16{0, int16(-32000 + i*11)}}}
		}
		if _, err := docTableGrid(rows); err == nil {
			t.Fatal("oversized table grid was accepted")
		}
	})
	t.Run("content and merges", func(t *testing.T) {
		operand := make([]byte, 3+4*2+3*20)
		operand[2] = 3
		for i, edge := range []uint16{0, 100, 200, 300} {
			binary.LittleEndian.PutUint16(operand[3+i*2:], edge)
		}
		base := 3 + 4*2
		binary.LittleEndian.PutUint16(operand[base:], 2)
		binary.LittleEndian.PutUint16(operand[base+20:], 1)
		tap := parseDOCTap(operand)
		if tap == nil || !tap.cells[0].hfirst || !tap.cells[1].hcont {
			t.Fatal("horizontal merge flags were not parsed")
		}
		rows := []docRow{{cells: [][]docBlock{{{kind: 'p', inlines: []docInline{{text: "left"}}}, {kind: 'p', inlines: []docInline{{text: "second"}}}}, {{kind: 'p', inlines: []docInline{{text: "joined"}}}}, {{kind: 'p', inlines: []docInline{{text: "A"}, {breakLine: true}, {text: "B"}, {note: "fn"}}}}}, tap: tap}}
		grid, err := docTableGrid(rows)
		if err != nil {
			t.Fatal(err)
		}
		got := renderDOCDocument([]docBlock{{kind: 't', table: grid}}, []docNote{{id: "fn", blocks: []docBlock{{kind: 'p', inlines: []docInline{{text: "note"}}}}}})
		want := "|  |  |  |\n| --- | --- | --- |\n| left<br>second<br>joined |  | A<br>B[^1] |\n\n[^1]: note"
		if got != want {
			t.Fatalf("table = %q", got)
		}
	})
}

func TestDOCStyleResolutionKeepsDeepInheritance(t *testing.T) {
	raw := make(map[uint16]docRawStyle, 300)
	raw[0] = docRawStyle{base: 0xfff, heading: 3, chpx: []byte{0x35, 0x08, 1}}
	for i := uint16(1); i < 300; i++ {
		raw[i] = docRawStyle{base: i - 1}
	}
	style := resolveDOCStyles(raw).get(299)
	if style.heading != 3 || !style.chp.bold {
		t.Fatalf("deep style = %#v", style)
	}
}
