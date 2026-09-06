package store

import (
	"context"
	"errors"
	"strings"
	"time"

	"github.com/uptrace/bun"
)

// ConversationChoiceSelection identifies the options selected for a normal user message.
type ConversationChoiceSelection struct {
	PromptItemID string
	OptionIDs    []string
}

func (s *Store) ConversationChoiceText(ctx context.Context, conversationID string, choice ConversationChoiceSelection) (string, error) {
	tx, err := s.db.BeginTx(ctx, nil)
	if err != nil {
		return "", err
	}
	defer func() { _ = tx.Rollback() }()
	text, _, err := conversationChoiceTextTx(ctx, tx, conversationID, choice)
	return text, err
}

func (s *Store) BeginConversationChoiceTurn(ctx context.Context, conversationID string, choice ConversationChoiceSelection, clientMessageID *string, now time.Time) (ConversationTurn, ConversationItem, error) {
	return s.beginConversationTurn(ctx, conversationID, "", clientMessageID, &choice, now)
}

func conversationChoiceTextTx(ctx context.Context, tx bun.Tx, conversationID string, choice ConversationChoiceSelection) (string, map[string]any, error) {
	prompt, err := conversationItemTx(ctx, tx, choice.PromptItemID)
	if err != nil || prompt.ConversationID != conversationID || prompt.Kind != ConversationMultipleChoicePrompt {
		return "", nil, errors.New("multiple-choice prompt is unavailable")
	}
	var selectedBefore bool
	if err := tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM conversation_items WHERE parent_item_id = ? AND kind = 'multiple_choice_selection')`, prompt.ID).Scan(&selectedBefore); err != nil {
		return "", nil, err
	}
	if selectedBefore {
		return "", nil, errors.New("multiple-choice prompt already has a selection")
	}
	mode := textJSON(prompt.Payload["selection_mode"])
	options, err := storedChoiceOptions(prompt.Payload["options"])
	if err != nil {
		return "", nil, err
	}
	if mode == "pick_one" && len(choice.OptionIDs) != 1 ||
		mode == "pick_many" && len(choice.OptionIDs) == 0 {
		return "", nil, errors.New("multiple-choice selection has an invalid size")
	}
	requested := make(map[string]struct{}, len(choice.OptionIDs))
	for _, id := range choice.OptionIDs {
		if _, exists := requested[id]; exists || strings.TrimSpace(id) == "" {
			return "", nil, errors.New("multiple-choice option ids must be unique")
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
		return "", nil, errors.New("multiple-choice option is not in the prompt")
	}
	labels := make([]string, 0, len(selected))
	for _, option := range selected {
		labels = append(labels, option.Label)
	}
	return strings.Join(labels, ", "), map[string]any{"prompt_item_id": prompt.ID, "selection_mode": mode, "selected_options": choiceOptionsPayload(selected)}, nil
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
