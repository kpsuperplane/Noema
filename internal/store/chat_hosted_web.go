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
	result, err := conversationJSONObject(search.Result)
	if err != nil {
		return hostedSearchValues{}, errors.New("hosted web search result is invalid")
	}
	var action any
	if len(search.ProviderAction) != 0 {
		action, err = conversationJSON(search.ProviderAction)
		if err != nil || jsonObjectValue(action) == nil {
			return hostedSearchValues{}, errors.New("hosted web provider action is invalid")
		}
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
	parsed, err := url.Parse(trimmed)
	if err != nil || parsed.Scheme == "" || parsed.Host == "" {
		return redactedSensitiveWebURL
	}
	parsed.User = nil
	query := parsed.Query()
	for name := range query {
		if _, sensitive := hostedWebCredentialQueryNames[strings.ToLower(name)]; sensitive {
			query.Del(name)
		}
	}
	parsed.RawQuery = query.Encode()
	return parsed.String()
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
