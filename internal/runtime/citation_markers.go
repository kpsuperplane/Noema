package runtime

import (
	"net/url"
	"strconv"
	"strings"
	"unicode"

	"github.com/kpsuperplane/noema/internal/provider"
)

const (
	providerCitationMarkerStart = "\ue200cite\ue202"
	providerCitationMarkerEnd   = '\ue201'
	providerCitationSeparator   = '\ue202'
)

type normalizedProviderText struct {
	Text              string
	Citations         []provider.Citation
	UnresolvedMarkers int
}

func normalizeProviderText(text string, existing []provider.Citation) normalizedProviderText {
	var display strings.Builder
	removals := make([][2]int, 0)
	citations := make([]provider.Citation, 0, len(existing))
	seen := make(map[string]struct{})
	remaining := text
	rawOffset := 0
	unresolved := 0
	for {
		start := strings.Index(remaining, providerCitationMarkerStart)
		if start < 0 {
			break
		}
		prefix := remaining[:start]
		display.WriteString(prefix)
		rawOffset += utf16CodeUnitCount(prefix)
		body := remaining[start+len(providerCitationMarkerStart):]
		end := strings.IndexRune(body, providerCitationMarkerEnd)
		if end < 0 {
			end = strings.IndexFunc(body, unicode.IsSpace)
			if end < 0 {
				end = len(body)
			}
			malformed := remaining[:start+len(providerCitationMarkerStart)+end]
			display.WriteString(malformed[start:])
			rawOffset += utf16CodeUnitCount(malformed[start:])
			remaining = body[end:]
			unresolved++
			continue
		}
		markerLength := start + len(providerCitationMarkerStart) + end + len(string(providerCitationMarkerEnd))
		marker := remaining[start:markerLength]
		markerEnd := rawOffset + utf16CodeUnitCount(marker)
		displayEnd := utf16CodeUnitCount(display.String())
		references := strings.FieldsFunc(body[:end], func(character rune) bool {
			return character == providerCitationSeparator
		})
		resolvedByAnnotation := false
		for _, citation := range existing {
			if citation.StartIndex != nil && citation.EndIndex != nil &&
				*citation.StartIndex < markerEnd && *citation.EndIndex > rawOffset {
				resolvedByAnnotation = true
				break
			}
		}
		direct := make([]provider.Citation, 0, len(references))
		for _, reference := range references {
			parsed, err := url.Parse(reference)
			if err != nil || parsed.Hostname() == "" || parsed.User != nil ||
				(parsed.Scheme != "http" && parsed.Scheme != "https") {
				continue
			}
			endIndex := displayEnd
			direct = append(direct, provider.Citation{
				Title: parsed.Hostname(), URL: parsed.String(), EndIndex: &endIndex,
			})
		}
		if resolvedByAnnotation || len(direct) == len(references) {
			for _, citation := range direct {
				key := citation.URL + "\x00" + citationIndexKey(citation.EndIndex)
				if _, exists := seen[key]; exists {
					continue
				}
				seen[key] = struct{}{}
				citations = append(citations, citation)
			}
			removals = append(removals, [2]int{rawOffset, markerEnd})
		} else {
			display.WriteString(marker)
			unresolved += len(references) - len(direct)
			if len(references) == len(direct) {
				unresolved++
			}
		}
		rawOffset = markerEnd
		remaining = remaining[markerLength:]
	}
	display.WriteString(remaining)
	for _, citation := range existing {
		citation.StartIndex = adjustedCitationIndex(citation.StartIndex, removals)
		citation.EndIndex = adjustedCitationIndex(citation.EndIndex, removals)
		citations = append(citations, citation)
	}
	return normalizedProviderText{Text: display.String(), Citations: citations, UnresolvedMarkers: unresolved}
}

func adjustedCitationIndex(value *int, removals [][2]int) *int {
	if value == nil {
		return nil
	}
	adjusted := *value
	for _, removal := range removals {
		if *value >= removal[1] {
			adjusted -= removal[1] - removal[0]
		} else if *value > removal[0] {
			adjusted -= *value - removal[0]
		}
	}
	if adjusted < 0 {
		adjusted = 0
	}
	return &adjusted
}

func citationIndexKey(value *int) string {
	if value == nil {
		return ""
	}
	return strconv.Itoa(*value)
}

func utf16CodeUnitCount(value string) int {
	count := 0
	for _, character := range value {
		count++
		if character > 0xffff {
			count++
		}
	}
	return count
}
