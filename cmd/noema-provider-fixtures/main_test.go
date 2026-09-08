package main

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"
	"time"

	mcp "github.com/modelcontextprotocol/go-sdk/mcp"
)

func TestGmailListHasThreeCompletePages(t *testing.T) {
	f := &fixture{}
	cursor := ""
	seen := map[string]bool{}
	for page := 0; page < 10; page++ {
		request := httptest.NewRequest("GET", "/gmail/v1/users/me/messages?maxResults=100&pageToken="+cursor, nil)
		request.Header.Set("Authorization", "Bearer "+accountAToken)
		recorder := httptest.NewRecorder()
		f.gmail(recorder, request)
		var result struct {
			Messages      []struct{ ID string }
			NextPageToken string
		}
		if recorder.Code != 200 || json.Unmarshal(recorder.Body.Bytes(), &result) != nil {
			t.Fatalf("page %d: status %d, body %s", page, recorder.Code, recorder.Body)
		}
		for _, message := range result.Messages {
			if seen[message.ID] {
				t.Fatalf("duplicate message %s", message.ID)
			}
			seen[message.ID] = true
		}
		cursor = result.NextPageToken
		if cursor == "" {
			break
		}
	}
	if len(seen) != 20 {
		t.Fatalf("message count = %d", len(seen))
	}
}

func TestObligationControlAddsDateRevisionAndReceipt(t *testing.T) {
	f := &fixture{}
	if got := len(f.gmailAccount("account-a").Messages); got != 20 {
		t.Fatalf("initial account-a message count = %d", got)
	}
	for _, action := range []string{"obligations_extend", "obligations_receipt"} {
		recorder := httptest.NewRecorder()
		request := httptest.NewRequest(http.MethodPost, "/fixture/control", strings.NewReader(`{"action":"`+action+`"}`))
		f.operatorControl(recorder, request)
		if recorder.Code != http.StatusOK {
			t.Fatalf("control %s status = %d, body = %s", action, recorder.Code, recorder.Body)
		}
	}
	data := f.gmailAccount("account-a")
	if len(data.Messages) != 22 || len(data.Threads["a-thread-obligations"]) != 6 {
		t.Fatalf("updated obligation records = %d messages, %d thread entries", len(data.Messages), len(data.Threads["a-thread-obligations"]))
	}
	if len(f.gmailAccount("account-b").Messages) != 1 {
		t.Fatal("obligation records crossed the account boundary")
	}
	if got := len(f.notionPagesForAccount("account-a")); got != 8 {
		t.Fatalf("updated account-a Notion page count = %d", got)
	}
	for _, page := range f.notionPagesForAccount("account-b") {
		if strings.Contains(page.ID, "obligations") {
			t.Fatalf("obligation page crossed the Notion account boundary: %s", page.ID)
		}
	}
}

func TestReplyFixtureSeedsFiveClassesAndCorrection(t *testing.T) {
	f := &fixture{}
	data := f.gmailAccount("account-a")
	threads := []string{"a-thread-replies-client", "a-thread-replies-overdue", "a-thread-replies-friend", "a-thread-replies-newsletter", "a-thread-replies-resolved"}
	for _, thread := range threads {
		if len(data.Threads[thread]) == 0 {
			t.Fatalf("reply thread %s is empty", thread)
		}
	}
	if len(data.Threads["a-thread-replies-resolved"]) != 2 || len(data.Messages) != 20 {
		t.Fatalf("reply seed count = %d, resolved thread = %#v", len(data.Messages), data.Threads["a-thread-replies-resolved"])
	}
	correction := httptest.NewRecorder()
	request := httptest.NewRequest(http.MethodPost, "/fixture/control", strings.NewReader(`{"action":"replies_correction"}`))
	f.operatorControl(correction, request)
	if correction.Code != http.StatusOK {
		t.Fatalf("correction status = %d, body = %s", correction.Code, correction.Body)
	}
	data = f.gmailAccount("account-a")
	if len(data.Messages) != 21 || len(data.Threads["a-thread-replies-client"]) != 2 || data.Threads["a-thread-replies-client"][1] != "a-msg-023" {
		t.Fatalf("correction records = %d, client thread = %#v", len(data.Messages), data.Threads["a-thread-replies-client"])
	}
	correction = httptest.NewRecorder()
	f.operatorControl(correction, httptest.NewRequest(http.MethodPost, "/fixture/control", strings.NewReader(`{"action":"replies_correction"}`)))
	if correction.Code != http.StatusOK || len(f.gmailAccount("account-a").Messages) != 21 {
		t.Fatal("correction control was not idempotent")
	}
	if len(f.gmailAccount("account-b").Messages) != 1 {
		t.Fatal("reply records crossed the account boundary")
	}
}

func TestGmailSendReturnsReceiptAndAddsToExistingThread(t *testing.T) {
	f := &fixture{oauth: fixtureOAuth{tokens: map[string]fixtureToken{
		"send-token": {account: "account-a", scope: fixtureScope + " " + fixtureGmailSendScope, expires: time.Now().Add(time.Minute)},
	}}}
	body := "From: alex@example.test\nTo: client@example.test\nSubject: Re: Urgent: confirm launch support today\n\nI confirm the launch support window."
	encoded := base64.RawURLEncoding.EncodeToString([]byte(body))
	request := httptest.NewRequest(http.MethodPost, "/gmail/v1/users/me/messages/send", strings.NewReader(`{"raw":"`+encoded+`","threadId":"a-thread-replies-client"}`))
	request.Header.Set("Authorization", "Bearer send-token")
	recorder := httptest.NewRecorder()
	f.gmail(recorder, request)
	if recorder.Code != http.StatusOK {
		t.Fatalf("send status = %d, body = %s", recorder.Code, recorder.Body)
	}
	var sent gmailMessage
	if json.Unmarshal(recorder.Body.Bytes(), &sent) != nil || sent.ID == "" || sent.ThreadID != "a-thread-replies-client" || len(sent.LabelIDs) != 1 || sent.LabelIDs[0] != "SENT" {
		t.Fatalf("send receipt = %#v", sent)
	}
	threadRequest := httptest.NewRequest(http.MethodGet, "/gmail/v1/users/me/threads/a-thread-replies-client", nil)
	threadRequest.Header.Set("Authorization", "Bearer send-token")
	threadRecorder := httptest.NewRecorder()
	f.gmail(threadRecorder, threadRequest)
	if threadRecorder.Code != http.StatusOK || !strings.Contains(threadRecorder.Body.String(), sent.ID) {
		t.Fatalf("thread did not include sent message: status %d, body %s", threadRecorder.Code, threadRecorder.Body)
	}
}

func TestActionFixtureSeedsAgreementAndProposalSeparately(t *testing.T) {
	f := &fixture{}
	data := f.gmailAccount("account-a")
	seen := map[string]bool{}
	for _, message := range data.Messages {
		if strings.HasPrefix(message.ID, "a-msg-014") || strings.HasPrefix(message.ID, "a-msg-015") || strings.HasPrefix(message.ID, "a-msg-016") {
			seen[message.ID] = true
		}
	}
	if len(seen) != 3 {
		t.Fatalf("action message IDs = %#v", seen)
	}
	pages := f.notionPagesForAccount("account-a")
	var found bool
	for _, page := range pages {
		if page.ID == "notion-page-actions" {
			found = strings.Contains(page.Content, "Agreed action") && strings.Contains(page.Content, "without agreement")
		}
	}
	if !found {
		t.Fatal("Notion action page did not preserve both agreement and proposal language")
	}
}

func TestGmailListSupportsDateBounds(t *testing.T) {
	f := &fixture{}
	request := httptest.NewRequest("GET", "/gmail/v1/users/me/messages?q=after:2026/09/06%20before:2026/09/08%20launch%20support&maxResults=100", nil)
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
