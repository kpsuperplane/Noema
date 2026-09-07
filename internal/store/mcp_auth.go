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
	ID, OwnerHumanID, ConversationID, TurnID, CallItemID, ServerID                 string
	AuthorityKind, AuthorityID                                                     string
	AdapterConnectionID, AdapterSemanticDigest                                     string
	AdapterAuthorityRevision                                                       int
	TaskID, RunID, RunItemID                                                       string
	TaskGeneration                                                                 int64
	ActionID                                                                       string
	CapabilityName, BindingJSON, ArgumentsJSON, Provider                           string
	ProviderCallID, ProviderName, OAuthAttemptID, AdapterAttemptID, State, Failure string
	Revision, ProviderRound, OutputIndex                                           int
}

// CreateMCPAuthRequest suspends one exact stored provider call.
func (s *Store) CreateMCPAuthRequest(ctx context.Context, value MCPAuthRequest, now time.Time) (MCPAuthRequest, ConversationItem, error) {
	id, err := newID("mcp_auth")
	if err != nil {
		return MCPAuthRequest{}, ConversationItem{}, err
	}
	if value.AuthorityKind == "" {
		value.AuthorityKind, value.AuthorityID = "mcp_server", value.ServerID
	}
	if value.OwnerHumanID != "human:local" || !json.Valid([]byte(value.BindingJSON)) || !json.Valid([]byte(value.ArgumentsJSON)) ||
		(value.AuthorityKind != "mcp_server" && value.AuthorityKind != "adapter_connection" && value.AuthorityKind != "adapter_grant") || value.AuthorityID == "" ||
		(value.AuthorityKind == "adapter_grant" && (value.AdapterConnectionID == "" || value.AdapterSemanticDigest == "" || value.AdapterAuthorityRevision < 1)) {
		return MCPAuthRequest{}, ConversationItem{}, errors.New("MCP authentication request is invalid")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return MCPAuthRequest{}, ConversationItem{}, err
	}
	defer tx.Rollback()
	var payload string
	if value.TaskID == "" {
		if err := tx.QueryRowContext(ctx, `SELECT payload_json FROM conversation_items WHERE item_id=? AND conversation_id=?
 AND turn_id=? AND kind='tool_call' AND status='running'`, value.CallItemID, value.ConversationID, value.TurnID).Scan(&payload); err != nil {
			return MCPAuthRequest{}, ConversationItem{}, errors.New("MCP authentication call is unavailable")
		}
	} else {
		var status, kind, itemStatus string
		if err := tx.QueryRowContext(ctx, `SELECT r.provider_kind,i.round_index,i.sequence_index,i.item_kind,i.status,i.payload_json
FROM task_runs r JOIN tasks t ON t.task_id=r.task_id JOIN task_run_items i ON i.run_id=r.run_id
WHERE r.run_id=? AND r.task_id=? AND r.task_generation=? AND i.item_id=? AND t.current_run_id=r.run_id AND r.status IN ('running','waiting_for_approval')`,
			value.RunID, value.TaskID, value.TaskGeneration, value.RunItemID).Scan(&value.Provider, &value.ProviderRound, &value.OutputIndex, &kind, &itemStatus, &payload); err != nil || kind != "tool_call" || itemStatus != "running" {
			return MCPAuthRequest{}, ConversationItem{}, errors.New("MCP authentication Task call is unavailable")
		}
		_ = status
	}
	var item map[string]any
	if json.Unmarshal([]byte(payload), &item) != nil {
		return MCPAuthRequest{}, ConversationItem{}, errors.New("MCP authentication call is invalid")
	}
	metadata, _ := item["metadata"].(map[string]any)
	action, _ := metadata["action"].(map[string]any)
	if value.TaskID != "" {
		action = item
		metadata = item
	}
	if value.Provider == "" {
		value.Provider, _ = metadata["provider"].(string)
	}
	value.ProviderCallID, _ = action["provider_call_id"].(string)
	value.ProviderName, _ = action["provider_name"].(string)
	if value.TaskID == "" {
		value.OutputIndex = int(numberValue(metadata["output_index"]))
	}
	value.ID, value.Revision, value.State = id, 1, "awaiting_user"
	if value.Provider == "" || action["name"] != value.CapabilityName {
		return MCPAuthRequest{}, ConversationItem{}, errors.New("MCP authentication call is invalid")
	}
	if value.ActionID != "" && value.TaskID == "" {
		var state string
		if err := tx.QueryRowContext(ctx, `SELECT state FROM action_requests WHERE action_id=? AND conversation_id=? AND turn_id=? AND call_item_id=?`,
			value.ActionID, value.ConversationID, value.TurnID, value.CallItemID).Scan(&state); err != nil || state != "executing" {
			return MCPAuthRequest{}, ConversationItem{}, errors.New("MCP authentication action is unavailable")
		}
	} else if value.ActionID != "" {
		var state string
		if err := tx.QueryRowContext(ctx, `SELECT state FROM action_requests WHERE action_id=? AND task_id=? AND run_id=? AND run_item_id=?`,
			value.ActionID, value.TaskID, value.RunID, value.RunItemID).Scan(&state); err != nil || state != "executing" {
			return MCPAuthRequest{}, ConversationItem{}, errors.New("MCP authentication Task action is unavailable")
		}
	}
	_, err = tx.ExecContext(ctx, `INSERT INTO mcp_auth_requests (request_id,owner_human_id,conversation_id,turn_id,call_item_id,task_id,run_id,task_generation,run_item_id,
	 authority_kind,authority_id,mcp_server_id,capability_name,adapter_connection_id,adapter_semantic_digest,adapter_authority_revision,binding_json,arguments_json,provider,provider_round,output_index,provider_call_id,provider_name,action_request_id,state,created_at_ms,updated_at_ms)
	 VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,'awaiting_user',?,?)`, id, value.OwnerHumanID, nullText(value.ConversationID), nullText(value.TurnID),
		nullText(value.CallItemID), nullText(value.TaskID), nullText(value.RunID), nullableTaskGeneration(value.TaskID, value.TaskGeneration), nullText(value.RunItemID),
		value.AuthorityKind, value.AuthorityID, nullableMCPServer(value), value.CapabilityName, nullText(value.AdapterConnectionID), nullText(value.AdapterSemanticDigest), nullablePositive(value.AdapterAuthorityRevision), value.BindingJSON, value.ArgumentsJSON, value.Provider,
		value.ProviderRound, value.OutputIndex, value.ProviderCallID, value.ProviderName, nullText(value.ActionID), millis(now), millis(now))
	if err != nil {
		return MCPAuthRequest{}, ConversationItem{}, err
	}
	if value.TaskID != "" {
		if _, err = tx.ExecContext(ctx, `UPDATE task_runs SET status='waiting_for_approval',updated_at_ms=?
WHERE run_id=? AND task_id=? AND task_generation=? AND status='running'`, millis(now), value.RunID, value.TaskID, value.TaskGeneration); err != nil {
			return MCPAuthRequest{}, ConversationItem{}, err
		}
		if err := tx.Commit(); err != nil {
			return MCPAuthRequest{}, ConversationItem{}, err
		}
		return value, ConversationItem{}, nil
	}
	sequence, err := nextConversationSequenceTx(ctx, tx, value.ConversationID)
	if err != nil {
		return MCPAuthRequest{}, ConversationItem{}, err
	}
	notice, err := insertConversationOutputTx(ctx, tx, ConversationItem{ID: "item:" + id[9:], ConversationID: value.ConversationID,
		TurnID: value.TurnID, ParentItemID: value.CallItemID, Sequence: sequence, Kind: ConversationApprovalRequest,
		Status: "completed", AuthorActorID: "agent:primary", ContentText: "Authentication required",
		Payload: map[string]any{"id": id, "activity_kind": "authentication_request", "status": "completed",
			"title": "Authentication required", "summary": value.CapabilityName,
			"metadata": map[string]any{"source": value.AuthorityKind + "_authentication", "request_id": id}},
		Metadata: map[string]any{"source": value.AuthorityKind + "_authentication"}, CreatedAt: now})
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

const mcpAuthSelect = `SELECT request_id,owner_human_id,conversation_id,turn_id,call_item_id,task_id,run_id,task_generation,run_item_id,authority_kind,authority_id,COALESCE(mcp_server_id,''),
	 capability_name,COALESCE(adapter_connection_id,''),COALESCE(adapter_semantic_digest,''),COALESCE(adapter_authority_revision,0),binding_json,arguments_json,provider,provider_round,output_index,provider_call_id,provider_name,
	 COALESCE(oauth_attempt_id,''),COALESCE(adapter_attempt_id,''),state,COALESCE(failure_code,''),revision,COALESCE(action_request_id,'') FROM mcp_auth_requests`

func scanMCPAuth(row rowScanner) (MCPAuthRequest, error) {
	var v MCPAuthRequest
	var conversationID, turnID, callItemID, taskID, runID, runItemID sql.NullString
	var generation sql.NullInt64
	err := row.Scan(&v.ID, &v.OwnerHumanID, &conversationID, &turnID, &callItemID, &taskID, &runID, &generation, &runItemID,
		&v.AuthorityKind, &v.AuthorityID, &v.ServerID, &v.CapabilityName, &v.AdapterConnectionID, &v.AdapterSemanticDigest, &v.AdapterAuthorityRevision, &v.BindingJSON, &v.ArgumentsJSON, &v.Provider, &v.ProviderRound, &v.OutputIndex, &v.ProviderCallID, &v.ProviderName, &v.OAuthAttemptID, &v.AdapterAttemptID, &v.State, &v.Failure, &v.Revision, &v.ActionID)
	v.ConversationID, v.TurnID, v.CallItemID = conversationID.String, turnID.String, callItemID.String
	v.TaskID, v.RunID, v.RunItemID, v.TaskGeneration = taskID.String, runID.String, runItemID.String, generation.Int64
	return v, err
}

func (s *Store) MCPAuthRequest(ctx context.Context, id string, revision int) (MCPAuthRequest, error) {
	return scanMCPAuth(s.db.QueryRowContext(ctx, mcpAuthSelect+` WHERE request_id=? AND revision=?`, id, revision))
}
func (s *Store) PendingMCPAuthRequests(ctx context.Context, owner string, conversationID, taskID *string, limit int) ([]MCPAuthRequest, error) {
	return s.pendingAuthRequests(ctx, "mcp_server", owner, conversationID, taskID, limit)
}
func (s *Store) PendingAdapterAuthRequests(ctx context.Context, owner string, conversationID, taskID *string, limit int) ([]MCPAuthRequest, error) {
	rows, err := s.db.QueryContext(ctx, mcpAuthSelect+` WHERE authority_kind IN ('adapter_connection','adapter_grant') AND owner_human_id=? AND state IN ('awaiting_user','authorizing') AND (? IS NULL OR conversation_id=?) AND (? IS NULL OR task_id=?) ORDER BY created_at_ms LIMIT ?`, owner, conversationID, conversationID, taskID, taskID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var result []MCPAuthRequest
	for rows.Next() {
		value, e := scanMCPAuth(rows)
		if e != nil {
			return nil, e
		}
		result = append(result, value)
	}
	return result, rows.Err()
}
func (s *Store) pendingAuthRequests(ctx context.Context, kind, owner string, conversationID, taskID *string, limit int) ([]MCPAuthRequest, error) {
	rows, err := s.db.QueryContext(ctx, mcpAuthSelect+` WHERE authority_kind=? AND owner_human_id=? AND state IN ('awaiting_user','authorizing') AND (? IS NULL OR conversation_id=?) AND (? IS NULL OR task_id=?) ORDER BY created_at_ms LIMIT ?`, kind, owner, conversationID, conversationID, taskID, taskID, limit)
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

func nullableMCPServer(value MCPAuthRequest) any {
	if value.AuthorityKind == "mcp_server" {
		return value.AuthorityID
	}
	return nil
}
func nullablePositive(value int) any {
	if value > 0 {
		return value
	}
	return nil
}

// BeginAdapterOAuthAuthentication attaches one process-local attempt to matching paused calls.
func (s *Store) BeginAdapterOAuthAuthentication(ctx context.Context, request MCPAuthRequest, attempt string, now time.Time) (MCPAuthRequest, error) {
	if request.AuthorityKind != "adapter_grant" || request.State != "awaiting_user" || attempt == "" {
		return MCPAuthRequest{}, errors.New("adapter OAuth request is stale")
	}
	_, err := s.db.ExecContext(ctx, `UPDATE mcp_auth_requests SET state='authorizing',adapter_attempt_id=?,failure_code=NULL,updated_at_ms=? WHERE authority_kind='adapter_grant' AND authority_id=? AND state='awaiting_user'`, attempt, millis(now), request.AuthorityID)
	if err != nil {
		return MCPAuthRequest{}, err
	}
	return s.MCPAuthRequest(ctx, request.ID, request.Revision)
}

// CompleteAdapterOAuthAuthentication makes exact completed-attempt calls eligible for resumption.
func (s *Store) CompleteAdapterOAuthAuthentication(ctx context.Context, attempt string, now time.Time) error {
	result, err := s.db.ExecContext(ctx, `UPDATE mcp_auth_requests SET state='awaiting_user',adapter_attempt_id=NULL,updated_at_ms=? WHERE authority_kind='adapter_grant' AND adapter_attempt_id=? AND state='authorizing'`, millis(now), attempt)
	if err != nil {
		return err
	}
	if changed, _ := result.RowsAffected(); changed == 0 {
		return errors.New("adapter OAuth attempt is unavailable")
	}
	return nil
}

// RetryAdapterOAuthAuthentication returns one failed browser attempt to human attention.
func (s *Store) RetryAdapterOAuthAuthentication(ctx context.Context, request MCPAuthRequest, failure string, now time.Time) error {
	result, err := s.db.ExecContext(ctx, `UPDATE mcp_auth_requests SET state='awaiting_user',adapter_attempt_id=NULL,failure_code=?,updated_at_ms=?
	 WHERE request_id=? AND revision=? AND authority_kind='adapter_grant' AND adapter_attempt_id=? AND state='authorizing'`, failure, millis(now), request.ID, request.Revision, request.AdapterAttemptID)
	if err != nil {
		return err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return errors.New("adapter OAuth request changed")
	}
	return nil
}

// AdapterAuthRequestsForAttempt returns exact calls attached to one browser attempt.
func (s *Store) AdapterAuthRequestsForAttempt(ctx context.Context, attempt string) ([]MCPAuthRequest, error) {
	rows, err := s.db.QueryContext(ctx, mcpAuthSelect+` WHERE adapter_attempt_id=? AND state='authorizing' ORDER BY created_at_ms`, attempt)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var result []MCPAuthRequest
	for rows.Next() {
		value, scanErr := scanMCPAuth(rows)
		if scanErr != nil {
			return nil, scanErr
		}
		result = append(result, value)
	}
	return result, rows.Err()
}
func (s *Store) BeginMCPAuthentication(ctx context.Context, id string, revision int, owner, attempt string, now time.Time) (MCPAuthRequest, error) {
	request, err := s.MCPAuthRequest(ctx, id, revision)
	if err != nil || request.AuthorityKind != "mcp_server" || request.OwnerHumanID != owner || (request.State != "awaiting_user" && request.State != "authorizing") {
		return MCPAuthRequest{}, errors.New("MCP authentication request is stale")
	}
	_, err = s.db.ExecContext(ctx, `UPDATE mcp_auth_requests SET state='authorizing',oauth_attempt_id=?,failure_code=NULL,updated_at_ms=? WHERE owner_human_id=? AND mcp_server_id=? AND state IN ('awaiting_user','authorizing')`, attempt, millis(now), owner, request.ServerID)
	if err != nil {
		return MCPAuthRequest{}, err
	}
	return s.MCPAuthRequest(ctx, id, revision)
}

// AdapterAuthRequestsForConnection returns calls paused on one direct credential.
func (s *Store) AdapterAuthRequestsForConnection(ctx context.Context, connectionID string) ([]MCPAuthRequest, error) {
	rows, err := s.db.QueryContext(ctx, mcpAuthSelect+` WHERE ((authority_kind='adapter_connection' AND authority_id=?) OR (authority_kind='adapter_grant' AND adapter_connection_id=?)) AND state='awaiting_user' ORDER BY created_at_ms`, connectionID, connectionID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var result []MCPAuthRequest
	for rows.Next() {
		value, scanErr := scanMCPAuth(rows)
		if scanErr != nil {
			return nil, scanErr
		}
		result = append(result, value)
	}
	return result, rows.Err()
}

// AwaitingAdapterAuthConnections returns exact connections with calls that can resume.
func (s *Store) AwaitingAdapterAuthConnections(ctx context.Context, task bool) ([]string, error) {
	rows, err := s.db.QueryContext(ctx, `SELECT DISTINCT COALESCE(adapter_connection_id,authority_id) FROM mcp_auth_requests
	 WHERE authority_kind IN ('adapter_connection','adapter_grant') AND state='awaiting_user' AND (task_id IS NOT NULL)=? ORDER BY 1`, task)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var result []string
	for rows.Next() {
		var connectionID string
		if err = rows.Scan(&connectionID); err != nil {
			return nil, err
		}
		result = append(result, connectionID)
	}
	return result, rows.Err()
}

// BeginAdapterAuthResume fences one exact retry after credential replacement.
func (s *Store) BeginAdapterAuthResume(ctx context.Context, request MCPAuthRequest, now time.Time) (MCPAuthRequest, error) {
	if request.AuthorityKind != "adapter_connection" && request.AuthorityKind != "adapter_grant" || request.State != "awaiting_user" {
		return MCPAuthRequest{}, errors.New("adapter authentication resumption is invalid")
	}
	result, err := s.db.ExecContext(ctx, `UPDATE mcp_auth_requests SET state='resuming',updated_at_ms=? WHERE request_id=? AND revision=? AND state='awaiting_user'`, millis(now), request.ID, request.Revision)
	if err != nil {
		return MCPAuthRequest{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return MCPAuthRequest{}, errors.New("adapter authentication request changed")
	}
	return s.MCPAuthRequest(ctx, request.ID, request.Revision)
}

// RetryAdapterAuthentication returns a rejected replacement to human attention.
func (s *Store) RetryAdapterAuthentication(ctx context.Context, request MCPAuthRequest, now time.Time) (MCPAuthRequest, error) {
	result, err := s.db.ExecContext(ctx, `UPDATE mcp_auth_requests SET state='awaiting_user',failure_code='authentication_failed',updated_at_ms=? WHERE request_id=? AND revision=? AND authority_kind IN ('adapter_connection','adapter_grant') AND state='resuming'`, millis(now), request.ID, request.Revision)
	if err != nil {
		return MCPAuthRequest{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return MCPAuthRequest{}, errors.New("adapter authentication request changed")
	}
	return s.MCPAuthRequest(ctx, request.ID, request.Revision)
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

// RecoverTaskMCPAuthRequests returns direct Task calls cancelled during startup recovery.
func (s *Store) RecoverTaskMCPAuthRequests(ctx context.Context) ([]MCPAuthRequest, error) {
	rows, err := s.db.QueryContext(ctx, mcpAuthSelect+` WHERE task_id IS NOT NULL AND action_request_id IS NULL AND state='cancelled'
	 AND failure_code='outcome_uncertain' AND EXISTS (SELECT 1 FROM task_run_items i WHERE i.item_id=mcp_auth_requests.run_item_id AND i.status='running')
	 ORDER BY created_at_ms`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var result []MCPAuthRequest
	for rows.Next() {
		value, scanErr := scanMCPAuth(rows)
		if scanErr != nil {
			return nil, scanErr
		}
		result = append(result, value)
	}
	return result, rows.Err()
}

// RecoverConversationMCPAuthRequests returns direct calls cancelled during startup recovery.
func (s *Store) RecoverConversationMCPAuthRequests(ctx context.Context) ([]MCPAuthRequest, error) {
	rows, err := s.db.QueryContext(ctx, mcpAuthSelect+` WHERE conversation_id IS NOT NULL AND action_request_id IS NULL
 AND state='cancelled' AND failure_code='outcome_uncertain'
 AND EXISTS (SELECT 1 FROM conversation_items i WHERE i.item_id=mcp_auth_requests.call_item_id AND i.status='running')
 ORDER BY created_at_ms`)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var result []MCPAuthRequest
	for rows.Next() {
		value, scanErr := scanMCPAuth(rows)
		if scanErr != nil {
			return nil, scanErr
		}
		result = append(result, value)
	}
	return result, rows.Err()
}

// BeginMCPAuthResume records that one reviewed remote call can no longer be retried safely.
func (s *Store) BeginMCPAuthResume(ctx context.Context, request MCPAuthRequest, now time.Time) (MCPAuthRequest, error) {
	if request.State != "authorizing" {
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
	if request.ActionID == "" || (state != ActionSucceeded && state != ActionFailed && state != ActionOutcomeUncertain) ||
		(authState != "completed" && authState != "cancelled" && authState != "superseded") {
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
