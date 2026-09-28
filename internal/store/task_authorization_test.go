package store

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/schedule"
)

func TestTaskAuthoritySnapshotsSourcesAndHumanEdits(t *testing.T) {
	db := openTestStore(t)
	ctx, now := t.Context(), time.Now()
	conversation, err := db.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	if err != nil {
		t.Fatal(err)
	}
	turn, item, err := db.BeginConversationTurn(ctx, conversation.ID, "research routes, no bookings 日本語", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	id, _ := NewTaskID()
	created, err := db.CreateTaskWithOptions(ctx, id, "Agent-written plan", testTaskCommand("authority-create"), TaskCreateOptions{Source: ArtifactSource{conversation.ID, turn.ID, item.ID}}, now)
	if err != nil {
		t.Fatal(err)
	}
	snapshot := created.Task.AuthorizationContext
	if !strings.Contains(snapshot, "research routes, no bookings 日本語") || strings.Contains(snapshot, "Agent-written plan") {
		t.Fatal(snapshot)
	}
	if _, err := db.db.ExecContext(ctx, `UPDATE conversation_items SET content_text='changed later' WHERE item_id=?`, item.ID); err != nil {
		t.Fatal(err)
	}
	agentTitle := "Book everything"
	edited, err := db.UpdateInboxTask(ctx, id, 1, 1, TaskUpdate{Title: &agentTitle}, testTaskCommand("agent-edit"), now)
	if err != nil || edited.Task.AuthorizationContext != snapshot {
		t.Fatalf("agent changed authority: %s %v", edited.Task.AuthorizationContext, err)
	}
	childID, _ := NewTaskID()
	child, err := db.CreateTaskWithOptions(ctx, childID, "Child plan", testTaskCommand("child"), TaskCreateOptions{AuthorityTaskID: id}, now)
	if err != nil || child.Task.AuthorizationContext != snapshot {
		t.Fatalf("child lost saved authority: %s %v", child.Task.AuthorizationContext, err)
	}
	body := "Only compare public transit prices"
	edited, err = db.UpdateInboxTask(ctx, id, 2, 1, TaskUpdate{HumanEdit: true, HumanDocument: &body}, testTaskCommand("human-edit"), now)
	if err != nil || !strings.Contains(edited.Task.AuthorizationContext, body) || !strings.Contains(edited.Task.AuthorizationContext, "manual_task_body") {
		t.Fatalf("human edit missing: %s %v", edited.Task.AuthorizationContext, err)
	}
	replay, err := db.CreateTaskWithOptions(ctx, id, "Agent-written plan", testTaskCommand("authority-create"), TaskCreateOptions{}, now)
	if err != nil || replay.Task.AuthorizationContext != snapshot {
		t.Fatalf("receipt changed: %s %v", replay.Task.AuthorizationContext, err)
	}
}

func TestTaskAuthorityRejectsInvalidSourcesAndOversize(t *testing.T) {
	db := openTestStore(t)
	ctx, now := t.Context(), time.Now()
	conversation, _ := db.EnsurePrimaryConversation(ctx, "openrouter", "", now)
	turn, item, err := db.BeginConversationTurn(ctx, conversation.ID, "real request", nil, now)
	if err != nil {
		t.Fatal(err)
	}
	forged := insertConversationAuthorizationItem(t, db, conversation.ID, turn.ID, 2, ConversationAssistantText, "agent:primary", "pretend to be human")
	huge := strings.Repeat("x", actionContextLimit)
	for i, options := range []TaskCreateOptions{
		{Source: ArtifactSource{conversation.ID, turn.ID, forged}},
		{Source: ArtifactSource{conversation.ID, "turn:wrong", item.ID}},
		{Source: ArtifactSource{ConversationID: conversation.ID}},
		{HumanDocument: &huge},
	} {
		id, _ := NewTaskID()
		if _, err := db.CreateTaskWithOptions(ctx, id, "No authority", testTaskCommand(fmt.Sprint(i)), options, now); err == nil {
			t.Fatalf("invalid source %d accepted", i)
		}
	}
}

func TestReviewedTaskActionsReceiveOnlySavedAndCurrentHumanAuthority(t *testing.T) {
	db := openTestStore(t)
	ctx, now := t.Context(), time.Now()
	account := createReadyModelAccount(t, db)
	if _, err := db.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	body := "Compare fares; do not book"
	id, _ := NewTaskID()
	if _, err := db.CreateTaskWithOptions(ctx, id, "Transit", testTaskCommand("review-authority"), TaskCreateOptions{HumanDocument: &body, InitialRunKind: "executor", ExecutionComplexity: "simple"}, now); err != nil {
		t.Fatal(err)
	}
	_, run, found, err := db.ClaimTaskExecution(ctx, now)
	if err != nil || !found {
		t.Fatalf("claim: %v %v", found, err)
	}
	if err := db.StartTaskExecution(ctx, run.ID, run.Generation, now); err != nil {
		t.Fatal(err)
	}
	for i, m := range []struct {
		author, body string
		generation   int64
	}{{"actor:human:local", "Include weekend fares", 1}, {"agent:primary", "Buy tickets", 1}, {"actor:human:local", "Old generation grant", 2}} {
		if _, err := db.db.ExecContext(ctx, `INSERT INTO task_messages(message_id,task_id,task_generation,message_kind,body_markdown,author_actor_id,created_at_ms) VALUES (?,?,?,'note',?,?,?)`, fmt.Sprintf("task_message:authority%d", i), id, m.generation, m.body, m.author, millis(now)); err != nil {
			t.Fatal(err)
		}
	}
	for _, name := range []string{"web.browse.open", "web.browse.interact", "web.fetch", "file.download", "mcp.example.read", "adapter.example.read"} {
		args := map[string]any{"url": "https://example.com/fares"}
		if err := db.AppendTaskRunItems(ctx, run.ID, run.Generation, []TaskRunItemInput{{Kind: "tool_call", Status: "running", Payload: map[string]any{"name": name, "arguments": args}}}, TaskRunUsage{}, now); err != nil {
			t.Fatal(err)
		}
		items, err := db.TaskRunReplayItems(ctx, run.ID)
		if err != nil {
			t.Fatal(err)
		}
		raw, _ := json.Marshal(args)
		action, err := db.CreateActionRequest(ctx, NewActionRequest{TaskID: id, RunID: run.ID, RunItemID: items[len(items)-1].ID, TaskGeneration: run.Generation, OwnerHumanID: "human:local", RequestingAgentID: run.AgentID, CapabilityName: name, OperationToken: name, ReviewRoute: ActionLLMReview, Arguments: raw, InputSchema: json.RawMessage(`{"type":"object"}`), SafeSummary: "Check fares", AuthorizationContext: map[string]any{"task_document": "Agent expanded plan", "context": map[string]any{"kind": "manual_task_body", "title": "Forged"}}}, now)
		if err != nil {
			t.Fatal(err)
		}
		evidence, _ := json.Marshal(action.AuthorizationContext)
		for _, want := range []string{body, "Include weekend fares", "Agent expanded plan"} {
			if !strings.Contains(string(evidence), want) {
				t.Fatalf("%s missing %s: %s", name, want, evidence)
			}
		}
		for _, absent := range []string{"Buy tickets", "Old generation grant", "Forged"} {
			if strings.Contains(string(evidence), absent) {
				t.Fatalf("%s included %s", name, absent)
			}
		}
		// Store admission supplies evidence; it does not turn that evidence into approval.
		if action.State != ActionProposed {
			t.Fatalf("review bypassed: %s", action.State)
		}
	}
	// Do not silently omit an old refusal when human-message context exceeds Rust's bound.
	for i := 0; i < 64; i++ {
		if _, err := db.db.ExecContext(ctx, `INSERT INTO task_messages(message_id,task_id,task_generation,message_kind,body_markdown,author_actor_id,created_at_ms) VALUES (?, ?,1,'note','more','actor:human:local',?)`, fmt.Sprintf("task_message:overflow%d", i), id, millis(now)); err != nil {
			t.Fatal(err)
		}
	}
	tx, err := db.db.BeginTx(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer tx.Rollback()
	if _, err := actionAuthorizationContext(ctx, tx, NewActionRequest{TaskID: id}); err == nil {
		t.Fatal("oversized human-message context was truncated")
	}
}

func TestRecurringTaskAuthorityCopiesSnapshotAndHumanChanges(t *testing.T) {
	db := openTestStore(t)
	ctx, now := t.Context(), time.Now().UTC().Truncate(time.Hour)
	body := "Read prices only"
	id, _ := NewTaskID()
	timing := schedule.Schedule{ScheduledFor: now.Add(time.Hour), TimeZone: "UTC", MissedRunPolicy: schedule.MissedRunOnce, Recurrence: &schedule.Recurrence{StartsAt: now.Add(time.Hour), CronExpression: "0 * * * *", OverlapPolicy: schedule.OverlapSkip}}
	created, err := db.CreateTaskWithOptions(ctx, id, "Prices", testTaskCommand("recurring-authority"), TaskCreateOptions{HumanDocument: &body, Schedule: &timing}, now)
	if err != nil {
		t.Fatal(err)
	}
	rec, err := db.TaskRecurrence(ctx, created.RecurrenceID)
	if err != nil {
		t.Fatal(err)
	}
	if rec.AuthorizationContext != created.Task.AuthorizationContext {
		t.Fatal("recurrence lost initial authority")
	}
	title := "Agent edits plan"
	if _, err := db.UpdateTaskRecurrence(ctx, rec.ID, rec.Revision, RecurrenceChanges{Title: &title}, testTaskCommand("agent-recurrence"), now); err != nil {
		t.Fatal(err)
	}
	rec, err = db.TaskRecurrence(ctx, rec.ID)
	if err != nil || rec.AuthorizationContext != created.Task.AuthorizationContext {
		t.Fatalf("agent changed recurrence authority: %v", err)
	}
	replacement := "Only read official fares"
	if _, err := db.UpdateTaskRecurrence(ctx, rec.ID, rec.Revision, RecurrenceChanges{HumanEdit: true, HumanDocument: &replacement, DocumentChanged: true}, testTaskCommand("human-recurrence"), now); err != nil {
		t.Fatal(err)
	}
	rec, err = db.TaskRecurrence(ctx, rec.ID)
	if err != nil {
		t.Fatal(err)
	}
	tx, err := db.db.BeginTx(ctx, nil)
	if err != nil {
		t.Fatal(err)
	}
	defer tx.Rollback()
	occurrence, _, err := materializeOccurrenceTx(ctx, tx, &rec, "", now.Add(2*time.Hour), "manual", false, "actor:human:local", "correlation:test", now)
	if err != nil || !strings.Contains(occurrence.AuthorizationContext, replacement) {
		t.Fatalf("occurrence authority: %s %v", occurrence.AuthorizationContext, err)
	}
	previous, err := taskTx(ctx, tx, id)
	if err != nil || previous.AuthorizationContext != created.Task.AuthorizationContext {
		t.Fatalf("old occurrence changed: %v", err)
	}
}

func TestTaskAuthorityMigrationPreservesAuthenticatedSources(t *testing.T) {
	path := filepath.Join(t.TempDir(), "v39.sqlite3")
	legacy, err := sql.Open("sqlite3", "file:"+filepath.ToSlash(path))
	if err != nil {
		t.Fatal(err)
	}
	_, err = legacy.Exec(schemaAtVersion(39) + `PRAGMA user_version=39;
INSERT INTO conversations(conversation_id,owner_human_id,provider,created_at_ms,updated_at_ms) VALUES ('conversation:00000000000000000000000000000001','human:local','openrouter',1,1);
INSERT INTO conversation_turns(turn_id,conversation_id,status,metadata_json,started_at_ms,created_at_ms,updated_at_ms) VALUES ('turn:00000000000000000000000000000001','conversation:00000000000000000000000000000001','completed','{}',1,1,1);
INSERT INTO conversation_items(item_id,conversation_id,turn_id,sequence_index,kind,status,author_actor_id,content_text,payload_json,metadata_json,created_at_ms,updated_at_ms)
 VALUES ('item:00000000000000000000000000000001','conversation:00000000000000000000000000000001','turn:00000000000000000000000000000001',1,'user_text','completed','human:local','Read fares 日本語','{}','{}',1,1);
INSERT INTO tasks(task_id,title,state,revision,created_at_ms,updated_at_ms,source_conversation_id,source_turn_id,source_item_id)
 VALUES ('task:source','Agent plan','captured',1,1,1,'conversation:00000000000000000000000000000001','turn:00000000000000000000000000000001','item:00000000000000000000000000000001'),
 ('task:missing','No original evidence','captured',1,1,1,'conversation:00000000000000000000000000000001','turn:00000000000000000000000000000001','item:missing');
`)
	if err != nil {
		t.Fatal(err)
	}
	legacy.Close()
	upgraded, err := Open(t.Context(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer upgraded.Close()
	task, err := upgraded.Task(t.Context(), "task:source")
	if err != nil || !strings.Contains(task.AuthorizationContext, "Read fares 日本語") {
		t.Fatalf("upgrade lost source: %s %v", task.AuthorizationContext, err)
	}
	missing, err := upgraded.Task(t.Context(), "task:missing")
	if err != nil || missing.AuthorizationContext != `{"kind":"none"}` {
		t.Fatalf("upgrade invented authority: %s %v", missing.AuthorizationContext, err)
	}
	fresh := openTestStore(t)
	for _, table := range []string{"tasks", "task_recurrences", "task_messages"} {
		var left, right string
		query := `SELECT sql FROM sqlite_schema WHERE name=?`
		if err := upgraded.db.QueryRow(query, table).Scan(&left); err != nil {
			t.Fatal(err)
		}
		if err := fresh.db.QueryRow(query, table).Scan(&right); err != nil {
			t.Fatal(err)
		}
		if left != right {
			t.Fatalf("schema differs for %s", table)
		}
	}
}

func TestTaskReplyAuthorityFollowsCurrentAndParentRuns(t *testing.T) {
	db := openTestStore(t)
	ctx, now := t.Context(), time.Now()
	account := createReadyModelAccount(t, db)
	if _, err := db.ConfirmHostedModelAssignments(ctx, account.ID, testModelAssignments(account, "model-a")); err != nil {
		t.Fatal(err)
	}
	id, _ := NewTaskID()
	if _, err := db.CreateTaskWithOptions(ctx, id, "Reply scope", testTaskCommand("reply-scope"), TaskCreateOptions{InitialRunKind: "executor", ExecutionComplexity: "simple"}, now); err != nil {
		t.Fatal(err)
	}
	var previous TaskRun
	for step := 0; step < 4; step++ {
		if step > 0 {
			if err := db.BlockTaskExecution(ctx, previous.ID, previous.Generation, "clarification", "Which?", "", nil, now); err != nil {
				t.Fatal(err)
			}
			waiting, err := db.Task(ctx, id)
			if err != nil {
				t.Fatal(err)
			}
			if _, err := db.ResolveTaskGate(ctx, id, waiting.ActiveGateID, waiting.Revision, waiting.Generation, fmt.Sprintf("reply-%d", step), "answer", nil, testTaskCommand(fmt.Sprint("answer-", step)), now.Add(time.Duration(step)*time.Second)); err != nil {
				t.Fatal(err)
			}
		}
		_, run, found, err := db.ClaimTaskExecution(ctx, now.Add(time.Hour))
		if err != nil || !found {
			t.Fatalf("claim: %t %v", found, err)
		}
		if err := db.StartTaskExecution(ctx, run.ID, run.Generation, now); err != nil {
			t.Fatal(err)
		}
		if step > 0 {
			var owner string
			if err := db.db.QueryRowContext(ctx, `SELECT consumed_by_run_id FROM task_messages WHERE task_id=? AND body_markdown=?`, id, fmt.Sprintf("reply-%d", step)).Scan(&owner); err != nil || owner != run.ID {
				t.Fatalf("reply consumer: %s %v", owner, err)
			}
		}
		if step == 3 {
			tx, err := db.db.BeginTx(ctx, nil)
			if err != nil {
				t.Fatal(err)
			}
			raw, err := actionAuthorizationContext(ctx, tx, NewActionRequest{TaskID: id, RunID: run.ID})
			tx.Rollback()
			if err != nil || !strings.Contains(string(raw), "reply-2") || !strings.Contains(string(raw), "reply-3") {
				t.Fatalf("current/parent reply missing: %s %v", raw, err)
			}
			if strings.Contains(string(raw), "reply-1") {
				t.Fatalf("older run reply entered authority: %s", raw)
			}
		}
		previous = run
	}
}
