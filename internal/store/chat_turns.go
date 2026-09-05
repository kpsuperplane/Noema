package store

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"
)

const (
	conversationCursorPrefix = "conversation_item:"
	maxConversationText      = 256 * 1024
)

// ErrConversationTurnActive means the conversation already has foreground work.
var ErrConversationTurnActive = errors.New("conversation turn is already active")

// ConversationItemKind identifies one durable transcript item.
type ConversationItemKind string

const (
	ConversationUserText                ConversationItemKind = "user_text"
	ConversationAssistantText           ConversationItemKind = "assistant_text"
	ConversationToolCall                ConversationItemKind = "tool_call"
	ConversationToolResult              ConversationItemKind = "tool_result"
	ConversationActivity                ConversationItemKind = "activity"
	ConversationReasoning               ConversationItemKind = "reasoning"
	ConversationMultipleChoicePrompt    ConversationItemKind = "multiple_choice_prompt"
	ConversationMultipleChoiceSelection ConversationItemKind = "multiple_choice_selection"
	ConversationApprovalRequest         ConversationItemKind = "approval_request"
	ConversationErrorNotice             ConversationItemKind = "error_notice"
)

// ConversationItem is one durable visible transcript record.
type ConversationItem struct {
	ID                  string
	ConversationID      string
	TurnID              string
	ParentItemID        string
	Sequence            int64
	Cursor              string
	Kind                ConversationItemKind
	Status              string
	AuthorActorID       string
	ContentText         string
	ProviderContentText string
	Payload             map[string]any
	Metadata            map[string]any
	CreatedAt           time.Time
}

// ConversationItemPage is one newest-first query returned in display order.
type ConversationItemPage struct {
	Items         []ConversationItem
	BeforeCursor  string
	HasMoreBefore bool
}

// ConversationTurn is one durable causal Chat turn.
type ConversationTurn struct {
	ID             string
	ConversationID string
	TurnIndex      int64
	Status         string
}

// ProviderMessage is one stored text item supplied to a model provider.
type ProviderMessage struct {
	Role    string
	Content string
}

// ProviderUsage records safe usage metadata for one assistant response.
type ProviderUsage struct {
	Provider          string
	Model             string
	InputTokens       int
	OutputTokens      int
	TotalTokens       int
	CachedInputTokens int
	WebSearchRequests int
}

// ProviderCitation is one safe public citation attached to assistant text.
type ProviderCitation struct {
	Title      string `json:"title"`
	URL        string `json:"url"`
	StartIndex *int   `json:"start_index,omitempty"`
	EndIndex   *int   `json:"end_index,omitempty"`
}

// BeginConversationTurn atomically creates one turn and its completed user item.
func (s *Store) BeginConversationTurn(
	ctx context.Context,
	conversationID string,
	input string,
	clientMessageID *string,
	now time.Time,
) (ConversationTurn, ConversationItem, error) {
	if !utf8.ValidString(input) || len(input) > maxConversationText {
		return ConversationTurn{}, ConversationItem{}, errors.New("conversation input is invalid or too large")
	}
	if clientMessageID != nil && (len(*clientMessageID) == 0 || len(*clientMessageID) > 256) {
		return ConversationTurn{}, ConversationItem{}, errors.New("client message id is invalid")
	}
	turnID, err := newID("turn")
	if err != nil {
		return ConversationTurn{}, ConversationItem{}, err
	}
	itemID, err := newID("item")
	if err != nil {
		return ConversationTurn{}, ConversationItem{}, err
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ConversationTurn{}, ConversationItem{}, fmt.Errorf("begin conversation turn: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	if err := requireConversationTx(ctx, tx, conversationID); err != nil {
		return ConversationTurn{}, ConversationItem{}, err
	}
	var active bool
	if err := tx.QueryRowContext(ctx, `
SELECT EXISTS(
    SELECT 1 FROM conversation_turns
    WHERE conversation_id = ? AND status IN ('input_received', 'running', 'waiting_for_tool')
)`, conversationID).Scan(&active); err != nil {
		return ConversationTurn{}, ConversationItem{}, fmt.Errorf("check active conversation turn: %w", err)
	}
	if active {
		return ConversationTurn{}, ConversationItem{}, ErrConversationTurnActive
	}
	var turnIndex int64
	if err := tx.QueryRowContext(ctx, `
SELECT COALESCE(MAX(CAST(json_extract(metadata_json, '$.turn_index') AS INTEGER)), 0) + 1
FROM conversation_turns WHERE conversation_id = ?`, conversationID).Scan(&turnIndex); err != nil {
		return ConversationTurn{}, ConversationItem{}, fmt.Errorf("select conversation turn index: %w", err)
	}
	turnMetadata, _ := json.Marshal(map[string]any{"turn_index": turnIndex})
	if _, err := tx.ExecContext(ctx, `
INSERT INTO conversation_turns (
    turn_id, conversation_id, status, metadata_json,
    started_at_ms, created_at_ms, updated_at_ms
) VALUES (?, ?, 'input_received', ?, ?, ?, ?)`,
		turnID, conversationID, string(turnMetadata), millis(now), millis(now), millis(now)); err != nil {
		return ConversationTurn{}, ConversationItem{}, fmt.Errorf("create conversation turn: %w", err)
	}
	itemMetadata, _ := json.Marshal(map[string]any{
		"turn_index": turnIndex, "client_message_id": clientMessageID,
	})
	if _, err := tx.ExecContext(ctx, `
INSERT INTO conversation_items (
    item_id, conversation_id, turn_id, sequence_index, kind, status,
    author_actor_id, content_text, payload_json, metadata_json,
    created_at_ms, updated_at_ms
) VALUES (?, ?, ?, 1 + COALESCE((
    SELECT MAX(sequence_index) FROM conversation_items WHERE conversation_id = ?
), 0), 'user_text', 'completed', 'human:local', ?, '{}', ?, ?, ?)`,
		itemID, conversationID, turnID, conversationID, input, string(itemMetadata), millis(now), millis(now)); err != nil {
		return ConversationTurn{}, ConversationItem{}, fmt.Errorf("create conversation user item: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE conversations SET agent_status = 'input_received', updated_at_ms = ?
WHERE conversation_id = ?`, millis(now), conversationID); err != nil {
		return ConversationTurn{}, ConversationItem{}, fmt.Errorf("set conversation input status: %w", err)
	}
	item, err := conversationItemTx(ctx, tx, itemID)
	if err != nil {
		return ConversationTurn{}, ConversationItem{}, err
	}
	if err := tx.Commit(); err != nil {
		return ConversationTurn{}, ConversationItem{}, fmt.Errorf("commit conversation turn: %w", err)
	}
	return ConversationTurn{
		ID: turnID, ConversationID: conversationID, TurnIndex: turnIndex, Status: "input_received",
	}, item, nil
}

// BeginConversationContinuation starts one model turn from a saved terminal tool result.
func (s *Store) BeginConversationContinuation(
	ctx context.Context, conversationID, triggerItemID string, now time.Time,
) (ConversationTurn, error) {
	turnID, err := newID("turn")
	if err != nil {
		return ConversationTurn{}, err
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ConversationTurn{}, err
	}
	defer func() { _ = tx.Rollback() }()
	var triggerStatus string
	if err := tx.QueryRowContext(ctx, `SELECT status FROM conversation_items
WHERE item_id = ? AND conversation_id = ? AND deleted_at_ms IS NULL`,
		triggerItemID, conversationID).Scan(&triggerStatus); err != nil {
		return ConversationTurn{}, errors.New("conversation continuation trigger is unavailable")
	}
	if triggerStatus == "pending" || triggerStatus == "running" {
		return ConversationTurn{}, errors.New("conversation continuation trigger is not final")
	}
	var active bool
	if err := tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM conversation_turns
WHERE conversation_id = ? AND status IN ('input_received','running','waiting_for_tool'))`,
		conversationID).Scan(&active); err != nil {
		return ConversationTurn{}, err
	}
	if active {
		return ConversationTurn{}, ErrConversationTurnActive
	}
	var turnIndex int64
	if err := tx.QueryRowContext(ctx, `SELECT COALESCE(MAX(
CAST(json_extract(metadata_json, '$.turn_index') AS INTEGER)),0) + 1
FROM conversation_turns WHERE conversation_id = ?`, conversationID).Scan(&turnIndex); err != nil {
		return ConversationTurn{}, err
	}
	metadata, _ := json.Marshal(map[string]any{"turn_index": turnIndex, "continuation": "action_request"})
	if _, err := tx.ExecContext(ctx, `INSERT INTO conversation_turns (
turn_id, conversation_id, trigger_item_id, status, metadata_json,
started_at_ms, created_at_ms, updated_at_ms
) VALUES (?,?,?,'running',?,?,?,?)`, turnID, conversationID, triggerItemID,
		string(metadata), millis(now), millis(now), millis(now)); err != nil {
		return ConversationTurn{}, fmt.Errorf("create conversation continuation: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `UPDATE conversations SET agent_status = 'thinking', updated_at_ms = ?
WHERE conversation_id = ?`, millis(now), conversationID); err != nil {
		return ConversationTurn{}, err
	}
	if err := tx.Commit(); err != nil {
		return ConversationTurn{}, err
	}
	return ConversationTurn{ID: turnID, ConversationID: conversationID, TurnIndex: turnIndex, Status: "running"}, nil
}

// SetConversationAgentStatus saves one current Chat status.
func (s *Store) SetConversationAgentStatus(
	ctx context.Context,
	conversationID string,
	status string,
	now time.Time,
) error {
	switch status {
	case "idle", "input_received", "thinking", "tool_running",
		"waiting_for_previous_turn_completion", "interrupting", "error":
	default:
		return errors.New("conversation agent status is invalid")
	}
	result, err := s.db.ExecContext(ctx, `
UPDATE conversations SET agent_status = ?, updated_at_ms = ? WHERE conversation_id = ?`,
		status, millis(now.UTC()), conversationID)
	if err != nil {
		return fmt.Errorf("set conversation agent status: %w", err)
	}
	changed, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("inspect conversation status update: %w", err)
	}
	if changed != 1 {
		return ErrConversationNotFound
	}
	return nil
}

// ConversationProviderMessages returns completed human and assistant text in order.
func (s *Store) ConversationProviderMessages(
	ctx context.Context,
	conversationID string,
) ([]ProviderMessage, error) {
	if _, err := s.Conversation(ctx, conversationID); err != nil {
		return nil, err
	}
	rows, err := s.db.QueryContext(ctx, `
SELECT kind, CASE
    WHEN kind = 'assistant_text' THEN COALESCE(provider_content_text, content_text)
    ELSE content_text
END
FROM conversation_items
WHERE conversation_id = ? AND deleted_at_ms IS NULL AND status = 'completed'
  AND kind IN ('user_text', 'assistant_text')
ORDER BY sequence_index`, conversationID)
	if err != nil {
		return nil, fmt.Errorf("query provider conversation messages: %w", err)
	}
	defer rows.Close()
	messages := make([]ProviderMessage, 0)
	for rows.Next() {
		var kind, content string
		if err := rows.Scan(&kind, &content); err != nil {
			return nil, fmt.Errorf("scan provider conversation message: %w", err)
		}
		role := "assistant"
		if kind == "user_text" {
			role = "user"
		}
		messages = append(messages, ProviderMessage{Role: role, Content: content})
	}
	return messages, rows.Err()
}

// CompleteConversationTurn atomically saves assistant text and finishes its turn.
func (s *Store) CompleteConversationTurn(
	ctx context.Context,
	turn ConversationTurn,
	text string,
	providerText string,
	usage *ProviderUsage,
	now time.Time,
) (ConversationItem, error) {
	return s.CompleteConversationTurnOutput(ctx, turn, text, providerText, usage, nil, nil, 0, 0, now)
}

// CompleteConversationTurnOutput atomically saves reasoning, text, and terminal state.
func (s *Store) CompleteConversationTurnOutput(
	ctx context.Context,
	turn ConversationTurn,
	text string,
	providerText string,
	usage *ProviderUsage,
	reasoning []json.RawMessage,
	citations []ProviderCitation,
	unresolvedCitationMarkers int,
	providerRound int,
	now time.Time,
) (ConversationItem, error) {
	if strings.TrimSpace(text) == "" || !utf8.ValidString(text) || len(text) > maxConversationText {
		return ConversationItem{}, errors.New("assistant response is empty, invalid, or too large")
	}
	if providerRound < 0 || unresolvedCitationMarkers < 0 {
		return ConversationItem{}, errors.New("provider round is invalid")
	}
	for _, detail := range reasoning {
		if !json.Valid(detail) {
			return ConversationItem{}, errors.New("conversation reasoning is invalid")
		}
	}
	streamID := ConversationAssistantStreamID(turn.ID, providerRound)
	phase := "initial"
	if providerRound > 0 {
		phase = "continuation"
	}
	metadata := map[string]any{
		"turn_index": turn.TurnIndex, "response_index": 0, "output_index": 0,
		"provider_round": providerRound, "stream_id": streamID,
		"phase": "final_answer", "provider_item_id": nil,
	}
	addProviderCitationMetadata(metadata, citations, unresolvedCitationMarkers)
	if usage != nil {
		metadata["provider"] = usage.Provider
		ratio := float64(0)
		if usage.InputTokens > 0 {
			ratio = float64(usage.CachedInputTokens) / float64(usage.InputTokens)
		}
		metadata["provider_usage"] = map[string]any{
			"provider": usage.Provider, "model": usage.Model, "phase": phase,
			"response_index": 0, "output_index": 0, "input_tokens": usage.InputTokens,
			"output_tokens": usage.OutputTokens, "total_tokens": usage.TotalTokens,
			"cached_input_tokens": usage.CachedInputTokens, "cache_hit_ratio": ratio,
			"web_search_requests": usage.WebSearchRequests,
		}
	}
	return s.finishConversationTurn(
		ctx, turn, ConversationAssistantText, text, providerText, metadata,
		reasoning, providerRound, now,
	)
}

// CompleteConversationProgressAuditPause saves an audit pause without primary-provider attribution.
func (s *Store) CompleteConversationProgressAuditPause(
	ctx context.Context,
	turn ConversationTurn,
	text string,
	providerRound int,
	now time.Time,
) (ConversationItem, error) {
	if strings.TrimSpace(text) == "" || !utf8.ValidString(text) || len(text) > maxConversationText || providerRound < 0 {
		return ConversationItem{}, errors.New("progress audit response is empty, invalid, or too large")
	}
	return s.finishConversationTurn(
		ctx, turn, ConversationAssistantText, text, text,
		map[string]any{
			"turn_index": turn.TurnIndex, "response_index": 0, "output_index": 0,
			"provider_round": providerRound, "phase": "final_answer",
			"source": "progress_audit_pause", "provider": "progress_audit",
		}, nil, providerRound, now,
	)
}

func addProviderCitationMetadata(metadata map[string]any, citations []ProviderCitation, unresolved int) {
	if len(citations) != 0 {
		metadata["citations"] = citations
	}
	if unresolved != 0 {
		metadata["provider_citation_diagnostic"] = map[string]any{
			"code": "unresolved_marker", "count": unresolved,
		}
	}
}

// FailConversationTurn atomically saves a durable notice and marks the Chat failed.
func (s *Store) FailConversationTurn(
	ctx context.Context,
	turn ConversationTurn,
	message string,
	now time.Time,
) (ConversationItem, error) {
	if strings.TrimSpace(message) == "" {
		message = "The provider request failed."
	}
	if !utf8.ValidString(message) {
		message = strings.ToValidUTF8(message, "�")
	}
	if len(message) > 1000 {
		message = message[:1000]
		for !utf8.ValidString(message) {
			message = message[:len(message)-1]
		}
	}
	return s.finishConversationTurn(
		ctx, turn, ConversationErrorNotice, message, "",
		map[string]any{"turn_index": turn.TurnIndex, "recoverable": false}, nil, 0, now,
	)
}

// CancelConversationTurn cancels one active turn and restores an idle Chat.
func (s *Store) CancelConversationTurn(
	ctx context.Context,
	turn ConversationTurn,
	now time.Time,
) error {
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return fmt.Errorf("begin conversation turn cancellation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	result, err := tx.ExecContext(ctx, `
UPDATE conversation_turns
SET status = 'cancelled', completed_at_ms = ?, updated_at_ms = ?
WHERE turn_id = ? AND conversation_id = ?
  AND status IN ('input_received', 'running', 'waiting_for_tool')`,
		millis(now), millis(now), turn.ID, turn.ConversationID)
	if err != nil {
		return fmt.Errorf("cancel conversation turn: %w", err)
	}
	changed, err := result.RowsAffected()
	if err != nil {
		return fmt.Errorf("inspect conversation turn cancellation: %w", err)
	}
	if changed != 1 {
		return errors.New("conversation turn is already final")
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE conversation_items SET status = 'cancelled', updated_at_ms = ?
WHERE turn_id = ? AND status IN ('pending', 'running')`, millis(now), turn.ID); err != nil {
		return fmt.Errorf("cancel conversation turn items: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE conversations SET agent_status = 'idle', updated_at_ms = ?
WHERE conversation_id = ?`, millis(now), turn.ConversationID); err != nil {
		return fmt.Errorf("restore cancelled conversation: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("commit conversation turn cancellation: %w", err)
	}
	return nil
}

// RecoverConversationTurns cancels work left active by a stopped server.
func (s *Store) RecoverConversationTurns(ctx context.Context, now time.Time) (int64, error) {
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return 0, fmt.Errorf("begin conversation turn recovery: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	if _, err := tx.ExecContext(ctx, `
UPDATE conversations SET agent_status = 'idle', updated_at_ms = ?
WHERE conversation_id IN (
    SELECT conversation_id FROM conversation_turns
    WHERE status IN ('input_received', 'running', 'waiting_for_tool') AND NOT EXISTS (
        SELECT 1 FROM conversation_items choice WHERE choice.turn_id = conversation_turns.turn_id
        AND choice.kind = 'multiple_choice_prompt'
        AND json_extract(choice.payload_json, '$.lifecycle') IN ('pending', 'answered', 'resuming')
    )
)`, millis(now)); err != nil {
		return 0, fmt.Errorf("restore recovered conversations: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE conversation_items SET status = 'cancelled', updated_at_ms = ?
WHERE status IN ('pending', 'running') AND turn_id IN (
    SELECT turn_id FROM conversation_turns
    WHERE status IN ('input_received', 'running', 'waiting_for_tool') AND NOT EXISTS (
        SELECT 1 FROM conversation_items choice WHERE choice.turn_id = conversation_turns.turn_id
        AND choice.kind = 'multiple_choice_prompt'
        AND json_extract(choice.payload_json, '$.lifecycle') IN ('pending', 'answered', 'resuming')
    )
)`, millis(now)); err != nil {
		return 0, fmt.Errorf("cancel recovered conversation items: %w", err)
	}
	result, err := tx.ExecContext(ctx, `
UPDATE conversation_turns
SET status = 'cancelled', completed_at_ms = ?, updated_at_ms = ?
WHERE status IN ('input_received', 'running', 'waiting_for_tool') AND NOT EXISTS (
    SELECT 1 FROM conversation_items choice WHERE choice.turn_id = conversation_turns.turn_id
    AND choice.kind = 'multiple_choice_prompt'
    AND json_extract(choice.payload_json, '$.lifecycle') IN ('pending', 'answered', 'resuming')
)`, millis(now), millis(now))
	if err != nil {
		return 0, fmt.Errorf("cancel recovered conversation turns: %w", err)
	}
	changed, err := result.RowsAffected()
	if err != nil {
		return 0, fmt.Errorf("inspect conversation turn recovery: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return 0, fmt.Errorf("commit conversation turn recovery: %w", err)
	}
	return changed, nil
}

func (s *Store) finishConversationTurn(
	ctx context.Context,
	turn ConversationTurn,
	kind ConversationItemKind,
	content string,
	providerContent string,
	metadata map[string]any,
	reasoning []json.RawMessage,
	providerRound int,
	now time.Time,
) (ConversationItem, error) {
	now = now.UTC()
	providerKind, _ := metadata["provider"].(string)
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ConversationItem{}, fmt.Errorf("begin conversation turn completion: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	var status, conversationProvider string
	if err := tx.QueryRowContext(ctx, `
SELECT conversation_turns.status, conversations.provider
FROM conversation_turns
JOIN conversations ON conversations.conversation_id = conversation_turns.conversation_id
WHERE conversation_turns.turn_id = ? AND conversation_turns.conversation_id = ?`,
		turn.ID, turn.ConversationID).Scan(&status, &conversationProvider); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return ConversationItem{}, errors.New("conversation turn not found")
		}
		return ConversationItem{}, err
	}
	if status != "input_received" && status != "running" {
		return ConversationItem{}, errors.New("conversation turn is already final")
	}
	if providerKind == "" {
		providerKind = conversationProvider
	}
	metadata["provider"] = providerKind
	parentID, err := conversationTurnParentTx(ctx, tx, turn)
	if err != nil {
		return ConversationItem{}, err
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE conversation_items SET status = 'failed', updated_at_ms = ?
WHERE turn_id = ? AND status IN ('pending', 'running')`, millis(now), turn.ID); err != nil {
		return ConversationItem{}, fmt.Errorf("close unfinished conversation items: %w", err)
	}
	sequence, err := nextConversationSequenceTx(ctx, tx, turn.ConversationID)
	if err != nil {
		return ConversationItem{}, err
	}
	if len(reasoning) != 0 {
		_, err := insertConversationOutputTx(ctx, tx, ConversationItem{
			ID:             stableConversationOutputID(turn.ID, "reasoning", providerRound, 0),
			ConversationID: turn.ConversationID, TurnID: turn.ID, ParentItemID: parentID,
			Sequence: sequence, Kind: ConversationReasoning, Status: "completed",
			AuthorActorID: "agent:primary", Payload: map[string]any{"provider_details": reasoning},
			Metadata: map[string]any{
				"turn_index": turn.TurnIndex, "output_index": 0,
				"provider_round": providerRound, "source": "provider_reasoning",
				"provider": providerKind,
			},
			CreatedAt: now,
		})
		if err != nil {
			return ConversationItem{}, err
		}
		sequence++
	}
	itemID := stableConversationOutputID(turn.ID, string(kind), providerRound, 0)
	encodedMetadata, _ := json.Marshal(metadata)
	payload := "{}"
	itemStatus := "completed"
	author := "agent:primary"
	if kind == ConversationErrorNotice {
		itemStatus = "failed"
		encodedPayload, _ := json.Marshal(map[string]any{"message": content, "recoverable": false})
		payload = string(encodedPayload)
	}
	var storedProvider any
	if providerContent != "" && providerContent != content {
		storedProvider = providerContent
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO conversation_items (
    item_id, conversation_id, turn_id, parent_item_id, sequence_index,
    kind, status, author_actor_id, content_text, provider_content_text,
    payload_json, metadata_json, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		itemID, turn.ConversationID, turn.ID, parentID, sequence,
		kind, itemStatus, author, content, storedProvider, payload, string(encodedMetadata),
		millis(now), millis(now)); err != nil {
		return ConversationItem{}, fmt.Errorf("create final conversation item: %w", err)
	}
	turnStatus, agentStatus := "completed", "idle"
	if kind == ConversationErrorNotice {
		turnStatus, agentStatus = "failed", "error"
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE conversation_turns SET status = ?, completed_at_ms = ?, updated_at_ms = ? WHERE turn_id = ?`,
		turnStatus, millis(now), millis(now), turn.ID); err != nil {
		return ConversationItem{}, fmt.Errorf("finish conversation turn: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE conversations SET agent_status = ?, updated_at_ms = ? WHERE conversation_id = ?`,
		agentStatus, millis(now), turn.ConversationID); err != nil {
		return ConversationItem{}, fmt.Errorf("finish conversation status: %w", err)
	}
	if err := completeResumingConversationChoicesTx(ctx, tx, turn.ID, now); err != nil {
		return ConversationItem{}, err
	}
	item, err := conversationItemTx(ctx, tx, itemID)
	if err != nil {
		return ConversationItem{}, err
	}
	if err := tx.Commit(); err != nil {
		return ConversationItem{}, fmt.Errorf("commit conversation turn completion: %w", err)
	}
	return item, nil
}

func conversationTurnParentTx(ctx context.Context, tx *sql.Tx, turn ConversationTurn) (string, error) {
	var parentID string
	err := tx.QueryRowContext(ctx, `SELECT item_id FROM conversation_items
WHERE turn_id = ? AND kind = 'user_text' ORDER BY sequence_index LIMIT 1`, turn.ID).Scan(&parentID)
	if err == nil {
		return parentID, nil
	}
	if !errors.Is(err, sql.ErrNoRows) {
		return "", fmt.Errorf("find conversation turn parent: %w", err)
	}
	if err := tx.QueryRowContext(ctx, `SELECT trigger_item_id FROM conversation_turns
WHERE turn_id = ? AND conversation_id = ? AND trigger_item_id IS NOT NULL`,
		turn.ID, turn.ConversationID).Scan(&parentID); err != nil {
		return "", errors.New("conversation turn parent is unavailable")
	}
	return parentID, nil
}

// ConversationItemPage returns visible items before an optional cursor.
func (s *Store) ConversationItemPage(
	ctx context.Context,
	conversationID string,
	cursor string,
	limit int,
) (ConversationItemPage, error) {
	if limit < 1 || limit > 200 {
		return ConversationItemPage{}, errors.New("conversation item limit must be within 1..200")
	}
	if _, err := s.Conversation(ctx, conversationID); err != nil {
		return ConversationItemPage{}, err
	}
	before := int64(1<<63 - 1)
	if cursor != "" {
		value, ok := strings.CutPrefix(cursor, conversationCursorPrefix)
		parsed, err := strconv.ParseInt(value, 10, 64)
		if !ok || err != nil || parsed < 1 {
			return ConversationItemPage{}, errors.New("invalid conversation item cursor")
		}
		before = parsed
	}
	rows, err := s.db.QueryContext(ctx, `
SELECT item_id, conversation_id, COALESCE(turn_id, ''), COALESCE(parent_item_id, ''),
       sequence_index, kind, status, author_actor_id, COALESCE(content_text, ''),
       COALESCE(provider_content_text, ''), payload_json, metadata_json, created_at_ms
FROM conversation_items
WHERE conversation_id = ? AND deleted_at_ms IS NULL
  AND kind NOT IN ('model_context_update', 'reasoning') AND sequence_index < ?
ORDER BY sequence_index DESC LIMIT ?`, conversationID, before, limit+1)
	if err != nil {
		return ConversationItemPage{}, fmt.Errorf("query conversation item page: %w", err)
	}
	defer rows.Close()
	items := make([]ConversationItem, 0, limit+1)
	for rows.Next() {
		item, err := scanConversationItem(rows)
		if err != nil {
			return ConversationItemPage{}, err
		}
		items = append(items, item)
	}
	if err := rows.Err(); err != nil {
		return ConversationItemPage{}, err
	}
	page := ConversationItemPage{HasMoreBefore: len(items) > limit}
	if page.HasMoreBefore {
		items = items[:limit]
	}
	for left, right := 0, len(items)-1; left < right; left, right = left+1, right-1 {
		items[left], items[right] = items[right], items[left]
	}
	page.Items = items
	if len(items) > 0 {
		page.BeforeCursor = items[0].Cursor
	}
	return page, nil
}

func requireConversationTx(ctx context.Context, tx *sql.Tx, conversationID string) error {
	var exists bool
	if err := tx.QueryRowContext(ctx, `
SELECT EXISTS(SELECT 1 FROM conversations WHERE conversation_id = ? AND owner_human_id = 'human:local')`,
		conversationID).Scan(&exists); err != nil {
		return fmt.Errorf("check conversation: %w", err)
	}
	if !exists {
		return ErrConversationNotFound
	}
	return nil
}

func conversationItemTx(ctx context.Context, tx *sql.Tx, itemID string) (ConversationItem, error) {
	return scanConversationItem(tx.QueryRowContext(ctx, `
SELECT item_id, conversation_id, COALESCE(turn_id, ''), COALESCE(parent_item_id, ''),
       sequence_index, kind, status, author_actor_id, COALESCE(content_text, ''),
       COALESCE(provider_content_text, ''), payload_json, metadata_json, created_at_ms
FROM conversation_items WHERE item_id = ?`, itemID))
}

func scanConversationItem(row rowScanner) (ConversationItem, error) {
	var item ConversationItem
	var kind string
	var payload, metadata string
	var createdAt int64
	if err := row.Scan(
		&item.ID, &item.ConversationID, &item.TurnID, &item.ParentItemID,
		&item.Sequence, &kind, &item.Status, &item.AuthorActorID, &item.ContentText,
		&item.ProviderContentText, &payload, &metadata, &createdAt,
	); err != nil {
		return ConversationItem{}, fmt.Errorf("scan conversation item: %w", err)
	}
	item.Kind = ConversationItemKind(kind)
	item.Cursor = conversationCursorPrefix + strconv.FormatInt(item.Sequence, 10)
	item.CreatedAt = fromMillis(createdAt)
	if err := json.Unmarshal([]byte(payload), &item.Payload); err != nil {
		return ConversationItem{}, errors.New("stored conversation item payload is invalid")
	}
	if err := json.Unmarshal([]byte(metadata), &item.Metadata); err != nil {
		return ConversationItem{}, errors.New("stored conversation item metadata is invalid")
	}
	return item, nil
}

// ConversationAssistantStreamID returns the stream identity for one provider round.
func ConversationAssistantStreamID(turnID string, providerRound int) string {
	phase := "initial"
	if providerRound == 1 {
		phase = "continuation"
	} else if providerRound > 1 {
		phase = fmt.Sprintf("continuation-%d", providerRound-1)
	}
	return "assistant_stream:" + turnID + ":" + phase + ":response:0"
}

func stableConversationItemID(turnID string, kind string) string {
	return stableConversationOutputID(turnID, kind, 0, 0)
}

func stableConversationOutputID(turnID string, kind string, providerRound int, outputIndex int) string {
	digest := sha256.Sum256([]byte(fmt.Sprintf(
		"%s:%s:%d:%d", turnID, kind, providerRound, outputIndex,
	)))
	return "item:" + hex.EncodeToString(digest[:16])
}
