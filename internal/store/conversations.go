package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/uptrace/bun"
)

var ErrConversationNotFound = errors.New("conversation not found")

// Conversation is the durable identity of one local Chat.
type Conversation struct {
	ID        string
	Provider  string
	CWD       string
	CreatedAt time.Time
	UpdatedAt time.Time
}

// ConversationWorkingDirectory returns and persists one Conversation's safe
// working directory. An explicit existing directory replaces the stored value.
func (s *Store) ConversationWorkingDirectory(ctx context.Context, conversationID, requested string) (string, error) {
	conversation, err := s.Conversation(ctx, conversationID)
	if err != nil {
		return "", err
	}
	explicit := strings.TrimSpace(requested)
	path := conversation.CWD
	if explicit != "" {
		path = explicit
	} else if path == "" {
		path = filepath.Join(s.homeRoot, "conversations", conversationID)
	}
	path, err = filepath.Abs(path)
	if err != nil {
		return "", fmt.Errorf("resolve conversation working directory: %w", err)
	}
	if explicit != "" {
		info, statErr := os.Stat(path)
		if statErr != nil {
			return "", fmt.Errorf("conversation working directory is unavailable: %w", statErr)
		}
		if !info.IsDir() {
			return "", errors.New("conversation working directory is not a directory")
		}
	} else if err := os.MkdirAll(path, 0o700); err != nil {
		return "", fmt.Errorf("create conversation working directory: %w", err)
	}
	if _, err := s.db.ExecContext(ctx, `UPDATE conversations SET cwd=?,updated_at_ms=? WHERE conversation_id=?`, path, millis(time.Now().UTC()), conversationID); err != nil {
		return "", fmt.Errorf("persist conversation working directory: %w", err)
	}
	return path, nil
}

// PrimaryConversation returns the local human's active primary Chat.
func (s *Store) PrimaryConversation(ctx context.Context) (*Conversation, error) {
	row := s.db.QueryRowContext(ctx, `
SELECT conversations.conversation_id, conversations.provider,
       COALESCE(conversations.cwd, ''), conversations.created_at_ms, conversations.updated_at_ms
FROM local_human_state
JOIN conversations ON conversations.conversation_id = local_human_state.primary_conversation_id
WHERE local_human_state.state_id = 1`)
	conversation, err := scanConversation(row)
	if errors.Is(err, ErrConversationNotFound) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return &conversation, nil
}

// EnsurePrimaryConversation returns or creates the local human's primary Chat.
func (s *Store) EnsurePrimaryConversation(
	ctx context.Context,
	providerKind string,
	cwd string,
	now time.Time,
) (Conversation, error) {
	providerKind = strings.TrimSpace(providerKind)
	if providerKind == "" || len(providerKind) > 64 {
		return Conversation{}, errors.New("conversation provider is invalid")
	}
	cwd = strings.TrimSpace(cwd)
	now = now.UTC()
	var conversationCWD *string
	if cwd != "" {
		conversationCWD = &cwd
	}
	draft := NewLocalConversationForProvider(providerKind, nil, conversationCWD)
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return Conversation{}, fmt.Errorf("begin primary conversation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()

	current, err := primaryConversationTx(ctx, tx)
	if err == nil {
		return commitConversation(tx, current)
	}
	if !errors.Is(err, ErrConversationNotFound) {
		return Conversation{}, err
	}
	id, err := newID("conversation")
	if err != nil {
		return Conversation{}, err
	}
	var storedCWD any
	if cwd != "" {
		storedCWD = cwd
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO conversations (
		conversation_id, owner_human_id, provider, cwd, created_at_ms, updated_at_ms
) VALUES (?, ?, ?, ?, ?, ?)`, id, draft.Owner.HumanID, draft.Provider, storedCWD, millis(now), millis(now)); err != nil {
		return Conversation{}, fmt.Errorf("create primary conversation: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE local_human_state SET primary_conversation_id = ? WHERE state_id = 1`, id); err != nil {
		return Conversation{}, fmt.Errorf("select primary conversation: %w", err)
	}
	conversation, err := commitConversation(tx, Conversation{
		ID: id, Provider: draft.Provider, CWD: cwd, CreatedAt: now, UpdatedAt: now,
	})
	if err == nil {
		s.NotifyWork()
	}
	return conversation, err
}

// Conversation returns one local-human-owned Chat by exact identifier.
func (s *Store) Conversation(ctx context.Context, id string) (Conversation, error) {
	return scanConversation(s.db.QueryRowContext(ctx, `
SELECT conversation_id, provider, COALESCE(cwd, ''), created_at_ms, updated_at_ms
FROM conversations WHERE conversation_id = ? AND owner_human_id = 'human:local'`, id))
}

func primaryConversationTx(ctx context.Context, tx bun.Tx) (Conversation, error) {
	return scanConversation(tx.QueryRowContext(ctx, `
SELECT conversations.conversation_id, conversations.provider,
       COALESCE(conversations.cwd, ''), conversations.created_at_ms, conversations.updated_at_ms
FROM local_human_state
JOIN conversations ON conversations.conversation_id = local_human_state.primary_conversation_id
WHERE local_human_state.state_id = 1`))
}

func scanConversation(row rowScanner) (Conversation, error) {
	var conversation Conversation
	var createdAt, updatedAt int64
	if err := row.Scan(&conversation.ID, &conversation.Provider, &conversation.CWD, &createdAt, &updatedAt); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return Conversation{}, ErrConversationNotFound
		}
		return Conversation{}, fmt.Errorf("scan conversation: %w", err)
	}
	conversation.CreatedAt = fromMillis(createdAt)
	conversation.UpdatedAt = fromMillis(updatedAt)
	return conversation, nil
}

func commitConversation(tx bun.Tx, conversation Conversation) (Conversation, error) {
	if err := tx.Commit(); err != nil {
		return Conversation{}, fmt.Errorf("commit primary conversation: %w", err)
	}
	return conversation, nil
}
