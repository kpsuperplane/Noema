package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"time"

	"github.com/uptrace/bun"
)

// ConversationA2UIContinuation is one answered A2UI action ready for provider resumption.
type ConversationA2UIContinuation struct {
	Surface ConversationItem
	Call    ConversationItem
	Action  ConversationItem
	Result  ConversationItem
	Turn    ConversationTurn
}

func (s *Store) ConversationA2UIInteraction(ctx context.Context, conversationID, interactionID string) (ConversationItem, error) {
	if conversationID == "" || interactionID == "" {
		return ConversationItem{}, errors.New("A2UI interaction is invalid")
	}
	var id string
	err := s.db.QueryRowContext(ctx, `SELECT item_id FROM conversation_items
WHERE conversation_id = ? AND kind = 'a2ui_card'
AND json_extract(payload_json, '$.payload.interaction_id') = ?
ORDER BY sequence_index LIMIT 1`, conversationID, interactionID).Scan(&id)
	if err != nil {
		return ConversationItem{}, errors.New("A2UI interaction is missing")
	}
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return ConversationItem{}, err
	}
	defer func() { _ = tx.Rollback() }()
	item, err := conversationItemTx(ctx, tx, id)
	if err != nil {
		return ConversationItem{}, err
	}
	return item, tx.Commit()
}

// ResolveConversationA2UI atomically records one valid action and its provider result.
func (s *Store) ResolveConversationA2UI(
	ctx context.Context,
	conversationID, surfaceItemID, interactionID string,
	expectedRevision int,
	projection, resultPayload map[string]any,
	clientMessageID *string,
	now time.Time,
) (ConversationA2UIContinuation, error) {
	if conversationID == "" || surfaceItemID == "" || interactionID == "" || expectedRevision < 1 ||
		clientMessageID != nil && (*clientMessageID == "" || len(*clientMessageID) > 256) {
		return ConversationA2UIContinuation{}, errors.New("A2UI action is invalid")
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ConversationA2UIContinuation{}, err
	}
	defer func() { _ = tx.Rollback() }()
	surface, err := conversationItemTx(ctx, tx, surfaceItemID)
	storedProjection, _ := surface.Payload["payload"].(map[string]any)
	if err != nil || surface.ConversationID != conversationID || surface.Kind != ConversationA2UICard ||
		textJSON(surface.Payload["interaction_state"]) != "pending" ||
		textJSON(storedProjection["interaction_id"]) != interactionID ||
		intJSON(storedProjection["interaction_revision"]) != expectedRevision {
		return ConversationA2UIContinuation{}, errors.New("A2UI interaction is stale")
	}
	var turn ConversationTurn
	if err := tx.QueryRowContext(ctx, `SELECT turn_id, conversation_id,
CAST(json_extract(metadata_json, '$.turn_index') AS INTEGER), status
FROM conversation_turns WHERE turn_id = ? AND conversation_id = ?`, surface.TurnID, conversationID,
	).Scan(&turn.ID, &turn.ConversationID, &turn.TurnIndex, &turn.Status); err != nil {
		return ConversationA2UIContinuation{}, err
	}
	if turn.Status != "waiting_for_tool" {
		return ConversationA2UIContinuation{}, errors.New("A2UI turn is unavailable")
	}
	call, err := conversationItemTx(ctx, tx, textJSON(surface.Payload["call_item_id"]))
	if err != nil || call.Kind != ConversationToolCall || call.Status != "running" || call.TurnID != turn.ID {
		return ConversationA2UIContinuation{}, errors.New("A2UI tool call is unavailable")
	}
	actionValue, ok := choiceAction(call.Payload)
	if !ok {
		return ConversationA2UIContinuation{}, errors.New("A2UI tool call is invalid")
	}
	sequence, err := nextConversationSequenceTx(ctx, tx, conversationID)
	if err != nil {
		return ConversationA2UIContinuation{}, err
	}
	providerRound := intJSON(surface.Payload["provider_round"])
	outputIndex := providerOutputIndexValue(surface)
	actionID := stableConversationOutputID(turn.ID, "a2ui_action", providerRound, outputIndex)
	action, err := insertConversationOutputTx(ctx, tx, ConversationItem{
		ID: actionID, ConversationID: conversationID, TurnID: turn.ID,
		ParentItemID: surface.ID, Sequence: sequence, Kind: ConversationA2UICard,
		Status: "completed", AuthorActorID: "human:local",
		Payload: map[string]any{
			"id":     fmt.Sprintf("a2ui:%s:%d", turn.ID, expectedRevision+1),
			"schema": "a2ui.v0.9.1", "payload": projection,
		},
		Metadata: map[string]any{"source": "a2ui_action", "client_message_id": optionalText(clientMessageID)}, CreatedAt: now,
	})
	if err != nil {
		return ConversationA2UIContinuation{}, err
	}
	resultID := stableConversationOutputID(turn.ID, "tool_result", providerRound, outputIndex)
	result, err := insertConversationOutputTx(ctx, tx, ConversationItem{
		ID: resultID, ConversationID: conversationID, TurnID: turn.ID,
		ParentItemID: call.ID, Sequence: sequence + 1, Kind: ConversationToolResult,
		Status: "completed", AuthorActorID: "agent:primary",
		Payload: map[string]any{
			"id":            fmt.Sprintf("tool_result:%s:%d:%d", turn.ID, providerRound, outputIndex),
			"activity_kind": "tool_result", "status": "completed",
			"title": "Tool result: " + textJSON(actionValue["name"]), "summary": textJSON(actionValue["name"]),
			"metadata": map[string]any{
				"turn_index": turn.TurnIndex, "output_index": outputIndex,
				"provider": textJSON(surface.Metadata["provider"]), "display": map[string]any{},
				"action": map[string]any{
					"call_id":          fmt.Sprintf("tool_call:%s:%d:%d", turn.ID, providerRound, outputIndex),
					"provider_call_id": actionValue["provider_call_id"], "provider_name": actionValue["provider_name"],
					"name": actionValue["name"], "success": true, "payload": resultPayload,
				},
			},
		},
		Metadata: map[string]any{"turn_index": turn.TurnIndex, "output_index": outputIndex,
			"provider_round": providerRound, "source": "provider_action_result", "provider": textJSON(surface.Metadata["provider"])}, CreatedAt: now,
	})
	if err != nil {
		return ConversationA2UIContinuation{}, err
	}
	encoded, _ := json.Marshal(surface.Payload)
	var payload map[string]any
	_ = json.Unmarshal(encoded, &payload)
	payload["interaction_state"] = "answered"
	encoded, _ = json.Marshal(payload)
	resultUpdate, err := tx.ExecContext(ctx, `UPDATE conversation_items SET payload_json = ?, updated_at_ms = ?
WHERE item_id = ? AND kind = 'a2ui_card' AND json_extract(payload_json, '$.interaction_state') = 'pending'
AND CAST(json_extract(payload_json, '$.payload.interaction_revision') AS INTEGER) = ?`,
		string(encoded), millis(now), surface.ID, expectedRevision)
	if err != nil {
		return ConversationA2UIContinuation{}, err
	}
	if changed, _ := resultUpdate.RowsAffected(); changed != 1 {
		return ConversationA2UIContinuation{}, errors.New("A2UI interaction is stale")
	}
	if _, err := tx.ExecContext(ctx, `UPDATE conversation_items SET status = 'completed', updated_at_ms = ?
WHERE item_id = ? AND status = 'running'`, millis(now), call.ID); err != nil {
		return ConversationA2UIContinuation{}, err
	}
	if err := tx.Commit(); err != nil {
		return ConversationA2UIContinuation{}, err
	}
	surface.Payload = payload
	call.Status = "completed"
	return ConversationA2UIContinuation{Surface: surface, Call: call, Action: action, Result: result, Turn: turn}, nil
}

// ClaimConversationA2UI moves one answered action and its turn into resumption.
func (s *Store) ClaimConversationA2UI(ctx context.Context, surfaceItemID string, now time.Time) (ConversationA2UIContinuation, error) {
	return s.changeConversationA2UI(ctx, surfaceItemID, "answered", "resuming", "running", now)
}

// ReleaseConversationA2UI restores one interrupted resumption for restart recovery.
func (s *Store) ReleaseConversationA2UI(ctx context.Context, surfaceItemID string, now time.Time) error {
	_, err := s.changeConversationA2UI(ctx, surfaceItemID, "resuming", "answered", "waiting_for_tool", now)
	return err
}

func (s *Store) changeConversationA2UI(ctx context.Context, id, from, to, turnStatus string, now time.Time) (ConversationA2UIContinuation, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ConversationA2UIContinuation{}, err
	}
	defer func() { _ = tx.Rollback() }()
	surface, err := conversationItemTx(ctx, tx, id)
	if err != nil || surface.Kind != ConversationA2UICard || textJSON(surface.Payload["interaction_state"]) != from {
		return ConversationA2UIContinuation{}, errors.New("A2UI interaction state changed")
	}
	surface.Payload["interaction_state"] = to
	encoded, _ := json.Marshal(surface.Payload)
	if _, err := tx.ExecContext(ctx, `UPDATE conversation_items SET payload_json = ?, updated_at_ms = ? WHERE item_id = ?`, string(encoded), millis(now), id); err != nil {
		return ConversationA2UIContinuation{}, err
	}
	var turn ConversationTurn
	if err := tx.QueryRowContext(ctx, `SELECT turn_id, conversation_id, CAST(json_extract(metadata_json, '$.turn_index') AS INTEGER), status FROM conversation_turns WHERE turn_id = ?`, surface.TurnID).Scan(&turn.ID, &turn.ConversationID, &turn.TurnIndex, &turn.Status); err != nil {
		return ConversationA2UIContinuation{}, err
	}
	expected := "running"
	if from == "answered" {
		expected = "waiting_for_tool"
	}
	if turn.Status != expected {
		return ConversationA2UIContinuation{}, errors.New("A2UI turn state changed")
	}
	if _, err := tx.ExecContext(ctx, `UPDATE conversation_turns SET status = ?, updated_at_ms = ? WHERE turn_id = ?`, turnStatus, millis(now), turn.ID); err != nil {
		return ConversationA2UIContinuation{}, err
	}
	turn.Status = turnStatus
	if to == "resuming" {
		if _, err := tx.ExecContext(ctx, `UPDATE conversations SET agent_status = 'thinking', updated_at_ms = ? WHERE conversation_id = ?`, millis(now), turn.ConversationID); err != nil {
			return ConversationA2UIContinuation{}, err
		}
	}
	action, result, err := a2uiResolutionItemsTx(ctx, tx, surface)
	if err != nil {
		return ConversationA2UIContinuation{}, err
	}
	if err := tx.Commit(); err != nil {
		return ConversationA2UIContinuation{}, err
	}
	return ConversationA2UIContinuation{Surface: surface, Action: action, Result: result, Turn: turn}, nil
}

// RecoverConversationA2UI resets interrupted claims and returns answered actions.
func (s *Store) RecoverConversationA2UI(ctx context.Context, now time.Time) ([]ConversationA2UIContinuation, error) {
	stamp := millis(now)
	if _, err := s.db.ExecContext(ctx, `UPDATE conversation_items SET payload_json = json_set(payload_json, '$.interaction_state', 'answered'), updated_at_ms = ? WHERE kind = 'a2ui_card' AND json_extract(payload_json, '$.interaction_state') = 'resuming'`, stamp); err != nil {
		return nil, err
	}
	if _, err := s.db.ExecContext(ctx, `UPDATE conversation_turns SET status = 'waiting_for_tool', updated_at_ms = ? WHERE turn_id IN (SELECT turn_id FROM conversation_items WHERE kind = 'a2ui_card' AND json_extract(payload_json, '$.interaction_state') = 'answered') AND status = 'running'`, stamp); err != nil {
		return nil, err
	}
	if _, err := s.db.ExecContext(ctx, `UPDATE conversation_items SET payload_json = json_set(payload_json, '$.interaction_state', 'failed'), updated_at_ms = ? WHERE kind = 'a2ui_card' AND json_extract(payload_json, '$.interaction_state') = 'answered' AND turn_id IN (SELECT turn_id FROM conversation_turns WHERE status IN ('failed','cancelled','completed','interrupted'))`, stamp); err != nil {
		return nil, err
	}
	rows, err := s.db.QueryContext(ctx, `SELECT item_id FROM conversation_items WHERE kind = 'a2ui_card' AND json_extract(payload_json, '$.interaction_state') = 'answered' ORDER BY sequence_index`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var ids []string
	for rows.Next() {
		var id string
		if err := rows.Scan(&id); err != nil {
			return nil, err
		}
		ids = append(ids, id)
	}
	result := make([]ConversationA2UIContinuation, 0, len(ids))
	for _, id := range ids {
		item, err := s.a2uiContinuation(ctx, id)
		if err != nil {
			return nil, err
		}
		result = append(result, item)
	}
	return result, rows.Err()
}

func (s *Store) a2uiContinuation(ctx context.Context, id string) (ConversationA2UIContinuation, error) {
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return ConversationA2UIContinuation{}, err
	}
	defer func() { _ = tx.Rollback() }()
	surface, err := conversationItemTx(ctx, tx, id)
	if err != nil {
		return ConversationA2UIContinuation{}, err
	}
	action, result, err := a2uiResolutionItemsTx(ctx, tx, surface)
	if err != nil {
		return ConversationA2UIContinuation{}, err
	}
	var turn ConversationTurn
	if err := tx.QueryRowContext(ctx, `SELECT turn_id, conversation_id, CAST(json_extract(metadata_json, '$.turn_index') AS INTEGER), status FROM conversation_turns WHERE turn_id = ?`, surface.TurnID).Scan(&turn.ID, &turn.ConversationID, &turn.TurnIndex, &turn.Status); err != nil {
		return ConversationA2UIContinuation{}, err
	}
	return ConversationA2UIContinuation{Surface: surface, Action: action, Result: result, Turn: turn}, tx.Commit()
}

func a2uiResolutionItemsTx(ctx context.Context, tx bun.Tx, surface ConversationItem) (ConversationItem, ConversationItem, error) {
	var actionID string
	if err := tx.QueryRowContext(ctx, `SELECT item_id FROM conversation_items WHERE parent_item_id = ? AND kind = 'a2ui_card' ORDER BY sequence_index LIMIT 1`, surface.ID).Scan(&actionID); err != nil {
		return ConversationItem{}, ConversationItem{}, errors.New("A2UI action is unavailable")
	}
	action, err := conversationItemTx(ctx, tx, actionID)
	if err != nil {
		return ConversationItem{}, ConversationItem{}, err
	}
	var resultID string
	if err := tx.QueryRowContext(ctx, `SELECT item_id FROM conversation_items WHERE parent_item_id = ? AND kind = 'tool_result' ORDER BY sequence_index LIMIT 1`, textJSON(surface.Payload["call_item_id"])).Scan(&resultID); err != nil {
		return ConversationItem{}, ConversationItem{}, errors.New("A2UI tool result is unavailable")
	}
	result, err := conversationItemTx(ctx, tx, resultID)
	return action, result, err
}

func completeResumingConversationA2UITx(ctx context.Context, tx bun.Tx, turnID, state string, now time.Time) error {
	if state == "failed" {
		if _, err := tx.ExecContext(ctx, `UPDATE conversation_items SET payload_json = json_set(payload_json, '$.payload.lifecycle', 'failed'), updated_at_ms = ? WHERE turn_id = ? AND kind = 'a2ui_card' AND json_extract(payload_json, '$.payload.lifecycle') = 'answered' AND parent_item_id IN (SELECT item_id FROM conversation_items WHERE turn_id = ? AND kind = 'a2ui_card' AND json_extract(payload_json, '$.interaction_state') = 'resuming')`, millis(now), turnID, turnID); err != nil {
			return fmt.Errorf("fail resumed A2UI projection: %w", err)
		}
	}
	_, err := tx.ExecContext(ctx, `UPDATE conversation_items SET payload_json = json_set(payload_json, '$.interaction_state', ?), updated_at_ms = ? WHERE turn_id = ? AND kind = 'a2ui_card' AND json_extract(payload_json, '$.interaction_state') = 'resuming'`, state, millis(now), turnID)
	if err != nil {
		return fmt.Errorf("complete resumed A2UI interaction: %w", err)
	}
	return nil
}
