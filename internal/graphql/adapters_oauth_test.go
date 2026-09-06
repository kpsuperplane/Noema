package graphql

import (
	"testing"

	"github.com/kpsuperplane/noema/internal/adapter"
	"github.com/kpsuperplane/noema/internal/graphql/model"
)

func TestOAuthNextActionUsesMatchingApplicationAndGrantState(t *testing.T) {
	definition := adapter.Definition{
		Manifest:       adapter.Manifest{Reviewed: true, Authentication: adapter.Authentication{Kind: "oauth2_authorization_code_pkce", ProfileDigest: "profile-current"}},
		SemanticDigest: "semantic-current",
		Operations:     []adapter.CompiledOperation{{Operation: adapter.Operation{OperationID: "lookup", Authorization: adapter.Authorization{Kind: "oauth_scopes", AcceptedScopeSets: [][]string{{"scope.read", "scope.write"}}}}}},
	}
	applications := []adapter.OAuthApplication{
		{ApplicationID: "application-current", ProfileDigest: "profile-current", Revision: 3, Status: "active"},
		{ApplicationID: "application-other", ProfileDigest: "profile-other", Revision: 9, Status: "active"},
	}
	for _, test := range []struct {
		name, status, wantKind string
		granted, missing       []string
	}{
		{name: "sufficient", status: "active", wantKind: "attach_account", granted: []string{"scope.read", "scope.write"}},
		{name: "expansion", status: "active", wantKind: "add_access", granted: []string{"scope.read"}, missing: []string{"scope.write"}},
		{name: "revoked", status: "revoked", wantKind: "reconnect_account", granted: []string{"scope.read", "scope.write"}},
	} {
		t.Run(test.name, func(t *testing.T) {
			view := &model.AdapterDefinition{Scopes: []string{"scope.read", "scope.write"}, Connections: []*model.AdapterConnection{}}
			oauth := adapter.OAuthSnapshot{Applications: applications, Grants: []adapter.OAuthGrant{
				{GrantID: "grant-other", ApplicationID: "application-other", AuthorityRevision: 2, Status: "active", GrantedScopes: []string{"scope.read", "scope.write"}},
				{GrantID: "grant-current", ApplicationID: "application-current", AuthorityRevision: 4, Status: test.status, GrantedScopes: test.granted},
			}}
			projectOAuthDefinition(view, definition, adapter.ServiceSnapshot{}, oauth)
			if view.NextAction == nil || view.NextAction.Kind != test.wantKind || view.NextAction.ApplicationID == nil || *view.NextAction.ApplicationID != "application-current" || view.NextAction.GrantID == nil || *view.NextAction.GrantID != "grant-current" {
				t.Fatalf("next action = %#v", view.NextAction)
			}
			if len(view.ConnectionActions) == 0 || view.ConnectionActions[0] != view.NextAction {
				t.Fatalf("connection actions = %#v", view.ConnectionActions)
			}
			if len(view.NextAction.MissingScopes) != len(test.missing) || len(test.missing) != 0 && view.NextAction.MissingScopes[0] != test.missing[0] {
				t.Fatalf("missing scopes = %#v", view.NextAction.MissingScopes)
			}
		})
	}
}

func TestOAuthNextActionReconnectsPendingConnection(t *testing.T) {
	definition := adapter.Definition{
		Manifest:       adapter.Manifest{Reviewed: true, Authentication: adapter.Authentication{Kind: "oauth2_authorization_code_pkce", ProfileDigest: "profile-current"}},
		SemanticDigest: "semantic-current",
		Operations:     []adapter.CompiledOperation{{Operation: adapter.Operation{OperationID: "lookup"}}},
	}
	view := &model.AdapterDefinition{Scopes: []string{}, Connections: []*model.AdapterConnection{}}
	snapshot := adapter.ServiceSnapshot{Connections: []adapter.Connection{{
		ConnectionID: "connection-pending", SemanticDigest: definition.SemanticDigest, Status: "authentication_required", ConnectionRevision: 4, PolicyRevision: 2,
		AllowedOperations: []string{}, Authentication: adapter.ConnectionAuthentication{Kind: "pending"},
	}}}
	oauth := adapter.OAuthSnapshot{Applications: []adapter.OAuthApplication{{ApplicationID: "application-current", ProfileDigest: "profile-current", Revision: 3, Status: "active"}}}
	projectOAuthDefinition(view, definition, snapshot, oauth)
	if view.NextAction == nil || view.NextAction.Kind != "reconnect_account" || view.NextAction.ApplicationID == nil || *view.NextAction.ApplicationID != "application-current" || view.NextAction.ConnectionID == nil || *view.NextAction.ConnectionID != "connection-pending" || view.NextAction.ExpectedConnectionRevision == nil || *view.NextAction.ExpectedConnectionRevision != 4 || len(view.NextAction.OperationIds) != 1 || view.NextAction.OperationIds[0] != "lookup" {
		t.Fatalf("pending connection action = %#v", view.NextAction)
	}
	if view.NextAction.GrantID != nil || len(view.NextAction.MissingScopes) != 0 {
		t.Fatalf("pending connection action contains grant state = %#v", view.NextAction)
	}
}
