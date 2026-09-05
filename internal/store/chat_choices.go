package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/uptrace/bun"
)

// ConversationChoiceContinuation is one answered choice ready for exact resumption.
type ConversationChoiceContinuation struct {
	Prompt    ConversationItem
	Call      ConversationItem
	Selection ConversationItem
	Result    ConversationItem
	Turn      ConversationTurn
}

// ResolveConversationChoice atomically records one human selection and its provider result.
func (s *Store) ResolveConversationChoice(
	ctx context.Context,
	conversationID string,
	promptItemID string,
	selectedOptionIDs []string,
	clientMessageID *string,
	now time.Time,
) (ConversationChoiceContinuation, error) {
	if strings.TrimSpace(conversationID) == "" || strings.TrimSpace(promptItemID) == "" {
		return ConversationChoiceContinuation{}, errors.New("multiple-choice selection is invalid")
	}
	if clientMessageID != nil && (*clientMessageID == "" || len(*clientMessageID) > 256) {
		return ConversationChoiceContinuation{}, errors.New("client message id is invalid")
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ConversationChoiceContinuation{}, fmt.Errorf("begin multiple-choice selection: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	prompt, err := conversationItemTx(ctx, tx, promptItemID)
	if err != nil || prompt.ConversationID != conversationID || prompt.Kind != ConversationMultipleChoicePrompt {
		return ConversationChoiceContinuation{}, errors.New("multiple-choice prompt is unavailable")
	}
	if textJSON(prompt.Payload["lifecycle"]) != "pending" {
		return ConversationChoiceContinuation{}, errors.New("multiple-choice prompt already has a selection")
	}
	mode := textJSON(prompt.Payload["selection_mode"])
	options, err := storedChoiceOptions(prompt.Payload["options"])
	if err != nil {
		return ConversationChoiceContinuation{}, err
	}
	if mode == "pick_one" && len(selectedOptionIDs) != 1 ||
		mode == "pick_many" && len(selectedOptionIDs) == 0 {
		return ConversationChoiceContinuation{}, errors.New("multiple-choice selection has an invalid size")
	}
	requested := make(map[string]struct{}, len(selectedOptionIDs))
	for _, id := range selectedOptionIDs {
		if _, exists := requested[id]; exists || strings.TrimSpace(id) == "" {
			return ConversationChoiceContinuation{}, errors.New("multiple-choice option ids must be unique")
		}
		requested[id] = struct{}{}
	}
	selected := make([]ConversationMultipleChoiceOption, 0, len(requested))
	for _, option := range options {
		if _, exists := requested[option.ID]; exists {
			selected = append(selected, option)
			delete(requested, option.ID)
		}
	}
	if len(requested) != 0 {
		return ConversationChoiceContinuation{}, errors.New("multiple-choice option is not in the prompt")
	}
	var turn ConversationTurn
	if err := tx.QueryRowContext(ctx, `
SELECT turn_id, conversation_id, CAST(json_extract(metadata_json, '$.turn_index') AS INTEGER), status FROM conversation_turns
WHERE turn_id = ? AND conversation_id = ?`, prompt.TurnID, conversationID,
	).Scan(&turn.ID, &turn.ConversationID, &turn.TurnIndex, &turn.Status); err != nil {
		return ConversationChoiceContinuation{}, fmt.Errorf("read multiple-choice turn: %w", err)
	}
	if turn.Status != "waiting_for_tool" {
		return ConversationChoiceContinuation{}, fmt.Errorf("multiple-choice turn is unavailable in state %s", turn.Status)
	}
	callItemID := textJSON(prompt.Payload["call_item_id"])
	call, err := conversationItemTx(ctx, tx, callItemID)
	if err != nil || call.Kind != ConversationToolCall || call.Status != "running" || call.TurnID != turn.ID {
		return ConversationChoiceContinuation{}, errors.New("multiple-choice tool call is unavailable")
	}
	sequence, err := nextConversationSequenceTx(ctx, tx, conversationID)
	if err != nil {
		return ConversationChoiceContinuation{}, err
	}
	labels := make([]string, 0, len(selected))
	for _, option := range selected {
		labels = append(labels, option.Label)
	}
	selectionID := stableConversationOutputID(turn.ID, "multiple_choice_selection", intJSON(prompt.Payload["provider_round"]), providerOutputIndexValue(prompt))
	selection, err := insertConversationOutputTx(ctx, tx, ConversationItem{
		ID: selectionID, ConversationID: conversationID, TurnID: turn.ID,
		ParentItemID: prompt.ID, Sequence: sequence, Kind: ConversationMultipleChoiceSelection,
		Status: "completed", AuthorActorID: "human:local", ContentText: strings.Join(labels, ", "),
		Payload: map[string]any{
			"prompt_item_id": prompt.ID, "selection_mode": mode, "selected_options": choiceOptionsPayload(selected),
		},
		Metadata: map[string]any{"client_message_id": optionalText(clientMessageID)}, CreatedAt: now,
	})
	if err != nil {
		return ConversationChoiceContinuation{}, err
	}
	action, ok := choiceAction(call.Payload)
	if !ok {
		return ConversationChoiceContinuation{}, errors.New("multiple-choice tool call is invalid")
	}
	providerRound := intJSON(prompt.Payload["provider_round"])
	outputIndex := providerOutputIndexValue(prompt)
	resultPayload := map[string]any{"status": "resolved", "selected_options": choiceOptionsPayload(selected)}
	resultID := stableConversationOutputID(turn.ID, "tool_result", providerRound, outputIndex)
	activityID := fmt.Sprintf("tool_result:%s:%d:%d", turn.ID, providerRound, outputIndex)
	result, err := insertConversationOutputTx(ctx, tx, ConversationItem{
		ID: resultID, ConversationID: conversationID, TurnID: turn.ID,
		ParentItemID: call.ID, Sequence: sequence + 1, Kind: ConversationToolResult,
		Status: "completed", AuthorActorID: "agent:primary",
		Payload: map[string]any{
			"id": activityID, "activity_kind": "tool_result", "status": "completed",
			"title": "Tool result: " + textJSON(action["name"]), "summary": textJSON(action["name"]),
			"metadata": map[string]any{
				"turn_index": turn.TurnIndex, "output_index": outputIndex,
				"provider": textJSON(prompt.Metadata["provider"]), "display": map[string]any{},
				"action": map[string]any{
					"call_id":          fmt.Sprintf("tool_call:%s:%d:%d", turn.ID, providerRound, outputIndex),
					"provider_call_id": action["provider_call_id"], "provider_name": action["provider_name"],
					"name": action["name"], "success": true, "payload": resultPayload,
				},
			},
		},
		Metadata: map[string]any{
			"turn_index": turn.TurnIndex, "output_index": outputIndex, "provider_round": providerRound,
			"source": "provider_action_result", "provider": textJSON(prompt.Metadata["provider"]),
		}, CreatedAt: now,
	})
	if err != nil {
		return ConversationChoiceContinuation{}, err
	}
	prompt.Payload["lifecycle"] = "answered"
	prompt.Payload["interaction_revision"] = 2
	encodedPrompt, _ := json.Marshal(prompt.Payload)
	if _, err := tx.ExecContext(ctx, `UPDATE conversation_items SET payload_json = ?, updated_at_ms = ? WHERE item_id = ?`,
		string(encodedPrompt), millis(now), prompt.ID); err != nil {
		return ConversationChoiceContinuation{}, fmt.Errorf("answer multiple-choice prompt: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `UPDATE conversation_items SET status = 'completed', updated_at_ms = ? WHERE item_id = ?`,
		millis(now), call.ID); err != nil {
		return ConversationChoiceContinuation{}, fmt.Errorf("finish multiple-choice tool call: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return ConversationChoiceContinuation{}, fmt.Errorf("commit multiple-choice selection: %w", err)
	}
	prompt.Payload["lifecycle"] = "answered"
	prompt.Payload["interaction_revision"] = 2
	call.Status = "completed"
	return ConversationChoiceContinuation{Prompt: prompt, Call: call, Selection: selection, Result: result, Turn: turn}, nil
}

// ClaimConversationChoice moves one answered choice and its original turn into execution.
func (s *Store) ClaimConversationChoice(ctx context.Context, promptItemID string, now time.Time) (ConversationChoiceContinuation, error) {
	return s.changeConversationChoice(ctx, promptItemID, "answered", "resuming", "running", now)
}

// ReleaseConversationChoice restores one interrupted claim for restart recovery.
func (s *Store) ReleaseConversationChoice(ctx context.Context, promptItemID string, now time.Time) error {
	_, err := s.changeConversationChoice(ctx, promptItemID, "resuming", "answered", "waiting_for_tool", now)
	return err
}

func (s *Store) changeConversationChoice(
	ctx context.Context, promptItemID, from, to, turnStatus string, now time.Time,
) (ConversationChoiceContinuation, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ConversationChoiceContinuation{}, err
	}
	defer func() { _ = tx.Rollback() }()
	prompt, err := conversationItemTx(ctx, tx, promptItemID)
	if err != nil || prompt.Kind != ConversationMultipleChoicePrompt || textJSON(prompt.Payload["lifecycle"]) != from {
		return ConversationChoiceContinuation{}, errors.New("multiple-choice prompt state changed")
	}
	prompt.Payload["lifecycle"] = to
	encoded, _ := json.Marshal(prompt.Payload)
	if _, err := tx.ExecContext(ctx, `UPDATE conversation_items SET payload_json = ?, updated_at_ms = ? WHERE item_id = ?`,
		string(encoded), millis(now), prompt.ID); err != nil {
		return ConversationChoiceContinuation{}, err
	}
	var turn ConversationTurn
	if err := tx.QueryRowContext(ctx, `SELECT turn_id, conversation_id, CAST(json_extract(metadata_json, '$.turn_index') AS INTEGER), status FROM conversation_turns WHERE turn_id = ?`, prompt.TurnID).Scan(&turn.ID, &turn.ConversationID, &turn.TurnIndex, &turn.Status); err != nil {
		return ConversationChoiceContinuation{}, err
	}
	expectedTurnStatus := "running"
	if from == "answered" {
		expectedTurnStatus = "waiting_for_tool"
	}
	if turn.Status != expectedTurnStatus {
		return ConversationChoiceContinuation{}, errors.New("multiple-choice turn state changed")
	}
	if turnStatus != "" {
		if _, err := tx.ExecContext(ctx, `UPDATE conversation_turns SET status = ?, updated_at_ms = ? WHERE turn_id = ?`,
			turnStatus, millis(now), turn.ID); err != nil {
			return ConversationChoiceContinuation{}, err
		}
		turn.Status = turnStatus
	}
	if to == "resuming" {
		if _, err := tx.ExecContext(ctx, `UPDATE conversations SET agent_status = 'thinking', updated_at_ms = ? WHERE conversation_id = ?`,
			millis(now), turn.ConversationID); err != nil {
			return ConversationChoiceContinuation{}, err
		}
	}
	selection, result, err := choiceResolutionItemsTx(ctx, tx, prompt)
	if err != nil {
		return ConversationChoiceContinuation{}, err
	}
	if err := tx.Commit(); err != nil {
		return ConversationChoiceContinuation{}, err
	}
	prompt.Payload["lifecycle"] = to
	return ConversationChoiceContinuation{Prompt: prompt, Selection: selection, Result: result, Turn: turn}, nil
}

// RecoverConversationChoices resets interrupted claims and returns answered choices.
func (s *Store) RecoverConversationChoices(ctx context.Context, now time.Time) ([]ConversationChoiceContinuation, error) {
	if _, err := s.db.ExecContext(ctx, `
UPDATE conversation_items SET payload_json = json_set(payload_json, '$.lifecycle', 'answered'), updated_at_ms = ?
WHERE kind = 'multiple_choice_prompt' AND json_extract(payload_json, '$.lifecycle') = 'resuming'`, millis(now)); err != nil {
		return nil, fmt.Errorf("reset multiple-choice claims: %w", err)
	}
	if _, err := s.db.ExecContext(ctx, `
UPDATE conversation_turns SET status = 'waiting_for_tool', updated_at_ms = ?
WHERE turn_id IN (SELECT turn_id FROM conversation_items WHERE kind = 'multiple_choice_prompt'
AND json_extract(payload_json, '$.lifecycle') = 'answered') AND status = 'running'`, millis(now)); err != nil {
		return nil, fmt.Errorf("reset multiple-choice turns: %w", err)
	}
	if _, err := s.db.ExecContext(ctx, `
UPDATE conversation_items SET payload_json = json_set(payload_json, '$.lifecycle', 'failed'), updated_at_ms = ?
WHERE kind = 'multiple_choice_prompt' AND json_extract(payload_json, '$.lifecycle') = 'answered'
AND turn_id IN (SELECT turn_id FROM conversation_turns WHERE status IN ('failed','cancelled','completed','interrupted'))`,
		millis(now)); err != nil {
		return nil, fmt.Errorf("reconcile multiple-choice turns: %w", err)
	}
	rows, err := s.db.QueryContext(ctx, `SELECT item_id FROM conversation_items
WHERE kind = 'multiple_choice_prompt' AND json_extract(payload_json, '$.lifecycle') = 'answered'
ORDER BY sequence_index`)
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
	result := make([]ConversationChoiceContinuation, 0, len(ids))
	for _, id := range ids {
		choice, err := s.choiceContinuation(ctx, id)
		if err != nil {
			return nil, err
		}
		result = append(result, choice)
	}
	return result, rows.Err()
}

func (s *Store) choiceContinuation(ctx context.Context, id string) (ConversationChoiceContinuation, error) {
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return ConversationChoiceContinuation{}, err
	}
	defer func() { _ = tx.Rollback() }()
	prompt, err := conversationItemTx(ctx, tx, id)
	if err != nil {
		return ConversationChoiceContinuation{}, err
	}
	selection, result, err := choiceResolutionItemsTx(ctx, tx, prompt)
	if err != nil {
		return ConversationChoiceContinuation{}, err
	}
	var turn ConversationTurn
	if err := tx.QueryRowContext(ctx, `SELECT turn_id, conversation_id, CAST(json_extract(metadata_json, '$.turn_index') AS INTEGER), status FROM conversation_turns WHERE turn_id = ?`, prompt.TurnID).Scan(&turn.ID, &turn.ConversationID, &turn.TurnIndex, &turn.Status); err != nil {
		return ConversationChoiceContinuation{}, err
	}
	return ConversationChoiceContinuation{Prompt: prompt, Selection: selection, Result: result, Turn: turn}, tx.Commit()
}

func choiceResolutionItemsTx(ctx context.Context, tx bun.Tx, prompt ConversationItem) (ConversationItem, ConversationItem, error) {
	var selectionID string
	if err := tx.QueryRowContext(ctx, `SELECT item_id FROM conversation_items
WHERE parent_item_id = ? AND kind = 'multiple_choice_selection' ORDER BY sequence_index LIMIT 1`, prompt.ID).Scan(&selectionID); err != nil {
		return ConversationItem{}, ConversationItem{}, errors.New("multiple-choice selection is unavailable")
	}
	selection, err := conversationItemTx(ctx, tx, selectionID)
	if err != nil {
		return ConversationItem{}, ConversationItem{}, err
	}
	var resultID string
	if err := tx.QueryRowContext(ctx, `SELECT item_id FROM conversation_items
WHERE parent_item_id = ? AND kind = 'tool_result' ORDER BY sequence_index LIMIT 1`, textJSON(prompt.Payload["call_item_id"])).Scan(&resultID); err != nil {
		return ConversationItem{}, ConversationItem{}, errors.New("multiple-choice tool result is unavailable")
	}
	result, err := conversationItemTx(ctx, tx, resultID)
	return selection, result, err
}

func storedChoiceOptions(value any) ([]ConversationMultipleChoiceOption, error) {
	values, ok := value.([]any)
	if !ok || len(values) == 0 {
		return nil, errors.New("stored multiple-choice options are invalid")
	}
	result := make([]ConversationMultipleChoiceOption, 0, len(values))
	for _, value := range values {
		option, ok := value.(map[string]any)
		if !ok || strings.TrimSpace(textJSON(option["id"])) == "" || strings.TrimSpace(textJSON(option["label"])) == "" {
			return nil, errors.New("stored multiple-choice option is invalid")
		}
		result = append(result, ConversationMultipleChoiceOption{ID: textJSON(option["id"]), Label: textJSON(option["label"])})
	}
	return result, nil
}

func choiceOptionsPayload(options []ConversationMultipleChoiceOption) []map[string]any {
	result := make([]map[string]any, 0, len(options))
	for _, option := range options {
		result = append(result, map[string]any{"id": option.ID, "label": option.Label})
	}
	return result
}

func choiceAction(payload map[string]any) (map[string]any, bool) {
	metadata, ok := payload["metadata"].(map[string]any)
	if !ok {
		return nil, false
	}
	action, ok := metadata["action"].(map[string]any)
	return action, ok
}

func textJSON(value any) string { valueText, _ := value.(string); return valueText }
func intJSON(value any) int     { valueNumber, _ := value.(float64); return int(valueNumber) }
func providerOutputIndexValue(item ConversationItem) int {
	return intJSON(item.Metadata["output_index"])
}
func optionalText(value *string) any {
	if value == nil {
		return nil
	}
	return *value
}

func completeResumingConversationChoicesTx(ctx context.Context, tx bun.Tx, turnID string, now time.Time) error {
	if _, err := tx.ExecContext(ctx, `
UPDATE conversation_items
SET payload_json = json_set(payload_json, '$.lifecycle', 'completed'), updated_at_ms = ?
WHERE turn_id = ? AND kind = 'multiple_choice_prompt'
  AND json_extract(payload_json, '$.lifecycle') = 'resuming'`, millis(now), turnID); err != nil {
		return fmt.Errorf("complete resumed multiple-choice prompt: %w", err)
	}
	return nil
}
