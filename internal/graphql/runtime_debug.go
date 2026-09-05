package graphql

import (
	"context"
	"time"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) runtimeDebugProfile(ctx context.Context, input model.RuntimeDebugProfileInput) (*model.RuntimeDebugProfile, error) {
	kind := "conversation_turn"
	if input.Kind == model.RuntimeDebugScopeKindTaskRun {
		kind = "task_run"
	}
	profile, err := r.Store.RuntimeDebugProfile(ctx, store.RuntimeDebugScope{Kind: kind, ID: input.ScopeID})
	if err != nil || profile == nil || profile.OwnerHumanID != localHumanID || len(profile.Spans) == 0 {
		return nil, err
	}
	return runtimeDebugProfileModel(profile, time.Now()), nil
}

func runtimeDebugProfileModel(value *store.RuntimeDebugProfile, now time.Time) *model.RuntimeDebugProfile {
	status := debugStatus(value.Status)
	effectiveEnd := now.UTC()
	if value.EndedAt != nil {
		effectiveEnd = *value.EndedAt
	}
	elapsed := max(effectiveEnd.Sub(value.StartedAt).Milliseconds(), 0)
	result := &model.RuntimeDebugProfile{
		Kind: model.RuntimeDebugScopeKindConversationTurn, ScopeID: value.Scope.ID, Status: status,
		StartedAt: debugTime(value.StartedAt), ElapsedMilliseconds: debugInt(elapsed),
	}
	if value.Scope.Kind == "task_run" {
		result.Kind = model.RuntimeDebugScopeKindTaskRun
	}
	if value.EndedAt != nil {
		ended := debugTime(*value.EndedAt)
		result.EndedAt = &ended
	}
	intervals := make([][2]int64, 0, len(value.Spans))
	terminal := status != model.RuntimeDebugStatusRunning
	for _, span := range value.Spans {
		end := effectiveEnd
		if span.EndedAt != nil {
			end = *span.EndedAt
		}
		duration := max(end.Sub(span.StartedAt).Milliseconds(), 0)
		if span.DurationMilliseconds != nil {
			duration = *span.DurationMilliseconds
		}
		spanStatus := debugStatus(span.Status)
		if terminal && spanStatus == model.RuntimeDebugStatusRunning {
			spanStatus = model.RuntimeDebugStatusInterrupted
		}
		startOffset := max(span.StartedAt.Sub(value.StartedAt).Milliseconds(), 0)
		projected := &model.RuntimeDebugSpan{
			ID: span.ID, Category: debugCategory(span.Category), Name: span.Name, Status: spanStatus,
			StartedAt: debugTime(span.StartedAt), StartOffsetMilliseconds: debugInt(startOffset),
			DurationMilliseconds: debugInt(duration),
		}
		if span.EndedAt != nil {
			ended := debugTime(*span.EndedAt)
			projected.EndedAt = &ended
		}
		projectDebugMetadata(projected, span.Metadata)
		result.Spans = append(result.Spans, projected)
		intervalEnd := min(max(end.Sub(value.StartedAt).Milliseconds(), 0), elapsed)
		if intervalEnd > startOffset && startOffset < elapsed {
			intervals = append(intervals, [2]int64{startOffset, intervalEnd})
		}
	}
	accounted := unionDebugIntervals(intervals)
	result.AccountedMilliseconds = debugInt(accounted)
	result.UninstrumentedMilliseconds = debugInt(max(elapsed-accounted, 0))
	return result
}

func projectDebugMetadata(result *model.RuntimeDebugSpan, value store.RuntimeDebugMetadata) {
	result.Provider = debugString(value.Provider)
	result.Model = debugString(value.Model)
	result.Phase = debugString(value.Phase)
	result.ToolName = debugString(value.ToolName)
	result.CorrelationID = debugString(value.CorrelationID)
	result.ResponseIndex = value.ResponseIndex
	result.RoundIndex = value.RoundIndex
	result.InputTokens = value.InputTokens
	result.CachedInputTokens = value.CachedInputTokens
	result.OutputTokens = value.OutputTokens
	result.TotalTokens = value.TotalTokens
}

func unionDebugIntervals(values [][2]int64) int64 {
	var total, start, end int64
	for index, value := range values {
		if index == 0 || value[0] > end {
			if index != 0 {
				total += end - start
			}
			start, end = value[0], value[1]
		} else if value[1] > end {
			end = value[1]
		}
	}
	if len(values) != 0 {
		total += end - start
	}
	return total
}

func debugStatus(value string) model.RuntimeDebugStatus {
	status := map[string]model.RuntimeDebugStatus{
		"completed": model.RuntimeDebugStatusCompleted, "failed": model.RuntimeDebugStatusFailed,
		"cancelled": model.RuntimeDebugStatusCancelled, "interrupted": model.RuntimeDebugStatusInterrupted,
	}[value]
	if status == "" {
		return model.RuntimeDebugStatusRunning
	}
	return status
}

func debugCategory(value string) model.RuntimeDebugSpanCategory {
	return map[string]model.RuntimeDebugSpanCategory{
		"provider": model.RuntimeDebugSpanCategoryProvider, "tool": model.RuntimeDebugSpanCategoryTool,
		"runtime": model.RuntimeDebugSpanCategoryRuntime, "persistence": model.RuntimeDebugSpanCategoryPersistence,
	}[value]
}

func debugString(value string) *string {
	if value == "" {
		return nil
	}
	return &value
}

func debugTime(value time.Time) string { return value.UTC().Format(time.RFC3339Nano) }

func debugInt(value int64) int {
	limit := int64(^uint(0) >> 1)
	if value > limit {
		return int(limit)
	}
	return int(value)
}
