package store

import (
	"context"
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"maps"
	"time"
)

type PrimaryNotificationWrite struct {
	Event        WorkEvent
	Conversation Conversation
	Source       string
	Text         string
	Task         *Task
	Artifacts    []ArtifactWithVersions
	Metadata     map[string]any
}

func (s *Store) PrimaryTaskNotificationCursor(ctx context.Context) (int64, error) {
	var cursor int64
	err := s.db.QueryRowContext(ctx, `SELECT primary_task_notification_event_id
FROM local_human_state WHERE state_id=1`).Scan(&cursor)
	return cursor, err
}

func (s *Store) AdvancePrimaryTaskNotification(ctx context.Context, eventID int64) error {
	result, err := s.db.ExecContext(ctx, `UPDATE local_human_state
SET primary_task_notification_event_id=?
WHERE state_id=1 AND primary_task_notification_event_id < ?`, eventID, eventID)
	if err != nil {
		return err
	}
	changed, _ := result.RowsAffected()
	if changed == 0 {
		var current int64
		if err := s.db.QueryRowContext(ctx, `SELECT primary_task_notification_event_id
FROM local_human_state WHERE state_id=1`).Scan(&current); err != nil {
			return err
		}
		if current >= eventID {
			return nil
		}
		return errors.New("primary notification cursor changed")
	}
	return nil
}

func (s *Store) CommitPrimaryNotification(ctx context.Context, write PrimaryNotificationWrite, now time.Time) ([]ConversationItem, error) {
	if write.Event.ID < 1 || write.Conversation.ID == "" || write.Source == "" || write.Task == nil && write.Text == "" {
		return nil, errors.New("primary notification is invalid")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return nil, err
	}
	defer tx.Rollback()
	var cursor int64
	if err = tx.QueryRowContext(ctx, `SELECT primary_task_notification_event_id
FROM local_human_state WHERE state_id=1`).Scan(&cursor); err != nil {
		return nil, err
	}
	if write.Event.ID <= cursor {
		return nil, tx.Commit()
	}
	current, err := primaryConversationTx(ctx, tx)
	if err != nil || current.ID != write.Conversation.ID {
		return nil, errors.New("primary conversation changed")
	}
	now = now.UTC()
	notificationID := fmt.Sprintf("work-event:%d", write.Event.ID)
	turnID := ""
	turnIndex := int64(0)
	if write.Text != "" {
		turnID = notificationStableID("turn", notificationID)
		if err = tx.QueryRowContext(ctx, `SELECT COALESCE(MAX(CAST(json_extract(metadata_json,'$.turn_index') AS INTEGER)),0)+1
FROM conversation_turns WHERE conversation_id=?`, current.ID).Scan(&turnIndex); err != nil {
			return nil, err
		}
		metadata, _ := json.Marshal(map[string]any{"turn_index": turnIndex, "source": write.Source, "notification_id": notificationID})
		if _, err = tx.ExecContext(ctx, `INSERT INTO conversation_turns
(turn_id,conversation_id,status,metadata_json,started_at_ms,completed_at_ms,created_at_ms,updated_at_ms)
VALUES (?,?,'completed',?,?,?,?,?) ON CONFLICT(turn_id) DO NOTHING`, turnID, current.ID,
			string(metadata), millis(now), millis(now), millis(now), millis(now)); err != nil {
			return nil, err
		}
	}
	sequence, err := nextConversationSequenceTx(ctx, tx, current.ID)
	if err != nil {
		return nil, err
	}
	baseMetadata := map[string]any{"source": write.Source, "notification_id": notificationID, "work_event_id": write.Event.ID}
	for key, value := range write.Metadata {
		baseMetadata[key] = value
	}
	items := make([]ConversationItem, 0, 2+len(write.Artifacts))
	appendItem := func(kind ConversationItemKind, content string, payload map[string]any, suffix string) error {
		metadata := maps.Clone(baseMetadata)
		if turnIndex != 0 {
			metadata["turn_index"] = turnIndex
		}
		item, insertErr := insertConversationOutputTx(ctx, tx, ConversationItem{
			ID: notificationStableID("item", notificationID+":"+suffix), ConversationID: current.ID,
			TurnID: turnID, Sequence: sequence, Kind: kind, Status: "completed",
			AuthorActorID: "agent:primary", ContentText: content, Payload: payload,
			Metadata: metadata, CreatedAt: now,
		})
		if insertErr != nil {
			return insertErr
		}
		sequence++
		items = append(items, item)
		return nil
	}
	if write.Text != "" {
		if err = appendItem(ConversationAssistantText, write.Text, map[string]any{}, "text"); err != nil {
			return nil, err
		}
	}
	if write.Task != nil {
		suppress := false
		notificationKind, _ := write.Metadata["notification_kind"].(string)
		waitingNotification := notificationKind == "task_waiting" ||
			notificationKind == "" && write.Event.Kind == "task.started"
		if write.Text != "" && waitingNotification {
			var previous sql.NullInt64
			err = tx.QueryRowContext(ctx, `SELECT MAX(sequence_index) FROM conversation_items
WHERE conversation_id=? AND kind='task_reference' AND json_extract(payload_json,'$.task_id')=?`, current.ID, write.Task.ID).Scan(&previous)
			if err != nil {
				return nil, err
			}
			if previous.Valid {
				var messages int
				messageEnd := sequence
				// The notification text was appended above. Exclude that new
				// message when measuring the gap after the prior reference.
				messageEnd--
				err = tx.QueryRowContext(ctx, `SELECT COUNT(*) FROM conversation_items WHERE conversation_id=?
				AND sequence_index>? AND sequence_index<? AND kind IN ('user_text','assistant_text','multiple_choice_prompt','multiple_choice_selection')`, current.ID, previous.Int64, messageEnd).Scan(&messages)
				if err != nil {
					return nil, err
				}
				suppress = messages <= 1
			}
		}
		if turnID == "" && write.Task.Source.ConversationID == current.ID {
			turnID = write.Task.Source.TurnID
		}
		if !suppress {
			err = appendItem(ConversationTaskReference, "", map[string]any{"task_id": write.Task.ID}, "task")
		}
		if err != nil {
			return nil, err
		}
	}
	for _, artifact := range write.Artifacts {
		version := artifact.CurrentVersion
		payload := map[string]any{"artifact_id": artifact.Artifact.ID, "artifact_version_id": version.ID,
			"title": artifact.Artifact.Title, "artifact_kind": artifact.Artifact.Kind,
			"storage_kind": artifact.Artifact.StorageKind, "media_type": version.MediaType}
		if version.ExternalURL != nil {
			payload["external_url"] = *version.ExternalURL
		} else {
			payload["download_url"] = "/artifacts/versions/" + version.ID[len("artifact_version:"):] + "/download"
		}
		if err = appendItem(ConversationArtifactReference, "", payload, "artifact:"+version.ID); err != nil {
			return nil, err
		}
	}
	if _, err = tx.ExecContext(ctx, `UPDATE local_human_state SET primary_task_notification_event_id=?
WHERE state_id=1 AND primary_task_notification_event_id=?`, write.Event.ID, cursor); err != nil {
		return nil, err
	}
	if err = tx.Commit(); err != nil {
		return nil, err
	}
	return items, nil
}

// RecordCapabilityReady appends one idempotent API or MCP readiness event.
func (s *Store) RecordCapabilityReady(ctx context.Context, kind, name, connectionID, revision string, enabled int, now time.Time) error {
	if kind != "api" && kind != "mcp" || name == "" || connectionID == "" || revision == "" || enabled < 0 {
		return errors.New("capability readiness is invalid")
	}
	tx, err := s.db.BeginTx(ctx, &sql.TxOptions{Isolation: sql.LevelSerializable})
	if err != nil {
		return err
	}
	defer tx.Rollback()
	var exists bool
	err = tx.QueryRowContext(ctx, `SELECT EXISTS(SELECT 1 FROM work_events WHERE kind='capability.ready'
AND json_extract(payload_json,'$.integration_kind')=? AND json_extract(payload_json,'$.connection_id')=?
AND json_extract(payload_json,'$.connection_revision')=?)`, kind, connectionID, revision).Scan(&exists)
	if err != nil || exists {
		return err
	}
	_, err = insertWorkEvent(ctx, tx, "workspace:personal", "", "", "", 1, "capability.ready",
		"actor:human:local", nil, "correlation:capability:"+kind+":"+connectionID,
		map[string]any{"v": 1, "integration_kind": kind, "integration_name": name,
			"connection_id": connectionID, "connection_revision": revision, "enabled_tool_count": enabled}, now)
	if err == nil {
		err = tx.Commit()
	}
	if err == nil {
		s.NotifyWork()
	}
	return err
}

func notificationStableID(prefix, value string) string {
	digest := sha256.Sum256([]byte(value))
	return prefix + ":" + hex.EncodeToString(digest[:16])
}
