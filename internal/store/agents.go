package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"
)

const (
	PrimaryAgentID      = "agent:primary"
	TaskExecutorAgentID = "agent:task-executor"
	TaskReviewerAgentID = "agent:task-reviewer"
)

var (
	ErrAgentNotFound               = errors.New("Agent not found")
	ErrAcpAgentNotFound            = errors.New("ACP Agent not found")
	ErrAcpAgentRevisionConflict    = errors.New("ACP Agent revision conflict")
	ErrAcpAgentAuthenticationBusy  = errors.New("ACP Agent authentication is active")
	ErrInvalidAcpAgent             = errors.New("invalid ACP Agent")
	ErrAcpAuthenticationNotPending = errors.New("ACP authentication is not pending")
)

// Agent is one durable Agent identity.
type Agent struct {
	ID          string
	DisplayName *string
	SystemRole  *string
}

// AcpAgent is one configured local ACP process.
type AcpAgent struct {
	AgentID               string
	DisplayName           string
	Command               string
	Arguments             []string
	Enabled               bool
	AuthStatus            string
	HealthStatus          string
	ImplementationName    *string
	ImplementationVersion *string
	Capabilities          map[string]any
	ConnectionRevision    int64
	LastError             *string
}

const acpAgentSelect = `
SELECT a.agent_id, a.display_name, c.command, c.arguments_json, c.enabled,
       c.auth_status, c.health_status, c.implementation_name,
       c.implementation_version, c.capabilities_json,
       c.connection_revision, c.last_error
FROM acp_agents c JOIN agents a ON a.agent_id = c.agent_id`

// Agents returns all built-in and configured Agent identities.
func (s *Store) Agents(ctx context.Context) ([]Agent, error) {
	rows, err := s.db.QueryContext(ctx, `
SELECT agent_id, display_name, system_role FROM agents
ORDER BY CASE system_role
    WHEN 'primary' THEN 0 WHEN 'task_executor' THEN 1 WHEN 'task_reviewer' THEN 2 ELSE 3 END,
    display_name IS NULL, display_name, agent_id`)
	if err != nil {
		return nil, fmt.Errorf("query Agents: %w", err)
	}
	defer rows.Close()
	agents := make([]Agent, 0)
	for rows.Next() {
		agent, err := scanAgent(rows)
		if err != nil {
			return nil, err
		}
		agents = append(agents, agent)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("read Agents: %w", err)
	}
	return agents, nil
}

// Agent returns one Agent identity.
func (s *Store) Agent(ctx context.Context, id string) (Agent, error) {
	return scanAgent(s.db.QueryRowContext(ctx,
		"SELECT agent_id, display_name, system_role FROM agents WHERE agent_id = ?", id))
}

// CreateAcpAgent creates one custom Agent and its process configuration.
func (s *Store) CreateAcpAgent(
	ctx context.Context,
	displayName string,
	command string,
	arguments []string,
	now time.Time,
) (AcpAgent, error) {
	displayName, command, err := validateAcpConfiguration(displayName, command, arguments)
	if err != nil {
		return AcpAgent{}, err
	}
	id, err := newID("agent")
	if err != nil {
		return AcpAgent{}, err
	}
	if arguments == nil {
		arguments = []string{}
	}
	encoded, _ := json.Marshal(arguments)
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return AcpAgent{}, fmt.Errorf("begin ACP Agent creation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	stamp := millis(now.UTC())
	if _, err := tx.ExecContext(ctx, `
INSERT INTO agents (agent_id, display_name, created_at_ms, updated_at_ms)
VALUES (?, ?, ?, ?)`, id, displayName, stamp, stamp); err != nil {
		return AcpAgent{}, fmt.Errorf("create ACP Agent identity: %w", err)
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO acp_agents (agent_id, command, arguments_json, created_at_ms, updated_at_ms)
VALUES (?, ?, ?, ?, ?)`, id, command, string(encoded), stamp, stamp); err != nil {
		return AcpAgent{}, fmt.Errorf("create ACP Agent configuration: %w", err)
	}
	created, err := scanAcpAgent(tx.QueryRowContext(ctx, acpAgentSelect+" WHERE a.agent_id = ?", id))
	if err != nil {
		return AcpAgent{}, err
	}
	if err := tx.Commit(); err != nil {
		return AcpAgent{}, fmt.Errorf("commit ACP Agent creation: %w", err)
	}
	return created, nil
}

// AcpAgents returns configured ACP Agents in stable display order.
func (s *Store) AcpAgents(ctx context.Context) ([]AcpAgent, error) {
	rows, err := s.db.QueryContext(ctx, acpAgentSelect+" ORDER BY a.display_name, a.agent_id")
	if err != nil {
		return nil, fmt.Errorf("query ACP Agents: %w", err)
	}
	defer rows.Close()
	agents := make([]AcpAgent, 0)
	for rows.Next() {
		agent, err := scanAcpAgent(rows)
		if err != nil {
			return nil, err
		}
		agents = append(agents, agent)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("read ACP Agents: %w", err)
	}
	return agents, nil
}

// AcpAgent returns one configured ACP Agent.
func (s *Store) AcpAgent(ctx context.Context, id string) (AcpAgent, error) {
	return scanAcpAgent(s.db.QueryRowContext(ctx, acpAgentSelect+" WHERE a.agent_id = ?", id))
}

// UpdateAcpAgent replaces editable process configuration at one revision.
func (s *Store) UpdateAcpAgent(
	ctx context.Context,
	id string,
	expectedRevision int64,
	displayName string,
	command string,
	arguments []string,
	enabled bool,
	now time.Time,
) (AcpAgent, error) {
	if expectedRevision <= 0 {
		return AcpAgent{}, ErrAcpAgentRevisionConflict
	}
	displayName, command, err := validateAcpConfiguration(displayName, command, arguments)
	if err != nil {
		return AcpAgent{}, err
	}
	if arguments == nil {
		arguments = []string{}
	}
	encoded, _ := json.Marshal(arguments)
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return AcpAgent{}, fmt.Errorf("begin ACP Agent update: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	result, err := tx.ExecContext(ctx, `
UPDATE acp_agents SET command = ?, arguments_json = ?, enabled = ?,
    auth_status = 'unknown', health_status = 'unknown',
    implementation_name = NULL, implementation_version = NULL,
    capabilities_json = '{}', connection_revision = connection_revision + 1,
    last_error = NULL, updated_at_ms = ?
WHERE agent_id = ? AND connection_revision = ?`,
		command, string(encoded), enabled, millis(now.UTC()), id, expectedRevision)
	if err != nil {
		return AcpAgent{}, fmt.Errorf("update ACP Agent: %w", err)
	}
	changed, _ := result.RowsAffected()
	if changed != 1 {
		return AcpAgent{}, classifyAcpRevision(ctx, tx, id)
	}
	if _, err := tx.ExecContext(ctx,
		"UPDATE agents SET display_name = ?, updated_at_ms = ? WHERE agent_id = ?",
		displayName, millis(now.UTC()), id); err != nil {
		return AcpAgent{}, fmt.Errorf("update ACP Agent identity: %w", err)
	}
	updated, err := scanAcpAgent(tx.QueryRowContext(ctx, acpAgentSelect+" WHERE a.agent_id = ?", id))
	if err != nil {
		return AcpAgent{}, err
	}
	if err := tx.Commit(); err != nil {
		return AcpAgent{}, fmt.Errorf("commit ACP Agent update: %w", err)
	}
	return updated, nil
}

// DeleteAcpAgent deletes one unreferenced configuration revision.
func (s *Store) DeleteAcpAgent(ctx context.Context, id string, expectedRevision int64) (bool, error) {
	if expectedRevision <= 0 {
		return false, ErrAcpAgentRevisionConflict
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return false, fmt.Errorf("begin ACP Agent deletion: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	var revision int64
	if err := tx.QueryRowContext(ctx,
		"SELECT connection_revision FROM acp_agents WHERE agent_id = ?", id).Scan(&revision); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return false, nil
		}
		return false, fmt.Errorf("read ACP Agent revision: %w", err)
	}
	if revision != expectedRevision {
		return false, ErrAcpAgentRevisionConflict
	}
	var pending bool
	if err := tx.QueryRowContext(ctx, `
SELECT EXISTS(SELECT 1 FROM acp_auth_attempts WHERE agent_id = ? AND state = 'pending')`, id).Scan(&pending); err != nil {
		return false, fmt.Errorf("check ACP authentication: %w", err)
	}
	if pending {
		return false, ErrAcpAgentAuthenticationBusy
	}
	if _, err := tx.ExecContext(ctx, "DELETE FROM acp_agents WHERE agent_id = ?", id); err != nil {
		return false, fmt.Errorf("delete ACP Agent configuration: %w", err)
	}
	result, err := tx.ExecContext(ctx,
		"DELETE FROM agents WHERE agent_id = ? AND system_role IS NULL", id)
	if err != nil {
		return false, fmt.Errorf("delete ACP Agent identity: %w", err)
	}
	changed, _ := result.RowsAffected()
	if changed != 1 {
		return false, errors.New("ACP Agent identity is protected")
	}
	if err := tx.Commit(); err != nil {
		return false, fmt.Errorf("commit ACP Agent deletion: %w", err)
	}
	return true, nil
}

// RecordAcpAgentProbe saves safe initialization metadata at one revision.
func (s *Store) RecordAcpAgentProbe(
	ctx context.Context,
	id string,
	expectedRevision int64,
	healthStatus string,
	authStatus string,
	implementationName *string,
	implementationVersion *string,
	capabilities map[string]any,
	lastError *string,
	now time.Time,
) (AcpAgent, error) {
	if expectedRevision <= 0 || !validAcpHealth(healthStatus) || !validAcpAuth(authStatus) {
		return AcpAgent{}, ErrInvalidAcpAgent
	}
	implementationName = boundedOptional(implementationName, 256)
	implementationVersion = boundedOptional(implementationVersion, 256)
	lastError = boundedOptional(lastError, 512)
	if capabilities == nil {
		capabilities = map[string]any{}
	}
	encoded, err := json.Marshal(capabilities)
	if err != nil {
		return AcpAgent{}, fmt.Errorf("encode ACP capabilities: %w", err)
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return AcpAgent{}, fmt.Errorf("begin ACP Agent probe update: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	result, err := tx.ExecContext(ctx, `
UPDATE acp_agents SET health_status = ?, auth_status = ?, implementation_name = ?,
    implementation_version = ?, capabilities_json = ?, last_error = ?, updated_at_ms = ?
WHERE agent_id = ? AND connection_revision = ?`, healthStatus, authStatus,
		implementationName, implementationVersion, string(encoded), lastError,
		millis(now.UTC()), id, expectedRevision)
	if err != nil {
		return AcpAgent{}, fmt.Errorf("record ACP Agent probe: %w", err)
	}
	changed, _ := result.RowsAffected()
	if changed != 1 {
		return AcpAgent{}, classifyAcpRevision(ctx, tx, id)
	}
	updated, err := scanAcpAgent(tx.QueryRowContext(ctx, acpAgentSelect+" WHERE a.agent_id = ?", id))
	if err != nil {
		return AcpAgent{}, err
	}
	if err := tx.Commit(); err != nil {
		return AcpAgent{}, fmt.Errorf("commit ACP Agent probe: %w", err)
	}
	return updated, nil
}

// BeginAcpAuthentication stores one pending authentication attempt.
func (s *Store) BeginAcpAuthentication(
	ctx context.Context,
	agentID string,
	expectedRevision int64,
	methodID string,
	now time.Time,
) (string, error) {
	methodID = strings.TrimSpace(methodID)
	if expectedRevision <= 0 || methodID == "" || len(methodID) > 256 || strings.ContainsRune(methodID, 0) {
		return "", ErrInvalidAcpAgent
	}
	attemptID, err := newID("acp_auth")
	if err != nil {
		return "", err
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return "", fmt.Errorf("begin ACP authentication: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	var revision int64
	if err := tx.QueryRowContext(ctx,
		"SELECT connection_revision FROM acp_agents WHERE agent_id = ?", agentID).Scan(&revision); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return "", ErrAcpAgentNotFound
		}
		return "", fmt.Errorf("read ACP Agent: %w", err)
	}
	if revision != expectedRevision {
		return "", ErrAcpAgentRevisionConflict
	}
	var pending bool
	if err := tx.QueryRowContext(ctx, `
SELECT EXISTS(SELECT 1 FROM acp_auth_attempts WHERE agent_id = ? AND state = 'pending')`, agentID).Scan(&pending); err != nil {
		return "", fmt.Errorf("check ACP authentication: %w", err)
	}
	if pending {
		return "", ErrAcpAgentAuthenticationBusy
	}
	if _, err := tx.ExecContext(ctx, `
INSERT INTO acp_auth_attempts (
    attempt_id, agent_id, connection_revision, method_id, state, created_at_ms
) VALUES (?, ?, ?, ?, 'pending', ?)`,
		attemptID, agentID, expectedRevision, methodID, millis(now.UTC())); err != nil {
		return "", fmt.Errorf("store ACP authentication: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return "", fmt.Errorf("commit ACP authentication: %w", err)
	}
	return attemptID, nil
}

// FinishAcpAuthentication saves only safe attempt status and diagnostics.
func (s *Store) FinishAcpAuthentication(
	ctx context.Context,
	attemptID string,
	succeeded bool,
	safeMessage *string,
	now time.Time,
) (AcpAgent, error) {
	safeMessage = boundedOptional(safeMessage, 512)
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return AcpAgent{}, fmt.Errorf("begin ACP authentication completion: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	var agentID string
	var revision int64
	if err := tx.QueryRowContext(ctx, `
SELECT agent_id, connection_revision FROM acp_auth_attempts
WHERE attempt_id = ? AND state = 'pending'`, attemptID).Scan(&agentID, &revision); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return AcpAgent{}, ErrAcpAuthenticationNotPending
		}
		return AcpAgent{}, fmt.Errorf("read ACP authentication: %w", err)
	}
	state, authStatus := "failed", "failed"
	if succeeded {
		state, authStatus, safeMessage = "completed", "authenticated", nil
	}
	stamp := millis(now.UTC())
	result, err := tx.ExecContext(ctx, `
UPDATE acp_auth_attempts SET state = ?, safe_message = ?, completed_at_ms = ?
WHERE attempt_id = ? AND state = 'pending'`, state, safeMessage, stamp, attemptID)
	if err != nil {
		return AcpAgent{}, fmt.Errorf("finish ACP authentication: %w", err)
	}
	changed, _ := result.RowsAffected()
	if changed != 1 {
		return AcpAgent{}, ErrAcpAuthenticationNotPending
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE acp_agents SET auth_status = ?, last_error = ?, updated_at_ms = ?
WHERE agent_id = ? AND connection_revision = ?`,
		authStatus, safeMessage, stamp, agentID, revision); err != nil {
		return AcpAgent{}, fmt.Errorf("update ACP Agent authentication: %w", err)
	}
	updated, err := scanAcpAgent(tx.QueryRowContext(ctx, acpAgentSelect+" WHERE a.agent_id = ?", agentID))
	if err != nil {
		return AcpAgent{}, err
	}
	if err := tx.Commit(); err != nil {
		return AcpAgent{}, fmt.Errorf("commit ACP authentication completion: %w", err)
	}
	return updated, nil
}

func scanAgent(row rowScanner) (Agent, error) {
	var agent Agent
	var displayName, systemRole sql.NullString
	if err := row.Scan(&agent.ID, &displayName, &systemRole); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return Agent{}, ErrAgentNotFound
		}
		return Agent{}, fmt.Errorf("scan Agent: %w", err)
	}
	if displayName.Valid {
		agent.DisplayName = &displayName.String
	}
	if systemRole.Valid {
		agent.SystemRole = &systemRole.String
	}
	return agent, nil
}

func scanAcpAgent(row rowScanner) (AcpAgent, error) {
	var agent AcpAgent
	var arguments, capabilities string
	var enabled int
	var implementationName, implementationVersion, lastError sql.NullString
	if err := row.Scan(&agent.AgentID, &agent.DisplayName, &agent.Command, &arguments,
		&enabled, &agent.AuthStatus, &agent.HealthStatus, &implementationName,
		&implementationVersion, &capabilities, &agent.ConnectionRevision, &lastError); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return AcpAgent{}, ErrAcpAgentNotFound
		}
		return AcpAgent{}, fmt.Errorf("scan ACP Agent: %w", err)
	}
	if err := json.Unmarshal([]byte(arguments), &agent.Arguments); err != nil {
		return AcpAgent{}, fmt.Errorf("decode ACP arguments: %w", err)
	}
	if err := json.Unmarshal([]byte(capabilities), &agent.Capabilities); err != nil {
		return AcpAgent{}, fmt.Errorf("decode ACP capabilities: %w", err)
	}
	agent.Enabled = enabled == 1
	if implementationName.Valid {
		agent.ImplementationName = &implementationName.String
	}
	if implementationVersion.Valid {
		agent.ImplementationVersion = &implementationVersion.String
	}
	if lastError.Valid {
		agent.LastError = &lastError.String
	}
	return agent, nil
}

func validateAcpConfiguration(displayName, command string, arguments []string) (string, string, error) {
	displayName, command = strings.TrimSpace(displayName), strings.TrimSpace(command)
	if displayName == "" || len(displayName) > 128 || command == "" || len(command) > 4096 ||
		strings.ContainsRune(displayName, 0) || strings.ContainsRune(command, 0) || len(arguments) > 128 {
		return "", "", ErrInvalidAcpAgent
	}
	for _, argument := range arguments {
		if len(argument) > 4096 || strings.ContainsRune(argument, 0) {
			return "", "", ErrInvalidAcpAgent
		}
	}
	return displayName, command, nil
}

func classifyAcpRevision(ctx context.Context, tx *sql.Tx, id string) error {
	var exists bool
	if err := tx.QueryRowContext(ctx,
		"SELECT EXISTS(SELECT 1 FROM acp_agents WHERE agent_id = ?)", id).Scan(&exists); err != nil {
		return fmt.Errorf("check ACP Agent: %w", err)
	}
	if !exists {
		return ErrAcpAgentNotFound
	}
	return ErrAcpAgentRevisionConflict
}

func validAcpHealth(value string) bool {
	return value == "unknown" || value == "healthy" || value == "unavailable"
}

func validAcpAuth(value string) bool {
	switch value {
	case "unknown", "none", "required", "authenticated", "failed":
		return true
	default:
		return false
	}
}

func boundedOptional(value *string, limit int) *string {
	if value == nil {
		return nil
	}
	bounded := string([]rune(*value)[:min(len([]rune(*value)), limit)])
	return &bounded
}
