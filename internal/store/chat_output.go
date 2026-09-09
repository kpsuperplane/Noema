package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"
	"unicode/utf8"
)

// SaveConversationOutput updates one readable provider section in place.
func (s *Store) SaveConversationOutput(ctx context.Context, turn ConversationTurn, round, index, section int, kind, phase, providerID, text, providerText, status string, citations []ProviderCitation, now time.Time) (ConversationItem, error) {
	if !utf8.ValidString(text) || len(text) > maxConversationText || !utf8.ValidString(providerText) || len(providerText) > maxConversationText {
		return ConversationItem{}, errors.New("provider output is invalid or too large")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ConversationItem{}, err
	}
	defer tx.Rollback()
	var current, providerKind string
	if err := tx.QueryRowContext(ctx, `SELECT t.status, c.provider FROM conversation_turns t JOIN conversations c ON c.conversation_id = t.conversation_id WHERE t.turn_id = ? AND t.conversation_id = ?`, turn.ID, turn.ConversationID).Scan(&current, &providerKind); err != nil {
		return ConversationItem{}, err
	}
	if current != "input_received" && current != "running" {
		return ConversationItem{}, errors.New("conversation turn is no longer running")
	}
	id := stableConversationOutputID(turn.ID, fmt.Sprintf("section:%s:%d", kind, section), round, index)
	metadata := map[string]any{"provider": providerKind, "turn_index": turn.TurnIndex, "response_index": index, "provider_round": round, "output_index": index, "section_index": section, "provider_item_id": providerID, "provider_phase": phase, "provider_output_kind": kind, "stream_id": id, "phase": "commentary", "output_status": status}
	addProviderCitationMetadata(metadata, citations, 0)
	if phase == "final_answer" {
		metadata["phase"] = phase
	}
	encoded, _ := json.Marshal(metadata)
	var storedProvider any
	if providerText != text && providerText != "" {
		storedProvider = providerText
	}
	result, err := tx.ExecContext(ctx, `UPDATE conversation_items SET content_text = ?, provider_content_text = ?, metadata_json = ?, status = ?, updated_at_ms = ? WHERE item_id = ?`, text, storedProvider, string(encoded), status, millis(now), id)
	if err != nil {
		return ConversationItem{}, err
	}
	count, err := result.RowsAffected()
	if err != nil {
		return ConversationItem{}, err
	}
	if count == 0 {
		if strings.TrimSpace(text) == "" {
			return ConversationItem{}, nil
		}
		sequence, err := nextConversationSequenceTx(ctx, tx, turn.ConversationID)
		if err != nil {
			return ConversationItem{}, err
		}
		parent, err := conversationTurnParentTx(ctx, tx, turn)
		if err != nil {
			return ConversationItem{}, err
		}
		_, err = insertConversationOutputTx(ctx, tx, ConversationItem{ID: id, ConversationID: turn.ConversationID, TurnID: turn.ID, ParentItemID: parent, Sequence: sequence, Kind: ConversationAssistantText, Status: status, AuthorActorID: "agent:primary", ContentText: text, ProviderContentText: providerText, Payload: map[string]any{}, Metadata: metadata, CreatedAt: now})
		if err != nil {
			return ConversationItem{}, err
		}
	}
	item, err := conversationItemTx(ctx, tx, id)
	if err != nil {
		return ConversationItem{}, err
	}
	if err = tx.Commit(); err != nil {
		return ConversationItem{}, err
	}
	return item, nil
}
