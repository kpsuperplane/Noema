package webtool

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"net/url"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/andybalholm/cascadia"
	"github.com/kpsuperplane/noema/internal/provider"
	"golang.org/x/net/html"
)

type searchRequest struct {
	Query      string `json:"query"`
	Reason     string `json:"reason,omitempty"`
	MaxResults int    `json:"max_results,omitempty"`
}

type searchResult struct {
	Rank    int    `json:"rank"`
	Title   string `json:"title"`
	URL     string `json:"url"`
	Snippet string `json:"snippet"`
}

type searchResponse struct {
	Provider         string         `json:"provider"`
	ProviderContract string         `json:"provider_contract"`
	Query            string         `json:"query"`
	Results          []searchResult `json:"results"`
	Summary          string         `json:"summary"`
	FallbackFrom     string         `json:"fallback_from,omitempty"`
	FallbackReason   string         `json:"fallback_reason,omitempty"`
}

var errProviderAuthentication = errors.New("web provider authentication failed")

func parseSearch(raw json.RawMessage) (searchRequest, error) {
	var input struct {
		Query      string `json:"query"`
		Reason     string `json:"reason,omitempty"`
		MaxResults *int   `json:"max_results,omitempty"`
	}
	if err := decodeExact(raw, &input); err != nil {
		if err.Error() == "nested arguments payload cannot include outer fields" {
			return searchRequest{}, err
		}
		return searchRequest{}, errors.New("arguments do not match the web.search schema")
	}
	request := searchRequest{Query: input.Query, Reason: input.Reason, MaxResults: 5}
	if input.MaxResults != nil {
		request.MaxResults = *input.MaxResults
	}
	if request.MaxResults < 0 {
		return request, errors.New("arguments do not match the web.search schema")
	}
	request.Query = strings.TrimSpace(request.Query)
	request.Reason = strings.TrimSpace(request.Reason)
	if request.Query == "" {
		return request, errors.New("query is required")
	}
	if utf8.RuneCountInString(request.Query) > 500 {
		return request, errors.New("query is too long")
	}
	if utf8.RuneCountInString(request.Reason) > 500 {
		return request, errors.New("web.search reason is invalid")
	}
	if request.MaxResults < 1 {
		request.MaxResults = 1
	}
	if request.MaxResults > 10 {
		request.MaxResults = 10
	}
	return request, nil
}

func (s *Service) search(ctx context.Context, raw json.RawMessage) (searchResponse, []string, error) {
	request, err := parseSearch(raw)
	if err != nil {
		return searchResponse{}, nil, err
	}
	account, fallbackFrom, fallbackReason, err := s.account(ctx, SearchName)
	if err != nil {
		return searchResponse{}, nil, err
	}
	var candidates []searchResult
	switch account.ProviderKind {
	case "duckduckgo_public":
		candidates, err = s.searchDuckDuckGo(ctx, request)
	case "exa", "tinyfish", "firecrawl":
		candidates, err = s.searchHosted(ctx, account, request)
	default:
		err = errors.New("configured search provider is unavailable")
	}
	if err != nil {
		if errors.Is(err, errProviderAuthentication) && account.AuthMethod != provider.AuthNone {
			_ = s.database.MarkProviderAuthenticationFailed(ctx, account.ID, account.Metadata.CredentialRevision(), time.Now())
		}
		return searchResponse{}, nil, err
	}
	results := make([]searchResult, 0, request.MaxResults)
	urls := make([]string, 0, request.MaxResults)
	for _, candidate := range candidates {
		normalized, checkErr := normalizePublicURL(ctx, candidate.URL)
		if checkErr != nil {
			continue
		}
		candidate.Rank = len(results) + 1
		candidate.Title = normalizeText(candidate.Title)
		candidate.Snippet = normalizeText(candidate.Snippet)
		candidate.URL = normalized
		observed, _ := observationURL(ctx, normalized)
		results, urls = append(results, candidate), append(urls, observed)
		if len(results) == request.MaxResults {
			break
		}
	}
	summary := fmt.Sprintf("Found %d web results", len(results))
	if len(results) == 0 {
		summary = "No web results found"
	}
	contract := "hosted_provider"
	if account.ProviderKind == "duckduckgo_public" {
		contract = "best_effort_public"
	}
	return searchResponse{Provider: account.ProviderKind, ProviderContract: contract, Query: request.Query, Results: results,
		Summary: summary, FallbackFrom: fallbackFrom, FallbackReason: fallbackReason}, urls, nil
}

func (s *Service) searchDuckDuckGo(ctx context.Context, request searchRequest) ([]searchResult, error) {
	endpoint, err := url.Parse(s.endpoints["duckduckgo_public"])
	if err != nil {
		return nil, errors.New("search provider is unavailable")
	}
	query := endpoint.Query()
	query.Set("q", request.Query)
	endpoint.RawQuery = query.Encode()
	httpRequest, _ := http.NewRequestWithContext(ctx, http.MethodGet, endpoint.String(), nil)
	httpRequest.Header.Set("User-Agent", "Noema/0.1 web.search (+https://github.com/kpsuperplane/Noema)")
	response, err := (&http.Client{Timeout: 10 * time.Second}).Do(httpRequest)
	if err != nil {
		return nil, errors.New("search provider request failed")
	}
	if response.StatusCode == http.StatusForbidden || response.StatusCode == http.StatusTooManyRequests {
		response.Body.Close()
		return nil, errors.New("search provider rate limit reached")
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		response.Body.Close()
		return nil, errors.New("search provider request failed")
	}
	body, err := boundedBody(response, 1_000_000)
	if err != nil {
		return nil, err
	}
	document, err := html.Parse(bytes.NewReader(body))
	if err != nil {
		return nil, errors.New("search provider response is invalid")
	}
	resultSelector := cascadia.MustCompile(".result.results_links, .web-result")
	titleSelector := cascadia.MustCompile(".result__a")
	snippetSelector := cascadia.MustCompile(".result__snippet")
	var results []searchResult
	for _, node := range cascadia.QueryAll(document, resultSelector) {
		title := cascadia.Query(node, titleSelector)
		if title == nil {
			continue
		}
		href := attribute(title, "href")
		if parsed, parseErr := url.Parse(href); parseErr == nil && parsed.Hostname() == "duckduckgo.com" && parsed.Path == "/l/" {
			href = parsed.Query().Get("uddg")
		} else if strings.HasPrefix(href, "//") {
			href = "https:" + href
		}
		snippet := cascadia.Query(node, snippetSelector)
		results = append(results, searchResult{Title: nodeText(title), URL: href, Snippet: nodeText(snippet)})
	}
	return results, nil
}

func (s *Service) searchHosted(ctx context.Context, account provider.Account, request searchRequest) ([]searchResult, error) {
	secret := ""
	if account.AuthMethod != provider.AuthNone {
		var err error
		secret, err = s.secret(ctx, account)
		if err != nil {
			return nil, errors.New("search provider credential is unavailable")
		}
	}
	method, endpoint := http.MethodPost, ""
	body := any(nil)
	headers := map[string]string{}
	switch account.ProviderKind {
	case "exa":
		endpoint, body, headers["x-api-key"] = s.endpoints["exa"]+"/search", map[string]any{"query": request.Query, "numResults": request.MaxResults}, secret
	case "tinyfish":
		method, endpoint, body, headers["x-api-key"] = http.MethodGet, s.endpoints["tinyfish_search"], nil, secret
		values := url.Values{"query": {request.Query}}
		if request.Reason != "" {
			values.Set("purpose", request.Reason)
		}
		endpoint += "?" + values.Encode()
	case "firecrawl":
		endpoint = s.endpoints["firecrawl"] + "/search"
		body = map[string]any{"query": request.Query, "limit": request.MaxResults, "sources": []string{"web"}, "timeout": 60000}
		if secret != "" {
			headers["Authorization"] = "Bearer " + secret
		}
	}
	value, err := requestJSON(ctx, method, endpoint, body, headers, 75*time.Second)
	if err != nil {
		return nil, err
	}
	if account.ProviderKind == "firecrawl" {
		data, ok := value["data"].(map[string]any)
		if value["success"] != true || !ok {
			return nil, errors.New("search provider response is invalid")
		}
		value = data
		value["results"] = data["web"]
	}
	items, ok := value["results"].([]any)
	if !ok {
		return nil, errors.New("search provider response is invalid")
	}
	results := make([]searchResult, 0, len(items))
	for _, item := range items {
		entry, _ := item.(map[string]any)
		snippet := stringValue(entry, "snippet")
		if snippet == "" {
			snippet = stringValue(entry, "description")
		}
		if snippet == "" {
			if highlights, ok := entry["highlights"].([]any); ok && len(highlights) != 0 {
				snippet, _ = highlights[0].(string)
			}
		}
		results = append(results, searchResult{Title: stringValue(entry, "title"), URL: stringValue(entry, "url"), Snippet: snippet})
	}
	return results, nil
}

func requestJSON(ctx context.Context, method, endpoint string, body any, headers map[string]string, timeout time.Duration) (map[string]any, error) {
	var data []byte
	if body != nil {
		data, _ = json.Marshal(body)
	}
	request, err := http.NewRequestWithContext(ctx, method, endpoint, bytes.NewReader(data))
	if err != nil {
		return nil, errors.New("web provider request failed")
	}
	request.Header.Set("Content-Type", "application/json")
	for key, value := range headers {
		request.Header.Set(key, value)
	}
	response, err := (&http.Client{Timeout: timeout}).Do(request)
	if err != nil {
		return nil, errors.New("web provider request failed")
	}
	if response.StatusCode == http.StatusUnauthorized || response.StatusCode == http.StatusForbidden {
		response.Body.Close()
		return nil, errProviderAuthentication
	}
	if response.StatusCode == http.StatusTooManyRequests {
		response.Body.Close()
		return nil, errors.New("web provider rate limit reached")
	}
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		response.Body.Close()
		return nil, errors.New("web provider request failed")
	}
	raw, err := boundedBody(response, 5<<20)
	if err != nil {
		return nil, err
	}
	var value map[string]any
	if json.Unmarshal(raw, &value) != nil {
		return nil, errors.New("web provider response is invalid")
	}
	return value, nil
}

func nodeText(node *html.Node) string {
	if node == nil {
		return ""
	}
	var values []string
	var walk func(*html.Node)
	walk = func(current *html.Node) {
		if current.Type == html.TextNode {
			values = append(values, current.Data)
		}
		for child := current.FirstChild; child != nil; child = child.NextSibling {
			walk(child)
		}
	}
	walk(node)
	return normalizeText(strings.Join(values, " "))
}

func attribute(node *html.Node, key string) string {
	for _, attribute := range node.Attr {
		if attribute.Key == key {
			return attribute.Val
		}
	}
	return ""
}

func normalizeText(value string) string { return strings.Join(strings.Fields(value), " ") }
func stringValue(value map[string]any, key string) string {
	result, _ := value[key].(string)
	return result
}
