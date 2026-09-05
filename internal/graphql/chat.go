package graphql

import (
	"context"
	"errors"
	"fmt"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) primaryConversation(ctx context.Context) (*model.PrimaryConversation, error) {
	conversation, err := r.Store.PrimaryConversation(ctx)
	if err != nil || conversation == nil {
		return nil, err
	}
	if providerKind, err := r.primaryModelProviderKind(ctx); err != nil {
		return nil, err
	} else if providerKind != "" {
		conversation.Provider = providerKind
	}
	return r.primaryConversationModel(ctx, *conversation)
}

func (r *Resolver) conversationTranscriptPage(
	ctx context.Context,
	input model.ConversationTranscriptPageInput,
) (*model.ConversationTranscriptPage, error) {
	limit := 80
	if input.Limit != nil {
		limit = *input.Limit
	}
	if limit < 1 || limit > 200 {
		return nil, errors.New("conversationTranscriptPage limit must be within 1..200")
	}
	cursor := ""
	if input.Cursor != nil {
		cursor = *input.Cursor
	}
	page, err := r.Store.ConversationItemPage(ctx, input.ConversationID, cursor, limit)
	if err != nil {
		return nil, err
	}
	return conversationTranscriptPageModel(page)
}

func (r *Resolver) sendConversationTurn(
	ctx context.Context,
	input model.SendConversationTurnInput,
) (*model.TurnAccepted, error) {
	if r.Chat == nil {
		return nil, errors.New("Chat runtime is unavailable")
	}
	accepted, err := r.Chat.SendTurn(ctx, runtime.SendTurnInput{
		ConversationID: input.ConversationID, Input: input.Input,
		ClientMessageID: input.ClientMessageID, ClientTimeZone: input.ClientTimeZone,
	})
	if err != nil {
		return nil, err
	}
	return &model.TurnAccepted{
		ConversationID: accepted.ConversationID, ClientMessageID: accepted.ClientMessageID,
	}, nil
}

func (r *Resolver) sendMultipleChoiceSelection(
	ctx context.Context, input model.SendMultipleChoiceSelectionInput,
) (*model.TurnAccepted, error) {
	if r.Chat == nil {
		return nil, errors.New("Chat runtime is unavailable")
	}
	accepted, err := r.Chat.SendMultipleChoiceSelection(
		ctx, input.ConversationID, input.PromptItemID, input.SelectedOptionIds, input.ClientMessageID,
	)
	if err != nil {
		return nil, err
	}
	return &model.TurnAccepted{ConversationID: accepted.ConversationID, ClientMessageID: accepted.ClientMessageID}, nil
}

func (r *Resolver) conversationEvents(
	ctx context.Context,
	conversationID string,
) (<-chan model.ConversationEvent, error) {
	if r.Chat == nil {
		return nil, errors.New("Chat runtime is unavailable")
	}
	runtimeEvents, err := r.Chat.Subscribe(ctx, conversationID)
	if err != nil {
		return nil, err
	}
	events := make(chan model.ConversationEvent, 16)
	go func() {
		defer close(events)
		for {
			select {
			case <-ctx.Done():
				return
			case event, ok := <-runtimeEvents:
				if !ok {
					return
				}
				mapped, mapErr := conversationEventModel(event)
				if mapErr != nil {
					return
				}
				select {
				case events <- mapped:
				case <-ctx.Done():
					return
				}
			}
		}
	}()
	return events, nil
}

func conversationEventModel(event runtime.Event) (model.ConversationEvent, error) {
	switch event.Kind {
	case runtime.EventSubscriptionReady:
		return model.SubscriptionReadyEvent{ConversationID: event.ConversationID}, nil
	case runtime.EventHumanInterventionsChanged:
		return model.HumanInterventionsChangedEvent{ConversationID: event.ConversationID}, nil
	case runtime.EventAgentStatus:
		status, err := agentStatusModel(event.Status)
		if err != nil {
			return nil, err
		}
		return model.AgentStatusEvent{ConversationID: event.ConversationID, Status: status}, nil
	case runtime.EventAssistantDelta:
		return model.AssistantTextDeltaEvent{
			ConversationID: event.ConversationID, TurnID: event.TurnID,
			StreamID: event.StreamID, ResponseIndex: event.ResponseIndex, Delta: event.Delta,
		}, nil
	case runtime.EventConversationItem:
		if event.Item == nil {
			return nil, errors.New("Chat item event is missing its item")
		}
		item, err := transcriptItemModel(*event.Item)
		if err != nil {
			return nil, err
		}
		cursor := event.Item.Cursor
		turnID := chatOptionalString(event.Item.TurnID)
		return model.ConversationItemEvent{
			ConversationID: event.ConversationID, ClientMessageID: event.ClientMessageID,
			ItemID: event.Item.ID, Cursor: &cursor, TurnID: turnID,
			Metadata: event.Item.Metadata, Item: item,
		}, nil
	case runtime.EventTurnCompleted:
		return model.TurnCompletedEvent{
			ConversationID: event.ConversationID, ClientMessageID: event.ClientMessageID,
		}, nil
	case runtime.EventTransientError:
		correlation := "uncorrelated"
		if event.ClientMessageID != nil && *event.ClientMessageID != "" {
			correlation = *event.ClientMessageID
		}
		return model.ConversationItemEvent{
			ConversationID: event.ConversationID, ClientMessageID: event.ClientMessageID,
			ItemID:   "graphql_runtime_error:" + event.ConversationID + ":" + correlation,
			Metadata: map[string]any{},
			Item:     model.ErrorNotice{Message: event.TransientMessage, Recoverable: false},
		}, nil
	default:
		return nil, errors.New("Chat runtime event is unsupported")
	}
}

func agentStatusModel(status runtime.AgentStatus) (model.AgentStatus, error) {
	switch status {
	case runtime.AgentStatusIdle:
		return model.AgentStatusIdle, nil
	case runtime.AgentStatusInputReceived:
		return model.AgentStatusInputReceived, nil
	case runtime.AgentStatusThinking:
		return model.AgentStatusThinking, nil
	case runtime.AgentStatusError:
		return model.AgentStatusError, nil
	default:
		return "", errors.New("Chat agent status is unsupported")
	}
}

func conversationTranscriptPageModel(
	page store.ConversationItemPage,
) (*model.ConversationTranscriptPage, error) {
	items := make([]*model.ConversationItem, 0, len(page.Items))
	for _, stored := range page.Items {
		item, err := transcriptItemModel(stored)
		if err != nil {
			return nil, err
		}
		items = append(items, &model.ConversationItem{
			ItemID: stored.ID, Cursor: stored.Cursor, TurnID: chatOptionalString(stored.TurnID),
			Metadata: stored.Metadata, Item: item,
		})
	}
	return &model.ConversationTranscriptPage{
		Items: items,
		PageInfo: &model.ConversationTranscriptPageInfo{
			BeforeCursor: chatOptionalString(page.BeforeCursor), HasMoreBefore: page.HasMoreBefore,
		},
	}, nil
}

func transcriptItemModel(item store.ConversationItem) (model.TranscriptItem, error) {
	switch item.Kind {
	case store.ConversationUserText:
		return model.UserText{Text: item.ContentText}, nil
	case store.ConversationAssistantText:
		return model.AssistantText{Text: item.ContentText}, nil
	case store.ConversationMultipleChoicePrompt:
		options, err := multipleChoiceOptionsModel(item.Payload["options"])
		if err != nil {
			return nil, err
		}
		mode, err := multipleChoiceModeModel(item.Payload["selection_mode"])
		if err != nil {
			return nil, err
		}
		prompt, _ := item.Payload["prompt"].(string)
		if prompt == "" {
			return nil, errors.New("stored multiple-choice prompt is invalid")
		}
		return model.MultipleChoicePrompt{Prompt: prompt, SelectionMode: mode, Options: options}, nil
	case store.ConversationMultipleChoiceSelection:
		options, err := multipleChoiceOptionsModel(item.Payload["selected_options"])
		if err != nil {
			return nil, err
		}
		mode, err := multipleChoiceModeModel(item.Payload["selection_mode"])
		if err != nil {
			return nil, err
		}
		promptID, _ := item.Payload["prompt_item_id"].(string)
		if promptID == "" {
			return nil, errors.New("stored multiple-choice selection is invalid")
		}
		return model.MultipleChoiceSelection{PromptItemID: promptID, SelectionMode: mode, SelectedOptions: options}, nil
	case store.ConversationToolCall, store.ConversationToolResult, store.ConversationApprovalRequest:
		id, _ := item.Payload["id"].(string)
		kind, _ := item.Payload["activity_kind"].(string)
		title, _ := item.Payload["title"].(string)
		summary, _ := item.Payload["summary"].(string)
		metadata, _ := item.Payload["metadata"].(map[string]any)
		if id == "" || kind == "" || title == "" || metadata == nil {
			return nil, errors.New("stored conversation activity is invalid")
		}
		status, err := activityStatusModel(item.Status)
		if err != nil {
			return nil, err
		}
		return model.Activity{
			ID: id, ActivityKind: kind, Status: status, Title: title,
			Summary: chatOptionalString(summary), Metadata: metadata,
		}, nil
	case store.ConversationErrorNotice:
		message := item.ContentText
		if stored, ok := item.Payload["message"].(string); ok {
			message = stored
		}
		recoverable, _ := item.Payload["recoverable"].(bool)
		return model.ErrorNotice{Message: message, Recoverable: recoverable}, nil
	default:
		return nil, fmt.Errorf("conversation item kind %q is unsupported", item.Kind)
	}
}

func multipleChoiceModeModel(value any) (model.MultipleChoiceSelectionMode, error) {
	switch value {
	case "pick_one":
		return model.MultipleChoiceSelectionModePickOne, nil
	case "pick_many":
		return model.MultipleChoiceSelectionModePickMany, nil
	default:
		return "", errors.New("stored multiple-choice mode is invalid")
	}
}

func multipleChoiceOptionsModel(value any) ([]*model.MultipleChoiceOption, error) {
	var values []any
	switch typed := value.(type) {
	case []any:
		values = typed
	case []map[string]any:
		values = make([]any, len(typed))
		for index := range typed {
			values[index] = typed[index]
		}
	}
	if len(values) == 0 {
		return nil, errors.New("stored multiple-choice options are invalid")
	}
	options := make([]*model.MultipleChoiceOption, 0, len(values))
	for _, value := range values {
		entry, ok := value.(map[string]any)
		if !ok {
			return nil, errors.New("stored multiple-choice option is invalid")
		}
		id, _ := entry["id"].(string)
		label, _ := entry["label"].(string)
		if id == "" || label == "" {
			return nil, errors.New("stored multiple-choice option is invalid")
		}
		options = append(options, &model.MultipleChoiceOption{ID: id, Label: label})
	}
	return options, nil
}

func activityStatusModel(status string) (model.TurnActivityStatus, error) {
	switch status {
	case "pending", "running":
		return model.TurnActivityStatusStarted, nil
	case "completed":
		return model.TurnActivityStatusCompleted, nil
	case "failed", "cancelled", "interrupted":
		return model.TurnActivityStatusFailed, nil
	default:
		return "", errors.New("stored conversation activity status is invalid")
	}
}

func chatOptionalString(value string) *string {
	if value == "" {
		return nil
	}
	copy := value
	return &copy
}

func (r *Resolver) primaryConversationModel(
	ctx context.Context,
	conversation store.Conversation,
) (*model.PrimaryConversation, error) {
	page, err := r.Store.ConversationItemPage(ctx, conversation.ID, "", 80)
	if err != nil {
		return nil, err
	}
	transcript, err := conversationTranscriptPageModel(page)
	if err != nil {
		return nil, err
	}
	return &model.PrimaryConversation{
		ConversationID: conversation.ID, Provider: conversation.Provider,
		LatestTranscriptPage: transcript,
	}, nil
}
