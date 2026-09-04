package acp

import (
	"errors"
	"os/exec"
	"sync"
	"time"
)

type managedProcess struct {
	tree processTree
	done chan error
	once sync.Once
	err  error
}

func newManagedProcess(cmd *exec.Cmd, tree processTree) *managedProcess {
	process := &managedProcess{tree: tree, done: make(chan error, 1)}
	go func() { process.done <- cmd.Wait() }()
	return process
}

func (p *managedProcess) stop() error {
	p.once.Do(func() {
		defer p.tree.close()
		if p.wait(200 * time.Millisecond) {
			return
		}
		_ = p.tree.terminate(false)
		if p.wait(500 * time.Millisecond) {
			return
		}
		_ = p.tree.terminate(true)
		if !p.wait(2 * time.Second) {
			p.err = errors.New("ACP process tree did not stop")
		}
	})
	return p.err
}

func (p *managedProcess) wait(timeout time.Duration) bool {
	select {
	case <-p.done:
		return true
	case <-time.After(timeout):
		return false
	}
}
