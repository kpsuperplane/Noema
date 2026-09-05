package provider

import (
	"errors"
	"os/exec"
	"sync"
	"time"
)

type foundationProcess struct {
	command *exec.Cmd
	done    chan error
	once    sync.Once
	err     error
}

func newFoundationProcess(command *exec.Cmd) *foundationProcess {
	process := &foundationProcess{command: command, done: make(chan error, 1)}
	go func() { process.done <- command.Wait() }()
	return process
}

func (p *foundationProcess) stop() error {
	p.once.Do(func() {
		select {
		case <-p.done:
			return
		default:
		}
		_ = p.command.Process.Kill()
		select {
		case <-p.done:
		case <-time.After(2 * time.Second):
			p.err = errors.New("Foundation Models bridge process did not stop")
		}
	})
	return p.err
}
