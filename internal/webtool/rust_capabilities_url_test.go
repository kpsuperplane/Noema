package webtool

import (
	"context"
	"net/netip"
	"testing"

	"github.com/kpsuperplane/noema/internal/netpolicy"
)

// Rust source: crates/noema-capabilities/src/web/url_policy.rs::accepts_public_url_without_resolving_dns.
func TestRustCapabilities_accepts_public_url_without_resolving_dns(t *testing.T) {
	url, err := netpolicy.CheckURLTarget("https://www.rust-lang.org/learn")
	if err != nil {
		t.Fatalf("public URL: %v", err)
	}
	if url.Hostname() != "www.rust-lang.org" {
		t.Errorf("public URL host = %q", url.Hostname())
	}
	normalized, err := observationURL(context.Background(), " HTTPS://1.1.1.1:443/a/../b?q=1#section ")
	if err != nil {
		t.Fatalf("normalize URL: %v", err)
	}
	if normalized != "https://1.1.1.1/b?q=1" {
		t.Errorf("normalized URL = %q", normalized)
	}
	if _, err := netpolicy.CheckURLTarget("https://user:secret@example.com/"); err == nil {
		t.Error("credential URL was accepted")
	}
	if !netpolicy.IsPublic(netip.MustParseAddr("2606:4700:4700::1111")) {
		t.Error("public IPv6 address was blocked")
	}
	if !netpolicy.IsPublic(netip.MustParseAddr("3fff:1000::1")) {
		t.Error("neighboring public IPv6 address was blocked")
	}
}

// Rust source: crates/noema-capabilities/src/web/url_policy.rs::rejects_private_literals_reserved_hosts_and_credentials.
func TestRustCapabilities_rejects_private_literals_reserved_hosts_and_credentials(t *testing.T) {
	for _, raw := range []string{
		"file:///etc/passwd",
		"http://localhost/",
		"http://127.0.0.1/",
		"http://10.0.0.1/",
		"http://172.16.0.1/",
		"http://192.168.0.1/",
		"http://[::1]/",
		"http://[fc00::1]/",
		"http://[fe80::1]/",
		"http://2130706433/",
		"http://0x7f000001/",
		"http://017700000001/",
		"http://127.1/",
		"http://192.88.99.1/",
		"http://[fec0::1]/",
		"http://[64:ff9b::7f00:1]/",
		"http://[64:ff9b:1::7f00:1]/",
		"http://[2002:7f00:1::]/",
		"http://[100::1]/",
		"http://[2001::1]/",
		"http://[3fff::1]/",
		"http://[4000::1]/",
		"https://user:secret@example.com/",
	} {
		if _, err := netpolicy.CheckURLTarget(raw); err == nil {
			t.Errorf("expected blocked URL to fail: %s", raw)
		}
	}
	ordinary, err := netpolicy.CheckURLTarget("https://example.com/path#section")
	if err != nil {
		t.Fatalf("ordinary fragment: %v", err)
	}
	if ordinary.Fragment != "section" {
		t.Errorf("ordinary fragment = %q", ordinary.Fragment)
	}
}
