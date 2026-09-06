package documents

import (
	"fmt"
	"strings"
	"unicode/utf16"
	"unicode/utf8"
)

const docListLevels = 9

type docMarker byte

const (
	docBullet docMarker = iota
	docDecimal
	docUpperRoman
	docLowerRoman
	docUpperAlpha
	docLowerAlpha
	docNone
)

func docMarkerFor(n byte) docMarker {
	switch n {
	case 0:
		return docDecimal
	case 1:
		return docUpperRoman
	case 2:
		return docLowerRoman
	case 3:
		return docUpperAlpha
	case 4:
		return docLowerAlpha
	case 23:
		return docBullet
	case 0xff:
		return docNone
	default:
		return docDecimal
	}
}
func (m docMarker) ordered() bool { return m != docBullet && m != docNone }
func (m docMarker) ordinal(n uint64) string {
	switch m {
	case docUpperRoman:
		return strings.ToUpper(docRoman(n))
	case docLowerRoman:
		return docRoman(n)
	case docUpperAlpha:
		return strings.ToUpper(docAlpha(n))
	case docLowerAlpha:
		return docAlpha(n)
	default:
		return fmt.Sprint(n)
	}
}
func (m docMarker) label(n uint64) string {
	if m == docBullet {
		return "-"
	}
	return m.ordinal(n) + "."
}
func docAlpha(n uint64) string {
	if n == 0 {
		return "0"
	}
	var b []byte
	for n > 0 {
		n--
		b = append(b, 'a'+byte(n%26))
		n /= 26
	}
	for i, j := 0, len(b)-1; i < j; i, j = i+1, j-1 {
		b[i], b[j] = b[j], b[i]
	}
	return string(b)
}
func docRoman(n uint64) string {
	if n == 0 || n > 3999 {
		return fmt.Sprint(n)
	}
	var b strings.Builder
	for _, x := range []struct {
		n uint64
		s string
	}{{1000, "m"}, {900, "cm"}, {500, "d"}, {400, "cd"}, {100, "c"}, {90, "xc"}, {50, "l"}, {40, "xl"}, {10, "x"}, {9, "ix"}, {5, "v"}, {4, "iv"}, {1, "i"}} {
		for n >= x.n {
			b.WriteString(x.s)
			n -= x.n
		}
	}
	return b.String()
}

type docNumPart struct {
	level int
	text  string
}
type docLevel struct {
	marker  docMarker
	start   uint64
	restart *uint32
	pattern []docNumPart
	legal   bool
}

func defaultDOCLevel() docLevel { return docLevel{marker: docBullet, start: 1} }

type docListDef struct {
	lsid     uint32
	levels   [docListLevels]docLevel
	override [docListLevels]*uint64
}
type docLists map[uint16]docListDef

func unknownDOCList(ilfo uint16) docListDef {
	d := docListDef{lsid: ^uint32(ilfo)}
	for i := range d.levels {
		d.levels[i] = defaultDOCLevel()
	}
	return d
}

func parseDOCLists(word, table []byte) docLists {
	out := docLists{}
	lfc, ll := int(u32(word, 0x2e2)), int(u32(word, 0x2e6))
	ofc, ol := int(u32(word, 0x2ea)), int(u32(word, 0x2ee))
	if ll == 0 {
		return out
	}
	by := parseDOCListDefs(table, lfc, ll)
	lfo, ok := boundedSlice(table, ofc, ol)
	if !ok || len(lfo) < 4 {
		return out
	}
	count := int(u32(lfo, 0))
	if count > 65_535 {
		return out
	}
	pos := 4
	refs := make([]struct {
		lsid uint32
		n    int
	}, 0, count)
	for i := 0; i < count; i++ {
		r, ok := boundedSlice(lfo, pos, 16)
		if !ok {
			return out
		}
		refs = append(refs, struct {
			lsid uint32
			n    int
		}{u32(r, 0), int(r[12])})
		pos += 16
	}
	for i, ref := range refs {
		ilfo := uint16(i + 1)
		levels, ok := by[ref.lsid]
		d := docListDef{lsid: ref.lsid, levels: levels}
		if !ok {
			d = unknownDOCList(ilfo)
		}
		for j := 0; j < ref.n; j++ {
			r, ok := boundedSlice(lfo, pos, 8)
			if !ok {
				break
			}
			start := uint64(u32(r, 0))
			bits := r[4]
			level := int(bits & 0xf)
			pos += 8
			if bits&0x20 != 0 {
				lvl, next, ok := parseDOCLevel(lfo, pos)
				if !ok {
					break
				}
				pos = next
				if level < docListLevels {
					if bits&0x10 != 0 {
						x := lvl.start
						d.override[level] = &x
					}
					d.levels[level] = lvl
				}
			} else if bits&0x10 != 0 && level < docListLevels {
				x := start
				d.levels[level].start = x
				d.override[level] = &x
			}
		}
		out[ilfo] = d
	}
	return out
}
func parseDOCListDefs(table []byte, fc, lcb int) map[uint32][docListLevels]docLevel {
	out := map[uint32][docListLevels]docLevel{}
	plf, ok := boundedSlice(table, fc, len(table)-fc)
	if !ok || len(plf) < 2 {
		return out
	}
	n := int(u16(plf, 0))
	if n > 65_535 {
		return out
	}
	type head struct {
		id     uint32
		simple bool
	}
	heads := make([]head, 0, n)
	pos := 2
	for i := 0; i < n; i++ {
		r, ok := boundedSlice(plf, pos, 28)
		if !ok {
			return out
		}
		heads = append(heads, head{u32(r, 0), r[26]&1 != 0})
		pos += 28
	}
	pos = lcb
	for _, h := range heads {
		var levels [docListLevels]docLevel
		for i := range levels {
			levels[i] = defaultDOCLevel()
		}
		limit := docListLevels
		if h.simple {
			limit = 1
		}
		for i := 0; i < limit; i++ {
			lvl, next, ok := parseDOCLevel(plf, pos)
			if !ok {
				return out
			}
			levels[i] = lvl
			pos = next
		}
		if h.simple {
			for i := 1; i < docListLevels; i++ {
				levels[i] = levels[0]
			}
		}
		out[h.id] = levels
	}
	return out
}
func parseDOCLevel(b []byte, pos int) (docLevel, int, bool) {
	r, ok := boundedSlice(b, pos, len(b)-pos)
	if !ok || len(r) < 28 {
		return docLevel{}, pos, false
	}
	start := uint64(u32(r, 0))
	marker := docMarkerFor(r[4])
	legal, noRestart := r[5]&4 != 0, r[5]&8 != 0
	offsets := r[6:15]
	next := 28 + int(r[25]) + int(r[24])
	if next+2 > len(r) {
		return docLevel{}, pos, false
	}
	n := int(u16(r, next))
	next += 2
	if n > 4096 || next+n*2 > len(r) {
		return docLevel{}, pos, false
	}
	var parts []docNumPart
	var literal strings.Builder
	flush := func() {
		if literal.Len() > 0 {
			parts = append(parts, docNumPart{level: -1, text: literal.String()})
			literal.Reset()
		}
	}
	if marker.ordered() {
		units := bytesToU16(r[next : next+n*2])
		for i, ch := range units {
			placeholder := false
			if ch <= 8 {
				for _, x := range offsets {
					if x == 0 {
						break
					}
					if int(x) == i+1 {
						placeholder = true
						break
					}
				}
			}
			if placeholder {
				flush()
				parts = append(parts, docNumPart{level: int(ch)})
			} else {
				rr := utf16.Decode([]uint16{ch})[0]
				if rr >= 0x20 {
					literal.WriteRune(rr)
				}
			}
		}
		flush()
	}
	next += n * 2
	var restart *uint32
	if noRestart {
		x := uint32(r[26])
		restart = &x
	}
	return docLevel{marker, start, restart, parts, legal}, pos + next, true
}

type docCounterState struct {
	values  [docListLevels]uint64
	started [docListLevels]bool
}
type docCounters struct {
	states map[uint32]docCounterState
	used   map[[2]uint16]bool
}

func (c *docCounters) next(ilfo uint16, d docListDef, level int) (uint64, string) {
	if c.states == nil {
		c.states = map[uint32]docCounterState{}
		c.used = map[[2]uint16]bool{}
	}
	level = min(level, docListLevels-1)
	st := c.states[d.lsid]
	key := [2]uint16{ilfo, uint16(level)}
	first := !c.used[key]
	c.used[key] = true
	v := d.levels[level].start
	if d.override[level] != nil && first {
		v = *d.override[level]
	} else if st.started[level] {
		v = st.values[level] + 1
	}
	st.values[level] = v
	st.started[level] = true
	for i := level + 1; i < docListLevels; i++ {
		r := d.levels[i].restart
		if r == nil || uint32(level) < *r {
			st.started[i] = false
		}
	}
	c.states[d.lsid] = st
	return v, docComposite(d, st, level)
}
func docComposite(d docListDef, st docCounterState, level int) string {
	l := d.levels[level]
	if len(l.pattern) == 0 {
		return ""
	}
	var b strings.Builder
	for _, p := range l.pattern {
		if p.level < 0 {
			b.WriteString(p.text)
			continue
		}
		i := min(p.level, docListLevels-1)
		m := d.levels[i].marker
		if l.legal {
			m = docDecimal
		}
		v := d.levels[i].start
		if st.started[i] {
			v = st.values[i]
		}
		b.WriteString(m.ordinal(v))
	}
	s := b.String()
	if s == l.marker.label(st.values[level]) {
		return ""
	}
	return s
}

type docListEntry struct {
	level   int
	id      uint32
	marker  docMarker
	number  uint64
	label   string
	inlines []docInline
}

func renderDOCList(entries []docListEntry, notes map[string]int) string {
	if len(entries) == 0 {
		return ""
	}
	minLevel := entries[0].level
	for _, e := range entries {
		minLevel = min(minLevel, e.level)
	}
	var out []string
	for i := 0; i < len(entries); {
		if entries[i].level > minLevel {
			i++
			continue
		}
		e := entries[i]
		marker := "- "
		if e.label != "" {
			marker = "- " + escapeMarkdown(e.label, false) + " "
		} else if e.marker == docDecimal {
			marker = fmt.Sprintf("%d. ", e.number)
		} else if e.marker != docBullet {
			marker = "- " + e.marker.label(e.number) + " "
		}
		body := renderDOCInlines(e.inlines, false, notes)
		j := i + 1
		for j < len(entries) && entries[j].level > minLevel {
			j++
		}
		if j > i+1 {
			sub := renderDOCList(entries[i+1:j], notes)
			indent := strings.Repeat(" ", utf8.RuneCountInString(marker))
			lines := strings.Split(sub, "\n")
			for i, line := range lines {
				if line != "" {
					lines[i] = indent + line
				}
			}
			body += "\n\n" + strings.Join(lines, "\n")
		}
		out = append(out, marker+body)
		i = j
	}
	return strings.Join(out, "\n\n")
}
