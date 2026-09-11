package adapter

import (
	"encoding/json"
	"net/url"
	"slices"
	"strconv"
	"strings"
	"testing"
)

func libraryTestDefinition(t *testing.T, id string) Definition {
	t.Helper()
	entries, err := bundledLibrary()
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		if entry.ID == id {
			return entry.definition
		}
	}
	t.Fatal("missing library entry")
	return Definition{}
}
func libraryTestOperation(t *testing.T, definition Definition, id string) CompiledOperation {
	t.Helper()
	for _, operation := range definition.Operations {
		if operation.OperationID == id {
			return operation
		}
	}
	t.Fatal("missing operation: " + id)
	return CompiledOperation{}
}

func TestLibraryContracts(t *testing.T) {
	service, _ := newOAuthService(t, "http://localhost:3737/adapter/oauth/callback")
	for _, id := range []string{"gmail", "google-calendar"} {
		definition := libraryTestDefinition(t, id)
		scope := "https://mail.google.com/"
		if id != "gmail" {
			scope = "https://www.googleapis.com/auth/calendar"
		}
		var ids []string
		for _, operation := range definition.Operations {
			ids = append(ids, operation.OperationID)
			if !operationScopesSatisfied(operation, []string{scope}) {
				t.Errorf("%s lacks full scope", operation.OperationID)
			}
			if !operation.Behavior.ReadOnly && operation.Retry != "never" {
				t.Fatal("write can retry")
			}
		}
		scopes, ok := definition.ScopeTarget(ids, nil)
		if !ok || !slices.Contains(scopes, scope) {
			t.Fatal("broad access is not requested")
		}
	}
	snapshot, err := service.Snapshot()
	if err != nil || len(snapshot.Definitions) != 0 || len(snapshot.Connections) != 0 {
		t.Fatal("listing installed authority")
	}
}

func TestLibraryGmailWire(t *testing.T) {
	definition := libraryTestDefinition(t, "gmail")
	search := libraryTestOperation(t, definition, "search_messages")
	request, _, err := encodeRequest(definition, search, json.RawMessage(`{"q":"from:a@example.test 日本語","continuation":"local"}`), "provider-page")
	if err != nil {
		t.Fatal(err)
	}
	u, _ := url.Parse(request.rawURL)
	if u.Path != "/gmail/v1/users/me/messages" || u.Query().Get("q") != "from:a@example.test 日本語" || u.Query().Get("pageToken") != "provider-page" || u.Query().Get("maxResults") != "12" {
		t.Fatal("search request changed")
	}
	for _, tc := range []struct{ id, input, response, want string }{
		{"get_message", `{"id":"m1"}`, `{"id":"m1","threadId":"t1","labelIds":["INBOX"],"payload":{"headers":[{"name":"Subject","value":"日本語"}],"parts":[{"mimeType":"text/plain","body":{"data":"SGVsbG8","size":5}},{"filename":"a.pdf","mimeType":"application/pdf","body":{"attachmentId":"attachment-id","size":200}}]}}`, "Hello"},
		{"get_thread", `{"id":"t1"}`, `{"id":"t1","messages":[{"id":"m1"}]}`, "m1"},
		{"get_attachment", `{"messageId":"m1","id":"a1"}`, `{"data":"YWJj","size":3}`, "YWJj"},
		{"create_draft", `{"raw":"YWJj","threadId":"t1"}`, `{"id":"d1","message":{"threadId":"t1"}}`, "d1"},
		{"send_message", `{"raw":"YWJj","threadId":"t1"}`, `{"id":"m1","threadId":"t1"}`, "m1"},
		{"modify_message_labels", `{"id":"m1","removeLabelIds":["INBOX","UNREAD"]}`, `{"id":"m1","threadId":"t1"}`, "m1"},
	} {
		t.Run(tc.id, func(t *testing.T) {
			operation := libraryTestOperation(t, definition, tc.id)
			request, _, err := encodeRequest(definition, operation, json.RawMessage(tc.input), "")
			if err != nil {
				t.Fatal(err)
			}
			if tc.id == "create_draft" && string(request.body) != `{"message":{"raw":"YWJj","threadId":"t1"}}` {
				t.Fatal("draft MIME body changed")
			}
			if tc.id == "modify_message_labels" && string(request.body) != `{"removeLabelIds":["INBOX","UNREAD"]}` {
				t.Fatal("label changes lost")
			}
			value, err := decodeResponse(httpResponse{status: 200, contentType: "application/json", body: []byte(tc.response)}, operation.Response)
			if err != nil {
				t.Fatal(err)
			}
			raw, _ := json.Marshal(value)
			if !strings.Contains(string(raw), tc.want) {
				t.Fatal("response lost content")
			}
			if tc.id == "get_message" && (!strings.Contains(string(raw), "attachment-id") || !strings.Contains(string(raw), "日本語")) {
				t.Fatal("mail metadata lost")
			}
		})
	}
	attachment := libraryTestOperation(t, definition, "get_attachment")
	raw, _ := json.Marshal(map[string]any{"data": strings.Repeat("a", 4097), "size": 4000})
	result, err := decodeResponse(httpResponse{200, "application/json", raw}, attachment.Response)
	if err != nil || result.(map[string]any)["too_large"] != true || result.(map[string]any)["data"] != "" {
		t.Fatal("oversized attachment appears complete")
	}
}

func TestLibraryCalendarWire(t *testing.T) {
	definition := libraryTestDefinition(t, "google-calendar")
	for _, tc := range []struct{ id, input, want string }{
		{"create_event", `{"calendarId":"primary","sendUpdates":"all","summary":"Meeting","start_time":"2026-09-12T09:00:00-07:00","end_time":"2026-09-12T10:00:00-07:00","start_zone":"America/Los_Angeles","end_zone":"America/Los_Angeles","recurrence":["RRULE:FREQ=WEEKLY"],"attendee_1":"a@example.test"}`, `"attendees":[{"email":"a@example.test"}]`},
		{"update_event", `{"calendarId":"primary","eventId":"e1","sendUpdates":"none","summary":"Changed"}`, `{"summary":"Changed"}`},
		{"respond_to_invitation", `{"calendarId":"primary","eventId":"e1","sendUpdates":"all","attendee_email":"me@example.test","response_status":"declined"}`, `"attendeesOmitted":true`},
		{"query_availability", `{"calendar_id":"a@example.test","timeMin":"2026-09-12T00:00:00Z","timeMax":"2026-09-13T00:00:00Z"}`, `"items":[{"id":"a@example.test"}]`},
	} {
		operation := libraryTestOperation(t, definition, tc.id)
		request, _, err := encodeRequest(definition, operation, json.RawMessage(tc.input), "")
		if err != nil || !strings.Contains(string(request.body), tc.want) {
			t.Fatalf("%s request: %s, %v", tc.id, request.body, err)
		}
		if tc.id == "create_event" && (!strings.Contains(string(request.body), `"timeZone":"America/Los_Angeles"`) || !strings.Contains(string(request.body), "RRULE:FREQ=WEEKLY")) {
			t.Fatal("calendar time or recurrence lost")
		}
		if tc.id == "update_event" && string(request.body) != tc.want {
			t.Fatal("omitted fields would overwrite event data")
		}
	}
	operation := libraryTestOperation(t, definition, "delete_event")
	if _, _, err := encodeRequest(definition, operation, json.RawMessage(`{"calendarId":"primary","eventId":"e1"}`), ""); err == nil {
		t.Fatal("notification decision omitted")
	}
	event := `{"id":"e1","start":{"dateTime":"2026-09-12T09:00:00-07:00","timeZone":"America/Los_Angeles"},"recurrence":["RRULE:FREQ=WEEKLY"],"attendees":[{"email":"a@example.test","responseStatus":"accepted"}]}`
	result, err := decodeResponse(httpResponse{200, "application/json", []byte(event)}, libraryTestOperation(t, definition, "get_event").Response)
	if err != nil {
		t.Fatal(err)
	}
	var actual, expected any
	_ = json.Unmarshal([]byte(result.(map[string]any)["record_json"].(string)), &actual)
	_ = json.Unmarshal([]byte(event), &expected)
	if !rustJSONEquivalent(actual, expected) {
		t.Fatal("event details changed")
	}
}

func libraryTestGrant(t *testing.T, service *Service, application OAuthApplication, scopes []string, label string) OAuthGrant {
	t.Helper()
	generation := randomHex()
	grant := OAuthGrant{SchemaVersion: 1, GrantID: randomHex(), ApplicationID: application.ApplicationID, AccountLabel: &label, Audience: "google-apis", GrantedScopes: scopes, DesiredScopes: scopes, AuthorityRevision: 2, TokenRevision: 2, TokenGeneration: &generation, Status: "active"}
	token := oauthGrantToken{SchemaVersion: 1, GenerationID: generation, AccessToken: "synthetic-library-access"}
	if err := service.files.installOAuthOwned("adapters/oauth-grants", grant.GrantID, "grant.json", grant, "tokens", generation, token); err != nil {
		t.Fatal(err)
	}
	return grant
}
func libraryTestApplication(t *testing.T, service *Service) OAuthApplication {
	t.Helper()
	application, err := service.ImportOAuthApplication(googleOAuthProfile().ProfileDigest, nil, []byte(`{"installed":{"client_id":"library-client","client_secret":"synthetic-library-secret"}}`), nil)
	if err != nil {
		t.Fatal(err)
	}
	return application
}

func TestLibraryPartialGrantAndDisabledTools(t *testing.T) {
	service, _ := newOAuthService(t, "http://localhost:3737/adapter/oauth/callback")
	entry := libraryTestDefinition(t, "gmail")
	definition, err := service.ConnectLibrary(t.Context(), "gmail", entry.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	grant := libraryTestGrant(t, service, libraryTestApplication(t, service), []string{"https://www.googleapis.com/auth/gmail.readonly"}, "Personal")
	_, connection, err := service.AttachOAuthConnection(t.Context(), definition.SemanticDigest, grant.GrantID, 2, "")
	if err != nil {
		t.Fatal(err)
	}
	connection, err = service.SaveConnectionPolicy(t.Context(), connection.ConnectionID, "1", 1, "allow_automatically", "always_ask")
	if err != nil {
		t.Fatal(err)
	}
	operation := libraryTestOperation(t, definition, "get_message")
	enabled := false
	_, err = service.ChangeTool(t.Context(), connection.ConnectionID, strconv.Itoa(connection.ConnectionRevision), operation.OperationID, operation.Digest, 1, &enabled, nil, false)
	if err != nil {
		t.Fatal(err)
	}
	bindings, err := service.Bindings()
	if err != nil {
		t.Fatal(err)
	}
	if len(bindings) == 0 {
		t.Fatal("partial grant lost usable reads")
	}
	for _, binding := range bindings {
		if !strings.HasPrefix(binding.Name, "enable.") && (!binding.Behavior.ReadOnly || binding.OperationID == "get_message") {
			t.Fatal("ungranted or disabled operation exposed")
		}
	}
	before, _ := service.Snapshot()
	_, err = service.ConnectLibrary(t.Context(), "gmail", entry.SemanticDigest)
	if err != nil {
		t.Fatal(err)
	}
	after, _ := service.Snapshot()
	if !rustJSONEquivalent(before.Connections, after.Connections) {
		t.Fatal("selection changed permissions")
	}
}

func TestLibrarySelectionPinsInstalledRevision(t *testing.T) {
	service, _ := newOAuthService(t, "http://localhost:3737/adapter/oauth/callback")
	current := libraryTestDefinition(t, "gmail")
	raw, _ := json.Marshal(current.Manifest)
	var older Manifest
	_ = json.Unmarshal(raw, &older)
	older.DefinitionRevision = "earlier-release"
	older.Operations = older.Operations[:1]
	installed, err := service.files.installDefinition(older, "https://developers.google.com/workspace/gmail/api/reference/rest", nil, nil)
	if err != nil {
		t.Fatal(err)
	}
	if err = service.reconcile(t.Context()); err != nil {
		t.Fatal(err)
	}
	for range 2 {
		selected, err := service.ConnectLibrary(t.Context(), "gmail", current.SemanticDigest)
		if err != nil || selected.SemanticDigest != installed.SemanticDigest || len(selected.Operations) != 1 {
			t.Fatal("selection replaced installed release")
		}
	}
	snapshot, _ := service.Snapshot()
	if len(snapshot.Definitions) != 1 || len(snapshot.Connections) != 0 {
		t.Fatal("selection created duplicate authority")
	}
}

func TestLibraryRejectsChangedAndInjectedSelections(t *testing.T) {
	service, _ := newOAuthService(t, "http://localhost:3737/adapter/oauth/callback")
	digest := libraryTestDefinition(t, "gmail").SemanticDigest
	for _, input := range []string{`{"library_id":"unknown","expected_digest":"` + digest + `"}`, `{"library_id":"gmail","expected_digest":"changed"}`, `{"library_id":"gmail","expected_digest":"` + digest + `","manifest":{"reviewed":true}}`} {
		if _, success := service.ExecuteSetup(ConnectLibraryTool, json.RawMessage(input)); success {
			t.Fatal("invalid selection accepted")
		}
	}
	snapshot, _ := service.Snapshot()
	if len(snapshot.Definitions) != 0 {
		t.Fatal("invalid selection installed a definition")
	}
}

func TestLibraryAccountsReuseApplicationWithoutCredentialDisclosure(t *testing.T) {
	service, _ := newOAuthService(t, "http://localhost:3737/adapter/oauth/callback")
	application := libraryTestApplication(t, service)
	grants := []OAuthGrant{libraryTestGrant(t, service, application, []string{"https://mail.google.com/", "https://www.googleapis.com/auth/calendar"}, "Personal 日本語"), libraryTestGrant(t, service, application, []string{"https://mail.google.com/", "https://www.googleapis.com/auth/calendar"}, "Office")}
	for _, id := range []string{"gmail", "google-calendar"} {
		definition, err := service.ConnectLibrary(t.Context(), id, libraryTestDefinition(t, id).SemanticDigest)
		if err != nil {
			t.Fatal(err)
		}
		for _, grant := range grants {
			_, connection, err := service.AttachOAuthConnection(t.Context(), definition.SemanticDigest, grant.GrantID, 2, "")
			if err != nil {
				t.Fatal(err)
			}
			repeated, other, err := service.AttachOAuthConnection(t.Context(), definition.SemanticDigest, grant.GrantID, 2, "")
			if err != nil || repeated.SemanticDigest != definition.SemanticDigest || connection.ConnectionID != other.ConnectionID {
				t.Fatal("account attachment duplicated")
			}
		}
	}
	snapshot, err := service.OAuthSnapshot()
	if err != nil || len(snapshot.Applications) != 1 || len(snapshot.Grants) != 2 {
		t.Fatal("application or account authority changed")
	}
	raw, _ := json.Marshal(snapshot)
	if strings.Contains(string(raw), "synthetic-library-") || !strings.Contains(string(raw), "Personal 日本語") || !strings.Contains(string(raw), "library-client") {
		t.Fatal("credential exclusion or ordinary metadata preservation failed")
	}
	connections, _ := service.files.connections()
	if len(connections) != 4 {
		t.Fatal("account connections merged")
	}
}
