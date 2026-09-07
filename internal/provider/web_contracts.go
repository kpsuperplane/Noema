package provider

import (
	"errors"
	"strings"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/netpolicy"
)

// WebSummaryStrategy is the shared size policy used before a page enters a
// summarizer. It is public so the web tool uses the same boundary as provider
// responses and parity tests.
func WebSummaryStrategy(chars int) string { return providerWebSummaryStrategy(chars) }

// NormalizeWebText collapses page whitespace at the provider boundary.
func NormalizeWebText(value string) string { return providerWebNormalizeText(value) }

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
