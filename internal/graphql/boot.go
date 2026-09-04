package graphql

import (
	"context"

	"github.com/kpsuperplane/noema/internal/graphql/model"
)

func localStatus() *model.LocalStatus {
	return &model.LocalStatus{}
}

func localModelSetup() *model.LocalModelSetup {
	return &model.LocalModelSetup{
		RuntimeStatus: model.LocalModelRuntimeStatusInactive,
		IsReady:       false,
	}
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
