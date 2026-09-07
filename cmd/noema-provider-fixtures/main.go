// Command noema-provider-fixtures serves the synthetic provider contracts used
// by the live personal-assistant acceptance cases.
//
// It intentionally runs outside Noema's home and does not expose a Noema
// socket, database, credential, or task directory. The public routes are
// documentation and provider protocol surfaces only. Operator request traces
// are available from the unlisted /fixture/requests route.
package main

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"flag"
	"fmt"
	"log"
	"net/http"
	"os"
	"strconv"
	"strings"
	"sync"
	"time"

	mcp "github.com/modelcontextprotocol/go-sdk/mcp"
)

const (
	fixtureVersion = "2026-09-07-gmail-v1-notion-mcp-v4"
	accountAToken  = "fixture-account-a"
	accountBToken  = "fixture-account-b"
)

type requestTrace struct {
	At      string `json:"at"`
	Method  string `json:"method"`
	Path    string `json:"path"`
	Query   string `json:"query,omitempty"`
	Account string `json:"account,omitempty"`
	Status  int    `json:"status"`
}

type fixture struct {
	mu       sync.Mutex
	requests []requestTrace
	oauth    fixtureOAuth
}

func (f *fixture) trace(r *http.Request, status int) {
	account := f.requestAccount(r)
	f.mu.Lock()
	defer f.mu.Unlock()
	f.requests = append(f.requests, requestTrace{
		At: time.Now().UTC().Format(time.RFC3339Nano), Method: r.Method,
		Path: r.URL.Path, Query: r.URL.RawQuery, Account: account, Status: status,
	})
}

func (f *fixture) requestAccount(r *http.Request) string {
	value := strings.TrimSpace(r.Header.Get("Authorization"))
	if strings.HasPrefix(value, "Bearer ") {
		switch strings.TrimPrefix(value, "Bearer ") {
		case accountAToken:
			return "account-a"
		case accountBToken:
			return "account-b"
		}
		f.mu.Lock()
		defer f.mu.Unlock()
		if token, ok := f.oauth.tokens[strings.TrimPrefix(value, "Bearer ")]; ok && time.Now().Before(token.expires) {
			return token.account
		}
	}
	return "anonymous"
}

func (f *fixture) traces() []requestTrace {
	f.mu.Lock()
	defer f.mu.Unlock()
	return append([]requestTrace(nil), f.requests...)
}

func writeJSON(w http.ResponseWriter, status int, value any) {
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(value)
}

func writeText(w http.ResponseWriter, status int, media, value string) {
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("Content-Type", media)
	w.WriteHeader(status)
	_, _ = w.Write([]byte(value))
}

func (f *fixture) health(w http.ResponseWriter, r *http.Request) {
	if r.URL.Path != "/health" {
		return
	}
	if r.Method != http.MethodGet {
		f.trace(r, http.StatusMethodNotAllowed)
		writeJSON(w, http.StatusMethodNotAllowed, map[string]any{"error": "method_not_allowed"})
		return
	}
	f.trace(r, http.StatusOK)
	writeJSON(w, http.StatusOK, map[string]any{"ok": true, "fixture": fixtureVersion, "services": []string{"gmail", "notion"}})
}

func (f *fixture) operatorTrace(w http.ResponseWriter, r *http.Request) {
	if r.URL.Path != "/fixture/requests" || r.Method != http.MethodGet {
		f.trace(r, http.StatusNotFound)
		writeJSON(w, http.StatusNotFound, map[string]string{"error": "not_found"})
		return
	}
	// This route is intentionally not linked from provider documentation. It
	// contains only ordinary request metadata and never stores auth headers.
	f.trace(r, http.StatusOK)
	writeJSON(w, http.StatusOK, map[string]any{"fixture": fixtureVersion, "requests": f.traces()})
}

func (f *fixture) docs(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		f.trace(r, http.StatusMethodNotAllowed)
		writeJSON(w, http.StatusMethodNotAllowed, map[string]string{"error": "method_not_allowed"})
		return
	}
	switch r.URL.Path {
	case "/gmail/docs":
		f.trace(r, http.StatusOK)
		writeText(w, http.StatusOK, "text/markdown; charset=utf-8", gmailDocs)
	case "/notion/docs":
		f.trace(r, http.StatusOK)
		writeText(w, http.StatusOK, "text/markdown; charset=utf-8", notionDocs)
	default:
		return
	}
}

type gmailMessage struct {
	ID           string       `json:"id"`
	ThreadID     string       `json:"threadId"`
	LabelIDs     []string     `json:"labelIds"`
	Snippet      string       `json:"snippet"`
	HistoryID    string       `json:"historyId"`
	InternalDate string       `json:"internalDate"`
	SizeEstimate int          `json:"sizeEstimate"`
	Payload      gmailPayload `json:"payload"`
}

type gmailPayload struct {
	PartID   string         `json:"partId,omitempty"`
	MimeType string         `json:"mimeType"`
	Filename string         `json:"filename,omitempty"`
	Headers  []gmailHeader  `json:"headers"`
	Body     gmailBody      `json:"body"`
	Parts    []gmailPayload `json:"parts,omitempty"`
}

type gmailHeader struct {
	Name  string `json:"name"`
	Value string `json:"value"`
}

type gmailBody struct {
	Size         int    `json:"size"`
	Data         string `json:"data,omitempty"`
	AttachmentID string `json:"attachmentId,omitempty"`
}

type gmailAccount struct {
	Email    string
	History  string
	Messages []gmailMessage
	Threads  map[string][]string
}

func gmailAccounts() map[string]gmailAccount {
	return map[string]gmailAccount{
		"account-a": {Email: "alex@example.test", History: "history-a-20260906", Messages: []gmailMessage{
			gmailMessageFor("a-msg-001", "a-thread-trip", []string{"INBOX", "IMPORTANT"}, "Trip itinerary confirmation", "Your upcoming trip itinerary is confirmed. Departure is Thursday.", "2026-09-05T08:15:00Z", "From: travel@example.test\nTo: alex@example.test\nSubject: Trip itinerary confirmation\n\nYour upcoming trip itinerary is confirmed. Departure is Thursday."),
			gmailMessageFor("a-msg-002", "a-thread-trip", []string{"INBOX"}, "Re: Trip itinerary confirmation", "The airline changed the connection. Please review the updated itinerary.", "2026-09-05T09:20:00Z", "From: travel@example.test\nTo: alex@example.test\nSubject: Re: Trip itinerary confirmation\n\nThe airline changed the connection. Please review the updated itinerary."),
			gmailMessageFor("a-msg-003", "a-thread-trip", []string{"INBOX"}, "Hotel booking for Lisbon", "The hotel booking for the upcoming trip is held until Monday.", "2026-09-05T10:30:00Z", "From: hotel@example.test\nTo: alex@example.test\nSubject: Hotel booking for Lisbon\n\nThe hotel booking for the upcoming trip is held until Monday."),
			gmailMessageFor("a-msg-004", "a-thread-work", []string{"INBOX"}, "Q3 planning notes", "The planning review is on Tuesday afternoon.", "2026-09-05T11:00:00Z", "From: manager@example.test\nTo: alex@example.test\nSubject: Q3 planning notes\n\nThe planning review is on Tuesday afternoon."),
			gmailMessageFor("a-msg-005", "a-thread-work", []string{"STARRED"}, "Reimbursement policy", "Receipts are due within thirty days.", "2026-09-04T12:00:00Z", "From: finance@example.test\nTo: alex@example.test\nSubject: Reimbursement policy\n\nReceipts are due within thirty days."),
			gmailMessageFor("a-msg-006", "a-thread-friend", []string{"INBOX"}, "Dinner next week", "Are you free for dinner next Wednesday?", "2026-09-03T18:00:00Z", "From: friend@example.test\nTo: alex@example.test\nSubject: Dinner next week\n\nAre you free for dinner next Wednesday?"),
			gmailMessageWithAttachment("a-msg-007", "a-thread-trip", []string{"INBOX", "IMPORTANT"}, "Packing list for upcoming trip", "Here is the packing list for the upcoming trip.", "2026-09-03T19:00:00Z", "trip-checklist.txt", "passport\ncharger\nwalking shoes\n"),
		}, Threads: map[string][]string{"a-thread-trip": {"a-msg-001", "a-msg-002", "a-msg-003", "a-msg-007"}, "a-thread-work": {"a-msg-004", "a-msg-005"}, "a-thread-friend": {"a-msg-006"}}},
		"account-b": {Email: "blair@example.test", History: "history-b-20260906", Messages: []gmailMessage{
			gmailMessageFor("b-msg-001", "b-thread-private", []string{"INBOX"}, "Private account note", "This message belongs to the second synthetic account.", "2026-09-05T08:00:00Z", "From: private@example.test\nTo: blair@example.test\nSubject: Private account note\n\nThis message belongs to the second synthetic account."),
		}, Threads: map[string][]string{"b-thread-private": {"b-msg-001"}}},
	}
}

func gmailMessageFor(id, thread string, labels []string, subject, snippet, timestamp, body string) gmailMessage {
	encoded := base64.RawURLEncoding.EncodeToString([]byte(body))
	return gmailMessage{ID: id, ThreadID: thread, LabelIDs: labels, Snippet: snippet, HistoryID: "history-" + id,
		InternalDate: strconv.FormatInt(parseTime(timestamp).UnixMilli(), 10), SizeEstimate: len(body),
		Payload: gmailPayload{MimeType: "text/plain", Headers: []gmailHeader{{Name: "From", Value: headerValue(body, "From")}, {Name: "To", Value: headerValue(body, "To")}, {Name: "Subject", Value: subject}, {Name: "Date", Value: timestamp}}, Body: gmailBody{Size: len(body), Data: encoded}}}
}

func gmailMessageWithAttachment(id, thread string, labels []string, subject, snippet, timestamp, filename, attachment string) gmailMessage {
	body := "From: travel@example.test\nTo: alex@example.test\nSubject: " + subject + "\n\n" + snippet
	encoded := base64.RawURLEncoding.EncodeToString([]byte(body))
	attachmentID := "a-attachment-001"
	part := gmailPayload{PartID: "1", MimeType: "text/plain", Filename: filename, Headers: []gmailHeader{{Name: "Content-Disposition", Value: "attachment; filename=\"" + filename + "\""}}, Body: gmailBody{Size: len(attachment), AttachmentID: attachmentID}}
	textPart := gmailPayload{PartID: "0", MimeType: "text/plain", Headers: []gmailHeader{}, Body: gmailBody{Size: len(body), Data: encoded}}
	return gmailMessage{ID: id, ThreadID: thread, LabelIDs: labels, Snippet: snippet, HistoryID: "history-" + id,
		InternalDate: strconv.FormatInt(parseTime(timestamp).UnixMilli(), 10), SizeEstimate: len(body) + len(attachment),
		Payload: gmailPayload{MimeType: "multipart/mixed", Headers: []gmailHeader{{Name: "From", Value: "travel@example.test"}, {Name: "To", Value: "alex@example.test"}, {Name: "Subject", Value: subject}, {Name: "Date", Value: timestamp}}, Body: gmailBody{Size: 0}, Parts: []gmailPayload{textPart, part}}}
}

func parseTime(value string) time.Time {
	result, err := time.Parse(time.RFC3339, value)
	if err != nil {
		return time.Unix(0, 0).UTC()
	}
	return result
}

func headerValue(body, name string) string {
	for _, line := range strings.Split(body, "\n") {
		if strings.HasPrefix(line, name+": ") {
			return strings.TrimPrefix(line, name+": ")
		}
	}
	return ""
}

func (f *fixture) gmail(w http.ResponseWriter, r *http.Request) {
	if !strings.HasPrefix(r.URL.Path, "/gmail/v1/") {
		return
	}
	account := f.requestAccount(r)
	accounts := gmailAccounts()
	if account != "account-a" && account != "account-b" {
		f.trace(r, http.StatusUnauthorized)
		w.Header().Set("WWW-Authenticate", `Bearer realm="gmail-fixture", scope="https://www.googleapis.com/auth/gmail.readonly"`)
		writeJSON(w, http.StatusUnauthorized, map[string]any{"error": map[string]any{"code": 401, "message": "Login Required", "errors": []map[string]string{{"domain": "global", "reason": "authError", "message": "Login Required"}}}})
		return
	}
	data := accounts[account]
	path := strings.TrimPrefix(r.URL.Path, "/gmail/v1/users/me/")
	if r.Method != http.MethodGet {
		f.trace(r, http.StatusMethodNotAllowed)
		writeJSON(w, http.StatusMethodNotAllowed, map[string]any{"error": map[string]any{"code": 405, "message": "Method Not Allowed"}})
		return
	}
	switch {
	case path == "profile":
		f.trace(r, http.StatusOK)
		writeJSON(w, http.StatusOK, map[string]any{"emailAddress": data.Email, "messagesTotal": len(data.Messages), "threadsTotal": len(data.Threads), "historyId": data.History})
	case path == "messages":
		f.gmailList(w, r, data)
	case strings.HasPrefix(path, "messages/") && strings.HasSuffix(path, "/attachments/a-attachment-001"):
		id := strings.TrimSuffix(strings.TrimPrefix(path, "messages/"), "/attachments/a-attachment-001")
		if id != "a-msg-007" || account != "account-a" {
			f.trace(r, http.StatusNotFound)
			writeJSON(w, http.StatusNotFound, gmailError(404, "Requested entity was not found."))
			return
		}
		f.trace(r, http.StatusOK)
		attachment := "passport\ncharger\nwalking shoes\n"
		writeJSON(w, http.StatusOK, map[string]any{"size": len(attachment), "data": base64.RawURLEncoding.EncodeToString([]byte(attachment))})
	case strings.HasPrefix(path, "messages/"):
		id := strings.TrimPrefix(path, "messages/")
		f.gmailMessage(w, r, data, id)
	case strings.HasPrefix(path, "threads/"):
		id := strings.TrimPrefix(path, "threads/")
		f.gmailThread(w, r, data, id)
	default:
		f.trace(r, http.StatusNotFound)
		writeJSON(w, http.StatusNotFound, gmailError(404, "Requested entity was not found."))
	}
}

func (f *fixture) gmailList(w http.ResponseWriter, r *http.Request, account gmailAccount) {
	query := strings.ToLower(strings.TrimSpace(r.URL.Query().Get("q")))
	matches := make([]gmailMessage, 0, len(account.Messages))
	for _, message := range account.Messages {
		if gmailMatches(message, query) {
			matches = append(matches, message)
		}
	}
	page := 0
	switch r.URL.Query().Get("pageToken") {
	case "":
	case "page-2":
		page = 1
	case "page-3":
		page = 2
	default:
		f.trace(r, http.StatusBadRequest)
		writeJSON(w, http.StatusBadRequest, gmailError(400, "Invalid page token."))
		return
	}
	pageSize := 3
	if raw := r.URL.Query().Get("maxResults"); raw != "" {
		if value, err := strconv.Atoi(raw); err == nil && value > 0 && value <= 100 {
			pageSize = min(value, 3)
		}
	}
	start := page * pageSize
	if start > len(matches) {
		start = len(matches)
	}
	end := start + pageSize
	if end > len(matches) {
		end = len(matches)
	}
	items := make([]map[string]string, 0, end-start)
	for _, message := range matches[start:end] {
		items = append(items, map[string]string{"id": message.ID, "threadId": message.ThreadID})
	}
	result := map[string]any{"messages": items, "resultSizeEstimate": len(matches)}
	if end < len(matches) {
		result["nextPageToken"] = "page-" + strconv.Itoa(page+2)
	}
	f.trace(r, http.StatusOK)
	writeJSON(w, http.StatusOK, result)
}

func gmailMatches(message gmailMessage, query string) bool {
	if query == "" {
		return true
	}
	terms := strings.Fields(strings.Trim(query, `"`))
	search := strings.ToLower(message.Snippet + " " + message.PayloadHeader("Subject"))
	for _, term := range terms {
		if !strings.Contains(search, strings.Trim(term, `"`)) {
			return false
		}
	}
	return true
}

func (message gmailMessage) PayloadHeader(name string) string {
	for _, header := range message.Payload.Headers {
		if strings.EqualFold(header.Name, name) {
			return header.Value
		}
	}
	return ""
}

func (f *fixture) gmailMessage(w http.ResponseWriter, r *http.Request, account gmailAccount, id string) {
	for _, message := range account.Messages {
		if message.ID == id {
			f.trace(r, http.StatusOK)
			writeJSON(w, http.StatusOK, message)
			return
		}
	}
	f.trace(r, http.StatusNotFound)
	writeJSON(w, http.StatusNotFound, gmailError(404, "Requested entity was not found."))
}

func (f *fixture) gmailThread(w http.ResponseWriter, r *http.Request, account gmailAccount, id string) {
	ids, ok := account.Threads[id]
	if !ok {
		f.trace(r, http.StatusNotFound)
		writeJSON(w, http.StatusNotFound, gmailError(404, "Requested entity was not found."))
		return
	}
	byID := make(map[string]gmailMessage, len(account.Messages))
	for _, message := range account.Messages {
		byID[message.ID] = message
	}
	messages := make([]gmailMessage, 0, len(ids))
	for _, messageID := range ids {
		messages = append(messages, byID[messageID])
	}
	f.trace(r, http.StatusOK)
	writeJSON(w, http.StatusOK, map[string]any{"id": id, "historyId": account.History, "messages": messages})
}

func gmailError(code int, message string) map[string]any {
	return map[string]any{"error": map[string]any{"code": code, "message": message, "errors": []map[string]string{{"domain": "global", "reason": "notFound", "message": message}}}}
}

type notionPage struct {
	ID         string
	URL        string
	Title      string
	Content    string
	UpdatedAt  string
	Parent     string
	Object     string
	Blocks     []notionBlock
	Rows       []map[string]any
	Accessible bool
}

type notionBlock struct {
	ID       string
	Type     string
	Markdown string
	Children []notionBlock
}

func notionPages() []notionPage {
	return []notionPage{
		{ID: "notion-page-launch", URL: "https://www.notion.so/notion-page-launch", Title: "Launch project", Content: "Launch target: 2026-09-18.\nOwner: Alex.\nOpen decision: choose the support window.\nThe analytics milestone is at risk because the event schema is not final.", UpdatedAt: "2026-09-05T15:00:00.000Z", Parent: "workspace-root", Object: "page", Accessible: true,
			Blocks: []notionBlock{{ID: "notion-block-launch-details", Type: "callout", Markdown: "Support window: choose coverage before launch.\nRollback owner: still unassigned.", Children: []notionBlock{{ID: "notion-block-launch-analytics", Type: "bulleted_list_item", Markdown: "Analytics event schema is the remaining milestone dependency."}}}}},
		{ID: "notion-page-launch-notes", URL: "https://www.notion.so/notion-page-launch-notes", Title: "Launch project meeting notes", Content: "Decision needed: support coverage and rollback owner.\nContradictory note: the draft says 2026-09-20, but the approved target is 2026-09-18.", UpdatedAt: "2026-09-04T11:00:00.000Z", Parent: "notion-page-launch", Object: "page", Accessible: true,
			Blocks: []notionBlock{{ID: "notion-block-launch-notes", Type: "paragraph", Markdown: "The approved target remains 2026-09-18; the 2026-09-20 draft is stale."}}},
		{ID: "notion-page-launch-records", URL: "https://www.notion.so/notion-page-launch-records", Title: "Launch decision records", Content: "Related launch decision records.", UpdatedAt: "2026-09-03T09:00:00.000Z", Parent: "workspace-root", Object: "data_source", Accessible: true,
			Rows: []map[string]any{{"id": "launch-record-001", "decision": "support window", "status": "open"}, {"id": "launch-record-002", "decision": "rollback owner", "status": "open"}}},
		{ID: "notion-page-launch-archive", URL: "https://www.notion.so/notion-page-launch-archive", Title: "Launch project archive", Content: "Archived launch notes are retained for reference.", UpdatedAt: "2026-08-30T09:00:00.000Z", Parent: "workspace-root", Object: "page", Accessible: true},
		{ID: "notion-page-private", URL: "https://www.notion.so/notion-page-private", Title: "Private account page", Content: "This page belongs to the second synthetic account.", UpdatedAt: "2026-09-05T10:00:00.000Z", Parent: "workspace-private", Object: "page", Accessible: false},
	}
}

func accessibleNotionPages() []notionPage {
	pages := notionPages()
	result := make([]notionPage, 0, len(pages))
	for _, page := range pages {
		if page.Accessible {
			result = append(result, page)
		}
	}
	return result
}

func (f *fixture) notionCard(w http.ResponseWriter, r *http.Request) {
	if r.URL.Path != "/.well-known/mcp.json" || r.Method != http.MethodGet {
		return
	}
	f.trace(r, http.StatusOK)
	writeJSON(w, http.StatusOK, map[string]any{
		"name": "Notion", "description": "Synthetic Notion MCP server implementing the pinned read contract.",
		"endpoint": "https://noema.kevinpei.com/__pa-replay/notion/mcp",
	})
}

func (f *fixture) notionOAuthMetadata(w http.ResponseWriter, r *http.Request) {
	if r.URL.Path != "/.well-known/oauth-protected-resource" {
		return
	}
	// The first vertical slice uses a synthetic anonymous MCP connection. Keep
	// the route absent from the public card so Noema does not start a real OAuth
	// flow against this test process.
	f.trace(r, http.StatusNotFound)
	writeJSON(w, http.StatusNotFound, map[string]string{"error": "not_found"})
}

func (f *fixture) notionHandler() http.Handler {
	server := mcp.NewServer(&mcp.Implementation{Name: "notion-fixture", Title: "Notion", Version: fixtureVersion}, nil)
	readOnly, openWorld := true, true
	unsafe := false
	server.AddTool(&mcp.Tool{Name: "notion-get-self", Description: "Return the connected workspace and current user identity.", InputSchema: map[string]any{"type": "object", "additionalProperties": false}, OutputSchema: map[string]any{"type": "object"}, Annotations: &mcp.ToolAnnotations{ReadOnlyHint: readOnly, IdempotentHint: true, DestructiveHint: &unsafe, OpenWorldHint: &openWorld}}, notionTool(f, "notion-get-self"))
	server.AddTool(&mcp.Tool{Name: "notion-search", Description: "Search accessible Notion pages by concise content or title terms.", InputSchema: map[string]any{"type": "object", "properties": map[string]any{"query": map[string]any{"type": "string"}, "page_size": map[string]any{"type": "integer", "minimum": 1, "maximum": 50}, "start_cursor": map[string]any{"type": "string"}}, "required": []string{"query"}, "additionalProperties": false}, OutputSchema: map[string]any{"type": "object"}, Annotations: &mcp.ToolAnnotations{ReadOnlyHint: readOnly, IdempotentHint: true, DestructiveHint: &unsafe, OpenWorldHint: &openWorld}}, notionTool(f, "notion-search"))
	server.AddTool(&mcp.Tool{Name: "notion-fetch", Description: "Fetch one accessible page by its Notion ID or URL.", InputSchema: map[string]any{"type": "object", "properties": map[string]any{"id": map[string]any{"type": "string"}}, "required": []string{"id"}, "additionalProperties": false}, OutputSchema: map[string]any{"type": "object"}, Annotations: &mcp.ToolAnnotations{ReadOnlyHint: readOnly, IdempotentHint: true, DestructiveHint: &unsafe, OpenWorldHint: &openWorld}}, notionTool(f, "notion-fetch"))
	// Caddy terminates TLS and forwards the public host over loopback. The SDK's
	// localhost host check would reject that trusted reverse-proxy hop, so this
	// fixture disables only that check while the listener remains synthetic.
	handler := mcp.NewStreamableHTTPHandler(func(*http.Request) *mcp.Server { return server }, &mcp.StreamableHTTPOptions{JSONResponse: true, MaxRequestBodyBytes: 1 << 20, DisableLocalhostProtection: true})
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/notion/mcp" {
			return
		}
		f.trace(r, http.StatusOK)
		handler.ServeHTTP(w, r)
	})
}

func notionTool(f *fixture, name string) mcp.ToolHandler {
	return func(_ context.Context, request *mcp.CallToolRequest) (*mcp.CallToolResult, error) {
		var arguments map[string]any
		if len(request.Params.Arguments) != 0 && json.Unmarshal(request.Params.Arguments, &arguments) != nil {
			return toolError("invalid_arguments"), nil
		}
		if arguments == nil {
			arguments = map[string]any{}
		}
		pages := notionPages()
		var value any
		switch name {
		case "notion-get-self":
			value = map[string]any{"self": map[string]any{"id": "notion-user-a", "name": "Alex Example", "type": "person", "email": "alex@example.test"}, "workspace": map[string]any{"id": "notion-workspace-a", "name": "Personal acceptance workspace"}, "current_tool_access": map[string]any{"ai_search": map[string]any{"status": "not_available"}, "search": map[string]any{"status": "available"}}}
		case "notion-search":
			query, _ := arguments["query"].(string)
			cursor, _ := arguments["start_cursor"].(string)
			pageSize := 10
			if raw, ok := arguments["page_size"].(float64); ok {
				pageSize = int(raw)
			}
			if pageSize < 1 || pageSize > 50 {
				return toolError("invalid_page_size"), nil
			}
			matches := make([]notionPage, 0)
			for _, page := range accessibleNotionPages() {
				if notionMatches(page, query) {
					matches = append(matches, page)
				}
			}
			start := 0
			if cursor != "" {
				if _, err := fmt.Sscanf(cursor, "page-%d", &start); err != nil || start < 1 || start >= len(matches) {
					return toolError("invalid_cursor"), nil
				}
			}
			end := min(start+pageSize, len(matches))
			results := make([]map[string]any, 0)
			for _, page := range matches[start:end] {
				object := page.Object
				if object == "" {
					object = "page"
				}
				results = append(results, map[string]any{"object": object, "id": page.ID, "url": page.URL, "title": page.Title, "highlight": firstLine(page.Content), "path": []string{"Launch"}})
			}
			hasMore := end < len(matches)
			var nextCursor any
			if hasMore {
				nextCursor = fmt.Sprintf("page-%d", end)
			}
			value = map[string]any{"results": results, "has_more": hasMore, "next_cursor": nextCursor}
		case "notion-fetch":
			id, _ := arguments["id"].(string)
			id = strings.TrimSuffix(id, "/")
			found := false
			for _, page := range pages {
				if !page.Accessible {
					continue
				}
				for _, block := range page.Blocks {
					if id == block.ID {
						value = map[string]any{"object": "block", "id": block.ID, "page_id": page.ID, "type": block.Type, "markdown": block.Markdown, "children": notionBlockChildren(block.Children), "truncated": false, "unknown_block_ids": []string{}, "unknown_block_count": 0}
						found = true
						break
					}
					for _, child := range block.Children {
						if id == child.ID {
							value = map[string]any{"object": "block", "id": child.ID, "page_id": page.ID, "type": child.Type, "markdown": child.Markdown, "children": notionBlockChildren(child.Children), "truncated": false, "unknown_block_ids": []string{}, "unknown_block_count": 0}
							found = true
							break
						}
					}
					if found {
						break
					}
				}
				if found {
					break
				}
				if page.Object == "data_source" && (id == page.ID || id == "collection://notion-collection-launch") {
					value = map[string]any{"object": "data_source", "id": "collection://notion-collection-launch", "title": page.Title, "properties": map[string]any{"decision": map[string]any{"type": "title"}, "status": map[string]any{"type": "select"}}, "results": page.Rows, "has_more": false, "next_cursor": nil}
					found = true
					break
				}
				if id == page.ID || id == page.URL {
					unknown := make([]string, 0, len(page.Blocks))
					for _, block := range page.Blocks {
						unknown = append(unknown, block.ID)
					}
					value = map[string]any{"object": "page", "id": page.ID, "url": page.URL, "title": page.Title, "markdown": page.Content, "page_last_edited_at": page.UpdatedAt, "parent": map[string]any{"type": "page_id", "page_id": page.Parent}, "verification": map[string]any{"state": "verified", "expires_at": "2026-09-12T00:00:00.000Z"}, "truncated": len(unknown) != 0, "unknown_block_ids": unknown, "unknown_block_count": len(unknown)}
					found = true
					break
				}
			}
			if !found {
				return toolError("object_not_found"), nil
			}
		default:
			return toolError("unknown_tool"), nil
		}
		encoded, _ := json.Marshal(value)
		return &mcp.CallToolResult{Content: []mcp.Content{&mcp.TextContent{Text: string(encoded)}}, StructuredContent: value}, nil
	}
}

func notionBlockChildren(blocks []notionBlock) []map[string]any {
	children := make([]map[string]any, 0, len(blocks))
	for _, block := range blocks {
		children = append(children, map[string]any{"object": "block", "id": block.ID, "type": block.Type, "markdown": block.Markdown, "children": notionBlockChildren(block.Children)})
	}
	return children
}

func notionMatches(page notionPage, query string) bool {
	terms := strings.Fields(strings.ToLower(strings.TrimSpace(query)))
	search := strings.ToLower(page.Title + " " + page.Content)
	for _, term := range terms {
		if !strings.Contains(search, term) {
			return false
		}
	}
	return true
}

func firstLine(value string) string {
	if index := strings.IndexByte(value, '\n'); index >= 0 {
		return value[:index]
	}
	return value
}

func toolError(code string) *mcp.CallToolResult {
	return &mcp.CallToolResult{IsError: true, Content: []mcp.Content{&mcp.TextContent{Text: `{"code":"` + code + `"}`}}}
}

func main() {
	port := flag.Int("port", 3742, "HTTP port")
	flag.Parse()
	if *port < 1 || *port > 65535 {
		log.Fatal("port must be between 1 and 65535")
	}
	f := &fixture{}
	f.oauth.secret = os.Getenv("NOEMA_FIXTURE_CLIENT_SECRET")
	mux := http.NewServeMux()
	mux.HandleFunc("/health", f.health)
	mux.HandleFunc("/fixture/requests", f.operatorTrace)
	mux.HandleFunc("/gmail/docs", f.docs)
	mux.HandleFunc("/oauth/authorize", f.authorize)
	mux.HandleFunc("/oauth/token", f.token)
	mux.HandleFunc("/notion/docs", f.docs)
	mux.HandleFunc("/.well-known/mcp.json", f.notionCard)
	mux.HandleFunc("/.well-known/oauth-protected-resource", f.notionOAuthMetadata)
	mux.HandleFunc("/gmail/v1/", f.gmail)
	mux.Handle("/notion/mcp", f.notionHandler())
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/" {
			f.trace(r, http.StatusOK)
			writeText(w, http.StatusOK, "text/plain; charset=utf-8", "Noema provider fixtures: Gmail REST and Notion MCP\n")
			return
		}
		f.trace(r, http.StatusNotFound)
		writeJSON(w, http.StatusNotFound, map[string]string{"error": "not_found"})
	})
	server := &http.Server{Addr: ":" + strconv.Itoa(*port), Handler: mux, ReadHeaderTimeout: 5 * time.Second}
	log.Printf("provider fixtures listening on %s (version %s)", server.Addr, fixtureVersion)
	if err := server.ListenAndServe(); err != nil && err != http.ErrServerClosed {
		log.Fatal(err)
	}
}

var gmailDocs = `# Gmail API fixture

Contract: ` + fixtureVersion + ` (Gmail API v1-shaped read contract).

This service exposes a narrow, read-only subset of the Gmail API for the Noema
acceptance cases. The wire format follows Google's Gmail API v1 resources.

Official references:

- https://developers.google.com/gmail/api/reference/rest
- https://developers.google.com/gmail/api/reference/rest/v1/users.messages/list
- https://developers.google.com/gmail/api/reference/rest/v1/users.messages/get
- https://developers.google.com/gmail/api/reference/rest/v1/users.threads/get
- https://developers.google.com/gmail/api/reference/rest/v1/users.messages.attachments/get

## Base URL and authentication

The public base URL is ` + "`https://noema.kevinpei.com/__pa-replay/gmail/v1`" + `.
Every API request needs ` + "`Authorization: Bearer <access token>`" + `.
The required scope is ` + "`https://www.googleapis.com/auth/gmail.readonly`" + `.
Use OAuth authorization code with PKCE (S256) and client_secret_post.
The authorization endpoint is https://noema.kevinpei.com/__pa-replay/oauth/authorize.
The token endpoint is https://noema.kevinpei.com/__pa-replay/oauth/token.
The operator imports the reviewed OAuth profile and protected client document.
Select one synthetic account on the consent page. No real account is used.
Do not put credentials in a message, document, or report.

The API returns Google's JSON error envelope. Unsupported methods return 405.
Missing or expired credentials return 401. A token without the required scope
returns 403. Unknown message, thread, or attachment IDs return 404.

## Read operations

` + "`GET /users/me/profile`" + ` returns a Gmail ` + "`Profile`" + ` with
` + "`emailAddress`" + `, ` + "`messagesTotal`" + `, ` + "`threadsTotal`" + `, and
` + "`historyId`" + `.

` + "`GET /users/me/messages`" + ` accepts Gmail's ` + "`q`" + ` search string,
` + "`pageToken`" + ` cursor, and ` + "`maxResults`" + ` bound. It returns
` + "`messages`" + ` entries containing ` + "`id`" + ` and ` + "`threadId`" + `,
` + "`resultSizeEstimate`" + `, and an optional ` + "`nextPageToken`" + `.
Treat the cursor as opaque and continue until ` + "`nextPageToken`" + ` is absent.

` + "`GET /users/me/messages/{id}?format=full`" + ` returns a Gmail ` + "`Message`" + `
with stable IDs, labels, ` + "`internalDate`" + ` in milliseconds, ` + "`snippet`" + `,
` + "`sizeEstimate`" + `, and a MIME ` + "`payload`" + `. MIME headers and body data
use Google's base64url encoding. Multipart payloads contain nested ` + "`parts`" + `.

` + "`GET /users/me/threads/{id}`" + ` returns a Gmail ` + "`Thread`" + ` with its
full ` + "`messages`" + ` in conversation order.

` + "`GET /users/me/messages/{messageId}/attachments/{attachmentId}`" + ` returns
` + "`size`" + ` and base64url ` + "`data`" + ` for the selected attachment.

The fixture has two synthetic accounts. Their records and IDs are separate.
The token's account determines which records are visible.
`

var notionDocs = `# Notion MCP fixture

Contract: ` + fixtureVersion + ` (pinned Streamable HTTP MCP read contract).

This service implements the selected read-only portion of Notion's hosted MCP
contract. It uses the real MCP JSON-RPC protocol over Streamable HTTP.

Official references:

- https://developers.notion.com/guides/mcp/overview
- https://developers.notion.com/guides/mcp/mcp-supported-tools
- https://developers.notion.com/guides/mcp/build-mcp-client

## Server card and transport

The service URL is ` + "`https://noema.kevinpei.com/__pa-replay/notion`" + `.
Its server card is at ` + "`/.well-known/mcp.json`" + ` and advertises the endpoint
` + "`https://noema.kevinpei.com/__pa-replay/notion/mcp`" + `.
The endpoint supports MCP Streamable HTTP with JSON responses. Clients must use
the standard ` + "`Accept: application/json, text/event-stream`" + ` and
` + "`Mcp-Protocol-Version`" + ` headers. The service uses normal MCP initialize,
tools/list, and tools/call messages.

The selected catalog includes these official Notion tool names:

- ` + "`notion-get-self`" + `: return workspace and connected-user identity.
- ` + "`notion-search`" + `: search accessible pages with ` + "`query`" + `,
  optional ` + "`page_size`" + `, and opaque ` + "`start_cursor`" + `.
- ` + "`notion-fetch`" + `: fetch a page by its Notion ID or URL using ` + "`id`" + `.

Search results preserve page IDs, URLs, titles, highlights, and path details.
Fetch results preserve page IDs, parent IDs, edit timestamps, verification
fields, and Markdown content. Large pages may set ` + "`truncated: true`" + ` and
return ` + "`unknown_block_ids`" + `; fetch each returned ID to recover the full
nested block tree. The fixture also exposes one related data source with rows.
Search uses opaque ` + "`page-N`" + ` cursors and returns ` + "`has_more`" + ` until all
matching records are read. An inaccessible ID returns an MCP tool error with
code ` + "`object_not_found`" + `. An invalid cursor returns ` + "`invalid_cursor`" + `.
The catalog and schemas are part of the acceptance evidence.

This first fixture slice uses an anonymous synthetic MCP connection. It does not
claim to emulate Notion OAuth consent. OAuth compatibility is a separate case.
`
