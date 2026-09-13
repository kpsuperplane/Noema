package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"time"
)

// ModelContextUpdate replaces or removes one saved section of model context.
type ModelContextUpdate struct {
	SectionID   string `json:"section_id"`
	Operation   string `json:"operation"`
	Content     string `json:"content,omitempty"`
	Instruction string `json:"instruction"`
}

func (u ModelContextUpdate) Message() string {
	encoded, _ := json.Marshal(u)
	return "NOEMA_MODEL_CONTEXT_UPDATE\n" + string(encoded)
}

// AppendModelContextUpdates saves the ordered changes under the active turn.
func (s *Store) AppendModelContextUpdates(ctx context.Context, turn ConversationTurn, updates []ModelContextUpdate, now time.Time) ([]ConversationItem, error) {
	if len(updates) == 0 {
		return nil, nil
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, err
	}
	defer tx.Rollback()
	var active int
	if err := tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM conversation_turns WHERE turn_id=? AND conversation_id=? AND status IN ('input_received','running','waiting_for_tool')`, turn.ID, turn.ConversationID).Scan(&active); err != nil {
		return nil, err
	}
	if active != 1 {
		return nil, errors.New("conversation turn is already final")
	}
	parent, err := conversationTurnParentTx(ctx, tx, turn)
	if err != nil {
		return nil, err
	}
	sequence, err := nextConversationSequenceTx(ctx, tx, turn.ConversationID)
	if err != nil {
		return nil, err
	}
	items := make([]ConversationItem, 0, len(updates))
	for _, update := range updates {
		switch update.SectionID {
		case "agent.identity", "runtime.environment", "projects.catalog", "tools.visibility":
		default:
			return nil, errors.New("unknown model context section")
		}
		if update.Operation != "full" && update.Operation != "replacement" && update.Operation != "removal" {
			return nil, errors.New("invalid model context operation")
		}
		id, err := newID("item")
		if err != nil {
			return nil, err
		}
		item := ConversationItem{ID: id, ConversationID: turn.ConversationID, TurnID: turn.ID, ParentItemID: parent, Sequence: sequence,
			Kind: ConversationModelContextUpdate, Status: "completed", AuthorActorID: "agent:primary", ContentText: update.Message(),
			Payload: map[string]any{"model_context_update": update}, Metadata: map[string]any{"source": "model_context_ledger", "section_id": update.SectionID}, CreatedAt: now.UTC()}
		saved, err := insertConversationOutputTx(ctx, tx, item)
		if err != nil {
			return nil, err
		}
		items = append(items, saved)
		sequence++
	}
	return items, tx.Commit()
}
