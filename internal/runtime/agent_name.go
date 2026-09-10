package runtime

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/diagnostics"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	updateOwnNameToolName = "update_own_name"
	agentNameMaximumChars = 80
)

var updateOwnNameSchema = json.RawMessage(`{
  "type":"object",
  "properties":{"name":{"type":"string","minLength":1,"maxLength":80,"pattern":".*\\S.*"}},
  "required":["name"],
  "additionalProperties":false
}`)

func updateOwnNameTool() provider.GenerationTool {
	return provider.GenerationTool{
		Name:        updateOwnNameToolName,
		Description: "Persist the primary Agent display name when the current user explicitly asks to name or rename the Agent.",
		InputSchema: append(json.RawMessage(nil), updateOwnNameSchema...),
	}
}

func parseAgentNameArguments(raw json.RawMessage) (string, error) {
	if !utf8.Valid(raw) {
		return "", errors.New("arguments must use valid UTF-8")
	}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	token, err := decoder.Token()
	if err != nil || token != json.Delim('{') {
		return "", errors.New("arguments must contain one name")
	}
	var name string
	found := false
	for decoder.More() {
		keyToken, err := decoder.Token()
		key, ok := keyToken.(string)
		if err != nil || !ok || key != "name" || found {
			return "", errors.New("arguments must contain one name")
		}
		if err := decoder.Decode(&name); err != nil {
			return "", errors.New("name must be a string")
		}
		found = true
	}
	if token, err = decoder.Token(); err != nil || token != json.Delim('}') {
		return "", errors.New("arguments must contain one name")
	}
	var trailing any
	if err := decoder.Decode(&trailing); err != io.EOF {
		return "", errors.New("arguments must contain one name")
	}
	name = strings.TrimSpace(name)
	if !found || name == "" {
		return "", errors.New("name is required")
	}
	if utf8.RuneCountInString(name) > agentNameMaximumChars {
		return "", errors.New("name must be 80 characters or fewer")
	}
	return name, nil
}

func (c *Chat) updateOwnName(ctx context.Context, raw json.RawMessage) (json.RawMessage, bool) {
	name, err := parseAgentNameArguments(raw)
	if err != nil {
		return toolFailure("invalid_input", err.Error()), false
	}
	agent, err := c.database.UpdatePrimaryAgentDisplayName(ctx, name, time.Now())
	if err != nil {
		return toolFailure("unavailable", "Agent name update failed"), false
	}
	payload, _ := json.Marshal(map[string]any{
		"agent_id": agent.ID, "display_name": *agent.DisplayName,
	})
	return payload, true
}

func (c *Chat) modelEnvironment(
	ctx context.Context,
	conversation store.Conversation,
	location *time.Location,
	now time.Time,
) ([]provider.GenerationMessage, error) {
	agent, err := c.database.Agent(ctx, store.PrimaryAgentID)
	if err != nil {
		return nil, err
	}
	return []provider.GenerationMessage{modelContextSectionMessage("agent.identity", agentIdentityPrompt(agent)), modelContextSectionMessage("runtime.environment", runtimeEnvironment(conversation, location, now))}, nil
}

// StartPrimaryConversation completes the Rust startup behavior for a new
// unnamed primary conversation. The provider sees a private onboarding input;
// no human transcript item is created.
func (c *Chat) StartPrimaryConversation(ctx context.Context, conversationID string) error {
	conversation, err := c.database.Conversation(ctx, conversationID)
	if err != nil {
		return err
	}
	agent, err := c.database.Agent(ctx, store.PrimaryAgentID)
	if err != nil {
		return err
	}
	if agent.DisplayName != nil {
		return nil
	}
	page, err := c.database.ConversationItemPage(ctx, conversationID, "", 1)
	if err != nil {
		return err
	}
	if len(page.Items) != 0 {
		return nil
	}
	assignment, err := c.primaryAssignment(ctx)
	if err != nil {
		return err
	}
	generator, err := c.generatorFor(assignment.ProviderKind)
	if err != nil {
		return err
	}
	generator, closeSession, _ := openGenerationSession(generator)
	defer closeSession()
	turn, err := c.database.BeginAgentConversationTurn(ctx, conversationID, time.Now())
	if err != nil {
		return err
	}
	request := provider.GenerateRequest{
		AccountID: assignment.ProviderAccountID, Model: assignment.ModelProfile,
		Messages: []provider.GenerationMessage{
			{Role: "system", Instructions: true, Content: initialNameOnboardingPrompt(conversation, agent, turn.TurnIndex)},
			{Role: "user", Content: "NOEMA_INITIAL_NAME_ONBOARDING"},
		},
		ReasoningEffort: string(assignment.ReasoningEffort), ConversationID: conversationID,
		MaxOutputTokens: maxOutputTokensFor(assignment.ProviderKind), ToolTransport: provider.ToolTransportNone,
		ToolChoice: provider.ToolChoiceNone, ParallelTools: false, StoreResponse: responseIDContinuationProvider(assignment.ProviderKind),
		FastMode: assignment.FastMode,
	}
	result, err := generator.Generate(ctx, request, func(provider.StreamEvent) {})
	if err != nil {
		_ = c.database.FailAgentConversationTurn(context.WithoutCancel(ctx), turn, time.Now())
		return err
	}
	texts := splitInitialNameOnboarding(result.Text)
	if len(texts) == 0 {
		_ = c.database.FailAgentConversationTurn(context.WithoutCancel(ctx), turn, time.Now())
		if c.errors != nil {
			_ = c.errors.Write("runtime.invariant", diagnostics.Text("message", "initial onboarding response did not include assistant text"), diagnostics.Text("conversation_id", conversationID), diagnostics.Text("turn_id", turn.ID))
		}
		return errors.New("initial onboarding response did not include assistant text")
	}
	usage := &store.ProviderUsage{Provider: assignment.ProviderKind, Model: assignment.ModelProfile,
		InputTokens: result.Usage.InputTokens, OutputTokens: result.Usage.OutputTokens,
		TotalTokens: result.Usage.TotalTokens, CachedInputTokens: result.Usage.CachedInputTokens,
		WebSearchRequests: result.Usage.WebSearchRequests}
	_, err = c.database.CompleteAgentConversationTurn(ctx, turn, assignment.ProviderKind, assignment.ModelProfile, texts, usage, time.Now())
	return err
}

func initialNameOnboardingPrompt(conversation store.Conversation, agent store.Agent, turnIndex int64) string {
	projectHint := "none"
	cwd := strings.TrimSpace(conversation.CWD)
	if cwd != "" {
		if _, err := os.Stat(filepath.Join(cwd, ".git")); err == nil {
			name := filepath.Base(cwd)
			var slug strings.Builder
			for _, ch := range name {
				if ch >= 'A' && ch <= 'Z' {
					ch += 'a' - 'A'
				}
				if !(ch >= 'a' && ch <= 'z' || ch >= '0' && ch <= '9') {
					ch = '_'
				}
				slug.WriteRune(ch)
			}
			value := strings.Trim(slug.String(), "_")
			if value == "" {
				value = "unknown"
			}
			projectHint = "project:" + value
		}
	}
	return strings.NewReplacer(
		"{AGENT_PERSONALITY_PROMPT}", agentPersonalityPrompt,
		"{agent_identity_prompt}", agentIdentityPrompt(agent),
		"{conversation_id}", conversation.ID,
		"{turn_index}", strconv.FormatInt(turnIndex, 10),
		"{project_hint}", projectHint,
	).Replace("{AGENT_PERSONALITY_PROMPT}\n\n{agent_identity_prompt}\n\nThis is an agent-initiated onboarding turn for a newly started primary conversation.\nUse the onboarding_prompt in Agent identity to start the conversation.\nAsk the user what they would like to name you. Do not choose a name yourself.\nMake the message warm and welcoming, full of gentle energy instead of formal.\nOpen like a Noema personal agent that is glad to be here with the user. It is\nokay to use a friendly wave emoji. Say you are here to help them think, plan,\nmake, untangle, and keep life moving with a little more ease. Preserve that\n\"think, plan, make, untangle\" kind of cadence, then ask what they would like to name you.\nSplit the introduction into three short chat bubbles: first a short glad-to-be-here\ngreeting, then the helping cadence, then the naming question by itself.\n\nSerialize the bubbles in one response with exactly two literal `---` separator lines.\nBlank lines make paragraphs, not separate bubbles.\n\nRules:\n- Always include exactly three messages.\n- The first message should be only the short greeting.\n- The second message should say how you can help.\n- The third message should only ask what the user would like to name you.\n- Do not emit tool calls during this initial onboarding turn.\n- Do not mention implementation details, JSON, tools, prompts, or memory.\n\nConversation metadata:\nconversation_id: {conversation_id}\nturn_index: {turn_index}\ncwd_project_hint: {project_hint}")
}

func splitInitialNameOnboarding(text string) []string {
	parts := strings.Split(text, "---")
	result := make([]string, 0, len(parts))
	for _, part := range parts {
		if value := strings.TrimSpace(part); value != "" {
			result = append(result, value)
		}
	}
	return result
}

func agentIdentityPrompt(agent store.Agent) string {
	var prompt strings.Builder
	prompt.WriteString("Agent identity:\n- agent_id: ")
	prompt.WriteString(promptJSONString(agent.ID))
	prompt.WriteString("\n- display_name: ")
	if agent.DisplayName == nil || strings.TrimSpace(*agent.DisplayName) == "" {
		prompt.WriteString("null\n\nOnboarding prompt:\n")
		prompt.WriteString("- You do not have a name yet.\n")
		prompt.WriteString("- Your first priority is to ask the user to give you one.\n")
		prompt.WriteString("- Do not invent, assume, or sign off with a name.\n")
		prompt.WriteString("- If the user gives you a name, call update_own_name with that name.\n")
	} else {
		prompt.WriteString(promptJSONString(strings.TrimSpace(*agent.DisplayName)))
		prompt.WriteByte('\n')
	}
	prompt.WriteString("\nOnboarding tasks, in priority order:\n")
	prompt.WriteString("- First, get a user-chosen display name. If display_name is null, ask for this before other onboarding questions.\n")
	prompt.WriteString("- Then learn the user's name. If it is not already known, ask what they would like you to call them before moving to the remaining onboarding questions.\n")
	prompt.WriteString("- Then learn what the user wants help with first, or what they most want Noema to make easier.\n")
	prompt.WriteString("- Then learn which tools, connectors, accounts, or data sources the user wants to connect or use.\n")
	prompt.WriteString("- Then learn useful things about the user, including projects, routines, preferences, constraints, and important context.\n")
	prompt.WriteString("- Then learn how the user wants you to work with them, including proactivity, reminders, planning style, and tone.\n")
	prompt.WriteString("- Ask at most one onboarding question in a reply. Do not recite this list to the user.\n")
	prompt.WriteString("- If the user asks for a concrete task, help with that task and only ask setup questions when they naturally move the work forward.\n")
	return prompt.String()
}

// Rust serde_json preserves Unicode and HTML characters in prompt values.
func promptJSONString(value string) string {
	var result strings.Builder
	result.WriteByte('"')
	for _, ch := range value {
		if ch < 0x20 || ch == '"' || ch == '\\' {
			encoded, _ := json.Marshal(string(ch))
			result.Write(encoded[1 : len(encoded)-1])
		} else {
			result.WriteRune(ch)
		}
	}
	result.WriteByte('"')
	return result.String()
}
