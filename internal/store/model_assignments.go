package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"strings"

	"github.com/kpsuperplane/noema/internal/provider"

	"github.com/uptrace/bun"
)

var (
	// ErrInvalidModelAssignments means a complete hosted assignment set failed validation.
	ErrInvalidModelAssignments = errors.New("invalid hosted model assignments")
	// ErrModelAccountNotReady means the selected account cannot serve hosted model requests.
	ErrModelAccountNotReady = errors.New("hosted model account is not ready")
	// ErrTaskModelPoolEntryNotFound means one global pool setting is absent.
	ErrTaskModelPoolEntryNotFound = errors.New("Task model pool entry not found")
)

// HostedModelRole identifies one required hosted model assignment.
type HostedModelRole string

const (
	HostedModelNoema               HostedModelRole = "noema"
	HostedModelSimpleTasks         HostedModelRole = "simple_tasks"
	HostedModelMediumTasks         HostedModelRole = "medium_tasks"
	HostedModelDifficultTasks      HostedModelRole = "difficult_tasks"
	HostedModelTaskReviewer        HostedModelRole = "task_reviewer"
	HostedModelWebFetchSummarizer  HostedModelRole = "web_fetch_summarizer"
	HostedModelToolProgressAudit   HostedModelRole = "tool_progress_audit"
	HostedModelActionReviewer      HostedModelRole = "action_reviewer"
	HostedModelMemoryConsolidation HostedModelRole = "memory_consolidation"
)

var hostedModelRoles = []HostedModelRole{
	HostedModelNoema,
	HostedModelSimpleTasks,
	HostedModelMediumTasks,
	HostedModelDifficultTasks,
	HostedModelTaskReviewer,
	HostedModelWebFetchSummarizer,
	HostedModelToolProgressAudit,
	HostedModelActionReviewer,
	HostedModelMemoryConsolidation,
}

// ModelSelectionMode states who owns the concrete model choice.
type ModelSelectionMode string

const (
	ModelSelectionNoemaRecommended ModelSelectionMode = "noema_recommended"
	ModelSelectionExplicitProfile  ModelSelectionMode = "explicit_profile"
)

// ModelReasoningEffort is one provider-supported reasoning level.
type ModelReasoningEffort string

const (
	ModelReasoningNone    ModelReasoningEffort = "none"
	ModelReasoningMinimal ModelReasoningEffort = "minimal"
	ModelReasoningLow     ModelReasoningEffort = "low"
	ModelReasoningMedium  ModelReasoningEffort = "medium"
	ModelReasoningHigh    ModelReasoningEffort = "high"
	ModelReasoningXHigh   ModelReasoningEffort = "xhigh"
)

// ModelAssignment is one durable hosted onboarding model preference.
type ModelAssignment struct {
	Role              HostedModelRole
	ProviderKind      string
	ProviderAccountID string
	SelectionMode     ModelSelectionMode
	ModelProfile      string
	ReasoningEffort   ModelReasoningEffort
	FastMode          bool
}

// HostedModelRoles returns all required roles in stable product order.
func HostedModelRoles() []HostedModelRole {
	return append([]HostedModelRole(nil), hostedModelRoles...)
}

// ConfirmHostedModelAssignments fills missing roles without replacing current selections.
// A successful concurrent caller wins. Later complete calls return false.
func (s *Store) ConfirmHostedModelAssignments(
	ctx context.Context,
	providerAccountID string,
	assignments []ModelAssignment,
) (bool, error) {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return false, fmt.Errorf("begin hosted model assignment confirmation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()

	account, err := scanProviderAccount(tx.QueryRowContext(ctx, providerAccountSelect+`
WHERE provider_account_id = ?`, providerAccountID))
	if err != nil {
		if errors.Is(err, provider.ErrAccountNotFound) {
			return false, fmt.Errorf("%w: account does not exist", ErrModelAccountNotReady)
		}
		return false, err
	}
	if !isHostedModelProvider(account.ProviderKind) || !account.IsActive ||
		account.Status != provider.StatusAuthenticated {
		return false, fmt.Errorf("%w: account is not active and authenticated", ErrModelAccountNotReady)
	}
	if err := validateAssignmentSet(providerAccountID, account.ProviderKind, assignments); err != nil {
		return false, err
	}
	var existing, reviewers, hosted int
	if err := tx.QueryRowContext(ctx, `SELECT COUNT(*),
    COUNT(CASE WHEN role='action_reviewer' THEN 1 END),
    COUNT(CASE WHEN provider_kind<>'local_models' THEN 1 END)
FROM hosted_model_assignments`).Scan(&existing, &reviewers, &hosted); err != nil {
		return false, fmt.Errorf("count hosted model assignments: %w", err)
	}
	if account.ProviderKind == "local_models" {
		if reviewers == 0 && hosted > 0 {
			return false, fmt.Errorf("%w: select a hosted provider to restore the missing action reviewer", ErrInvalidModelAssignments)
		}
		existing -= reviewers
	}
	if existing == len(assignments) {
		return false, nil
	}
	for _, assignment := range assignments {
		if assignment.ProviderKind != account.ProviderKind ||
			assignment.ProviderAccountID != account.ID {
			return false, fmt.Errorf("%w: assignments must use the selected account", ErrInvalidModelAssignments)
		}
		if account.ProviderKind == "local_models" {
			if _, err := modelAssignmentAccount(ctx, tx, assignment); err != nil {
				return false, err
			}
		} else if err := validateAssignmentProfile(account.Metadata, assignment); err != nil {
			return false, err
		}
		if _, err := tx.ExecContext(ctx, `
INSERT INTO hosted_model_assignments (
    role, provider_kind, provider_account_id, selection_mode,
    model_profile, reasoning_effort, fast_mode
) VALUES (?, ?, ?, ?, NULLIF(?, ''), NULLIF(?, ''), ?)
ON CONFLICT(role) DO NOTHING`,
			assignment.Role, assignment.ProviderKind, assignment.ProviderAccountID,
			assignment.SelectionMode, assignment.ModelProfile, assignment.ReasoningEffort,
			assignment.FastMode,
		); err != nil {
			return false, fmt.Errorf("store hosted model assignment %s: %w", assignment.Role, err)
		}
	}
	if err := tx.Commit(); err != nil {
		return false, fmt.Errorf("commit hosted model assignments: %w", err)
	}
	return true, nil
}

// SaveHostedModelAssignment replaces one existing hosted model role.
func (s *Store) SaveHostedModelAssignment(
	ctx context.Context,
	assignment ModelAssignment,
) (ModelAssignment, error) {
	if err := validateSingleAssignment(assignment); err != nil {
		return ModelAssignment{}, err
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return ModelAssignment{}, fmt.Errorf("begin hosted model assignment update: %w", err)
	}
	defer func() { _ = tx.Rollback() }()
	if _, err := modelAssignmentAccount(ctx, tx, assignment); err != nil {
		return ModelAssignment{}, err
	}
	if err := saveHostedModelAssignmentTx(ctx, tx, assignment); err != nil {
		return ModelAssignment{}, err
	}
	if err := tx.Commit(); err != nil {
		return ModelAssignment{}, fmt.Errorf("commit hosted model assignment update: %w", err)
	}
	return assignment, nil
}

// HostedModelAssignments returns the complete hosted assignment set.
func (s *Store) HostedModelAssignments(ctx context.Context) ([]ModelAssignment, error) {
	rows, err := s.db.QueryContext(ctx, `
SELECT role, provider_kind, provider_account_id, selection_mode,
       model_profile, reasoning_effort, fast_mode
FROM hosted_model_assignments`)
	if err != nil {
		return nil, fmt.Errorf("query hosted model assignments: %w", err)
	}
	defer rows.Close()

	byRole := make(map[HostedModelRole]ModelAssignment, len(hostedModelRoles))
	for rows.Next() {
		var assignment ModelAssignment
		var profile, effort sql.NullString
		var fastMode int
		if err := rows.Scan(
			&assignment.Role, &assignment.ProviderKind, &assignment.ProviderAccountID,
			&assignment.SelectionMode, &profile, &effort, &fastMode,
		); err != nil {
			return nil, fmt.Errorf("scan hosted model assignment: %w", err)
		}
		assignment.ModelProfile = profile.String
		assignment.ReasoningEffort = ModelReasoningEffort(effort.String)
		assignment.FastMode = fastMode == 1
		byRole[assignment.Role] = assignment
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("read hosted model assignments: %w", err)
	}
	if len(byRole) == 0 {
		return []ModelAssignment{}, nil
	}
	localSet := len(byRole) == len(hostedModelRoles)-1
	if localSet {
		for role, assignment := range byRole {
			localSet = localSet && role != HostedModelActionReviewer && assignment.ProviderKind == "local_models"
		}
	}
	if len(byRole) != len(hostedModelRoles) && !localSet {
		return nil, fmt.Errorf("%w: stored assignments are incomplete", ErrInvalidModelAssignments)
	}
	assignments := make([]ModelAssignment, 0, len(hostedModelRoles))
	for _, role := range hostedModelRoles {
		if localSet && role == HostedModelActionReviewer {
			continue
		}
		assignment, ok := byRole[role]
		if !ok {
			return nil, fmt.Errorf("%w: stored role %s is missing", ErrInvalidModelAssignments, role)
		}
		assignments = append(assignments, assignment)
	}
	return assignments, nil
}

func validateAssignmentSet(providerAccountID, providerKind string, assignments []ModelAssignment) error {
	expectedCount := len(hostedModelRoles)
	if providerKind == "local_models" {
		expectedCount--
	}
	if providerAccountID == "" || len(assignments) != expectedCount {
		return fmt.Errorf("%w: model roles are incomplete", ErrInvalidModelAssignments)
	}
	expected := make(map[HostedModelRole]bool, len(hostedModelRoles))
	for _, role := range hostedModelRoles {
		if providerKind == "local_models" && role == HostedModelActionReviewer {
			continue
		}
		expected[role] = true
	}
	seen := make(map[HostedModelRole]bool, len(hostedModelRoles))
	for _, assignment := range assignments {
		if !expected[assignment.Role] || seen[assignment.Role] {
			return fmt.Errorf("%w: roles are incomplete or duplicated", ErrInvalidModelAssignments)
		}
		seen[assignment.Role] = true
		if assignment.ProviderAccountID != providerAccountID {
			return fmt.Errorf("%w: assignments must use one provider account", ErrInvalidModelAssignments)
		}
		if err := validateSingleAssignment(assignment); err != nil {
			return err
		}
	}
	return nil
}

func validateSingleAssignment(assignment ModelAssignment) error {
	knownRole := false
	for _, role := range hostedModelRoles {
		knownRole = knownRole || assignment.Role == role
	}
	if !knownRole || assignment.ProviderAccountID == "" || !isHostedModelProvider(assignment.ProviderKind) {
		return fmt.Errorf("%w: model role or provider is unsupported", ErrInvalidModelAssignments)
	}
	switch assignment.SelectionMode {
	case ModelSelectionNoemaRecommended:
		if assignment.ModelProfile != "" || assignment.ReasoningEffort != "" {
			return fmt.Errorf("%w: recommended selections cannot set a profile or effort", ErrInvalidModelAssignments)
		}
	case ModelSelectionExplicitProfile:
		if assignment.ModelProfile == "" || strings.TrimSpace(assignment.ModelProfile) != assignment.ModelProfile {
			return fmt.Errorf("%w: explicit selections require an exact profile", ErrInvalidModelAssignments)
		}
		if assignment.ReasoningEffort != "" && !validReasoningEffort(assignment.ReasoningEffort) {
			return fmt.Errorf("%w: reasoning effort is unsupported", ErrInvalidModelAssignments)
		}
	default:
		return fmt.Errorf("%w: selection mode is unsupported", ErrInvalidModelAssignments)
	}
	return nil
}

func modelAssignmentAccount(
	ctx context.Context,
	tx bun.Tx,
	assignment ModelAssignment,
) (provider.Account, error) {
	account, err := scanProviderAccount(tx.QueryRowContext(ctx, providerAccountSelect+`
WHERE provider_account_id = ?`, assignment.ProviderAccountID))
	if err != nil {
		if errors.Is(err, provider.ErrAccountNotFound) {
			return provider.Account{}, fmt.Errorf("%w: account does not exist", ErrModelAccountNotReady)
		}
		return provider.Account{}, err
	}
	if account.ProviderKind != assignment.ProviderKind || !account.IsActive || !account.IsDefault ||
		account.Status != provider.StatusAuthenticated {
		return provider.Account{}, fmt.Errorf("%w: account is not ready or does not match", ErrModelAccountNotReady)
	}
	if account.ProviderKind == "local_models" {
		if assignment.SelectionMode != ModelSelectionExplicitProfile || assignment.ReasoningEffort != "" {
			return provider.Account{}, fmt.Errorf("%w: local model selection is invalid", ErrInvalidModelAssignments)
		}
		var ready int
		if err := tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM local_model_installations WHERE model_id=? AND status='installed' AND is_active=1)`, assignment.ModelProfile).Scan(&ready); err != nil || ready != 1 {
			return provider.Account{}, fmt.Errorf("%w: local model is not active", ErrModelAccountNotReady)
		}
	} else if err := validateAssignmentProfile(account.Metadata, assignment); err != nil {
		return provider.Account{}, err
	}
	return account, nil
}

func saveHostedModelAssignmentTx(
	ctx context.Context,
	tx bun.Tx,
	assignment ModelAssignment,
) error {
	result, err := tx.ExecContext(ctx, `
UPDATE hosted_model_assignments SET
    provider_kind = ?, provider_account_id = ?, selection_mode = ?,
    model_profile = NULLIF(?, ''), reasoning_effort = NULLIF(?, ''), fast_mode = ?
WHERE role = ?`, assignment.ProviderKind, assignment.ProviderAccountID,
		assignment.SelectionMode, assignment.ModelProfile, assignment.ReasoningEffort,
		assignment.FastMode, assignment.Role)
	if err != nil {
		return fmt.Errorf("save hosted model assignment %s: %w", assignment.Role, err)
	}
	changed, _ := result.RowsAffected()
	if changed != 1 {
		return fmt.Errorf("%w: stored role %s is missing", ErrInvalidModelAssignments, assignment.Role)
	}
	return nil
}

func validateAssignmentProfile(metadata provider.AccountMetadata, assignment ModelAssignment) error {
	if assignment.SelectionMode == ModelSelectionNoemaRecommended {
		return nil
	}
	var profiles []provider.ModelProfile
	if profilesJSON := metadata["profiles"]; len(profilesJSON) > 0 {
		if err := json.Unmarshal(profilesJSON, &profiles); err != nil {
			return fmt.Errorf("%w: account profiles are invalid", ErrModelAccountNotReady)
		}
	}
	for _, profile := range profiles {
		if profile.ID != assignment.ModelProfile {
			continue
		}
		if len(profile.ReasoningEfforts) == 0 {
			if assignment.ReasoningEffort != "" {
				return fmt.Errorf("%w: profile does not support reasoning effort", ErrInvalidModelAssignments)
			}
			return nil
		}
		if assignment.ReasoningEffort == "" {
			return fmt.Errorf("%w: profile requires reasoning effort", ErrInvalidModelAssignments)
		}
		for _, effort := range profile.ReasoningEfforts {
			if effort == string(assignment.ReasoningEffort) {
				return nil
			}
		}
		return fmt.Errorf("%w: profile does not support reasoning effort", ErrInvalidModelAssignments)
	}
	return fmt.Errorf("%w: model profile is unavailable", ErrInvalidModelAssignments)
}

func validReasoningEffort(effort ModelReasoningEffort) bool {
	switch effort {
	case ModelReasoningNone, ModelReasoningMinimal, ModelReasoningLow,
		ModelReasoningMedium, ModelReasoningHigh, ModelReasoningXHigh:
		return true
	default:
		return false
	}
}

func isHostedModelProvider(kind string) bool {
	return kind == "codex" || kind == "openai" || kind == "openrouter" ||
		kind == "local_models"
}
