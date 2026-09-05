package store

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"time"
)

// AdapterDefinitionIndex is one safe filesystem-derived definition row.
type AdapterDefinitionIndex struct {
	Digest, DefinitionID, AdapterID, DefinitionRevision, SourceReference, DisplayName string
	Reviewed, Superseded                                                              bool
	OperationCount                                                                    int
}

// AdapterConnectionIndex is one safe filesystem-derived connection row.
type AdapterConnectionIndex struct {
	ID, Slug, Label, Digest, Status, DataSharingPolicy, UnsafeActionPolicy string
	ConnectionRevision, PolicyRevision                                     int
	AllowedOperations                                                      []string
}

// ReconcileAdapters replaces disposable adapter indexes in one transaction.
func (s *Store) ReconcileAdapters(ctx context.Context, definitions []AdapterDefinitionIndex, connections []AdapterConnectionIndex, now time.Time) error {
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer func() { _ = tx.Rollback() }()
	if _, err = tx.ExecContext(ctx, "DELETE FROM adapter_connections"); err != nil {
		return err
	}
	if _, err = tx.ExecContext(ctx, "DELETE FROM adapter_definitions"); err != nil {
		return err
	}
	for _, value := range definitions {
		if _, err = tx.ExecContext(ctx, `INSERT INTO adapter_definitions(semantic_digest,definition_id,adapter_id,definition_revision,source_reference,display_name,reviewed,superseded,operation_count,manifest_relative_path,updated_at_ms) VALUES(?,?,?,?,?,?,?,?,?,?,?)`,
			value.Digest, value.DefinitionID, value.AdapterID, value.DefinitionRevision, value.SourceReference, value.DisplayName, value.Reviewed, value.Superseded, value.OperationCount, "adapters/definitions/"+value.Digest+"/manifest.json", millis(now)); err != nil {
			return err
		}
	}
	for _, value := range connections {
		allowed, marshalErr := json.Marshal(value.AllowedOperations)
		if marshalErr != nil {
			return marshalErr
		}
		var sharing, unsafe any
		if value.DataSharingPolicy != "" {
			sharing = value.DataSharingPolicy
		}
		if value.UnsafeActionPolicy != "" {
			unsafe = value.UnsafeActionPolicy
		}
		if _, err = tx.ExecContext(ctx, `INSERT INTO adapter_connections(connection_id,connection_slug,connection_label,semantic_digest,status,connection_revision,policy_revision,data_sharing_policy,unsafe_action_policy,allowed_operations_json,descriptor_relative_path,updated_at_ms) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)`,
			value.ID, value.Slug, value.Label, value.Digest, value.Status, value.ConnectionRevision, value.PolicyRevision, sharing, unsafe, string(allowed), "adapters/connections/"+value.ID+"/connection.json", millis(now)); err != nil {
			return err
		}
	}
	return tx.Commit()
}

// AdapterIndexCounts returns current index counts for recovery checks.
func (s *Store) AdapterIndexCounts(ctx context.Context) (int, int, error) {
	var definitions, connections int
	if err := s.db.QueryRowContext(ctx, "SELECT count(*) FROM adapter_definitions").Scan(&definitions); err != nil {
		return 0, 0, err
	}
	if err := s.db.QueryRowContext(ctx, "SELECT count(*) FROM adapter_connections").Scan(&connections); err != nil {
		return 0, 0, err
	}
	return definitions, connections, nil
}

// HasAdapterReferences reports whether current ACP configuration names one adapter definition.
func (s *Store) HasAdapterReferences(ctx context.Context, definitionID string) (bool, error) {
	if definitionID == "" {
		return false, errors.New("adapter definition is required")
	}
	// Task and Chat action requests retain exact completed history. Only active requests block removal.
	var exists bool
	err := s.db.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM action_requests WHERE state IN ('proposed','awaiting_approval','executable','executing') AND json_extract(authorization_context_json,'$.destination.service_id')=?)`, definitionID).Scan(&exists)
	return exists, err
}
