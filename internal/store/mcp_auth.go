package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"time"
)

// MCPAuthRequest is one secret-free durable call authentication interruption.
type MCPAuthRequest struct {
	ID, OwnerHumanID, ConversationID, TurnID, CallItemID, ServerID string
	ActionID                                                       string
	CapabilityName, BindingJSON, ArgumentsJSON, Provider           string
	ProviderCallID, ProviderName, OAuthAttemptID, State, Failure   string
	Revision, ProviderRound, OutputIndex                           int
}

// CreateMCPAuthRequest suspends one exact stored provider call.
func (s *Store) CreateMCPAuthRequest(ctx context.Context, value MCPAuthRequest, now time.Time) (MCPAuthRequest, ConversationItem, error) {
	id, err := newID("mcp_auth")
	if err != nil {
		return MCPAuthRequest{}, ConversationItem{}, err
	}
	if value.OwnerHumanID != "human:local" || !json.Valid([]byte(value.BindingJSON)) || !json.Valid([]byte(value.ArgumentsJSON)) {
		return MCPAuthRequest{}, ConversationItem{}, errors.New("MCP authentication request is invalid")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return MCPAuthRequest{}, ConversationItem{}, err
	}
	defer tx.Rollback()
	var payload string
	if err := tx.QueryRowContext(ctx, `SELECT payload_json FROM conversation_items WHERE item_id=? AND conversation_id=?
 AND turn_id=? AND kind='tool_call' AND status='running'`, value.CallItemID, value.ConversationID, value.TurnID).Scan(&payload); err != nil {
		return MCPAuthRequest{}, ConversationItem{}, errors.New("MCP authentication call is unavailable")
	}
	var item map[string]any
	if json.Unmarshal([]byte(payload), &item) != nil {
		return MCPAuthRequest{}, ConversationItem{}, errors.New("MCP authentication call is invalid")
	}
	metadata, _ := item["metadata"].(map[string]any)
	action, _ := metadata["action"].(map[string]any)
	value.Provider, _ = metadata["provider"].(string)
	value.ProviderCallID, _ = action["provider_call_id"].(string)
	value.ProviderName, _ = action["provider_name"].(string)
	value.OutputIndex = int(numberValue(metadata["output_index"]))
	value.ID, value.Revision, value.State = id, 1, "awaiting_user"
	if value.Provider == "" || action["name"] != value.CapabilityName {
		return MCPAuthRequest{}, ConversationItem{}, errors.New("MCP authentication call is invalid")
	}
	if value.ActionID != "" {
		var state string
		if err := tx.QueryRowContext(ctx, `SELECT state FROM action_requests WHERE action_id=? AND conversation_id=? AND turn_id=? AND call_item_id=?`,
			value.ActionID, value.ConversationID, value.TurnID, value.CallItemID).Scan(&state); err != nil || state != "executing" {
			return MCPAuthRequest{}, ConversationItem{}, errors.New("MCP authentication action is unavailable")
		}
	}
	_, err = tx.ExecContext(ctx, `INSERT INTO mcp_auth_requests (request_id,owner_human_id,conversation_id,turn_id,call_item_id,
 mcp_server_id,capability_name,binding_json,arguments_json,provider,provider_round,output_index,provider_call_id,provider_name,action_request_id,state,created_at_ms,updated_at_ms)
 VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,'awaiting_user',?,?)`, id, value.OwnerHumanID, value.ConversationID, value.TurnID,
		value.CallItemID, value.ServerID, value.CapabilityName, value.BindingJSON, value.ArgumentsJSON, value.Provider,
		value.ProviderRound, value.OutputIndex, value.ProviderCallID, value.ProviderName, nullText(value.ActionID), millis(now), millis(now))
	if err != nil {
		return MCPAuthRequest{}, ConversationItem{}, err
	}
	sequence, err := nextConversationSequenceTx(ctx, tx, value.ConversationID)
	if err != nil {
		return MCPAuthRequest{}, ConversationItem{}, err
	}
	notice, err := insertConversationOutputTx(ctx, tx, ConversationItem{ID: "item:" + id[9:], ConversationID: value.ConversationID,
		TurnID: value.TurnID, ParentItemID: value.CallItemID, Sequence: sequence, Kind: ConversationApprovalRequest,
		Status: "completed", AuthorActorID: "agent:primary", ContentText: "Authentication required",
		Payload:  map[string]any{"id": id, "activity_kind": "authentication_request", "status": "completed", "title": "Authentication required"},
		Metadata: map[string]any{"source": "mcp_authentication"}, CreatedAt: now})
	if err != nil {
		return MCPAuthRequest{}, ConversationItem{}, err
	}
	if _, err = tx.ExecContext(ctx, `UPDATE conversation_turns SET status='completed',completed_at_ms=?,updated_at_ms=? WHERE turn_id=? AND conversation_id=? AND status='running'`, millis(now), millis(now), value.TurnID, value.ConversationID); err != nil {
		return MCPAuthRequest{}, ConversationItem{}, err
	}
	if _, err = tx.ExecContext(ctx, `UPDATE conversations SET agent_status='idle',updated_at_ms=? WHERE conversation_id=?`, millis(now), value.ConversationID); err != nil {
		return MCPAuthRequest{}, ConversationItem{}, err
	}
	if err := tx.Commit(); err != nil {
		return MCPAuthRequest{}, ConversationItem{}, err
	}
	return value, notice, nil
}

func numberValue(value any) float64 { result, _ := value.(float64); return result }

const mcpAuthSelect = `SELECT request_id,owner_human_id,conversation_id,turn_id,call_item_id,mcp_server_id,
 capability_name,binding_json,arguments_json,provider,provider_round,output_index,provider_call_id,provider_name,
 COALESCE(oauth_attempt_id,''),state,COALESCE(failure_code,''),revision,COALESCE(action_request_id,'') FROM mcp_auth_requests`

func scanMCPAuth(row rowScanner) (MCPAuthRequest, error) {
	var v MCPAuthRequest
	err := row.Scan(&v.ID, &v.OwnerHumanID, &v.ConversationID, &v.TurnID, &v.CallItemID, &v.ServerID, &v.CapabilityName, &v.BindingJSON, &v.ArgumentsJSON, &v.Provider, &v.ProviderRound, &v.OutputIndex, &v.ProviderCallID, &v.ProviderName, &v.OAuthAttemptID, &v.State, &v.Failure, &v.Revision, &v.ActionID)
	return v, err
}

func (s *Store) MCPAuthRequest(ctx context.Context, id string, revision int) (MCPAuthRequest, error) {
	return scanMCPAuth(s.db.QueryRowContext(ctx, mcpAuthSelect+` WHERE request_id=? AND revision=?`, id, revision))
}
func (s *Store) PendingMCPAuthRequests(ctx context.Context, owner string, conversationID *string, limit int) ([]MCPAuthRequest, error) {
	rows, err := s.db.QueryContext(ctx, mcpAuthSelect+` WHERE owner_human_id=? AND state IN ('awaiting_user','authorizing') AND (? IS NULL OR conversation_id=?) ORDER BY created_at_ms LIMIT ?`, owner, conversationID, conversationID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var result []MCPAuthRequest
	for rows.Next() {
		value, err := scanMCPAuth(rows)
		if err != nil {
			return nil, err
		}
		result = append(result, value)
	}
	return result, rows.Err()
}
func (s *Store) BeginMCPAuthentication(ctx context.Context, id string, revision int, owner, attempt string, now time.Time) (MCPAuthRequest, error) {
	request, err := s.MCPAuthRequest(ctx, id, revision)
	if err != nil || request.OwnerHumanID != owner || (request.State != "awaiting_user" && request.State != "authorizing") {
		return MCPAuthRequest{}, errors.New("MCP authentication request is stale")
	}
	_, err = s.db.ExecContext(ctx, `UPDATE mcp_auth_requests SET state='authorizing',oauth_attempt_id=?,failure_code=NULL,updated_at_ms=? WHERE owner_human_id=? AND mcp_server_id=? AND state IN ('awaiting_user','authorizing')`, attempt, millis(now), owner, request.ServerID)
	if err != nil {
		return MCPAuthRequest{}, err
	}
	return s.MCPAuthRequest(ctx, id, revision)
}
func (s *Store) MCPAuthRequestsForAttempt(ctx context.Context, attempt string) ([]MCPAuthRequest, error) {
	rows, err := s.db.QueryContext(ctx, mcpAuthSelect+` WHERE oauth_attempt_id=? AND state='authorizing' ORDER BY created_at_ms`, attempt)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var result []MCPAuthRequest
	for rows.Next() {
		v, e := scanMCPAuth(rows)
		if e != nil {
			return nil, e
		}
		result = append(result, v)
	}
	return result, rows.Err()
}

// BeginMCPAuthResume records that one reviewed remote call can no longer be retried safely.
func (s *Store) BeginMCPAuthResume(ctx context.Context, request MCPAuthRequest, now time.Time) (MCPAuthRequest, error) {
	if request.ActionID == "" || request.State != "authorizing" {
		return MCPAuthRequest{}, errors.New("MCP authentication resumption is invalid")
	}
	result, err := s.db.ExecContext(ctx, `UPDATE mcp_auth_requests SET state='resuming',updated_at_ms=?
 WHERE request_id=? AND revision=? AND state='authorizing'`, millis(now), request.ID, request.Revision)
	if err != nil {
		return MCPAuthRequest{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return MCPAuthRequest{}, errors.New("MCP authentication request changed")
	}
	return s.MCPAuthRequest(ctx, request.ID, request.Revision)
}
func (s *Store) FinishMCPAuthRequest(ctx context.Context, id string, revision int, state, failure string, now time.Time) (MCPAuthRequest, error) {
	if state != "completed" && state != "cancelled" && state != "superseded" {
		return MCPAuthRequest{}, errors.New("invalid MCP authentication result")
	}
	result, err := s.db.ExecContext(ctx, `UPDATE mcp_auth_requests SET state=?,failure_code=NULLIF(?,''),updated_at_ms=? WHERE request_id=? AND revision=? AND state IN ('awaiting_user','authorizing','resuming')`, state, failure, millis(now), id, revision)
	if err != nil {
		return MCPAuthRequest{}, err
	}
	if n, _ := result.RowsAffected(); n != 1 {
		return MCPAuthRequest{}, errors.New("MCP authentication request changed")
	}
	return s.MCPAuthRequest(ctx, id, revision)
}

// FinishMCPAuthAction atomically closes one suspended reviewed call and its action claim.
func (s *Store) FinishMCPAuthAction(ctx context.Context, request MCPAuthRequest, state ActionRequestState,
	output json.RawMessage, failure, authState string, now time.Time) (MCPAuthRequest, ActionRequest, error) {
	if request.ActionID == "" || (state != ActionSucceeded && state != ActionFailed) ||
		(authState != "completed" && authState != "cancelled") {
		return MCPAuthRequest{}, ActionRequest{}, errors.New("MCP authentication action result is invalid")
	}
	storedOutput, err := boundedJSONObject(output, actionArgumentsLimit)
	if err != nil {
		return MCPAuthRequest{}, ActionRequest{}, errors.New("MCP authentication action output is invalid")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return MCPAuthRequest{}, ActionRequest{}, err
	}
	defer tx.Rollback()
	result, err := tx.ExecContext(ctx, `UPDATE action_requests SET state=?,output_json=?,failure_code=NULLIF(?,''),completed_at_ms=?,updated_at_ms=?
 WHERE action_id=? AND revision=1 AND state='executing'`, state, string(storedOutput), failure, millis(now), millis(now), request.ActionID)
	if err != nil {
		return MCPAuthRequest{}, ActionRequest{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return MCPAuthRequest{}, ActionRequest{}, errors.New("MCP authentication action claim changed")
	}
	if err := insertActionEvent(ctx, tx, request.ActionID, string(state), "system:action_gateway",
		map[string]any{"failure_code": nullText(failure)}, now); err != nil {
		return MCPAuthRequest{}, ActionRequest{}, err
	}
	result, err = tx.ExecContext(ctx, `UPDATE mcp_auth_requests SET state=?,failure_code=NULLIF(?,''),updated_at_ms=?
 WHERE request_id=? AND revision=? AND state IN ('awaiting_user','authorizing','resuming')`, authState, failure, millis(now), request.ID, request.Revision)
	if err != nil {
		return MCPAuthRequest{}, ActionRequest{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return MCPAuthRequest{}, ActionRequest{}, errors.New("MCP authentication request changed")
	}
	action, err := actionRequestTx(ctx, tx, request.ActionID, 1)
	if err != nil {
		return MCPAuthRequest{}, ActionRequest{}, err
	}
	auth, err := scanMCPAuth(tx.QueryRowContext(ctx, mcpAuthSelect+` WHERE request_id=? AND revision=?`, request.ID, request.Revision))
	if err != nil {
		return MCPAuthRequest{}, ActionRequest{}, err
	}
	if err := tx.Commit(); err != nil {
		return MCPAuthRequest{}, ActionRequest{}, err
	}
	return auth, action, nil
}

// MCPAuthConversationCall rebuilds one exact stored provider result target.
func (s *Store) MCPAuthConversationCall(ctx context.Context, request MCPAuthRequest) (ConversationTurn, ConversationToolResultInput, error) {
	var turn ConversationTurn
	if err := s.db.QueryRowContext(ctx, `SELECT turn_id,conversation_id,
 CAST(json_extract(metadata_json,'$.turn_index') AS INTEGER),status FROM conversation_turns
 WHERE turn_id=? AND conversation_id=?`, request.TurnID, request.ConversationID).Scan(
		&turn.ID, &turn.ConversationID, &turn.TurnIndex, &turn.Status); err != nil {
		return turn, ConversationToolResultInput{}, err
	}
	return turn, ConversationToolResultInput{CallItemID: request.CallItemID, Provider: request.Provider,
		ProviderRound: request.ProviderRound, OutputIndex: request.OutputIndex,
		ProviderCallID: request.ProviderCallID, ProviderName: request.ProviderName, Name: request.CapabilityName}, nil
}
