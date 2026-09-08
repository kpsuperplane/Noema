package provider

import (
	"encoding/json"
	"strings"
)

// ServiceCatalogRow renders Rust model_tools.rs service ownership metadata.
func ServiceCatalogRow(connectionID, displayName, label, description string) string {
	quote := func(value string) string {
		var text strings.Builder
		text.WriteByte('"')
		for _, ch := range value {
			if ch < 0x20 || ch == '"' || ch == '\\' {
				b, _ := json.Marshal(string(ch))
				text.Write(b[1 : len(b)-1])
			} else {
				text.WriteRune(ch)
			}
		}
		text.WriteByte('"')
		return text.String()
	}
	row := "- service\t" + connectionID + "\tname=" + quote(displayName)
	if label != "" {
		row += "\tconnection_label=" + quote(label)
	}
	if description != "" {
		row += "\tdescription=" + quote(description)
	}
	return row
}
