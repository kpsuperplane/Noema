package runtime

import (
	"context"
	"errors"
	"os/exec"
	"time"
)

type boundedCommandOutput struct {
	bytes     []byte
	limit     int
	truncated bool
}

func (w *boundedCommandOutput) Write(content []byte) (int, error) {
	available := max(w.limit-len(w.bytes), 0)
	w.bytes = append(w.bytes, content[:min(len(content), available)]...)
	w.truncated = w.truncated || len(content) > available
	return len(content), nil
}

func runFileParseCommand(
	ctx context.Context,
	command *exec.Cmd,
	outputLimit int,
) ([]byte, error, bool) {
	var output boundedCommandOutput
	output.limit = outputLimit
	command.Stdout = &output
	configureFileParseProcess(command)
	if err := command.Start(); err != nil {
		return nil, err, false
	}
	tree, err := attachFileParseProcess(command)
	if err != nil {
		_ = command.Process.Kill()
		_ = command.Wait()
		return nil, err, false
	}
	defer tree.close()
	done := make(chan error, 1)
	go func() { done <- command.Wait() }()
	select {
	case err = <-done:
		if output.truncated {
			return nil, errors.New("file parse worker output exceeded its limit"), false
		}
		return output.bytes, err, false
	case <-ctx.Done():
		_ = tree.terminate()
		select {
		case <-done:
		case <-time.After(2 * time.Second):
		}
		return nil, ctx.Err(), true
	}
}
