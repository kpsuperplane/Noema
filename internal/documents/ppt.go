package documents

import (
	"bytes"
	"sort"
	"strings"
	"unicode"
	"unicode/utf16"

	"github.com/abemedia/go-cfb"
)

const (
	maxPPTRecordDepth   = 64
	maxPPTRecords       = 16_000_000
	maxPPTPersistValues = 200_000
)

type pptRecord struct {
	version uint16
	kind    uint16
	body    []byte
}

func pptRecordAt(data []byte, offset int) (pptRecord, int, bool) {
	if offset < 0 || offset > len(data)-8 {
		return pptRecord{}, 0, false
	}
	length := int(u32(data, offset+4))
	body, ok := boundedSlice(data, offset+8, length)
	if !ok {
		return pptRecord{}, 0, false
	}
	return pptRecord{version: u16(data, offset), kind: u16(data, offset+2), body: body}, offset + 8 + length, true
}

func pptChildren(data []byte, visit func(pptRecord) bool) {
	for position := 0; ; {
		record, next, ok := pptRecordAt(data, position)
		if !ok || !visit(record) {
			return
		}
		position = next
	}
}

type pptPendingShape struct {
	textType byte
	text     strings.Builder
	styles   pptStyleRuns
}

type pptMasterStyles map[uint16][]pptMasterLevel

type pptSegment struct {
	blocks  []docBlock
	id      uint32
	hasID   bool
	isNotes bool
}

type pptExtractor struct {
	segments      []pptSegment
	current       []docBlock
	list          []docListEntry
	pending       *pptPendingShape
	masters       []pptNamedMaster
	activeMaster  int
	shapeCounter  uint64
	records       uint64
	currentNotes  bool
	recovering    bool
	encrypted     bool
	blockCount    int
	resourceError bool
}

type pptNamedMaster struct {
	id     uint32
	styles pptMasterStyles
}

type pptLayout struct {
	persist    map[uint32]int
	slides     []byte
	notes      []byte
	masters    []byte
	hasNotes   bool
	hasMasters bool
}

func parsePPT(input []byte) (string, error) {
	compound, err := cfb.NewReader(bytes.NewReader(input))
	if err != nil {
		return "", errInvalidDocument
	}
	data, err := readDOCStream(compound, "PowerPoint Document")
	if err != nil {
		return "", errInvalidDocument
	}
	currentUser, _ := readDOCStream(compound, "Current User")
	if len(currentUser) >= 16 && u32(currentUser, 12) == 0xf3d1c4df {
		return "", errInvalidDocument
	}
	extractor := &pptExtractor{}
	parsed, err := extractor.parseSlides(data, currentUser)
	if err != nil {
		return "", err
	}
	if !parsed {
		extractor = &pptExtractor{recovering: true}
		if err = extractor.walk(data, 1); err != nil {
			return "", err
		}
		extractor.endSegment(0, false)
	}
	blocks := extractor.blocks()
	if extractor.encrypted || extractor.resourceError {
		return "", errInvalidDocument
	}
	var output markdownDocument
	output.append(renderDOCBlocks(blocks, nil))
	return output.String(), nil
}

func locatePPTDocument(data, currentUser []byte) (pptLayout, bool, error) {
	if len(currentUser) < 20 {
		return pptLayout{}, false, nil
	}
	persist := make(map[uint32]int)
	documentID, hasDocument := uint32(0), false
	editOffset := int(u32(currentUser, 16))
	for range 100 {
		if editOffset == 0 {
			break
		}
		record, _, ok := pptRecordAt(data, editOffset)
		if !ok || record.kind != 0x0ff5 || len(record.body) < 20 {
			return pptLayout{}, false, nil
		}
		if !hasDocument {
			documentID, hasDocument = u32(record.body, 16), true
		}
		directory, _, ok := pptRecordAt(data, int(u32(record.body, 12)))
		if ok && directory.kind == 0x1772 {
			for position := 0; position < len(directory.body); {
				if position > len(directory.body)-4 {
					return pptLayout{}, false, nil
				}
				header := u32(directory.body, position)
				start, count := header&0x000f_ffff, int(header>>20)
				position += 4
				if count > (len(directory.body)-position)/4 {
					return pptLayout{}, false, nil
				}
				if len(persist) > maxPPTPersistValues-count {
					return pptLayout{}, false, errInvalidDocument
				}
				for index := range count {
					id := start + uint32(index)
					if _, found := persist[id]; !found {
						persist[id] = int(u32(directory.body, position))
					}
					position += 4
				}
			}
		}
		previous := int(u32(record.body, 8))
		if previous == editOffset {
			break
		}
		editOffset = previous
	}
	if !hasDocument {
		return pptLayout{}, false, nil
	}
	documentOffset, ok := persist[documentID]
	if !ok {
		return pptLayout{}, false, nil
	}
	document, _, ok := pptRecordAt(data, documentOffset)
	if !ok || document.kind != 0x03e8 {
		return pptLayout{}, false, nil
	}
	layout := pptLayout{persist: persist}
	pptChildren(document.body, func(record pptRecord) bool {
		if record.kind != 0x0ff0 {
			return true
		}
		switch record.version >> 4 {
		case 0:
			if layout.slides == nil {
				layout.slides = record.body
			}
		case 1:
			if !layout.hasMasters {
				layout.masters, layout.hasMasters = record.body, true
			}
		case 2:
			if !layout.hasNotes {
				layout.notes, layout.hasNotes = record.body, true
			}
		}
		return true
	})
	return layout, layout.slides != nil, nil
}

func (extractor *pptExtractor) parseSlides(data, currentUser []byte) (bool, error) {
	layout, ok, err := locatePPTDocument(data, currentUser)
	if err != nil {
		return false, err
	}
	if !ok {
		return false, nil
	}
	extractor.masters = collectPPTMasters(layout.masters, layout.hasMasters, layout.persist, data)
	if err := extractor.walkSlideList(layout.slides, layout.persist, data, false, 0x03ee); err != nil {
		return false, err
	}
	if layout.hasNotes {
		if err := extractor.walkSlideList(layout.notes, layout.persist, data, true, 0x03f0); err != nil {
			return false, err
		}
	}
	return true, nil
}

func pptMasterStyleTable(master []byte) pptMasterStyles {
	styles := make(pptMasterStyles)
	pptChildren(master, func(record pptRecord) bool {
		if record.kind == 0x0fa3 {
			instance := record.version >> 4
			if _, found := styles[instance]; !found {
				styles[instance] = parsePPTMasterStyle(record.body, instance)
			}
		}
		return true
	})
	return styles
}

func collectPPTMasters(list []byte, hasList bool, persist map[uint32]int, data []byte) []pptNamedMaster {
	var masters []pptNamedMaster
	if hasList {
		pptChildren(list, func(record pptRecord) bool {
			if record.kind != 0x03f3 || len(record.body) < 16 {
				return true
			}
			offset, found := persist[u32(record.body, 0)]
			master, _, ok := pptRecordAt(data, offset)
			if found && ok && master.kind == 0x03f8 {
				masters = append(masters, pptNamedMaster{id: u32(record.body, 12), styles: pptMasterStyleTable(master.body)})
			}
			return true
		})
	}
	if len(masters) != 0 {
		return masters
	}
	offsets := make([]int, 0, len(persist))
	for _, offset := range persist {
		offsets = append(offsets, offset)
	}
	sort.Ints(offsets)
	for _, offset := range offsets {
		master, _, ok := pptRecordAt(data, offset)
		if ok && master.kind == 0x03f8 {
			masters = append(masters, pptNamedMaster{styles: pptMasterStyleTable(master.body)})
		}
	}
	return masters
}

func (extractor *pptExtractor) walkSlideList(list []byte, persist map[uint32]int, data []byte, notes bool, container uint16) error {
	var persistID, slideID uint32
	hasPending := false
	var result error
	pptChildren(list, func(record pptRecord) bool {
		if record.kind == 0x03f3 {
			id, hasID, err := extractor.finishSlide(persistID, slideID, hasPending, persist, data, container, notes)
			if err != nil {
				result = err
				return false
			}
			extractor.endSegment(id, hasID)
			extractor.currentNotes = notes
			hasPending = len(record.body) >= 4
			if hasPending {
				persistID, slideID = u32(record.body, 0), u32(record.body, 12)
			}
			if !notes {
				extractor.selectMaster(persistID, hasPending, persist, data)
			}
			return true
		}
		if err := extractor.handleRecord(record, 1); err != nil {
			result = err
			return false
		}
		return true
	})
	if result != nil {
		return result
	}
	id, hasID, err := extractor.finishSlide(persistID, slideID, hasPending, persist, data, container, notes)
	if err != nil {
		return err
	}
	extractor.endSegment(id, hasID)
	return nil
}

func (extractor *pptExtractor) finishSlide(persistID, slideID uint32, pending bool, persist map[uint32]int, data []byte, container uint16, notes bool) (uint32, bool, error) {
	if !pending {
		return 0, false, nil
	}
	id, hasID := slideID, !notes && slideID != 0
	offset, found := persist[persistID]
	if !found {
		return id, hasID, nil
	}
	record, _, ok := pptRecordAt(data, offset)
	if !ok || record.kind != container {
		return id, hasID, nil
	}
	if notes {
		pptChildren(record.body, func(child pptRecord) bool {
			if child.kind == 0x03f1 && len(child.body) >= 4 {
				id, hasID = u32(child.body, 0), u32(child.body, 0) != 0
				return false
			}
			return true
		})
	}
	return id, hasID, extractor.walk(record.body, 1)
}

func (extractor *pptExtractor) selectMaster(persistID uint32, pending bool, persist map[uint32]int, data []byte) {
	extractor.activeMaster = 0
	if !pending {
		return
	}
	offset, found := persist[persistID]
	if !found {
		return
	}
	slide, _, ok := pptRecordAt(data, offset)
	if !ok || slide.kind != 0x03ee {
		return
	}
	var masterID uint32
	hasMaster := false
	pptChildren(slide.body, func(record pptRecord) bool {
		if record.kind == 0x03ef && len(record.body) >= 16 {
			masterID, hasMaster = u32(record.body, 12), true
			return false
		}
		return true
	})
	if hasMaster {
		for index, master := range extractor.masters {
			if master.id == masterID {
				extractor.activeMaster = index
				return
			}
		}
	}
}

func (extractor *pptExtractor) walk(data []byte, depth int) error {
	if depth > maxPPTRecordDepth {
		return errInvalidDocument
	}
	var result error
	pptChildren(data, func(record pptRecord) bool {
		if err := extractor.handleRecord(record, depth); err != nil {
			result = err
			return false
		}
		return true
	})
	return result
}

func (extractor *pptExtractor) handleRecord(record pptRecord, depth int) error {
	extractor.records++
	if extractor.records > maxPPTRecords {
		return errInvalidDocument
	}
	if record.version&0x000f != 0x000f {
		extractor.atom(record.kind, record.body)
		return nil
	}
	switch record.kind {
	case 0x2f14:
		extractor.encrypted = true
	case 0x03f0:
		if extractor.recovering && !pptNotesMaster(record.body) {
			extractor.endSegment(0, false)
			extractor.currentNotes = true
			if err := extractor.walk(record.body, depth+1); err != nil {
				return err
			}
			extractor.endSegment(0, false)
			extractor.currentNotes = false
		}
	case 0x03f8, 0x0fc9:
	case 0x0ff0:
		if record.version>>4 == 0 {
			return extractor.walk(record.body, depth+1)
		}
	default:
		return extractor.walk(record.body, depth+1)
	}
	return nil
}

func pptNotesMaster(data []byte) bool {
	master := false
	pptChildren(data, func(record pptRecord) bool {
		if record.kind == 0x03f1 && len(record.body) >= 4 {
			master = u32(record.body, 0)&0x8000_0000 != 0
			return false
		}
		return true
	})
	return master
}

func (extractor *pptExtractor) atom(kind uint16, body []byte) {
	switch kind {
	case 0x0f9f:
		extractor.flushShape()
		textType := byte(1)
		if len(body) > 0 {
			textType = body[0]
		}
		extractor.pending = &pptPendingShape{textType: textType}
	case 0x0fa0:
		units := make([]uint16, len(body)/2)
		for index := range units {
			units[index] = u16(body, index*2)
		}
		extractor.pushText(string(utf16.Decode(units)))
	case 0x0fa8:
		characters := make([]rune, len(body))
		for index, value := range body {
			characters[index] = rune(value)
		}
		extractor.pushText(string(characters))
	case 0x0fa1:
		if extractor.pending != nil {
			extractor.pending.styles = parsePPTStyleText(body, pptUTF16Length(extractor.pending.text.String()))
		}
	}
}

func (extractor *pptExtractor) pushText(text string) {
	if extractor.pending == nil {
		extractor.pending = &pptPendingShape{textType: 1}
	}
	extractor.pending.text.WriteString(text)
}

func pptUTF16Length(value string) int {
	length := 0
	for _, character := range value {
		length += utf16.RuneLen(character)
	}
	return length
}

func pptBool(value *bool, fallback bool) bool {
	if value == nil {
		return fallback
	}
	return *value
}

func pptCleanText(value string) string {
	var result strings.Builder
	result.Grow(len(value))
	skipLF := false
	for _, character := range value {
		if skipLF {
			skipLF = false
			if character == '\n' {
				continue
			}
		}
		switch character {
		case '\u00a0':
			result.WriteByte(' ')
		case '\u00ad', '\u200b', '\ufeff':
		case '\t':
			result.WriteRune(character)
		case '\r':
			result.WriteByte(' ')
			skipLF = true
		case '\n':
			result.WriteByte(' ')
		default:
			if !unicode.IsControl(character) {
				result.WriteRune(character)
			}
		}
	}
	return result.String()
}

func (extractor *pptExtractor) flushList() {
	if len(extractor.list) == 0 {
		return
	}
	extractor.addBlock(docBlock{kind: 'l', list: extractor.list})
	extractor.list = nil
}

func (extractor *pptExtractor) addBlock(block docBlock) {
	if extractor.blockCount >= maxDocumentBlocks {
		extractor.resourceError = true
		return
	}
	extractor.current = append(extractor.current, block)
	extractor.blockCount++
}

func (extractor *pptExtractor) masterLevel(textType byte, depth uint16) pptMasterLevel {
	if extractor.activeMaster < 0 || extractor.activeMaster >= len(extractor.masters) {
		return pptMasterLevel{}
	}
	levels := extractor.masters[extractor.activeMaster].styles[uint16(textType)]
	if int(depth) >= len(levels) {
		return pptMasterLevel{}
	}
	return levels[depth]
}

func (extractor *pptExtractor) flushShape() {
	shape := extractor.pending
	extractor.pending = nil
	if shape == nil || shape.text.Len() == 0 || extractor.resourceError {
		return
	}
	extractor.shapeCounter++
	isTitle := shape.textType == 0 || shape.textType == 6
	paragraphIndex, characterIndex := 0, 0
	paragraphLeft, characterLeft := int(^uint(0)>>1), int(^uint(0)>>1)
	if len(shape.styles.paragraphs) > 0 {
		paragraphLeft = shape.styles.paragraphs[0].count
	}
	if len(shape.styles.characters) > 0 {
		characterLeft = shape.styles.characters[0].count
	}
	paragraph := func() pptParaProps {
		if paragraphIndex < len(shape.styles.paragraphs) {
			return shape.styles.paragraphs[paragraphIndex]
		}
		return pptParaProps{}
	}
	character := func() pptCharProps {
		if characterIndex < len(shape.styles.characters) {
			return shape.styles.characters[characterIndex]
		}
		return pptCharProps{}
	}
	type pptParagraph struct {
		inlines []docInline
		depth   uint16
		bullet  *bool
	}
	var paragraphs []pptParagraph
	var inlines []docInline
	var run strings.Builder
	runStyle := docStyle{}
	flushRun := func() {
		text := pptCleanText(run.String())
		run.Reset()
		if text != "" {
			inlines = append(inlines, docInline{text: text, style: runStyle})
		}
	}
	for _, value := range shape.text.String() {
		properties := paragraph()
		master := extractor.masterLevel(shape.textType, properties.depth)
		chars := character()
		style := docStyle{bold: pptBool(chars.bold, pptBool(master.bold, false)), italic: pptBool(chars.italic, pptBool(master.italic, false))}
		switch value {
		case '\r':
			flushRun()
			paragraphs = append(paragraphs, pptParagraph{inlines: inlines, depth: properties.depth, bullet: properties.bullet})
			inlines = nil
		case '\u000b':
			flushRun()
			inlines = append(inlines, docInline{breakLine: true})
		default:
			if style != runStyle && run.Len() != 0 {
				flushRun()
			}
			runStyle = style
			run.WriteRune(value)
		}
		width := utf16.RuneLen(value)
		characterLeft = max(0, characterLeft-width)
		if characterLeft == 0 {
			characterIndex++
			characterLeft = int(^uint(0) >> 1)
			if characterIndex < len(shape.styles.characters) {
				characterLeft = shape.styles.characters[characterIndex].count
			}
		}
		paragraphLeft = max(0, paragraphLeft-width)
		if paragraphLeft == 0 {
			paragraphIndex++
			paragraphLeft = int(^uint(0) >> 1)
			if paragraphIndex < len(shape.styles.paragraphs) {
				paragraphLeft = shape.styles.paragraphs[paragraphIndex].count
			}
		}
	}
	flushRun()
	if len(inlines) > 0 {
		properties := paragraph()
		paragraphs = append(paragraphs, pptParagraph{inlines: inlines, depth: properties.depth, bullet: properties.bullet})
	}
	extractor.flushList()
	for _, item := range paragraphs {
		if docInlinesEmpty(item.inlines) {
			extractor.flushList()
			continue
		}
		master := extractor.masterLevel(shape.textType, item.depth)
		if isTitle {
			extractor.flushList()
			for index := range item.inlines {
				item.inlines[index].style.bold = item.inlines[index].style.bold && !pptBool(master.bold, false)
				item.inlines[index].style.italic = item.inlines[index].style.italic && !pptBool(master.italic, false)
			}
			extractor.addBlock(docBlock{kind: 'h', level: 2, inlines: item.inlines})
			continue
		}
		if pptBool(item.bullet, pptBool(master.bullet, false)) {
			extractor.list = append(extractor.list, docListEntry{level: int(item.depth), id: uint32(extractor.shapeCounter), marker: docBullet, inlines: item.inlines})
			if len(extractor.list) > maxDocumentBlocks {
				extractor.resourceError = true
			}
		} else {
			extractor.flushList()
			extractor.addBlock(docBlock{kind: 'p', inlines: item.inlines})
		}
	}
}

func (extractor *pptExtractor) endSegment(id uint32, hasID bool) {
	extractor.flushShape()
	extractor.flushList()
	if len(extractor.current) == 0 {
		return
	}
	extractor.segments = append(extractor.segments, pptSegment{
		blocks: extractor.current, id: id, hasID: hasID, isNotes: extractor.currentNotes,
	})
	extractor.current = nil
}

func (extractor *pptExtractor) blocks() []docBlock {
	extractor.endSegment(0, false)
	var slides, notes []pptSegment
	for _, segment := range extractor.segments {
		if segment.isNotes {
			notes = append(notes, segment)
		} else {
			slides = append(slides, segment)
		}
	}
	used := make([]bool, len(notes))
	var output []docBlock
	for _, slide := range slides {
		output = append(output, slide.blocks...)
		for index, note := range notes {
			if !used[index] && slide.hasID && note.hasID && slide.id == note.id {
				used[index] = true
				output = append(output, docBlock{kind: 'q', blocks: note.blocks})
			}
		}
	}
	for index, note := range notes {
		if !used[index] && len(note.blocks) > 0 {
			output = append(output, docBlock{kind: 'q', blocks: note.blocks})
		}
	}
	return output
}
