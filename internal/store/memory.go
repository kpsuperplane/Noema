package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
)

// MemorySourceRange is one finite completed Chat range for Memory updates.
type MemorySourceRange struct {
	ConversationID string
	CapturedHead   int64
	Items          []ConversationItem
}

// CaptureMemorySourceRange reads one finite completed source range.
func (s *Store) CaptureMemorySourceRange(
	ctx context.Context,
	conversationID string,
	afterSequence int64,
) (MemorySourceRange, error) {
	if afterSequence < 0 {
		return MemorySourceRange{}, errors.New("Memory source sequence cannot be negative")
	}
	if _, err := s.Conversation(ctx, conversationID); err != nil {
		return MemorySourceRange{}, err
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{ReadOnly: true})
	if err != nil {
		return MemorySourceRange{}, fmt.Errorf("begin Memory source snapshot: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	var head int64
	if err := tx.QueryRowContext(ctx, `
SELECT COALESCE(MAX(sequence_index), 0)
FROM conversation_items
WHERE conversation_id = ? AND deleted_at_ms IS NULL AND status = 'completed'
  AND (kind IN ('user_text', 'multiple_choice_selection', 'assistant_text', 'tool_result')
    OR (kind = 'activity'
      AND json_extract(metadata_json, '$.source') = 'provider_action'
      AND json_extract(payload_json, '$.activity_kind') = 'tool_result'))`, conversationID).Scan(&head); err != nil {
		return MemorySourceRange{}, fmt.Errorf("read Memory source head: %w", err)
	}
	rows, err := tx.QueryContext(ctx, `
SELECT item_id, conversation_id, COALESCE(turn_id, ''), COALESCE(parent_item_id, ''),
       sequence_index, kind, status, author_actor_id, COALESCE(content_text, ''),
       COALESCE(provider_content_text, ''), payload_json, metadata_json, created_at_ms
FROM conversation_items
WHERE conversation_id = ? AND deleted_at_ms IS NULL
  AND sequence_index > ? AND sequence_index <= ? AND status = 'completed'
  AND (kind IN ('user_text', 'multiple_choice_selection', 'assistant_text', 'tool_result')
    OR (kind = 'activity'
      AND json_extract(metadata_json, '$.source') = 'provider_action'
      AND json_extract(payload_json, '$.activity_kind') = 'tool_result'))
ORDER BY sequence_index`, conversationID, afterSequence, head)
	if err != nil {
		return MemorySourceRange{}, fmt.Errorf("read Memory source items: %w", err)
	}
	defer rows.Close()
	items := make([]ConversationItem, 0)
	for rows.Next() {
		item, err := scanConversationItem(rows)
		if err != nil {
			return MemorySourceRange{}, err
		}
		items = append(items, item)
	}
	if err := rows.Err(); err != nil {
		return MemorySourceRange{}, err
	}
	if err := tx.Commit(); err != nil {
		return MemorySourceRange{}, fmt.Errorf("finish Memory source snapshot: %w", err)
	}
	return MemorySourceRange{
		ConversationID: conversationID, CapturedHead: head, Items: items,
	}, nil
}

// VisibleConversationItem returns one non-deleted item by exact ID.
func (s *Store) VisibleConversationItem(ctx context.Context, itemID string) (*ConversationItem, error) {
	item, err := scanConversationItem(s.db.QueryRowContext(ctx, `
SELECT item_id, conversation_id, COALESCE(turn_id, ''), COALESCE(parent_item_id, ''),
       sequence_index, kind, status, author_actor_id, COALESCE(content_text, ''),
       COALESCE(provider_content_text, ''), payload_json, metadata_json, created_at_ms
FROM conversation_items WHERE item_id = ? AND deleted_at_ms IS NULL`, itemID))
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, nil
		}
		return nil, err
	}
	return &item, nil
}
