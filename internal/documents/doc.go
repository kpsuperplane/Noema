package documents

import (
	"bytes"
	"encoding/binary"
	"fmt"
	"io"
	"sort"
	"unicode/utf16"

	"github.com/abemedia/go-cfb"
	"golang.org/x/text/encoding"
	"golang.org/x/text/encoding/charmap"
	"golang.org/x/text/encoding/japanese"
	"golang.org/x/text/encoding/korean"
	"golang.org/x/text/encoding/simplifiedchinese"
	"golang.org/x/text/encoding/traditionalchinese"
)

type docPiece struct {
	cp0, cp1, fc int
	compressed   bool
	prc          int
}
type docText struct {
	runes []rune
	fcs   []uint32
	cps   []uint32
	piece []uint32
}

func (t *docText) atCP(cp int) int {
	return sort.Search(len(t.cps), func(i int) bool { return int(t.cps[i]) >= cp })
}

type docStyle struct{ bold, italic, strike bool }
type docInline struct {
	text      string
	style     docStyle
	link      string
	note      string
	breakLine bool
}
type docBlock struct {
	kind    byte // p, h, q, c, l, t
	level   int
	inlines []docInline
	blocks  []docBlock
	code    string
	list    []docListEntry
	table   [][]docCell
}
type docCell []docBlock
type docNote struct {
	id     string
	blocks []docBlock
}
type docParser struct {
	word, table []byte
	text        docText
	pieces      []docPiece
	prcs        [][]byte
	chpx, papx  docRuns
	styles      docStyles
	lists       docLists
	noteRefs    map[int]string
	counters    docCounters
}

func parseDOC(input []byte) (string, error) {
	if bytes.HasPrefix(input, []byte("{\\rtf")) {
		return parseRTF(input)
	}
	compound, err := cfb.NewReader(bytes.NewReader(input))
	if err != nil {
		return "", errInvalidDocument
	}
	word, err := readDOCStream(compound, "WordDocument")
	if err != nil || u16(word, 0) != 0xA5EC {
		return "", errInvalidDocument
	}
	flags := u16(word, 0x0a)
	if flags&0x100 != 0 {
		return "", errInvalidDocument
	}
	tableName, other := "0Table", "1Table"
	if flags&0x200 != 0 {
		tableName, other = other, tableName
	}
	table, err := readDOCStream(compound, tableName)
	if err != nil {
		table, _ = readDOCStream(compound, other)
	}
	data, _ := readDOCStream(compound, "Data")
	p := &docParser{word: word, table: table, noteRefs: make(map[int]string)}
	clxFC, clxLen := int(u32(word, 0x1a2)), int(u32(word, 0x1a6))
	if clxLen != 0 {
		p.pieces, p.prcs, err = parseDOCClx(table, clxFC, clxLen)
		if err != nil {
			return "", err
		}
	} else {
		lo, hi := int(u32(word, 0x18)), int(u32(word, 0x1c))
		if hi > lo {
			p.pieces = []docPiece{{cp1: hi - lo, fc: lo, compressed: true, prc: -1}}
		}
	}
	totalCP := 0
	for _, off := range []int{0x4c, 0x50, 0x54, 0x58, 0x5c, 0x60} {
		totalCP += int(u32(word, off))
	}
	lid := u16(word, 6)
	if flags&0x4000 != 0 && u16(word, 0x3c) != 0 {
		lid = u16(word, 0x3c)
	}
	p.text = extractDOCText(word, p.pieces, totalCP, docEncoding(lid))
	p.chpx = parseDOCFKPs(word, table, 0xfa, false, data)
	p.papx = parseDOCFKPs(word, table, 0x102, true, data)
	p.styles = parseDOCStyles(word, table)
	p.lists = parseDOCLists(word, table)
	noteRanges := p.parseNotes()
	mainEnd := p.text.atCP(int(u32(word, 0x4c)))
	blocks, err := p.blocks(0, mainEnd)
	if err != nil {
		return "", err
	}
	notes := make([]docNote, 0)
	work := docWorkBudget(len(blocks))
	if !work.add(len(noteRanges)) {
		return "", errInvalidDocument
	}
	for _, nr := range noteRanges {
		if nr.lo < nr.hi {
			b, e := p.blocks(nr.lo, nr.hi)
			if e == nil {
				if !work.add(len(b)) {
					return "", errInvalidDocument
				}
				notes = append(notes, docNote{nr.id, b})
			}
		}
	}
	var output markdownDocument
	output.append(renderDOCDocument(blocks, notes))
	return output.String(), nil
}

type docWorkBudget int

func (b *docWorkBudget) add(n int) bool {
	if n < 0 || int(*b) > maxDocumentBlocks-n {
		return false
	}
	*b += docWorkBudget(n)
	return true
}
func readDOCStream(r *cfb.Reader, name string) ([]byte, error) {
	s, err := r.OpenStream(name)
	if err != nil {
		return nil, err
	}
	if s.Size < 0 || s.Size > maxDocumentInputBytes {
		return nil, errInvalidDocument
	}
	b := make([]byte, int(s.Size))
	n, err := s.ReadAt(b, 0)
	if err != nil && err != io.EOF {
		return nil, errInvalidDocument
	}
	if n != len(b) {
		return nil, errInvalidDocument
	}
	return b, nil
}
func u16(b []byte, off int) uint16 {
	if off < 0 || off > len(b)-2 {
		return 0
	}
	return binary.LittleEndian.Uint16(b[off:])
}
func u32(b []byte, off int) uint32 {
	if off < 0 || off > len(b)-4 {
		return 0
	}
	return binary.LittleEndian.Uint32(b[off:])
}
func boundedSlice(b []byte, off, size int) ([]byte, bool) {
	if off < 0 || size < 0 || off > len(b) || size > len(b)-off {
		return nil, false
	}
	return b[off : off+size], true
}
func parseDOCClx(table []byte, off, size int) ([]docPiece, [][]byte, error) {
	clx, ok := boundedSlice(table, off, size)
	if !ok {
		return nil, nil, errInvalidDocument
	}
	var prcs [][]byte
	for pos := 0; pos < len(clx); {
		switch clx[pos] {
		case 1:
			if pos > len(clx)-3 {
				return nil, nil, errInvalidDocument
			}
			n := int(u16(clx, pos+1))
			body, ok := boundedSlice(clx, pos+3, n)
			if !ok || len(prcs) >= 200_000 {
				return nil, nil, errInvalidDocument
			}
			prcs = append(prcs, append([]byte(nil), body...))
			pos += 3 + n
		case 2:
			if pos > len(clx)-5 {
				return nil, nil, errInvalidDocument
			}
			plc, ok := boundedSlice(clx, pos+5, int(u32(clx, pos+1)))
			if !ok || len(plc) < 16 || (len(plc)-4)%12 != 0 {
				return nil, nil, errInvalidDocument
			}
			n := (len(plc) - 4) / 12
			if n > 200_000 {
				return nil, nil, errInvalidDocument
			}
			out := make([]docPiece, 0, n)
			for i := 0; i < n; i++ {
				cp0, cp1 := int(u32(plc, i*4)), int(u32(plc, (i+1)*4))
				if cp1 < cp0 {
					return nil, nil, errInvalidDocument
				}
				pcd := (n+1)*4 + i*8
				raw := u32(plc, pcd+2)
				prm := u16(plc, pcd+6)
				piece := docPiece{cp0: cp0, cp1: cp1, fc: int(raw & 0x3fffffff), compressed: raw&0x40000000 != 0, prc: -1}
				if piece.compressed {
					piece.fc /= 2
				}
				if prm&1 != 0 {
					idx := int(prm >> 1)
					if idx < len(prcs) {
						piece.prc = idx
					}
				} else if prm != 0 {
					if grp := docPRM0(prm); grp != nil {
						piece.prc = len(prcs)
						prcs = append(prcs, grp)
					}
				}
				out = append(out, piece)
			}
			return out, prcs, nil
		default:
			return nil, nil, errInvalidDocument
		}
	}
	return nil, nil, errInvalidDocument
}
func docPRM0(prm uint16) []byte {
	m := map[uint16]uint16{0x0c: 0x260a, 0x18: 0x2416, 0x19: 0x2417, 0x55: 0x0835, 0x56: 0x0836, 0x57: 0x0837, 0x78: 0x2640}
	s, ok := m[(prm>>1)&0x7f]
	if !ok {
		return nil
	}
	return []byte{byte(s), byte(s >> 8), byte(prm >> 8)}
}

type docCharset struct {
	enc  encoding.Encoding
	dbcs byte
}

func docEncoding(lid uint16) docCharset {
	primary := lid & 0x3ff
	switch primary {
	case 0x11:
		return docCharset{japanese.ShiftJIS, 1}
	case 0x12:
		return docCharset{korean.EUCKR, 2}
	case 0x04:
		if lid == 0x0404 || lid == 0x0c04 || lid == 0x1404 || lid == 0x7c04 {
			return docCharset{traditionalchinese.Big5, 2}
		}
		return docCharset{simplifiedchinese.GBK, 2}
	case 0x01, 0x20, 0x29:
		return docCharset{charmap.Windows1256, 0}
	case 0x02, 0x19, 0x22, 0x23:
		return docCharset{charmap.Windows1251, 0}
	case 0x05, 0x0e, 0x15, 0x18, 0x1a, 0x1b, 0x24:
		return docCharset{charmap.Windows1250, 0}
	case 0x08:
		return docCharset{charmap.Windows1253, 0}
	case 0x0d:
		return docCharset{charmap.Windows1255, 0}
	case 0x1e:
		return docCharset{charmap.Windows874, 0}
	case 0x1f, 0x2c:
		return docCharset{charmap.Windows1254, 0}
	case 0x25, 0x26, 0x27:
		return docCharset{charmap.Windows1257, 0}
	case 0x2a:
		return docCharset{charmap.Windows1258, 0}
	default:
		return docCharset{charmap.Windows1252, 0}
	}
}
func (c docCharset) lead(b byte) bool {
	if c.dbcs == 1 {
		return b >= 0x81 && b <= 0x9f || b >= 0xe0 && b <= 0xfc
	}
	return c.dbcs == 2 && b >= 0x81 && b <= 0xfe
}
func extractDOCText(word []byte, pieces []docPiece, total int, charset docCharset) docText {
	var out docText
	cp := 0
	for pi, piece := range pieces {
		if cp >= total {
			break
		}
		n := min(piece.cp1-piece.cp0, total-cp)
		if n <= 0 {
			continue
		}
		if piece.compressed {
			b, ok := boundedSlice(word, piece.fc, n)
			if !ok {
				continue
			}
			for i := 0; i < len(b); {
				width := 1
				if charset.lead(b[i]) && i+1 < len(b) {
					width = 2
				}
				s, err := charset.enc.NewDecoder().Bytes(b[i : i+width])
				if err != nil {
					s = []byte("\ufffd")
				}
				for _, r := range string(s) {
					out.runes = append(out.runes, r)
					out.fcs = append(out.fcs, uint32(piece.fc+i))
					out.cps = append(out.cps, uint32(cp))
					out.piece = append(out.piece, uint32(pi))
				}
				i += width
				cp += width
			}
		} else {
			b, ok := boundedSlice(word, piece.fc, n*2)
			if !ok {
				continue
			}
			units := make([]uint16, 0, n)
			for i := 0; i+1 < len(b); i += 2 {
				units = append(units, u16(b, i))
			}
			unit := 0
			for _, r := range utf16.Decode(units) {
				out.runes = append(out.runes, r)
				out.fcs = append(out.fcs, uint32(piece.fc+unit*2))
				out.cps = append(out.cps, uint32(cp))
				out.piece = append(out.piece, uint32(pi))
				w := 1
				if r > 0xffff {
					w = 2
				}
				unit += w
				cp += w
			}
		}
		if len(out.runes) > maxDocumentTextBytes {
			break
		}
	}
	return out
}

type docNoteRange struct {
	lo, hi int
	id     string
}

func (p *docParser) parsePLC(off, element int) ([]uint32, int) {
	fc, n := int(u32(p.word, off)), int(u32(p.word, off+4))
	plc, ok := boundedSlice(p.table, fc, n)
	if !ok || n < 8 {
		return nil, 0
	}
	count := (n - 4) / (4 + element)
	if element == 0 {
		count = n/4 - 1
	}
	if count < 0 || count > 200_000 {
		return nil, 0
	}
	cps := make([]uint32, count+1)
	for i := range cps {
		cps[i] = u32(plc, i*4)
	}
	return cps, count
}
func (p *docParser) parseNotes() []docNoteRange {
	ccpText := int(u32(p.word, 0x4c))
	ednBase := ccpText + int(u32(p.word, 0x50)) + int(u32(p.word, 0x54)) + int(u32(p.word, 0x58)) + int(u32(p.word, 0x5c))
	var out []docNoteRange
	for _, x := range []struct {
		ref, text, base int
		prefix          string
	}{{0xaa, 0xb2, ccpText, "fn"}, {0x20a, 0x212, ednBase, "en"}} {
		refs, n := p.parsePLC(x.ref, 2)
		txt, _ := p.parsePLC(x.text, 0)
		for i := 0; i < n && i < len(refs); i++ {
			p.noteRefs[p.text.atCP(int(refs[i]))] = fmt.Sprintf("%s%d", x.prefix, i)
			if i+1 < len(txt) {
				out = append(out, docNoteRange{p.text.atCP(x.base + int(txt[i])), p.text.atCP(x.base + int(txt[i+1])), fmt.Sprintf("%s%d", x.prefix, i)})
			}
		}
	}
	return out
}
