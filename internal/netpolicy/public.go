// Package netpolicy owns public-network URL and address checks.
package netpolicy

import (
	"context"
	"errors"
	"net"
	"net/http"
	"net/netip"
	"net/url"
	"strings"
	"time"
)

var ErrURLUnavailable = errors.New("public URL target is unavailable")

// CheckedURL is one normalized URL with pinned public addresses.
type CheckedURL struct {
	URL       *url.URL
	Addresses []netip.Addr
}

// CheckURL validates and resolves one public HTTP or HTTPS URL.
func CheckURL(ctx context.Context, raw string) (CheckedURL, error) {
	parsed, err := url.Parse(strings.TrimSpace(raw))
	if err != nil || parsed.Hostname() == "" ||
		(parsed.Scheme != "http" && parsed.Scheme != "https") {
		return CheckedURL{}, errors.New("public URL is invalid")
	}
	if parsed.User != nil {
		return CheckedURL{}, errors.New("public URL credentials are unavailable")
	}
	host := strings.TrimSuffix(strings.ToLower(parsed.Hostname()), ".")
	if blockedHostname(host) {
		return CheckedURL{}, errors.New("public URL target is blocked")
	}
	addresses, err := ResolvePublic(ctx, host)
	if err != nil {
		return CheckedURL{}, ErrURLUnavailable
	}
	return CheckedURL{URL: parsed, Addresses: addresses}, nil
}

// ResolvePublic returns all resolved addresses only when each one is public.
func ResolvePublic(ctx context.Context, host string) ([]netip.Addr, error) {
	if address, err := netip.ParseAddr(host); err == nil {
		if IsPublic(address) {
			return []netip.Addr{address.Unmap()}, nil
		}
		return nil, errors.New("address is not public")
	}
	addresses, err := net.DefaultResolver.LookupNetIP(ctx, "ip", host)
	if err != nil || len(addresses) == 0 {
		return nil, errors.New("host did not resolve")
	}
	for index := range addresses {
		addresses[index] = addresses[index].Unmap()
		if !IsPublic(addresses[index]) {
			return nil, errors.New("address is not public")
		}
	}
	return addresses, nil
}

// PinnedClient returns a no-proxy client for one checked URL.
func PinnedClient(checked CheckedURL, timeout time.Duration) *http.Client {
	port := checked.URL.Port()
	if port == "" {
		port = "80"
		if checked.URL.Scheme == "https" {
			port = "443"
		}
	}
	transport := http.DefaultTransport.(*http.Transport).Clone()
	transport.Proxy = nil
	transport.DialContext = func(ctx context.Context, network, _ string) (net.Conn, error) {
		var last error
		for _, address := range checked.Addresses {
			connection, err := (&net.Dialer{Timeout: timeout}).DialContext(
				ctx, network, net.JoinHostPort(address.String(), port),
			)
			if err == nil {
				return connection, nil
			}
			last = err
		}
		return nil, last
	}
	return &http.Client{
		Transport: transport, Timeout: timeout,
		CheckRedirect: func(*http.Request, []*http.Request) error { return http.ErrUseLastResponse },
	}
}

// IsPublic reports whether one address is globally routable under Noema policy.
func IsPublic(address netip.Addr) bool {
	address = address.Unmap()
	if !address.IsGlobalUnicast() || address.IsPrivate() {
		return false
	}
	if address.Is6() && !netip.MustParsePrefix("2000::/3").Contains(address) {
		return false
	}
	for _, raw := range []string{
		"0.0.0.0/8", "100.64.0.0/10", "127.0.0.0/8", "169.254.0.0/16",
		"192.0.0.0/24", "192.0.2.0/24", "192.88.99.0/24", "198.18.0.0/15",
		"198.51.100.0/24", "203.0.113.0/24", "224.0.0.0/4", "240.0.0.0/4",
		"::/128", "::1/128", "64:ff9b::/96", "64:ff9b:1::/48", "2001::/23",
		"2001:db8::/32", "2002::/16", "3fff::/20", "fc00::/7", "fe80::/10", "ff00::/8",
	} {
		if netip.MustParsePrefix(raw).Contains(address) {
			return false
		}
	}
	return true
}

func blockedHostname(host string) bool {
	return host == "localhost" || strings.HasSuffix(host, ".localhost") ||
		strings.HasSuffix(host, ".local") || strings.HasSuffix(host, ".internal") ||
		strings.HasSuffix(host, ".test") || strings.HasSuffix(host, ".invalid") ||
		strings.HasSuffix(host, ".example")
}
