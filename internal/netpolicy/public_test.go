package netpolicy

import (
	"net/netip"
	"testing"
)

func TestPublicAddressPolicyRejectsInternalAndSpecialRanges(t *testing.T) {
	for _, raw := range []string{
		"0.0.0.0", "10.0.0.1", "100.64.0.1", "127.0.0.1", "169.254.1.1",
		"192.0.2.1", "198.18.0.1", "203.0.113.1", "::1", "fc00::1", "fe80::1", "2001:db8::1",
	} {
		if IsPublic(netip.MustParseAddr(raw)) {
			t.Fatalf("address %s is public", raw)
		}
	}
	for _, raw := range []string{"8.8.8.8", "1.1.1.1", "2606:4700:4700::1111"} {
		if !IsPublic(netip.MustParseAddr(raw)) {
			t.Fatalf("address %s is blocked", raw)
		}
	}
	for _, host := range []string{"localhost", "a.localhost", "host.local", "host.internal", "name.test"} {
		if !blockedHostname(host) {
			t.Fatalf("host %s is not blocked", host)
		}
	}
}
