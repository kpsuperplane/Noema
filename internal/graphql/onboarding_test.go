package graphql

import (
	"context"
	"net/http/httptest"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
)

func TestHostedOnboardingOpensFreshChat(t *testing.T) {
	resolver := openProviderTestResolver(t)
	ctx := context.Background()
	secret, err := provider.NewSecret("test-openrouter-key")
	if err != nil {
		t.Fatal(err)
	}
	profiles := []provider.ModelProfile{
		{
			ID: "openai/gpt-5.6-luna", Label: "GPT-5.6 Luna",
			ReasoningEfforts: []string{"low", "high"}, DefaultReasoningEffort: "low",
		},
		{
			ID: "openai/gpt-5.6-sol", Label: "GPT-5.6 Sol",
			ReasoningEfforts: []string{"medium"}, DefaultReasoningEffort: "medium",
		},
	}
	if _, err := resolver.ProviderAccounts.PublishVerifiedSecret(
		ctx,
		"provider_account:openrouter:default",
		0,
		provider.AuthSecretInput,
		secret,
		profiles,
		time.Now(),
	); err != nil {
		t.Fatal(err)
	}

	status, err := resolver.onboardingStatus(ctx)
	if err != nil || status.IsUserOnboarded || len(status.Steps) != 1 {
		t.Fatalf("pre-selection status = %#v, %v", status, err)
	}
	setup, err := resolver.onboardingModelSetup(ctx, "provider_account:openrouter:default")
	if err != nil {
		t.Fatal(err)
	}
	if len(setup.Profiles) != 2 || len(setup.Recommendations) != 9 {
		t.Fatalf("model setup = %#v", setup)
	}
	for _, recommendation := range setup.Recommendations {
		if recommendation.DisabledReason != nil {
			t.Fatalf("disabled recommendation = %#v", recommendation)
		}
	}

	server := httptest.NewServer(NewHandler(resolver))
	t.Cleanup(server.Close)
	selection := map[string]any{"selectionMode": "NOEMA_RECOMMENDED", "fastMode": false}
	input := map[string]any{"providerAccountId": "provider_account:openrouter:default"}
	for _, field := range []string{
		"noema", "simpleTasks", "mediumTasks", "difficultTasks", "taskReviewer",
		"webFetchSummarizer", "toolProgressAudit", "actionReviewer", "memoryConsolidation",
	} {
		input[field] = selection
	}
	confirmed := postGraphQL(t, server.URL, `
mutation Confirm($input: ConfirmOnboardingModelSelectionsInput!) {
  confirmOnboardingModelSelections(input: $input) {
    isUserOnboarded
    steps { id status providerKind providerAccountId }
  }
}`, map[string]any{"input": input})
	if len(confirmed.Errors) != 0 {
		t.Fatalf("confirm errors = %#v", confirmed.Errors)
	}
	confirmation := confirmed.Data["confirmOnboardingModelSelections"].(map[string]any)
	if confirmation["isUserOnboarded"] != true {
		t.Fatalf("confirmation = %#v", confirmation)
	}

	ready := postGraphQL(t, server.URL, `
mutation Ensure { ensurePrimaryConversation { conversationId provider } }
`, nil)
	if len(ready.Errors) != 0 {
		t.Fatalf("ensure errors = %#v", ready.Errors)
	}
	conversation := ready.Data["ensurePrimaryConversation"].(map[string]any)
	if conversation["provider"] != "openrouter" || conversation["conversationId"] == "" {
		t.Fatalf("ensured conversation = %#v", conversation)
	}

	boot := postGraphQL(t, server.URL, `
query ChatBoot {
  localStatus { primaryAgentDisplayName }
  onboardingStatus {
    isUserOnboarded
    steps { id status providerKind providerAccountId accountKey displayName providerAccountStatus authMethod }
  }
  providerAccountCatalog { providerKind displayName preferredAuthMethod supportedAuthMethods }
  providerAccounts { providerAccountId providerKind displayName status isActive isDefault }
  primaryConversation { conversationId provider }
}`, nil)
	if len(boot.Errors) != 0 {
		t.Fatalf("Chat boot errors = %#v", boot.Errors)
	}
	primary := boot.Data["primaryConversation"].(map[string]any)
	if primary["conversationId"] != conversation["conversationId"] || primary["provider"] != "openrouter" {
		t.Fatalf("Chat boot primary = %#v", primary)
	}
}
