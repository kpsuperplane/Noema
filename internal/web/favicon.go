package web

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/binary"
	"errors"
	"fmt"
	"image"
	_ "image/gif"
	_ "image/jpeg"
	"image/png"
	"io"
	"mime"
	"net/http"
	"net/netip"
	"net/url"
	"strings"
	"sync"
	"time"

	_ "github.com/fyne-io/image/ico"
	"github.com/kpsuperplane/noema/internal/netpolicy"
	"golang.org/x/image/draw"
	_ "golang.org/x/image/webp"
	"golang.org/x/net/html"
	"golang.org/x/net/idna"
)

const (
	faviconSourceLimit = 256 * 1024
	faviconSize        = 32
	faviconCacheLimit  = 1024
)

type faviconFailure byte

const (
	faviconMissing faviconFailure = iota + 1
	faviconTransient
	faviconTimeout
)

func (failure faviconFailure) Error() string { return "favicon unavailable" }

type faviconHandler struct {
	mu    sync.Mutex
	cache map[string][]byte
}

type faviconResponse struct {
	body      []byte
	finalURL  *url.URL
	mediaType string
}

// NewFaviconHandler serves normalized public-site icons from one bounded cache.
func NewFaviconHandler() http.Handler {
	return &faviconHandler{cache: make(map[string][]byte)}
}

func (h *faviconHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	hostname, err := normalizeFaviconHostname(r.PathValue("hostname"))
	if err != nil {
		http.Error(w, "invalid hostname", http.StatusBadRequest)
		return
	}
	body := h.read(hostname)
	if body == nil {
		ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
		defer cancel()
		body, err = fetchFavicon(ctx, hostname)
		if err != nil {
			status := http.StatusBadGateway
			if errors.Is(err, context.DeadlineExceeded) || err == faviconTimeout {
				status = http.StatusGatewayTimeout
			} else if err == faviconMissing {
				status = http.StatusNotFound
			}
			http.Error(w, http.StatusText(status), status)
			return
		}
		h.write(hostname, body)
	}
	digest := sha256.Sum256(body)
	etag := fmt.Sprintf("\"%x\"", digest)
	w.Header().Set("Cache-Control", "private, max-age=86400")
	w.Header().Set("ETag", etag)
	if r.Header.Get("If-None-Match") == etag {
		w.WriteHeader(http.StatusNotModified)
		return
	}
	w.Header().Set("Content-Type", "image/png")
	_, _ = w.Write(body)
}

func normalizeFaviconHostname(raw string) (string, error) {
	raw = strings.TrimSuffix(strings.TrimSpace(raw), ".")
	if raw == "" || len(raw) > 253 || strings.ContainsAny(raw, "/\\:@") {
		return "", errors.New("invalid hostname")
	}
	if _, err := netip.ParseAddr(raw); err == nil {
		return "", errors.New("IP addresses are unavailable")
	}
	hostname, err := idna.Lookup.ToASCII(raw)
	if err != nil || len(hostname) > 253 {
		return "", errors.New("invalid hostname")
	}
	return strings.ToLower(hostname), nil
}

func (h *faviconHandler) read(hostname string) []byte {
	h.mu.Lock()
	defer h.mu.Unlock()
	return h.cache[hostname]
}

func (h *faviconHandler) write(hostname string, body []byte) {
	h.mu.Lock()
	defer h.mu.Unlock()
	if len(h.cache) >= faviconCacheLimit {
		for victim := range h.cache {
			delete(h.cache, victim)
			break
		}
	}
	h.cache[hostname] = body
}

func fetchFavicon(ctx context.Context, hostname string) ([]byte, error) {
	last := faviconMissing
	for _, scheme := range []string{"https", "http"} {
		root, _ := url.Parse(scheme + "://" + hostname + "/")
		direct, err := fetchFaviconURL(ctx, root.ResolveReference(&url.URL{Path: "favicon.ico"}), "image/*,*/*;q=0.1")
		if err == nil {
			if normalized, imageErr := normalizeFaviconImage(direct.body); imageErr == nil {
				return normalized, nil
			} else {
				err = imageErr
			}
		}
		last = strongerFaviconFailure(last, err)
		page, pageErr := fetchFaviconURL(ctx, root, "text/html,application/xhtml+xml")
		if pageErr != nil {
			last = strongerFaviconFailure(last, pageErr)
			continue
		}
		if page.mediaType != "" && page.mediaType != "text/html" && page.mediaType != "application/xhtml+xml" {
			continue
		}
		declared := declaredFavicon(page.body, page.finalURL)
		if declared == nil {
			continue
		}
		response, declaredErr := fetchFaviconURL(ctx, declared, "image/*,*/*;q=0.1")
		if declaredErr == nil {
			if normalized, imageErr := normalizeFaviconImage(response.body); imageErr == nil {
				return normalized, nil
			} else {
				declaredErr = imageErr
			}
		}
		last = strongerFaviconFailure(last, declaredErr)
	}
	return nil, last
}

func fetchFaviconURL(ctx context.Context, current *url.URL, accept string) (faviconResponse, error) {
	for redirects := 0; redirects <= 3; redirects++ {
		checked, err := netpolicy.CheckURL(ctx, current.String())
		if err != nil {
			if errors.Is(err, netpolicy.ErrURLUnavailable) {
				return faviconResponse{}, faviconTransient
			}
			return faviconResponse{}, faviconMissing
		}
		request, _ := http.NewRequestWithContext(ctx, http.MethodGet, checked.URL.String(), nil)
		request.Header.Set("User-Agent", "NoemaFavicon/0.1 (+https://github.com/kpsuperplane/Noema)")
		request.Header.Set("Accept", accept)
		response, err := netpolicy.PinnedClient(checked, 5*time.Second).Do(request)
		if err != nil {
			if errors.Is(err, context.DeadlineExceeded) {
				return faviconResponse{}, faviconTimeout
			}
			return faviconResponse{}, faviconTransient
		}
		if response.StatusCode >= 300 && response.StatusCode < 400 {
			location, locationErr := response.Location()
			response.Body.Close()
			if locationErr != nil || redirects == 3 {
				return faviconResponse{}, faviconTransient
			}
			current = checked.URL.ResolveReference(location)
			continue
		}
		if response.StatusCode < 200 || response.StatusCode >= 300 {
			response.Body.Close()
			if response.StatusCode >= 400 && response.StatusCode < 500 {
				return faviconResponse{}, faviconMissing
			}
			return faviconResponse{}, faviconTransient
		}
		if response.ContentLength > faviconSourceLimit {
			response.Body.Close()
			return faviconResponse{}, faviconMissing
		}
		body, readErr := io.ReadAll(io.LimitReader(response.Body, faviconSourceLimit+1))
		response.Body.Close()
		if readErr != nil {
			return faviconResponse{}, faviconTransient
		}
		if len(body) > faviconSourceLimit {
			return faviconResponse{}, faviconMissing
		}
		mediaType, _, _ := mime.ParseMediaType(response.Header.Get("Content-Type"))
		return faviconResponse{body: body, finalURL: checked.URL, mediaType: strings.ToLower(mediaType)}, nil
	}
	return faviconResponse{}, faviconTransient
}

func declaredFavicon(body []byte, base *url.URL) *url.URL {
	document, err := html.Parse(bytes.NewReader(body))
	if err != nil {
		return nil
	}
	var icon, apple *url.URL
	var visit func(*html.Node)
	visit = func(node *html.Node) {
		if node.Type == html.ElementNode && node.Data == "link" {
			rel, href := htmlAttribute(node, "rel"), htmlAttribute(node, "href")
			tokens := strings.Fields(strings.ToLower(rel))
			if href != "" {
				if parsed, parseErr := url.Parse(href); parseErr == nil {
					candidate := base.ResolveReference(parsed)
					if icon == nil && containsString(tokens, "icon") && !containsString(tokens, "mask-icon") {
						icon = candidate
					} else if apple == nil {
						for _, token := range tokens {
							if strings.HasPrefix(token, "apple-touch-icon") {
								apple = candidate
							}
						}
					}
				}
			}
		}
		for child := node.FirstChild; child != nil; child = child.NextSibling {
			visit(child)
		}
	}
	visit(document)
	if icon != nil {
		return icon
	}
	return apple
}

func htmlAttribute(node *html.Node, name string) string {
	for _, attribute := range node.Attr {
		if attribute.Key == name {
			return attribute.Val
		}
	}
	return ""
}

func containsString(values []string, target string) bool {
	for _, value := range values {
		if value == target {
			return true
		}
	}
	return false
}

func normalizeFaviconImage(body []byte) ([]byte, error) {
	input, ok := boundedFaviconInput(body)
	if !ok {
		return nil, faviconMissing
	}
	config, format, err := image.DecodeConfig(bytes.NewReader(input))
	if err != nil || config.Width < 1 || config.Height < 1 || config.Width > 1024 || config.Height > 1024 {
		return nil, faviconMissing
	}
	switch format {
	case "gif", "ico", "jpeg", "png", "webp":
	default:
		return nil, faviconMissing
	}
	source, _, err := image.Decode(bytes.NewReader(input))
	if err != nil {
		return nil, faviconMissing
	}
	width, height := faviconSize, faviconSize
	if config.Width > config.Height {
		height = max(1, config.Height*faviconSize/config.Width)
	} else {
		width = max(1, config.Width*faviconSize/config.Height)
	}
	destination := image.NewNRGBA(image.Rect(0, 0, faviconSize, faviconSize))
	left, top := (faviconSize-width)/2, (faviconSize-height)/2
	draw.CatmullRom.Scale(destination, image.Rect(left, top, left+width, top+height), source, source.Bounds(), draw.Over, nil)
	var output bytes.Buffer
	if png.Encode(&output, destination) != nil || output.Len() > faviconSourceLimit {
		return nil, faviconMissing
	}
	return output.Bytes(), nil
}

func boundedFaviconInput(body []byte) ([]byte, bool) {
	if len(body) < 4 || !bytes.Equal(body[:4], []byte{0, 0, 1, 0}) {
		return body, true
	}
	if len(body) < 6 {
		return nil, false
	}
	count := int(binary.LittleEndian.Uint16(body[4:6]))
	directoryEnd := 6 + 16*count
	if count == 0 || count > 64 || directoryEnd > len(body) {
		return nil, false
	}
	bestScore := -1
	var bestEntry, bestData []byte
	for index := 0; index < count; index++ {
		offset := 6 + 16*index
		size := int(binary.LittleEndian.Uint32(body[offset+8 : offset+12]))
		start := int(binary.LittleEndian.Uint32(body[offset+12 : offset+16]))
		if size == 0 || start < directoryEnd || start > len(body)-size {
			return nil, false
		}
		width, height := (int(body[offset])+255)%256+1, (int(body[offset+1])+255)%256+1
		if score := width * height; score > bestScore {
			bestScore = score
			bestEntry, bestData = body[offset:offset+16], body[start:start+size]
		}
	}
	if len(bestData) < 8 || !bytes.Equal(bestData[:8], []byte("\x89PNG\r\n\x1a\n")) && len(bestData) < 36 {
		return nil, false
	}
	input := make([]byte, 22+len(bestData))
	copy(input[:6], []byte{0, 0, 1, 0, 1, 0})
	copy(input[6:22], bestEntry)
	binary.LittleEndian.PutUint32(input[18:22], 22)
	copy(input[22:], bestData)
	return input, true
}

func strongerFaviconFailure(left faviconFailure, right error) faviconFailure {
	if left == faviconTimeout || right == faviconTimeout || errors.Is(right, context.DeadlineExceeded) {
		return faviconTimeout
	}
	if left == faviconTransient || right == faviconTransient {
		return faviconTransient
	}
	return faviconMissing
}
