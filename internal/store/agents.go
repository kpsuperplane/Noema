package store

import (
	"context"
	"database/sql"
	"errors"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/uptrace/bun"
)

const (
	PrimaryAgentID      = "agent:primary"
	TaskExecutorAgentID = "agent:task-executor"
	TaskReviewerAgentID = "agent:task-reviewer"
)

var (
	ErrAgentNotFound           = errors.New("Agent not found")
	ErrInvalidAgentDisplayName = errors.New("invalid Agent display name")
)

// Agent is one durable Agent identity.
type Agent struct {
	bun.BaseModel `bun:"table:agents"`
	ID            string `bun:"agent_id,pk"`
	DisplayName   *string
	SystemRole    *string
}

// Agents returns the supported built-in Agent identities.
func (s *Store) Agents(ctx context.Context) ([]Agent, error) {
	agents := make([]Agent, 0)
	err := s.db.NewSelect().Model(&agents).Where("system_role IS NOT NULL").OrderExpr(`CASE system_role
 WHEN 'primary' THEN 0 WHEN 'task_executor' THEN 1 WHEN 'task_reviewer' THEN 2 ELSE 3 END,
 display_name IS NULL, display_name, agent_id`).Scan(ctx)
	return agents, err
}

// Agent returns one Agent identity.
func (s *Store) Agent(ctx context.Context, id string) (Agent, error) {
	var agent Agent
	err := s.db.NewSelect().Model(&agent).Where("agent_id = ?", id).Scan(ctx)
	if errors.Is(err, sql.ErrNoRows) {
		return Agent{}, ErrAgentNotFound
	}
	return agent, err
}

// UpdatePrimaryAgentDisplayName changes the primary Agent's visible name.
func (s *Store) UpdatePrimaryAgentDisplayName(
	ctx context.Context,
	displayName string,
	now time.Time,
) (Agent, error) {
	displayName = strings.TrimSpace(displayName)
	if displayName == "" || !utf8.ValidString(displayName) || utf8.RuneCountInString(displayName) > 128 {
		return Agent{}, ErrInvalidAgentDisplayName
	}
	var agent Agent
	err := s.db.NewUpdate().Model(&agent).Set("display_name = ?", displayName).
		Set("updated_at_ms = ?", millis(now.UTC())).Where("agent_id = ? AND system_role = 'primary'", PrimaryAgentID).
		Returning("agent_id, display_name, system_role").Scan(ctx)
	if errors.Is(err, sql.ErrNoRows) {
		return Agent{}, ErrAgentNotFound
	}
	return agent, err
}
