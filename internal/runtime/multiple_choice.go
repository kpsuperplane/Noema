package runtime

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"strings"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const presentMultipleChoiceName = "noema.present_multiple_choice"

var presentMultipleChoiceSchema = json.RawMessage(`{
  "type":"object",
  "properties":{
    "prompt":{"type":"string","minLength":1},
    "selection_mode":{"type":"string","enum":["pick_one","pick_many"]},
    "options":{"type":"array","minItems":1,"items":{"type":"object","properties":{"id":{"type":"string","minLength":1},"label":{"type":"string","minLength":1}},"required":["id","label"],"additionalProperties":false}}
  },
  "required":["prompt","selection_mode","options"],
  "additionalProperties":false
}`)

type multipleChoiceArguments struct {
	Prompt        string                                   `json:"prompt"`
	SelectionMode string                                   `json:"selection_mode"`
	Options       []store.ConversationMultipleChoiceOption `json:"options"`
}

func presentMultipleChoiceTool() provider.GenerationTool {
	return provider.GenerationTool{
		Name:        presentMultipleChoiceName,
		Description: "Display a question with optional multiple-choice buttons and finish this turn. Put the complete question in prompt; do not repeat it in assistant text. A selected label arrives later as a normal user message.",
		InputSchema: append(json.RawMessage(nil), presentMultipleChoiceSchema...),
	}
}

func parseMultipleChoiceArguments(raw json.RawMessage) (*multipleChoiceArguments, error) {
	if len(raw) == 0 || len(raw) > 256*1024 {
		return nil, errors.New("multiple-choice arguments are invalid or too large")
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.DisallowUnknownFields()
	var value multipleChoiceArguments
	if err := decoder.Decode(&value); err != nil {
		return nil, errors.New("multiple-choice arguments are invalid")
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return nil, errors.New("multiple-choice arguments are invalid")
	}
	value.Prompt = strings.TrimSpace(value.Prompt)
	if value.Prompt == "" || (value.SelectionMode != "pick_one" && value.SelectionMode != "pick_many") || len(value.Options) == 0 {
		return nil, errors.New("multiple-choice arguments are invalid")
	}
	ids := make(map[string]struct{}, len(value.Options))
	for index := range value.Options {
		value.Options[index].ID = strings.TrimSpace(value.Options[index].ID)
		value.Options[index].Label = strings.TrimSpace(value.Options[index].Label)
		if value.Options[index].ID == "" || value.Options[index].Label == "" {
			return nil, errors.New("multiple-choice arguments are invalid")
		}
		if _, exists := ids[value.Options[index].ID]; exists {
			return nil, errors.New("multiple-choice option ids must be unique")
		}
		ids[value.Options[index].ID] = struct{}{}
	}
	return &value, nil
}

// SendMultipleChoiceSelection sends the selected labels through the normal message queue.
func (c *Chat) SendMultipleChoiceSelection(ctx context.Context, conversationID, promptItemID string, selectedOptionIDs []string, clientMessageID *string) (TurnAccepted, error) {
	choice := &store.ConversationChoiceSelection{PromptItemID: promptItemID, OptionIDs: append([]string(nil), selectedOptionIDs...)}
	input, err := c.database.ConversationChoiceText(ctx, conversationID, *choice)
	if err != nil {
		return TurnAccepted{}, err
	}
	return c.SendTurn(ctx, SendTurnInput{ConversationID: conversationID, Input: input, ClientMessageID: clientMessageID, choice: choice})
}
