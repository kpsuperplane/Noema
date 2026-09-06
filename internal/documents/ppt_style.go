package documents

type pptParaProps struct {
	count  int
	depth  uint16
	bullet *bool
}

type pptCharProps struct {
	count        int
	bold, italic *bool
}

type pptMasterLevel struct {
	bullet, bold, italic *bool
}

type pptStyleRuns struct {
	paragraphs []pptParaProps
	characters []pptCharProps
}

func parsePPTStyleText(body []byte, textLength int) pptStyleRuns {
	var runs pptStyleRuns
	position, covered := 0, 0
	for covered <= textLength {
		if len(runs.paragraphs) >= maxDocumentTextBytes {
			break
		}
		if position > len(body)-6 {
			break
		}
		count := int(u32(body, position))
		depth := u16(body, position+4)
		position += 6
		bullet, next, ok := parsePPTParagraphException(body, position)
		if !ok {
			return runs
		}
		position = next
		runs.paragraphs = append(runs.paragraphs, pptParaProps{count: count, depth: depth, bullet: bullet})
		if count == 0 || covered > textLength-count {
			break
		}
		covered += count
	}
	covered = 0
	for covered <= textLength {
		if len(runs.characters) >= maxDocumentTextBytes {
			break
		}
		if position > len(body)-4 {
			break
		}
		count := int(u32(body, position))
		position += 4
		character, next, ok := parsePPTCharacterException(body, position)
		if !ok {
			break
		}
		position = next
		character.count = count
		runs.characters = append(runs.characters, character)
		if count == 0 || covered > textLength-count {
			break
		}
		covered += count
	}
	return runs
}

func parsePPTParagraphException(body []byte, position int) (*bool, int, bool) {
	if position < 0 || position > len(body)-4 {
		return nil, 0, false
	}
	mask := u32(body, position)
	position += 4
	var bullet *bool
	if mask&0x000f != 0 {
		if position > len(body)-2 {
			return nil, 0, false
		}
		if mask&1 != 0 {
			value := u16(body, position)&1 != 0
			bullet = &value
		}
		position += 2
	}
	for bit, size := range map[uint32]int{
		0x0080: 2, 0x0010: 2, 0x0040: 2, 0x0020: 4,
		0x0800: 2, 0x1000: 2, 0x2000: 2, 0x4000: 2,
		0x0100: 2, 0x0400: 2, 0x8000: 2,
	} {
		if mask&bit != 0 {
			position += size
		}
	}
	if mask&0x0010_0000 != 0 {
		if position > len(body)-2 {
			return nil, 0, false
		}
		count := int(u16(body, position))
		if count > (len(body)-position-2)/4 {
			return nil, 0, false
		}
		position += 2 + count*4
	}
	for _, bit := range []uint32{0x0001_0000, 0x000e_0000, 0x0020_0000} {
		if mask&bit != 0 {
			position += 2
		}
	}
	return bullet, position, position <= len(body)
}

func parsePPTCharacterException(body []byte, position int) (pptCharProps, int, bool) {
	var result pptCharProps
	if position < 0 || position > len(body)-4 {
		return result, 0, false
	}
	mask := u32(body, position)
	position += 4
	if mask&0xffff != 0 {
		if position > len(body)-2 {
			return result, 0, false
		}
		style := u16(body, position)
		if mask&1 != 0 {
			value := style&1 != 0
			result.bold = &value
		}
		if mask&2 != 0 {
			value := style&2 != 0
			result.italic = &value
		}
		position += 2
	}
	for bit, size := range map[uint32]int{
		0x0001_0000: 2, 0x0020_0000: 2, 0x0040_0000: 2,
		0x0080_0000: 2, 0x0002_0000: 2, 0x0004_0000: 4,
		0x0008_0000: 2,
	} {
		if mask&bit != 0 {
			position += size
		}
	}
	return result, position, position <= len(body)
}

func parsePPTMasterStyle(body []byte, instance uint16) []pptMasterLevel {
	if len(body) < 2 {
		return nil
	}
	levels := min(int(u16(body, 0)), 10)
	position := 2
	result := make([]pptMasterLevel, 0, levels)
	for range levels {
		if instance >= 5 {
			position += 2
		}
		bullet, next, ok := parsePPTParagraphException(body, position)
		if !ok {
			break
		}
		character, next, ok := parsePPTCharacterException(body, next)
		if !ok {
			break
		}
		position = next
		result = append(result, pptMasterLevel{bullet: bullet, bold: character.bold, italic: character.italic})
	}
	return result
}
