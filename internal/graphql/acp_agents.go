package graphql

import (
	"context"
	"errors"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/acp"
	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) acpAgents(ctx context.Context) ([]*model.AcpAgent, error) {
	agents, err := r.Store.AcpAgents(ctx)
	if err != nil {
		return nil, err
	}
	result := make([]*model.AcpAgent, 0, len(agents))
	for _, agent := range agents {
		result = append(result, acpAgentModel(agent))
	}
	return result, nil
}

func (r *Resolver) createAcpAgent(ctx context.Context, input model.CreateAcpAgentInput) (*model.AcpAgent, error) {
	agent, err := r.Store.CreateAcpAgent(ctx, input.DisplayName, input.Command, input.Arguments, time.Now())
	if err != nil {
		return nil, err
	}
	return acpAgentModel(agent), nil
}

func (r *Resolver) updateAcpAgent(ctx context.Context, input model.UpdateAcpAgentInput) (*model.AcpAgent, error) {
	agent, err := r.Store.UpdateAcpAgent(ctx, input.AgentID, int64(input.ExpectedRevision),
		input.DisplayName, input.Command, input.Arguments, input.Enabled, time.Now())
	if err != nil {
		return nil, err
	}
	return acpAgentModel(agent), nil
}

func (r *Resolver) deleteAcpAgent(ctx context.Context, input model.DeleteAcpAgentInput) (bool, error) {
	return r.Store.DeleteAcpAgent(ctx, input.AgentID, int64(input.ExpectedRevision))
}

func (r *Resolver) testAcpAgent(ctx context.Context, input model.TestAcpAgentInput) (*model.AcpAgent, error) {
	agent, err := r.requireAcpAgent(ctx, input.AgentID, int64(input.ExpectedRevision))
	if err != nil {
		return nil, err
	}
	probe, probeErr := acp.Probe(ctx, acp.Command{Path: agent.Command, Args: agent.Arguments})
	health, authStatus := "healthy", "none"
	capabilities := probe.Capabilities
	name, version := probe.ImplementationName, probe.ImplementationVersion
	var lastError *string
	if probeErr != nil {
		health, authStatus = "unavailable", "unknown"
		capabilities = map[string]any{}
		name, version = nil, nil
		message := "ACP initialization failed"
		if errors.Is(probeErr, acp.ErrInitializationTimedOut) {
			message = "ACP initialization timed out"
		}
		lastError = &message
	} else if methods, ok := capabilities["authMethods"].([]any); ok && len(methods) > 0 {
		authStatus = "required"
	}
	storeCtx, cancelStore := context.WithTimeout(context.WithoutCancel(ctx), 5*time.Second)
	defer cancelStore()
	updated, err := r.Store.RecordAcpAgentProbe(storeCtx, agent.AgentID, agent.ConnectionRevision,
		health, authStatus, name, version, capabilities, lastError, time.Now())
	if err != nil {
		return nil, err
	}
	return acpAgentModel(updated), nil
}

func (r *Resolver) authenticateAcpAgent(
	ctx context.Context,
	input model.AuthenticateAcpAgentInput,
) (*model.AcpAgent, error) {
	agent, err := r.requireAcpAgent(ctx, input.AgentID, int64(input.ExpectedRevision))
	if err != nil {
		return nil, err
	}
	attemptID, err := r.Store.BeginAcpAuthentication(ctx, agent.AgentID,
		agent.ConnectionRevision, input.MethodID, time.Now())
	if err != nil {
		return nil, err
	}
	authCtx, cancel := context.WithTimeout(ctx, 5*time.Minute)
	defer cancel()
	authErr := acp.Authenticate(authCtx,
		acp.Command{Path: agent.Command, Args: agent.Arguments}, input.MethodID)
	var safeMessage *string
	if authErr != nil {
		message := "ACP authentication failed"
		if errors.Is(authCtx.Err(), context.DeadlineExceeded) {
			message = "ACP authentication timed out"
		}
		safeMessage = &message
	}
	storeCtx, cancelStore := context.WithTimeout(context.WithoutCancel(ctx), 5*time.Second)
	defer cancelStore()
	updated, err := r.Store.FinishAcpAuthentication(storeCtx, attemptID, authErr == nil, safeMessage, time.Now())
	if err != nil {
		return nil, err
	}
	return acpAgentModel(updated), nil
}

func (r *Resolver) requireAcpAgent(ctx context.Context, id string, revision int64) (store.AcpAgent, error) {
	agent, err := r.Store.AcpAgent(ctx, id)
	if err != nil {
		return store.AcpAgent{}, err
	}
	if revision <= 0 || agent.ConnectionRevision != revision {
		return store.AcpAgent{}, store.ErrAcpAgentRevisionConflict
	}
	return agent, nil
}

func acpAgentModel(agent store.AcpAgent) *model.AcpAgent {
	capabilities := agent.Capabilities
	if capabilities == nil {
		capabilities = map[string]any{}
	}
	return &model.AcpAgent{
		AgentID: agent.AgentID, DisplayName: agent.DisplayName, Command: agent.Command,
		Arguments: agent.Arguments, Enabled: agent.Enabled,
		AuthStatus:            model.AcpAgentAuthStatus(strings.ToUpper(agent.AuthStatus)),
		HealthStatus:          model.AcpAgentHealthStatus(strings.ToUpper(agent.HealthStatus)),
		ImplementationName:    agent.ImplementationName,
		ImplementationVersion: agent.ImplementationVersion,
		Capabilities:          capabilities, ConnectionRevision: int(agent.ConnectionRevision),
		LastError: agent.LastError,
	}
}
