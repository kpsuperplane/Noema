package store

import (
	"fmt"
	"unicode"
)

// ConversationError is a closed-domain conversation validation error.
//
// Code identifies the Rust conversation error variant. The remaining fields
// carry the values that the variant reports.
type ConversationError struct {
	Code          ConversationErrorCode
	Kind          string
	Value         string
	ReferenceKind string
}

// ConversationErrorCode identifies one conversation validation failure.
type ConversationErrorCode string

const (
	ConversationErrorInvalidEnum      ConversationErrorCode = "invalid_enum"
	ConversationErrorEmptyReferenceID ConversationErrorCode = "empty_reference_id"
)

func (e *ConversationError) Error() string {
	if e == nil {
		return "conversation error"
	}
	switch e.Code {
	case ConversationErrorInvalidEnum:
		return fmt.Sprintf("invalid %s value: %s", e.Kind, e.Value)
	case ConversationErrorEmptyReferenceID:
		return fmt.Sprintf("%s reference id cannot be empty", e.ReferenceKind)
	default:
		return "conversation validation failed"
	}
}

// ActorRef identifies the actor that authored a conversation item.
type ActorRef struct {
	ActorID string
}

// NewActorRef validates an actor reference id.
func NewActorRef(actorID string) (ActorRef, error) {
	if isBlankConversationReference(actorID) {
		return ActorRef{}, &ConversationError{
			Code: ConversationErrorEmptyReferenceID, ReferenceKind: "actor",
		}
	}
	return ActorRef{ActorID: actorID}, nil
}

// ConversationOwnerRef identifies the human owner of a conversation.
type ConversationOwnerRef struct {
	HumanID string
}

// NewConversationOwnerRef validates a conversation owner id.
func NewConversationOwnerRef(humanID string) (ConversationOwnerRef, error) {
	if isBlankConversationReference(humanID) {
		return ConversationOwnerRef{}, &ConversationError{
			Code: ConversationErrorEmptyReferenceID, ReferenceKind: "conversation_owner",
		}
	}
	return ConversationOwnerRef{HumanID: humanID}, nil
}

func isBlankConversationReference(value string) bool {
	for _, character := range value {
		if !unicode.IsSpace(character) {
			return false
		}
	}
	return true
}

// NewConversation is the validated input shape for creating a conversation.
type NewConversation struct {
	Title          *string
	Owner          ConversationOwnerRef
	PrimaryHumanID *string
	PrimaryAgentID *string
	Provider       string
	Model          *string
	CWD            *string
	Metadata       map[string]any
}

// NewLocalConversation builds the default local Codex conversation shape.
func NewLocalConversation(model, cwd *string) NewConversation {
	return NewLocalConversationForProvider("codex", model, cwd)
}

// NewLocalConversationForProvider builds the default local conversation shape
// for a runtime provider.
func NewLocalConversationForProvider(provider string, model, cwd *string) NewConversation {
	owner, err := NewConversationOwnerRef("human:local")
	if err != nil {
		panic("static local human owner id must be valid")
	}
	humanID := owner.HumanID
	agentID := "agent:primary"
	return NewConversation{
		Owner:          owner,
		PrimaryHumanID: &humanID,
		PrimaryAgentID: &agentID,
		Provider:       provider,
		Model:          model,
		CWD:            cwd,
		Metadata:       map[string]any{},
	}
}

// AgentStatus is the durable live-agent status vocabulary.
type AgentStatus string

const (
	AgentStatusIdle                             AgentStatus = "idle"
	AgentStatusInputReceived                    AgentStatus = "input_received"
	AgentStatusThinking                         AgentStatus = "thinking"
	AgentStatusToolRunning                      AgentStatus = "tool_running"
	AgentStatusWaitingForPreviousTurnCompletion AgentStatus = "waiting_for_previous_turn_completion"
	AgentStatusInterrupting                     AgentStatus = "interrupting"
	AgentStatusError                            AgentStatus = "error"
)

// String returns the stable storage value.
func (s AgentStatus) String() string { return string(s) }

// ParseAgentStatus parses a stable storage value.
func ParseAgentStatus(value string) (AgentStatus, error) {
	status := AgentStatus(value)
	switch status {
	case AgentStatusIdle, AgentStatusInputReceived, AgentStatusThinking,
		AgentStatusToolRunning, AgentStatusWaitingForPreviousTurnCompletion,
		AgentStatusInterrupting, AgentStatusError:
		return status, nil
	default:
		return "", invalidConversationEnum("agent_status", value)
	}
}

// AllAgentStatuses returns every durable agent status in wire order.
func AllAgentStatuses() []AgentStatus {
	return []AgentStatus{
		AgentStatusIdle, AgentStatusInputReceived, AgentStatusThinking,
		AgentStatusToolRunning, AgentStatusWaitingForPreviousTurnCompletion,
		AgentStatusInterrupting, AgentStatusError,
	}
}

// ConversationTurnStatus is the durable lifecycle status of one turn.
type ConversationTurnStatus string

const (
	ConversationTurnInputReceived  ConversationTurnStatus = "input_received"
	ConversationTurnRunning        ConversationTurnStatus = "running"
	ConversationTurnWaitingForTool ConversationTurnStatus = "waiting_for_tool"
	ConversationTurnInterrupted    ConversationTurnStatus = "interrupted"
	ConversationTurnCompleted      ConversationTurnStatus = "completed"
	ConversationTurnFailed         ConversationTurnStatus = "failed"
	ConversationTurnCancelled      ConversationTurnStatus = "cancelled"
)

func (s ConversationTurnStatus) String() string { return string(s) }

// ParseConversationTurnStatus parses a stable storage value.
func ParseConversationTurnStatus(value string) (ConversationTurnStatus, error) {
	status := ConversationTurnStatus(value)
	switch status {
	case ConversationTurnInputReceived, ConversationTurnRunning,
		ConversationTurnWaitingForTool, ConversationTurnInterrupted,
		ConversationTurnCompleted, ConversationTurnFailed, ConversationTurnCancelled:
		return status, nil
	default:
		return "", invalidConversationEnum("conversation_turn_status", value)
	}
}

// AllConversationTurnStatuses returns every turn status in wire order.
func AllConversationTurnStatuses() []ConversationTurnStatus {
	return []ConversationTurnStatus{
		ConversationTurnInputReceived, ConversationTurnRunning,
		ConversationTurnWaitingForTool, ConversationTurnInterrupted,
		ConversationTurnCompleted, ConversationTurnFailed, ConversationTurnCancelled,
	}
}

// ConversationItemStatus is the durable execution status of one item.
type ConversationItemStatus string

const (
	ConversationItemPending     ConversationItemStatus = "pending"
	ConversationItemRunning     ConversationItemStatus = "running"
	ConversationItemCompleted   ConversationItemStatus = "completed"
	ConversationItemFailed      ConversationItemStatus = "failed"
	ConversationItemCancelled   ConversationItemStatus = "cancelled"
	ConversationItemInterrupted ConversationItemStatus = "interrupted"
)

func (s ConversationItemStatus) String() string { return string(s) }

// ParseConversationItemStatus parses a stable storage value.
func ParseConversationItemStatus(value string) (ConversationItemStatus, error) {
	status := ConversationItemStatus(value)
	switch status {
	case ConversationItemPending, ConversationItemRunning, ConversationItemCompleted,
		ConversationItemFailed, ConversationItemCancelled, ConversationItemInterrupted:
		return status, nil
	default:
		return "", invalidConversationEnum("conversation_item_status", value)
	}
}

// AllConversationItemStatuses returns every item status in wire order.
func AllConversationItemStatuses() []ConversationItemStatus {
	return []ConversationItemStatus{
		ConversationItemPending, ConversationItemRunning, ConversationItemCompleted,
		ConversationItemFailed, ConversationItemCancelled, ConversationItemInterrupted,
	}
}

// ConversationContextSummaryStatus is the lifecycle status of a context summary.
type ConversationContextSummaryStatus string

const (
	ConversationSummaryPending    ConversationContextSummaryStatus = "pending"
	ConversationSummaryActive     ConversationContextSummaryStatus = "active"
	ConversationSummaryFailed     ConversationContextSummaryStatus = "failed"
	ConversationSummarySuperseded ConversationContextSummaryStatus = "superseded"
)

func (s ConversationContextSummaryStatus) String() string { return string(s) }

// ParseConversationContextSummaryStatus parses a stable storage value.
func ParseConversationContextSummaryStatus(value string) (ConversationContextSummaryStatus, error) {
	status := ConversationContextSummaryStatus(value)
	switch status {
	case ConversationSummaryPending, ConversationSummaryActive,
		ConversationSummaryFailed, ConversationSummarySuperseded:
		return status, nil
	default:
		return "", invalidConversationEnum("conversation_context_summary_status", value)
	}
}

// AllConversationContextSummaryStatuses returns every summary status in wire order.
func AllConversationContextSummaryStatuses() []ConversationContextSummaryStatus {
	return []ConversationContextSummaryStatus{
		ConversationSummaryPending, ConversationSummaryActive,
		ConversationSummaryFailed, ConversationSummarySuperseded,
	}
}

// String returns the stable storage value.
func (k ConversationItemKind) String() string { return string(k) }

// ParseConversationItemKind parses a stable storage value.
func ParseConversationItemKind(value string) (ConversationItemKind, error) {
	kind := ConversationItemKind(value)
	switch kind {
	case ConversationUserText, ConversationAssistantText, ConversationActivity,
		ConversationA2UICard, ConversationMultipleChoicePrompt,
		ConversationMultipleChoiceSelection, ConversationToolCall,
		ConversationToolResult, ConversationReasoning, ConversationModelContextUpdate,
		ConversationApprovalRequest, ConversationApprovalResult,
		ConversationArtifactReference, ConversationTaskReference, ConversationErrorNotice:
		return kind, nil
	default:
		return "", invalidConversationEnum("conversation_item_kind", value)
	}
}

// AllConversationItemKinds returns every item kind in wire order.
func AllConversationItemKinds() []ConversationItemKind {
	return []ConversationItemKind{
		ConversationUserText, ConversationAssistantText, ConversationActivity,
		ConversationA2UICard, ConversationMultipleChoicePrompt,
		ConversationMultipleChoiceSelection, ConversationToolCall,
		ConversationToolResult, ConversationReasoning, ConversationModelContextUpdate,
		ConversationApprovalRequest, ConversationApprovalResult,
		ConversationArtifactReference, ConversationTaskReference, ConversationErrorNotice,
	}
}

func invalidConversationEnum(kind, value string) error {
	return &ConversationError{
		Code:  ConversationErrorInvalidEnum,
		Kind:  kind,
		Value: value,
	}
}
