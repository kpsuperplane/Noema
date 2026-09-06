package main

import (
	"encoding/base64"
	"encoding/json"
	"net/http/httptest"
	"testing"
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
