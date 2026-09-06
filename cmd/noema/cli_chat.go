package main

import (
	"context"
	"encoding/json"
	"errors"
	"io"

	"github.com/urfave/cli/v3"
)

const transcriptFields = `__typename
 ... on UserText{text} ... on AssistantText{text} ... on ErrorNotice{message recoverable}
 ... on Activity{id activityKind status title summary metadata}
 ... on MultipleChoicePrompt{prompt selectionMode options{id label}}
 ... on MultipleChoiceSelection{promptItemId selectionMode selectedOptions{id label}}
 ... on A2UISurface{id interactionId surfaceId revision lifecycle snapshot hasActions}
 ... on ArtifactReference{artifactId artifactVersionId title artifactKind}
 ... on TaskReference{taskId}`
const chatReadQuery = `query($input:ConversationTranscriptPageInput!){conversationTranscriptPage(input:$input){items{itemId cursor turnId metadata item{` + transcriptFields + `}}pageInfo{beforeCursor hasMoreBefore}}}`
const chatSendQuery = `mutation($input:SendConversationTurnInput!){sendConversationTurn(input:$input){conversationId clientMessageId}}`
const chatWatchQuery = `subscription($id:String!){conversationEvents(conversationId:$id){__typename
 ... on SubscriptionReadyEvent{conversationId}
 ... on HumanInterventionsChangedEvent{conversationId}
 ... on AssistantTextDeltaEvent{conversationId deltaTurnId:turnId streamId responseIndex delta}
 ... on AgentStatusEvent{conversationId status}
 ... on ConversationItemEvent{conversationId clientMessageId itemId cursor turnId metadata item{` + transcriptFields + `}}
 ... on TurnCompletedEvent{conversationId clientMessageId}}}`
const interventionQuery = `query($id:String!){pendingHumanInterventions(conversationId:$id,first:100){__typename
 ... on GovernedAction{actionId revision safeSummary actionState:state}
 ... on McpAuthenticationIntervention{requestId revision serverDisplayName state}
 ... on AdapterAuthenticationIntervention{requestId revision serviceDisplayName state}
 ... on McpSetupIntervention{itemId displayName setupStatus}
 ... on AdapterDefinition{definitionId definitionRevision displayName reviewed}
 ... on AdapterOauthClientSetupIntervention{profileDigest displayName}
 ... on AdapterOauthAccountSetupIntervention{setupKey providerDisplayName}
}}`

func chatCommands(input io.Reader, action clientAction) *cli.Command {
	return &cli.Command{Name: "chat", Usage: "Read or send messages in Chat", Flags: []cli.Flag{stringFlag("conversation", "Conversation ID; defaults to primary Chat")}, Commands: []*cli.Command{
		{Name: "conversation", Usage: "Read the primary conversation identity", Action: action(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 0); err != nil {
				return err
			}
			return l.printQuery(ctx, `{primaryConversation{conversationId provider}}`, nil)
		})},
		{Name: "read", Usage: "Read one transcript page", Flags: []cli.Flag{&cli.IntFlag{Name: "limit", Value: 80}, stringFlag("cursor", "Read the previous page at this cursor")}, Action: action(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 0); err != nil {
				return err
			}
			id, err := chatConversation(ctx, c, l, false)
			if err != nil {
				return err
			}
			if id == "" {
				return l.emit(map[string]any{"data": map[string]any{"primaryConversation": nil}})
			}
			values := map[string]any{"conversationId": id, "limit": c.Int("limit")}
			if c.IsSet("cursor") {
				values["cursor"] = c.String("cursor")
			}
			return l.printQuery(ctx, chatReadQuery, map[string]any{"input": values})
		})},
		{Name: "watch", Usage: "Stream conversation events", Action: action(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 0); err != nil {
				return err
			}
			id, err := chatConversation(ctx, c, l, false)
			if err != nil {
				return err
			}
			if id == "" {
				return errors.New("primary Chat does not exist; send a message first")
			}
			return l.stream(ctx, apiRequest{Query: chatWatchQuery, Variables: map[string]any{"id": id}}, nil)
		})},
		{Name: "send", Usage: "Send a message and stream its response", ArgsUsage: "[TEXT]", Flags: []cli.Flag{stringFlag("file", "Message file; - reads stdin"), stringFlag("request-id", "Client message ID for response correlation"), stringFlag("time-zone", "IANA time zone for this message"), &cli.BoolFlag{Name: "no-wait", Usage: "Return after Noema accepts the message"}}, Action: action(func(ctx context.Context, c *cli.Command, l *localClient) error {
			message, err := commandText(c, input, true)
			if err != nil {
				return err
			}
			id, err := chatConversation(ctx, c, l, true)
			if err != nil {
				return err
			}
			clientID := requestID(c)
			values := map[string]any{"conversationId": id, "input": message, "clientMessageId": clientID}
			if c.IsSet("time-zone") {
				values["clientTimeZone"] = c.String("time-zone")
			}
			send := func() error { return l.printQuery(ctx, chatSendQuery, map[string]any{"input": values}) }
			if c.Bool("no-wait") {
				return send()
			}
			baseline := map[string]bool{}
			sent, failed, started := false, false, false
			return l.stream(ctx, apiRequest{Query: chatWatchQuery, Variables: map[string]any{"id": id}}, func(response *apiResponse) (bool, error) {
				var data struct {
					Event struct {
						Type            string `json:"__typename"`
						ClientMessageID string
						Item            struct {
							Type   string `json:"__typename"`
							Status string
						}
					} `json:"conversationEvents"`
				}
				if err := json.Unmarshal(response.Data, &data); err != nil {
					return false, err
				}
				event := data.Event
				if event.Type == "SubscriptionReadyEvent" {
					if !sent {
						pending, err := l.pendingChatInput(ctx, id)
						if err != nil {
							return false, err
						}
						for _, item := range pending {
							baseline[string(item)] = true
						}
						sent = true
						return false, send()
					}
					return false, nil
				}
				if !sent {
					return false, errors.New("conversation event arrived before subscription readiness")
				}
				if err := l.emit(response); err != nil {
					return false, err
				}
				if event.Type == "ConversationItemEvent" && event.ClientMessageID == clientID {
					started = true
				}
				if event.ClientMessageID == clientID && (event.Item.Type == "ErrorNotice" || event.Item.Type == "Activity" && event.Item.Status == "FAILED") {
					failed = true
				}
				if event.Type == "TurnCompletedEvent" && event.ClientMessageID == clientID {
					if failed {
						return false, errors.New("Chat turn failed; inspect the streamed response")
					}
					return true, nil
				}
				if event.Type == "HumanInterventionsChangedEvent" && started {
					pending, err := l.pendingChatInput(ctx, id)
					if err != nil {
						return false, err
					}
					for _, item := range pending {
						if !baseline[string(item)] {
							return true, l.emit(map[string]any{"status": "needs_input", "conversationId": id, "clientMessageId": clientID, "interventions": pending})
						}
					}
				}
				return false, nil
			})
		})},
	}}
}

func chatConversation(ctx context.Context, c *cli.Command, l *localClient, create bool) (string, error) {
	if id := c.String("conversation"); id != "" {
		return id, nil
	}
	query := `{primaryConversation{conversationId}}`
	field := "primaryConversation"
	if create {
		query = `mutation{ensurePrimaryConversation{conversationId}}`
		field = "ensurePrimaryConversation"
	}
	response, err := l.query(ctx, apiRequest{Query: query})
	if err != nil {
		if response != nil {
			if e := l.emit(response); e != nil {
				return "", e
			}
		}
		return "", err
	}
	var data map[string]struct{ ConversationID string }
	if err = json.Unmarshal(response.Data, &data); err != nil {
		return "", err
	}
	return data[field].ConversationID, nil
}

func (l *localClient) pendingChatInput(ctx context.Context, id string) ([]json.RawMessage, error) {
	response, err := l.query(ctx, apiRequest{Query: interventionQuery, Variables: map[string]any{"id": id}})
	if err != nil {
		if response != nil {
			if e := l.emit(response); e != nil {
				return nil, e
			}
		}
		return nil, err
	}
	var state struct {
		Pending []json.RawMessage `json:"pendingHumanInterventions"`
	}
	err = json.Unmarshal(response.Data, &state)
	return state.Pending, err
}
