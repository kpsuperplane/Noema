package main

import (
	"errors"
	"os"
	"os/exec"
)

func shutdownSignals() []os.Signal { return []os.Signal{os.Interrupt} }
func preparePlatform() error {
	return errors.New("combined development requires Unix for the private GraphQL socket")
}
func runningAsRoot() bool              { return false }
func configureProcess(child *exec.Cmd) {}
func stopProcess(child *exec.Cmd) {
	if child != nil && child.Process != nil {
		_ = child.Process.Kill()
	}
}
