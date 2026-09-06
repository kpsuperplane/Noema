package graphql

import (
	"context"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) taskExecutionPolicy(ctx context.Context) (*model.TaskExecutionPolicy, error) {
	value, err := r.Store.TaskExecutionPolicy(ctx)
	if err != nil {
		return nil, err
	}
	return taskExecutionPolicyModel(value), nil
}

func (r *Resolver) updateTaskExecutionPolicy(ctx context.Context, input model.TaskExecutionPolicyInput) (*model.TaskExecutionPolicy, error) {
	current, err := r.Store.TaskExecutionPolicy(ctx)
	if err != nil {
		return nil, err
	}
	current.MaxProviderContinuations = int64(input.MaxProviderContinuations)
	current.MaxToolCalls = int64(input.MaxToolCalls)
	current.MaxActiveMinutes = int64(input.MaxActiveMinutes)
	current.ProgressAuditInterval = int64(input.ProgressAuditInterval)
	value, err := r.Store.UpdateTaskExecutionPolicy(ctx, current)
	if err != nil {
		return nil, err
	}
	return taskExecutionPolicyModel(value), nil
}

func taskExecutionPolicyModel(value store.TaskExecutionPolicy) *model.TaskExecutionPolicy {
	return &model.TaskExecutionPolicy{MaxProviderContinuations: int(value.MaxProviderContinuations),
		MaxToolCalls: int(value.MaxToolCalls), MaxActiveMinutes: int(value.MaxActiveMinutes),
		ProgressAuditInterval: int(value.ProgressAuditInterval), MaxAutomaticRetries: int(value.MaxAutomaticRetries),
		MaxReviewRounds: int(value.MaxReviewRounds)}
}
