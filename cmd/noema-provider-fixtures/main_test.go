package main

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	mcp "github.com/modelcontextprotocol/go-sdk/mcp"
)

func TestGmailListHasThreeCompletePages(t *testing.T) {
	f := &fixture{}
	cursor := ""
	seen := map[string]bool{}
	for page, count := range []int{3, 3, 1} {
		request := httptest.NewRequest("GET", "/gmail/v1/users/me/messages?maxResults=100&pageToken="+cursor, nil)
		request.Header.Set("Authorization", "Bearer "+accountAToken)
		recorder := httptest.NewRecorder()
		f.gmail(recorder, request)
		var result struct {
			Messages      []struct{ ID string }
			NextPageToken string
		}
		if recorder.Code != 200 || json.Unmarshal(recorder.Body.Bytes(), &result) != nil || len(result.Messages) != count {
			t.Fatalf("page %d: status %d, body %s", page, recorder.Code, recorder.Body)
		}
		for _, message := range result.Messages {
			if seen[message.ID] {
				t.Fatalf("duplicate message %s", message.ID)
			}
			seen[message.ID] = true
		}
		cursor = result.NextPageToken
		if (page < 2) != (cursor != "") {
			t.Fatalf("page %d has incorrect continuation", page)
		}
	}
	if len(seen) != 7 {
		t.Fatalf("message count = %d", len(seen))
	}
}

func TestGmailListSupportsDateBounds(t *testing.T) {
	f := &fixture{}
	request := httptest.NewRequest("GET", "/gmail/v1/users/me/messages?q=after:2026/09/06%20before:2026/09/08&maxResults=100", nil)
	request.Header.Set("Authorization", "Bearer "+accountAToken)
	recorder := httptest.NewRecorder()
	f.gmailList(recorder, request, gmailAccounts()["account-a"])
	var result struct {
		Messages []struct{ ID string } `json:"messages"`
	}
	if recorder.Code != http.StatusOK || json.Unmarshal(recorder.Body.Bytes(), &result) != nil {
		t.Fatalf("date-bounded list: status %d, body %s", recorder.Code, recorder.Body)
	}
	if len(result.Messages) != 2 || result.Messages[0].ID != "a-msg-004" || result.Messages[1].ID != "a-msg-005" {
		t.Fatalf("date-bounded messages = %#v", result.Messages)
	}
}

func TestAttachmentMessagePreservesTextAndAttachmentParts(t *testing.T) {
	message := gmailAccounts()["account-a"].Messages[6]
	payload := message.Payload
	if payload.MimeType != "multipart/mixed" || payload.Body.Data != "" || len(payload.Parts) != 2 {
		t.Fatal("multipart container must hold separate text and attachment parts")
	}
	text := payload.Parts[0]
	body, err := base64.RawURLEncoding.DecodeString(text.Body.Data)
	if err != nil || text.MimeType != "text/plain" || len(body) != text.Body.Size || text.Filename != "" {
		t.Fatal("text part did not preserve its encoded body")
	}
	attachment := payload.Parts[1]
	if attachment.Filename != "trip-checklist.txt" || attachment.Body.AttachmentID == "" || attachment.Body.Data != "" {
		t.Fatal("attachment part did not preserve its filename and separate body reference")
	}
}

func TestCalendarListPreservesGoogleShapeAndPagination(t *testing.T) {
	f := &fixture{}
	seen := map[string]bool{}
	pageToken := ""
	for page := 0; page < 2; page++ {
		request := httptest.NewRequest("GET", "/calendar/v3/calendars/primary/events?maxResults=100&pageToken="+pageToken, nil)
		request.Header.Set("Authorization", "Bearer "+accountAToken)
		recorder := httptest.NewRecorder()
		f.calendarAPI(recorder, request)
		var result struct {
			Items         []calendarEvent `json:"items"`
			NextPageToken string          `json:"nextPageToken"`
		}
		if recorder.Code != http.StatusOK || json.Unmarshal(recorder.Body.Bytes(), &result) != nil || len(result.Items) == 0 {
			t.Fatalf("calendar page %d: status %d, body %s", page, recorder.Code, recorder.Body)
		}
		for _, event := range result.Items {
			if seen[event.ID] || strings.HasPrefix(event.ID, "cal-b-") {
				t.Fatalf("unexpected or duplicate calendar event %s", event.ID)
			}
			seen[event.ID] = true
		}
		pageToken = result.NextPageToken
	}
	if len(seen) != 4 || pageToken != "" {
		t.Fatalf("calendar records = %#v, next page = %q", seen, pageToken)
	}
}

func TestCalendarWriteRoundTripStaysWithinAccount(t *testing.T) {
	f := &fixture{}
	body := `{"summary":"Synthetic planning hold","start":{"dateTime":"2026-09-10T09:00:00-07:00"},"end":{"dateTime":"2026-09-10T09:30:00-07:00"}}`
	request := httptest.NewRequest("POST", "/calendar/v3/calendars/primary/events", strings.NewReader(body))
	request.Header.Set("Authorization", "Bearer "+accountAToken)
	recorder := httptest.NewRecorder()
	f.calendarAPI(recorder, request)
	if recorder.Code != http.StatusCreated {
		t.Fatalf("insert status = %d, body = %s", recorder.Code, recorder.Body)
	}
	var created calendarEvent
	if json.Unmarshal(recorder.Body.Bytes(), &created) != nil || created.ID == "" {
		t.Fatal("insert did not return an event")
	}
	update := httptest.NewRequest("PATCH", "/calendar/v3/calendars/primary/events/"+created.ID, strings.NewReader(`{"summary":"Updated planning hold"}`))
	update.Header.Set("Authorization", "Bearer "+accountAToken)
	recorder = httptest.NewRecorder()
	f.calendarAPI(recorder, update)
	if recorder.Code != http.StatusOK || !strings.Contains(recorder.Body.String(), "Updated planning hold") {
		t.Fatalf("update status = %d, body = %s", recorder.Code, recorder.Body)
	}
	other := httptest.NewRequest("GET", "/calendar/v3/calendars/primary/events/"+created.ID, nil)
	other.Header.Set("Authorization", "Bearer "+accountBToken)
	recorder = httptest.NewRecorder()
	f.calendarAPI(recorder, other)
	if recorder.Code != http.StatusNotFound {
		t.Fatalf("account boundary status = %d", recorder.Code)
	}
	remove := httptest.NewRequest("DELETE", "/calendar/v3/calendars/primary/events/"+created.ID, nil)
	remove.Header.Set("Authorization", "Bearer "+accountAToken)
	recorder = httptest.NewRecorder()
	f.calendarAPI(recorder, remove)
	if recorder.Code != http.StatusNoContent {
		t.Fatalf("delete status = %d", recorder.Code)
	}
}

func TestNotionSearchPaginatesAndHidesPrivateRecords(t *testing.T) {
	first := callNotionTool(t, "notion-search", map[string]any{"query": "launch", "page_size": 1})
	if first["has_more"] != true || first["next_cursor"] != "page-1" {
		t.Fatalf("first search page = %#v", first)
	}
	seen := map[string]bool{}
	for _, raw := range first["results"].([]map[string]any) {
		seen[raw["id"].(string)] = true
	}
	cursor := first["next_cursor"].(string)
	for cursor != "" {
		page := callNotionTool(t, "notion-search", map[string]any{"query": "launch", "page_size": 1, "start_cursor": cursor})
		for _, raw := range page["results"].([]map[string]any) {
			id := raw["id"].(string)
			if seen[id] {
				t.Fatalf("duplicate search result %s", id)
			}
			seen[id] = true
		}
		if page["has_more"] == true {
			cursor = page["next_cursor"].(string)
		} else {
			cursor = ""
		}
	}
	if len(seen) != 4 || seen["notion-page-private"] {
		t.Fatalf("search records = %#v", seen)
	}
	invalid := callNotionTool(t, "notion-search", map[string]any{"query": "launch", "start_cursor": "stale-cursor"})
	if !invalidResult(invalid, "invalid_cursor") {
		t.Fatalf("invalid cursor result = %#v", invalid)
	}
}

func TestNotionFetchRecoversNestedBlocksAndRelatedRecords(t *testing.T) {
	page := callNotionTool(t, "notion-fetch", map[string]any{"id": "notion-page-launch"})
	if page["truncated"] != true || page["unknown_block_count"] != 1 {
		t.Fatalf("truncated page = %#v", page)
	}
	ids := page["unknown_block_ids"].([]string)
	if len(ids) != 1 || ids[0] != "notion-block-launch-details" {
		t.Fatalf("unknown blocks = %#v", ids)
	}
	block := callNotionTool(t, "notion-fetch", map[string]any{"id": ids[0]})
	if block["object"] != "block" || block["truncated"] != false {
		t.Fatalf("block = %#v", block)
	}
	children := block["children"].([]map[string]any)
	if len(children) != 1 || children[0]["id"] != "notion-block-launch-analytics" {
		t.Fatalf("nested children = %#v", children)
	}
	collection := callNotionTool(t, "notion-fetch", map[string]any{"id": "collection://notion-collection-launch"})
	if collection["object"] != "data_source" || len(collection["results"].([]map[string]any)) != 2 {
		t.Fatalf("related collection = %#v", collection)
	}
}

func callNotionTool(t *testing.T, name string, arguments map[string]any) map[string]any {
	t.Helper()
	encoded, err := json.Marshal(arguments)
	if err != nil {
		t.Fatal(err)
	}
	result, err := notionTool(&fixture{}, name)(context.Background(), &mcp.CallToolRequest{Params: &mcp.CallToolParamsRaw{Arguments: encoded}})
	if err != nil || result == nil {
		t.Fatalf("Notion %s call = %#v, %v", name, result, err)
	}
	if result.IsError {
		var value map[string]any
		if len(result.Content) != 0 {
			if text, ok := result.Content[0].(*mcp.TextContent); ok {
				_ = json.Unmarshal([]byte(text.Text), &value)
			}
		}
		return value
	}
	value, ok := result.StructuredContent.(map[string]any)
	if !ok {
		t.Fatalf("Notion %s structured result = %#v", name, result.StructuredContent)
	}
	return value
}

func invalidResult(value map[string]any, code string) bool {
	return value != nil && value["code"] == code
}
