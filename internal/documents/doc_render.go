package documents

import "strings"

func renderDOCInlines(in []docInline, table bool, notes map[string]int) string {
	var out strings.Builder
	for _, x := range in {
		if x.note != "" {
			if n := notes[x.note]; n > 0 {
				out.WriteString("[^")
				out.WriteString(docDecimal.ordinal(uint64(n)))
				out.WriteByte(']')
			}
			continue
		}
		if x.breakLine {
			if table {
				out.WriteByte('\n')
			} else {
				out.WriteString("\\\n")
			}
			continue
		}
		text := x.text
		if text == "" {
			continue
		}
		lead := len(text) - len(strings.TrimLeft(text, " \t\r\n"))
		trailStart := len(strings.TrimRight(text, " \t\r\n"))
		core := text[lead:trailStart]
		out.WriteString(text[:lead])
		var rendered string
		if x.style.bold || x.style.italic || x.style.strike {
			open := ""
			if x.style.strike {
				open += "~~"
			}
			if x.style.bold {
				open += "**"
			}
			if x.style.italic {
				open += "*"
			}
			close := reverseASCII(open)
			rendered = open + docEscape(core, table, true, out.Len() == 0) + close
		} else {
			rendered = docEscape(core, table, false, out.Len() == 0)
		}
		if x.link != "" {
			rendered = "[" + rendered + "](" + docURL(x.link) + ")"
		}
		out.WriteString(rendered)
		out.WriteString(text[trailStart:])
	}
	return out.String()
}
func reverseASCII(s string) string {
	b := []byte(s)
	for i, j := 0, len(b)-1; i < j; i, j = i+1, j-1 {
		b[i], b[j] = b[j], b[i]
	}
	return string(b)
}
func docURL(s string) string {
	var b strings.Builder
	space := false
	for _, r := range s {
		switch r {
		case '<':
			b.WriteString("%3C")
		case '>':
			b.WriteString("%3E")
		case '|':
			b.WriteString("%7C")
		default:
			if r < ' ' {
				continue
			}
			if r == ' ' || r == '(' || r == ')' {
				space = true
			}
			b.WriteRune(r)
		}
	}
	if space {
		return "<" + b.String() + ">"
	}
	return b.String()
}
func docEscape(s string, table, styled, lineStart bool) string {
	r := []rune(s)
	last := map[rune]int{}
	for i, c := range r {
		switch c {
		case '*', '_', '~', '`', ']':
			last[c] = i
		}
	}
	var b strings.Builder
	has := !lineStart
	for i, c := range r {
		if c == '\n' {
			b.WriteRune(c)
			has = false
			continue
		}
		start := !has
		if !isSpace(c) {
			has = true
		}
		nextActive := i+1 >= len(r) || !isSpace(r[i+1])
		paired := func(x rune) bool { return last[x] > i }
		escape := false
		switch c {
		case '\\':
			escape = true
		case '`':
			escape = styled || paired(c)
		case '*':
			escape = styled || start || nextActive && paired(c)
		case '_':
			prevAlpha := i > 0 && isAlphaNum(r[i-1])
			nextAlpha := i+1 < len(r) && isAlphaNum(r[i+1])
			escape = styled || nextActive && !(prevAlpha && nextAlpha) && paired(c)
		case '~':
			escape = styled || nextActive && paired(c)
		case '[':
			escape = paired(']')
		case '<':
			escape = i+1 < len(r) && (isASCIIAlpha(r[i+1]) || strings.ContainsRune("/!?", r[i+1]))
		case '|':
			escape = table
		case '#':
			if start {
				j := i
				for j < len(r) && r[j] == '#' {
					j++
				}
				escape = j == len(r) || isSpace(r[j])
			}
		case '-', '+', '>', '=':
			escape = start
		case '0', '1', '2', '3', '4', '5', '6', '7', '8', '9':
			if start {
				j := i
				for j < len(r) && r[j] >= '0' && r[j] <= '9' {
					j++
				}
				if j < len(r) && (r[j] == '.' || r[j] == ')') && (j+1 == len(r) || isSpace(r[j+1])) {
					for _, x := range r[i:j] {
						b.WriteRune(x)
					}
					b.WriteByte('\\')
					b.WriteRune(r[j])
					continue
				}
			}
		}
		if escape {
			b.WriteByte('\\')
		}
		b.WriteRune(c)
	}
	return b.String()
}
func isSpace(r rune) bool      { return r == ' ' || r == '\t' || r == '\r' || r == '\n' }
func isASCIIAlpha(r rune) bool { return r >= 'a' && r <= 'z' || r >= 'A' && r <= 'Z' }
func isAlphaNum(r rune) bool   { return isASCIIAlpha(r) || r >= '0' && r <= '9' || r > 127 }
