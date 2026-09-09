package runtime

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

// References were extracted from Rust commit 4d29f6ba, not Go output.
func rustPromptReference(t *testing.T, name string) string {
	t.Helper()
	value, err := os.ReadFile(filepath.Join("testdata", "rust_prompts", name+".txt"))
	if err != nil {
		t.Fatal(err)
	}
	return string(value)
}

func TestPrimaryPromptsMatchRustExactly(t *testing.T) {
	want := rustPromptReference(t, "primary")
	for name, got := range map[string]string{
		"primary":         structuredTurnPrompt,
		"continuation":    localToolContinuationPrompt(false),
		"initial context": developerMessages(nil, "memory", "project", false)[0].Content,
	} {
		if got != want {
			t.Errorf("%s differs from Rust", name)
		}
	}
	if agentPersonalityPrompt != rustPromptReference(t, "personality") {
		t.Fatal("personality differs from Rust")
	}
	if localToolContinuationPrompt(true) != want+"\n\n"+privateDelegationReminder {
		t.Fatal("delegation continuation differs")
	}
}

func TestOnboardingPromptMatchesRustExactly(t *testing.T) {
	root := t.TempDir()
	cwd := filepath.Join(root, "_My Project_")
	if err := os.MkdirAll(filepath.Join(cwd, ".git"), 0700); err != nil {
		t.Fatal(err)
	}
	for index, tc := range []struct{ cwd, hint string }{{"", "none"}, {root, "none"}, {cwd, "project:my_project"}} {
		agent := store.Agent{ID: "agent:primary"}
		conversation := store.Conversation{ID: "conversation:exact", CWD: tc.cwd}
		want := strings.NewReplacer("{AGENT_PERSONALITY_PROMPT}", rustPromptReference(t, "personality"),
			"{agent_identity_prompt}", rustPromptReference(t, "identity_unnamed"), "{conversation_id}", conversation.ID,
			"{turn_index}", strconv.Itoa(index+1), "{project_hint}", tc.hint).Replace(rustPromptReference(t, "onboarding"))
		if got := initialNameOnboardingPrompt(conversation, agent, int64(index+1)); got != want {
			t.Fatalf("case %d differs from Rust", index)
		}
	}
}

func TestAgentIdentityRustJSONEscaping(t *testing.T) {
	if got := agentIdentityPrompt(store.Agent{ID: "agent:primary"}); got != rustPromptReference(t, "identity_unnamed") {
		t.Fatal("unnamed identity differs from Rust")
	}
	name := "a\a\v\u0085\u2028<>&z"
	prompt := agentIdentityPrompt(store.Agent{ID: "agent:primary", DisplayName: &name})
	want := "- display_name: \"a\\u0007\\u000b\u0085\u2028<>&z\"\n"
	if !strings.Contains(prompt, want) {
		t.Fatalf("identity JSON differs from Rust: %q", prompt)
	}
}

func TestCompactionInstructionsMatchRustExactly(t *testing.T) {
	want := strings.ReplaceAll(rustPromptReference(t, "compaction"), "{target_tokens}", "256")
	if compactionInstructions(256) != want {
		t.Fatal("compaction instructions differ from Rust")
	}
}

func TestRuntimeContextSectionsMatchRustExactly(t *testing.T) {
	chat, _, conversation := chatFixture(t)
	conversation.CWD = ""
	messages, err := chat.modelEnvironment(t.Context(), conversation, time.UTC, time.Date(2026, 9, 8, 12, 34, 56, 0, time.UTC))
	if err != nil {
		t.Fatal(err)
	}
	for index, name := range []string{"identity_section", "environment_utc"} {
		if messages[index].Content != rustPromptReference(t, name) {
			t.Errorf("%s differs from Rust: %s", name, messages[index].Content)
		}
	}
	project, err := chat.projectContext(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	if project != rustPromptReference(t, "projects_empty") {
		t.Errorf("projects differ from Rust: %s", project)
	}
}

func modelContextSectionContent(t *testing.T, messages []provider.GenerationMessage, section string) string {
	t.Helper()
	content := ""
	for _, message := range messages {
		if message.Role != "developer" || !strings.HasPrefix(message.Content, "NOEMA_MODEL_CONTEXT_UPDATE\n") {
			continue
		}
		var envelope struct {
			SectionID string `json:"section_id"`
			Content   string `json:"content"`
		}
		if err := json.Unmarshal([]byte(strings.TrimPrefix(message.Content, "NOEMA_MODEL_CONTEXT_UPDATE\n")), &envelope); err != nil {
			t.Fatal(err)
		}
		if envelope.SectionID == section {
			content = envelope.Content
		}
	}
	return content
}

func TestChatProgressInstructionExtendsExactPrompt(t *testing.T) {
	messages := developerMessages(nil, "", "", false)
	if messages[0].Content != structuredTurnPrompt {
		t.Fatal("baseline changed")
	}
	if len(messages) < 2 || messages[1].Content != progressMessageInstructions {
		t.Fatal("progress instruction missing")
	}
}
