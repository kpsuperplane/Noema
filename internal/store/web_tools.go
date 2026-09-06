package store

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
)

// WebProviderBinding is one selected account for a model-visible web tool.
type WebProviderBinding struct {
	ToolName, CapabilityID, ProviderAccountID string
	RoutePosition                             int
}

// WebProviderRoute returns configured accounts in route order.
func (s *Store) WebProviderRoute(ctx context.Context, toolName string) ([]WebProviderBinding, error) {
	rows, err := s.db.QueryContext(ctx, `SELECT tool_name,capability_id,provider_account_id,route_position
FROM provider_capability_bindings WHERE tool_name=? ORDER BY route_position`, toolName)
	if err != nil {
		return nil, fmt.Errorf("query web provider route: %w", err)
	}
	defer rows.Close()
	var result []WebProviderBinding
	for rows.Next() {
		var value WebProviderBinding
		if err := rows.Scan(&value.ToolName, &value.CapabilityID, &value.ProviderAccountID, &value.RoutePosition); err != nil {
			return nil, fmt.Errorf("scan web provider route: %w", err)
		}
		result = append(result, value)
	}
	return result, rows.Err()
}

// HasWebProviderOverride reports whether search or fetch has an explicit choice.
func (s *Store) HasWebProviderOverride(ctx context.Context) (bool, error) {
	var count int
	err := s.db.QueryRowContext(ctx, `SELECT COUNT(*) FROM provider_capability_bindings
WHERE tool_name IN ('web.search','web.fetch')`).Scan(&count)
	return count != 0, err
}

// SaveWebProviderBinding replaces one search or fetch choice.
func (s *Store) SaveWebProviderBinding(ctx context.Context, toolName, accountID string, now time.Time) error {
	if toolName != "web.search" && toolName != "web.fetch" {
		return errors.New("web provider binding is invalid")
	}
	return s.replaceWebProviderRoute(ctx, toolName, []string{accountID}, now)
}

// ClearWebProviderOverrides restores native hosted web selection.
func (s *Store) ClearWebProviderOverrides(ctx context.Context) error {
	_, err := s.db.ExecContext(ctx, `DELETE FROM provider_capability_bindings WHERE tool_name IN ('web.search','web.fetch')`)
	return err
}

// SaveBrowserProviderRoute replaces the ordered browser provider route.
func (s *Store) SaveBrowserProviderRoute(ctx context.Context, accountIDs []string, now time.Time) error {
	if len(accountIDs) == 0 {
		return errors.New("browser provider route cannot be empty")
	}
	return s.replaceWebProviderRoute(ctx, "web.browse", accountIDs, now)
}

func (s *Store) replaceWebProviderRoute(ctx context.Context, toolName string, accountIDs []string, now time.Time) error {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	seen := map[string]bool{}
	for _, accountID := range accountIDs {
		if seen[accountID] {
			return errors.New("web provider route contains a duplicate account")
		}
		seen[accountID] = true
		account, err := scanProviderAccount(tx.QueryRowContext(ctx, providerAccountSelect+` WHERE provider_account_id=?`, accountID))
		if err != nil {
			return err
		}
		available := false
		for _, capability := range provider.Capabilities(account) {
			available = available || capability.ID == toolName && capability.Status == "available"
		}
		if !account.IsActive || !available {
			return errors.New("provider account does not supply the requested capability")
		}
	}
	if _, err := tx.ExecContext(ctx, `DELETE FROM provider_capability_bindings WHERE tool_name=?`, toolName); err != nil {
		return err
	}
	for position, accountID := range accountIDs {
		id := fmt.Sprintf("provider_capability_binding:%s:%s", toolName, toolName)
		if position != 0 {
			id = fmt.Sprintf("%s:%d", id, position)
		}
		if _, err := tx.ExecContext(ctx, `INSERT INTO provider_capability_bindings
(binding_id,tool_name,capability_id,provider_account_id,route_position,created_at_ms,updated_at_ms)
VALUES (?,?,?,?,?,?,?)`, id, toolName, toolName, accountID, position, millis(now.UTC()), millis(now.UTC())); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// ObserveURLs records exact public URLs returned by one structured web result.
func (s *Store) ObserveURLs(ctx context.Context, kind, source string, urls []string, now time.Time) error {
	if (kind != "search_result" && kind != "fetched_link" && kind != "browser_link") || source == "" || len(source) > 500 || len(urls) > 256 {
		return errors.New("observed URL input is invalid")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	for _, value := range urls {
		if value == "" || len(value) > 2048 {
			return errors.New("observed URL is invalid")
		}
		_, err := tx.ExecContext(ctx, `INSERT INTO observed_urls
(normalized_url,source_kind,source_event_reference,first_observed_at_ms,last_observed_at_ms)
VALUES (?,?,?,?,?) ON CONFLICT(normalized_url) DO UPDATE SET source_kind=excluded.source_kind,
source_event_reference=excluded.source_event_reference,last_observed_at_ms=excluded.last_observed_at_ms`,
			value, kind, source, millis(now.UTC()), millis(now.UTC()))
		if err != nil {
			return err
		}
	}
	return tx.Commit()
}

// URLWasObserved reports whether an exact normalized public URL was returned before.
func (s *Store) URLWasObserved(ctx context.Context, value string) (bool, error) {
	var exists int
	err := s.db.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM observed_urls WHERE normalized_url=?)`, value).Scan(&exists)
	return exists == 1, err
}
