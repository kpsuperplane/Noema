package store

import (
	"context"
	"errors"

	"github.com/uptrace/bun"
)

// TaskExecutionPolicy contains global limits or one immutable run snapshot.
type TaskExecutionPolicy struct {
	MaxProviderContinuations int64
	MaxToolCalls             int64
	MaxActiveMinutes         int64
	ProgressAuditInterval    int64
	MaxAutomaticRetries      int64
	MaxReviewRounds          int64
}

// TaskExecutionPolicy returns the policy used when the next run is claimed.
func (s *Store) TaskExecutionPolicy(ctx context.Context) (TaskExecutionPolicy, error) {
	return scanTaskExecutionPolicy(s.db.QueryRowContext(ctx, `SELECT max_provider_continuations,
max_tool_calls,max_active_minutes,progress_audit_interval,max_automatic_retries,max_review_rounds
FROM task_execution_policy WHERE policy_id='default'`))
}

func taskExecutionPolicyTx(ctx context.Context, tx bun.Tx) (TaskExecutionPolicy, error) {
	return scanTaskExecutionPolicy(tx.QueryRowContext(ctx, `SELECT max_provider_continuations,
max_tool_calls,max_active_minutes,progress_audit_interval,max_automatic_retries,max_review_rounds
FROM task_execution_policy WHERE policy_id='default'`))
}

// UpdateTaskExecutionPolicy replaces all six stored limits.
func (s *Store) UpdateTaskExecutionPolicy(ctx context.Context, value TaskExecutionPolicy) (TaskExecutionPolicy, error) {
	if err := validateTaskExecutionPolicy(value); err != nil {
		return TaskExecutionPolicy{}, err
	}
	result, err := s.db.ExecContext(ctx, `UPDATE task_execution_policy SET max_provider_continuations=?,
max_tool_calls=?,max_active_minutes=?,progress_audit_interval=?,max_automatic_retries=?,max_review_rounds=?
WHERE policy_id='default'`, value.MaxProviderContinuations, value.MaxToolCalls, value.MaxActiveMinutes,
		value.ProgressAuditInterval, value.MaxAutomaticRetries, value.MaxReviewRounds)
	if err != nil {
		return TaskExecutionPolicy{}, err
	}
	if changed, _ := result.RowsAffected(); changed != 1 {
		return TaskExecutionPolicy{}, errors.New("default Task execution policy is unavailable")
	}
	return s.TaskExecutionPolicy(ctx)
}

func validateTaskExecutionPolicy(value TaskExecutionPolicy) error {
	if value.MaxProviderContinuations < 1 || value.MaxProviderContinuations > 1000 ||
		value.MaxToolCalls < 1 || value.MaxToolCalls > 10000 ||
		value.MaxActiveMinutes < 1 || value.MaxActiveMinutes > 10080 ||
		value.ProgressAuditInterval < 1 || value.ProgressAuditInterval > value.MaxProviderContinuations ||
		value.MaxAutomaticRetries < 0 || value.MaxAutomaticRetries > 20 ||
		value.MaxReviewRounds < 1 || value.MaxReviewRounds > 20 {
		return errors.New("Task execution policy is out of bounds")
	}
	return nil
}

func scanTaskExecutionPolicy(row rowScanner) (TaskExecutionPolicy, error) {
	var value TaskExecutionPolicy
	err := row.Scan(&value.MaxProviderContinuations, &value.MaxToolCalls, &value.MaxActiveMinutes,
		&value.ProgressAuditInterval, &value.MaxAutomaticRetries, &value.MaxReviewRounds)
	if err == nil {
		err = validateTaskExecutionPolicy(value)
	}
	return value, err
}
