//go:build linux

package home

import (
	"errors"
	"os"
	"os/exec"
	"os/user"
	"path/filepath"
	"strconv"
	"syscall"
	"testing"
)

func TestTaskFilesReportUnwritableHomeAndPreserveExistingContent(t *testing.T) {
	if os.Geteuid() != 0 {
		t.Skip("requires root to start the unprivileged helper")
	}
	nobody, err := user.Lookup("nobody")
	if err != nil {
		t.Skipf("nobody user unavailable: %v", err)
	}
	uid, err := strconv.ParseUint(nobody.Uid, 10, 32)
	if err != nil {
		t.Fatal(err)
	}
	gid, err := strconv.ParseUint(nobody.Gid, 10, 32)
	if err != nil {
		t.Fatal(err)
	}
	base := t.TempDir()
	if err := os.Chmod(base, 0o755); err != nil {
		t.Fatal(err)
	}
	binDir := filepath.Join(base, "bin")
	if err := os.Mkdir(binDir, 0o755); err != nil {
		t.Fatal(err)
	}
	path, err := os.MkdirTemp(base, "noema-home-")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		_ = filepath.Walk(path, func(name string, info os.FileInfo, walkErr error) error {
			if walkErr != nil {
				return walkErr
			}
			if info.IsDir() {
				return os.Chmod(name, 0o700)
			}
			return os.Chmod(name, 0o600)
		})
		_ = os.Chmod(binDir, 0o755)
		_ = os.Chmod(base, 0o755)
		_ = os.RemoveAll(base)
	})
	root, err := os.OpenRoot(path)
	if err != nil {
		t.Fatal(err)
	}
	taskID := "task:00000000000000000000000000000002"
	if _, err := CreatePendingTaskDocument(root, taskID, "# Task\n"); err != nil {
		t.Fatal(err)
	}
	if err := CommitTaskDocument(root, taskID); err != nil {
		t.Fatal(err)
	}
	if err := WriteTaskFile(root, taskID, "notes/progress.md", "committed\n"); err != nil {
		t.Fatal(err)
	}
	if err := root.Close(); err != nil {
		t.Fatal(err)
	}
	helperPath := filepath.Join(binDir, "noema-home-helper-"+filepath.Base(path)+".test")
	t.Cleanup(func() { _ = os.Remove(helperPath) })
	binary, err := os.ReadFile(os.Args[0])
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(helperPath, binary, 0o755); err != nil {
		t.Fatal(err)
	}
	taskName, err := taskName(taskID)
	if err != nil {
		t.Fatal(err)
	}
	if err := filepath.Walk(path, func(name string, info os.FileInfo, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if info.IsDir() {
			return os.Chmod(name, 0o555)
		}
		return os.Chmod(name, 0o444)
	}); err != nil {
		t.Fatal(err)
	}
	if err := os.Chmod(helperPath, 0o555); err != nil {
		t.Fatal(err)
	}
	cmd := exec.Command(helperPath, "-test.run", "^TestUnwritableTaskFilesHelper$", "-test.v")
	cmd.Env = append(os.Environ(),
		"NOEMA_UNWRITABLE_HELPER=1",
		"NOEMA_UNWRITABLE_ROOT="+path,
		"NOEMA_UNWRITABLE_TASK="+taskID,
		"NOEMA_UNWRITABLE_TASK_NAME="+taskName,
	)
	cmd.SysProcAttr = &syscall.SysProcAttr{Credential: &syscall.Credential{Uid: uint32(uid), Gid: uint32(gid)}}
	if output, err := cmd.CombinedOutput(); err != nil {
		if errors.Is(err, os.ErrPermission) {
			t.Skipf("sandbox does not permit an unprivileged helper executable: %v", err)
		}
		t.Fatalf("unprivileged helper: %v\n%s", err, output)
	}
}

func TestUnwritableTaskFilesHelper(t *testing.T) {
	if os.Getenv("NOEMA_UNWRITABLE_HELPER") != "1" {
		return
	}
	root, err := os.OpenRoot(os.Getenv("NOEMA_UNWRITABLE_ROOT"))
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()
	taskID := os.Getenv("NOEMA_UNWRITABLE_TASK")
	if err := WriteTaskFile(root, taskID, "notes/progress.md", "replaced\n"); err == nil {
		t.Fatal("write succeeded in an unwritable home")
	}
	content, err := ReadTaskFile(root, taskID, "notes/progress.md")
	if err != nil || content != "committed\n" {
		t.Fatalf("existing content = %q, %v", content, err)
	}
}
