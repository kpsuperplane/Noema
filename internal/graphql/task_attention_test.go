package graphql

import (
	"context"
	"strings"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestTaskAttentionMapsEveryGateKindAndBoundsUnicode(t *testing.T) {
	task := &model.TaskCard{TaskID: "task:0123456789abcdef0123456789abcdef",
		ValidActions: []model.ValidTaskAction{model.ValidTaskActionAnswer, model.ValidTaskActionCancel}}
	cases := []struct {
		kind  string
		want  model.TaskAttentionKind
		title string
	}{
		{"clarification", model.TaskAttentionKindClarificationRequired, "Clarification required"},
		{"approval", model.TaskAttentionKindApprovalRequired, "Approval required"},
		{"recovery", model.TaskAttentionKindRecoveryRequired, "Recovery decision required"},
	}
	for _, test := range cases {
		t.Run(test.kind, func(t *testing.T) {
			gate := store.TaskGate{Kind: test.kind, Prompt: strings.Repeat("界", 401)}
			attention := taskAttentionModel(task, &gate)
			if attention == nil || attention.Kind != test.want || attention.Title != test.title ||
				len([]rune(attention.Summary)) != 400 || attention.Task != task || len(attention.ValidActions) != 2 {
				t.Fatalf("attention = %#v", attention)
			}
		})
	}
}

func TestTaskAttentionReadsUseExistingFiltersAndCursors(t *testing.T) {
	r := readyAgentTestResolver(t)
	ctx := context.Background()
	project, err := r.createProject(ctx, model.CreateProjectInput{
		WorkspaceID: personalWorkspaceID, Name: "Attention", ClientMutationID: "attention-project",
	})
	if err != nil {
		t.Fatal(err)
	}
	projectID := project.Project.ProjectID
	clarificationID := openAttentionTask(t, r, "Clarify", &projectID, "clarification", strings.Repeat("界", 401))
	openAttentionTask(t, r, "Approve", &projectID, "approval", "Approve this action?")
	openAttentionTask(t, r, "Recover", nil, "recovery", "The run failed.")

	detail, err := r.task(ctx, clarificationID)
	if err != nil || detail.Attention == nil || detail.Attention.Task.TaskID != clarificationID ||
		len([]rune(detail.Attention.Summary)) != 400 {
		t.Fatalf("Task detail attention = %#v, %v", detail, err)
	}
	listed, err := r.tasks(ctx, model.TaskListInput{WorkspaceID: personalWorkspaceID,
		ProjectID: &projectID, AttentionOnly: true, Scope: model.TaskScopeActive}, nil, nil)
	if err != nil || len(listed.Edges) != 2 {
		t.Fatalf("attention Task list = %#v, %v", listed, err)
	}
	for _, edge := range listed.Edges {
		if edge.Node.Attention == nil || edge.Node.Attention.Task.TaskID != edge.Node.TaskID {
			t.Fatalf("Task summary attention = %#v", edge.Node)
		}
	}

	first := 1
	root := &queryRootResolver{r}
	pageOne, err := root.NeedsYou(ctx, personalWorkspaceID, &projectID, &first, nil)
	if err != nil || len(pageOne.Edges) != 1 || !pageOne.PageInfo.HasNextPage || pageOne.PageInfo.EndCursor == nil {
		t.Fatalf("first Needs You page = %#v, %v", pageOne, err)
	}
	pageTwo, err := root.NeedsYou(ctx, personalWorkspaceID, &projectID, &first, pageOne.PageInfo.EndCursor)
	if err != nil || len(pageTwo.Edges) != 1 || pageTwo.PageInfo.HasNextPage ||
		pageTwo.Edges[0].Node.Task.Project.ProjectID != projectID {
		t.Fatalf("second Needs You page = %#v, %v", pageTwo, err)
	}
}

func openAttentionTask(t *testing.T, r *Resolver, title string, projectID *string, kind, prompt string) string {
	t.Helper()
	ctx, now := context.Background(), time.Now()
	captured, err := r.captureTask(ctx, model.CaptureTaskInput{WorkspaceID: personalWorkspaceID,
		ProjectID: projectID, Title: title, TaskDocument: title, ClientMutationID: "capture-" + title})
	if err != nil {
		t.Fatal(err)
	}
	queued, err := r.queueTask(ctx, model.QueueTaskInput{TaskID: captured.Task.TaskID,
		ExpectedRevision: 1, ExpectedGeneration: 1, ClientMutationID: "queue-" + title})
	if err != nil {
		t.Fatal(err)
	}
	_, run, found, err := r.Store.ClaimTaskExecution(ctx, now)
	if err != nil || !found {
		t.Fatalf("claim Task = %#v, %v", run, err)
	}
	if err = r.Store.StartTaskExecution(ctx, run.ID, int64(queued.Task.Generation), now); err != nil {
		t.Fatal(err)
	}
	if kind == "recovery" {
		err = r.Store.FailTaskExecution(ctx, run.ID, int64(queued.Task.Generation), "provider_failed", prompt, false, now)
	} else {
		err = r.Store.BlockTaskExecution(ctx, run.ID, int64(queued.Task.Generation), kind, prompt, "Context", nil, now)
	}
	if err != nil {
		t.Fatal(err)
	}
	return captured.Task.TaskID
}
