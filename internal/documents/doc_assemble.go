package documents

import (
	"fmt"
	"sort"
	"strings"
	"unicode"
)

type docField struct {
	instruction strings.Builder
	result      []docInline
	separated   bool
}
type docParagraph struct {
	inlines []docInline
	fields  []docField
	text    strings.Builder
	style   docStyle
}

func (p *docParagraph) flush() {
	if p.text.Len() == 0 {
		return
	}
	x := docInline{text: p.text.String(), style: p.style}
	p.text.Reset()
	if len(p.fields) > 0 {
		f := &p.fields[len(p.fields)-1]
		if f.separated {
			f.result = append(f.result, x)
		} else {
			f.instruction.WriteString(x.text)
		}
	} else {
		p.inlines = append(p.inlines, x)
	}
}
func (p *docParagraph) push(r rune, s docStyle) {
	if s != p.style {
		p.flush()
		p.style = s
	}
	p.text.WriteRune(r)
}
func (p *docParagraph) inline(x docInline) {
	p.flush()
	if len(p.fields) > 0 {
		f := &p.fields[len(p.fields)-1]
		if f.separated {
			f.result = append(f.result, x)
		}
	} else {
		p.inlines = append(p.inlines, x)
	}
}
func (p *docParagraph) begin() {
	p.flush()
	if len(p.fields) < 64 {
		p.fields = append(p.fields, docField{})
	}
}
func (p *docParagraph) separate() {
	p.flush()
	if len(p.fields) > 0 {
		p.fields[len(p.fields)-1].separated = true
	}
}
func (p *docParagraph) end() {
	p.flush()
	if len(p.fields) == 0 {
		return
	}
	f := p.fields[len(p.fields)-1]
	p.fields = p.fields[:len(p.fields)-1]
	link := docHyperlink(f.instruction.String())
	for _, x := range f.result {
		if link != "" {
			x.link = link
		}
		p.inline(x)
	}
}
func (p *docParagraph) finish() []docInline {
	p.flush()
	for len(p.fields) > 0 {
		p.end()
	}
	return p.inlines
}

type docStyledRun struct {
	kind       byte
	paragraphs [][]docInline
}

func (s *docStyledRun) push(kind byte, in []docInline, blocks *[]docBlock) {
	if s.kind != kind {
		s.flush(blocks)
		s.kind = kind
	}
	s.paragraphs = append(s.paragraphs, in)
}
func (s *docStyledRun) flush(blocks *[]docBlock) {
	if s.kind == 0 {
		return
	}
	if s.kind == 'c' {
		lines := make([]string, len(s.paragraphs))
		for i, p := range s.paragraphs {
			lines[i] = docPlain(p)
		}
		for len(lines) > 0 && strings.TrimSpace(lines[0]) == "" {
			lines = lines[1:]
		}
		for len(lines) > 0 && strings.TrimSpace(lines[len(lines)-1]) == "" {
			lines = lines[:len(lines)-1]
		}
		if len(lines) > 0 {
			*blocks = append(*blocks, docBlock{kind: 'c', code: strings.Join(lines, "\n")})
		}
	} else {
		for _, p := range s.paragraphs {
			if !docInlinesEmpty(p) {
				*blocks = append(*blocks, docBlock{kind: 'q', inlines: p})
			}
		}
	}
	s.kind = 0
	s.paragraphs = nil
}

func (p *docParser) piecePRM(index int) []byte {
	if index < 0 || index >= len(p.pieces) {
		return nil
	}
	i := p.pieces[index].prc
	if i < 0 || i >= len(p.prcs) {
		return nil
	}
	return p.prcs[i]
}
func (p *docParser) charStyle(fc uint32, index int) docStyle {
	istd := uint16(0)
	if x := p.papx.lookup(fc); x != nil {
		istd = x.istd
	}
	chpx := []byte(nil)
	if x := p.chpx.lookup(fc); x != nil {
		chpx = x.chpx
	}
	if id, ok := docChpxStyle(chpx); ok {
		istd = id
	}
	base := p.styles.get(istd).chp
	s := applyDOCChpx(chpx, base, base)
	if index < len(p.text.piece) {
		s = applyDOCChpx(p.piecePRM(int(p.text.piece[index])), s, base)
	}
	return s
}
func (p *docParser) effectivePap(fc uint32, index int) (uint16, docPap) {
	istd := uint16(0)
	d := docPap{}
	if x := p.papx.lookup(fc); x != nil {
		istd = x.istd
		d = x.pap
	}
	s := p.styles.get(istd)
	d = s.pap.over(d)
	if index < len(p.text.piece) {
		var x docPap
		applyDOCPap(p.piecePRM(int(p.text.piece[index])), nil, &x, 0)
		d = d.over(x)
	}
	return istd, d
}

type docTableState struct {
	cells []docBlock
	row   [][]docBlock
	rows  []docRow
}
type docRow struct {
	cells [][]docBlock
	tap   *docTap
}

func (t *docTableState) cell(blocks []docBlock) { t.row = append(t.row, blocks) }
func (t *docTableState) finishRow(tap *docTap) {
	if len(t.row) > 0 {
		t.rows = append(t.rows, docRow{append([][]docBlock(nil), t.row...), tap})
		t.row = nil
	}
}
func (t *docTableState) flush(blocks *[]docBlock) error {
	if len(t.cells) > 0 {
		t.cell(t.cells)
		t.cells = nil
	}
	t.finishRow(nil)
	if len(t.rows) == 0 {
		return nil
	}
	header := 0
	if t.rows[0].tap != nil && t.rows[0].tap.header {
		header = 1
	}
	grid, err := docTableGrid(t.rows)
	if err != nil {
		return err
	}
	*blocks = append(*blocks, docBlock{kind: 't', level: header, table: grid})
	t.rows = nil
	return nil
}
func docTableGrid(rows []docRow) ([][]docCell, error) {
	var edges []int
	for _, row := range rows {
		if row.tap != nil {
			for _, edge := range row.tap.boundaries[1:] {
				edges = append(edges, int(edge))
			}
		}
	}
	sort.Ints(edges)
	clusters := edges[:0]
	for _, edge := range edges {
		if len(clusters) == 0 || edge-clusters[len(clusters)-1] > 10 {
			clusters = append(clusters, edge)
		}
	}
	width := len(clusters)
	for _, row := range rows {
		width = max(width, len(row.cells))
	}
	if width > 0 && len(rows) > 65_536/width {
		return nil, errInvalidDocument
	}
	grid := make([][]docCell, len(rows))
	active := map[[2]int]bool{}
	for r, row := range rows {
		line := make([]docCell, width)
		col := 0
		next := map[[2]int]bool{}
		for source := 0; source < len(row.cells); source++ {
			blocks := append([]docBlock(nil), row.cells[source]...)
			right := col + 1
			if row.tap != nil && source+1 < len(row.tap.boundaries) {
				right = sort.SearchInts(clusters, int(row.tap.boundaries[source+1])-10) + 1
			}
			right = max(right, col+1)
			if right > len(line) {
				line = append(line, make([]docCell, right-len(line))...)
			}
			tc := docTapCell{}
			if row.tap != nil && source < len(row.tap.cells) {
				tc = row.tap.cells[source]
			}
			if tc.hfirst {
				for source+1 < len(row.cells) && source+1 < len(row.tap.cells) && row.tap.cells[source+1].hcont {
					source++
					blocks = append(blocks, row.cells[source]...)
					if source+1 < len(row.tap.boundaries) {
						right = sort.SearchInts(clusters, int(row.tap.boundaries[source+1])-10) + 1
					}
				}
			}
			key := [2]int{col, right}
			line[col] = blocks
			if tc.vcont && active[key] {
				line[col] = nil
				next[key] = true
			} else if tc.vfirst {
				next[key] = true
			}
			col = right
		}
		grid[r] = line
		active = next
	}
	return grid, nil
}

func (p *docParser) blocks(lo, hi int) ([]docBlock, error) {
	var blocks []docBlock
	var lists []docListEntry
	var styled docStyledRun
	var cellStyled docStyledRun
	var table docTableState
	para := docParagraph{}
	cells := 0
	flushLists := func() {
		if len(lists) > 0 {
			blocks = append(blocks, docBlock{kind: 'l', list: lists})
			lists = nil
		}
	}
	for i := lo; i < hi && i < len(p.text.runes); i++ {
		r, fc := p.text.runes[i], p.text.fcs[i]
		if id, ok := p.noteRefs[i]; ok {
			para.inline(docInline{note: id})
			continue
		}
		switch r {
		case '\r', '\u0007', '\u000c', '\u000e':
			istd, pap := p.effectivePap(fc, i)
			in := para.finish()
			para = docParagraph{}
			cellMark := r == '\u0007'
			inTable := pap.inTable != nil && *pap.inTable
			if inTable || cellMark {
				styled.flush(&blocks)
				flushLists()
				inner := pap.itap != nil && *pap.itap > 1 || pap.innerCell != nil && *pap.innerCell || pap.innerRow != nil && *pap.innerRow
				if inner {
					p.emitCell(istd, in, &table.cells, &cellStyled)
				} else if cellMark && pap.rowEnd != nil && *pap.rowEnd {
					cellStyled.flush(&table.cells)
					table.finishRow(pap.tap)
				} else if cellMark {
					p.emitCell(istd, in, &table.cells, &cellStyled)
					cellStyled.flush(&table.cells)
					table.cell(table.cells)
					table.cells = nil
					cells++
					if cells > 65_536 {
						return nil, errInvalidDocument
					}
				} else {
					p.emitCell(istd, in, &table.cells, &cellStyled)
				}
			} else {
				if err := table.flush(&blocks); err != nil {
					return nil, err
				}
				p.emitParagraph(istd, pap, in, &blocks, &lists, &styled, flushLists)
			}
		case '\u000b':
			para.inline(docInline{breakLine: true})
		case '\u0013':
			para.begin()
		case '\u0014':
			para.separate()
		case '\u0015':
			para.end()
		case '\t':
			para.push(' ', p.charStyle(fc, i))
		case '\u001e':
			para.push('-', p.charStyle(fc, i))
		case '\u0001', '\u0002', '\u0005', '\u0008', '\u001f':
		default:
			if !unicode.IsControl(r) {
				para.push(r, p.charStyle(fc, i))
			}
		}
		if len(blocks) > maxDocumentBlocks || len(table.rows) > 4_096 {
			return nil, errInvalidDocument
		}
	}
	in := para.finish()
	cellStyled.flush(&table.cells)
	if err := table.flush(&blocks); err != nil {
		return nil, err
	}
	if !docInlinesEmpty(in) {
		styled.flush(&blocks)
		flushLists()
		blocks = append(blocks, docBlock{kind: 'p', inlines: in})
	}
	styled.flush(&blocks)
	flushLists()
	return blocks, nil
}
func (p *docParser) emitCell(istd uint16, in []docInline, blocks *[]docBlock, styled *docStyledRun) {
	if k := p.styles.get(istd).block; k != 0 {
		styled.push(k, in, blocks)
	} else {
		styled.flush(blocks)
		if !docInlinesEmpty(in) {
			*blocks = append(*blocks, docBlock{kind: 'p', inlines: in})
		}
	}
}
func (p *docParser) emitParagraph(istd uint16, pap docPap, in []docInline, blocks *[]docBlock, lists *[]docListEntry, styled *docStyledRun, flushLists func()) {
	s := p.styles.get(istd)
	if s.block != 0 {
		flushLists()
		styled.push(s.block, in, blocks)
		return
	}
	if docInlinesEmpty(in) {
		styled.flush(blocks)
		flushLists()
		return
	}
	heading := s.heading
	if heading == 0 && pap.outline != nil {
		heading = *pap.outline
	}
	ilfo := uint16(0)
	if pap.ilfo != nil {
		ilfo = *pap.ilfo
	}
	if heading > 0 {
		styled.flush(blocks)
		flushLists()
		if label := p.headingLabel(ilfo, pap); label != "" {
			in = append([]docInline{{text: label}}, in...)
		}
		for i := range in {
			if s.chp.bold {
				in[i].style.bold = false
			}
			if s.chp.italic {
				in[i].style.italic = false
			}
			if s.chp.strike {
				in[i].style.strike = false
			}
		}
		*blocks = append(*blocks, docBlock{kind: 'h', level: heading, inlines: in})
		return
	}
	if ilfo != 0 && ilfo != 0xf801 {
		level := 0
		if pap.ilvl != nil {
			level = int(*pap.ilvl)
		}
		d, ok := p.lists[ilfo]
		if !ok {
			d = unknownDOCList(ilfo)
		}
		ld := d.levels[min(level, 8)]
		if ld.marker != docNone {
			number, label := uint64(0), ""
			if ld.marker.ordered() {
				number, label = p.counters.next(ilfo, d, level)
			}
			styled.flush(blocks)
			*lists = append(*lists, docListEntry{level, d.lsid, ld.marker, number, label, in})
			return
		}
	}
	styled.flush(blocks)
	flushLists()
	*blocks = append(*blocks, docBlock{kind: 'p', inlines: in})
}
func (p *docParser) headingLabel(ilfo uint16, pap docPap) string {
	if ilfo == 0 || ilfo == 0xf801 {
		return ""
	}
	d, ok := p.lists[ilfo]
	if !ok {
		return ""
	}
	level := 0
	if pap.ilvl != nil {
		level = int(*pap.ilvl)
	}
	ld := d.levels[min(level, 8)]
	if !ld.marker.ordered() {
		return ""
	}
	n, label := p.counters.next(ilfo, d, level)
	if label == "" {
		label = ld.marker.label(n)
	}
	return label + " "
}

func docInlinesEmpty(in []docInline) bool {
	for _, x := range in {
		if strings.TrimSpace(x.text) != "" || x.note != "" {
			return false
		}
	}
	return true
}
func docPlain(in []docInline) string {
	var b strings.Builder
	for _, x := range in {
		b.WriteString(x.text)
		if x.breakLine {
			b.WriteByte('\n')
		}
	}
	return b.String()
}
func docHyperlink(s string) string {
	tokens := docFieldTokens(s)
	if len(tokens) == 0 || !strings.EqualFold(tokens[0], "HYPERLINK") {
		return ""
	}
	var target, anchor string
	for i := 1; i < len(tokens); i++ {
		if strings.HasPrefix(tokens[i], "\\") {
			if i+1 < len(tokens) && strings.EqualFold(tokens[i], "\\l") {
				anchor = tokens[i+1]
				i++
			} else if i+1 < len(tokens) && (strings.EqualFold(tokens[i], "\\o") || strings.EqualFold(tokens[i], "\\t")) {
				i++
			}
			continue
		}
		if target == "" {
			target = tokens[i]
		}
	}
	if target != "" && anchor != "" {
		target += "#" + anchor
	}
	return strings.TrimSpace(target)
}
func docFieldTokens(s string) []string {
	var out []string
	for i := 0; i < len(s); {
		for i < len(s) && s[i] <= ' ' {
			i++
		}
		if i >= len(s) {
			break
		}
		var b strings.Builder
		if s[i] == '"' {
			i++
			for i < len(s) && s[i] != '"' {
				if s[i] == '\\' && i+1 < len(s) && (s[i+1] == '"' || s[i+1] == '\\') {
					i++
				}
				b.WriteByte(s[i])
				i++
			}
			if i < len(s) {
				i++
			}
		} else {
			for i < len(s) && s[i] > ' ' {
				b.WriteByte(s[i])
				i++
			}
		}
		out = append(out, b.String())
	}
	return out
}

func renderDOCDocument(blocks []docBlock, notes []docNote) string {
	valid := map[string]docNote{}
	for _, note := range notes {
		if _, seen := valid[note.id]; len(note.blocks) > 0 && !seen {
			valid[note.id] = note
		}
	}
	numbers := map[string]int{}
	var scan func([]docBlock)
	scan = func(blocks []docBlock) {
		for _, block := range blocks {
			sets := [][]docInline{block.inlines}
			for _, entry := range block.list {
				sets = append(sets, entry.inlines)
			}
			for _, inlines := range sets {
				for _, inline := range inlines {
					if _, ok := valid[inline.note]; ok && inline.note != "" {
						if _, seen := numbers[inline.note]; !seen {
							numbers[inline.note] = len(numbers) + 1
						}
					}
				}
			}
			for _, row := range block.table {
				for _, cell := range row {
					scan(cell)
				}
			}
		}
	}
	scan(blocks)
	for _, note := range notes {
		if _, ok := valid[note.id]; ok {
			if _, seen := numbers[note.id]; !seen {
				numbers[note.id] = len(numbers) + 1
			}
		}
	}
	parts := []string{}
	if body := renderDOCBlocks(blocks, numbers); body != "" {
		parts = append(parts, body)
	}
	ordered := make([]string, len(numbers))
	for id, n := range numbers {
		ordered[n-1] = id
	}
	for _, id := range ordered {
		body := renderDOCBlocks(valid[id].blocks, numbers)
		if body == "" {
			continue
		}
		lines := strings.Split(body, "\n")
		definition := fmt.Sprintf("[^%d]: %s", numbers[id], lines[0])
		for _, line := range lines[1:] {
			definition += "\n"
			if line != "" {
				definition += "    " + line
			}
		}
		parts = append(parts, definition)
	}
	return strings.Join(parts, "\n\n")
}

func renderDOCBlocks(blocks []docBlock, notes map[string]int) string {
	parts := make([]string, 0, len(blocks))
	for _, block := range blocks {
		var rendered string
		switch block.kind {
		case 'p':
			rendered = strings.TrimSpace(renderDOCInlines(block.inlines, false, notes))
		case 'h':
			rendered = strings.Repeat("#", max(1, min(block.level, 6))) + " " + strings.TrimSpace(renderDOCInlines(block.inlines, false, notes))
		case 'q':
			text := strings.TrimSpace(renderDOCInlines(block.inlines, false, notes))
			if text != "" {
				rendered = "> " + strings.ReplaceAll(text, "\n", "\n> ")
			}
		case 'c':
			fence := "```"
			for strings.Contains(block.code, fence) {
				fence += "`"
			}
			rendered = fence + "\n" + strings.TrimRight(block.code, "\n") + "\n" + fence
		case 'l':
			rendered = renderDOCList(block.list, notes)
		case 't':
			rendered = renderDOCTable(block.table, block.level > 0, notes)
		}
		if rendered != "" {
			parts = append(parts, rendered)
		}
	}
	return strings.Join(parts, "\n\n")
}

func renderDOCTable(rows [][]docCell, header bool, notes map[string]int) string {
	if len(rows) == 0 {
		return ""
	}
	width := 0
	for _, row := range rows {
		width = max(width, len(row))
	}
	if width == 0 {
		return ""
	}
	line := func(row []docCell) string {
		var b strings.Builder
		b.WriteByte('|')
		for i := 0; i < width; i++ {
			b.WriteByte(' ')
			if i < len(row) {
				b.WriteString(renderDOCCell(row[i], notes))
			}
			b.WriteString(" |")
		}
		return b.String()
	}
	start := 0
	result := []string{line(make([]docCell, width))}
	if header && len(rows) > 0 {
		result[0] = line(rows[0])
		start = 1
	}
	result = append(result, "|"+strings.Repeat(" --- |", width))
	for _, row := range rows[start:] {
		result = append(result, line(row))
	}
	return strings.Join(result, "\n")
}

func renderDOCCell(blocks []docBlock, notes map[string]int) string {
	parts := make([]string, 0, len(blocks))
	for _, block := range blocks {
		var text string
		switch block.kind {
		case 'p':
			text = renderDOCInlines(block.inlines, true, notes)
		case 'h':
			text = "**" + renderDOCInlines(block.inlines, true, notes) + "**"
		case 'l':
			text = renderDOCList(block.list, notes)
		case 'q':
			text = renderDOCInlines(block.inlines, true, notes)
		case 'c':
			text = "`" + strings.ReplaceAll(block.code, "`", "\\`") + "`"
		case 't':
			text = renderDOCTable(block.table, block.level > 0, notes)
		}
		text = strings.TrimSpace(strings.ReplaceAll(text, "\n", "<br>"))
		if text != "" {
			parts = append(parts, text)
		}
	}
	return strings.ReplaceAll(strings.Join(parts, "<br>"), "|", "\\|")
}
