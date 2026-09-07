// Package webtool implements Noema's explicit public web tools.
package webtool

import (
	"context"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"net/url"
	"strings"
	"sync"
	"time"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/netpolicy"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const SearchName = "web.search"
const FetchName = "web.fetch"

var SearchSchema = json.RawMessage(`{"type":"object","properties":{"query":{"type":"string","minLength":1,"maxLength":500,"description":"The exact internet search query to send to the configured search provider."},"reason":{"type":"string","maxLength":500,"description":"Brief reason this search is useful for the current response."},"max_results":{"type":"integer","minimum":1,"maximum":10,"description":"Maximum number of search results to return."}},"required":["query"],"additionalProperties":false}`)
var FetchSchema = json.RawMessage(`{"type":"object","properties":{"url":{"type":"string","minLength":1,"maxLength":2048,"description":"The public http(s) URL of a web page or text resource to fetch and read."},"reason":{"type":"string","maxLength":500,"description":"Brief reason this page is useful for the current response."},"max_chars":{"type":"integer","minimum":1000,"maximum":20000,"description":"Maximum characters to return after extraction and optional summarization."}},"required":["url"],"additionalProperties":false}`)

var Tools = []provider.GenerationTool{
	{Name: SearchName, Description: "Search the public web using Noema's configured search provider.", InputSchema: SearchSchema},
	{Name: FetchName, Description: "Fetch and read a public web page or UTF-8 text resource using Noema's configured web fetch provider. When following a search result or fetched-page link, pass its exact URL unchanged.", InputSchema: FetchSchema},
}

// Service executes web tools with current account, URL, and model authorities.
type Service struct {
	database           *store.Store
	accounts           *provider.AccountService
	artifacts          *artifact.Service
	generators         map[string]provider.Generator
	endpoints          map[string]string
	browserPath        string
	obscuraHome        string
	browserMaxSessions int
	browserOldSpaceMB  int
	browserMu          sync.Mutex
	browsers           map[string]*browserSession
	browserGeneration  uint64
	browserRevision    uint64
}

// BindingSnapshot identifies the exact provider authority used by a reviewed fetch.
type BindingSnapshot struct {
	ProviderAccountID  string `json:"provider_account_id"`
	CredentialRevision uint64 `json:"credential_revision"`
}

// CurrentBinding returns the exact current provider account and credential revision.
func (s *Service) CurrentBinding(ctx context.Context, name string) (BindingSnapshot, error) {
	account, _, _, err := s.account(ctx, name)
	if err != nil {
		return BindingSnapshot{}, err
	}
	return BindingSnapshot{ProviderAccountID: account.ID, CredentialRevision: account.Metadata.CredentialRevision()}, nil
}

// New creates one web tool service.
func New(database *store.Store, accounts *provider.AccountService, generators map[string]provider.Generator, artifacts *artifact.Service, homeRoot, browserPath string, maxSessions, oldSpaceMB int) (*Service, error) {
	if database == nil || accounts == nil {
		return nil, errors.New("web tool dependencies are unavailable")
	}
	path := strings.TrimSpace(browserPath)
	if maxSessions < 1 || maxSessions > 8 || oldSpaceMB < 256 || oldSpaceMB > 4096 {
		return nil, errors.New("browser limits are invalid")
	}
	return &Service{database: database, accounts: accounts, artifacts: artifacts, generators: generators, browserPath: path,
		obscuraHome:        homeRoot,
		browserMaxSessions: maxSessions, browserOldSpaceMB: oldSpaceMB, browsers: make(map[string]*browserSession), endpoints: map[string]string{
			"obscura":           obscuraReleaseURL,
			"duckduckgo_public": "https://html.duckduckgo.com/html/", "exa": "https://api.exa.ai",
			"tinyfish_search": "https://api.search.tinyfish.ai", "tinyfish_fetch": "https://api.fetch.tinyfish.ai",
			"firecrawl": "https://api.firecrawl.dev/v2",
			"kernel":    "https://api.onkernel.com",
		}}, nil
}

// Close stops all browser workers.
func (s *Service) Close() {
	s.browserMu.Lock()
	sessions := make([]*browserSession, 0, len(s.browsers))
	for owner, session := range s.browsers {
		delete(s.browsers, owner)
		sessions = append(sessions, session)
	}
	s.browserMu.Unlock()
	for _, session := range sessions {
		s.closeBrowserSession(session)
	}
}

// Explicit reports whether configured web tools replace native hosted web.
func (s *Service) Explicit(ctx context.Context) bool {
	explicit, err := s.database.HasWebProviderOverride(ctx)
	return err == nil && explicit
}

// Execute runs one explicit tool and records its public output URLs.
func (s *Service) Execute(ctx context.Context, name string, raw json.RawMessage, source string) (json.RawMessage, bool) {
	var value any
	var urls []string
	var err error
	switch name {
	case SearchName:
		value, urls, err = s.search(ctx, raw)
	case FetchName:
		value, urls, err = s.fetch(ctx, raw)
	default:
		err = errors.New("web tool is unavailable")
	}
	if err != nil {
		payload, _ := json.Marshal(map[string]any{"error": safeError(err)})
		return payload, false
	}
	kind := "search_result"
	if name == FetchName {
		kind = "fetched_link"
	}
	observed := make([]string, len(urls))
	for index := range urls {
		observed[index], err = observationKey(urls[index])
		if err != nil {
			return json.RawMessage(`{"error":"web result contains an invalid URL"}`), false
		}
	}
	if err := s.database.ObserveURLs(ctx, kind, source, observed, time.Now()); err != nil {
		payload, _ := json.Marshal(map[string]any{"error": "web result could not be saved"})
		return payload, false
	}
	payload, _ := json.Marshal(value)
	return payload, true
}

// FetchObserved reports whether one valid exact fetch target was returned before.
func (s *Service) FetchObserved(ctx context.Context, raw json.RawMessage) (bool, error) {
	request, err := parseFetch(raw)
	if err != nil {
		return false, err
	}
	normalized, err := observationURL(ctx, request.URL)
	if err != nil {
		return false, err
	}
	return s.database.URLWasObserved(ctx, normalized)
}

func (s *Service) account(ctx context.Context, toolName string) (provider.Account, string, string, error) {
	route, err := s.database.WebProviderRoute(ctx, toolName)
	if err != nil {
		return provider.Account{}, "", "", err
	}
	id := "provider_account:duckduckgo_public:system"
	if toolName == FetchName {
		id = "provider_account:direct_http:system"
	}
	if len(route) != 0 {
		id = route[0].ProviderAccountID
	}
	account, err := s.accounts.LoadAccount(ctx, id)
	if err != nil || !account.IsActive || !hasCapability(account, toolName) {
		if len(route) == 0 {
			return provider.Account{}, "", "", errors.New("web provider is unavailable")
		}
		reason := "bound provider account is no longer available"
		if err == nil && account.IsActive {
			reason = "bound provider account does not declare " + toolName
			for _, capability := range provider.Capabilities(account) {
				if capability.ID == toolName {
					reason = "bound provider capability " + toolName + " is " + capability.Status
				}
			}
		}
		fallbackFrom := id
		if toolName == FetchName {
			id = "provider_account:direct_http:system"
		} else {
			id = "provider_account:duckduckgo_public:system"
		}
		fallback, fallbackErr := s.accounts.LoadAccount(ctx, id)
		return fallback, fallbackFrom, reason, fallbackErr
	}
	return account, "", "", nil
}

func hasCapability(account provider.Account, name string) bool {
	for _, capability := range provider.Capabilities(account) {
		if capability.ID == name && capability.Status == "available" {
			return true
		}
	}
	return false
}

func (s *Service) secret(ctx context.Context, account provider.Account) (string, error) {
	secret, err := s.accounts.LoadSecretAtRevision(ctx, account.ID, account.Metadata.CredentialRevision())
	if err != nil {
		return "", err
	}
	var value string
	err = secret.Use(func(raw string) error { value = raw; return nil })
	return value, err
}

func decodeExact(raw json.RawMessage, target any) error {
	var outer map[string]json.RawMessage
	if json.Unmarshal(raw, &outer) == nil {
		if nested, exists := outer["arguments"]; exists {
			if len(outer) != 1 {
				return errors.New("nested arguments payload cannot include outer fields")
			}
			raw = nested
		}
	}
	decoder := json.NewDecoder(strings.NewReader(string(raw)))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(target); err != nil {
		return errors.New("arguments do not match the web tool schema")
	}
	if decoder.Decode(&struct{}{}) == nil {
		return errors.New("arguments do not match the web tool schema")
	}
	return nil
}

func normalizePublicURL(ctx context.Context, raw string) (string, error) {
	normalized, err := normalizePublicURLTarget(raw)
	if err != nil {
		return "", err
	}
	checked, err := netpolicy.CheckURL(ctx, normalized)
	if err != nil {
		return "", err
	}
	return checked.URL.String(), nil
}

// normalizePublicURLTarget validates and normalizes a public URL without DNS
// resolution. Callers that perform a network request must use normalizePublicURL.
func normalizePublicURLTarget(raw string) (string, error) {
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

func observationURL(ctx context.Context, raw string) (string, error) {
	normalized, err := normalizePublicURL(ctx, raw)
	if err != nil {
		return "", err
	}
	return observationKey(normalized)
}

func observationKey(normalized string) (string, error) {
	parsed, _ := url.Parse(normalized)
	if parsed == nil || parsed.Hostname() == "" {
		return "", errors.New("observed URL is invalid")
	}
	parsed.Fragment = ""
	return parsed.String(), nil
}

func boundedBody(response *http.Response, max int64) ([]byte, error) {
	defer response.Body.Close()
	reader := http.MaxBytesReader(nil, response.Body, max)
	data, err := io.ReadAll(reader)
	if err != nil {
		return nil, errors.New("web provider response is too large")
	}
	return data, nil
}

func safeError(err error) string {
	switch {
	case errors.Is(err, context.DeadlineExceeded):
		return "web provider timed out"
	default:
		return err.Error()
	}
}
