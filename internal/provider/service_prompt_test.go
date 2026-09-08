package provider

import "testing"

func TestServiceCatalogRowMatchesRust(t *testing.T) {
	got := ServiceCatalogRow("connection:one", "Name <docs>", "Work", "Description\n\u2028")
	want := "- service\tconnection:one\tname=\"Name <docs>\"\tconnection_label=\"Work\"\tdescription=\"Description\\n\u2028\""
	if got != want {
		t.Fatalf("Rust service row differs: %q", got)
	}
	if got := ServiceCatalogRow("connection:one", "Name", "", ""); got != "- service\tconnection:one\tname=\"Name\"" {
		t.Fatalf("empty metadata differs: %q", got)
	}
}
