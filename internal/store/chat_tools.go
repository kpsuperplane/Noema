package store

import (
	"bytes"
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strings"
	"time"
)

// ConversationToolCallInput is one validated provider call for durable storage.
type ConversationToolCallInput struct {
	ProviderRound  int
	OutputIndex    int
	ProviderCallID string
	ProviderName   string
	Name           string
	Arguments      json.RawMessage
}

// ConversationToolRound is one provider response that requests an immediate tool.
type ConversationToolRound struct {
	Commentary         string
	ProviderCommentary string
	Reasoning          []json.RawMessage
	Call               ConversationToolCallInput
}

// ConversationToolResultInput is one terminal result for a stored provider call.
type ConversationToolResultInput struct {
	CallItemID     string
	ProviderRound  int
	OutputIndex    int
	ProviderCallID string
	ProviderName   string
	Name           string
	Success        bool
	Payload        json.RawMessage
}

// ConversationProviderItems returns complete durable provider context in order.
func (s *Store) ConversationProviderItems(
	ctx context.Context,
	conversationID string,
) ([]ConversationItem, error) {
	if _, err := s.Conversation(ctx, conversationID); err != nil {
		return nil, err
	}
	rows, err := s.db.QueryContext(ctx, `
SELECT item_id, conversation_id, COALESCE(turn_id, ''), COALESCE(parent_item_id, ''),
       sequence_index, kind, status, author_actor_id, COALESCE(content_text, ''),
       COALESCE(provider_content_text, ''), payload_json, metadata_json, created_at_ms
FROM conversation_items
WHERE conversation_id = ? AND deleted_at_ms IS NULL AND (
    (kind IN ('user_text', 'assistant_text', 'reasoning') AND status = 'completed')
    OR (kind IN ('tool_call', 'tool_result')
        AND status IN ('completed', 'failed', 'cancelled', 'interrupted'))
)
ORDER BY sequence_index`, conversationID)
	if err != nil {
		return nil, fmt.Errorf("query provider conversation items: %w", err)
	}
	defer rows.Close()
	items := make([]ConversationItem, 0)
	for rows.Next() {
		item, err := scanConversationItem(rows)
		if err != nil {
			return nil, err
		}
		items = append(items, item)
	}
	return items, rows.Err()
}

// StartConversationToolRound stores provider output before one immediate read.
func (s *Store) StartConversationToolRound(
	ctx context.Context,
	turn ConversationTurn,
	round ConversationToolRound,
	now time.Time,
) ([]ConversationItem, error) {
	if round.Call.ProviderRound < 0 || round.Call.OutputIndex < 0 ||
		strings.TrimSpace(round.Call.ProviderCallID) == "" ||
		strings.TrimSpace(round.Call.ProviderName) == "" ||
		strings.TrimSpace(round.Call.Name) == "" {
		return nil, errors.New("conversation tool call is invalid")
	}
	arguments, err := conversationJSONObject(round.Call.Arguments)
	if err != nil {
		return nil, errors.New("conversation tool arguments are invalid")
	}
	for _, detail := range round.Reasoning {
		if !json.Valid(detail) {
			return nil, errors.New("conversation reasoning is invalid")
		}
	}
	if len(round.Commentary) > maxConversationText || len(round.ProviderCommentary) > maxConversationText {
		return nil, errors.New("conversation commentary is too large")
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, fmt.Errorf("begin conversation tool round: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	if err := requireActiveTurnTx(ctx, tx, turn, "input_received"); err != nil {
		return nil, err
	}
	var parentID string
	if err := tx.QueryRowContext(ctx, `
SELECT item_id FROM conversation_items
WHERE turn_id = ? AND kind = 'user_text' ORDER BY sequence_index LIMIT 1`, turn.ID).Scan(&parentID); err != nil {
		return nil, fmt.Errorf("find conversation user item: %w", err)
	}
	sequence, err := nextConversationSequenceTx(ctx, tx, turn.ConversationID)
	if err != nil {
		return nil, err
	}
	items := make([]ConversationItem, 0, 3)
	metadata := func(kind string, output int) map[string]any {
		return map[string]any{
			"turn_index": turn.TurnIndex, "output_index": output,
			"provider_round": round.Call.ProviderRound, "source": kind,
			"provider": "openrouter",
		}
	}
	if len(round.Reasoning) != 0 {
		item, err := insertConversationOutputTx(ctx, tx, ConversationItem{
			ID:             stableConversationOutputID(turn.ID, "reasoning", round.Call.ProviderRound, 0),
			ConversationID: turn.ConversationID, TurnID: turn.ID, ParentItemID: parentID,
			Sequence: sequence, Kind: ConversationReasoning, Status: "completed",
			AuthorActorID: "agent:primary",
			Payload:       map[string]any{"provider_details": round.Reasoning},
			Metadata:      metadata("provider_reasoning", 0), CreatedAt: now,
		})
		if err != nil {
			return nil, err
		}
		items = append(items, item)
		sequence++
	}
	if strings.TrimSpace(round.Commentary) != "" {
		commentaryMetadata := metadata("provider_commentary", 0)
		commentaryMetadata["stream_id"] = assistantStreamID(turn.ID)
		commentaryMetadata["response_index"] = 0
		item, err := insertConversationOutputTx(ctx, tx, ConversationItem{
			ID:             stableConversationOutputID(turn.ID, "assistant_text", round.Call.ProviderRound, 0),
			ConversationID: turn.ConversationID, TurnID: turn.ID, ParentItemID: parentID,
			Sequence: sequence, Kind: ConversationAssistantText, Status: "completed",
			AuthorActorID: "agent:primary", ContentText: round.Commentary,
			ProviderContentText: round.ProviderCommentary,
			Payload:             map[string]any{}, Metadata: commentaryMetadata, CreatedAt: now,
		})
		if err != nil {
			return nil, err
		}
		items = append(items, item)
		sequence++
	}
	callID := stableConversationOutputID(
		turn.ID, "tool_call", round.Call.ProviderRound, round.Call.OutputIndex,
	)
	activityID := fmt.Sprintf("tool_call:%s:%d:%d", turn.ID, round.Call.ProviderRound, round.Call.OutputIndex)
	item, err := insertConversationOutputTx(ctx, tx, ConversationItem{
		ID: callID, ConversationID: turn.ConversationID, TurnID: turn.ID,
		ParentItemID: parentID, Sequence: sequence, Kind: ConversationToolCall,
		Status: "running", AuthorActorID: "agent:primary",
		Payload: map[string]any{
			"id": activityID, "activity_kind": "tool_call", "status": "started",
			"title": "Tool call: " + round.Call.Name, "summary": round.Call.Name,
			"metadata": map[string]any{
				"turn_index": turn.TurnIndex, "output_index": round.Call.OutputIndex,
				"provider": "openrouter", "display": map[string]any{},
				"action": map[string]any{
					"id": activityID, "provider_call_id": round.Call.ProviderCallID,
					"provider_name": round.Call.ProviderName, "name": round.Call.Name,
					"payload": arguments,
				},
			},
		},
		Metadata: metadata("provider_action", round.Call.OutputIndex), CreatedAt: now,
	})
	if err != nil {
		return nil, err
	}
	items = append(items, item)
	if _, err := tx.ExecContext(ctx, `
UPDATE conversation_turns SET status = 'running', updated_at_ms = ?
WHERE turn_id = ? AND conversation_id = ?`, millis(now), turn.ID, turn.ConversationID); err != nil {
		return nil, fmt.Errorf("start conversation tool turn: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return nil, fmt.Errorf("commit conversation tool round: %w", err)
	}
	return items, nil
}

// FinishConversationToolCall atomically closes one call and stores its result.
func (s *Store) FinishConversationToolCall(
	ctx context.Context,
	turn ConversationTurn,
	result ConversationToolResultInput,
	now time.Time,
) (ConversationItem, error) {
	if result.CallItemID == "" || result.ProviderRound < 0 || result.OutputIndex < 0 ||
		strings.TrimSpace(result.ProviderCallID) == "" || strings.TrimSpace(result.ProviderName) == "" ||
		strings.TrimSpace(result.Name) == "" {
		return ConversationItem{}, errors.New("conversation tool result is invalid")
	}
	expectedCallID := stableConversationOutputID(
		turn.ID, "tool_call", result.ProviderRound, result.OutputIndex,
	)
	if result.CallItemID != expectedCallID {
		return ConversationItem{}, errors.New("conversation tool result call does not match its output")
	}
	payloadValue, err := conversationJSON(result.Payload)
	if err != nil {
		return ConversationItem{}, errors.New("conversation tool result payload is invalid")
	}
	status := "failed"
	if result.Success {
		status = "completed"
	}
	now = now.UTC()
	resultID := stableConversationOutputID(turn.ID, "tool_result", result.ProviderRound, result.OutputIndex)
	activityID := fmt.Sprintf("tool_result:%s:%d:%d", turn.ID, result.ProviderRound, result.OutputIndex)
	callActivityID := fmt.Sprintf("tool_call:%s:%d:%d", turn.ID, result.ProviderRound, result.OutputIndex)
	payload := map[string]any{
		"id": activityID, "activity_kind": "tool_result", "status": status,
		"title": "Tool result: " + result.Name, "summary": result.Name,
		"metadata": map[string]any{
			"turn_index": turn.TurnIndex, "output_index": result.OutputIndex,
			"provider": "openrouter", "display": map[string]any{},
			"action": map[string]any{
				"call_id": callActivityID, "provider_call_id": result.ProviderCallID,
				"provider_name": result.ProviderName, "name": result.Name,
				"success": result.Success, "payload": payloadValue,
			},
		},
	}
	metadata := map[string]any{
		"turn_index": turn.TurnIndex, "output_index": result.OutputIndex,
		"provider_round": result.ProviderRound, "source": "provider_action_result",
		"provider": "openrouter",
	}
	encodedPayload, _ := json.Marshal(payload)
	encodedMetadata, _ := json.Marshal(metadata)
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ConversationItem{}, fmt.Errorf("begin conversation tool result: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	var existingParent, existingStatus, existingPayload, existingMetadata string
	err = tx.QueryRowContext(ctx, `
SELECT COALESCE(parent_item_id, ''), status, payload_json, metadata_json
FROM conversation_items WHERE item_id = ? AND conversation_id = ?`,
		resultID, turn.ConversationID,
	).Scan(&existingParent, &existingStatus, &existingPayload, &existingMetadata)
	if err == nil {
		if existingParent != result.CallItemID || existingStatus != status ||
			existingPayload != string(encodedPayload) || existingMetadata != string(encodedMetadata) {
			return ConversationItem{}, errors.New("conversation tool result conflicts with stored data")
		}
		item, itemErr := conversationItemTx(ctx, tx, resultID)
		if itemErr != nil {
			return ConversationItem{}, itemErr
		}
		if err := tx.Commit(); err != nil {
			return ConversationItem{}, err
		}
		return item, nil
	}
	if !errors.Is(err, sql.ErrNoRows) {
		return ConversationItem{}, fmt.Errorf("inspect conversation tool result: %w", err)
	}
	var callStatus, kind, callPayloadJSON string
	if err := tx.QueryRowContext(ctx, `
SELECT status, kind, payload_json FROM conversation_items
WHERE item_id = ? AND conversation_id = ? AND turn_id = ?`,
		result.CallItemID, turn.ConversationID, turn.ID,
	).Scan(&callStatus, &kind, &callPayloadJSON); err != nil ||
		kind != string(ConversationToolCall) || callStatus != "running" {
		return ConversationItem{}, errors.New("conversation tool call is not running")
	}
	var callPayload map[string]any
	if json.Unmarshal([]byte(callPayloadJSON), &callPayload) != nil {
		return ConversationItem{}, errors.New("stored conversation tool call is invalid")
	}
	callMetadata, _ := callPayload["metadata"].(map[string]any)
	callAction, _ := callMetadata["action"].(map[string]any)
	if callAction["provider_call_id"] != result.ProviderCallID ||
		callAction["provider_name"] != result.ProviderName || callAction["name"] != result.Name {
		return ConversationItem{}, errors.New("conversation tool result does not match its call")
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE conversation_items SET status = ?, updated_at_ms = ? WHERE item_id = ?`,
		status, millis(now), result.CallItemID); err != nil {
		return ConversationItem{}, fmt.Errorf("close conversation tool call: %w", err)
	}
	sequence, err := nextConversationSequenceTx(ctx, tx, turn.ConversationID)
	if err != nil {
		return ConversationItem{}, err
	}
	item, err := insertConversationOutputTx(ctx, tx, ConversationItem{
		ID: resultID, ConversationID: turn.ConversationID, TurnID: turn.ID,
		ParentItemID: result.CallItemID, Sequence: sequence, Kind: ConversationToolResult,
		Status: status, AuthorActorID: "agent:primary", Payload: payload,
		Metadata: metadata, CreatedAt: now,
	})
	if err != nil {
		return ConversationItem{}, err
	}
	if err := tx.Commit(); err != nil {
		return ConversationItem{}, fmt.Errorf("commit conversation tool result: %w", err)
	}
	return item, nil
}

func requireActiveTurnTx(ctx context.Context, tx *sql.Tx, turn ConversationTurn, status string) error {
	var current string
	if err := tx.QueryRowContext(ctx, `
SELECT status FROM conversation_turns WHERE turn_id = ? AND conversation_id = ?`,
		turn.ID, turn.ConversationID).Scan(&current); err != nil {
		return errors.New("conversation turn is unavailable")
	}
	if current != status {
		return errors.New("conversation turn state changed")
	}
	return nil
}

func nextConversationSequenceTx(ctx context.Context, tx *sql.Tx, conversationID string) (int64, error) {
	var sequence int64
	if err := tx.QueryRowContext(ctx, `
SELECT 1 + COALESCE(MAX(sequence_index), 0) FROM conversation_items WHERE conversation_id = ?`,
		conversationID).Scan(&sequence); err != nil {
		return 0, fmt.Errorf("select conversation item sequence: %w", err)
	}
	return sequence, nil
}

func insertConversationOutputTx(
	ctx context.Context,
	tx *sql.Tx,
	item ConversationItem,
) (ConversationItem, error) {
	payload, err := json.Marshal(item.Payload)
	if err != nil {
		return ConversationItem{}, errors.New("conversation item payload is invalid")
	}
	metadata, err := json.Marshal(item.Metadata)
	if err != nil {
		return ConversationItem{}, errors.New("conversation item metadata is invalid")
	}
	var content, providerContent any
	if item.ContentText != "" {
		content = item.ContentText
	}
	if item.ProviderContentText != "" && item.ProviderContentText != item.ContentText {
		providerContent = item.ProviderContentText
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO conversation_items (
    item_id, conversation_id, turn_id, parent_item_id, sequence_index,
    kind, status, author_actor_id, content_text, provider_content_text,
    payload_json, metadata_json, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		item.ID, item.ConversationID, item.TurnID, item.ParentItemID, item.Sequence,
		item.Kind, item.Status, item.AuthorActorID, content, providerContent,
		string(payload), string(metadata), millis(item.CreatedAt), millis(item.CreatedAt)); err != nil {
		return ConversationItem{}, fmt.Errorf("create conversation output item: %w", err)
	}
	return conversationItemTx(ctx, tx, item.ID)
}

func conversationJSONObject(raw json.RawMessage) (map[string]any, error) {
	value, err := conversationJSON(raw)
	if err != nil {
		return nil, err
	}
	object, ok := value.(map[string]any)
	if !ok {
		return nil, errors.New("JSON value is not an object")
	}
	return object, nil
}

func conversationJSON(raw json.RawMessage) (any, error) {
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	var value any
	if err := decoder.Decode(&value); err != nil {
		return nil, err
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		if err == nil {
			return nil, errors.New("JSON has trailing data")
		}
		return nil, err
	}
	return value, nil
}
