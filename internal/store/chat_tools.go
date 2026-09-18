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
	"unicode/utf8"

	"github.com/uptrace/bun"

	"github.com/kpsuperplane/noema/internal/toolmarker"
)

// ConversationToolCallInput is one validated provider call for durable storage.
type ConversationToolCallInput struct {
	ProviderRound  int
	OutputIndex    int
	ProviderItemID string
	ProviderCallID string
	ProviderName   string
	Name           string
	Arguments      json.RawMessage
}

// ConversationToolRound is one provider response that requests an immediate tool.
type ConversationToolRound struct {
	Provider                  string
	Commentary                string
	ProviderCommentary        string
	Citations                 []ProviderCitation
	UnresolvedCitationMarkers int
	Reasoning                 []json.RawMessage
	Call                      ConversationToolCallInput
	MultipleChoice            *ConversationMultipleChoiceInput
	A2UI                      *ConversationA2UIInput
}

// ConversationA2UIInput contains one validated A2UI projection and its resume authority.
type ConversationA2UIInput struct {
	Projection         map[string]any
	HasActions         bool
	ProviderSelection  map[string]any
	ResponseID         string
	HostedState        bool
	CredentialRevision uint64
	ToolCatalogDigest  string
}

// ConversationMultipleChoiceOption is one ordered choice exposed to the human.
type ConversationMultipleChoiceOption struct {
	ID    string `json:"id"`
	Label string `json:"label"`
}

// ConversationMultipleChoiceInput contains display options for one question.
type ConversationMultipleChoiceInput struct {
	Prompt        string
	SelectionMode string
	Options       []ConversationMultipleChoiceOption
}

// ConversationToolResultInput is one terminal result for a stored provider call.
type ConversationToolResultInput struct {
	CallItemID     string
	Provider       string
	ProviderRound  int
	OutputIndex    int
	ProviderCallID string
	ProviderName   string
	Name           string
	Success        bool
	Payload        json.RawMessage
}

// ConversationContext is the latest summary checkpoint and subsequent durable provider items.
type ConversationContext struct {
	Summary         string
	RecentJSON      string
	ThroughSequence int64
	Items           []ConversationItem
}

// ConversationProviderItems returns complete durable provider context in order.
func (s *Store) ConversationProviderItems(
	ctx context.Context,
	conversationID string,
) ([]ConversationItem, error) {
	if _, err := s.Conversation(ctx, conversationID); err != nil {
		return nil, err
	}
	return s.conversationProviderItemsAfter(ctx, conversationID, 0)
}

// ConversationProviderContext returns one matching checkpoint and items after its coverage.
func (s *Store) ConversationProviderContext(
	ctx context.Context, conversationID, providerKind, modelProfile string,
) (ConversationContext, error) {
	if _, err := s.Conversation(ctx, conversationID); err != nil {
		return ConversationContext{}, err
	}
	var resetSequence int64
	if err := s.db.QueryRowContext(ctx, `SELECT COALESCE(MAX(sequence_index), 0)
FROM conversation_items
WHERE conversation_id = ? AND deleted_at_ms IS NULL AND kind = 'activity'
  AND status = 'completed'
  AND json_extract(payload_json, '$.activity_kind') = 'context_reset'`, conversationID).Scan(&resetSequence); err != nil {
		return ConversationContext{}, fmt.Errorf("query conversation context reset: %w", err)
	}
	var value ConversationContext
	err := s.db.QueryRowContext(ctx, `SELECT
COALESCE(json_extract(payload_json,'$.summary'),''),
COALESCE(json_extract(payload_json,'$.recent_messages'),'[]'),
COALESCE(json_extract(payload_json,'$.through_sequence'),0)
FROM conversation_items WHERE conversation_id=? AND kind='model_context_update'
AND sequence_index > ?
AND json_extract(payload_json,'$.provider_kind')=? AND json_extract(payload_json,'$.model_profile')=?
ORDER BY sequence_index DESC LIMIT 1`, conversationID, resetSequence, providerKind, modelProfile).
		Scan(&value.Summary, &value.RecentJSON, &value.ThroughSequence)
	if err != nil && !errors.Is(err, sql.ErrNoRows) {
		return ConversationContext{}, err
	}
	if value.ThroughSequence < resetSequence {
		value.ThroughSequence = resetSequence
	}
	items, err := s.conversationProviderItemsAfter(ctx, conversationID, value.ThroughSequence)
	value.Items = items
	return value, err
}

func (s *Store) conversationProviderItemsAfter(
	ctx context.Context, conversationID string, after int64,
) ([]ConversationItem, error) {
	rows, err := s.db.QueryContext(ctx, `
SELECT item_id, conversation_id, COALESCE(turn_id, ''), COALESCE(parent_item_id, ''),
       sequence_index, kind, status, author_actor_id, COALESCE(content_text, ''),
       COALESCE(provider_content_text, ''), payload_json, metadata_json, created_at_ms
FROM conversation_items
WHERE conversation_id = ? AND sequence_index > ? AND deleted_at_ms IS NULL AND (
    (kind IN ('user_text', 'multiple_choice_selection', 'assistant_text', 'reasoning') AND status = 'completed')
    OR (kind = 'model_context_update' AND status = 'completed'
        AND json_type(payload_json, '$.model_context_update') = 'object')
    OR (kind = 'task_reference' AND status = 'completed')
    OR (kind IN ('tool_call', 'tool_result')
        AND status IN ('completed', 'failed', 'cancelled', 'interrupted'))
)
ORDER BY sequence_index`, conversationID, after)
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

// AppendConversationContextUpdate saves one hidden summary under the active turn.
func (s *Store) AppendConversationContextUpdate(
	ctx context.Context, turn ConversationTurn, providerKind, modelProfile, summary string,
	recent any, throughSequence int64, now time.Time,
) error {
	if strings.TrimSpace(providerKind) == "" ||
		strings.TrimSpace(summary) == "" || len(summary) > maxConversationText || throughSequence < 1 {
		return errors.New("conversation context update is invalid")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	var active int
	if err := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM conversation_turns
WHERE turn_id=? AND conversation_id=? AND status IN ('input_received','running')`, turn.ID, turn.ConversationID).Scan(&active); err != nil {
		return err
	}
	if active != 1 {
		return errors.New("conversation turn is already final")
	}
	parentID, err := conversationTurnParentTx(ctx, tx, turn)
	if err != nil {
		return err
	}
	sequence, err := nextConversationSequenceTx(ctx, tx, turn.ConversationID)
	if err != nil {
		return err
	}
	id, err := newID("item")
	if err != nil {
		return err
	}
	_, err = insertConversationOutputTx(ctx, tx, ConversationItem{ID: id, ConversationID: turn.ConversationID,
		TurnID: turn.ID, ParentItemID: parentID, Sequence: sequence, Kind: ConversationModelContextUpdate, Status: "completed",
		AuthorActorID: "agent:primary", Payload: map[string]any{"provider_kind": providerKind,
			"model_profile": modelProfile, "summary": summary, "recent_messages": recent,
			"through_sequence": throughSequence},
		Metadata: map[string]any{"source": "context_compaction"}, CreatedAt: now.UTC()})
	if err != nil {
		return err
	}
	return tx.Commit()
}

// AppendConversationContextReset records the exact context reset command
// without creating a provider turn. The item is the durable prompt boundary.
func (s *Store) AppendConversationContextReset(
	ctx context.Context, conversationID string, clientMessageID *string, now time.Time,
) (ConversationItem, error) {
	if clientMessageID != nil && (len(*clientMessageID) == 0 || len(*clientMessageID) > 256) {
		return ConversationItem{}, errors.New("client message id is invalid")
	}
	activityID := "context_reset:" + conversationID
	if clientMessageID != nil {
		activityID = "context_reset:" + *clientMessageID
	}
	metadata := map[string]any{
		"source":            "context_reset_command",
		"client_message_id": clientMessageID,
		"presentation":      map[string]any{"tone": "neutral"},
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ConversationItem{}, err
	}
	defer func() { _ = tx.Rollback() }()
	if err := requireConversationTx(ctx, tx, conversationID); err != nil {
		return ConversationItem{}, err
	}
	sequence, err := nextConversationSequenceTx(ctx, tx, conversationID)
	if err != nil {
		return ConversationItem{}, err
	}
	itemID, err := newID("item")
	if err != nil {
		return ConversationItem{}, err
	}
	item, err := insertConversationOutputTx(ctx, tx, ConversationItem{
		ID: itemID, ConversationID: conversationID, Sequence: sequence,
		Kind: ConversationActivity, Status: "completed", AuthorActorID: "system:context-runtime",
		ContentText: "Context reset",
		Payload: map[string]any{
			"id": activityID, "activity_kind": "context_reset", "status": "completed",
			"title": "Context reset", "summary": nil, "metadata": metadata,
		},
		Metadata: metadata, CreatedAt: now,
	})
	if err != nil {
		return ConversationItem{}, err
	}
	if err := tx.Commit(); err != nil {
		return ConversationItem{}, err
	}
	return item, nil
}

// AppendConversationActivity saves one readable runtime activity under an active turn.
func (s *Store) AppendConversationActivity(
	ctx context.Context,
	turn ConversationTurn,
	providerRound int,
	activityKind, title, summary, status string,
	details map[string]any,
	now time.Time,
) (ConversationItem, error) {
	if providerRound < 0 || activityKind == "" || len(activityKind) > 64 ||
		strings.TrimSpace(title) == "" || utf8.RuneCountInString(title) > 1000 || !utf8.ValidString(title) ||
		utf8.RuneCountInString(summary) > 4000 || !utf8.ValidString(summary) ||
		(status != "running" && status != "completed" && status != "failed") {
		return ConversationItem{}, errors.New("conversation activity is invalid")
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ConversationItem{}, err
	}
	defer func() { _ = tx.Rollback() }()
	var active int
	if err := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM conversation_turns
WHERE turn_id = ? AND conversation_id = ? AND status IN ('input_received','running')`,
		turn.ID, turn.ConversationID).Scan(&active); err != nil {
		return ConversationItem{}, err
	}
	if active != 1 {
		return ConversationItem{}, errors.New("conversation turn is already final")
	}
	parentID, err := conversationTurnParentTx(ctx, tx, turn)
	if err != nil {
		return ConversationItem{}, err
	}
	sequence, err := nextConversationSequenceTx(ctx, tx, turn.ConversationID)
	if err != nil {
		return ConversationItem{}, err
	}
	itemID, err := newID("item")
	if err != nil {
		return ConversationItem{}, err
	}
	if details == nil {
		details = map[string]any{}
	}
	item, err := insertConversationOutputTx(ctx, tx, ConversationItem{
		ID: itemID, ConversationID: turn.ConversationID, TurnID: turn.ID,
		ParentItemID: parentID, Sequence: sequence, Kind: ConversationActivity,
		Status: status, AuthorActorID: "agent:primary",
		Payload: map[string]any{
			"id":            "activity:" + strings.TrimPrefix(itemID, "item:"),
			"activity_kind": activityKind, "title": title,
			"summary": summary, "metadata": details,
		},
		Metadata: map[string]any{
			"turn_index": turn.TurnIndex, "provider_round": providerRound,
			"source": activityKind,
		},
		CreatedAt: now,
	})
	if err != nil {
		return ConversationItem{}, err
	}
	if err := tx.Commit(); err != nil {
		return ConversationItem{}, err
	}
	return item, nil
}

// StartConversationToolRound stores provider output before one immediate read.
func (s *Store) StartConversationToolRound(
	ctx context.Context,
	turn ConversationTurn,
	round ConversationToolRound,
	now time.Time,
) ([]ConversationItem, error) {
	if strings.TrimSpace(round.Provider) == "" || strings.TrimSpace(round.Provider) != round.Provider ||
		round.Call.ProviderRound < 0 || round.Call.OutputIndex < 0 ||
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
	if round.UnresolvedCitationMarkers < 0 {
		return nil, errors.New("conversation citation diagnostic is invalid")
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, fmt.Errorf("begin conversation tool round: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	expectedStatus := "running"
	if round.Call.ProviderRound == 0 && turn.Status == "input_received" {
		expectedStatus = "input_received"
	}
	if err := requireActiveTurnTx(ctx, tx, turn, expectedStatus); err != nil {
		return nil, err
	}
	var activeItems int
	if err := tx.QueryRowContext(ctx, `
SELECT COUNT(*) FROM conversation_items
WHERE turn_id = ? AND status IN ('pending', 'running')`, turn.ID).Scan(&activeItems); err != nil {
		return nil, fmt.Errorf("inspect active conversation items: %w", err)
	}
	if activeItems != 0 {
		return nil, errors.New("conversation tool call is already running")
	}
	parentID, err := conversationTurnParentTx(ctx, tx, turn)
	if err != nil {
		return nil, err
	}
	sequence, err := nextConversationSequenceTx(ctx, tx, turn.ConversationID)
	if err != nil {
		return nil, err
	}
	items := make([]ConversationItem, 0, 4)
	metadata := func(kind string, output int) map[string]any {
		return map[string]any{
			"turn_index": turn.TurnIndex, "output_index": output,
			"provider_round": round.Call.ProviderRound, "source": kind,
			"provider": round.Provider,
		}
	}
	if len(round.Reasoning) != 0 {
		item, err := insertConversationOutputTx(ctx, tx, ConversationItem{
			ID:             ConversationOutputID(turn.ID, "reasoning", round.Call.ProviderRound, 0),
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
	var streamedCommentary bool
	if err := tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM conversation_items WHERE turn_id = ? AND json_extract(metadata_json, '$.provider_round') = ? AND json_extract(metadata_json, '$.provider_output_kind') = 'message')`, turn.ID, round.Call.ProviderRound).Scan(&streamedCommentary); err != nil {
		return nil, err
	}
	if !streamedCommentary && strings.TrimSpace(round.Commentary) != "" {
		commentaryMetadata := metadata("provider_commentary", 0)
		commentaryMetadata["phase"] = "commentary"
		commentaryMetadata["stream_id"] = ConversationAssistantStreamID(
			turn.ID, round.Call.ProviderRound,
		)
		commentaryMetadata["response_index"] = 0
		addProviderCitationMetadata(commentaryMetadata, round.Citations, round.UnresolvedCitationMarkers)
		item, err := insertConversationOutputTx(ctx, tx, ConversationItem{
			ID:             ConversationOutputID(turn.ID, "assistant_text", round.Call.ProviderRound, 0),
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
	callID := ConversationOutputID(
		turn.ID, "tool_call", round.Call.ProviderRound, round.Call.OutputIndex,
	)
	item, err := insertConversationOutputTx(ctx, tx, ConversationItem{
		ID: callID, ConversationID: turn.ConversationID, TurnID: turn.ID,
		ParentItemID: parentID, Sequence: sequence, Kind: ConversationToolCall,
		Status: "running", AuthorActorID: "agent:primary",
		Payload: map[string]any{
			"id": callID, "activity_kind": "tool_call", "status": "started",
			"title": "Tool call: " + round.Call.Name, "summary": round.Call.Name,
			"metadata": map[string]any{
				"turn_index": turn.TurnIndex, "output_index": round.Call.OutputIndex,
				"provider": round.Provider, "display": map[string]any{},
				"action": map[string]any{
					"id": callID, "provider_item_id": round.Call.ProviderItemID,
					"provider_call_id": round.Call.ProviderCallID,
					"provider_name":    round.Call.ProviderName, "name": round.Call.Name,
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
	turnStatus := "running"
	if choice := round.MultipleChoice; choice != nil {
		if strings.TrimSpace(choice.Prompt) == "" ||
			(choice.SelectionMode != "pick_one" && choice.SelectionMode != "pick_many") ||
			len(choice.Options) == 0 {
			return nil, errors.New("conversation multiple-choice prompt is invalid")
		}
		promptID := ConversationOutputID(
			turn.ID, "multiple_choice_prompt", round.Call.ProviderRound, round.Call.OutputIndex,
		)
		prompt, err := insertConversationOutputTx(ctx, tx, ConversationItem{
			ID: promptID, ConversationID: turn.ConversationID, TurnID: turn.ID,
			ParentItemID: callID, Sequence: sequence + 1, Kind: ConversationMultipleChoicePrompt,
			Status: "completed", AuthorActorID: "agent:primary",
			Payload: map[string]any{
				"prompt": choice.Prompt, "selection_mode": choice.SelectionMode,
				"options": choiceOptionsPayload(choice.Options),
			},
			Metadata: metadata("multiple_choice_prompt", round.Call.OutputIndex), CreatedAt: now,
		})
		if err != nil {
			return nil, err
		}
		items = append(items, prompt)
	}
	if surface := round.A2UI; surface != nil && len(surface.Projection) != 0 {
		if surface.HasActions && (surface.ProviderSelection == nil || surface.ToolCatalogDigest == "") {
			return nil, errors.New("conversation A2UI authority is invalid")
		}
		projection := cloneJSONMap(surface.Projection)
		interactionID := fmt.Sprintf("interaction:%s:%s:%s", turn.ConversationID, turn.ID, round.Call.ProviderCallID)
		if surface.HasActions {
			projection["interaction_id"] = interactionID
			projection["interaction_revision"] = 1
			projection["lifecycle"] = "pending"
		}
		cardID := ConversationOutputID(turn.ID, "a2ui_card", round.Call.ProviderRound, round.Call.OutputIndex)
		card, err := insertConversationOutputTx(ctx, tx, ConversationItem{
			ID: cardID, ConversationID: turn.ConversationID, TurnID: turn.ID,
			ParentItemID: callID, Sequence: sequence + 1, Kind: ConversationA2UICard,
			Status: "completed", AuthorActorID: "agent:primary",
			Payload: map[string]any{
				"id":     fmt.Sprintf("a2ui:%s:%d", turn.ID, round.Call.OutputIndex),
				"schema": "a2ui.v0.9.1", "payload": projection,
				"interaction_state": func() string {
					if surface.HasActions {
						return "pending"
					}
					return "completed"
				}(),
				"provider_selection": surface.ProviderSelection, "provider_round": round.Call.ProviderRound,
				"response_id": surface.ResponseID, "hosted_state": surface.HostedState,
				"credential_revision": surface.CredentialRevision, "tool_catalog_digest": surface.ToolCatalogDigest,
				"call_item_id": callID,
			},
			Metadata: metadata("a2ui_card", round.Call.OutputIndex), CreatedAt: now,
		})
		if err != nil {
			return nil, err
		}
		items = append(items, card)
		if surface.HasActions {
			turnStatus = "waiting_for_tool"
		}
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE conversation_turns SET status = ?, updated_at_ms = ?
WHERE turn_id = ? AND conversation_id = ?`, turnStatus, millis(now), turn.ID, turn.ConversationID); err != nil {
		return nil, fmt.Errorf("start conversation tool turn: %w", err)
	}
	if round.MultipleChoice != nil || round.A2UI != nil && round.A2UI.HasActions {
		if _, err := tx.ExecContext(ctx, `
UPDATE conversations SET agent_status = 'idle', updated_at_ms = ? WHERE conversation_id = ?`,
			millis(now), turn.ConversationID); err != nil {
			return nil, fmt.Errorf("pause conversation for A2UI: %w", err)
		}
	}
	if err := completeResumingConversationA2UITx(ctx, tx, turn.ID, "completed", now); err != nil {
		return nil, err
	}
	if err := tx.Commit(); err != nil {
		return nil, fmt.Errorf("commit conversation tool round: %w", err)
	}
	for index := range items {
		decorateConversationToolMarker(&items[index], nil)
	}
	return items, nil
}

func cloneJSONMap(value map[string]any) map[string]any {
	encoded, _ := json.Marshal(value)
	var cloned map[string]any
	_ = json.Unmarshal(encoded, &cloned)
	return cloned
}

// FinishConversationToolCall atomically closes one call and stores its result.
func (s *Store) FinishConversationToolCall(
	ctx context.Context,
	turn ConversationTurn,
	result ConversationToolResultInput,
	now time.Time,
) (ConversationItem, error) {
	if result.CallItemID == "" || strings.TrimSpace(result.Provider) == "" ||
		strings.TrimSpace(result.Provider) != result.Provider || result.ProviderRound < 0 || result.OutputIndex < 0 ||
		strings.TrimSpace(result.ProviderCallID) == "" || strings.TrimSpace(result.ProviderName) == "" ||
		strings.TrimSpace(result.Name) == "" {
		return ConversationItem{}, errors.New("conversation tool result is invalid")
	}
	expectedCallID := ConversationOutputID(
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
	var resultMarker map[string]any
	if marker, ok := toolmarker.Build(result.Name, status, true, nil, payloadValue); ok {
		resultMarker = marker
	}
	if resultMarker == nil && result.Name == "web.browse.open" {
		var callPayloadJSON string
		if queryErr := s.db.QueryRowContext(ctx, `
SELECT payload_json FROM conversation_items
WHERE item_id = ? AND conversation_id = ?`, result.CallItemID, turn.ConversationID).Scan(&callPayloadJSON); queryErr == nil {
			var callPayload map[string]any
			if json.Unmarshal([]byte(callPayloadJSON), &callPayload) == nil {
				if callMetadata, ok := callPayload["metadata"].(map[string]any); ok {
					if callAction, ok := callMetadata["action"].(map[string]any); ok {
						if marker, ok := toolmarker.ForAction("tool_call", status, callAction); ok {
							resultMarker = marker
						}
					}
				}
			}
		}
	}
	now = now.UTC()
	resultID := ConversationOutputID(turn.ID, "tool_result", result.ProviderRound, result.OutputIndex)
	payload := map[string]any{
		"id": resultID, "activity_kind": "tool_result", "status": status,
		"title": "Tool result: " + result.Name, "summary": result.Name,
		"metadata": map[string]any{
			"turn_index": turn.TurnIndex, "output_index": result.OutputIndex,
			"provider": result.Provider, "display": map[string]any{},
			"action": map[string]any{
				"call_id": result.CallItemID, "provider_call_id": result.ProviderCallID,
				"provider_name": result.ProviderName, "name": result.Name,
				"success": result.Success, "payload": payloadValue,
			},
		},
	}
	metadata := map[string]any{
		"turn_index": turn.TurnIndex, "output_index": result.OutputIndex,
		"provider_round": result.ProviderRound, "source": "provider_action_result",
		"provider": result.Provider,
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
		decorateConversationToolMarker(&item, resultMarker)
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
	if callMetadata["provider"] != result.Provider ||
		callAction["provider_call_id"] != result.ProviderCallID ||
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
	decorateConversationToolMarker(&item, resultMarker)
	return item, nil
}

func requireActiveTurnTx(ctx context.Context, tx bun.Tx, turn ConversationTurn, status string) error {
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

func nextConversationSequenceTx(ctx context.Context, tx bun.Tx, conversationID string) (int64, error) {
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
	tx bun.Tx,
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
		item.ID, item.ConversationID, nullableText(item.TurnID), nullableText(item.ParentItemID), item.Sequence,
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
