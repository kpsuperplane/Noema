package graphql

import (
	"encoding/json"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/adapter"
	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestLibraryAgentSelectionContinuesAccountSetup(t *testing.T) {
	resolver := readyAgentTestResolver(t)
	service, err := adapter.NewService(resolver.home, resolver.Store)
	if err != nil {
		t.Fatal(err)
	}
	resolver.SetAdapters(service)
	if err := service.SetOAuthCallback("http://localhost:3737/adapter/oauth/callback"); err != nil {
		t.Fatal(err)
	}
	ctx := auth.WithDesktopAccess(t.Context())
	management, err := resolver.adapterManagement(ctx)
	if err != nil || len(management.Library) != 2 || len(management.Definitions) != 0 {
		t.Fatal("library listing installed definitions", err)
	}
	for _, entry := range management.Library {
		input, _ := json.Marshal(map[string]string{"library_id": entry.ID, "expected_digest": entry.SemanticDigest})
		if _, success := service.ExecuteSetup(adapter.ConnectLibraryTool, input); !success {
			t.Fatal("agent selection failed")
		}
	}
	management, err = resolver.adapterManagement(ctx)
	if err != nil {
		t.Fatal(err)
	}
	reviews, accounts, clients := projectAdapterInterventions(management.Definitions, management.OauthState)
	if len(reviews) != 0 || len(accounts) != 0 || len(clients) != 1 {
		t.Fatal("selection did not request shared Google setup")
	}

	captured, err := resolver.captureTask(ctx, model.CaptureTaskInput{WorkspaceID: personalWorkspaceID, Title: "Connect mail", TaskDocument: "Connect Gmail", ExecutorAgentID: stringAddress("agent:task-executor"), ClientMutationID: "library-task"})
	if err != nil {
		t.Fatal(err)
	}
	_, err = resolver.queueTask(ctx, model.QueueTaskInput{TaskID: captured.Task.TaskID, ExpectedRevision: 1, ExpectedGeneration: 1, ClientMutationID: "library-queue"})
	if err != nil {
		t.Fatal(err)
	}
	_, run, found, err := resolver.Store.ClaimTaskExecution(ctx, time.Now())
	if err != nil || !found {
		t.Fatal("task was not claimed", err)
	}
	if err = resolver.Store.StartTaskExecution(ctx, run.ID, run.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	if err = resolver.Store.FinishTaskPlanning(ctx, run.ID, run.Generation, "simple", time.Now()); err != nil {
		t.Fatal(err)
	}
	_, run, found, err = resolver.Store.ClaimTaskExecution(ctx, time.Now())
	if err != nil || !found {
		t.Fatal("executor was not claimed", err)
	}
	if err = resolver.Store.StartTaskExecution(ctx, run.ID, run.Generation, time.Now()); err != nil {
		t.Fatal(err)
	}
	item := store.TaskRunItemInput{Kind: "tool_result", Status: "completed", Payload: map[string]any{"name": adapter.ConnectLibraryTool, "success": true, "result": map[string]any{"definition_id": management.Library[0].DefinitionID}}}
	if err = resolver.Store.AppendTaskRunItems(ctx, run.ID, run.Generation, []store.TaskRunItemInput{item}, store.TaskRunUsage{}, time.Now()); err != nil {
		t.Fatal(err)
	}
	taskDefinitions, err := resolver.taskLibraryDefinitions(ctx, captured.Task.TaskID, management.Definitions)
	if err != nil || len(taskDefinitions) != 1 || taskDefinitions[0].DefinitionID != management.Library[0].DefinitionID {
		t.Fatal("Task setup lost or crossed selections", err)
	}
	_, _, taskClients := projectAdapterInterventions(taskDefinitions, management.OauthState)
	if len(taskClients) != 1 {
		t.Fatal("Task selection has no account setup")
	}
	profile := management.OauthState.Profiles[0].ProfileDigest
	if _, err = service.ImportOAuthApplication(profile, nil, []byte(`{"installed":{"client_id":"library-test-client","client_secret":"library-test-secret"}}`), nil); err != nil {
		t.Fatal(err)
	}
	management, err = resolver.adapterManagement(ctx)
	if err != nil {
		t.Fatal(err)
	}
	reviews, accounts, clients = projectAdapterInterventions(management.Definitions, management.OauthState)
	if len(reviews) != 0 || len(clients) != 0 || len(accounts) != 1 {
		t.Fatal("application import did not continue to account consent")
	}
	setup := accounts[0].(*model.AdapterOauthAccountSetupIntervention)
	if len(setup.DependentDefinitions) != 2 || setup.NextAction.Kind != "add_account" {
		t.Fatal("compatible definitions did not share consent")
	}
	for _, entry := range management.Library {
		definition, err := resolver.connectAdapterLibrary(ctx, model.ConnectAdapterLibraryInput{LibraryID: entry.ID, ExpectedDigest: entry.SemanticDigest})
		if err != nil || definition.ConnectionCount != 0 {
			t.Fatal("selection granted account access")
		}
	}
	snapshot, _ := service.Snapshot()
	if len(snapshot.Definitions) != 2 || len(snapshot.Connections) != 0 {
		t.Fatal("selection duplicated definitions or created connections")
	}
	// A partial grant proceeds to policy, while additional accounts remain available.
	definition := snapshot.Definitions[0]
	oauth, err := service.OAuthSnapshot()
	if err != nil {
		t.Fatal(err)
	}
	grantID := "partial-grant"
	oauth.Grants = []adapter.OAuthGrant{{GrantID: grantID, ApplicationID: oauth.Applications[0].ApplicationID, Status: "active", AuthorityRevision: 2, GrantedScopes: []string{"https://www.googleapis.com/auth/gmail.readonly"}}}
	connection := adapter.Connection{ConnectionID: "partial-connection", SemanticDigest: definition.SemanticDigest, Status: "active", ConnectionRevision: 1, PolicyRevision: 1, Authentication: adapter.ConnectionAuthentication{Kind: "oauth_grant", GrantID: grantID}}
	state := adapter.ServiceSnapshot{Definitions: []adapter.Definition{definition}, Connections: []adapter.Connection{connection}}
	view := definitionModel(definition, state)
	projectOAuthDefinition(view, definition, state, oauth)
	hasAddAccount := false
	for _, action := range view.ConnectionActions {
		hasAddAccount = hasAddAccount || action.Kind == "add_account"
	}
	if !hasAddAccount {
		t.Fatal("connected API cannot add another account")
	}
	reviews, accounts, _ = projectAdapterInterventions([]*model.AdapterDefinition{view}, management.OauthState)
	if len(reviews) != 1 || len(accounts) != 0 {
		t.Fatal("partial consent requested more access before policy")
	}
	view.Connections[0].PolicyConfigured = true
	reviews, accounts, _ = projectAdapterInterventions([]*model.AdapterDefinition{view}, management.OauthState)
	if len(reviews) != 0 || len(accounts) != 0 {
		t.Fatal("partial consent repeated the access request")
	}

}
