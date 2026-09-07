package provider

import (
	"errors"
	"strings"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/netpolicy"
)

const (
	providerWebRawMarkdownLimit = 8_000
	providerWebSinglePassLimit  = 250_000
	providerWebChunkedLimit     = 1_000_000
)

// WebSummaryStrategy is the shared size policy used before a page enters a
// summarizer. It is public so the web tool uses the same boundary as provider
// responses and parity tests.
func WebSummaryStrategy(chars int) string { return providerWebSummaryStrategy(chars) }

func providerWebSummaryStrategy(chars int) string {
	switch {
	case chars <= providerWebRawMarkdownLimit:
		return "not_summarized"
	case chars <= providerWebSinglePassLimit:
		return "single_pass"
	case chars <= providerWebChunkedLimit:
		return "chunked"
	default:
		return "refuse"
	}
}

// NormalizeWebText collapses page whitespace at the provider boundary.
func NormalizeWebText(value string) string { return providerWebNormalizeText(value) }

func providerWebNormalizeText(value string) string { return strings.Join(strings.Fields(value), " ") }

// NormalizePublicURLTarget validates one public URL without DNS resolution.
// The caller must still use the net policy's checked client before I/O.
func NormalizePublicURLTarget(raw string) (string, error) {
	if !utf8.ValidString(raw) || utf8.RuneCountInString(raw) > 2048 {
		return "", errors.New("public URL is invalid")
	}
	checked, err := netpolicy.CheckURLTarget(strings.TrimSpace(raw))
	if err != nil {
		return "", err
	}
	checked.Fragment = ""
	return checked.String(), nil
}

// SafeWebSessionID validates an upstream browser session before path use.
func SafeWebSessionID(value string) bool { return providerWebSafeSessionID(value) }

func providerWebSafeSessionID(value string) bool {
	if value == "" || len(value) > 200 {
		return false
	}
	for _, r := range value {
		if (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') || (r >= '0' && r <= '9') || r == '-' || r == '_' {
			continue
		}
		return false
	}
	return true
}
