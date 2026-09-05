package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/uptrace/bun"
)

// MCPDefinition is one reusable non-secret transport definition.
type MCPDefinition struct {
	ID, Revision, DisplayName, TransportKind string
	SafeConfig                               json.RawMessage
}

// MCPServer is one concrete MCP connection and its current authority.
type MCPServer struct {
	ID, DefinitionID, DefinitionRevision, DisplayName, TransportKind string
	ConnectionLabel, ServiceDescription                              string
	SafeConfig                                                       json.RawMessage
	ConnectionRevision, SecretRevision                               string
	Enabled                                                          bool
	DataSharingPolicy, UnsafeActionPolicy                            string
	PolicyRevision                                                   int
	HealthStatus, AuthStatus                                         string
	ToolCount, AvailableToolCount, PendingToolCount                  int
	DefaultedToolCount, DisabledToolCount                            int
}

// MCPHint is one behavior value and its source.
type MCPHint struct {
	Value  *bool
	Source string
}

// MCPTool is one exact discovered tool and its behavior policy.
type MCPTool struct {
	ID, ServerID, Name, Description, SourceRevision string
	InputSchema, OutputSchema, Annotations          json.RawMessage
	ReadOnly, Idempotent, Destructive, OpenWorld    MCPHint
	Status                                          string
	PolicyRevision                                  int
}

// NewMCPConnection is one verified catalog ready for atomic publication.
type NewMCPConnection struct {
	Definition                         MCPDefinition
	ReuseDefinition                    bool
	ServerID                           string
	ConnectionLabel                    string
	ConnectionRevision, SecretRevision string
	ServiceDescription, AuthStatus     string
	Tools                              []MCPTool
}

// MCPInvocation is one joined authority snapshot.
type MCPInvocation struct {
	Server MCPServer
	Tool   MCPTool
}

// MCPDefinition returns one reusable definition.
func (s *Store) MCPDefinition(ctx context.Context, id string) (MCPDefinition, error) {
	var value MCPDefinition
	var safeConfig string
	err := s.db.QueryRowContext(ctx, `SELECT mcp_definition_id, definition_revision,
 display_name, transport_kind, safe_config_json FROM mcp_definitions WHERE mcp_definition_id=?`, id).Scan(
		&value.ID, &value.Revision, &value.DisplayName, &value.TransportKind, &safeConfig)
	if err != nil {
		return MCPDefinition{}, errors.New("MCP definition not found")
	}
	value.SafeConfig = json.RawMessage(safeConfig)
	return value, nil
}

const mcpServerSelect = `
SELECT s.mcp_server_id, d.mcp_definition_id, d.definition_revision, d.display_name,
       d.transport_kind, d.safe_config_json, COALESCE(s.connection_label, ''),
       s.connection_revision, COALESCE(s.secret_revision, ''),
       COALESCE(s.service_description, ''), s.enabled,
       COALESCE(s.data_sharing_policy, ''), COALESCE(s.unsafe_action_policy, ''),
       s.policy_revision, s.health_status, s.auth_status,
       COUNT(t.mcp_tool_id),
       COALESCE(SUM(CASE WHEN t.status IN ('ready','defaulted') THEN 1 ELSE 0 END), 0),
       0,
       COALESCE(SUM(CASE WHEN t.status = 'defaulted' THEN 1 ELSE 0 END), 0),
       COALESCE(SUM(CASE WHEN t.status = 'disabled' THEN 1 ELSE 0 END), 0)
FROM mcp_servers s
JOIN mcp_definitions d ON d.mcp_definition_id = s.mcp_definition_id
LEFT JOIN mcp_tools t ON t.mcp_server_id = s.mcp_server_id`

// CommitMCPConnection publishes one definition, connection, and catalog atomically.
func (s *Store) CommitMCPConnection(ctx context.Context, input NewMCPConnection, now time.Time) (MCPServer, error) {
	if err := validateNewMCPConnection(input); err != nil {
		return MCPServer{}, err
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return MCPServer{}, fmt.Errorf("begin MCP publication: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	if input.ReuseDefinition {
		var revision string
		if err := tx.QueryRowContext(ctx, `SELECT definition_revision FROM mcp_definitions WHERE mcp_definition_id=?`, input.Definition.ID).Scan(&revision); err != nil || revision != input.Definition.Revision {
			return MCPServer{}, errors.New("MCP definition revision changed")
		}
	} else if _, err := tx.ExecContext(ctx, `
INSERT INTO mcp_definitions (mcp_definition_id, definition_revision, display_name, transport_kind, safe_config_json, created_at_ms)
VALUES (?, ?, ?, ?, ?, ?)`, input.Definition.ID, input.Definition.Revision,
		input.Definition.DisplayName, input.Definition.TransportKind, string(input.Definition.SafeConfig), millis(now)); err != nil {
		return MCPServer{}, fmt.Errorf("create MCP definition: %w", err)
	}
	_, err = tx.ExecContext(ctx, `
INSERT INTO mcp_servers (mcp_server_id, mcp_definition_id, connection_label, connection_revision,
 secret_revision, service_description, health_status, auth_status, created_at_ms, updated_at_ms)
VALUES (?, ?, NULLIF(?, ''), ?, NULLIF(?, ''), NULLIF(?, ''), 'healthy', ?, ?, ?)`,
		input.ServerID, input.Definition.ID, strings.TrimSpace(input.ConnectionLabel),
		input.ConnectionRevision, input.SecretRevision, input.ServiceDescription, input.AuthStatus,
		millis(now), millis(now))
	if err != nil {
		return MCPServer{}, fmt.Errorf("create MCP server: %w", err)
	}
	for _, tool := range input.Tools {
		if err := insertMCPTool(ctx, tx, input.ServerID, tool, now); err != nil {
			return MCPServer{}, err
		}
	}
	server, err := mcpServerTx(ctx, tx, input.ServerID)
	if err != nil {
		return MCPServer{}, err
	}
	if err := tx.Commit(); err != nil {
		return MCPServer{}, fmt.Errorf("commit MCP publication: %w", err)
	}
	return server, nil
}

// ReconcileMCPConnection replaces one catalog after checking its connection authority.
func (s *Store) ReconcileMCPConnection(ctx context.Context, serverID, revision, secretRevision,
	authStatus string, tools []MCPTool, now time.Time) (MCPServer, error) {
	if len(tools) > 128 || !validMCPStatus(authStatus, "none", "authenticated") {
		return MCPServer{}, errors.New("invalid MCP discovery")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return MCPServer{}, err
	}
	defer func() { _ = tx.Rollback() }()
	rows, err := tx.QueryContext(ctx, mcpToolSelect+` WHERE mcp_server_id=?`, serverID)
	if err != nil {
		return MCPServer{}, err
	}
	current := make(map[string]MCPTool)
	for rows.Next() {
		tool, scanErr := scanMCPTool(rows)
		if scanErr != nil {
			_ = rows.Close()
			return MCPServer{}, scanErr
		}
		current[tool.Name] = tool
	}
	if err := rows.Close(); err != nil {
		return MCPServer{}, err
	}
	for index := range tools {
		previous, exists := current[tools[index].Name]
		if !exists {
			continue
		}
		tools[index].ID = previous.ID
		if previous.SourceRevision == tools[index].SourceRevision {
			tools[index].ReadOnly = previous.ReadOnly
			tools[index].Idempotent = previous.Idempotent
			tools[index].Destructive = previous.Destructive
			tools[index].OpenWorld = previous.OpenWorld
			tools[index].Status = previous.Status
			tools[index].PolicyRevision = previous.PolicyRevision
		}
	}
	result, err := tx.ExecContext(ctx, `UPDATE mcp_servers SET secret_revision=NULLIF(?, ''),
 auth_status=?, health_status='healthy', updated_at_ms=? WHERE mcp_server_id=? AND connection_revision=?`,
		secretRevision, authStatus, millis(now), serverID, revision)
	if err != nil {
		return MCPServer{}, fmt.Errorf("update MCP connection: %w", err)
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return MCPServer{}, errors.New("MCP connection revision changed")
	}
	if _, err := tx.ExecContext(ctx, `DELETE FROM mcp_tools WHERE mcp_server_id=?`, serverID); err != nil {
		return MCPServer{}, err
	}
	for _, tool := range tools {
		if err := insertMCPTool(ctx, tx, serverID, tool, now); err != nil {
			return MCPServer{}, err
		}
	}
	server, err := mcpServerTx(ctx, tx, serverID)
	if err != nil {
		return MCPServer{}, err
	}
	if err := tx.Commit(); err != nil {
		return MCPServer{}, err
	}
	return server, nil
}

func insertMCPTool(ctx context.Context, tx bun.Tx, serverID string, tool MCPTool, now time.Time) error {
	if tool.ServerID != "" && tool.ServerID != serverID {
		return errors.New("MCP tool server does not match")
	}
	_, err := tx.ExecContext(ctx, `
INSERT INTO mcp_tools (mcp_tool_id, mcp_server_id, name, description, input_schema_json,
 output_schema_json, annotations_json, source_revision, read_only, read_only_source,
 idempotent, idempotent_source, destructive, destructive_source, open_world, open_world_source,
 status, policy_revision, created_at_ms, updated_at_ms)
VALUES (?, ?, ?, NULLIF(?, ''), ?, NULLIF(?, ''), ?, ?, ?, NULLIF(?, ''), ?, NULLIF(?, ''),
 ?, NULLIF(?, ''), ?, NULLIF(?, ''), ?, ?, ?, ?)`, tool.ID, serverID, tool.Name, tool.Description,
		string(tool.InputSchema), nullableJSON(tool.OutputSchema), string(tool.Annotations), tool.SourceRevision,
		nullBool(tool.ReadOnly.Value), tool.ReadOnly.Source, nullBool(tool.Idempotent.Value), tool.Idempotent.Source,
		nullBool(tool.Destructive.Value), tool.Destructive.Source, nullBool(tool.OpenWorld.Value), tool.OpenWorld.Source,
		tool.Status, tool.PolicyRevision, millis(now), millis(now))
	if err != nil {
		return fmt.Errorf("store MCP tool: %w", err)
	}
	return nil
}

func nullableJSON(value json.RawMessage) any {
	if len(value) == 0 {
		return nil
	}
	return string(value)
}

// MCPServers returns all connections in stable display order.
func (s *Store) MCPServers(ctx context.Context) ([]MCPServer, error) {
	rows, err := s.db.QueryContext(ctx, mcpServerSelect+` GROUP BY s.mcp_server_id ORDER BY d.display_name, s.mcp_server_id`)
	if err != nil {
		return nil, fmt.Errorf("query MCP servers: %w", err)
	}
	defer rows.Close()
	result := make([]MCPServer, 0)
	for rows.Next() {
		value, err := scanMCPServer(rows)
		if err != nil {
			return nil, err
		}
		result = append(result, value)
	}
	return result, rows.Err()
}

// MCPServer returns one connection.
func (s *Store) MCPServer(ctx context.Context, id string) (MCPServer, error) {
	return scanMCPServer(s.db.QueryRowContext(ctx, mcpServerSelect+` WHERE s.mcp_server_id=? GROUP BY s.mcp_server_id`, id))
}

func mcpServerTx(ctx context.Context, tx bun.Tx, id string) (MCPServer, error) {
	return scanMCPServer(tx.QueryRowContext(ctx, mcpServerSelect+` WHERE s.mcp_server_id=? GROUP BY s.mcp_server_id`, id))
}

func scanMCPServer(row rowScanner) (MCPServer, error) {
	var v MCPServer
	var safeConfig string
	if err := row.Scan(&v.ID, &v.DefinitionID, &v.DefinitionRevision, &v.DisplayName,
		&v.TransportKind, &safeConfig, &v.ConnectionLabel, &v.ConnectionRevision,
		&v.SecretRevision, &v.ServiceDescription, &v.Enabled, &v.DataSharingPolicy,
		&v.UnsafeActionPolicy, &v.PolicyRevision, &v.HealthStatus, &v.AuthStatus,
		&v.ToolCount, &v.AvailableToolCount, &v.PendingToolCount, &v.DefaultedToolCount,
		&v.DisabledToolCount); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return MCPServer{}, errors.New("MCP server not found")
		}
		return MCPServer{}, fmt.Errorf("scan MCP server: %w", err)
	}
	v.SafeConfig = json.RawMessage(safeConfig)
	return v, nil
}

// MCPTools returns one server catalog in stable name order.
func (s *Store) MCPTools(ctx context.Context, serverID string) ([]MCPTool, error) {
	rows, err := s.db.QueryContext(ctx, mcpToolSelect+` WHERE mcp_server_id=? ORDER BY name`, serverID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	result := make([]MCPTool, 0)
	for rows.Next() {
		tool, err := scanMCPTool(rows)
		if err != nil {
			return nil, err
		}
		result = append(result, tool)
	}
	return result, rows.Err()
}

const mcpToolSelect = `SELECT mcp_tool_id, mcp_server_id, name, COALESCE(description, ''),
 input_schema_json, COALESCE(output_schema_json, ''), annotations_json, source_revision,
 read_only, COALESCE(read_only_source, ''), idempotent, COALESCE(idempotent_source, ''),
 destructive, COALESCE(destructive_source, ''), open_world, COALESCE(open_world_source, ''),
 status, policy_revision FROM mcp_tools`

func scanMCPTool(row rowScanner) (MCPTool, error) {
	var v MCPTool
	var readOnly, idempotent, destructive, openWorld sql.NullBool
	var input, output, annotations string
	if err := row.Scan(&v.ID, &v.ServerID, &v.Name, &v.Description, &input,
		&output, &annotations, &v.SourceRevision, &readOnly, &v.ReadOnly.Source,
		&idempotent, &v.Idempotent.Source, &destructive, &v.Destructive.Source,
		&openWorld, &v.OpenWorld.Source, &v.Status, &v.PolicyRevision); err != nil {
		return MCPTool{}, fmt.Errorf("scan MCP tool: %w", err)
	}
	v.InputSchema, v.OutputSchema = json.RawMessage(input), json.RawMessage(output)
	v.Annotations = json.RawMessage(annotations)
	v.ReadOnly.Value, v.Idempotent.Value = boolPointer(readOnly), boolPointer(idempotent)
	v.Destructive.Value, v.OpenWorld.Value = boolPointer(destructive), boolPointer(openWorld)
	return v, nil
}

// MCPInvocationSnapshot returns one exact joined call authority.
func (s *Store) MCPInvocationSnapshot(ctx context.Context, serverID, toolID string) (MCPInvocation, error) {
	server, err := s.MCPServer(ctx, serverID)
	if err != nil {
		return MCPInvocation{}, err
	}
	tool, err := scanMCPTool(s.db.QueryRowContext(ctx, mcpToolSelect+` WHERE mcp_server_id=? AND mcp_tool_id=?`, serverID, toolID))
	if err != nil {
		return MCPInvocation{}, err
	}
	return MCPInvocation{Server: server, Tool: tool}, nil
}

// SaveMCPConnectionPolicy applies one revision-fenced connection policy.
func (s *Store) SaveMCPConnectionPolicy(ctx context.Context, id, connectionRevision string,
	expected int, dataSharing, unsafeActions string, now time.Time) (MCPServer, error) {
	if !validMCPStatus(dataSharing, "allow_automatically", "review_every_call") ||
		!validMCPStatus(unsafeActions, "always_ask", "reviewer_may_approve", "never_ask") ||
		(dataSharing == "review_every_call" && unsafeActions == "never_ask") {
		return MCPServer{}, errors.New("invalid MCP connection policy")
	}
	result, err := s.db.ExecContext(ctx, `UPDATE mcp_servers SET data_sharing_policy=?,
 unsafe_action_policy=?, policy_revision=policy_revision+1, enabled=1, updated_at_ms=?
 WHERE mcp_server_id=? AND connection_revision=? AND policy_revision=?`, dataSharing,
		unsafeActions, millis(now), id, connectionRevision, expected)
	if err != nil {
		return MCPServer{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return MCPServer{}, errors.New("MCP policy authority changed")
	}
	return s.MCPServer(ctx, id)
}

// SaveMCPConnectionLabel applies one authority-fenced label change.
func (s *Store) SaveMCPConnectionLabel(ctx context.Context, id, revision string,
	expected, replacement *string, now time.Time) (MCPServer, error) {
	var expectedValue, replacementValue any
	if expected != nil {
		expectedValue = strings.TrimSpace(*expected)
	}
	if replacement != nil {
		value := strings.TrimSpace(*replacement)
		if value == "" {
			return MCPServer{}, errors.New("MCP connection label is invalid")
		}
		replacementValue = value
	}
	result, err := s.db.ExecContext(ctx, `UPDATE mcp_servers SET connection_label=?, updated_at_ms=?
 WHERE mcp_server_id=? AND connection_revision=? AND connection_label IS ?`, replacementValue,
		millis(now), id, revision, expectedValue)
	if err != nil {
		return MCPServer{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return MCPServer{}, errors.New("MCP connection label changed")
	}
	return s.MCPServer(ctx, id)
}

// SaveMCPToolPolicy applies one exact human override.
func (s *Store) SaveMCPToolPolicy(ctx context.Context, serverID, connectionRevision, toolID,
	sourceRevision string, expected int, enabled *bool, behavior *[4]bool, now time.Time) (MCPTool, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return MCPTool{}, err
	}
	defer func() { _ = tx.Rollback() }()
	var currentRevision string
	if err := tx.QueryRowContext(ctx, `SELECT connection_revision FROM mcp_servers WHERE mcp_server_id=?`, serverID).Scan(&currentRevision); err != nil || currentRevision != connectionRevision {
		return MCPTool{}, errors.New("MCP connection revision changed")
	}
	var result sql.Result
	if behavior != nil {
		result, err = tx.ExecContext(ctx, `UPDATE mcp_tools SET read_only=?, read_only_source='human',
 idempotent=?, idempotent_source='human', destructive=?, destructive_source='human',
 open_world=?, open_world_source='human', status='ready', policy_revision=policy_revision+1,
 updated_at_ms=? WHERE mcp_server_id=? AND mcp_tool_id=? AND source_revision=? AND policy_revision=?`,
			behavior[0], behavior[1], behavior[2], behavior[3], millis(now), serverID, toolID, sourceRevision, expected)
	} else if enabled != nil {
		if *enabled {
			result, err = tx.ExecContext(ctx, `UPDATE mcp_tools SET status=CASE
 WHEN read_only_source='safe_default' OR idempotent_source='safe_default'
   OR destructive_source='safe_default' OR open_world_source='safe_default' THEN 'defaulted'
 ELSE 'ready' END, policy_revision=policy_revision+1,
 updated_at_ms=? WHERE mcp_server_id=? AND mcp_tool_id=? AND source_revision=? AND policy_revision=?`,
				millis(now), serverID, toolID, sourceRevision, expected)
		} else {
			result, err = tx.ExecContext(ctx, `UPDATE mcp_tools SET status='disabled', policy_revision=policy_revision+1,
 updated_at_ms=? WHERE mcp_server_id=? AND mcp_tool_id=? AND source_revision=? AND policy_revision=?`,
				millis(now), serverID, toolID, sourceRevision, expected)
		}
	} else {
		return MCPTool{}, errors.New("MCP tool policy change is invalid")
	}
	if err != nil {
		return MCPTool{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return MCPTool{}, errors.New("MCP tool policy authority changed")
	}
	tool, err := scanMCPTool(tx.QueryRowContext(ctx, mcpToolSelect+` WHERE mcp_server_id=? AND mcp_tool_id=?`, serverID, toolID))
	if err != nil {
		return MCPTool{}, err
	}
	if err := tx.Commit(); err != nil {
		return MCPTool{}, err
	}
	return tool, nil
}

// ResetMCPToolPolicy restores the current remote annotations and safe defaults.
func (s *Store) ResetMCPToolPolicy(ctx context.Context, serverID, connectionRevision, toolID,
	sourceRevision string, expected int, hints [4]MCPHint, status string, now time.Time) (MCPTool, error) {
	if !validMCPStatus(status, "ready", "defaulted") {
		return MCPTool{}, errors.New("invalid MCP tool reset")
	}
	result, err := s.db.ExecContext(ctx, `UPDATE mcp_tools SET read_only=?, read_only_source=?,
 idempotent=?, idempotent_source=?, destructive=?, destructive_source=?, open_world=?,
 open_world_source=?, status=?, policy_revision=policy_revision+1, updated_at_ms=?
 WHERE mcp_server_id=? AND mcp_tool_id=? AND source_revision=? AND policy_revision=?
 AND EXISTS (SELECT 1 FROM mcp_servers WHERE mcp_server_id=? AND connection_revision=?)`,
		nullBool(hints[0].Value), hints[0].Source, nullBool(hints[1].Value), hints[1].Source,
		nullBool(hints[2].Value), hints[2].Source, nullBool(hints[3].Value), hints[3].Source,
		status, millis(now), serverID, toolID, sourceRevision, expected, serverID, connectionRevision)
	if err != nil {
		return MCPTool{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return MCPTool{}, errors.New("MCP tool policy authority changed")
	}
	return scanMCPTool(s.db.QueryRowContext(ctx, mcpToolSelect+` WHERE mcp_server_id=? AND mcp_tool_id=?`, serverID, toolID))
}

// ClassifyMCPTool fills only missing source hints under exact catalog and policy fences.
func (s *Store) ClassifyMCPTool(ctx context.Context, serverID, connectionRevision, toolID,
	sourceRevision string, expected int, behavior [4]bool, now time.Time) (MCPTool, error) {
	result, err := s.db.ExecContext(ctx, `UPDATE mcp_tools SET
 read_only=CASE WHEN read_only_source='safe_default' THEN ? ELSE read_only END,
 read_only_source=CASE WHEN read_only_source='safe_default' THEN 'model' ELSE read_only_source END,
 idempotent=CASE WHEN idempotent_source='safe_default' THEN ? ELSE idempotent END,
 idempotent_source=CASE WHEN idempotent_source='safe_default' THEN 'model' ELSE idempotent_source END,
 destructive=CASE WHEN destructive_source='safe_default' THEN ? ELSE destructive END,
 destructive_source=CASE WHEN destructive_source='safe_default' THEN 'model' ELSE destructive_source END,
 open_world=CASE WHEN open_world_source='safe_default' THEN ? ELSE open_world END,
 open_world_source=CASE WHEN open_world_source='safe_default' THEN 'model' ELSE open_world_source END,
 status=CASE WHEN status='disabled' THEN 'disabled' ELSE 'ready' END,
 policy_revision=policy_revision+1,updated_at_ms=?
 WHERE mcp_server_id=? AND mcp_tool_id=? AND source_revision=? AND policy_revision=?
 AND EXISTS (SELECT 1 FROM mcp_servers WHERE mcp_server_id=? AND connection_revision=?)
 AND (read_only_source='safe_default' OR idempotent_source='safe_default' OR destructive_source='safe_default' OR open_world_source='safe_default')`,
		behavior[0], behavior[1], behavior[2], behavior[3], millis(now), serverID, toolID, sourceRevision, expected,
		serverID, connectionRevision)
	if err != nil {
		return MCPTool{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return MCPTool{}, errors.New("MCP tool classification authority changed")
	}
	return scanMCPTool(s.db.QueryRowContext(ctx, mcpToolSelect+` WHERE mcp_server_id=? AND mcp_tool_id=?`, serverID, toolID))
}

// FenceMCPServer disables one connection before protected credential deletion.
func (s *Store) FenceMCPServer(ctx context.Context, id, revision string, now time.Time) error {
	result, err := s.db.ExecContext(ctx, `UPDATE mcp_servers SET enabled=0, health_status='unavailable',
 auth_status='unavailable', updated_at_ms=? WHERE mcp_server_id=? AND connection_revision=?`,
		millis(now), id, revision)
	if err != nil {
		return err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return errors.New("MCP connection revision changed")
	}
	return nil
}

// DeleteMCPServer removes one fenced connection and its current catalog.
func (s *Store) DeleteMCPServer(ctx context.Context, id string) (bool, error) {
	result, err := s.db.ExecContext(ctx, `DELETE FROM mcp_servers WHERE mcp_server_id=?`, id)
	if err != nil {
		return false, fmt.Errorf("delete MCP server: %w", err)
	}
	changed, _ := result.RowsAffected()
	return changed == 1, nil
}

// MarkMCPUnavailable records a transport failure only for the observed authority.
func (s *Store) MarkMCPUnavailable(ctx context.Context, id, revision, authStatus string, now time.Time) error {
	if !validMCPStatus(authStatus, "none", "needs_auth", "authenticated", "unavailable") {
		authStatus = "unavailable"
	}
	_, err := s.db.ExecContext(ctx, `UPDATE mcp_servers SET health_status='unavailable', auth_status=?,
 updated_at_ms=? WHERE mcp_server_id=? AND connection_revision=?`, authStatus, millis(now), id, revision)
	return err
}

// UpdateMCPSecretRevision publishes one protected credential refresh with an exact fence.
func (s *Store) UpdateMCPSecretRevision(ctx context.Context, id, connectionRevision, oldSecretRevision,
	newSecretRevision string, now time.Time) error {
	result, err := s.db.ExecContext(ctx, `UPDATE mcp_servers SET secret_revision=?, auth_status='authenticated',
 health_status='healthy', updated_at_ms=? WHERE mcp_server_id=? AND connection_revision=? AND secret_revision=?`,
		newSecretRevision, millis(now), id, connectionRevision, oldSecretRevision)
	if err != nil {
		return err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return errors.New("MCP credential authority changed")
	}
	return nil
}

// MCPOAuthAttempt is the safe durable projection of one browser attempt.
type MCPOAuthAttempt struct {
	ID, OwnerHumanID, ServerID, Status, FailureCode, ResultServerID string
	ExpiresAt, CreatedAt, UpdatedAt                                 time.Time
}

// CreateMCPOAuthAttempt reserves one safe durable attempt row.
func (s *Store) CreateMCPOAuthAttempt(ctx context.Context, value MCPOAuthAttempt) error {
	_, err := s.db.ExecContext(ctx, `INSERT INTO mcp_oauth_attempts
 (attempt_id, owner_human_id, mcp_server_id, status, expires_at_ms, created_at_ms, updated_at_ms)
 VALUES (?, ?, NULLIF(?, ''), 'waiting_for_user', ?, ?, ?)`, value.ID, value.OwnerHumanID,
		value.ServerID, millis(value.ExpiresAt), millis(value.CreatedAt), millis(value.UpdatedAt))
	return err
}

// FinishMCPOAuthAttempt records one terminal safe result.
func (s *Store) FinishMCPOAuthAttempt(ctx context.Context, id, status, failure, serverID string, now time.Time) error {
	if !validMCPStatus(status, "completed", "failed") {
		return errors.New("invalid MCP OAuth state")
	}
	result, err := s.db.ExecContext(ctx, `UPDATE mcp_oauth_attempts SET status=?, failure_code=NULLIF(?, ''),
 result_mcp_server_id=NULLIF(?, ''), updated_at_ms=? WHERE attempt_id=? AND status='waiting_for_user'`,
		status, failure, serverID, millis(now), id)
	if err != nil {
		return err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return errors.New("MCP OAuth attempt changed")
	}
	return nil
}

// MCPOAuthAttempt returns one safe attempt owned by the local human.
func (s *Store) MCPOAuthAttempt(ctx context.Context, id, owner string, now time.Time) (MCPOAuthAttempt, error) {
	var v MCPOAuthAttempt
	var expires, created, updated int64
	err := s.db.QueryRowContext(ctx, `SELECT attempt_id, owner_human_id, COALESCE(mcp_server_id, ''),
 status, COALESCE(failure_code, ''), COALESCE(result_mcp_server_id, ''), expires_at_ms, created_at_ms, updated_at_ms
 FROM mcp_oauth_attempts WHERE attempt_id=? AND owner_human_id=? AND expires_at_ms>?`, id, owner, millis(now)).Scan(
		&v.ID, &v.OwnerHumanID, &v.ServerID, &v.Status, &v.FailureCode, &v.ResultServerID, &expires, &created, &updated)
	if err != nil {
		return MCPOAuthAttempt{}, errors.New("MCP OAuth attempt not found")
	}
	v.ExpiresAt, v.CreatedAt, v.UpdatedAt = fromMillis(expires), fromMillis(created), fromMillis(updated)
	return v, nil
}

func validateNewMCPConnection(v NewMCPConnection) error {
	if v.ServerID == "" || v.Definition.ID == "" || v.Definition.Revision == "" ||
		strings.TrimSpace(v.Definition.DisplayName) == "" ||
		!validMCPStatus(v.Definition.TransportKind, "stdio", "streamable_http") ||
		!validMCPStatus(v.AuthStatus, "none", "authenticated") || len(v.Tools) > 128 ||
		!json.Valid(v.Definition.SafeConfig) {
		return errors.New("invalid MCP connection")
	}
	return nil
}

func validMCPStatus(value string, allowed ...string) bool {
	for _, candidate := range allowed {
		if value == candidate {
			return true
		}
	}
	return false
}

func nullBool(value *bool) any {
	if value == nil {
		return nil
	}
	return *value
}
func boolPointer(value sql.NullBool) *bool {
	if !value.Valid {
		return nil
	}
	result := value.Bool
	return &result
}
