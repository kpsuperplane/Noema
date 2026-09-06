package graphql

import (
	"context"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) localStatus(ctx context.Context) (*model.LocalStatus, error) {
	agent, err := r.Store.Agent(ctx, store.PrimaryAgentID)
	if err != nil {
		return nil, err
	}
	return &model.LocalStatus{PrimaryAgentDisplayName: agent.DisplayName}, nil
}

func localModelEvents(ctx context.Context) <-chan *model.LocalModelEvent {
	events := make(chan *model.LocalModelEvent)
	go func() {
		defer close(events)
		<-ctx.Done()
	}()
	return events
}

func tasksEvents(ctx context.Context) <-chan *model.TasksEvent {
	events := make(chan *model.TasksEvent)
	go func() {
		defer close(events)
		<-ctx.Done()
	}()
	return events
}
