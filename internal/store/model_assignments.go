package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"strings"

	"github.com/kpsuperplane/noema/internal/provider"
)

var (
	// ErrInvalidModelAssignments means a complete hosted assignment set failed validation.
	ErrInvalidModelAssignments = errors.New("invalid hosted model assignments")
	// ErrModelAccountNotReady means the selected account cannot serve hosted model requests.
	ErrModelAccountNotReady = errors.New("hosted model account is not ready")
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

// ConfirmHostedModelAssignments stores one complete assignment set.
// A successful concurrent caller wins. Later complete calls return false.
func (s *Store) ConfirmHostedModelAssignments(
	ctx context.Context,
	providerAccountID string,
	assignments []ModelAssignment,
) (bool, error) {
	if err := validateAssignmentSet(providerAccountID, assignments); err != nil {
		return false, err
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return false, fmt.Errorf("begin hosted model assignment confirmation: %w", err)
	}
	defer func() { _ = tx.Rollback() }()

	var existing int
	if err := tx.QueryRowContext(ctx, "SELECT COUNT(*) FROM hosted_model_assignments").Scan(&existing); err != nil {
		return false, fmt.Errorf("count hosted model assignments: %w", err)
	}
	if existing == len(hostedModelRoles) {
		return false, nil
	}
	if existing != 0 {
		return false, fmt.Errorf("%w: stored assignments are incomplete", ErrInvalidModelAssignments)
	}

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
	for _, assignment := range assignments {
		if assignment.ProviderKind != account.ProviderKind ||
			assignment.ProviderAccountID != account.ID {
			return false, fmt.Errorf("%w: assignments must use the selected account", ErrInvalidModelAssignments)
		}
		if err := validateAssignmentProfile(account.Metadata, assignment); err != nil {
			return false, err
		}
		if _, err := tx.ExecContext(ctx, `
INSERT INTO hosted_model_assignments (
    role, provider_kind, provider_account_id, selection_mode,
    model_profile, reasoning_effort, fast_mode
) VALUES (?, ?, ?, ?, NULLIF(?, ''), NULLIF(?, ''), ?)`,
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
	if len(byRole) != len(hostedModelRoles) {
		return nil, fmt.Errorf("%w: stored assignments are incomplete", ErrInvalidModelAssignments)
	}
	assignments := make([]ModelAssignment, 0, len(hostedModelRoles))
	for _, role := range hostedModelRoles {
		assignment, ok := byRole[role]
		if !ok {
			return nil, fmt.Errorf("%w: stored role %s is missing", ErrInvalidModelAssignments, role)
		}
		assignments = append(assignments, assignment)
	}
	return assignments, nil
}

func validateAssignmentSet(providerAccountID string, assignments []ModelAssignment) error {
	if providerAccountID == "" || len(assignments) != len(hostedModelRoles) {
		return fmt.Errorf("%w: exactly nine roles are required", ErrInvalidModelAssignments)
	}
	expected := make(map[HostedModelRole]bool, len(hostedModelRoles))
	for _, role := range hostedModelRoles {
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
		if assignment.ReasoningEffort == "" {
			return nil
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
	return kind == "codex" || kind == "openai" || kind == "openrouter"
}
