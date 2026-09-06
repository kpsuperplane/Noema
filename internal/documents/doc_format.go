package documents

import (
	"sort"
	"strings"
	"unicode/utf16"
)

type docPap struct {
	inTable, rowEnd     *bool
	outline             *int
	ilfo                *uint16
	ilvl                *byte
	itap                *int32
	innerCell, innerRow *bool
	tap                 *docTap
}

func (p docPap) over(n docPap) docPap {
	return docPap{pick(p.inTable, n.inTable), pick(p.rowEnd, n.rowEnd), pick(p.outline, n.outline), pick(p.ilfo, n.ilfo), pick(p.ilvl, n.ilvl), pick(p.itap, n.itap), pick(p.innerCell, n.innerCell), pick(p.innerRow, n.innerRow), pick(p.tap, n.tap)}
}
func pick[T any](base, over *T) *T {
	if over != nil {
		return over
	}
	return base
}

type docTap struct {
	boundaries []int16
	cells      []docTapCell
	header     bool
}
type docTapCell struct{ hfirst, hcont, vfirst, vcont bool }
type docRunProps struct {
	chpx []byte
	istd uint16
	pap  docPap
}
type docRun struct {
	lo, hi uint32
	props  docRunProps
}
type docRuns []docRun

func (r docRuns) lookup(fc uint32) *docRunProps {
	i := sort.Search(len(r), func(i int) bool { return r[i].lo > fc })
	if i == 0 {
		return nil
	}
	x := &r[i-1]
	if fc < x.hi {
		return &x.props
	}
	return nil
}

func parseDOCFKPs(word, table []byte, fib int, pap bool, data []byte) docRuns {
	fc, n := int(u32(word, fib)), int(u32(word, fib+4))
	plc, ok := boundedSlice(table, fc, n)
	if !ok || len(plc) < 8 {
		return nil
	}
	count := (len(plc) - 4) / 8
	if count > 200_000 {
		return nil
	}
	out := make(docRuns, 0, count*4)
	for i := 0; i < count; i++ {
		pn := int(u32(plc, (count+1)*4+i*4) & 0x3fffff)
		page, ok := boundedSlice(word, pn*512, 512)
		if ok {
			parseDOCFKPPage(page, pap, data, &out)
		}
		if len(out) > 200_000 {
			return nil
		}
	}
	sort.Slice(out, func(i, j int) bool { return out[i].lo < out[j].lo })
	return out
}
func parseDOCFKPPage(page []byte, pap bool, data []byte, out *docRuns) {
	count := int(page[511])
	entry := 1
	if pap {
		entry = 13
	}
	if count > 120 {
		return
	}
	for i := 0; i < count; i++ {
		lo, hi := u32(page, i*4), u32(page, (i+1)*4)
		pos := (count+1)*4 + i*entry
		if pos >= 511 {
			break
		}
		bo := int(page[pos])
		props := docRunProps{}
		if bo != 0 {
			off := bo * 2
			if off >= 511 {
				continue
			}
			if !pap {
				n := int(page[off])
				if b, ok := boundedSlice(page, off+1, n); ok {
					props.chpx = append([]byte(nil), b...)
				}
			} else {
				cb := int(page[off])
				start, n := off+1, cb*2-1
				if cb == 0 && off+1 < 511 {
					start, n = off+2, int(page[off+1])*2
				}
				if b, ok := boundedSlice(page, start, n); ok && len(b) >= 2 {
					props.istd = u16(b, 0)
					applyDOCPap(b[2:], data, &props.pap, 0)
				}
			}
		}
		*out = append(*out, docRun{lo, hi, props})
	}
}

func walkDOCSprms(b []byte, fn func(uint16, []byte)) {
	for pos := 0; pos+2 <= len(b); {
		s := u16(b, pos)
		pos += 2
		var n int
		switch s >> 13 {
		case 0, 1:
			n = 1
		case 2, 4, 5:
			n = 2
		case 3:
			n = 4
		case 7:
			n = 3
		default:
			if s == 0xd608 {
				if pos+2 > len(b) {
					return
				}
				n = int(u16(b, pos)) + 1
			} else {
				if pos >= len(b) {
					return
				}
				n = int(b[pos]) + 1
			}
		}
		v, ok := boundedSlice(b, pos, n)
		if !ok {
			return
		}
		fn(s, v)
		pos += n
	}
}
func docPtr[T any](v T) *T { return &v }
func applyDOCPap(b, data []byte, p *docPap, depth int) {
	if depth > 4 {
		return
	}
	walkDOCSprms(b, func(s uint16, v []byte) {
		switch s {
		case 0x2416:
			p.inTable = docPtr(len(v) > 0 && v[0] != 0)
		case 0x2417:
			p.rowEnd = docPtr(len(v) > 0 && v[0] != 0)
		case 0x6646:
			off := int(u32(v, 0))
			n := int(u16(data, off))
			if x, ok := boundedSlice(data, off+2, n); ok {
				applyDOCPap(x, nil, p, depth+1)
			}
		case 0x2640:
			if len(v) > 0 {
				n := int(v[0])
				if n < 9 {
					n++
				} else {
					n = 0
				}
				p.outline = docPtr(n)
			}
		case 0x260a:
			if len(v) > 0 {
				p.ilvl = docPtr(v[0])
			}
		case 0x460b:
			p.ilfo = docPtr(u16(v, 0))
		case 0x6649:
			p.itap = docPtr(int32(u32(v, 0)))
		case 0x664a:
			n := int32(u32(v, 0))
			if p.itap != nil {
				n += *p.itap
			}
			p.itap = docPtr(n)
		case 0x244b:
			p.innerCell = docPtr(len(v) > 0 && v[0] != 0)
		case 0x244c:
			p.innerRow = docPtr(len(v) > 0 && v[0] != 0)
		case 0xd608:
			if tap := parseDOCTap(v); tap != nil {
				if p.tap != nil {
					tap.header = p.tap.header
				}
				p.tap = tap
			}
		case 0x3404:
			on := len(v) > 0 && v[0] != 0
			if p.tap == nil && on {
				p.tap = &docTap{header: true}
			} else if p.tap != nil {
				p.tap.header = on
			}
		case 0xd62b:
			if len(v) > 2 && p.tap != nil && int(v[1]) < len(p.tap.cells) {
				c := &p.tap.cells[int(v[1])]
				c.vcont = v[2] == 1
				c.vfirst = v[2] == 3
			}
		}
	})
}
func parseDOCTap(v []byte) *docTap {
	if len(v) < 3 {
		return nil
	}
	n := int(v[2])
	if n > 63 {
		return nil
	}
	t := &docTap{boundaries: make([]int16, n+1), cells: make([]docTapCell, n)}
	for i := 0; i <= n; i++ {
		o := 3 + i*2
		if o+2 > len(v) {
			return nil
		}
		t.boundaries[i] = int16(u16(v, o))
	}
	base := 3 + (n+1)*2
	for i := 0; i < n; i++ {
		o := base + i*20
		if o+2 > len(v) {
			break
		}
		f := u16(v, o)
		horz := f & 3
		t.cells[i].hcont = horz == 1
		t.cells[i].hfirst = horz >= 2
		vert := (f >> 5) & 3
		t.cells[i].vcont = vert == 1
		t.cells[i].vfirst = vert == 3
	}
	return t
}
func docToggle(v []byte, base bool) (bool, bool) {
	if len(v) == 0 {
		return false, false
	}
	if v[0] < 2 {
		return v[0] == 1, true
	}
	if v[0] == 0x80 {
		return base, true
	}
	return !base, v[0] == 0x81
}
func applyDOCChpx(b []byte, current, base docStyle) docStyle {
	walkDOCSprms(b, func(s uint16, v []byte) {
		switch s {
		case 0x0835:
			if x, ok := docToggle(v, base.bold); ok {
				current.bold = x
			}
		case 0x0836:
			if x, ok := docToggle(v, base.italic); ok {
				current.italic = x
			}
		case 0x0837:
			if x, ok := docToggle(v, base.strike); ok {
				current.strike = x
			}
		}
	})
	return current
}
func docChpxStyle(b []byte) (uint16, bool) {
	var out uint16
	ok := false
	walkDOCSprms(b, func(s uint16, v []byte) {
		if s == 0x4a30 && len(v) >= 2 {
			out = u16(v, 0)
			ok = true
		}
	})
	return out, ok
}

type docResolvedStyle struct {
	chp     docStyle
	pap     docPap
	heading int
	block   byte
}
type docStyles struct{ m map[uint16]docResolvedStyle }

func (s docStyles) get(i uint16) docResolvedStyle {
	if x, ok := s.m[i]; ok {
		return x
	}
	return docResolvedStyle{}
}

type docRawStyle struct {
	base       uint16
	chpx, papx []byte
	heading    int
	block      byte
	paragraph  bool
}

func parseDOCStyles(word, table []byte) docStyles {
	out := docStyles{m: map[uint16]docResolvedStyle{}}
	fc, n := int(u32(word, 0xa2)), int(u32(word, 0xa6))
	stsh, ok := boundedSlice(table, fc, n)
	if !ok || len(stsh) < 6 {
		return out
	}
	header, count, baseSize := int(u16(stsh, 0)), int(u16(stsh, 2)), int(u16(stsh, 4))
	if count > 65_535 {
		return out
	}
	raw := map[uint16]docRawStyle{}
	pos := 2 + header
	for i := 0; i < count && pos+2 <= len(stsh); i++ {
		n := int(u16(stsh, pos))
		pos += 2
		if n == 0 {
			continue
		}
		r, ok := boundedSlice(stsh, pos, n)
		if !ok {
			break
		}
		pos += n
		if x, ok := parseDOCRawStyle(r, baseSize); ok {
			raw[uint16(i)] = x
		}
	}
	return resolveDOCStyles(raw)
}
func resolveDOCStyles(raw map[uint16]docRawStyle) docStyles {
	out := docStyles{m: map[uint16]docResolvedStyle{}}
	for id := range raw {
		if _, ok := out.m[id]; ok {
			continue
		}
		chain := []uint16{}
		seen := map[uint16]bool{}
		cursor := id
		base := docResolvedStyle{}
		for len(chain) <= len(raw) {
			if x, ok := out.m[cursor]; ok {
				base = x
				break
			}
			r, ok := raw[cursor]
			if !ok || seen[cursor] {
				break
			}
			seen[cursor] = true
			chain = append(chain, cursor)
			if r.base == 0xfff || r.base == cursor {
				break
			}
			cursor = r.base
		}
		for i := len(chain) - 1; i >= 0; i-- {
			r := raw[chain[i]]
			if r.paragraph && len(r.papx) >= 2 {
				var d docPap
				applyDOCPap(r.papx[2:], nil, &d, 0)
				base.pap = base.pap.over(d)
			}
			base.chp = applyDOCChpx(r.chpx, base.chp, base.chp)
			if r.heading > 0 {
				base.heading = r.heading
			}
			if r.block != 0 {
				base.block = r.block
			}
			out.m[chain[i]] = base
		}
	}
	return out
}
func parseDOCRawStyle(r []byte, baseSize int) (docRawStyle, bool) {
	if len(r) < 6 {
		return docRawStyle{}, false
	}
	sti := u16(r, 0) & 0xfff
	second := u16(r, 2)
	x := docRawStyle{base: (second >> 4) & 0xfff, paragraph: second&0xf == 1}
	if sti >= 1 && sti <= 9 {
		x.heading = int(sti)
	}
	cupx := int(u16(r, 4) & 0xf)
	nameOff := max(baseSize, 10)
	if nameOff+2 > len(r) {
		return x, true
	}
	units := int(u16(r, nameOff))
	nameBytes := units * 2
	b, ok := boundedSlice(r, nameOff+2, nameBytes)
	if !ok {
		return x, true
	}
	name := strings.ToLower(strings.TrimSpace(string(utf16.Decode(bytesToU16(b)))))
	switch name {
	case "quote", "intense quote", "block text", "quotations":
		x.block = 'q'
	case "html preformatted", "source code", "preformatted text":
		x.block = 'c'
	}
	pos := nameOff + 4 + nameBytes
	var upx [][]byte
	for i := 0; i < cupx; i++ {
		if pos&1 == 1 {
			pos++
		}
		if pos+2 > len(r) {
			break
		}
		n := int(u16(r, pos))
		b, ok := boundedSlice(r, pos+2, n)
		if !ok {
			break
		}
		upx = append(upx, b)
		pos += 2 + n
	}
	if x.paragraph {
		if len(upx) > 0 {
			x.papx = append([]byte(nil), upx[0]...)
		}
		if len(upx) > 1 {
			x.chpx = append([]byte(nil), upx[1]...)
		}
	} else if len(upx) > 0 {
		x.chpx = append([]byte(nil), upx[0]...)
	}
	return x, true
}
func bytesToU16(b []byte) []uint16 {
	u := make([]uint16, 0, len(b)/2)
	for i := 0; i+1 < len(b); i += 2 {
		u = append(u, u16(b, i))
	}
	return u
}
