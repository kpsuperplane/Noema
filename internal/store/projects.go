package store

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/base64"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

var (
	ErrProjectNotFound   = errors.New("project not found")
	ErrStaleRevision     = errors.New("stale revision")
	ErrInvalidTransition = errors.New("invalid transition")
	ErrCommandConflict   = errors.New("command receipt conflict")
	ErrInvalidCursor     = errors.New("invalid cursor")
)

// Project is one stored personal Project.
type Project struct {
	ID, WorkspaceID, Name, Description string
	Folder                             *string
	Revision                           int64
	ArchivedAt                         *time.Time
	CreatedAt, UpdatedAt               time.Time
}

// WorkEvent is one globally ordered Tasks event.
type WorkEvent struct {
	ID, SubjectRevision      int64
	EventID                  string
	WorkspaceID, Kind        string
	ProjectID, TaskID, RunID string
	ActorID                  string
	CausationID              *string
	CorrelationID            string
	Payload                  map[string]any
	OccurredAt               time.Time
}

// ProjectCommand identifies one repeat-safe Project command.
type ProjectCommand struct {
	ActorID, Name, ClientMutationID, RequestDigest string
}

// ProjectResult is one committed or replayed Project command result.
type ProjectResult struct {
	Project        Project
	Event          WorkEvent
	DocumentDigest string
	Replayed       bool
}

// ProjectChanges contains optional Project metadata replacements.
type ProjectChanges struct {
	Name, Description *string
	Folder            *string
	SetFolder         bool
	DocumentChanged   bool
	DocumentDigest    string
}

// ProjectPage is one bounded Project list page.
type ProjectPage struct {
	Projects    []Project
	Cursors     []string
	EndCursor   *string
	HasNextPage bool
}

// NewProjectID creates one portable Project identifier.
func NewProjectID() (string, error) { return newID("project") }

// EncodeWorkEventCursor returns the stable client cursor for one event sequence.
func EncodeWorkEventCursor(sequence int64) (string, error) {
	if sequence <= 0 {
		return "", ErrInvalidCursor
	}
	return base64.RawURLEncoding.EncodeToString([]byte("work-event:v1:" + strconv.FormatInt(sequence, 10))), nil
}

// DecodeWorkEventCursor validates one stable client cursor.
func DecodeWorkEventCursor(cursor string) (int64, error) {
	decoded, err := base64.RawURLEncoding.DecodeString(cursor)
	if err != nil || base64.RawURLEncoding.EncodeToString(decoded) != cursor {
		return 0, ErrInvalidCursor
	}
	value, found := strings.CutPrefix(string(decoded), "work-event:v1:")
	if !found || value == "" || len(value) > 1 && value[0] == '0' {
		return 0, ErrInvalidCursor
	}
	sequence, err := strconv.ParseInt(value, 10, 64)
	if err != nil || sequence <= 0 {
		return 0, ErrInvalidCursor
	}
	return sequence, nil
}

// LookupProjectReceipt returns an earlier equal command before filesystem work begins.
func (s *Store) LookupProjectReceipt(ctx context.Context, command ProjectCommand) (ProjectResult, bool, error) {
	row := s.db.QueryRowContext(ctx, `
SELECT request_digest, response_json FROM command_receipts
WHERE actor_id = ? AND command_name = ? AND client_mutation_id = ?`,
		command.ActorID, command.Name, command.ClientMutationID)
	result, found, err := scanProjectReceipt(row, command.RequestDigest)
	return result, found, err
}

// CreateProject stores one active Project, event, and command receipt.
func (s *Store) CreateProject(
	ctx context.Context, id, workspaceID, name, description string, folder *string,
	documentDigest string, command ProjectCommand, now time.Time,
) (ProjectResult, error) {
	if !validProjectID(id) || workspaceID != "workspace:personal" || !validProjectDigest(documentDigest) {
		return ProjectResult{}, errors.New("invalid Project target")
	}
	name = strings.TrimSpace(name)
	description = strings.TrimSpace(description)
	if name == "" {
		return ProjectResult{}, errors.New("Project name cannot be empty")
	}
	folder, err := normalizeProjectFolder(folder)
	if err != nil {
		return ProjectResult{}, err
	}
	project := Project{ID: id, WorkspaceID: workspaceID, Name: name, Description: description,
		Folder: folder, Revision: 1, CreatedAt: now.UTC(), UpdatedAt: now.UTC()}
	return s.projectCommand(ctx, command, now, func(tx *sql.Tx) (ProjectResult, error) {
		if _, err := tx.ExecContext(ctx, `INSERT INTO projects
(project_id, workspace_id, name, description, folder, revision, created_at_ms, updated_at_ms)
VALUES (?, ?, ?, ?, ?, 1, ?, ?)`, project.ID, project.WorkspaceID, project.Name,
			project.Description, nullableString(project.Folder), millis(project.CreatedAt), millis(project.UpdatedAt)); err != nil {
			return ProjectResult{}, fmt.Errorf("insert Project: %w", err)
		}
		event, err := insertWorkEvent(ctx, tx, project.WorkspaceID, project.ID, "", "", 1,
			"project.created", command.ActorID, nil, projectCorrelation(command),
			map[string]any{"v": 1, "revision": int64(1)}, project.UpdatedAt)
		return ProjectResult{Project: project, Event: event, DocumentDigest: documentDigest}, err
	})
}

// UpdateProject changes active Project metadata or records one document change.
func (s *Store) UpdateProject(
	ctx context.Context, id string, expectedRevision int64, changes ProjectChanges,
	command ProjectCommand, now time.Time,
) (ProjectResult, error) {
	if !validProjectID(id) || expectedRevision <= 0 {
		return ProjectResult{}, errors.New("invalid Project update")
	}
	if changes.Name == nil && changes.Description == nil && !changes.SetFolder && !changes.DocumentChanged {
		return ProjectResult{}, errors.New("Project update has no replacement")
	}
	if (changes.SetFolder || changes.DocumentChanged) && !validProjectDigest(changes.DocumentDigest) {
		return ProjectResult{}, errors.New("invalid Project document digest")
	}
	if changes.Name != nil {
		value := strings.TrimSpace(*changes.Name)
		if value == "" {
			return ProjectResult{}, errors.New("Project name cannot be empty")
		}
		changes.Name = &value
	}
	if changes.Description != nil {
		value := strings.TrimSpace(*changes.Description)
		changes.Description = &value
	}
	if changes.SetFolder {
		folder, err := normalizeProjectFolder(changes.Folder)
		if err != nil {
			return ProjectResult{}, err
		}
		changes.Folder = folder
	}
	return s.projectCommand(ctx, command, now, func(tx *sql.Tx) (ProjectResult, error) {
		current, err := projectTx(ctx, tx, id)
		if err != nil {
			return ProjectResult{}, err
		}
		if current.Revision != expectedRevision {
			return ProjectResult{}, ErrStaleRevision
		}
		if current.ArchivedAt != nil {
			return ProjectResult{}, ErrInvalidTransition
		}
		changed := make([]string, 0, 4)
		if changes.Name != nil {
			current.Name = *changes.Name
			changed = append(changed, "name")
		}
		if changes.Description != nil {
			current.Description = *changes.Description
			changed = append(changed, "description")
		}
		if changes.SetFolder {
			current.Folder = cloneStoreString(changes.Folder)
			changed = append(changed, "folder")
		}
		if changes.DocumentChanged {
			changed = append(changed, "document")
		}
		current.Revision++
		current.UpdatedAt = now.UTC()
		result, err := tx.ExecContext(ctx, `UPDATE projects SET name = ?, description = ?, folder = ?,
revision = ?, updated_at_ms = ? WHERE project_id = ? AND revision = ?`, current.Name,
			current.Description, nullableString(current.Folder), current.Revision, millis(current.UpdatedAt), id, expectedRevision)
		if err != nil {
			return ProjectResult{}, fmt.Errorf("update Project: %w", err)
		}
		if count, err := result.RowsAffected(); err != nil || count != 1 {
			return ProjectResult{}, ErrStaleRevision
		}
		event, err := insertWorkEvent(ctx, tx, current.WorkspaceID, current.ID, "", "", current.Revision,
			"project.updated", command.ActorID, nil, projectCorrelation(command),
			map[string]any{"v": 1, "revision": current.Revision, "changed_fields": changed}, current.UpdatedAt)
		return ProjectResult{Project: current, Event: event, DocumentDigest: changes.DocumentDigest}, err
	})
}

// SetProjectArchived changes the active or archived lifecycle state.
func (s *Store) SetProjectArchived(
	ctx context.Context, id string, expectedRevision int64, archived bool,
	command ProjectCommand, now time.Time,
) (ProjectResult, error) {
	if !validProjectID(id) || expectedRevision <= 0 {
		return ProjectResult{}, errors.New("invalid Project lifecycle command")
	}
	return s.projectCommand(ctx, command, now, func(tx *sql.Tx) (ProjectResult, error) {
		current, err := projectTx(ctx, tx, id)
		if err != nil {
			return ProjectResult{}, err
		}
		if current.Revision != expectedRevision {
			return ProjectResult{}, ErrStaleRevision
		}
		if archived == (current.ArchivedAt != nil) {
			return ProjectResult{}, ErrInvalidTransition
		}
		current.Revision++
		current.UpdatedAt = now.UTC()
		kind := "project.reopened"
		if archived {
			value := current.UpdatedAt
			current.ArchivedAt = &value
			kind = "project.archived"
		} else {
			current.ArchivedAt = nil
		}
		changed, err := tx.ExecContext(ctx, `UPDATE projects SET archived_at_ms = ?, revision = ?, updated_at_ms = ?
WHERE project_id = ? AND revision = ?`, nullableTime(current.ArchivedAt), current.Revision,
			millis(current.UpdatedAt), id, expectedRevision)
		if err != nil {
			return ProjectResult{}, fmt.Errorf("change Project lifecycle: %w", err)
		}
		if count, err := changed.RowsAffected(); err != nil || count != 1 {
			return ProjectResult{}, ErrStaleRevision
		}
		event, err := insertWorkEvent(ctx, tx, current.WorkspaceID, current.ID, "", "", current.Revision,
			kind, command.ActorID, nil, projectCorrelation(command),
			map[string]any{"v": 1, "revision": current.Revision}, current.UpdatedAt)
		return ProjectResult{Project: current, Event: event}, err
	})
}

// Project returns one Project by identity.
func (s *Store) Project(ctx context.Context, id string) (Project, error) {
	if !validProjectID(id) {
		return Project{}, ErrProjectNotFound
	}
	return scanProject(s.db.QueryRowContext(ctx, projectSelect+" WHERE project_id = ?", id))
}

// ListProjects returns one stable Project page.
func (s *Store) ListProjects(
	ctx context.Context, workspaceID string, includeArchived bool, first int, after *string,
) (ProjectPage, error) {
	if workspaceID != "workspace:personal" || first < 1 || first > 100 {
		return ProjectPage{}, errors.New("invalid Project list")
	}
	hash := projectQueryDigest(workspaceID, includeArchived)
	var afterTime int64
	var afterID string
	if after != nil {
		var err error
		afterTime, afterID, err = decodeProjectCursor(*after, hash)
		if err != nil {
			return ProjectPage{}, err
		}
	}
	rows, err := s.db.QueryContext(ctx, projectSelect+`
 WHERE workspace_id = ? AND (? OR archived_at_ms IS NULL)
 AND (? = 0 OR updated_at_ms < ? OR (updated_at_ms = ? AND project_id < ?))
 ORDER BY updated_at_ms DESC, project_id DESC LIMIT ?`, workspaceID, includeArchived,
		afterTime, afterTime, afterTime, afterID, first+1)
	if err != nil {
		return ProjectPage{}, fmt.Errorf("list Projects: %w", err)
	}
	defer rows.Close()
	projects := make([]Project, 0, first+1)
	for rows.Next() {
		project, err := scanProject(rows)
		if err != nil {
			return ProjectPage{}, err
		}
		projects = append(projects, project)
	}
	if err := rows.Err(); err != nil {
		return ProjectPage{}, err
	}
	page := ProjectPage{HasNextPage: len(projects) > first}
	if page.HasNextPage {
		projects = projects[:first]
	}
	page.Projects = projects
	page.Cursors = make([]string, len(projects))
	for index, project := range projects {
		page.Cursors[index] = encodeProjectCursor(hash, millis(project.UpdatedAt), project.ID)
	}
	if len(projects) > 0 {
		cursor := encodeProjectCursor(hash, millis(projects[len(projects)-1].UpdatedAt), projects[len(projects)-1].ID)
		page.EndCursor = &cursor
	}
	return page, nil
}

// WorkEvents returns shared workspace events after one global cursor.
func (s *Store) WorkEvents(ctx context.Context, workspaceID string, after int64, limit int) ([]WorkEvent, error) {
	if workspaceID != "workspace:personal" || after < 0 || limit < 1 || limit > 100 {
		return nil, errors.New("invalid work event query")
	}
	rows, err := s.db.QueryContext(ctx, `SELECT event_id, event_key, workspace_id, COALESCE(project_id,''),
COALESCE(task_id,''), COALESCE(run_id,''), subject_revision, kind, actor_id, causation_id, correlation_id,
payload_json, occurred_at_ms FROM work_events
WHERE workspace_id = ? AND event_id > ? ORDER BY event_id LIMIT ?`, workspaceID, after, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	events := make([]WorkEvent, 0)
	for rows.Next() {
		event, err := scanWorkEvent(rows)
		if err != nil {
			return nil, err
		}
		events = append(events, event)
	}
	return events, rows.Err()
}

// WorkEventsForTask returns shared events for one Task after a global cursor.
func (s *Store) WorkEventsForTask(ctx context.Context, taskID string, after int64, limit int) ([]WorkEvent, error) {
	if !validTaskID(taskID) || after < 0 || limit < 1 || limit > 100 {
		return nil, errors.New("invalid Task work event query")
	}
	rows, err := s.db.QueryContext(ctx, `SELECT event_id, event_key, workspace_id, COALESCE(project_id,''),
COALESCE(task_id,''), COALESCE(run_id,''), subject_revision, kind, actor_id, causation_id, correlation_id,
payload_json, occurred_at_ms FROM work_events
WHERE task_id = ? AND event_id > ? ORDER BY event_id LIMIT ?`, taskID, after, limit)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	events := make([]WorkEvent, 0)
	for rows.Next() {
		event, err := scanWorkEvent(rows)
		if err != nil {
			return nil, err
		}
		events = append(events, event)
	}
	return events, rows.Err()
}

// LatestTaskWorkEvent returns the newest shared event for one Task.
func (s *Store) LatestTaskWorkEvent(ctx context.Context, taskID string) (WorkEvent, error) {
	return scanWorkEvent(s.db.QueryRowContext(ctx, `SELECT event_id, event_key, workspace_id,
COALESCE(project_id,''), COALESCE(task_id,''), COALESCE(run_id,''), subject_revision, kind, actor_id,
causation_id, correlation_id, payload_json, occurred_at_ms
FROM work_events WHERE task_id = ? ORDER BY event_id DESC LIMIT 1`, taskID))
}

// LatestWorkEventSequence returns a workspace's durable high-water sequence.
func (s *Store) LatestWorkEventSequence(ctx context.Context, workspaceID string) (int64, error) {
	if workspaceID != "workspace:personal" {
		return 0, errors.New("invalid work event query")
	}
	var sequence sql.NullInt64
	err := s.db.QueryRowContext(ctx,
		`SELECT MAX(event_id) FROM work_events WHERE workspace_id = ?`, workspaceID).Scan(&sequence)
	return sequence.Int64, err
}

// LatestTaskWorkEventSequence returns a Task's durable high-water sequence.
func (s *Store) LatestTaskWorkEventSequence(ctx context.Context, taskID string) (int64, error) {
	if !validTaskID(taskID) {
		return 0, errors.New("invalid Task work event query")
	}
	var sequence sql.NullInt64
	err := s.db.QueryRowContext(ctx,
		`SELECT MAX(event_id) FROM work_events WHERE task_id = ?`, taskID).Scan(&sequence)
	return sequence.Int64, err
}

// SubscribeWork returns durable-ledger wakeups until the context ends.
func (s *Store) SubscribeWork(ctx context.Context) <-chan struct{} {
	wake := make(chan struct{}, 1)
	s.workMu.Lock()
	s.workSubscribers[wake] = struct{}{}
	s.workMu.Unlock()
	go func() {
		<-ctx.Done()
		s.workMu.Lock()
		delete(s.workSubscribers, wake)
		close(wake)
		s.workMu.Unlock()
	}()
	return wake
}

// NotifyWork wakes subscribers after a complete durable projection commits.
func (s *Store) NotifyWork() {
	s.workMu.Lock()
	defer s.workMu.Unlock()
	for wake := range s.workSubscribers {
		select {
		case wake <- struct{}{}:
		default:
		}
	}
}

// ProjectReceiptResult returns the committed result for one staged Project request.
func (s *Store) ProjectReceiptResult(ctx context.Context, projectID, requestDigest string) (ProjectResult, bool, error) {
	row := s.db.QueryRowContext(ctx, `SELECT request_digest, response_json FROM command_receipts
WHERE result_project_id = ? AND request_digest = ? ORDER BY result_event_id DESC LIMIT 1`,
		projectID, requestDigest)
	return scanProjectReceipt(row, requestDigest)
}

func (s *Store) projectCommand(
	ctx context.Context, command ProjectCommand, now time.Time,
	change func(*sql.Tx) (ProjectResult, error),
) (ProjectResult, error) {
	if err := validateProjectCommand(command); err != nil {
		return ProjectResult{}, err
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ProjectResult{}, err
	}
	defer func() { _ = tx.Rollback() }()
	if replay, found, err := scanProjectReceipt(tx.QueryRowContext(ctx, `SELECT request_digest, response_json
FROM command_receipts WHERE actor_id = ? AND command_name = ? AND client_mutation_id = ?`,
		command.ActorID, command.Name, command.ClientMutationID), command.RequestDigest); err != nil {
		return ProjectResult{}, err
	} else if found {
		replay.Replayed = true
		return replay, nil
	}
	result, err := change(tx)
	if err != nil {
		return ProjectResult{}, err
	}
	response, err := json.Marshal(result)
	if err != nil {
		return ProjectResult{}, err
	}
	if _, err := tx.ExecContext(ctx, `INSERT INTO command_receipts
(actor_id, command_name, client_mutation_id, request_digest, result_project_id, result_task_id, result_event_id, response_json, created_at_ms)
VALUES (?, ?, ?, ?, ?, NULL, ?, ?, ?)`, command.ActorID, command.Name, command.ClientMutationID,
		command.RequestDigest, result.Project.ID, result.Event.ID, string(response), millis(now.UTC())); err != nil {
		return ProjectResult{}, fmt.Errorf("store command receipt: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return ProjectResult{}, err
	}
	return result, nil
}

func insertWorkEvent(ctx context.Context, tx *sql.Tx, workspaceID, projectID, taskID, runID string,
	revision int64, kind, actorID string, causationID *string, correlationID string,
	payload map[string]any, now time.Time) (WorkEvent, error) {
	if workspaceID != "workspace:personal" || revision <= 0 || strings.TrimSpace(kind) == "" ||
		!strings.HasPrefix(actorID, "actor:") || !strings.HasPrefix(correlationID, "correlation:") ||
		(projectID != "" && !validProjectID(projectID)) || (taskID != "" && !validTaskID(taskID)) ||
		(runID != "" && !strings.HasPrefix(runID, "run:")) {
		return WorkEvent{}, errors.New("invalid work event")
	}
	if causationID != nil && !hasAnyPrefix(*causationID, "event:", "command:", "run:") {
		return WorkEvent{}, errors.New("invalid work event causation")
	}
	encoded, err := json.Marshal(payload)
	if err != nil {
		return WorkEvent{}, err
	}
	eventID, err := newID("event")
	if err != nil {
		return WorkEvent{}, err
	}
	result, err := tx.ExecContext(ctx, `INSERT INTO work_events
(event_key, workspace_id, project_id, task_id, run_id, subject_revision, kind, actor_id, causation_id,
 correlation_id, payload_json, occurred_at_ms)
VALUES (?, ?, NULLIF(?,''), NULLIF(?,''), NULLIF(?,''), ?, ?, ?, ?, ?, ?, ?)`, eventID, workspaceID, projectID, taskID, runID,
		revision, kind, actorID, nullableString(causationID), correlationID, string(encoded), millis(now.UTC()))
	if err != nil {
		return WorkEvent{}, fmt.Errorf("insert work event: %w", err)
	}
	id, err := result.LastInsertId()
	return WorkEvent{ID: id, EventID: eventID, WorkspaceID: workspaceID, ProjectID: projectID, TaskID: taskID, RunID: runID,
		SubjectRevision: revision, Kind: kind, ActorID: actorID, CausationID: cloneStoreString(causationID),
		CorrelationID: correlationID, Payload: payload, OccurredAt: now.UTC()}, err
}

func hasAnyPrefix(value string, prefixes ...string) bool {
	for _, prefix := range prefixes {
		if strings.HasPrefix(value, prefix) {
			return true
		}
	}
	return false
}

func scanProjectReceipt(row rowScanner, digest string) (ProjectResult, bool, error) {
	var stored, response string
	if err := row.Scan(&stored, &response); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return ProjectResult{}, false, nil
		}
		return ProjectResult{}, false, err
	}
	if stored != digest {
		return ProjectResult{}, true, ErrCommandConflict
	}
	var result ProjectResult
	if err := json.Unmarshal([]byte(response), &result); err != nil {
		return ProjectResult{}, true, fmt.Errorf("decode command receipt: %w", err)
	}
	result.Replayed = true
	return result, true, nil
}

const projectSelect = `SELECT project_id, workspace_id, name, description, folder, revision,
archived_at_ms, created_at_ms, updated_at_ms FROM projects`

func projectTx(ctx context.Context, tx *sql.Tx, id string) (Project, error) {
	return scanProject(tx.QueryRowContext(ctx, projectSelect+" WHERE project_id = ?", id))
}

func scanProject(row rowScanner) (Project, error) {
	var project Project
	var folder sql.NullString
	var archived sql.NullInt64
	var created, updated int64
	if err := row.Scan(&project.ID, &project.WorkspaceID, &project.Name, &project.Description,
		&folder, &project.Revision, &archived, &created, &updated); err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return Project{}, ErrProjectNotFound
		}
		return Project{}, err
	}
	if folder.Valid {
		project.Folder = &folder.String
	}
	if archived.Valid {
		value := fromMillis(archived.Int64)
		project.ArchivedAt = &value
	}
	project.CreatedAt, project.UpdatedAt = fromMillis(created), fromMillis(updated)
	return project, nil
}

func scanWorkEvent(row rowScanner) (WorkEvent, error) {
	var event WorkEvent
	var causation sql.NullString
	var payload string
	var occurred int64
	if err := row.Scan(&event.ID, &event.EventID, &event.WorkspaceID, &event.ProjectID, &event.TaskID, &event.RunID,
		&event.SubjectRevision, &event.Kind, &event.ActorID, &causation, &event.CorrelationID,
		&payload, &occurred); err != nil {
		return WorkEvent{}, err
	}
	if err := json.Unmarshal([]byte(payload), &event.Payload); err != nil {
		return WorkEvent{}, err
	}
	event.OccurredAt = fromMillis(occurred)
	if causation.Valid {
		event.CausationID = &causation.String
	}
	return event, nil
}

func projectCorrelation(command ProjectCommand) string {
	return "correlation:graphql:" + command.ClientMutationID
}

func validateProjectCommand(command ProjectCommand) error {
	if strings.TrimSpace(command.ActorID) == "" || strings.TrimSpace(command.Name) == "" ||
		command.ClientMutationID == "" || strings.TrimSpace(command.ClientMutationID) != command.ClientMutationID ||
		len(command.RequestDigest) != 64 {
		return errors.New("invalid Project command")
	}
	decoded, err := hex.DecodeString(command.RequestDigest)
	if err != nil || hex.EncodeToString(decoded) != command.RequestDigest {
		return errors.New("invalid Project command digest")
	}
	return nil
}

func normalizeProjectFolder(folder *string) (*string, error) {
	if folder == nil {
		return nil, nil
	}
	value := strings.TrimSpace(*folder)
	if value == "" || strings.ContainsRune(value, 0) || !filepath.IsAbs(value) {
		return nil, errors.New("Project folder must be one absolute path")
	}
	return &value, nil
}

func validProjectID(id string) bool {
	if !strings.HasPrefix(id, "project:") || len(id) != 40 {
		return false
	}
	decoded, err := hex.DecodeString(strings.TrimPrefix(id, "project:"))
	return err == nil && hex.EncodeToString(decoded) == strings.TrimPrefix(id, "project:")
}

func validProjectDigest(value string) bool {
	if len(value) != 64 {
		return false
	}
	decoded, err := hex.DecodeString(value)
	return err == nil && hex.EncodeToString(decoded) == value
}

func projectQueryDigest(workspace string, archived bool) string {
	value := sha256.Sum256([]byte("project-query:v1\x00" + workspace + "\x00" + strconv.FormatBool(archived)))
	return hex.EncodeToString(value[:])
}

func encodeProjectCursor(hash string, updated int64, id string) string {
	return base64.RawURLEncoding.EncodeToString([]byte(hash + "\x00" + strconv.FormatInt(updated, 10) + "\x00" + id))
}

func decodeProjectCursor(cursor, expectedHash string) (int64, string, error) {
	decoded, err := base64.RawURLEncoding.DecodeString(cursor)
	if err != nil {
		return 0, "", ErrInvalidCursor
	}
	parts := strings.Split(string(decoded), "\x00")
	if len(parts) != 3 || parts[0] != expectedHash || !validProjectID(parts[2]) {
		return 0, "", ErrInvalidCursor
	}
	updated, err := strconv.ParseInt(parts[1], 10, 64)
	if err != nil || updated <= 0 {
		return 0, "", ErrInvalidCursor
	}
	return updated, parts[2], nil
}

func nullableString(value *string) any {
	if value == nil {
		return nil
	}
	return *value
}

func nullableTime(value *time.Time) any {
	if value == nil {
		return nil
	}
	return millis(*value)
}

func cloneStoreString(value *string) *string {
	if value == nil {
		return nil
	}
	copy := *value
	return &copy
}
