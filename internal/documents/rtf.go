package documents

import (
	"strconv"
	"strings"
	"unicode/utf16"

	"golang.org/x/text/encoding/charmap"
)

type rtfState struct {
	suppress  bool
	ignorable bool
	uc        int
}

func parseRTF(data []byte) (string, error) {
	if !strings.HasPrefix(string(data[:min(len(data), 6)]), "{\\rtf") {
		return "", errInvalidDocument
	}
	state := rtfState{uc: 1}
	stack := make([]rtfState, 0, 16)
	var output markdownDocument
	var paragraph strings.Builder
	codePage, skip, controls := 1252, 0, 0
	var highSurrogate uint16
	emitByte := func(value byte) {
		if state.suppress {
			return
		}
		if skip > 0 {
			skip--
			return
		}
		if value == '\r' || value == '\n' {
			return
		}
		appendDocumentRune(&paragraph, rtfCharmap(codePage).DecodeByte(value))
	}
	emitRune := func(value rune) {
		if state.suppress {
			return
		}
		if skip > 0 {
			skip--
			return
		}
		appendDocumentRune(&paragraph, value)
	}
	emitUnicode := func(value rune) {
		if !state.suppress {
			appendDocumentRune(&paragraph, value)
		}
	}
	for index := 0; index < len(data); {
		switch data[index] {
		case '{':
			if len(stack) >= maxDocumentXMLDepth {
				return "", errInvalidDocument
			}
			stack = append(stack, state)
			index++
		case '}':
			if len(stack) == 0 {
				return "", errInvalidDocument
			}
			state = stack[len(stack)-1]
			stack = stack[:len(stack)-1]
			index++
		case '\\':
			controls++
			if controls > maxDocumentXMLTokens {
				return "", errInvalidDocument
			}
			index++
			if index >= len(data) {
				break
			}
			symbol := data[index]
			switch symbol {
			case '\\', '{', '}':
				emitByte(symbol)
				index++
				continue
			case '\'':
				if index+2 >= len(data) {
					return "", errInvalidDocument
				}
				value, err := strconv.ParseUint(string(data[index+1:index+3]), 16, 8)
				if err != nil {
					return "", errInvalidDocument
				}
				emitByte(byte(value))
				index += 3
				continue
			case '*':
				state.ignorable = true
				index++
				continue
			case '~':
				emitRune('\u00a0')
				index++
				continue
			case '_':
				emitRune('\u2011')
				index++
				continue
			case '-':
				index++
				continue
			case '\r', '\n':
				for index < len(data) && (data[index] == '\r' || data[index] == '\n') {
					index++
				}
				emitRune('\n')
				continue
			}
			start := index
			for index < len(data) && ((data[index] >= 'a' && data[index] <= 'z') || (data[index] >= 'A' && data[index] <= 'Z')) {
				index++
			}
			if start == index {
				index++
				continue
			}
			word := strings.ToLower(string(data[start:index]))
			sign := 1
			if index < len(data) && data[index] == '-' {
				sign = -1
				index++
			}
			numberStart := index
			for index < len(data) && data[index] >= '0' && data[index] <= '9' {
				index++
			}
			parameter, hasParameter := 0, numberStart != index
			if hasParameter {
				parsed, err := strconv.ParseInt(string(data[numberStart:index]), 10, 32)
				if err != nil {
					return "", errInvalidDocument
				}
				parameter = int(parsed) * sign
			}
			if index < len(data) && data[index] == ' ' {
				index++
			}
			if state.ignorable {
				state.suppress = true
				state.ignorable = false
			}
			if rtfSuppressedDestination(word) {
				state.suppress = true
			}
			if word == "bin" {
				if !hasParameter || parameter < 0 || parameter > len(data)-index {
					return "", errInvalidDocument
				}
				index += parameter
				continue
			}
			if state.suppress {
				continue
			}
			switch word {
			case "ansicpg":
				if hasParameter {
					codePage = parameter
				}
			case "uc":
				if hasParameter && parameter >= 0 && parameter <= 16 {
					state.uc = parameter
				}
			case "u":
				if !hasParameter {
					continue
				}
				unit := uint16(parameter)
				if unit >= 0xd800 && unit <= 0xdbff {
					highSurrogate = unit
				} else if unit >= 0xdc00 && unit <= 0xdfff && highSurrogate != 0 {
					emitUnicode(utf16.DecodeRune(rune(highSurrogate), rune(unit)))
					highSurrogate = 0
				} else {
					highSurrogate = 0
					emitUnicode(rune(unit))
				}
				skip = state.uc
			case "par":
				output.block("", paragraph.String())
				paragraph.Reset()
			case "line", "page", "column":
				appendDocumentRune(&paragraph, '\n')
			case "tab":
				appendDocumentRune(&paragraph, '\t')
			case "emdash":
				emitRune('\u2014')
			case "endash":
				emitRune('\u2013')
			case "bullet":
				emitRune('\u2022')
			case "lquote":
				emitRune('\u2018')
			case "rquote":
				emitRune('\u2019')
			case "ldblquote":
				emitRune('\u201c')
			case "rdblquote":
				emitRune('\u201d')
			}
		default:
			emitByte(data[index])
			index++
		}
		if output.count >= maxPreviewCharacters {
			break
		}
	}
	output.block("", paragraph.String())
	return output.String(), nil
}

func rtfSuppressedDestination(word string) bool {
	switch word {
	case "fonttbl", "colortbl", "stylesheet", "info", "pict", "object", "header", "headerl",
		"headerr", "headerf", "footer", "footerl", "footerr", "footerf", "fldinst", "datastore",
		"themedata", "colorschememapping", "xmlnstbl", "generator", "ftnsep", "ftnsepc",
		"aftnsep", "aftnsepc", "latentstyles", "listtable", "listoverridetable", "rsidtbl",
		"filetbl", "revtbl", "datafield", "bkmkend", "annotation", "atnid", "atnauthor",
		"template", "defchp", "defpap", "panose", "falt", "objdata", "blipuid", "nonshppict",
		"wgrffmtfilter", "pgdsctbl", "docvar", "sp", "sn", "sv", "shpinst", "background",
		"userprops", "operator", "author", "title", "subject", "keywords", "doccomm", "creatim",
		"revtim", "printim":
		return true
	default:
		return false
	}
}

func rtfCharmap(codePage int) *charmap.Charmap {
	switch codePage {
	case 874:
		return charmap.Windows874
	case 1250:
		return charmap.Windows1250
	case 1251:
		return charmap.Windows1251
	case 1252:
		return charmap.Windows1252
	case 1253:
		return charmap.Windows1253
	case 1254:
		return charmap.Windows1254
	case 1255:
		return charmap.Windows1255
	case 1256:
		return charmap.Windows1256
	case 1257:
		return charmap.Windows1257
	case 1258:
		return charmap.Windows1258
	default:
		return charmap.Windows1252
	}
}
