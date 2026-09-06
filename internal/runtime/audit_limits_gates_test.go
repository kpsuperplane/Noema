package runtime

import (
	"context"
	"fmt"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestAuditTaskGateAnswerReachesResumedProvider(t *testing.T) {
	for _, kind := range []string{"clarification", "approval"} {
		t.Run(kind, func(t *testing.T) {
			chat, db, _ := chatFixture(t)
			task := createQueuedRuntimeTask(t, db, chat.home, "Ask before continuing.")
			before := generatorFunc(func(context.Context, provider.GenerateRequest, func(provider.StreamEvent)) (provider.GenerationResult, error) {
				return taskToolResult("gate", taskReportBlocked, map[string]any{"gate_kind": kind, "question": "Use the selected target?", "context_markdown": "A human response is required."}), nil
			})
			worker, err := NewTaskExecution(t.Context(), db, before, before, before, chat.home)
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(worker.Close)
			waiting := waitRuntimeTask(t, db, task.ID, func(value store.Task) bool { return value.StageKey == "waiting" })
			worker.Close()
			gate, err := db.TaskGate(t.Context(), waiting.ActiveGateID)
			if err != nil || gate.Kind != kind || gate.Prompt != "Use the selected target?" {
				t.Fatal("wrong gate", gate, err)
			}
			const answer = "Use target café 日本語"
			var decision *string
			if kind == "approval" {
				value := "approved"
				decision = &value
			}
			command := runtimeTaskCommand("answer_task", "audit-answer")
			for range 2 {
				if _, err = db.ResolveTaskGate(t.Context(), task.ID, gate.ID, waiting.Revision, waiting.Generation, answer, "answer", decision, command, time.Now()); err != nil {
					t.Fatal(err)
				}
			}
			seen := make(chan provider.GenerateRequest, 1)
			after := generatorFunc(func(ctx context.Context, request provider.GenerateRequest, _ func(provider.StreamEvent)) (provider.GenerationResult, error) {
				seen <- request
				<-ctx.Done()
				return provider.GenerationResult{}, ctx.Err()
			})
			worker, err = NewTaskExecution(t.Context(), db, after, after, after, chat.home)
			if err != nil {
				t.Fatal(err)
			}
			t.Cleanup(worker.Close)
			select {
			case request := <-seen:
				text := fmt.Sprint(request.Messages)
				if !strings.Contains(text, answer) {
					t.Errorf("accepted answer absent from resumed provider context")
				}
				if taskRequestRole(request.Tools) != "planner" {
					t.Errorf("resumed wrong role")
				}
			case <-time.After(5 * time.Second):
				t.Fatal("provider did not resume")
			}
			worker.Close()
			runs, err := db.TaskRuns(t.Context(), task.ID, 10)
			if err != nil || len(runs) != 2 {
				t.Errorf("duplicate resolution created %d runs: %v", len(runs), err)
			}
			messages, err := db.TaskMessages(t.Context(), task.ID, 10)
			if err != nil || len(messages) != 1 || messages[0].Body != answer {
				t.Errorf("answer persistence failed: %#v, %v", messages, err)
			}
		})
	}
}
