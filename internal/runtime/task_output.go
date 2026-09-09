package runtime

import (
	"context"
	"crypto/rand"
	"fmt"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *TaskExecution) taskOutputStream(ctx context.Context, run store.TaskRun, round int64) (*generationOutputStream, string, error) {
	var random [16]byte
	if _, err := rand.Read(random[:]); err != nil {
		return nil, "", err
	}
	id := fmt.Sprintf("run_item:%x", random)
	stream := newGenerationOutputStream(func(output []provider.GenerationOutput) error {
		status := "running"
		for _, item := range output {
			if item.Status == "failed" {
				status = "failed"
			}
		}
		return r.database.AppendTaskRunItems(context.WithoutCancel(ctx), run.ID, run.Generation, []store.TaskRunItemInput{{
			ID: id, Kind: "assistant_output", Status: status, Round: round, Payload: map[string]any{"output": output},
		}}, store.TaskRunUsage{}, time.Now())
	})
	return stream, id, nil
}

func taskOutputMessages(result provider.GenerationResult) []provider.GenerationMessage {
	// Native calls are replayed from their authoritative saved tool records.
	result.ToolCalls = nil
	return result.ReplayMessages()
}
