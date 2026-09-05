package runtime

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"strconv"
	"strings"
	"time"
	"unicode/utf8"

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
) (string, error) {
	agent, err := c.database.Agent(ctx, store.PrimaryAgentID)
	if err != nil {
		return "", err
	}
	return agentIdentityPrompt(agent) + "\n\n" + runtimeEnvironment(conversation, location, now), nil
}

func agentIdentityPrompt(agent store.Agent) string {
	var prompt strings.Builder
	prompt.WriteString("Agent identity:\n- agent_id: ")
	prompt.WriteString(strconv.Quote(agent.ID))
	prompt.WriteString("\n- display_name: ")
	if agent.DisplayName == nil {
		prompt.WriteString("null\n\nOnboarding prompt:\n")
		prompt.WriteString("- You do not have a name yet.\n")
		prompt.WriteString("- Your first priority is to ask the user to give you one.\n")
		prompt.WriteString("- Do not invent, assume, or sign off with a name.\n")
		prompt.WriteString("- If the user gives you a name, call update_own_name with that name.\n")
	} else {
		prompt.WriteString(strconv.Quote(*agent.DisplayName))
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
