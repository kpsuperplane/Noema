package main

import (
	"context"
	"encoding/base64"
	"encoding/json"
	"net/http/httptest"
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
