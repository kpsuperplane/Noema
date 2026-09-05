package notification

import (
	"bytes"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"sort"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/store"
)

const (
	liveActivityTopic          = "dev.noema.app.ios.push-type.liveactivity"
	liveActivityAttributesType = "NoemaTasksActivityAttributes"
)

type liveProjection struct {
	Content       map[string]any
	Signature     string
	FocusedTaskID string
}

type projectedTask struct {
	Task store.Task
	Run  *store.TaskRun
}

func (s *Service) reconcileLiveActivities(ctx context.Context) error {
	credential, err := readAPNSCredential(s.paths.APNSProvider())
	if err != nil || !credential.Configured {
		return nil
	}
	targets, err := s.database.LiveActivityTargets(ctx)
	if err != nil {
		return err
	}
	projection, err := s.liveProjection(ctx)
	if err != nil {
		return err
	}
	for _, target := range targets {
		if err := s.applyLiveProjection(ctx, target, projection); err != nil {
			return err
		}
	}
	return nil
}

func (s *Service) liveProjection(ctx context.Context) (*liveProjection, error) {
	var tasks []projectedTask
	var after *string
	for {
		page, err := s.database.ListTasks(ctx, store.TaskListFilter{
			StageKeys: []string{"doing", "waiting"}, Scope: "active",
		}, 100, after)
		if err != nil {
			return nil, err
		}
		for _, task := range page.Tasks {
			run, runErr := s.database.CurrentTaskRun(ctx, task)
			if runErr != nil {
				return nil, runErr
			}
			tasks = append(tasks, projectedTask{Task: task, Run: run})
		}
		if !page.HasNextPage || page.EndCursor == nil {
			break
		}
		after = page.EndCursor
	}
	if len(tasks) == 0 {
		return nil, nil
	}
	sort.Slice(tasks, func(i, j int) bool {
		left, right := tasks[i], tasks[j]
		if liveFocusRank(left.Run) != liveFocusRank(right.Run) {
			return liveFocusRank(left.Run) < liveFocusRank(right.Run)
		}
		leftUpdated, rightUpdated := left.Task.UpdatedAt, right.Task.UpdatedAt
		if left.Run != nil && left.Run.UpdatedAt.After(leftUpdated) {
			leftUpdated = left.Run.UpdatedAt
		}
		if right.Run != nil && right.Run.UpdatedAt.After(rightUpdated) {
			rightUpdated = right.Run.UpdatedAt
		}
		if !leftUpdated.Equal(rightUpdated) {
			return leftUpdated.After(rightUpdated)
		}
		return left.Task.ID < right.Task.ID
	})
	focus := tasks[0]
	agentID := focus.Task.ExecutorAgentID
	if focus.Run != nil {
		agentID = focus.Run.AgentID
	}
	agentName := "Agent"
	if agent, err := s.database.Agent(ctx, agentID); err == nil && agent.DisplayName != nil {
		agentName = *agent.DisplayName
	}
	var projectName any
	if focus.Task.ProjectID != "" {
		project, err := s.database.Project(ctx, focus.Task.ProjectID)
		if err != nil {
			return nil, err
		}
		projectName = project.Name
	}
	updatedAt := focus.Task.UpdatedAt
	var startedAt any
	if focus.Run != nil {
		if focus.Run.UpdatedAt.After(updatedAt) {
			updatedAt = focus.Run.UpdatedAt
		}
		if focus.Run.StartedAt != nil {
			startedAt = epoch(*focus.Run.StartedAt)
		}
	}
	updateLabel, updateAt := s.liveRunUpdate(ctx, focus.Run)
	if updateAt != nil && updateAt.After(updatedAt) {
		updatedAt = *updateAt
	}
	summaries := make([]any, 0, min(2, len(tasks)))
	for _, task := range tasks[:min(2, len(tasks))] {
		var started any
		if task.Run != nil && task.Run.StartedAt != nil {
			started = epoch(*task.Run.StartedAt)
		}
		summaries = append(summaries, map[string]any{
			"taskId": task.Task.ID, "title": notificationText(task.Task.Title),
			"phase": liveTaskPhase(task), "statusLabel": liveTaskStatus(task),
			"startedAtEpoch": started, "requiresAttention": liveTaskAttention(task),
		})
	}
	content := map[string]any{
		"focusTaskId": focus.Task.ID, "focusTitle": notificationText(focus.Task.Title),
		"projectName": projectName, "agentName": notificationText(agentName),
		"phase": liveTaskPhase(focus), "statusLabel": liveTaskStatus(focus),
		"activeTaskCount": len(tasks), "startedAtEpoch": startedAt, "updatedAtEpoch": epoch(updatedAt),
		"requiresAttention": liveTaskAttention(focus), "updateLabel": updateLabel,
		"updateAtEpoch":        map[bool]any{true: epoch(updatedAt), false: nil}[updateLabel != nil],
		"completedOutputCount": nil, "taskSummaries": summaries,
	}
	return makeLiveProjection(content, focus.Task.ID)
}

func (s *Service) terminalLiveProjection(ctx context.Context, activity store.ClientTaskActivity) (*liveProjection, error) {
	if activity.FocusedTaskID == "" {
		return nil, nil
	}
	task, err := s.database.Task(ctx, activity.FocusedTaskID)
	if errors.Is(err, store.ErrTaskNotFound) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	phase, label := "", ""
	if task.StageKey == "done" {
		phase, label = "completed", "Completed"
	} else if task.StageKey == "cancelled" {
		phase, label = "cancelled", "Cancelled"
	} else {
		return nil, nil
	}
	run, err := s.database.CurrentTaskRun(ctx, task)
	if err != nil {
		return nil, err
	}
	agentID := task.ExecutorAgentID
	if run != nil {
		agentID = run.AgentID
	}
	agentName := "Agent"
	if agent, agentErr := s.database.Agent(ctx, agentID); agentErr == nil && agent.DisplayName != nil {
		agentName = *agent.DisplayName
	}
	var projectName any
	if task.ProjectID != "" {
		project, projectErr := s.database.Project(ctx, task.ProjectID)
		if projectErr != nil {
			return nil, projectErr
		}
		projectName = project.Name
	}
	count, err := s.database.TaskArtifactCount(ctx, task.ID)
	if err != nil {
		return nil, err
	}
	var outputCount any
	if count > 0 {
		outputCount = count
	}
	return makeLiveProjection(map[string]any{
		"focusTaskId": task.ID, "focusTitle": notificationText(task.Title), "projectName": projectName,
		"agentName": notificationText(agentName), "phase": phase, "statusLabel": label,
		"activeTaskCount": 0, "startedAtEpoch": nil, "updatedAtEpoch": epoch(task.UpdatedAt),
		"requiresAttention": false, "updateLabel": nil, "updateAtEpoch": nil,
		"completedOutputCount": outputCount, "taskSummaries": []any{},
	}, task.ID)
}

func makeLiveProjection(content map[string]any, taskID string) (*liveProjection, error) {
	encoded, err := json.Marshal(content)
	if err != nil || len(encoded) > 4096 {
		return nil, errors.New("Live Activity projection exceeds 4096 bytes")
	}
	digest := sha256.Sum256(encoded)
	return &liveProjection{Content: content, Signature: hex.EncodeToString(digest[:]), FocusedTaskID: taskID}, nil
}

func (s *Service) applyLiveProjection(ctx context.Context, target store.LiveActivityTarget, projection *liveProjection) error {
	if target.Activity == nil {
		return nil
	}
	activity := *target.Activity
	if projection == nil {
		terminal, err := s.terminalLiveProjection(ctx, activity)
		if err != nil {
			return err
		}
		if terminal == nil && activity.ProjectionSignature != "" {
			terminal = &liveProjection{Content: activity.Projection, Signature: activity.ProjectionSignature, FocusedTaskID: activity.FocusedTaskID}
		}
		if len(activity.UpdateToken) > 0 && terminal != nil && target.Registration.Environment != nil {
			if err := s.queueLiveActivity(ctx, target, activity, store.LiveActivityEnd,
				"live:end:"+activity.TaskSessionID, terminal, 600); err != nil {
				return err
			}
			_, err = s.database.MarkClientTaskActivityEnding(ctx, target.Registration.ClientID, activity.ActivityID, time.Now())
			return err
		}
		if activity.Lifecycle == "dismissed" {
			_, err = s.database.ClearClientTaskActivityDismissal(ctx, target.Registration.ClientID, time.Now())
			return err
		}
		return nil
	}
	for activity.Lifecycle != "starting" && activity.Lifecycle != "active" {
		if activity.Lifecycle == "ending" {
			return nil
		}
		if activity.Lifecycle == "dismissed" && activity.ProjectionSignature != "" {
			if activity.ProjectionSignature == strings.Repeat("0", 64) || activity.FocusedTaskID == projection.FocusedTaskID {
				return nil
			}
			if _, err := s.database.ClearClientTaskActivityDismissal(ctx, target.Registration.ClientID, time.Now()); err != nil {
				return err
			}
		}
		changed, err := s.database.EnsureClientTaskActivitySession(ctx, target.Registration.ClientID, time.Now())
		if err != nil || !changed {
			return err
		}
		current, err := s.database.ClientTaskActivity(ctx, target.Registration.ClientID)
		if err != nil || current == nil {
			return err
		}
		activity = *current
	}
	if _, err := s.database.UpdateClientTaskActivityProjection(ctx, target.Registration.ClientID,
		projection.Content, projection.Signature, projection.FocusedTaskID, time.Now()); err != nil {
		return err
	}
	if activity.Lifecycle == "active" && len(activity.UpdateToken) > 0 {
		return s.queueLiveActivity(ctx, target, activity, store.LiveActivityUpdate,
			"live:update:"+projection.Signature, projection, 3600)
	}
	if activity.Lifecycle == "starting" && len(target.Registration.PushToStartToken) > 0 {
		return s.queueLiveActivity(ctx, target, activity, store.LiveActivityStart,
			"live:start:"+activity.TaskSessionID, projection, 3600)
	}
	return nil
}

func liveFocusRank(run *store.TaskRun) int {
	if run == nil {
		return 3
	}
	switch run.Status {
	case "running":
		return 0
	case "leased":
		return 1
	case "queued":
		return 2
	default:
		return 3
	}
}

func liveTaskPhase(task projectedTask) string {
	if task.Task.StageKey == "waiting" {
		return "reviewing"
	}
	if task.Run == nil {
		return "inProgress"
	}
	return map[string]string{"planner": "planning", "executor": "working", "reviewer": "reviewing"}[task.Run.Kind]
}

func liveTaskStatus(task projectedTask) string {
	if task.Task.StageKey == "waiting" {
		return "Needs You"
	}
	if task.Run == nil {
		return "In progress"
	}
	if task.Run.Status == "running" {
		return map[string]string{"planner": "Planning", "executor": "Working", "reviewer": "Reviewing"}[task.Run.Kind]
	}
	return map[string]string{
		"leased": "Starting", "queued": "Queued", "waiting_for_approval": "Needs You",
		"completed": "Completed", "interrupted": "Needs attention", "failed": "Needs attention",
		"cancelled": "Cancelled",
	}[task.Run.Status]
}

func liveTaskAttention(task projectedTask) bool {
	if task.Task.StageKey == "waiting" {
		return true
	}
	return task.Run != nil && (task.Run.Status == "waiting_for_approval" || task.Run.Status == "interrupted" || task.Run.Status == "failed")
}

func (s *Service) liveRunUpdate(ctx context.Context, run *store.TaskRun) (any, *time.Time) {
	if run == nil {
		return nil, nil
	}
	page, err := s.database.TaskRunItems(ctx, run.ID, 20, nil)
	if err != nil {
		return nil, nil
	}
	return liveTranscriptUpdate(page.Items)
}

func liveTranscriptUpdate(items []store.TaskRunItem) (any, *time.Time) {
	for index := len(items) - 1; index >= 0; index-- {
		item := items[index]
		if item.Kind == "tool_call" && item.Status == "running" {
			label := liveToolUpdateLabel(item)
			for _, candidate := range items[index+1:] {
				if label == "" && candidate.Kind == "assistant_output" && candidate.Status == "completed" &&
					candidate.Round == item.Round && candidate.Sequence < item.Sequence && candidate.Content != nil {
					label = liveActivityText(*candidate.Content)
					break
				}
			}
			if label != "" {
				updated := item.UpdatedAt
				return label, &updated
			}
		}
	}
	for _, item := range items {
		if item.Kind == "assistant_output" && item.Status == "running" {
			return nil, nil
		}
		if item.Status != "completed" || item.Content == nil {
			continue
		}
		if item.Kind == "assistant_output" || item.Kind == "progress_notice" && item.Payload["phase"] != "provider_response" {
			if label := liveActivityText(*item.Content); label != "" {
				updated := item.UpdatedAt
				return label, &updated
			}
		}
	}
	return nil, nil
}

func liveToolUpdateLabel(item store.TaskRunItem) string {
	name, _ := item.Payload["name"].(string)
	if name == "" && item.Content != nil {
		name = strings.TrimSpace(*item.Content)
	}
	if name == "" {
		return ""
	}
	arguments, _ := item.Payload["arguments"].(map[string]any)
	path, _ := arguments["path"].(string)
	path = strings.TrimSpace(path)
	var label string
	switch name {
	case "task.files.list":
		label = "Listing Task files"
		if path != "" && path != "." {
			label += " in " + path
		}
	case "task.files.read":
		label = liveLabelTarget("Reading", path)
	case "task.files.write":
		label = liveLabelTarget("Writing", path)
	case "task.files.delete":
		label = liveLabelTarget("Deleting", path)
	case "task.finish_planning":
		label = "Finishing plan"
	case "task.finish_review":
		label = "Finishing review"
	case "task.report_blocked":
		label = "Reporting Task blocked"
	case "file.parse":
		label = liveLabelTarget("Parsing", path)
	default:
		if strings.HasPrefix(name, "task.") || strings.HasPrefix(name, "file.") ||
			strings.HasPrefix(name, "project.") || strings.HasPrefix(name, "artifact.") {
			label = strings.NewReplacer(".", " ", "_", " ", "-", " ").Replace(strings.ToLower(name))
			label = strings.ToUpper(label[:1]) + label[1:]
		}
	}
	return liveActivityText(label)
}

func liveLabelTarget(prefix, target string) string {
	if target == "" {
		return prefix
	}
	return prefix + " " + target
}

func liveActivityText(value string) string {
	plain := notificationText(value)
	if len(plain) <= 120 {
		return plain
	}
	end := 117
	for end > 0 && !utf8.RuneStart(plain[end]) {
		end--
	}
	return strings.TrimSpace(plain[:end]) + "…"
}

func epoch(value time.Time) float64 { return float64(value.UnixMilli()) / 1000 }

func (s *Service) queueLiveActivity(ctx context.Context, target store.LiveActivityTarget, activity store.ClientTaskActivity,
	event store.LiveActivityEvent, key string, projection *liveProjection, ttl int) error {
	if target.Registration.Environment == nil {
		return nil
	}
	token := activity.UpdateToken
	if event == store.LiveActivityStart {
		token = target.Registration.PushToStartToken
	}
	payload := liveActivityPayload(event, target.Registration.ClientID, activity.ActivityID, s.origin, projection)
	urgency := "normal"
	if event == store.LiveActivityStart {
		urgency = "high"
	}
	return s.database.QueueLiveActivityDelivery(ctx, store.NewLiveActivityDelivery{
		ClientID: target.Registration.ClientID, DeliveryKey: key, ActivityID: activity.ActivityID,
		Token: token, Environment: *target.Registration.Environment, Event: event, Payload: payload,
		Urgency: urgency, TTLSeconds: ttl,
	}, time.Now())
}

func liveActivityPayload(event store.LiveActivityEvent, clientID, activityID, origin string, projection *liveProjection) map[string]any {
	aps := map[string]any{
		"timestamp": projection.Content["updatedAtEpoch"], "event": string(event),
		"content-state": projection.Content,
	}
	if event == store.LiveActivityStart {
		aps["attributes-type"] = liveActivityAttributesType
		aps["attributes"] = map[string]any{
			"activityId": activityID, "clientId": clientID, "serverOrigin": origin,
		}
		aps["alert"] = map[string]any{"title": "Noema Tasks", "body": projection.Content["focusTitle"]}
	}
	return map[string]any{"aps": aps, "route": "task", "taskId": projection.FocusedTaskID, "version": 1}
}

func (s *Service) drainLiveActivities(ctx context.Context) error {
	for {
		value, err := s.database.ClaimDueLiveActivityDelivery(ctx, time.Now())
		if err != nil || value == nil {
			return err
		}
		if value.Event != store.LiveActivityEnd {
			current, currentErr := s.liveDeliveryCurrent(ctx, *value)
			if currentErr != nil {
				return currentErr
			}
			if !current {
				if err := s.database.FinishLiveActivityDelivery(ctx, *value, store.APNSSuppressed, "stale_start", "", time.Now()); err != nil {
					return err
				}
				continue
			}
		}
		outcome, code, apnsID, revision := s.sendLiveActivity(ctx, *value)
		if value.Event == store.LiveActivityStart && outcome == store.APNSRetry && code == "transport_unavailable" {
			outcome = store.APNSFailed
		}
		if code != "" && (outcome == store.APNSFailed || outcome == store.APNSRetry) {
			_ = s.recordAPNSError(code, revision)
		}
		if err := s.database.FinishLiveActivityDelivery(ctx, *value, outcome, code, apnsID, time.Now()); err != nil {
			return err
		}
	}
}

func (s *Service) liveDeliveryCurrent(ctx context.Context, value store.LiveActivityDelivery) (bool, error) {
	activity, err := s.database.ClientTaskActivity(ctx, value.ClientID)
	if err != nil || activity == nil || activity.ActivityID != value.ActivityID || activity.Suppressed {
		return false, err
	}
	registration, err := s.database.ClientLiveActivityRegistration(ctx, value.ClientID)
	if err != nil || registration == nil || !registration.Enabled || registration.Environment == nil || *registration.Environment != value.Environment {
		return false, err
	}
	if value.Event == store.LiveActivityStart {
		return activity.Lifecycle == "starting" && bytes.Equal(registration.PushToStartToken, value.Token), nil
	}
	return activity.Lifecycle == "active" && bytes.Equal(activity.UpdateToken, value.Token), nil
}

func (s *Service) sendLiveActivity(ctx context.Context, value store.LiveActivityDelivery) (store.APNSDeliveryOutcome, string, string, int) {
	credential, err := readAPNSCredential(s.paths.APNSProvider())
	if err != nil {
		return store.APNSRetry, "provider_unavailable", "", 0
	}
	if !credential.Configured {
		return store.APNSFailed, "provider_unconfigured", "", credential.Revision
	}
	private, err := parseAPNSKey(*credential.PrivateKeyPEM)
	if err != nil {
		return store.APNSFailed, "provider_key_invalid", "", credential.Revision
	}
	jwt, err := makeAPNSJWT(private, *credential.TeamID, *credential.KeyID, time.Now())
	if err != nil {
		return store.APNSFailed, "provider_token_failed", "", credential.Revision
	}
	body, err := marshalAPNSPayload(value.Payload)
	if err != nil {
		return store.APNSFailed, "invalid_payload", "", credential.Revision
	}
	host := "api.push.apple.com"
	if value.Environment == store.APNSDevelopment {
		host = "api.sandbox.push.apple.com"
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodPost,
		"https://"+host+"/3/device/"+hex.EncodeToString(value.Token), bytes.NewReader(body))
	if err != nil {
		return store.APNSFailed, "invalid_request", "", credential.Revision
	}
	expires := int64(0)
	if value.TTLSeconds > 0 {
		expires = value.CreatedAt.Unix() + int64(value.TTLSeconds)
		if expires <= time.Now().Unix() {
			return store.APNSFailed, "delivery_expired", "", credential.Revision
		}
	}
	request.Header.Set("authorization", "bearer "+jwt)
	request.Header.Set("content-type", "application/json")
	request.Header.Set("apns-topic", liveActivityTopic)
	request.Header.Set("apns-push-type", "liveactivity")
	request.Header.Set("apns-priority", map[bool]string{true: "10", false: "5"}[value.Urgency == "high"])
	request.Header.Set("apns-expiration", fmt.Sprint(expires))
	if value.Event == store.LiveActivityUpdate && value.Urgency == "normal" {
		digest := sha256.Sum256([]byte(value.ActivityID))
		request.Header.Set("apns-collapse-id", hex.EncodeToString(digest[:]))
	}
	response, err := s.apns.Do(request)
	if err != nil {
		return store.APNSRetry, "transport_unavailable", "", credential.Revision
	}
	defer response.Body.Close()
	responseBody, _ := io.ReadAll(io.LimitReader(response.Body, 4097))
	apnsID := response.Header.Get("apns-id")
	if len(apnsID) > 128 || !isASCII(apnsID) {
		apnsID = ""
	}
	outcome, code := apnsResponseOutcome(response.StatusCode, responseBody)
	return outcome, code, apnsID, credential.Revision
}
