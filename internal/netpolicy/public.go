// Package netpolicy owns public-network URL and address checks.
package netpolicy

import (
	"context"
	"errors"
	"net"
	"net/http"
	"net/netip"
	"net/url"
	"path"
	"strconv"
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
	parsed, err := CheckURLTarget(raw)
	if err != nil {
		return CheckedURL{}, err
	}
	host := strings.TrimSuffix(strings.ToLower(parsed.Hostname()), ".")
	addresses, err := ResolvePublic(ctx, host)
	if err != nil {
		return CheckedURL{}, ErrURLUnavailable
	}
	return CheckedURL{URL: parsed, Addresses: addresses}, nil
}

// CheckURLTarget validates one public HTTP or HTTPS target without resolving its hostname.
func CheckURLTarget(raw string) (*url.URL, error) {
	parsed, err := url.Parse(strings.TrimSpace(raw))
	if err != nil || parsed.Hostname() == "" ||
		(parsed.Scheme != "http" && parsed.Scheme != "https") {
		return nil, errors.New("public URL is invalid")
	}
	if parsed.User != nil {
		return nil, errors.New("public URL credentials are unavailable")
	}
	parsed.Scheme = strings.ToLower(parsed.Scheme)
	host := strings.TrimSuffix(strings.ToLower(parsed.Hostname()), ".")
	parsed.Host = normalizedHost(parsed, host)
	parsed.Path = normalizedPath(parsed.Path)
	if blockedHostname(host) {
		return nil, errors.New("public URL target is blocked")
	}
	address, parseErr := netip.ParseAddr(host)
	isLiteral := parseErr == nil
	if !isLiteral {
		address, isLiteral = parseIPv4Literal(host)
	}
	if isLiteral && !IsPublic(address) {
		return nil, errors.New("public URL target is blocked")
	}
	return parsed, nil
}

func normalizedHost(parsed *url.URL, host string) string {
	port := parsed.Port()
	if (parsed.Scheme == "http" && port == "80") || (parsed.Scheme == "https" && port == "443") {
		port = ""
	}
	if strings.Contains(host, ":") {
		host = "[" + host + "]"
	}
	if port != "" {
		host += ":" + port
	}
	return host
}

func normalizedPath(value string) string {
	if value == "" {
		return ""
	}
	clean := path.Clean(value)
	if strings.HasPrefix(value, "/") && !strings.HasPrefix(clean, "/") {
		clean = "/" + clean
	}
	if strings.HasSuffix(value, "/") && clean != "/" && !strings.HasSuffix(clean, "/") {
		clean += "/"
	}
	return clean
}

func parseIPv4Literal(host string) (netip.Addr, bool) {
	parts := strings.Split(host, ".")
	if len(parts) == 0 || len(parts) > 4 {
		return netip.Addr{}, false
	}
	values := make([]uint64, len(parts))
	for index, part := range parts {
		if part == "" {
			return netip.Addr{}, false
		}
		base := 10
		text := part
		if strings.HasPrefix(text, "0x") || strings.HasPrefix(text, "0X") {
			base, text = 16, text[2:]
		} else if len(text) > 1 && text[0] == '0' {
			base = 8
		}
		value, err := strconv.ParseUint(text, base, 32)
		if err != nil {
			return netip.Addr{}, false
		}
		values[index] = value
	}
	var number uint64
	switch len(values) {
	case 1:
		number = values[0]
	case 2:
		if values[0] > 0xff || values[1] > 0xffffff {
			return netip.Addr{}, false
		}
		number = values[0]<<24 | values[1]
	case 3:
		if values[0] > 0xff || values[1] > 0xff || values[2] > 0xffff {
			return netip.Addr{}, false
		}
		number = values[0]<<24 | values[1]<<16 | values[2]
	case 4:
		for _, value := range values {
			if value > 0xff {
				return netip.Addr{}, false
			}
		}
		number = values[0]<<24 | values[1]<<16 | values[2]<<8 | values[3]
	}
	return netip.AddrFrom4([4]byte{byte(number >> 24), byte(number >> 16), byte(number >> 8), byte(number)}), true
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
