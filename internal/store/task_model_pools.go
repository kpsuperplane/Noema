package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"strings"
	"time"
)

// TaskModelPoolEntry combines stable pool metadata with its hosted model role.
type TaskModelPoolEntry struct {
	ID         string
	Complexity string
	Label      *string
	Assignment ModelAssignment
	SortOrder  int
	CreatedAt  time.Time
	UpdatedAt  time.Time
}

const taskModelPoolSelect = `
SELECT p.pool_entry_id, p.complexity, p.label, p.sort_order,
       p.created_at_ms, p.updated_at_ms,
       a.role, a.provider_kind, a.provider_account_id, a.selection_mode,
       COALESCE(a.model_profile, ''), COALESCE(a.reasoning_effort, ''), a.fast_mode
FROM task_model_pool_settings p
JOIN hosted_model_assignments a ON a.role = CASE p.complexity
    WHEN 'simple' THEN 'simple_tasks'
    WHEN 'medium' THEN 'medium_tasks'
    WHEN 'difficult' THEN 'difficult_tasks' END`

// TaskModelPoolEntries returns the three global Executor settings.
func (s *Store) TaskModelPoolEntries(ctx context.Context, complexity *string) ([]TaskModelPoolEntry, error) {
	rows, err := s.db.QueryContext(ctx, taskModelPoolSelect+`
WHERE (? IS NULL OR p.complexity = ?)
ORDER BY CASE p.complexity WHEN 'simple' THEN 0 WHEN 'medium' THEN 1 ELSE 2 END,
    p.sort_order, p.label, p.pool_entry_id`, complexity, complexity)
	if err != nil {
		return nil, fmt.Errorf("query Task model pools: %w", err)
	}
	defer rows.Close()
	entries := make([]TaskModelPoolEntry, 0, 3)
	for rows.Next() {
		entry, err := scanTaskModelPoolEntry(rows)
		if err != nil {
			return nil, err
		}
		entries = append(entries, entry)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("read Task model pools: %w", err)
	}
	return entries, nil
}

// UpdateTaskModelPoolEntry replaces one stable global Executor setting.
func (s *Store) UpdateTaskModelPoolEntry(
	ctx context.Context,
	id string,
	complexity string,
	label *string,
	assignment ModelAssignment,
	sortOrder int,
	now time.Time,
) (TaskModelPoolEntry, error) {
	role, expectedID, ok := taskModelPoolIdentity(complexity)
	if !ok || id != expectedID || assignment.Role != role {
		return TaskModelPoolEntry{}, ErrInvalidModelAssignments
	}
	if label != nil {
		trimmed := strings.TrimSpace(*label)
		if trimmed == "" || len(trimmed) > 128 {
			return TaskModelPoolEntry{}, ErrInvalidModelAssignments
		}
		label = &trimmed
	}
	if err := validateSingleAssignment(assignment); err != nil {
		return TaskModelPoolEntry{}, err
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return TaskModelPoolEntry{}, fmt.Errorf("begin Task model pool update: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	existing, err := scanTaskModelPoolEntry(tx.QueryRowContext(ctx,
		taskModelPoolSelect+" WHERE p.pool_entry_id = ?", id))
	if err != nil {
		return TaskModelPoolEntry{}, err
	}
	if !sameModelAssignment(existing.Assignment, assignment) {
		if _, err := modelAssignmentAccount(ctx, tx, assignment); err != nil {
			return TaskModelPoolEntry{}, err
		}
		if err := saveHostedModelAssignmentTx(ctx, tx, assignment); err != nil {
			return TaskModelPoolEntry{}, err
		}
	}
	if _, err := tx.ExecContext(ctx, `
UPDATE task_model_pool_settings SET label = ?, sort_order = ?, updated_at_ms = ?
WHERE pool_entry_id = ?`, label, sortOrder, millis(now.UTC()), id); err != nil {
		return TaskModelPoolEntry{}, fmt.Errorf("update Task model pool: %w", err)
	}
	updated, err := scanTaskModelPoolEntry(tx.QueryRowContext(ctx,
		taskModelPoolSelect+" WHERE p.pool_entry_id = ?", id))
	if err != nil {
		return TaskModelPoolEntry{}, err
	}
	if err := tx.Commit(); err != nil {
		return TaskModelPoolEntry{}, fmt.Errorf("commit Task model pool update: %w", err)
	}
	return updated, nil
}

func scanTaskModelPoolEntry(row rowScanner) (TaskModelPoolEntry, error) {
	var entry TaskModelPoolEntry
	var label sql.NullString
	var fastMode int
	var createdAt, updatedAt int64
	if err := row.Scan(&entry.ID, &entry.Complexity, &label, &entry.SortOrder,
		&createdAt, &updatedAt, &entry.Assignment.Role, &entry.Assignment.ProviderKind,
		&entry.Assignment.ProviderAccountID, &entry.Assignment.SelectionMode,
		&entry.Assignment.ModelProfile, &entry.Assignment.ReasoningEffort, &fastMode); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return TaskModelPoolEntry{}, ErrTaskModelPoolEntryNotFound
		}
		return TaskModelPoolEntry{}, fmt.Errorf("scan Task model pool: %w", err)
	}
	if label.Valid {
		entry.Label = &label.String
	}
	entry.Assignment.FastMode = fastMode == 1
	entry.CreatedAt = fromMillis(createdAt)
	entry.UpdatedAt = fromMillis(updatedAt)
	return entry, nil
}

func taskModelPoolIdentity(complexity string) (HostedModelRole, string, bool) {
	switch complexity {
	case "simple":
		return HostedModelSimpleTasks, "task_pool:setting:simple", true
	case "medium":
		return HostedModelMediumTasks, "task_pool:setting:medium", true
	case "difficult":
		return HostedModelDifficultTasks, "task_pool:setting:difficult", true
	default:
		return "", "", false
	}
}

func sameModelAssignment(left, right ModelAssignment) bool {
	return left.Role == right.Role && left.ProviderKind == right.ProviderKind &&
		left.ProviderAccountID == right.ProviderAccountID &&
		left.SelectionMode == right.SelectionMode && left.ModelProfile == right.ModelProfile &&
		left.ReasoningEffort == right.ReasoningEffort && left.FastMode == right.FastMode
}
