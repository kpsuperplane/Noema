package web

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
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
	"os"
	"path/filepath"
	"sort"
	"strconv"
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
	faviconSourceLimit  = 256 * 1024
	faviconSize         = 32
	faviconCacheLimit   = 1024
	faviconPositiveTTL  = 30 * 24 * time.Hour
	faviconMissingTTL   = 24 * time.Hour
	faviconTransientTTL = 5 * time.Minute
)

type faviconFailure byte

const (
	faviconMissing faviconFailure = iota + 1
	faviconTransient
	faviconTimeout
)

func (failure faviconFailure) Error() string { return "favicon unavailable" }

type faviconHandler struct {
	mu       sync.Mutex
	cacheDir string
	hostLock map[string]*sync.Mutex
}

type faviconCacheOutcome string

const (
	faviconAvailable       faviconCacheOutcome = "available"
	faviconCachedMissing   faviconCacheOutcome = "missing"
	faviconCachedTransient faviconCacheOutcome = "transient"
)

type faviconCacheEntry struct {
	outcome   faviconCacheOutcome
	body      []byte
	fetchedAt time.Time
	expiresAt time.Time
}

type faviconResponse struct {
	body      []byte
	finalURL  *url.URL
	mediaType string
}

// NewFaviconHandler serves normalized public-site icons from one bounded cache.
func NewFaviconHandler(cacheDir string) *faviconHandler {
	return &faviconHandler{cacheDir: cacheDir, hostLock: make(map[string]*sync.Mutex)}
}

// Seed primes one handler with an already normalized icon.
func (h *faviconHandler) Seed(hostname string, body []byte) { h.write(hostname, body) }

func (h *faviconHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	hostname, err := normalizeFaviconHostname(r.PathValue("hostname"))
	if err != nil {
		http.Error(w, "invalid hostname", http.StatusBadRequest)
		return
	}
	entry, fresh := h.cacheEntry(hostname, time.Now())
	if entry.outcome == faviconAvailable && (fresh || !entry.expiresAt.IsZero()) {
		body := entry.body
		h.writeResponse(w, r, body)
		return
	}
	if fresh {
		switch entry.outcome {
		case faviconCachedMissing:
			http.Error(w, http.StatusText(http.StatusNotFound), http.StatusNotFound)
			return
		case faviconCachedTransient:
			http.Error(w, http.StatusText(http.StatusBadGateway), http.StatusBadGateway)
			return
		}
	}
	lock := h.hostMutex(hostname)
	lock.Lock()
	defer lock.Unlock()
	entry, fresh = h.cacheEntry(hostname, time.Now())
	if fresh {
		switch entry.outcome {
		case faviconAvailable:
			h.writeResponse(w, r, entry.body)
			return
		case faviconCachedMissing:
			http.Error(w, http.StatusText(http.StatusNotFound), http.StatusNotFound)
			return
		case faviconCachedTransient:
			http.Error(w, http.StatusText(http.StatusBadGateway), http.StatusBadGateway)
			return
		}
	}
	body, err := func() ([]byte, error) {
		ctx, cancel := context.WithTimeout(r.Context(), 5*time.Second)
		defer cancel()
		return fetchFavicon(ctx, hostname)
	}()
	if err != nil {
		if err == faviconMissing {
			h.writeOutcome(hostname, faviconCachedMissing, nil, time.Now())
		} else {
			h.writeOutcome(hostname, faviconCachedTransient, nil, time.Now())
		}
		status := http.StatusBadGateway
		if errors.Is(err, context.DeadlineExceeded) || err == faviconTimeout {
			status = http.StatusGatewayTimeout
		}
		if err == faviconMissing {
			status = http.StatusNotFound
		}
		http.Error(w, http.StatusText(status), status)
		return
	}
	h.write(hostname, body)
	h.writeResponse(w, r, body)
}

func (h *faviconHandler) writeResponse(w http.ResponseWriter, r *http.Request, body []byte) {
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
		return "", errInvalidFaviconHostname
	}
	if _, err := netip.ParseAddr(raw); err == nil {
		return "", errInvalidFaviconHostname
	}
	hostname, err := idna.Lookup.ToASCII(raw)
	if err != nil || len(hostname) > 253 {
		return "", errInvalidFaviconHostname
	}
	return strings.ToLower(hostname), nil
}

var errInvalidFaviconHostname = errors.New("invalid favicon hostname")

func (h *faviconHandler) read(hostname string) []byte {
	entry, _ := h.cacheEntry(hostname, time.Now())
	if entry.outcome != faviconAvailable {
		return nil
	}
	return append([]byte(nil), entry.body...)
}

func (h *faviconHandler) write(hostname string, body []byte) {
	h.writeOutcome(hostname, faviconAvailable, body, time.Now())
}

func (h *faviconHandler) writeOutcome(hostname string, outcome faviconCacheOutcome, body []byte, now time.Time) {
	h.mu.Lock()
	defer h.mu.Unlock()
	if err := os.MkdirAll(h.cacheDir, 0o700); err != nil {
		return
	}
	_ = os.Chmod(h.cacheDir, 0o700)
	key := faviconCacheKey(hostname)
	pngPath := filepath.Join(h.cacheDir, key+".png")
	if outcome == faviconAvailable {
		if err := writeFaviconCacheFile(pngPath, body); err != nil {
			return
		}
	} else {
		_ = os.Remove(pngPath)
	}
	ttl := faviconPositiveTTL
	if outcome == faviconCachedMissing {
		ttl = faviconMissingTTL
	} else if outcome == faviconCachedTransient {
		ttl = faviconTransientTTL
	}
	metadata := faviconCacheMetadata{
		Hostname: hostname, Outcome: outcome, FetchedAt: now.Unix(), ExpiresAt: now.Add(ttl).Unix(),
	}
	encoded, err := json.Marshal(metadata)
	if err != nil || writeFaviconCacheFile(filepath.Join(h.cacheDir, key+".json"), encoded) != nil {
		return
	}
	evictFaviconCache(h.cacheDir)
}

func (h *faviconHandler) cacheEntry(hostname string, now time.Time) (faviconCacheEntry, bool) {
	key := faviconCacheKey(hostname)
	metadataBytes, err := os.ReadFile(filepath.Join(h.cacheDir, key+".json"))
	if err != nil {
		return faviconCacheEntry{}, false
	}
	var metadata faviconCacheMetadata
	if err := json.Unmarshal(metadataBytes, &metadata); err != nil || metadata.Hostname != hostname {
		return faviconCacheEntry{}, false
	}
	entry := faviconCacheEntry{
		outcome: metadata.Outcome, fetchedAt: time.Unix(metadata.FetchedAt, 0), expiresAt: time.Unix(metadata.ExpiresAt, 0),
	}
	if metadata.Outcome == faviconAvailable {
		body, err := os.ReadFile(filepath.Join(h.cacheDir, key+".png"))
		if err != nil {
			return faviconCacheEntry{}, false
		}
		entry.body = body
	}
	return entry, entry.expiresAt.After(now)
}

func (h *faviconHandler) hostMutex(hostname string) *sync.Mutex {
	h.mu.Lock()
	defer h.mu.Unlock()
	if lock := h.hostLock[hostname]; lock != nil {
		return lock
	}
	lock := &sync.Mutex{}
	h.hostLock[hostname] = lock
	return lock
}

type faviconCacheMetadata struct {
	Hostname  string              `json:"hostname"`
	Outcome   faviconCacheOutcome `json:"outcome"`
	FetchedAt int64               `json:"fetched_at"`
	ExpiresAt int64               `json:"expires_at"`
}

func faviconCacheKey(hostname string) string {
	digest := sha256.Sum256([]byte(hostname))
	return hex.EncodeToString(digest[:])
}

func writeFaviconCacheFile(path string, body []byte) error {
	directory := filepath.Dir(path)
	temporary, err := os.CreateTemp(directory, ".favicon-*tmp")
	if err != nil {
		return err
	}
	temporaryName := temporary.Name()
	defer os.Remove(temporaryName)
	if err := temporary.Chmod(0o600); err != nil {
		_ = temporary.Close()
		return err
	}
	if _, err := temporary.Write(body); err != nil {
		_ = temporary.Close()
		return err
	}
	if err := temporary.Sync(); err != nil {
		_ = temporary.Close()
		return err
	}
	if err := temporary.Close(); err != nil {
		return err
	}
	return os.Rename(temporaryName, path)
}

func evictFaviconCache(directory string) {
	entries, err := os.ReadDir(directory)
	if err != nil {
		return
	}
	type cachedMetadata struct {
		fetchedAt int64
		path      string
	}
	metadata := make([]cachedMetadata, 0)
	for _, entry := range entries {
		if filepath.Ext(entry.Name()) != ".json" {
			continue
		}
		path := filepath.Join(directory, entry.Name())
		body, err := os.ReadFile(path)
		if err != nil {
			continue
		}
		var value faviconCacheMetadata
		if json.Unmarshal(body, &value) == nil {
			metadata = append(metadata, cachedMetadata{fetchedAt: value.FetchedAt, path: path})
		}
	}
	if len(metadata) <= faviconCacheLimit {
		return
	}
	sort.Slice(metadata, func(i, j int) bool {
		if metadata[i].fetchedAt != metadata[j].fetchedAt {
			return metadata[i].fetchedAt < metadata[j].fetchedAt
		}
		return metadata[i].path < metadata[j].path
	})
	for _, value := range metadata[:len(metadata)-faviconCacheLimit] {
		_ = os.Remove(value.path)
		_ = os.Remove(strings.TrimSuffix(value.path, ".json") + ".png")
	}
}

func fetchFavicon(ctx context.Context, hostname string) ([]byte, error) {
	last := faviconMissing
	for _, scheme := range []string{"https", "http"} {
		root, _ := url.Parse(scheme + "://" + hostname + "/")
		attemptCtx, cancel := context.WithCancel(ctx)
		results := make(chan struct {
			body []byte
			err  error
		}, 2)
		go func() {
			response, err := fetchFaviconURL(attemptCtx, root.ResolveReference(&url.URL{Path: "favicon.ico"}), "image/*,*/*;q=0.1")
			if err == nil {
				response.body, err = normalizeFaviconImage(response.body)
			}
			results <- struct {
				body []byte
				err  error
			}{response.body, err}
		}()
		go func() {
			body, err := fetchDeclaredFavicon(attemptCtx, root)
			results <- struct {
				body []byte
				err  error
			}{body, err}
		}()
		first := <-results
		if first.err == nil {
			cancel()
			return first.body, nil
		}
		second := <-results
		cancel()
		if second.err == nil {
			return second.body, nil
		}
		last = strongerFaviconFailure(last, first.err)
		last = strongerFaviconFailure(last, second.err)
	}
	return nil, last
}

func fetchDeclaredFavicon(ctx context.Context, root *url.URL) ([]byte, error) {
	page, err := fetchFaviconURL(ctx, root, "text/html,application/xhtml+xml")
	if err != nil {
		return nil, err
	}
	if page.mediaType != "" && page.mediaType != "text/html" && page.mediaType != "application/xhtml+xml" {
		return nil, faviconMissing
	}
	declared := declaredFavicon(page.body, page.finalURL)
	if declared == nil {
		return nil, faviconMissing
	}
	response, err := fetchFaviconURL(ctx, declared, "image/*,*/*;q=0.1")
	if err != nil {
		return nil, err
	}
	return normalizeFaviconImage(response.body)
}

func fetchFaviconURL(ctx context.Context, current *url.URL, accept string) (faviconResponse, error) {
	for redirects := 0; redirects <= 3; redirects++ {
		checked, err := netpolicy.CheckURL(ctx, current.String())
		if err != nil {
			if errors.Is(ctx.Err(), context.DeadlineExceeded) {
				return faviconResponse{}, faviconTimeout
			}
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
	bestKind, bestDistance := 2, ^uint64(0)
	var best *url.URL
	var visit func(*html.Node)
	visit = func(node *html.Node) {
		if node.Type == html.ElementNode && node.Data == "link" {
			rel, href := htmlAttribute(node, "rel"), htmlAttribute(node, "href")
			tokens := strings.Fields(strings.ToLower(rel))
			if href != "" {
				if parsed, parseErr := url.Parse(href); parseErr == nil {
					candidate := base.ResolveReference(parsed)
					kind := 2
					if containsString(tokens, "icon") && !containsString(tokens, "mask-icon") {
						kind = 0
					} else if hasAppleIcon(tokens) {
						kind = 1
					}
					distance := faviconSizeDistance(htmlAttribute(node, "sizes"))
					if kind < 2 && (kind < bestKind || kind == bestKind && distance < bestDistance) {
						bestKind, bestDistance, best = kind, distance, candidate
					}
				}
			}
		}
		for child := node.FirstChild; child != nil; child = child.NextSibling {
			visit(child)
		}
	}
	visit(document)
	return best
}

func hasAppleIcon(tokens []string) bool {
	for _, token := range tokens {
		if strings.HasPrefix(token, "apple-touch-icon") {
			return true
		}
	}
	return false
}

func faviconSizeDistance(raw string) uint64 {
	best := ^uint64(0)
	for _, size := range strings.Fields(strings.ToLower(raw)) {
		width, height, ok := strings.Cut(size, "x")
		if !ok {
			continue
		}
		w, wErr := strconv.ParseUint(width, 10, 32)
		h, hErr := strconv.ParseUint(height, 10, 32)
		if wErr == nil && hErr == nil {
			best = min(best, absDiff(w, faviconSize)+absDiff(h, faviconSize))
		}
	}
	return best
}

func absDiff(value uint64, target int) uint64 {
	if value > uint64(target) {
		return value - uint64(target)
	}
	return uint64(target) - value
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
