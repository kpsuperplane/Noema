// Package childenv owns the environment boundary for external helper processes.
package childenv

import (
	"os"
	"strings"
)

// ExternalProcess keeps the host environment without Noema-owned authority.
func ExternalProcess() []string {
	current := os.Environ()
	result := make([]string, 0, len(current))
	for _, binding := range current {
		name, _, _ := strings.Cut(binding, "=")
		if name == "NOEMA_HOME" || name == "NOEMA_OPENAI__API_KEY" {
			continue
		}
		result = append(result, binding)
	}
	return result
}
