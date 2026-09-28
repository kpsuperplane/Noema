package store

import (
	"context"
	"encoding/json"
	"errors"

	"github.com/uptrace/bun"
)

// Task authority is saved separately from TASK.md because agents can edit that file.
func taskCreationAuthority(ctx context.Context, tx bun.Tx, title string, options TaskCreateOptions) (string, error) {
	if options.AuthorityTaskID != "" {
		parent, err := taskTx(ctx, tx, options.AuthorityTaskID)
		return parent.AuthorizationContext, err
	}
	if options.HumanDocument != nil {
		return manualTaskAuthority(title, options.HumanDocument)
	}
	source := options.Source
	if source.ConversationID == "" && source.TurnID == "" && source.ItemID == "" {
		return `{"kind":"none"}`, nil
	}
	if source.ConversationID == "" || source.TurnID == "" || source.ItemID == "" {
		return "", errors.New("Task authorization source is incomplete")
	}
	value, err := conversationAuthorizationContext(ctx, tx, source.ConversationID, source.TurnID, source.ItemID)
	if err != nil {
		return "", err
	}
	encoded, err := json.Marshal(value)
	return string(encoded), err
}

func manualTaskAuthority(title string, document *string) (string, error) {
	body := ""
	if document != nil {
		body = *document
	}
	encoded, err := json.Marshal(map[string]any{"kind": "manual_task_body", "title": title, "task_document_markdown": body})
	if err != nil || len(encoded) > actionContextLimit {
		return "", errors.New("Task authorization context is too large")
	}
	return string(encoded), nil
}

// Every reviewed tool uses the same authenticated evidence at admission.
func actionAuthorizationContext(ctx context.Context, tx bun.Tx, input NewActionRequest) ([]byte, error) {
	value := make(map[string]any, len(input.AuthorizationContext)+2)
	for key, entry := range input.AuthorizationContext {
		value[key] = entry
	}
	if input.TaskID == "" {
		authority, err := conversationAuthorizationContext(ctx, tx, input.ConversationID, input.TurnID, "")
		if err != nil {
			return nil, err
		}
		value["context"] = authority
		messages := authority["messages"].([]map[string]any)
		value["source_human_item_id"] = messages[len(messages)-1]["item_id"]
	} else {
		task, err := taskTx(ctx, tx, input.TaskID)
		if err != nil {
			return nil, err
		}
		var authority map[string]any
		if err := json.Unmarshal([]byte(task.AuthorizationContext), &authority); err != nil {
			return nil, err
		}
		rows, err := tx.QueryContext(ctx, `SELECT message_id,message_kind,body_markdown FROM task_messages
WHERE task_id=? AND task_generation=? AND author_actor_id='actor:human:local'
AND (consumed_by_run_id IS NULL OR consumed_by_run_id=? OR consumed_by_run_id=(SELECT parent_run_id FROM task_runs WHERE run_id=?))
ORDER BY created_at_ms,message_id LIMIT 65`, task.ID, task.Generation, input.RunID, input.RunID)
		if err != nil {
			return nil, err
		}
		defer rows.Close()
		messages := make([]map[string]any, 0)
		for rows.Next() {
			var id, kind, text string
			if err := rows.Scan(&id, &kind, &text); err != nil {
				return nil, err
			}
			messages = append(messages, map[string]any{"message_id": id, "kind": kind, "text": text})
		}
		if err := rows.Err(); err != nil {
			return nil, err
		}
		if len(messages) > 64 {
			return nil, errors.New("Task exceeds bounded human message context")
		}
		value["context"] = authority
		value["task_context"] = map[string]any{"title": task.Title, "task_document": value["task_document"], "human_messages": messages}
		delete(value, "task_title")
		delete(value, "task_document")
	}
	encoded, err := json.Marshal(value)
	if err != nil || len(encoded) > actionContextLimit {
		return nil, errors.New("action authorization context is invalid")
	}
	return encoded, nil
}

// Version 39 retained Chat source references, but not immutable Task authority.
// Recover only authenticated excerpts. Mutable Task files cannot recover manual authority.
func backfillTaskAuthorityV40(ctx context.Context, tx bun.Tx) error {
	var tasks []Task
	if err := tx.NewSelect().Model(&tasks).Where("source_item_id IS NOT NULL").Scan(ctx); err != nil {
		return err
	}
	for _, task := range tasks {
		authority, err := taskCreationAuthority(ctx, tx, task.Title, TaskCreateOptions{Source: task.Source})
		if err != nil {
			continue
		}
		if _, err := tx.ExecContext(ctx, `UPDATE tasks SET authorization_context_json=? WHERE task_id=?`, authority, task.ID); err != nil {
			return err
		}
	}
	_, err := tx.ExecContext(ctx, `UPDATE task_recurrences SET authorization_context_json=COALESCE(
(SELECT t.authorization_context_json FROM tasks t WHERE t.recurrence_id=task_recurrences.recurrence_id
ORDER BY t.created_at_ms,t.task_id LIMIT 1), authorization_context_json)`)
	return err
}
