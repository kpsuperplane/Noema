package provider

import (
	"errors"
	"os/exec"
	"sync"
	"time"
)

type foundationProcess struct {
	tree foundationProcessTree
	done chan error
	once sync.Once
	err  error
}

func attachFoundationProcess(command *exec.Cmd) (*foundationProcess, error) {
	tree, err := newFoundationProcessTree(command)
	if err != nil {
		return nil, err
	}
	process := &foundationProcess{tree: tree, done: make(chan error, 1)}
	go func() { process.done <- command.Wait() }()
	return process, nil
}

func (p *foundationProcess) stop() error {
	p.once.Do(func() {
		defer p.tree.close()
		select {
		case <-p.done:
			return
		default:
		}
		_ = p.tree.terminate()
		select {
		case <-p.done:
		case <-time.After(2 * time.Second):
			p.err = errors.New("Foundation Models bridge process did not stop")
		}
	})
	return p.err
}
