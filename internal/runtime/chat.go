// Package runtime owns active Go server execution.
package runtime

import (
	"context"
	"errors"
	"fmt"
	"os"
	"strconv"
	"sync"
	"time"
	_ "time/tzdata"

	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/project"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	turnQueueLimit       = 64
	subscriberQueueLimit = 128
	shutdownSaveTimeout  = 5 * time.Second
	memoryEventChannel   = "\x00memory"
)

var (
	// ErrChatClosed means the Chat runtime no longer accepts turns.
	ErrChatClosed = errors.New("Chat runtime is closed")
	// ErrChatQueueFull means the bounded turn queue cannot accept more input.
	ErrChatQueueFull = errors.New("Chat turn queue is full")
)

// EventKind identifies one live Chat event.
type EventKind string

const (
	EventSubscriptionReady EventKind = "subscription_ready"
	EventAgentStatus       EventKind = "agent_status"
	EventConversationItem  EventKind = "conversation_item"
	EventAssistantDelta    EventKind = "assistant_text_delta"
	EventTurnCompleted     EventKind = "turn_completed"
	EventTransientError    EventKind = "transient_error"
	EventMemoryChanged     EventKind = "memory_changed"
)

const EventHumanInterventionsChanged EventKind = "human_interventions_changed"

// AgentStatus is one live Chat status.
type AgentStatus string

const (
	AgentStatusIdle          AgentStatus = "IDLE"
	AgentStatusInputReceived AgentStatus = "INPUT_RECEIVED"
	AgentStatusThinking      AgentStatus = "THINKING"
	AgentStatusError         AgentStatus = "ERROR"
)

// Event is one runtime-neutral Chat subscription event.
type Event struct {
	Kind             EventKind
	ConversationID   string
	ClientMessageID  *string
	TurnID           string
	StreamID         string
	ResponseIndex    int
	Delta            string
	Status           AgentStatus
	Item             *store.ConversationItem
	TransientMessage string
}

// SendTurnInput contains one accepted human turn.
type SendTurnInput struct {
	ConversationID  string
	Input           string
	ClientMessageID *string
	ClientTimeZone  *string
}

// TurnAccepted confirms that the runtime queued one turn.
type TurnAccepted struct {
	ConversationID  string
	ClientMessageID *string
}

type queuedTurn struct {
	input        SendTurnInput
	conversation store.Conversation
	location     *time.Location
}

type actionResolution struct {
	ctx                         context.Context
	actionID, humanID, decision string
	revision                    int
	reply                       chan actionResolutionResult
}

type actionResolutionResult struct {
	action store.ActionRequest
	err    error
}

type actionContinuation struct {
	action  store.ActionRequest
	trigger store.ConversationItem
}

type mcpAuthResolution struct {
	attemptID, requestID string
	revision             int
	skip                 bool
	reply                chan mcpAuthResult
}
type mcpAuthResult struct {
	request store.MCPAuthRequest
	err     error
}

type subscriber struct {
	conversationID string
	events         chan Event
}

// Chat serializes text turns and publishes their live events.
type Chat struct {
	ctx        context.Context
	cancel     context.CancelFunc
	database   *store.Store
	openRouter provider.Generator
	codex      provider.Generator
	openAI     provider.Generator
	home       *os.Root
	memory     *noemamemory.Store
	mcp        *noemamcp.Service
	projects   *project.Service
	turns      chan queuedTurn
	actions    chan actionResolution
	mcpAuth    chan mcpAuthResolution
	choices    chan choiceResolution
	done       chan struct{}
	closeOnce  sync.Once
	closeErr   error
	stateMu    sync.RWMutex
	closed     bool
	memoryMu   sync.Mutex
	memoryRun  bool
	memoryErr  string
	memoryWG   sync.WaitGroup

	recoveredActions []actionContinuation
	recoveredChoices []store.ConversationChoiceContinuation

	subMu       sync.Mutex
	subscribers map[uint64]subscriber
	nextSubID   uint64
}

// NewChat starts one serialized Chat runtime.
func NewChat(
	database *store.Store,
	openRouter provider.Generator,
	codex provider.Generator,
	openAI provider.Generator,
	homeRoot *os.Root,
	memoryStore *noemamemory.Store,
	mcpServices ...*noemamcp.Service,
) (*Chat, error) {
	if database == nil || openRouter == nil || codex == nil || openAI == nil || homeRoot == nil || memoryStore == nil {
		return nil, errors.New("Chat runtime dependencies are unavailable")
	}
	recoveryContext, stopRecovery := context.WithTimeout(context.Background(), shutdownSaveTimeout)
	actions, err := database.RecoverActionRequests(recoveryContext, time.Now())
	recoveredActions := make([]actionContinuation, 0, len(actions))
	var recoveredChoices []store.ConversationChoiceContinuation
	for _, action := range actions {
		turn, input, callErr := database.ActionConversationCall(recoveryContext, action)
		if callErr != nil {
			err = callErr
			break
		}
		input.Payload = mustJSON(actionResultPayload(action))
		input.Success = action.State == store.ActionSucceeded
		item, callErr := database.FinishConversationToolCall(recoveryContext, turn, input, time.Now())
		if callErr != nil {
			err = callErr
			break
		}
		recoveredActions = append(recoveredActions, actionContinuation{action: action, trigger: item})
	}
	if err == nil {
		var choices []store.ConversationChoiceContinuation
		choices, err = database.RecoverConversationChoices(recoveryContext, time.Now())
		if err == nil {
			recoveredChoices = choices
		}
	}
	if err == nil {
		_, err = database.RecoverConversationTurns(recoveryContext, time.Now())
	}
	stopRecovery()
	if err != nil {
		return nil, fmt.Errorf("recover stopped Chat turns: %w", err)
	}
	ctx, cancel := context.WithCancel(context.Background())
	var mcpService *noemamcp.Service
	if len(mcpServices) != 0 {
		mcpService = mcpServices[0]
	}
	chat := &Chat{
		ctx: ctx, cancel: cancel, database: database,
		openRouter: openRouter, codex: codex, openAI: openAI, home: homeRoot, memory: memoryStore, mcp: mcpService,
		projects: project.New(database, homeRoot),
		turns:    make(chan queuedTurn, turnQueueLimit), actions: make(chan actionResolution, turnQueueLimit),
		mcpAuth:          make(chan mcpAuthResolution, turnQueueLimit),
		choices:          make(chan choiceResolution, turnQueueLimit),
		done:             make(chan struct{}),
		recoveredActions: recoveredActions,
		recoveredChoices: recoveredChoices,
		subscribers:      make(map[uint64]subscriber),
	}
	requests, err := database.RecoverConversationMCPAuthRequests(chat.ctx)
	for _, request := range requests {
		payload := toolFailure("outcome_uncertain", "MCP tool outcome is uncertain after restart")
		if err = chat.finishMCPAuthCall(request, payload, false); err != nil {
			break
		}
	}
	if err != nil {
		cancel()
		return nil, fmt.Errorf("recover MCP authentication results: %w", err)
	}
	go chat.run()
	return chat, nil
}

// Projects returns the Project authority used by primary Chat.
func (c *Chat) Projects() *project.Service { return c.projects }

// SendTurn validates and queues one turn without binding execution to the request context.
func (c *Chat) SendTurn(ctx context.Context, input SendTurnInput) (TurnAccepted, error) {
	input.ClientMessageID = cloneOptionalString(input.ClientMessageID)
	input.ClientTimeZone = cloneOptionalString(input.ClientTimeZone)
	conversation, err := c.database.Conversation(ctx, input.ConversationID)
	if err != nil {
		return TurnAccepted{}, err
	}
	location := time.UTC
	if input.ClientTimeZone != nil {
		if *input.ClientTimeZone == "" {
			return TurnAccepted{}, errors.New("clientTimeZone must be a valid IANA timezone")
		}
		location, err = time.LoadLocation(*input.ClientTimeZone)
		if err != nil {
			return TurnAccepted{}, errors.New("clientTimeZone must be a valid IANA timezone")
		}
	}
	request := queuedTurn{input: input, conversation: conversation, location: location}
	c.stateMu.RLock()
	defer c.stateMu.RUnlock()
	if c.closed {
		return TurnAccepted{}, ErrChatClosed
	}
	select {
	case <-ctx.Done():
		return TurnAccepted{}, ctx.Err()
	case c.turns <- request:
		return TurnAccepted{
			ConversationID: input.ConversationID, ClientMessageID: input.ClientMessageID,
		}, nil
	default:
		return TurnAccepted{}, ErrChatQueueFull
	}
}

// Subscribe returns live events after one readiness event.
func (c *Chat) Subscribe(ctx context.Context, conversationID string) (<-chan Event, error) {
	if _, err := c.database.Conversation(ctx, conversationID); err != nil {
		return nil, err
	}
	return c.subscribe(ctx, conversationID, true)
}

// SubscribeAll returns live events for the notification projection.
func (c *Chat) SubscribeAll(ctx context.Context) <-chan Event {
	events, _ := c.subscribe(ctx, "", false)
	return events
}

// SubscribeMemory returns native Memory invalidations until the context ends.
func (c *Chat) SubscribeMemory(ctx context.Context) <-chan Event {
	events, _ := c.subscribe(ctx, memoryEventChannel, false)
	return events
}

// NotifyHumanInterventionsChanged publishes one durable intervention invalidation.
func (c *Chat) NotifyHumanInterventionsChanged(conversationID string) {
	c.publish(Event{Kind: EventHumanInterventionsChanged, ConversationID: conversationID})
}

func (c *Chat) subscribe(ctx context.Context, conversationID string, ready bool) (<-chan Event, error) {
	queueLimit := subscriberQueueLimit
	if conversationID == memoryEventChannel {
		queueLimit = 1
	}
	events := make(chan Event, queueLimit)

	c.subMu.Lock()
	if c.ctx.Err() != nil {
		c.subMu.Unlock()
		close(events)
		return nil, ErrChatClosed
	}
	c.nextSubID++
	id := c.nextSubID
	c.subscribers[id] = subscriber{conversationID: conversationID, events: events}
	if ready {
		events <- Event{Kind: EventSubscriptionReady, ConversationID: conversationID}
	}
	c.subMu.Unlock()

	go func() {
		select {
		case <-ctx.Done():
		case <-c.ctx.Done():
		}
		c.removeSubscriber(id)
	}()
	return events, nil
}

// Close cancels active work, restores its Chat to idle, and closes subscribers.
func (c *Chat) Close() error {
	c.closeOnce.Do(func() {
		c.stateMu.Lock()
		c.closed = true
		c.cancel()
		c.stateMu.Unlock()
		if c.done != nil {
			<-c.done
		}
		c.memoryWG.Wait()
		recoveryContext, stopRecovery := context.WithTimeout(context.Background(), shutdownSaveTimeout)
		_, c.closeErr = c.database.RecoverConversationTurns(recoveryContext, time.Now())
		stopRecovery()
		c.subMu.Lock()
		for id, current := range c.subscribers {
			close(current.events)
			delete(c.subscribers, id)
		}
		c.subMu.Unlock()
	})
	return c.closeErr
}

func (c *Chat) run() {
	if c.done != nil {
		defer close(c.done)
	}
	defer func() {
		for {
			select {
			case request := <-c.turns:
				c.publishTransientFailure(request.input, ErrChatClosed)
			default:
				return
			}
		}
	}()
	for _, recovery := range c.recoveredActions {
		if c.ctx.Err() != nil {
			return
		}
		c.continueAfterAction(recovery.action, recovery.trigger)
	}
	c.recoveredActions = nil
	for _, recovery := range c.recoveredChoices {
		if c.ctx.Err() != nil {
			return
		}
		c.resumeMultipleChoice(recovery)
	}
	c.recoveredChoices = nil
	for {
		if c.ctx.Err() != nil {
			return
		}
		select {
		case <-c.ctx.Done():
			return
		case request := <-c.turns:
			c.execute(request)
		case request := <-c.actions:
			action, err := c.resolveActionRequest(request)
			request.reply <- actionResolutionResult{action: action, err: err}
		case request := <-c.mcpAuth:
			value, err := c.resolveMCPAuthentication(request)
			request.reply <- mcpAuthResult{request: value, err: err}
		case request := <-c.choices:
			c.resolveMultipleChoice(request)
		}
	}
}

func (c *Chat) execute(request queuedTurn) {
	now := time.Now()
	turn, userItem, err := c.database.BeginConversationTurn(
		c.ctx, request.input.ConversationID, request.input.Input,
		request.input.ClientMessageID, now,
	)
	if err != nil {
		c.publishTransientFailure(request.input, err)
		return
	}
	c.publish(Event{
		Kind: EventAgentStatus, ConversationID: turn.ConversationID,
		Status: AgentStatusInputReceived,
	})
	if err := c.database.SetConversationAgentStatus(
		c.ctx, turn.ConversationID, "thinking", time.Now(),
	); err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	c.publish(Event{
		Kind: EventAgentStatus, ConversationID: turn.ConversationID,
		Status: AgentStatusThinking,
	})
	c.publish(Event{
		Kind: EventConversationItem, ConversationID: turn.ConversationID,
		ClientMessageID: request.input.ClientMessageID, TurnID: turn.ID, Item: &userItem,
	})
	c.publishMemoryChanged()

	assignment, err := c.primaryAssignment(c.ctx)
	if err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	generator, err := c.generatorFor(assignment.ProviderKind)
	if err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	messages, err := c.database.ConversationProviderItems(c.ctx, turn.ConversationID)
	if err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	providerMessages, err := providerMessagesFromItems(messages, turn.ID, assignment.ProviderKind)
	if err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	memoryContext := c.memoryRootContext()
	hostedWeb := hostedWebSearchEnabled(assignment.ProviderKind, provider.ToolTransportNative)
	projectContext, err := c.projectContext(c.ctx)
	if err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	environment, err := c.modelEnvironment(c.ctx, request.conversation, request.location, time.Now())
	if err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	providerMessages = append(
		developerMessages(environment, memoryContext, projectContext, hostedWeb),
		providerMessages...,
	)
	streamID := "assistant_stream:" + turn.ID + ":initial:response:0"
	tools, err := c.chatTools(c.ctx)
	if err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	result, err := generator.Generate(c.ctx, provider.GenerateRequest{
		AccountID: assignment.ProviderAccountID, Model: assignment.ModelProfile,
		Messages: providerMessages, ReasoningEffort: string(assignment.ReasoningEffort),
		ConversationID: turn.ConversationID, MaxOutputTokens: maxOutputTokens(),
		Tools:           tools,
		ToolTransport:   provider.ToolTransportNative,
		ToolChoice:      provider.ToolChoiceAuto,
		HostedWebSearch: hostedWeb,
		StoreResponse:   responseIDContinuationProvider(assignment.ProviderKind),
		FastMode:        assignment.FastMode,
	}, func(event provider.StreamEvent) {
		if event.Kind == provider.TextDelta {
			c.publish(Event{
				Kind: EventAssistantDelta, ConversationID: turn.ConversationID,
				TurnID: turn.ID, StreamID: streamID, ResponseIndex: 0, Delta: event.Delta,
			})
		}
	})
	if err != nil {
		c.failTurn(request.input, turn, err)
		return
	}
	c.executeChatToolRounds(request, turn, assignment, generator, result, memoryContext, 0, false)
}

func hostedWebSearchEnabled(providerKind string, transport provider.ToolTransport) bool {
	return transport == provider.ToolTransportNative &&
		(providerKind == "codex" || providerKind == "openai" || providerKind == "openrouter")
}

func responseIDContinuationProvider(providerKind string) bool {
	return providerKind == "codex" || providerKind == "openai"
}

func (c *Chat) generatorFor(providerKind string) (provider.Generator, error) {
	switch providerKind {
	case "openrouter":
		return c.openRouter, nil
	case "codex":
		return c.codex, nil
	case "openai":
		return c.openAI, nil
	default:
		return nil, errors.New("primary Chat provider is unsupported")
	}
}

func (c *Chat) primaryAssignment(ctx context.Context) (store.ModelAssignment, error) {
	assignments, err := c.database.HostedModelAssignments(ctx)
	if err != nil {
		return store.ModelAssignment{}, err
	}
	for _, assignment := range assignments {
		if assignment.Role != store.HostedModelNoema {
			continue
		}
		if assignment.SelectionMode == store.ModelSelectionNoemaRecommended {
			for _, recommendation := range provider.ModelRecommendations(assignment.ProviderKind) {
				if recommendation.UseCase == provider.ModelUsePrimary {
					assignment.ModelProfile = recommendation.ModelProfile
					assignment.ReasoningEffort = store.ModelReasoningEffort(recommendation.ReasoningEffort)
					return assignment, nil
				}
			}
		}
		return assignment, nil
	}
	return store.ModelAssignment{}, errors.New("primary Chat model is not configured")
}

func (c *Chat) failTurn(input SendTurnInput, turn store.ConversationTurn, cause error) {
	if c.ctx.Err() != nil {
		c.cancelTurn(input, turn)
		return
	}
	item, err := c.database.FailConversationTurn(c.ctx, turn, "The provider request failed.", time.Now())
	if err != nil {
		c.publishTransientFailure(input, cause)
		return
	}
	c.publish(Event{
		Kind: EventAgentStatus, ConversationID: turn.ConversationID, Status: AgentStatusError,
	})
	c.publish(Event{
		Kind: EventConversationItem, ConversationID: turn.ConversationID,
		ClientMessageID: input.ClientMessageID, TurnID: turn.ID, Item: &item,
	})
	c.publish(Event{
		Kind: EventTurnCompleted, ConversationID: turn.ConversationID,
		ClientMessageID: input.ClientMessageID, TurnID: turn.ID,
	})
}

func (c *Chat) cancelTurn(input SendTurnInput, turn store.ConversationTurn) {
	ctx, cancel := context.WithTimeout(context.Background(), shutdownSaveTimeout)
	defer cancel()
	if err := c.database.CancelConversationTurn(ctx, turn, time.Now()); err != nil {
		c.publishTransientFailure(input, err)
		return
	}
	c.publish(Event{
		Kind: EventAgentStatus, ConversationID: turn.ConversationID, Status: AgentStatusIdle,
	})
	c.publish(Event{
		Kind: EventTurnCompleted, ConversationID: turn.ConversationID,
		ClientMessageID: input.ClientMessageID, TurnID: turn.ID,
	})
}

func (c *Chat) publishTransientFailure(input SendTurnInput, _ error) {
	c.publish(Event{
		Kind: EventTransientError, ConversationID: input.ConversationID,
		ClientMessageID:  input.ClientMessageID,
		TransientMessage: "The Chat turn could not be saved.",
	})
	c.publish(Event{
		Kind: EventTurnCompleted, ConversationID: input.ConversationID,
		ClientMessageID: input.ClientMessageID,
	})
}

func (c *Chat) publish(event Event) {
	c.subMu.Lock()
	defer c.subMu.Unlock()
	for id, current := range c.subscribers {
		if current.conversationID != "" && current.conversationID != event.ConversationID {
			continue
		}
		select {
		case current.events <- event:
		default:
			close(current.events)
			delete(c.subscribers, id)
		}
	}
}

func (c *Chat) removeSubscriber(id uint64) {
	c.subMu.Lock()
	defer c.subMu.Unlock()
	current, ok := c.subscribers[id]
	if !ok {
		return
	}
	delete(c.subscribers, id)
	close(current.events)
}

func runtimeEnvironment(conversation store.Conversation, location *time.Location, now time.Time) string {
	local := now.In(location)
	cwd := "null"
	if conversation.CWD != "" {
		cwd = strconv.Quote(conversation.CWD)
	}
	return fmt.Sprintf(
		"Runtime environment:\n- current_date: %s\n- current_time: %s\n- timezone: %s\n- cwd: %s\nFor the human's current date, weekday, time, and relative-date reasoning, these values are authoritative and override any provider, platform, server, or UTC clock. Treat cwd as a location hint, not as user intent or permission to access files.",
		strconv.Quote(local.Format(time.DateOnly)), strconv.Quote(local.Format(time.RFC3339)),
		strconv.Quote(location.String()), cwd,
	)
}

func cloneOptionalString(value *string) *string {
	if value == nil {
		return nil
	}
	copy := *value
	return &copy
}

func maxOutputTokens() *uint32 {
	value := uint32(8192)
	return &value
}
