package runtime

import (
	"crypto/sha256"
	"encoding/json"
	"reflect"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

// Chat owns one warm session. Switching conversations releases the old session.
// Only the serialized Chat loop reads or changes this state.
type chatProviderSession struct {
	provider.GenerationSession
	conversationID, turnID, responseID string
	assignment                         store.ModelAssignment
	credentialRevision                 uint64
	base                               []provider.GenerationMessage
	history                            [32]byte
}

func (c *Chat) closeChatSession() {
	if c.session != nil {
		_ = c.session.Close()
		c.session = nil
	}
}

func chatHistoryDigest(messages []provider.GenerationMessage) [32]byte {
	encoded, _ := json.Marshal(messages)
	return sha256.Sum256(encoded)
}

func (c *Chat) chatSessionRequest(generator provider.Generator, assignment store.ModelAssignment,
	turn store.ConversationTurn, base, completed, active, full []provider.GenerationMessage, compacted bool,
) (provider.Generator, []provider.GenerationMessage, string, error) {
	opener, supported := generator.(provider.SessionGenerator)
	if !supported || !responseIDContinuationProvider(assignment.ProviderKind) {
		c.closeChatSession()
		return generator, full, "", nil
	}
	account, err := c.database.ProviderAccount(c.ctx, assignment.ProviderAccountID)
	if err != nil {
		return nil, nil, "", err
	}
	revision := account.Metadata.CredentialRevision()
	session := c.session
	if session != nil && (compacted || session.conversationID != turn.ConversationID ||
		session.assignment != assignment || session.credentialRevision != revision ||
		!reflect.DeepEqual(session.base, base) || session.history != chatHistoryDigest(completed) ||
		!continuationReady(session.GenerationSession, session.responseID)) {
		c.closeChatSession()
		session = nil
	}
	messages, previousID := full, ""
	if session == nil {
		session = &chatProviderSession{GenerationSession: opener.OpenGenerationSession(),
			conversationID: turn.ConversationID, assignment: assignment, credentialRevision: revision, base: base}
		c.session = session
	} else {
		previousID = session.responseID
		// Top-level instructions must accompany every Responses request.
		messages = nil
		for _, message := range base {
			if message.Instructions {
				messages = append(messages, message)
			}
		}
		messages = append(messages, active...)
	}
	session.turnID, session.responseID = turn.ID, ""
	return session.GenerationSession, messages, previousID, nil
}

func (c *Chat) retainChatSession(turn store.ConversationTurn, assignment store.ModelAssignment, responseID string) {
	session := c.session
	if session == nil || session.turnID != turn.ID || responseID == "" {
		return
	}
	state, err := c.database.ConversationProviderContext(c.ctx, turn.ConversationID, assignment.ProviderKind, assignment.ModelProfile)
	if err != nil {
		return
	}
	completed, _, _, err := chatContextParts(state, "", assignment.ProviderKind)
	if err != nil {
		return
	}
	session.history = chatHistoryDigest(completed)
	session.responseID = responseID
}
