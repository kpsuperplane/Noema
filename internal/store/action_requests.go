package store

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/uptrace/bun"
)

const (
	actionArgumentsLimit = 1 << 20
	actionSchemaLimit    = 256 << 10
	actionContextLimit   = 256 << 10
)

// ActionRequestState is one durable request state.
type ActionRequestState string

const (
	ActionProposed         ActionRequestState = "proposed"
	ActionAwaitingApproval ActionRequestState = "awaiting_approval"
	ActionExecutable       ActionRequestState = "executable"
	ActionExecuting        ActionRequestState = "executing"
	ActionSucceeded        ActionRequestState = "succeeded"
	ActionFailed           ActionRequestState = "failed"
	ActionOutcomeUncertain ActionRequestState = "outcome_uncertain"
	ActionDeclined         ActionRequestState = "declined"
	ActionSuperseded       ActionRequestState = "superseded"
	ActionCancelled        ActionRequestState = "cancelled"
)

// ActionReviewRoute identifies the route selected before review.
type ActionReviewRoute string

const (
	ActionHumanReview ActionReviewRoute = "human_review"
	ActionLLMReview   ActionReviewRoute = "llm_review"
)

// ActionBehavior is the complete behavior snapshot shown during review.
type ActionBehavior struct {
	ReadOnly, RepeatSafe, Destructive, OpenWorld bool
}

// NewActionRequest contains one exact model proposal.
type NewActionRequest struct {
	ConversationID, TurnID, CallItemID string
	TaskID, RunID, RunItemID           string
	TaskGeneration                     int64
	OwnerHumanID, RequestingAgentID    string
	CapabilityName, OperationToken     string
	ReviewRoute                        ActionReviewRoute
	Behavior                           ActionBehavior
	Arguments, InputSchema             json.RawMessage
	AuthorizationContext               map[string]any
	SafeSummary                        string
}

// ActionAssessment is one closed reviewer result.
type ActionAssessment struct {
	Status, Authorization, Risk string
	ReviewerSelection           map[string]any
	ReasonCodes                 []string
	Explanation                 string
}

// ActionRequest is one saved proposal and its current result.
type ActionRequest struct {
	ID, OwnerHumanID, ConversationID, TurnID, CallItemID, ApprovalItemID string
	TaskID, RunID, RunItemID                                             string
	TaskGeneration                                                       int64
	RequestingAgentID, CapabilityName, OperationToken                    string
	Revision                                                             int
	ReviewRoute                                                          ActionReviewRoute
	Behavior                                                             ActionBehavior
	Arguments, InputSchema                                               map[string]any
	ArgumentsSHA256                                                      string
	AuthorizationContext                                                 map[string]any
	SafeSummary                                                          string
	State                                                                ActionRequestState
	Output                                                               map[string]any
	FailureCode                                                          string
	Assessment                                                           *ActionAssessment
	CreatedAt, UpdatedAt                                                 time.Time
}

// CreateActionRequest saves one exact call before model review.
func (s *Store) CreateActionRequest(ctx context.Context, input NewActionRequest, now time.Time) (ActionRequest, error) {
	arguments, err := boundedJSONObject(input.Arguments, actionArgumentsLimit)
	if err != nil {
		return ActionRequest{}, errors.New("action arguments are invalid")
	}
	schema, err := boundedJSONObject(input.InputSchema, actionSchemaLimit)
	if err != nil {
		return ActionRequest{}, errors.New("action input schema is invalid")
	}
	contextJSON, err := json.Marshal(input.AuthorizationContext)
	if err != nil || len(contextJSON) > actionContextLimit {
		return ActionRequest{}, errors.New("action authorization context is invalid")
	}
	if input.OwnerHumanID != "human:local" || input.RequestingAgentID == "" ||
		input.CapabilityName == "" || input.OperationToken == "" ||
		(input.ReviewRoute != ActionHumanReview && input.ReviewRoute != ActionLLMReview) ||
		strings.TrimSpace(input.SafeSummary) == "" || len(input.SafeSummary) > 1000 {
		return ActionRequest{}, errors.New("action request is invalid")
	}
	actionID, err := newID("action")
	if err != nil {
		return ActionRequest{}, err
	}
	digest := sha256.Sum256(arguments)
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ActionRequest{}, fmt.Errorf("begin action request: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	if err := requireExactActionOrigin(ctx, tx, input, arguments, "running"); err != nil {
		return ActionRequest{}, err
	}
	_, err = tx.ExecContext(ctx, `
INSERT INTO action_requests (
 action_id, revision, owner_human_id, conversation_id, turn_id, call_item_id, task_id, run_id, task_generation, run_item_id,
 requesting_agent_id, capability_name, operation_token, review_route,
 read_only, repeat_safe, destructive, open_world, arguments_json, arguments_sha256,
 input_schema_json, authorization_context_json, safe_summary, state,
 created_at_ms, updated_at_ms
) VALUES (?,1,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,'proposed',?,?)`,
		actionID, input.OwnerHumanID, nullText(input.ConversationID), nullText(input.TurnID), nullText(input.CallItemID),
		nullText(input.TaskID), nullText(input.RunID), nullableTaskGeneration(input.TaskID, input.TaskGeneration), nullText(input.RunItemID),
		input.RequestingAgentID, input.CapabilityName, input.OperationToken, input.ReviewRoute,
		input.Behavior.ReadOnly, input.Behavior.RepeatSafe, input.Behavior.Destructive,
		input.Behavior.OpenWorld, string(arguments), hex.EncodeToString(digest[:]), string(schema),
		string(contextJSON), input.SafeSummary, millis(now), millis(now))
	if err != nil {
		return ActionRequest{}, fmt.Errorf("store action request: %w", err)
	}
	if err := insertActionEvent(ctx, tx, actionID, "proposed", input.RequestingAgentID, map[string]any{"summary": input.SafeSummary}, now); err != nil {
		return ActionRequest{}, err
	}
	action, err := actionRequestTx(ctx, tx, actionID, 1)
	if err == nil && input.ReviewRoute == ActionHumanReview {
		var approvalID any
		if input.TaskID == "" {
			approval, approvalErr := insertActionApprovalItem(ctx, tx, action, now)
			if approvalErr != nil {
				return ActionRequest{}, approvalErr
			}
			approvalID = approval.ID
		}
		if _, err = tx.ExecContext(ctx, `INSERT INTO action_request_decisions
	(action_id, action_revision, state, created_at_ms) VALUES (?,1,'pending',?)`, actionID, millis(now)); err != nil {
			return ActionRequest{}, err
		}
		if _, err = tx.ExecContext(ctx, `UPDATE action_requests SET state='awaiting_approval',
	 approval_item_id=?, updated_at_ms=? WHERE action_id=? AND revision=1 AND state='proposed'`,
			approvalID, millis(now), actionID); err != nil {
			return ActionRequest{}, err
		}
		action, err = actionRequestTx(ctx, tx, actionID, 1)
	}
	if err == nil {
		err = tx.Commit()
	}
	return action, err
}

// RecordActionAssessment stores one review and prepares its next state.
func (s *Store) RecordActionAssessment(
	ctx context.Context, actionID string, revision int, assessment ActionAssessment, now time.Time,
) (ActionRequest, *ConversationItem, error) {
	recommendation, err := validateActionAssessment(assessment)
	if err != nil {
		return ActionRequest{}, nil, err
	}
	selection, _ := json.Marshal(assessment.ReviewerSelection)
	reasons, _ := json.Marshal(assessment.ReasonCodes)
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ActionRequest{}, nil, err
	}
	defer func() { _ = tx.Rollback() }()
	action, err := actionRequestTx(ctx, tx, actionID, revision)
	if err != nil || action.State != ActionProposed {
		return ActionRequest{}, nil, errors.New("action request review is stale")
	}
	if err := requireStoredActionOrigin(ctx, tx, action, "running"); err != nil {
		return ActionRequest{}, nil, err
	}
	if repeated, err := declinedBrowserEffectTx(ctx, tx, action); err != nil {
		return ActionRequest{}, nil, err
	} else if repeated {
		recommendation = "require_approval"
	}
	var storedSelection any
	if assessment.Status == "completed" {
		storedSelection = string(selection)
	}
	_, err = tx.ExecContext(ctx, `
INSERT INTO action_request_assessments (
 action_id, action_revision, status, reviewer_selection_json, authorization, risk,
 recommendation, reason_codes_json, explanation, created_at_ms
) VALUES (?,?,?,?,?,?,?,?,?,?)`, actionID, revision, assessment.Status, storedSelection,
		nullText(assessment.Authorization), nullText(assessment.Risk), recommendation,
		string(reasons), assessment.Explanation, millis(now))
	if err != nil {
		return ActionRequest{}, nil, fmt.Errorf("store action assessment: %w", err)
	}
	next := ActionExecutable
	var approval *ConversationItem
	if recommendation == "require_approval" {
		next = ActionAwaitingApproval
		if action.TaskID == "" {
			item, itemErr := insertActionApprovalItem(ctx, tx, action, now)
			if itemErr != nil {
				return ActionRequest{}, nil, itemErr
			}
			approval = &item
		}
		if _, err := tx.ExecContext(ctx, `
INSERT INTO action_request_decisions (action_id, action_revision, state, created_at_ms)
VALUES (?,?,'pending',?)`, actionID, revision, millis(now)); err != nil {
			return ActionRequest{}, nil, fmt.Errorf("store action decision: %w", err)
		}
		if err := insertActionEvent(ctx, tx, actionID, "approval_requested", "system:action_gateway", map[string]any{}, now); err != nil {
			return ActionRequest{}, nil, err
		}
	}
	if _, err := tx.ExecContext(ctx, `UPDATE action_requests SET state = ?, updated_at_ms = ?
WHERE action_id = ? AND revision = ? AND state = 'proposed'`, next, millis(now), actionID, revision); err != nil {
		return ActionRequest{}, nil, err
	}
	if err := insertActionEvent(ctx, tx, actionID, "reviewed", "system:action_reviewer", map[string]any{"recommendation": recommendation}, now); err != nil {
		return ActionRequest{}, nil, err
	}
	action, err = actionRequestTx(ctx, tx, actionID, revision)
	if err == nil {
		err = tx.Commit()
	}
	return action, approval, err
}

// DecideActionRequest applies one human decision to one exact revision.
func (s *Store) DecideActionRequest(
	ctx context.Context, actionID string, revision int, humanID, decision string, now time.Time,
) (ActionRequest, error) {
	if humanID != "human:local" || (decision != "approve" && decision != "decline") {
		return ActionRequest{}, errors.New("action decision is invalid")
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ActionRequest{}, err
	}
	defer func() { _ = tx.Rollback() }()
	action, err := actionRequestTx(ctx, tx, actionID, revision)
	if err != nil || action.OwnerHumanID != humanID || action.State != ActionAwaitingApproval {
		return ActionRequest{}, errors.New("action decision is stale or unavailable")
	}
	decisionState, next, event := "approved", ActionExecutable, "approved"
	var completed any
	if decision == "decline" {
		decisionState, next, event, completed = "declined", ActionDeclined, "declined", millis(now)
	}
	result, err := tx.ExecContext(ctx, `UPDATE action_request_decisions
SET state = ?, decided_by_human_id = ?, decided_at_ms = ?
WHERE action_id = ? AND action_revision = ? AND state = 'pending'`,
		decisionState, humanID, millis(now), actionID, revision)
	if err != nil {
		return ActionRequest{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return ActionRequest{}, errors.New("action decision is stale or unavailable")
	}
	if _, err := tx.ExecContext(ctx, `UPDATE action_requests
SET state = ?, completed_at_ms = ?, updated_at_ms = ?
WHERE action_id = ? AND revision = ? AND state = 'awaiting_approval'`,
		next, completed, millis(now), actionID, revision); err != nil {
		return ActionRequest{}, err
	}
	if err := insertActionEvent(ctx, tx, actionID, event, humanID, map[string]any{}, now); err != nil {
		return ActionRequest{}, err
	}
	action, err = actionRequestTx(ctx, tx, actionID, revision)
	if err == nil {
		err = tx.Commit()
	}
	return action, err
}

// ClaimActionRequest admits one exact call for one execution attempt.
func (s *Store) ClaimActionRequest(ctx context.Context, actionID string, revision int, now time.Time) (ActionRequest, error) {
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ActionRequest{}, err
	}
	defer func() { _ = tx.Rollback() }()
	action, err := actionRequestTx(ctx, tx, actionID, revision)
	if err != nil || action.State != ActionExecutable {
		return ActionRequest{}, errors.New("action request is not executable")
	}
	if err := requireStoredActionOrigin(ctx, tx, action, ""); err != nil {
		return ActionRequest{}, err
	}
	var decisionState string
	err = tx.QueryRowContext(ctx, `SELECT state FROM action_request_decisions
WHERE action_id = ? AND action_revision = ?`, actionID, revision).Scan(&decisionState)
	if err == nil {
		if decisionState != "approved" {
			return ActionRequest{}, errors.New("action approval is not executable")
		}
		if _, err := tx.ExecContext(ctx, `UPDATE action_request_decisions
SET state = 'consumed', consumed_at_ms = ?
WHERE action_id = ? AND action_revision = ? AND state = 'approved'`, millis(now), actionID, revision); err != nil {
			return ActionRequest{}, err
		}
	} else if !errors.Is(err, sql.ErrNoRows) {
		return ActionRequest{}, err
	} else if repeated, err := declinedBrowserEffectTx(ctx, tx, action); err != nil {
		return ActionRequest{}, err
	} else if repeated {
		return ActionRequest{}, errors.New("a declined browser effect requires new human approval")
	}
	result, err := tx.ExecContext(ctx, `UPDATE action_requests SET state = 'executing', updated_at_ms = ?
WHERE action_id = ? AND revision = ? AND state = 'executable'`, millis(now), actionID, revision)
	if err != nil {
		return ActionRequest{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return ActionRequest{}, errors.New("action request execution was already claimed")
	}
	if err := insertActionEvent(ctx, tx, actionID, "execution_started", "system:action_gateway", map[string]any{}, now); err != nil {
		return ActionRequest{}, err
	}
	action, err = actionRequestTx(ctx, tx, actionID, revision)
	if err == nil {
		err = tx.Commit()
	}
	return action, err
}

// FinishActionRequest stores one known or uncertain external outcome.
func (s *Store) FinishActionRequest(
	ctx context.Context, actionID string, revision int, state ActionRequestState,
	output json.RawMessage, failureCode string, now time.Time,
) (ActionRequest, error) {
	if state != ActionSucceeded && state != ActionFailed && state != ActionOutcomeUncertain {
		return ActionRequest{}, errors.New("action outcome is invalid")
	}
	var storedOutput any
	if len(output) != 0 {
		encoded, err := boundedJSONObject(output, actionArgumentsLimit)
		if err != nil {
			return ActionRequest{}, errors.New("action output is invalid")
		}
		storedOutput = string(encoded)
	}
	var storedFailure any
	if failureCode != "" {
		storedFailure = failureCode
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ActionRequest{}, err
	}
	defer func() { _ = tx.Rollback() }()
	result, err := tx.ExecContext(ctx, `UPDATE action_requests
SET state = ?, output_json = ?, failure_code = ?, completed_at_ms = ?, updated_at_ms = ?
WHERE action_id = ? AND revision = ? AND state = 'executing'`, state, storedOutput,
		storedFailure, millis(now), millis(now), actionID, revision)
	if err != nil {
		return ActionRequest{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return ActionRequest{}, errors.New("action execution claim is stale")
	}
	if err := insertActionEvent(ctx, tx, actionID, string(state), "system:action_gateway",
		map[string]any{"failure_code": nullText(failureCode)}, now); err != nil {
		return ActionRequest{}, err
	}
	action, err := actionRequestTx(ctx, tx, actionID, revision)
	if err == nil {
		err = tx.Commit()
	}
	return action, err
}

// SupersedeActionRequest closes one request after live authority changes.
func (s *Store) SupersedeActionRequest(
	ctx context.Context, actionID string, revision int, reason string, now time.Time,
) (ActionRequest, error) {
	if reason == "" || len(reason) > 128 {
		return ActionRequest{}, errors.New("action supersession reason is invalid")
	}
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ActionRequest{}, err
	}
	defer func() { _ = tx.Rollback() }()
	result, err := tx.ExecContext(ctx, `UPDATE action_requests
SET state = 'superseded', failure_code = ?, completed_at_ms = ?, updated_at_ms = ?
WHERE action_id = ? AND revision = ? AND state IN ('awaiting_approval','executable')`,
		reason, millis(now), millis(now), actionID, revision)
	if err != nil {
		return ActionRequest{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return ActionRequest{}, errors.New("action request is no longer executable")
	}
	if _, err := tx.ExecContext(ctx, `UPDATE action_request_decisions SET state = 'superseded'
WHERE action_id = ? AND action_revision = ? AND state IN ('pending','approved')`, actionID, revision); err != nil {
		return ActionRequest{}, err
	}
	if err := insertActionEvent(ctx, tx, actionID, "superseded", "system:action_gateway",
		map[string]any{"reason": reason}, now); err != nil {
		return ActionRequest{}, err
	}
	action, err := actionRequestTx(ctx, tx, actionID, revision)
	if err == nil {
		err = tx.Commit()
	}
	return action, err
}

// ActionConversationCall reloads the exact saved provider call for terminal replay.
func (s *Store) ActionConversationCall(
	ctx context.Context, action ActionRequest,
) (ConversationTurn, ConversationToolResultInput, error) {
	var turn ConversationTurn
	var payload string
	if err := s.db.QueryRowContext(ctx, `SELECT t.turn_id, t.conversation_id,
CAST(json_extract(t.metadata_json, '$.turn_index') AS INTEGER), t.status, i.payload_json
FROM conversation_turns t JOIN conversation_items i
 ON i.conversation_id = t.conversation_id AND i.turn_id = t.turn_id
WHERE t.conversation_id = ? AND t.turn_id = ? AND i.item_id = ?
 AND i.kind = 'tool_call' AND i.status = 'running'`,
		action.ConversationID, action.TurnID, action.CallItemID).Scan(
		&turn.ID, &turn.ConversationID, &turn.TurnIndex, &turn.Status, &payload); err != nil {
		return ConversationTurn{}, ConversationToolResultInput{}, errors.New("action origin call is unavailable")
	}
	var item map[string]any
	if json.Unmarshal([]byte(payload), &item) != nil {
		return ConversationTurn{}, ConversationToolResultInput{}, errors.New("action origin call is invalid")
	}
	metadata, _ := item["metadata"].(map[string]any)
	call, _ := metadata["action"].(map[string]any)
	providerRound, _ := actionNumber(action.AuthorizationContext, "provider_round")
	outputIndex, _ := actionNumber(metadata, "output_index")
	provider, _ := metadata["provider"].(string)
	providerCallID, _ := call["provider_call_id"].(string)
	providerName, _ := call["provider_name"].(string)
	name, _ := call["name"].(string)
	if provider == "" || providerCallID == "" || providerName == "" || name != action.CapabilityName {
		return ConversationTurn{}, ConversationToolResultInput{}, errors.New("action origin call is invalid")
	}
	return turn, ConversationToolResultInput{
		CallItemID: action.CallItemID, Provider: provider, ProviderRound: providerRound,
		OutputIndex: outputIndex, ProviderCallID: providerCallID, ProviderName: providerName, Name: name,
	}, nil
}

func actionNumber(value map[string]any, key string) (int, bool) {
	number, ok := value[key].(float64)
	return int(number), ok && number >= 0 && number <= float64(^uint(0)>>1)
}

// ActionRequest returns one exact request revision.
func (s *Store) ActionRequest(ctx context.Context, actionID string, revision int) (ActionRequest, error) {
	return scanActionRequest(s.db.QueryRowContext(ctx, actionSelect+` WHERE a.action_id = ? AND a.revision = ?`, actionID, revision))
}

// RecoverActionRequests closes interrupted claims and returns terminal calls that need durable results.
func (s *Store) RecoverActionRequests(ctx context.Context, now time.Time) ([]ActionRequest, error) {
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, err
	}
	defer func() { _ = tx.Rollback() }()
	rows, err := tx.QueryContext(ctx, `SELECT a.action_id, a.state
FROM action_requests a JOIN conversation_items i
 ON i.conversation_id = a.conversation_id AND i.item_id = a.call_item_id
WHERE i.status = 'running' AND a.state <> 'awaiting_approval'
 AND NOT EXISTS (SELECT 1 FROM mcp_auth_requests m WHERE m.action_request_id=a.action_id
  AND m.state IN ('awaiting_user','authorizing','resuming'))
ORDER BY a.created_at_ms, a.action_id`)
	if err != nil {
		return nil, err
	}
	type interrupted struct {
		id    string
		state ActionRequestState
	}
	items := make([]interrupted, 0)
	for rows.Next() {
		var item interrupted
		if err := rows.Scan(&item.id, &item.state); err != nil {
			_ = rows.Close()
			return nil, err
		}
		items = append(items, item)
	}
	if err := rows.Close(); err != nil {
		return nil, err
	}
	result := make([]ActionRequest, 0, len(items))
	for _, item := range items {
		switch item.state {
		case ActionProposed, ActionExecutable:
			if _, err := tx.ExecContext(ctx, `UPDATE action_requests SET state = 'cancelled',
failure_code = 'execution_interrupted', completed_at_ms = ?, updated_at_ms = ?
WHERE action_id = ? AND revision = 1 AND state IN ('proposed','executable')`,
				millis(now), millis(now), item.id); err != nil {
				return nil, err
			}
			if _, err := tx.ExecContext(ctx, `UPDATE action_request_decisions SET state = 'superseded'
WHERE action_id = ? AND action_revision = 1 AND state IN ('pending','approved')`, item.id); err != nil {
				return nil, err
			}
			if err := insertActionEvent(ctx, tx, item.id, "cancelled", "system:recovery",
				map[string]any{"reason": "execution_interrupted"}, now); err != nil {
				return nil, err
			}
		case ActionExecuting:
			if _, err := tx.ExecContext(ctx, `UPDATE action_requests SET state = 'outcome_uncertain',
failure_code = 'outcome_uncertain', completed_at_ms = ?, updated_at_ms = ?
WHERE action_id = ? AND revision = 1 AND state = 'executing'`,
				millis(now), millis(now), item.id); err != nil {
				return nil, err
			}
			if err := insertActionEvent(ctx, tx, item.id, "outcome_uncertain", "system:recovery",
				map[string]any{"failure_code": "outcome_uncertain"}, now); err != nil {
				return nil, err
			}
		}
		action, err := actionRequestTx(ctx, tx, item.id, 1)
		if err != nil {
			return nil, err
		}
		result = append(result, action)
	}
	if err := tx.Commit(); err != nil {
		return nil, err
	}
	return result, nil
}

// RecoverTaskActionRequests closes interrupted Task claims and returns missing results.
func (s *Store) RecoverTaskActionRequests(ctx context.Context, now time.Time) ([]ActionRequest, error) {
	now = now.UTC()
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, err
	}
	defer tx.Rollback()
	rows, err := tx.QueryContext(ctx, `SELECT a.action_id,a.state FROM action_requests a
JOIN task_run_items i ON i.run_id=a.run_id AND i.item_id=a.run_item_id
JOIN task_runs r ON r.run_id=a.run_id JOIN tasks t ON t.current_run_id=r.run_id AND t.generation=a.task_generation
WHERE a.task_id IS NOT NULL AND i.status='running' AND a.state<>'awaiting_approval'
 AND NOT EXISTS (SELECT 1 FROM mcp_auth_requests m WHERE m.action_request_id=a.action_id AND m.state IN ('awaiting_user','authorizing','resuming'))
ORDER BY a.created_at_ms,a.action_id`)
	if err != nil {
		return nil, err
	}
	type interrupted struct {
		id    string
		state ActionRequestState
	}
	var items []interrupted
	for rows.Next() {
		var item interrupted
		if err = rows.Scan(&item.id, &item.state); err != nil {
			_ = rows.Close()
			return nil, err
		}
		items = append(items, item)
	}
	if err = rows.Close(); err != nil {
		return nil, err
	}
	result := make([]ActionRequest, 0, len(items))
	for _, item := range items {
		switch item.state {
		case ActionProposed, ActionExecutable:
			if _, err = tx.ExecContext(ctx, `UPDATE action_requests SET state='cancelled',failure_code='execution_interrupted',completed_at_ms=?,updated_at_ms=? WHERE action_id=? AND state IN ('proposed','executable')`, millis(now), millis(now), item.id); err != nil {
				return nil, err
			}
			if _, err = tx.ExecContext(ctx, `UPDATE action_request_decisions SET state='superseded' WHERE action_id=? AND state IN ('pending','approved')`, item.id); err != nil {
				return nil, err
			}
			if err = insertActionEvent(ctx, tx, item.id, "cancelled", "system:recovery", map[string]any{"reason": "execution_interrupted"}, now); err != nil {
				return nil, err
			}
		case ActionExecuting:
			if _, err = tx.ExecContext(ctx, `UPDATE action_requests SET state='outcome_uncertain',failure_code='outcome_uncertain',completed_at_ms=?,updated_at_ms=? WHERE action_id=? AND state='executing'`, millis(now), millis(now), item.id); err != nil {
				return nil, err
			}
			if err = insertActionEvent(ctx, tx, item.id, "outcome_uncertain", "system:recovery", map[string]any{"failure_code": "outcome_uncertain"}, now); err != nil {
				return nil, err
			}
		}
		action, loadErr := actionRequestTx(ctx, tx, item.id, 1)
		if loadErr != nil {
			return nil, loadErr
		}
		if _, err = tx.ExecContext(ctx, `UPDATE task_runs SET status='waiting_for_approval',updated_at_ms=? WHERE run_id=? AND status IN ('leased','running','queued')`, millis(now), action.RunID); err != nil {
			return nil, err
		}
		result = append(result, action)
	}
	if err = tx.Commit(); err != nil {
		return nil, err
	}
	return result, nil
}

// PendingActionRequests lists pending requests for the local human.
func (s *Store) PendingActionRequests(
	ctx context.Context, humanID string, conversationID, taskID *string, limit int,
) ([]ActionRequest, error) {
	if humanID != "human:local" || limit < 1 || limit > 100 {
		return nil, errors.New("pending action request is invalid")
	}
	rows, err := s.db.QueryContext(ctx, actionSelect+`
WHERE a.owner_human_id = ? AND a.state = 'awaiting_approval'
  AND (? IS NULL OR a.conversation_id = ?)
  AND (? IS NULL OR a.task_id = ?)
  AND (a.task_id IS NULL OR EXISTS (
    SELECT 1 FROM tasks t
    WHERE t.task_id = a.task_id
      AND t.state NOT IN ('completed','cancelled')
      AND t.generation = a.task_generation
      AND t.current_run_id = a.run_id
  ))
ORDER BY a.created_at_ms DESC, a.action_id DESC LIMIT ?`, humanID, conversationID, conversationID, taskID, taskID, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	actions := make([]ActionRequest, 0)
	for rows.Next() {
		action, err := scanActionRequest(rows)
		if err != nil {
			return nil, err
		}
		actions = append(actions, action)
	}
	return actions, rows.Err()
}

// ConversationAuthorizationContext returns the bounded human authority for a turn.
func (s *Store) ConversationAuthorizationContext(ctx context.Context, conversationID, turnID string) (map[string]any, error) {
	anchorTurnID := turnID
	var anchorSequence int64
	var anchorKind, anchorAuthor string
	for hops := 0; hops < 16; hops++ {
		err := s.db.QueryRowContext(ctx, `
SELECT sequence_index, kind, author_actor_id
FROM conversation_items
WHERE conversation_id = ? AND turn_id = ? AND deleted_at_ms IS NULL
  AND status = 'completed'
  AND kind IN ('user_text', 'multiple_choice_selection')
ORDER BY sequence_index DESC LIMIT 1`, conversationID, anchorTurnID).
			Scan(&anchorSequence, &anchorKind, &anchorAuthor)
		if err == nil {
			break
		}
		if !errors.Is(err, sql.ErrNoRows) {
			return nil, err
		}
		var triggerItemID string
		if err := s.db.QueryRowContext(ctx, `SELECT COALESCE(trigger_item_id, '')
FROM conversation_turns WHERE conversation_id = ? AND turn_id = ?`, conversationID, anchorTurnID).Scan(&triggerItemID); err != nil || triggerItemID == "" {
			return nil, errors.New("action authorization source is unavailable")
		}
		var parentTurnID string
		if err := s.db.QueryRowContext(ctx, `SELECT COALESCE(turn_id, '')
FROM conversation_items WHERE conversation_id = ? AND item_id = ? AND deleted_at_ms IS NULL`, conversationID, triggerItemID).Scan(&parentTurnID); err != nil || parentTurnID == "" || parentTurnID == anchorTurnID {
			return nil, errors.New("action authorization source is unavailable")
		}
		anchorTurnID = parentTurnID
	}
	if anchorSequence == 0 {
		return nil, errors.New("action authorization source is unavailable")
	}
	if anchorAuthor != "human:local" || (anchorKind != "user_text" && anchorKind != "multiple_choice_selection") {
		return nil, errors.New("action authorization source is unavailable")
	}
	rows, err := s.db.QueryContext(ctx, `
SELECT item_id, kind, author_actor_id, content_text FROM conversation_items
WHERE conversation_id = ? AND deleted_at_ms IS NULL AND status = 'completed'
  AND content_text IS NOT NULL AND kind IN ('user_text','assistant_text','multiple_choice_prompt','multiple_choice_selection')
  AND sequence_index <= ?
ORDER BY sequence_index DESC LIMIT 7`, conversationID, anchorSequence)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	messages := make([]map[string]any, 0, 7)
	for rows.Next() {
		var id, kind, author, text string
		if err := rows.Scan(&id, &kind, &author, &text); err != nil {
			return nil, err
		}
		role := "assistant"
		if kind == "user_text" || kind == "multiple_choice_selection" {
			if author != "human:local" {
				return nil, errors.New("action authorization source is unavailable")
			}
			role = "human"
		} else if kind != "assistant_text" && kind != "multiple_choice_prompt" {
			return nil, errors.New("action authorization source is unavailable")
		}
		messages = append(messages, map[string]any{"item_id": id, "role": role, "text": text})
	}
	for left, right := 0, len(messages)-1; left < right; left, right = left+1, right-1 {
		messages[left], messages[right] = messages[right], messages[left]
	}
	if len(messages) == 0 || messages[len(messages)-1]["role"] != "human" {
		return nil, errors.New("action authorization source is unavailable")
	}
	context := map[string]any{"kind": "conversation_excerpt", "messages": messages}
	encoded, _ := json.Marshal(context)
	if len(encoded) > actionContextLimit {
		return nil, errors.New("action authorization context is too large")
	}
	return context, rows.Err()
}

func insertActionApprovalItem(ctx context.Context, tx bun.Tx, action ActionRequest, now time.Time) (ConversationItem, error) {
	digest := sha256.Sum256([]byte(action.ID + ":approval"))
	itemID := "item:" + hex.EncodeToString(digest[:16])
	sequence, err := nextConversationSequenceTx(ctx, tx, action.ConversationID)
	if err != nil {
		return ConversationItem{}, err
	}
	item, err := insertConversationOutputTx(ctx, tx, ConversationItem{
		ID: itemID, ConversationID: action.ConversationID, TurnID: action.TurnID,
		ParentItemID: action.CallItemID, Sequence: sequence, Kind: ConversationApprovalRequest,
		Status: "completed", AuthorActorID: "agent:primary", ContentText: "Approval requested",
		Payload: map[string]any{
			"id": "approval_request:" + action.ID, "activity_kind": "approval_request",
			"status": "completed", "title": "Approval requested", "summary": action.CapabilityName,
			"metadata": map[string]any{"action": map[string]any{
				"id": action.ID, "method": action.CapabilityName,
				"payload": map[string]any{"revision": action.Revision},
			}},
		},
		Metadata: map[string]any{"source": "action_request"}, CreatedAt: now,
	})
	if err != nil {
		return ConversationItem{}, err
	}
	if _, err := tx.ExecContext(ctx, `UPDATE action_requests SET approval_item_id = ?
WHERE action_id = ? AND revision = ? AND approval_item_id IS NULL`, itemID, action.ID, action.Revision); err != nil {
		return ConversationItem{}, err
	}
	if _, err := tx.ExecContext(ctx, `UPDATE conversation_turns
SET status = 'completed', completed_at_ms = ?, updated_at_ms = ?
WHERE turn_id = ? AND conversation_id = ? AND status = 'running'`,
		millis(now), millis(now), action.TurnID, action.ConversationID); err != nil {
		return ConversationItem{}, err
	}
	if _, err := tx.ExecContext(ctx, `UPDATE conversations SET agent_status = 'idle', updated_at_ms = ?
WHERE conversation_id = ?`, millis(now), action.ConversationID); err != nil {
		return ConversationItem{}, err
	}
	return item, nil
}

func validateActionAssessment(value ActionAssessment) (string, error) {
	if strings.TrimSpace(value.Explanation) == "" || utf8.RuneCountInString(value.Explanation) > 4000 || len(value.ReasonCodes) > 16 {
		return "", errors.New("action assessment is invalid")
	}
	for _, code := range value.ReasonCodes {
		if strings.TrimSpace(code) == "" || len(code) > 128 {
			return "", errors.New("action assessment is invalid")
		}
	}
	if value.Status != "completed" {
		if value.Status != "reviewer_unavailable" && value.Status != "invalid_response" ||
			value.Authorization != "" || value.Risk != "" || value.ReviewerSelection != nil {
			return "", errors.New("action assessment is invalid")
		}
		return "require_approval", nil
	}
	validAuthorization := value.Authorization == "explicit" || value.Authorization == "substantive" ||
		value.Authorization == "weak" || value.Authorization == "absent"
	validRisk := value.Risk == "low" || value.Risk == "medium" || value.Risk == "high" || value.Risk == "critical"
	if !validAuthorization || !validRisk || value.ReviewerSelection == nil {
		return "", errors.New("action assessment is invalid")
	}
	if (value.Authorization == "explicit" || value.Authorization == "substantive") &&
		(value.Risk == "low" || value.Risk == "medium") || value.Authorization == "weak" && value.Risk == "low" {
		return "auto_execute", nil
	}
	return "require_approval", nil
}

func requireExactActionOrigin(ctx context.Context, tx bun.Tx, input NewActionRequest, arguments []byte, turnState string) error {
	if input.TaskID != "" {
		var status, kind, itemStatus, payload string
		var generation int64
		err := tx.QueryRowContext(ctx, `SELECT r.status,r.task_generation,i.item_kind,i.status,i.payload_json
FROM task_runs r JOIN tasks t ON t.task_id=r.task_id JOIN task_run_items i ON i.run_id=r.run_id
WHERE r.run_id=? AND r.task_id=? AND i.item_id=? AND t.current_run_id=r.run_id AND t.generation=r.task_generation`,
			input.RunID, input.TaskID, input.RunItemID).Scan(&status, &generation, &kind, &itemStatus, &payload)
		if err != nil || generation != input.TaskGeneration || status != turnState || kind != "tool_call" || itemStatus != "running" {
			return errors.New("action Task origin is unavailable")
		}
		var item map[string]any
		if json.Unmarshal([]byte(payload), &item) != nil {
			return errors.New("action Task origin is invalid")
		}
		storedArguments, _ := json.Marshal(item["arguments"])
		if item["name"] != input.CapabilityName || string(storedArguments) != string(arguments) {
			return errors.New("action Task call does not match the request")
		}
		return nil
	}
	var storedTurn, kind, status, payload string
	err := tx.QueryRowContext(ctx, `SELECT t.status, i.kind, i.status, i.payload_json
FROM conversation_turns t JOIN conversation_items i
 ON i.conversation_id = t.conversation_id AND i.turn_id = t.turn_id
WHERE t.conversation_id = ? AND t.turn_id = ? AND i.item_id = ?`,
		input.ConversationID, input.TurnID, input.CallItemID).Scan(&storedTurn, &kind, &status, &payload)
	if err != nil || storedTurn != turnState || kind != "tool_call" || status != "running" {
		return errors.New("action origin execution is unavailable")
	}
	var item map[string]any
	if json.Unmarshal([]byte(payload), &item) != nil {
		return errors.New("action origin execution is invalid")
	}
	action, _ := item["metadata"].(map[string]any)["action"].(map[string]any)
	storedArguments, _ := json.Marshal(action["payload"])
	if action["name"] != input.CapabilityName || string(storedArguments) != string(arguments) {
		return errors.New("action origin call does not match the request")
	}
	return nil
}

func requireStoredActionOrigin(ctx context.Context, tx bun.Tx, action ActionRequest, requiredTurnState string) error {
	arguments, _ := json.Marshal(action.Arguments)
	input := NewActionRequest{ConversationID: action.ConversationID, TurnID: action.TurnID,
		CallItemID: action.CallItemID, TaskID: action.TaskID, RunID: action.RunID, RunItemID: action.RunItemID,
		TaskGeneration: action.TaskGeneration, CapabilityName: action.CapabilityName}
	if requiredTurnState != "" {
		return requireExactActionOrigin(ctx, tx, input, arguments, requiredTurnState)
	}
	states := []string{"running", "completed"}
	if action.TaskID != "" {
		states = []string{"running", "waiting_for_approval"}
	}
	for _, state := range states {
		if requireExactActionOrigin(ctx, tx, input, arguments, state) == nil {
			return nil
		}
	}
	return errors.New("action origin execution is stale")
}

func boundedJSONObject(raw json.RawMessage, limit int) ([]byte, error) {
	if len(raw) == 0 || len(raw) > limit {
		return nil, errors.New("JSON object exceeds its limit")
	}
	value, err := conversationJSONObject(raw)
	if err != nil {
		return nil, err
	}
	return json.Marshal(value)
}

func insertActionEvent(ctx context.Context, tx bun.Tx, actionID, kind, actor string, payload map[string]any, now time.Time) error {
	eventID, err := newID("action_event")
	if err != nil {
		return err
	}
	encoded, _ := json.Marshal(payload)
	_, err = tx.ExecContext(ctx, `INSERT INTO action_request_events
(event_id, action_id, action_revision, event_kind, actor_id, safe_payload_json, created_at_ms)
VALUES (?,?,1,?,?,?,?)`, eventID, actionID, kind, actor, string(encoded), millis(now))
	return err
}

func nullText(value string) any {
	if value == "" {
		return nil
	}
	return value
}

func nullableTaskGeneration(taskID string, generation int64) any {
	if taskID == "" {
		return nil
	}
	return generation
}

const actionSelect = `
SELECT a.action_id, a.revision, a.owner_human_id, a.conversation_id, a.turn_id,
 a.call_item_id, a.task_id, a.run_id, a.task_generation, a.run_item_id,
 COALESCE(a.approval_item_id,''), a.requesting_agent_id,
 a.capability_name, a.operation_token, a.review_route, a.read_only, a.repeat_safe,
 a.destructive, a.open_world, a.arguments_json, a.arguments_sha256,
 a.input_schema_json, a.authorization_context_json, a.safe_summary, a.state,
 COALESCE(a.output_json,''), COALESCE(a.failure_code,''), a.created_at_ms, a.updated_at_ms,
 aa.status, COALESCE(aa.reviewer_selection_json,''), COALESCE(aa.authorization,''),
 COALESCE(aa.risk,''), COALESCE(aa.reason_codes_json,'[]'), COALESCE(aa.explanation,'')
FROM action_requests a LEFT JOIN action_request_assessments aa
 ON aa.action_id = a.action_id AND aa.action_revision = a.revision`

func actionRequestTx(ctx context.Context, tx bun.Tx, actionID string, revision int) (ActionRequest, error) {
	return scanActionRequest(tx.QueryRowContext(ctx, actionSelect+` WHERE a.action_id = ? AND a.revision = ?`, actionID, revision))
}

func scanActionRequest(row rowScanner) (ActionRequest, error) {
	var action ActionRequest
	var arguments, schema, contextJSON, output string
	var readOnly, repeatSafe, destructive, openWorld int
	var created, updated int64
	var assessmentStatus, reviewer, authorization, risk, reasons, explanation sql.NullString
	var conversationID, turnID, callItemID, taskID, runID, runItemID sql.NullString
	var taskGeneration sql.NullInt64
	if err := row.Scan(&action.ID, &action.Revision, &action.OwnerHumanID,
		&conversationID, &turnID, &callItemID, &taskID, &runID, &taskGeneration, &runItemID, &action.ApprovalItemID,
		&action.RequestingAgentID, &action.CapabilityName, &action.OperationToken,
		&action.ReviewRoute, &readOnly, &repeatSafe, &destructive, &openWorld,
		&arguments, &action.ArgumentsSHA256, &schema, &contextJSON, &action.SafeSummary,
		&action.State, &output, &action.FailureCode, &created, &updated,
		&assessmentStatus, &reviewer, &authorization, &risk, &reasons, &explanation); err != nil {
		return ActionRequest{}, err
	}
	action.ConversationID, action.TurnID, action.CallItemID = conversationID.String, turnID.String, callItemID.String
	action.TaskID, action.RunID, action.RunItemID, action.TaskGeneration = taskID.String, runID.String, runItemID.String, taskGeneration.Int64
	action.Behavior = ActionBehavior{readOnly == 1, repeatSafe == 1, destructive == 1, openWorld == 1}
	action.CreatedAt, action.UpdatedAt = fromMillis(created), fromMillis(updated)
	if json.Unmarshal([]byte(arguments), &action.Arguments) != nil ||
		json.Unmarshal([]byte(schema), &action.InputSchema) != nil ||
		json.Unmarshal([]byte(contextJSON), &action.AuthorizationContext) != nil {
		return ActionRequest{}, errors.New("stored action request JSON is invalid")
	}
	if output != "" && json.Unmarshal([]byte(output), &action.Output) != nil {
		return ActionRequest{}, errors.New("stored action output is invalid")
	}
	if assessmentStatus.Valid {
		assessment := &ActionAssessment{Status: assessmentStatus.String,
			Authorization: authorization.String, Risk: risk.String, Explanation: explanation.String}
		if reviewer.String != "" {
			_ = json.Unmarshal([]byte(reviewer.String), &assessment.ReviewerSelection)
		}
		_ = json.Unmarshal([]byte(reasons.String), &assessment.ReasonCodes)
		action.Assessment = assessment
	}
	return action, nil
}

// A new browser reference cannot replace a human decision about the same effect.
func declinedBrowserEffectTx(ctx context.Context, tx bun.Tx, action ActionRequest) (bool, error) {
	if action.CapabilityName != "web.browse.interact" {
		return false, nil
	}
	effect := browserEffect(action.Arguments, action.AuthorizationContext)
	rows, err := tx.QueryContext(ctx, `SELECT arguments_json, authorization_context_json FROM action_requests
 WHERE state='declined' AND owner_human_id=? AND capability_name=?
 AND ((task_id=? AND task_generation=?) OR (conversation_id=? AND turn_id=?))`,
		action.OwnerHumanID, action.CapabilityName, action.TaskID, action.TaskGeneration, action.ConversationID, action.TurnID)
	if err != nil {
		return false, err
	}
	defer rows.Close()
	for rows.Next() {
		var argumentsJSON, contextJSON string
		if err := rows.Scan(&argumentsJSON, &contextJSON); err != nil {
			return false, err
		}
		var arguments, context map[string]any
		if err := json.Unmarshal([]byte(argumentsJSON), &arguments); err != nil {
			return false, err
		}
		if err := json.Unmarshal([]byte(contextJSON), &context); err != nil {
			return false, err
		}
		if effect == browserEffect(arguments, context) {
			return true, nil
		}
	}
	return false, rows.Err()
}

func browserEffect(arguments, context map[string]any) string {
	input := cloneJSONMap(arguments)
	delete(input, "ref")
	delete(input, "snapshot_revision")
	review, _ := context["browser_review_context"].(map[string]any)
	target, _ := review["target"].(map[string]any)
	target = cloneJSONMap(target)
	delete(target, "ref")
	page, _ := review["page"].(map[string]any)
	encoded, _ := json.Marshal(map[string]any{"input": input, "url": page["url"], "target": target})
	return string(encoded)
}
