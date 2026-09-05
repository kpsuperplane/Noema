package webtool

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"io"
	"os/exec"
	"strconv"
	"sync"
	"time"
)

const browserFrameLimit = 2 << 20

type browserProcess struct {
	command *exec.Cmd
	tree    browserProcessTree
	input   io.WriteCloser
	output  *bufio.Reader
	mu      sync.Mutex
	closed  bool
}

func startBrowserProcess(ctx context.Context, path string, oldSpaceMB int, generation uint64) (*browserProcess, error) {
	command := exec.Command(path, "--noema-browser-worker-v1", strconv.Itoa(oldSpaceMB), strconv.FormatUint(generation, 10))
	command.Env = browserWorkerEnvironment()
	configureBrowserCommand(command)
	input, err := command.StdinPipe()
	if err != nil {
		return nil, err
	}
	output, err := command.StdoutPipe()
	if err != nil {
		_ = input.Close()
		return nil, err
	}
	process := &browserProcess{command: command, input: input, output: bufio.NewReaderSize(output, 64*1024)}
	if err := command.Start(); err != nil {
		_ = input.Close()
		return nil, err
	}
	tree, err := attachBrowserCommand(command)
	if err != nil {
		_ = command.Process.Kill()
		_ = command.Wait()
		_ = input.Close()
		return nil, err
	}
	process.tree = tree
	start, cancel := context.WithTimeout(ctx, 10*time.Second)
	defer cancel()
	var handshake struct {
		Version int  `json:"version"`
		Ready   bool `json:"ready"`
	}
	if err := process.readJSON(start, &handshake); err != nil || handshake.Version != 1 || !handshake.Ready {
		process.close()
		return nil, errors.New("browser worker did not become ready")
	}
	return process, nil
}

func (p *browserProcess) call(ctx context.Context, request any, response any) error {
	p.mu.Lock()
	defer p.mu.Unlock()
	if p.closed {
		return errors.New("browser worker is unavailable")
	}
	data, err := json.Marshal(request)
	if err != nil || len(data) > browserFrameLimit {
		return errors.New("browser request is invalid")
	}
	command, cancel := context.WithTimeout(ctx, 30*time.Second)
	defer cancel()
	writeDone := make(chan error, 1)
	go func() { _, err := p.input.Write(append(data, '\n')); writeDone <- err }()
	select {
	case err = <-writeDone:
	case <-command.Done():
		err = command.Err()
	}
	if err == nil {
		err = p.readJSON(command, response)
	}
	if err != nil {
		p.closeLocked()
	}
	return err
}

func (p *browserProcess) readJSON(ctx context.Context, target any) error {
	type readResult struct {
		data []byte
		err  error
	}
	done := make(chan readResult, 1)
	go func() {
		line, err := p.output.ReadBytes('\n')
		if len(line) > browserFrameLimit {
			err = errors.New("browser worker frame is too large")
		}
		done <- readResult{line, err}
	}()
	select {
	case result := <-done:
		if result.err != nil {
			return result.err
		}
		if len(result.data) == 0 || len(result.data) > browserFrameLimit {
			return errors.New("browser worker frame is invalid")
		}
		decoder := json.NewDecoder(bytes.NewReader(result.data))
		decoder.DisallowUnknownFields()
		if err := decoder.Decode(target); err != nil {
			return errors.New("browser worker frame is invalid")
		}
		if decoder.Decode(&struct{}{}) == nil {
			return errors.New("browser worker frame is invalid")
		}
		return nil
	case <-ctx.Done():
		return ctx.Err()
	}
}

func (p *browserProcess) close() {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.closeLocked()
}

func (p *browserProcess) closeLocked() {
	if p.closed {
		return
	}
	p.closed = true
	_ = p.input.Close()
	terminateBrowserCommand(p.tree, p.command)
	_ = p.command.Wait()
	p.tree.close()
}
