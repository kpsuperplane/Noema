//go:build unix

package main

import (
	"os"
	"os/exec"
	"runtime"
	"syscall"
	"time"
)

func shutdownSignals() []os.Signal { return []os.Signal{os.Interrupt, syscall.SIGTERM, syscall.SIGHUP} }
func preparePlatform() error {
	if runtime.GOOS == "linux" && os.Geteuid() == 0 {
		syscall.Umask(022)
		if err := os.Setenv("NOEMA_DEV_ASSET_DIR", "/run/noema-dev/web-assets"); err != nil {
			return err
		}
	}
	return nil
}
func runningAsRoot() bool              { return os.Geteuid() == 0 }
func configureProcess(child *exec.Cmd) { child.SysProcAttr = &syscall.SysProcAttr{Setpgid: true} }
func stopProcess(child *exec.Cmd) {
	if child == nil || child.Process == nil {
		return
	}
	_ = syscall.Kill(-child.Process.Pid, syscall.SIGTERM)
	time.Sleep(time.Second)
	_ = syscall.Kill(-child.Process.Pid, syscall.SIGKILL)
}
