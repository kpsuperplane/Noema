package runtime

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"strings"
	"time"

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

type choiceResolution struct {
	ctx                          context.Context
	conversationID, promptItemID string
	selectedOptionIDs            []string
	clientMessageID              *string
	reply                        chan error
}

func presentMultipleChoiceTool() provider.GenerationTool {
	return provider.GenerationTool{
		Name:        presentMultipleChoiceName,
		Description: "Present a multiple-choice question with stable option IDs and labels to the human.",
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

func storedMultipleChoiceInput(
	value *multipleChoiceArguments, assignment store.ModelAssignment, responseID string, hostedState bool,
) *store.ConversationMultipleChoiceInput {
	if value == nil {
		return nil
	}
	return &store.ConversationMultipleChoiceInput{
		Prompt: value.Prompt, SelectionMode: value.SelectionMode, Options: value.Options,
		ProviderSelection: modelAssignmentValue(assignment), ResponseID: responseID, HostedState: hostedState,
	}
}

// SendMultipleChoiceSelection validates and queues one exact prompt answer.
func (c *Chat) SendMultipleChoiceSelection(
	ctx context.Context, conversationID, promptItemID string, selectedOptionIDs []string, clientMessageID *string,
) (TurnAccepted, error) {
	if _, err := c.database.Conversation(ctx, conversationID); err != nil {
		return TurnAccepted{}, err
	}
	request := choiceResolution{
		ctx:            ctx,
		conversationID: conversationID, promptItemID: promptItemID,
		selectedOptionIDs: append([]string(nil), selectedOptionIDs...),
		clientMessageID:   cloneOptionalString(clientMessageID),
		reply:             make(chan error, 1),
	}
	c.stateMu.RLock()
	defer c.stateMu.RUnlock()
	if c.closed {
		return TurnAccepted{}, ErrChatClosed
	}
	select {
	case <-ctx.Done():
		return TurnAccepted{}, ctx.Err()
	case c.choices <- request:
	}
	select {
	case <-ctx.Done():
		return TurnAccepted{}, ctx.Err()
	case <-c.ctx.Done():
		return TurnAccepted{}, ErrChatClosed
	case err := <-request.reply:
		if err != nil {
			return TurnAccepted{}, err
		}
		return TurnAccepted{ConversationID: conversationID, ClientMessageID: request.clientMessageID}, nil
	}
}

func (c *Chat) resolveMultipleChoice(request choiceResolution) {
	if err := request.ctx.Err(); err != nil {
		request.reply <- err
		return
	}
	choice, err := c.database.ResolveConversationChoice(
		c.ctx, request.conversationID, request.promptItemID, request.selectedOptionIDs,
		request.clientMessageID, time.Now(),
	)
	if err != nil {
		request.reply <- err
		return
	}
	for _, item := range []store.ConversationItem{choice.Selection, choice.Result} {
		item := item
		c.publish(Event{Kind: EventConversationItem, ConversationID: request.conversationID,
			ClientMessageID: request.clientMessageID, TurnID: choice.Turn.ID, Item: &item})
	}
	request.reply <- nil
	c.resumeMultipleChoice(choice)
}

func (c *Chat) resumeMultipleChoice(choice store.ConversationChoiceContinuation) {
	claimed, err := c.database.ClaimConversationChoice(c.ctx, choice.Prompt.ID, time.Now())
	if err != nil {
		return
	}
	assignment, err := storedModelAssignment(claimed.Prompt.Payload)
	if err != nil {
		c.failChoice(claimed, err)
		return
	}
	generator, err := c.generatorFor(assignment.ProviderKind)
	if err != nil {
		c.failChoice(claimed, err)
		return
	}
	conversation, err := c.database.Conversation(c.ctx, claimed.Turn.ConversationID)
	if err != nil {
		c.failChoice(claimed, err)
		return
	}
	clientMessageID, _ := claimed.Selection.Metadata["client_message_id"].(string)
	request := queuedTurn{input: SendTurnInput{ConversationID: conversation.ID}, conversation: conversation, location: time.UTC}
	if clientMessageID != "" {
		request.input.ClientMessageID = &clientMessageID
	}
	replay, err := storedToolResult(claimed.Result)
	if err != nil {
		c.failChoice(claimed, err)
		return
	}
	call, err := storedToolCallForChoice(c.database, c.ctx, claimed.Prompt)
	if err != nil {
		c.failChoice(claimed, err)
		return
	}
	replay.Arguments = call.Arguments
	providerRound := intJSONValue(claimed.Prompt.Payload["provider_round"])
	responseID, _ := claimed.Prompt.Payload["response_id"].(string)
	hostedState, _ := claimed.Prompt.Payload["hosted_state"].(bool)
	c.publish(Event{Kind: EventAgentStatus, ConversationID: conversation.ID, Status: AgentStatusThinking})
	result, _, err := c.generateChatToolContinuation(
		request, claimed.Turn, assignment, generator, providerRound+1, "", c.memoryRootContext(),
		responseID, hostedState, provider.GenerationMessage{Role: "tool", ToolResult: &replay},
	)
	if err != nil {
		if c.ctx.Err() != nil {
			_ = c.database.ReleaseConversationChoice(context.Background(), claimed.Prompt.ID, time.Now())
			return
		}
		c.failChoice(claimed, err)
		return
	}
	if err := c.database.CompleteConversationChoice(c.ctx, claimed.Prompt.ID, time.Now()); err != nil {
		c.failTurn(request.input, claimed.Turn, err)
		return
	}
	c.executeChatToolRounds(request, claimed.Turn, assignment, generator, result, c.memoryRootContext(), providerRound+1, hostedState)
}

func (c *Chat) failChoice(choice store.ConversationChoiceContinuation, err error) {
	_ = c.database.CompleteConversationChoice(c.ctx, choice.Prompt.ID, time.Now())
	c.failTurn(SendTurnInput{ConversationID: choice.Turn.ConversationID}, choice.Turn, err)
}

func storedToolCallForChoice(database *store.Store, ctx context.Context, prompt store.ConversationItem) (provider.ReplayToolCall, error) {
	items, err := database.ConversationProviderItems(ctx, prompt.ConversationID)
	if err != nil {
		return provider.ReplayToolCall{}, err
	}
	callID, _ := prompt.Payload["call_item_id"].(string)
	for _, item := range items {
		if item.ID == callID {
			return storedToolCall(item)
		}
	}
	return provider.ReplayToolCall{}, errors.New("multiple-choice tool call is unavailable")
}

func intJSONValue(value any) int { number, _ := value.(float64); return int(number) }
