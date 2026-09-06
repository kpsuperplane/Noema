package main

import (
	"context"
	"fmt"
	"io"
	"strconv"

	"github.com/urfave/cli/v3"
)

// controlCommands keeps the common inspection and decision operations short.
// It never guesses which request to approve: every decision requires the
// exact identifier and revision returned by a preceding read.
func controlCommands(input io.Reader, action clientAction) *cli.Command {
	return &cli.Command{Name: "connections", Usage: "Inspect registered API and MCP connections", Commands: []*cli.Command{
		{Name: "list", Usage: "List provider definitions and MCP servers", Action: action(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 0); err != nil {
				return err
			}
			return l.printQuery(ctx, `query Connections {
				adapterDefinitions {
					semanticDigest definitionId adapterId displayName definitionRevision sourceReference origin authenticationMode oauthProfileDigest scopes reviewed superseded manifestJson connectionCount
					operations { operationId method path readOnly idempotent destructive openWorld argumentNames }
					connections { connectionId status grantId accountId connectionRevision credentialRevision grantRevision policyRevision grantedScopes allowedOperations policyConfigured }
				}
				mcpServers { mcpServerId connectionRevision policyRevision displayName transportKind healthStatus authStatus toolCount pendingToolCount }
			}`, nil)
		})},
	}}
}

func proposalCommands(input io.Reader, action clientAction) *cli.Command {
	return &cli.Command{Name: "proposals", Usage: "Inspect and accept exact adapter proposals", Commands: []*cli.Command{
		{Name: "list", Usage: "List pending adapter definitions", Action: action(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 0); err != nil {
				return err
			}
			return l.printQuery(ctx, `query Proposals {
				adapterDefinitions { semanticDigest definitionId adapterId displayName definitionRevision sourceReference origin authenticationMode oauthProfileDigest scopes reviewed superseded manifestJson operations { operationId method path readOnly idempotent destructive openWorld argumentNames } }
			}`, nil)
		})},
		{Name: "accept", Usage: "Accept one exact pending adapter definition", ArgsUsage: "SEMANTIC_DIGEST", Action: action(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 1); err != nil {
				return err
			}
			return l.printQuery(ctx, `mutation AcceptProposal($input: ApproveAdapterDefinitionInput!) {
				approveAdapterDefinition(input: $input) { semanticDigest definitionId definitionRevision reviewed superseded manifestJson }
			}`, map[string]any{"input": map[string]any{"semanticDigest": c.Args().First()}})
		})},
	}}
}

func interventionCommands(input io.Reader, action clientAction) *cli.Command {
	return &cli.Command{Name: "interventions", Usage: "Inspect pending human decisions and setup requests", Commands: []*cli.Command{
		{Name: "list", Usage: "List all pending interventions", Flags: []cli.Flag{stringFlag("conversation", "Limit to one conversation"), stringFlag("task", "Limit to one Task"), &cli.IntFlag{Name: "first", Value: 100}}, Action: action(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 0); err != nil {
				return err
			}
			variables := map[string]any{"first": c.Int("first")}
			if c.IsSet("conversation") {
				variables["conversationId"] = c.String("conversation")
			}
			if c.IsSet("task") {
				variables["taskId"] = c.String("task")
			}
			return l.printQuery(ctx, `query Interventions($conversationId: String, $taskId: String, $first: Int) {
				pendingHumanInterventions(conversationId: $conversationId, taskId: $taskId, first: $first) {
					__typename
					... on GovernedAction { actionId revision conversationId taskId capabilityName reviewRoute safeSummary consequence arguments actionState: state }
					... on AdapterDefinition { semanticDigest definitionId adapterId displayName definitionRevision origin reviewed superseded }
					... on AdapterAuthenticationIntervention { requestId revision taskId serviceDisplayName capabilityName adapterState: state failureCode }
					... on McpAuthenticationIntervention { requestId revision taskId serverDisplayName capabilityName mcpState: state failureCode }
					... on McpSetupIntervention { itemId setupStatus displayName serviceUrl endpointUrl oauthSupported discoveredToolCount mcpServerId connectionRevision policyRevision toolCount }
				}
			}`, variables)
		})},
	}}
}

func actionCommands(input io.Reader, action clientAction) *cli.Command {
	return &cli.Command{Name: "actions", Usage: "Inspect and resolve governed actions", Commands: []*cli.Command{
		{Name: "list", Usage: "List pending governed actions", Flags: []cli.Flag{stringFlag("conversation", "Limit to one conversation"), stringFlag("task", "Limit to one Task"), &cli.IntFlag{Name: "first", Value: 100}}, Action: action(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 0); err != nil {
				return err
			}
			variables := map[string]any{"first": c.Int("first")}
			if c.IsSet("conversation") {
				variables["conversationId"] = c.String("conversation")
			}
			if c.IsSet("task") {
				variables["taskId"] = c.String("task")
			}
			return l.printQuery(ctx, `query Actions($conversationId: String, $taskId: String, $first: Int) {
				pendingGovernedActions(conversationId: $conversationId, taskId: $taskId, first: $first) {
					actionId revision conversationId taskId runId capabilityName reviewRoute safeSummary target { serviceName connectionLabel serviceId connectionId accountId }
					disclosure { recipient contentSummary }
					consequence destination arguments state output failureCode
					behavior { readOnly idempotent destructive openWorld }
					assessment { status authorization risk reasonCodes explanation }
				}
			}`, variables)
		})},
		{Name: "resolve", Usage: "Approve or decline one exact action revision", ArgsUsage: "ACTION_ID REVISION DECISION", Action: action(func(ctx context.Context, c *cli.Command, l *localClient) error {
			if err := argumentCount(c, 3); err != nil {
				return err
			}
			revision, err := strconv.Atoi(c.Args().Get(1))
			if err != nil || revision < 1 {
				return fmt.Errorf("action revision must be a positive integer")
			}
			decision := c.Args().Get(2)
			if decision != "APPROVE" && decision != "DECLINE" {
				return fmt.Errorf("decision must be APPROVE or DECLINE")
			}
			return l.printQuery(ctx, `mutation ResolveAction($input: ResolveGovernedActionInput!) {
				resolveGovernedAction(input: $input) { actionId revision state capabilityName output failureCode }
			}`, map[string]any{"input": map[string]any{"actionId": c.Args().First(), "expectedRevision": revision, "decision": decision}})
		})},
	}}
}
