package runtime

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"reflect"
	"strconv"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

type a2uiResolution struct {
	ctx                                                               context.Context
	conversationID, interactionID, surfaceID, sourceComponentID, name string
	expectedRevision                                                  int
	context                                                           map[string]any
	dataModel                                                         any
	clientMessageID                                                   *string
	reply                                                             chan error
}

// SendA2UIAction validates and queues one exact A2UI action.
func (c *Chat) SendA2UIAction(ctx context.Context, conversationID, interactionID string,
	expectedRevision int, surfaceID, sourceComponentID, name string,
	actionContext map[string]any, dataModel any, clientMessageID *string,
) (TurnAccepted, error) {
	if _, err := c.database.Conversation(ctx, conversationID); err != nil {
		return TurnAccepted{}, err
	}
	request := a2uiResolution{ctx: ctx, conversationID: conversationID, interactionID: interactionID,
		expectedRevision: expectedRevision, surfaceID: surfaceID, sourceComponentID: sourceComponentID,
		name: name, context: cloneA2UIJSON(actionContext), dataModel: cloneA2UIJSON(dataModel),
		clientMessageID: cloneOptionalString(clientMessageID), reply: make(chan error, 1)}
	c.stateMu.RLock()
	if c.closed {
		c.stateMu.RUnlock()
		return TurnAccepted{}, ErrChatClosed
	}
	select {
	case <-ctx.Done():
		c.stateMu.RUnlock()
		return TurnAccepted{}, ctx.Err()
	case <-c.ctx.Done():
		c.stateMu.RUnlock()
		return TurnAccepted{}, ErrChatClosed
	case c.a2ui <- request:
		c.stateMu.RUnlock()
	default:
		c.stateMu.RUnlock()
		return TurnAccepted{}, ErrChatQueueFull
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
func cloneA2UIJSON[T any](value T) T {
	encoded, _ := json.Marshal(value)
	var cloned T
	_ = json.Unmarshal(encoded, &cloned)
	return cloned
}
func (c *Chat) resolveA2UI(request a2uiResolution) {
	if err := request.ctx.Err(); err != nil {
		request.reply <- err
		return
	}
	source, err := c.database.ConversationA2UIInteraction(c.ctx, request.conversationID, request.interactionID)
	if err != nil {
		request.reply <- err
		return
	}
	projection, _ := source.Payload["payload"].(map[string]any)
	if textValue(source.Payload["interaction_state"]) != "pending" ||
		intJSONValue(projection["interaction_revision"]) != request.expectedRevision {
		request.reply <- errors.New("A2UI interaction is stale")
		return
	}
	if !boundedA2UIJSON(request.context) || !boundedA2UIJSON(request.dataModel) {
		request.reply <- errors.New("A2UI action data is too large")
		return
	}
	surfaces, _ := projection["surfaces"].(map[string]any)
	var surfaceKey string
	var surface a2uiSurface
	for key, raw := range surfaces {
		encoded, _ := json.Marshal(raw)
		var candidate a2uiSurface
		if json.Unmarshal(encoded, &candidate) == nil && candidate.SurfaceID == request.surfaceID {
			surfaceKey, surface = key, candidate
			break
		}
	}
	if surfaceKey == "" {
		request.reply <- errors.New("A2UI surface is stale")
		return
	}
	var declared *a2uiAction
	for index := range surface.Actions {
		if surface.Actions[index].SourceComponentID == request.sourceComponentID && surface.Actions[index].Name == request.name {
			declared = &surface.Actions[index]
			break
		}
	}
	if declared == nil {
		request.reply <- errors.New("A2UI action is not available")
		return
	}
	if surface.SendDataModel != (request.dataModel != nil) {
		request.reply <- errors.New("A2UI synchronized data model does not match the surface")
		return
	}
	if request.dataModel != nil {
		surface.DataModel = request.dataModel
	}
	resolvedContext, ok := resolveA2UIActionContext(declared.Context, surface.DataModel)
	if !ok {
		request.reply <- errors.New("stored A2UI action context is invalid")
		return
	}
	if !reflect.DeepEqual(request.context, resolvedContext) {
		request.reply <- errors.New("A2UI action context does not match the declared action")
		return
	}
	surface.Revision++
	encodedSurface, _ := json.Marshal(surface)
	var storedSurface map[string]any
	_ = json.Unmarshal(encodedSurface, &storedSurface)
	settled := cloneA2UIJSON(projection)
	settledSurfaces, _ := settled["surfaces"].(map[string]any)
	settledSurfaces[surfaceKey] = storedSurface
	settled["interaction_revision"] = request.expectedRevision + 1
	settled["lifecycle"] = "answered"
	resultPayload := map[string]any{"status": "resolved", "interaction_id": request.interactionID,
		"surface_id": request.surfaceID, "source_component_id": request.sourceComponentID,
		"action_name": request.name, "context": resolvedContext}
	if request.dataModel != nil {
		resultPayload["data_model"] = request.dataModel
	}
	continuation, err := c.database.ResolveConversationA2UI(c.ctx, request.conversationID, source.ID,
		request.interactionID, request.expectedRevision, settled, resultPayload, request.clientMessageID, time.Now())
	if err != nil {
		request.reply <- err
		return
	}
	for _, item := range []store.ConversationItem{continuation.Call, continuation.Action, continuation.Result} {
		item := item
		c.publish(Event{Kind: EventConversationItem, ConversationID: request.conversationID,
			ClientMessageID: request.clientMessageID, TurnID: continuation.Turn.ID, Item: &item})
	}
	request.reply <- nil
	c.resumeA2UI(continuation)
}
func boundedA2UIJSON(value any) bool {
	if value == nil {
		return true
	}
	encoded, err := json.Marshal(value)
	return err == nil && len(encoded) <= a2uiActionJSONLimit
}

func resolveA2UIActionContext(value, dataModel any) (map[string]any, bool) {
	if value == nil {
		return nil, true
	}
	object, ok := value.(map[string]any)
	if !ok {
		return nil, false
	}
	result := make(map[string]any, len(object))
	for key, child := range object {
		result[key] = resolveA2UIBindings(child, dataModel)
	}
	return result, true
}

func resolveA2UIBindings(value, dataModel any) any {
	object, ok := value.(map[string]any)
	if ok && len(object) == 1 {
		if path, bound := object["path"].(string); bound {
			parts, failure := parseA2UIPointer(path, 0)
			if failure != nil {
				return nil
			}
			current := dataModel
			for _, part := range parts {
				switch next := current.(type) {
				case map[string]any:
					current, ok = next[part]
				case []any:
					index, err := strconv.Atoi(part)
					ok = err == nil && index >= 0 && strconv.Itoa(index) == part && index < len(next)
					if ok {
						current = next[index]
					}
				default:
					ok = false
				}
				if !ok {
					return nil
				}
			}
			return current
		}
	}
	if ok {
		result := make(map[string]any, len(object))
		for key, child := range object {
			result[key] = resolveA2UIBindings(child, dataModel)
		}
		return result
	}
	if values, ok := value.([]any); ok {
		result := make([]any, len(values))
		for index, child := range values {
			result[index] = resolveA2UIBindings(child, dataModel)
		}
		return result
	}
	return value
}

func (c *Chat) resumeA2UI(value store.ConversationA2UIContinuation) {
	claimed, err := c.database.ClaimConversationA2UI(c.ctx, value.Surface.ID, time.Now())
	if err != nil {
		return
	}
	assignment, err := storedModelAssignment(claimed.Surface.Payload)
	if err != nil {
		c.failA2UI(claimed, err)
		return
	}
	credentialRevision, err := c.validateA2UIAuthority(c.ctx, claimed.Surface.Payload, assignment)
	if err != nil {
		c.failA2UI(claimed, err)
		return
	}
	generator, err := c.generatorFor(assignment.ProviderKind)
	if err != nil {
		c.failA2UI(claimed, err)
		return
	}
	generator, closeSession, _ := openGenerationSession(generator)
	defer closeSession()
	conversation, err := c.database.Conversation(c.ctx, claimed.Turn.ConversationID)
	if err != nil {
		c.failA2UI(claimed, err)
		return
	}
	clientID, _ := claimed.Action.Metadata["client_message_id"].(string)
	request := queuedTurn{input: SendTurnInput{ConversationID: conversation.ID}, conversation: conversation, location: time.UTC}
	if clientID != "" {
		request.input.ClientMessageID = &clientID
	}
	replay, err := storedA2UIToolResult(claimed.Result)
	if err != nil {
		c.failA2UI(claimed, err)
		return
	}
	call, err := storedA2UIToolCall(c.database, c.ctx, claimed.Surface)
	if err != nil {
		c.failA2UI(claimed, err)
		return
	}
	replay.Arguments = call.Arguments
	providerRound := intJSONValue(claimed.Surface.Payload["provider_round"])
	responseID, _ := claimed.Surface.Payload["response_id"].(string)
	hostedState, _ := claimed.Surface.Payload["hosted_state"].(bool)
	c.publish(Event{Kind: EventAgentStatus, ConversationID: conversation.ID, Status: AgentStatusThinking})
	result, _, err := c.generateChatToolContinuation(request, claimed.Turn, assignment, generator,
		providerRound+1, "", c.memoryRootContext(), responseID, hostedState,
		provider.GenerationMessage{Role: "tool", ToolResult: &replay}, &credentialRevision)
	if err != nil {
		if c.ctx.Err() != nil {
			_ = c.database.ReleaseConversationA2UI(context.Background(), claimed.Surface.ID, time.Now())
			return
		}
		c.failA2UI(claimed, err)
		return
	}
	c.executeChatToolRounds(request, claimed.Turn, assignment, generator, result, c.memoryRootContext(), providerRound+1, hostedState)
}

func (c *Chat) failA2UI(value store.ConversationA2UIContinuation, err error) {
	c.failTurn(SendTurnInput{ConversationID: value.Turn.ConversationID}, value.Turn, err)
}

func (c *Chat) a2uiAuthority(
	ctx context.Context, assignment store.ModelAssignment,
) (uint64, string, error) {
	account, err := c.database.ProviderAccount(ctx, assignment.ProviderAccountID)
	if err != nil || account.ProviderKind != assignment.ProviderKind {
		return 0, "", errors.New("A2UI provider route is unavailable")
	}
	tools, err := c.chatTools(ctx)
	if err != nil {
		return 0, "", err
	}
	encoded, err := json.Marshal(tools)
	if err != nil {
		return 0, "", errors.New("A2UI tool catalog is invalid")
	}
	digest := sha256.Sum256(encoded)
	return account.Metadata.CredentialRevision(), hex.EncodeToString(digest[:]), nil
}

func (c *Chat) validateA2UIAuthority(
	ctx context.Context, payload map[string]any, assignment store.ModelAssignment,
) (uint64, error) {
	revision, digest, err := c.a2uiAuthority(ctx, assignment)
	if err != nil {
		return 0, err
	}
	storedRevision, ok := payload["credential_revision"].(float64)
	storedDigest, _ := payload["tool_catalog_digest"].(string)
	if !ok || storedRevision < 0 || uint64(storedRevision) != revision || storedDigest == "" || storedDigest != digest {
		return 0, errors.New("A2UI provider authority changed")
	}
	return revision, nil
}

func storedA2UIToolResult(item store.ConversationItem) (provider.ReplayToolResult, error) {
	result, err := storedToolResult(item)
	if err != nil {
		return provider.ReplayToolResult{}, err
	}
	action, _ := nestedAction(item.Payload)
	result.Payload, err = json.Marshal(action["payload"])
	if err != nil {
		return provider.ReplayToolResult{}, errors.New("A2UI tool result is invalid")
	}
	return result, nil
}

func storedA2UIToolCall(database *store.Store, ctx context.Context, prompt store.ConversationItem) (provider.ReplayToolCall, error) {
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
	return provider.ReplayToolCall{}, errors.New("A2UI tool call is unavailable")
}

func intJSONValue(value any) int { number, _ := value.(float64); return int(number) }
