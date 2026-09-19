package graphql

import (
	"context"
	"errors"
	"fmt"
	"maps"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/toolmarker"
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
		if errors.Is(err, store.ErrConversationNotFound) {
			return nil, errors.New("conversation is unavailable")
		}
		return nil, err
	}
	return r.conversationTranscriptPageModel(ctx, page)
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
		if errors.Is(err, store.ErrConversationNotFound) {
			return nil, errors.New("conversation is unavailable")
		}
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
	if _, err := r.Store.Conversation(ctx, input.ConversationID); err != nil {
		if errors.Is(err, store.ErrConversationNotFound) {
			return nil, errors.New("conversation is unavailable")
		}
		return nil, err
	}
	accepted, err := r.Chat.SendMultipleChoiceSelection(
		ctx, input.ConversationID, input.PromptItemID, input.SelectedOptionIds, input.ClientMessageID,
	)
	if err != nil {
		if errors.Is(err, store.ErrConversationNotFound) {
			return nil, errors.New("conversation is unavailable")
		}
		return nil, err
	}
	return &model.TurnAccepted{ConversationID: accepted.ConversationID, ClientMessageID: accepted.ClientMessageID}, nil
}

func (r *Resolver) sendA2UIAction(
	ctx context.Context, input model.ProviderInteractionActionInput,
) (*model.TurnAccepted, error) {
	if r.Chat == nil {
		return nil, errors.New("Chat runtime is unavailable")
	}
	accepted, err := r.Chat.SendA2UIAction(ctx, input.ConversationID, input.InteractionID,
		input.ExpectedRevision, input.SurfaceID, input.SourceComponentID, input.ActionName,
		input.Context, input.DataModel.Value, input.ClientMessageID)
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
		if errors.Is(err, store.ErrConversationNotFound) {
			return nil, errors.New("conversation is unavailable")
		}
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
				mapped, mapErr := r.conversationEventModel(ctx, event)
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

func (r *Resolver) conversationEventModel(ctx context.Context, event runtime.Event) (model.ConversationEvent, error) {
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
		item, err := r.transcriptItemModel(ctx, *event.Item)
		if err != nil {
			return nil, err
		}
		var cursor *string
		if event.Item.Cursor != "" {
			value := event.Item.Cursor
			cursor = &value
		}
		turnID := chatOptionalString(event.Item.TurnID)
		return model.ConversationItemEvent{
			ConversationID: event.ConversationID, ClientMessageID: event.ClientMessageID,
			ItemID: event.Item.ID, Cursor: cursor, TurnID: turnID,
			Metadata: transcriptMetadata(*event.Item), Item: item,
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

func (r *Resolver) conversationTranscriptPageModel(
	ctx context.Context, page store.ConversationItemPage,
) (*model.ConversationTranscriptPage, error) {
	items := make([]*model.ConversationItem, 0, len(page.Items))
	for _, stored := range page.Items {
		item, err := r.transcriptItemModel(ctx, stored)
		if err != nil {
			return nil, err
		}
		if item == nil {
			continue
		}
		items = append(items, &model.ConversationItem{
			ItemID: stored.ID, Cursor: stored.Cursor, TurnID: chatOptionalString(stored.TurnID),
			Metadata: transcriptMetadata(stored), Item: item,
		})
	}
	return &model.ConversationTranscriptPage{
		Items: items,
		PageInfo: &model.ConversationTranscriptPageInfo{
			BeforeCursor: chatOptionalString(page.BeforeCursor), HasMoreBefore: page.HasMoreBefore,
		},
	}, nil
}

func (r *Resolver) transcriptItemModel(ctx context.Context, item store.ConversationItem) (model.TranscriptItem, error) {
	value, err := transcriptItemModel(item)
	reference, ok := value.(model.TaskReference)
	if err != nil || !ok {
		return value, err
	}
	task, taskErr := r.Store.Task(ctx, reference.TaskID)
	document, documentErr := home.ReadTaskDocument(r.home, reference.TaskID)
	if taskErr == nil && documentErr == nil {
		reference.Task = r.taskSummaryModel(ctx, task, personalWorkspaceID, document.Content)
		value = reference
	}
	return value, nil
}

func transcriptItemModel(item store.ConversationItem) (model.TranscriptItem, error) {
	switch item.Kind {
	case store.ConversationUserText:
		return model.UserText{Text: item.ContentText}, nil
	case store.ConversationAssistantText:
		if transcriptMetadata(item)["presentation"] == "marker" {
			return nil, nil
		}
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
	case store.ConversationA2UICard:
		projection, _ := item.Payload["payload"].(map[string]any)
		surfaces, _ := projection["surfaces"].(map[string]any)
		if len(surfaces) != 1 {
			return nil, errors.New("stored A2UI projection is invalid")
		}
		var snapshot map[string]any
		for _, value := range surfaces {
			snapshot, _ = value.(map[string]any)
		}
		catalog, _ := projection["catalog"].(map[string]any)
		actions, _ := snapshot["actions"].([]any)
		id, _ := item.Payload["id"].(string)
		surfaceID, _ := snapshot["surface_id"].(string)
		version, _ := snapshot["version"].(string)
		lifecycle, _ := projection["lifecycle"].(string)
		if state, _ := item.Payload["interaction_state"].(string); state == "completed" || state == "failed" {
			lifecycle = state
		}
		if lifecycle == "" {
			lifecycle = "completed"
		}
		if id == "" || surfaceID == "" || version != "v0.9.1" || catalog == nil || snapshot == nil {
			return nil, errors.New("stored A2UI surface is invalid")
		}
		var interactionID *string
		if value, ok := projection["interaction_id"].(string); ok && value != "" {
			interactionID = &value
		}
		var interactionRevision *int
		if value, ok := projection["interaction_revision"].(float64); ok {
			revision := int(value)
			interactionRevision = &revision
		}
		return model.A2UISurface{ID: id, InteractionID: interactionID, SurfaceID: surfaceID,
			Version: version, Revision: intJSONModel(snapshot["revision"]), InteractionRevision: interactionRevision,
			Lifecycle: lifecycle, Catalog: catalog, Snapshot: snapshot, HasActions: len(actions) != 0}, nil
	case store.ConversationActivity, store.ConversationToolCall, store.ConversationToolResult, store.ConversationApprovalRequest:
		if (item.Kind == store.ConversationToolCall || item.Kind == store.ConversationToolResult) && hiddenToolActivity(item.Payload) {
			return nil, nil
		}
		id, _ := item.Payload["id"].(string)
		kind, _ := item.Payload["activity_kind"].(string)
		title, _ := item.Payload["title"].(string)
		summary, _ := item.Payload["summary"].(string)
		metadata, _ := item.Payload["metadata"].(map[string]any)
		if metadata == nil {
			metadata = item.Metadata
		}
		if id == "" || kind == "" || title == "" || metadata == nil {
			return nil, errors.New("invalid replay payload")
		}
		if kind == "tool_call" || kind == "tool_result" {
			metadata = maps.Clone(metadata)
			action, _ := metadata["action"].(map[string]any)
			id = item.ID
			action = maps.Clone(action)
			if action == nil {
				action = map[string]any{}
			}
			if kind == "tool_call" {
				action["id"] = item.ID
			} else {
				action["call_id"] = item.ParentItemID
			}
			metadata["action"] = action
			if name, _ := action["name"].(string); name != "" {
				display, _ := metadata["display"].(map[string]any)
				display = maps.Clone(display)
				if display == nil {
					display = map[string]any{}
				}
				// Earlier records copied assistant commentary into the tool label.
				delete(display, "description")
				display["name"] = toolmarker.ReadableName(name)
				metadata["display"] = display
			}
			if marker, ok := toolmarker.ForAction(kind, item.Status, action); ok {
				display, _ := metadata["display"].(map[string]any)
				if display == nil {
					display = map[string]any{}
					metadata["display"] = display
				}
				display["marker"] = marker
			}
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
	case store.ConversationTaskReference:
		id, _ := item.Payload["task_id"].(string)
		if id == "" {
			return nil, errors.New("stored Task reference is invalid")
		}
		return model.TaskReference{TaskID: id}, nil
	case store.ConversationArtifactReference:
		artifactID, _ := item.Payload["artifact_id"].(string)
		versionID, _ := item.Payload["artifact_version_id"].(string)
		title, _ := item.Payload["title"].(string)
		kind, _ := item.Payload["artifact_kind"].(string)
		storage, _ := item.Payload["storage_kind"].(string)
		if artifactID == "" || versionID == "" || title == "" || kind == "" || storage == "" {
			return nil, errors.New("stored Artifact reference is invalid")
		}
		return model.ArtifactReference{ArtifactID: artifactID, ArtifactVersionID: &versionID,
			Title: title, ArtifactKind: kind, StorageKind: storage,
			ExternalURL: chatOptionalString(chatPayloadString(item.Payload, "external_url")),
			DownloadURL: chatOptionalString(chatPayloadString(item.Payload, "download_url")),
			MediaType:   chatOptionalString(chatPayloadString(item.Payload, "media_type"))}, nil
	default:
		return nil, fmt.Errorf("conversation item kind %q is unsupported", item.Kind)
	}
}

func hiddenToolActivity(payload map[string]any) bool {
	metadata, _ := payload["metadata"].(map[string]any)
	if metadata == nil {
		return false
	}
	action, _ := metadata["action"].(map[string]any)
	name, _ := action["name"].(string)
	switch name {
	case "enable.calendar.move_event", "task.delegate", "web.browse.close":
		return true
	default:
		return false
	}
}

func intJSONModel(value any) int { number, _ := value.(float64); return int(number) }

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

func chatPayloadString(payload map[string]any, key string) string {
	value, _ := payload[key].(string)
	return value
}

func (r *Resolver) primaryConversationModel(
	ctx context.Context,
	conversation store.Conversation,
) (*model.PrimaryConversation, error) {
	page, err := r.Store.ConversationItemPage(ctx, conversation.ID, "", 80)
	if err != nil {
		return nil, err
	}
	transcript, err := r.conversationTranscriptPageModel(ctx, page)
	if err != nil {
		return nil, err
	}
	return &model.PrimaryConversation{
		ConversationID: conversation.ID, Provider: conversation.Provider,
		LatestTranscriptPage: transcript,
	}, nil
}

// Transcript presentation is derived from provider facts for both replay and live reads.
func assistantPresentation(kind string, summary bool) string {
	if kind == "reasoning" && summary {
		return "marker"
	}
	return "bubble"
}

func transcriptMetadata(item store.ConversationItem) map[string]any {
	if item.Kind != store.ConversationAssistantText {
		return item.Metadata
	}
	metadata := make(map[string]any, len(item.Metadata)+1)
	for key, value := range item.Metadata {
		metadata[key] = value
	}
	kind, _ := metadata["provider_output_kind"].(string)
	if kind == "" {
		kind = "message"
	}
	summary, explicit := metadata["reasoning_summary"].(bool)
	if !explicit && metadata["provider"] == "codex" {
		// Saved Codex sections below 4096 are summary sections; content starts at 4096.
		section, ok := metadata["section_index"].(float64)
		summary = ok && section >= 0 && section < provider.ReasoningContentSectionOffset
	}
	metadata["presentation"] = assistantPresentation(kind, summary)
	return metadata
}
