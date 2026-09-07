package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"net/url"
	"strings"
	"time"
)

const maxConversationHostedSearches = 64
const redactedSensitiveWebURL = "[redacted sensitive web.fetch URL]"

var hostedWebCredentialQueryNames = map[string]struct{}{
	"access_token": {}, "api_key": {}, "apikey": {}, "client_assertion": {},
	"client_secret": {}, "code_verifier": {}, "device_code": {}, "id_token": {},
	"password": {}, "refresh_token": {}, "sig": {}, "user_code": {},
	"x-amz-security-token": {}, "x-amz-signature": {}, "x-goog-signature": {},
}

var hostedWebCredentialFields = map[string]struct{}{
	"access_token": {}, "api-key": {}, "api_key": {}, "apikey": {},
	"authorization": {}, "client_assertion": {}, "client_secret": {},
	"code_verifier": {}, "cookie": {}, "device_code": {}, "id_token": {},
	"password": {}, "proxy-authorization": {}, "proxy_authorization": {},
	"refresh_token": {}, "set-cookie": {}, "set_cookie": {}, "user_code": {},
	"x-api-key": {}, "x-auth-token": {}, "x_api_key": {}, "x_auth_token": {},
}

// ConversationWebSource is one exact public source used by hosted search.
type ConversationWebSource struct {
	Title string
	URL   string
}

// ConversationHostedSearch is one completed provider-hosted web action.
type ConversationHostedSearch struct {
	OutputIndex    int
	ID             string
	Name           string
	Status         string
	Arguments      json.RawMessage
	Result         json.RawMessage
	Sources        []ConversationWebSource
	ProviderAction json.RawMessage
}

// StoreConversationHostedSearches stores completed hosted actions in one transaction.
func (s *Store) StoreConversationHostedSearches(
	ctx context.Context,
	turn ConversationTurn,
	provider string,
	providerRound int,
	searches []ConversationHostedSearch,
	now time.Time,
) ([]ConversationItem, error) {
	provider = strings.TrimSpace(provider)
	if provider == "" || providerRound < 0 || len(searches) > maxConversationHostedSearches {
		return nil, errors.New("hosted web search batch is invalid")
	}
	if len(searches) == 0 {
		return nil, nil
	}
	normalized := make([]hostedSearchValues, 0, len(searches))
	seen := make(map[int]struct{}, len(searches))
	for _, search := range searches {
		if search.OutputIndex < 0 || strings.TrimSpace(search.Name) == "" || len(search.Status) > 64 {
			return nil, errors.New("hosted web search is invalid")
		}
		if _, exists := seen[search.OutputIndex]; exists {
			return nil, errors.New("hosted web search output index is duplicated")
		}
		seen[search.OutputIndex] = struct{}{}
		values, err := normalizeHostedSearch(search)
		if err != nil {
			return nil, err
		}
		normalized = append(normalized, values)
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, fmt.Errorf("begin hosted web search storage: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	var state, parentID string
	if err := tx.QueryRowContext(ctx, `
SELECT status FROM conversation_turns WHERE turn_id = ? AND conversation_id = ?`,
		turn.ID, turn.ConversationID,
	).Scan(&state); err != nil || state != "input_received" && state != "running" {
		return nil, errors.New("conversation turn state changed")
	}
	if err := tx.QueryRowContext(ctx, `
SELECT item_id FROM conversation_items
WHERE turn_id = ? AND kind IN ('user_text', 'multiple_choice_selection') ORDER BY sequence_index LIMIT 1`, turn.ID).Scan(&parentID); err != nil {
		return nil, fmt.Errorf("find conversation user item: %w", err)
	}
	sequence, err := nextConversationSequenceTx(ctx, tx, turn.ConversationID)
	if err != nil {
		return nil, err
	}
	items := make([]ConversationItem, 0, 2*len(normalized))
	for _, search := range normalized {
		correlationID := search.ID
		if correlationID == "" {
			correlationID = fmt.Sprintf("hosted_web_search:%s:%d:%d", turn.ID, providerRound, search.OutputIndex)
		}
		callID := stableConversationOutputID(turn.ID, "hosted_web_call", providerRound, search.OutputIndex)
		callActivityID := "tool_call:" + correlationID
		callAction := map[string]any{
			"id": correlationID, "provider_item_id": optionalHostedValue(search.ID),
			"provider_call_id": optionalHostedValue(search.ID), "provider_name": provider,
			"name": search.Name, "payload": search.Arguments,
			"hosted_web_search": true, "status": search.Status,
			"sources": search.Sources, "provider_action": search.ProviderAction,
		}
		call, err := insertConversationOutputTx(ctx, tx, ConversationItem{
			ID: callID, ConversationID: turn.ConversationID, TurnID: turn.ID,
			ParentItemID: parentID, Sequence: sequence, Kind: ConversationToolCall,
			Status: "completed", AuthorActorID: "agent:primary",
			Payload: hostedActivityPayload(
				callActivityID, "tool_call", "completed", search.Name, callAction,
				hostedCallDisplay(search.Name, search.Arguments), turn, provider, search.OutputIndex,
			),
			Metadata: hostedSearchMetadata(turn, provider, providerRound, search.OutputIndex), CreatedAt: now,
		})
		if err != nil {
			return nil, err
		}
		items = append(items, call)
		sequence++
		success := !strings.EqualFold(search.Status, "failed")
		resultStatus := "failed"
		if success {
			resultStatus = "completed"
		}
		resultAction := map[string]any{
			"call_id": correlationID, "provider_call_id": optionalHostedValue(search.ID),
			"provider_name": provider, "name": search.Name, "success": success,
			"payload": search.Result, "hosted_web_search": true,
		}
		result, err := insertConversationOutputTx(ctx, tx, ConversationItem{
			ID:             stableConversationOutputID(turn.ID, "hosted_web_result", providerRound, search.OutputIndex),
			ConversationID: turn.ConversationID, TurnID: turn.ID, ParentItemID: callID,
			Sequence: sequence, Kind: ConversationToolResult, Status: resultStatus,
			AuthorActorID: "agent:primary",
			Payload: hostedActivityPayload(
				"tool_result:"+correlationID, "tool_result", resultStatus, search.Name, resultAction,
				hostedResultDisplay(search.Name, search.Arguments, search.Result, success),
				turn, provider, search.OutputIndex,
			),
			Metadata: hostedSearchMetadata(turn, provider, providerRound, search.OutputIndex), CreatedAt: now,
		})
		if err != nil {
			return nil, err
		}
		items = append(items, result)
		sequence++
	}
	if err := tx.Commit(); err != nil {
		return nil, fmt.Errorf("commit hosted web search storage: %w", err)
	}
	return items, nil
}

type hostedSearchValues struct {
	ConversationHostedSearch
	Arguments, Result map[string]any
	Sources           []map[string]any
	ProviderAction    any
}

func normalizeHostedSearch(search ConversationHostedSearch) (hostedSearchValues, error) {
	arguments, err := conversationJSONObject(search.Arguments)
	if err != nil {
		return hostedSearchValues{}, errors.New("hosted web search arguments are invalid")
	}
	arguments = sanitizeHostedWebObject(arguments)
	result, err := conversationJSONObject(search.Result)
	if err != nil {
		return hostedSearchValues{}, errors.New("hosted web search result is invalid")
	}
	result = sanitizeHostedWebObject(result)
	var action any
	if len(search.ProviderAction) != 0 {
		action, err = conversationJSON(search.ProviderAction)
		if err != nil || jsonObjectValue(action) == nil {
			return hostedSearchValues{}, errors.New("hosted web provider action is invalid")
		}
		action = sanitizeHostedWebValue(action)
	}
	sources := make([]map[string]any, 0, len(search.Sources))
	for _, source := range search.Sources {
		parsed, err := url.Parse(strings.TrimSpace(source.URL))
		if err != nil || parsed.Host == "" || parsed.User != nil || parsed.Scheme != "http" && parsed.Scheme != "https" {
			return hostedSearchValues{}, errors.New("hosted web source URL is invalid")
		}
		sources = append(sources, map[string]any{"title": strings.TrimSpace(source.Title), "url": parsed.String()})
	}
	return hostedSearchValues{
		ConversationHostedSearch: search, Arguments: arguments, Result: result,
		Sources: sources, ProviderAction: action,
	}, nil
}

func jsonObjectValue(value any) map[string]any {
	object, _ := value.(map[string]any)
	return object
}

func hostedActivityPayload(
	id, kind, status, name string,
	action, display map[string]any,
	turn ConversationTurn,
	provider string,
	outputIndex int,
) map[string]any {
	return map[string]any{
		"id": id, "activity_kind": kind, "status": status,
		"title":   "Tool " + strings.TrimPrefix(kind, "tool_") + ": " + name,
		"summary": name,
		"metadata": map[string]any{
			"turn_index": turn.TurnIndex, "output_index": outputIndex,
			"provider": provider, "action": action, "display": display,
		},
	}
}

func hostedSearchMetadata(
	turn ConversationTurn,
	provider string,
	providerRound, outputIndex int,
) map[string]any {
	return map[string]any{
		"turn_index": turn.TurnIndex, "output_index": outputIndex,
		"provider_round": providerRound, "source": "provider_hosted_web", "provider": provider,
	}
}

func hostedCallDisplay(name string, arguments map[string]any) map[string]any {
	identity, access := "Web Search", "Searches public web"
	if name == "web.fetch" {
		identity, access = "Fetched Web Page", "Fetches public web pages"
	}
	display := map[string]any{"name": identity, "access": access}
	if target, ok := arguments["query"].(string); ok && strings.TrimSpace(target) != "" {
		display["target"] = strings.TrimSpace(target)
	} else if target, ok := arguments["url"].(string); ok && strings.TrimSpace(target) != "" {
		display["target"] = hostedDisplayURL(target)
	}
	display["marker"] = hostedMarker(identity, name, "complete", display["target"])
	return display
}

func hostedDisplayURL(raw string) string {
	trimmed := strings.TrimSpace(raw)
	if trimmed == redactedSensitiveWebURL {
		return trimmed
	}
	sanitized, _ := sanitizeHostedWebURL(trimmed)
	if sanitized == "" {
		return redactedSensitiveWebURL
	}
	return sanitized
}

func sanitizeHostedWebObject(value map[string]any) map[string]any {
	return sanitizeHostedWebValue(value).(map[string]any)
}

func sanitizeHostedWebValue(value any) any {
	switch current := value.(type) {
	case map[string]any:
		rejected := false
		for key, child := range current {
			lower := strings.ToLower(key)
			if _, sensitive := hostedWebCredentialFields[lower]; sensitive {
				current[key] = "[REDACTED]"
				continue
			}
			if lower == "url" || lower == "final_url" {
				if childURL, ok := child.(string); ok {
					sanitized, removed := sanitizeHostedWebURL(childURL)
					current[key], rejected = sanitized, rejected || removed
					continue
				}
			}
			current[key] = sanitizeHostedWebValue(child)
		}
		if rejected {
			current["__noema_rejected_sensitive_url"] = true
		}
	case []any:
		for index, child := range current {
			current[index] = sanitizeHostedWebValue(child)
		}
	}
	return value
}

func sanitizeHostedWebURL(raw string) (string, bool) {
	trimmed := strings.TrimSpace(raw)
	if trimmed == redactedSensitiveWebURL {
		return trimmed, false
	}
	parsed, err := url.Parse(trimmed)
	if err != nil || parsed.Scheme == "" || parsed.Host == "" {
		return redactedSensitiveWebURL, true
	}
	removed := parsed.User != nil
	parsed.User = nil
	parsed.RawQuery, removed = removeHostedWebCredentialPairs(parsed.RawQuery, false, removed)
	fragment, fragmentRemoved := removeHostedWebCredentialPairs(parsed.EscapedFragment(), true, false)
	if fragmentRemoved {
		if fragment == "" {
			parsed.Fragment, parsed.RawFragment = "", ""
		} else if decoded, decodeErr := url.PathUnescape(fragment); decodeErr == nil {
			parsed.Fragment, parsed.RawFragment = decoded, fragment
		} else {
			parsed.Fragment, parsed.RawFragment = fragment, ""
		}
		removed = true
	}
	return parsed.String(), removed
}

func removeHostedWebCredentialPairs(raw string, requireEquals, removed bool) (string, bool) {
	if raw == "" || requireEquals && !strings.Contains(raw, "=") {
		return raw, removed
	}
	parts := strings.Split(raw, "&")
	retained := parts[:0]
	for _, part := range parts {
		name := part
		if index := strings.IndexByte(part, '='); index >= 0 {
			name = part[:index]
		}
		decoded, err := url.QueryUnescape(name)
		if err == nil {
			if _, sensitive := hostedWebCredentialQueryNames[strings.ToLower(decoded)]; sensitive {
				removed = true
				continue
			}
		}
		retained = append(retained, part)
	}
	return strings.Join(retained, "&"), removed
}

func hostedResultDisplay(
	name string,
	arguments, result map[string]any,
	success bool,
) map[string]any {
	display := hostedCallDisplay(name, arguments)
	status := "complete"
	if !success {
		status = "error"
	}
	display["marker"] = hostedMarker(display["name"].(string), name, status, display["target"])
	if summary, ok := result["summary"].(string); ok && strings.TrimSpace(summary) != "" {
		display["result"] = strings.TrimSpace(summary)
	}
	return display
}

func hostedMarker(identity, kind, status string, target any) map[string]any {
	summary := identity
	if value, ok := target.(string); ok && value != "" {
		summary = value
	}
	detail := "Used " + identity
	if status == "error" {
		detail = identity + " failed"
	}
	marker := map[string]any{
		"identity": identity, "summary": summary, "detailTitle": detail,
		"status": status, "kind": kind,
	}
	if value, ok := target.(string); ok && value != "" {
		marker["subject"] = value
		if kind == "web.search" {
			marker["subjectLabel"] = "Search"
		} else {
			marker["subjectLabel"] = "Page"
		}
	}
	return marker
}

func optionalHostedValue(value string) any {
	if value == "" {
		return nil
	}
	return value
}
