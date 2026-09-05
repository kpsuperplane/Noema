package store

import (
	"context"
	"fmt"
	"path/filepath"
	"sync"
	"testing"
	"time"
)

func TestWALConcurrentWritesSurviveColdReopen(t *testing.T) {
	path := filepath.Join(t.TempDir(), "noema.sqlite3")
	database, err := Open(t.Context(), path)
	if err != nil {
		t.Fatal(err)
	}
	const writers = 16
	errors := make(chan error, writers)
	var wait sync.WaitGroup
	for index := range writers {
		wait.Add(1)
		go func() {
			defer wait.Done()
			id := fmt.Sprintf("task:%032x", index+1)
			_, err := database.CreateTask(context.Background(), id, "Concurrent write", "correlation:"+id, time.Now())
			errors <- err
		}()
	}
	wait.Wait()
	close(errors)
	for err := range errors {
		if err != nil {
			t.Fatal(err)
		}
	}
	if err := database.Close(); err != nil {
		t.Fatal(err)
	}

	reopened, err := Open(t.Context(), path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.Close()
	var integrity string
	if err := reopened.db.QueryRowContext(t.Context(), "PRAGMA integrity_check").Scan(&integrity); err != nil || integrity != "ok" {
		t.Fatalf("cold-reopen integrity = %q, %v", integrity, err)
	}
	var count int
	if err := reopened.db.QueryRowContext(t.Context(), "SELECT count(*) FROM tasks").Scan(&count); err != nil || count != writers {
		t.Fatalf("task count = %d, %v", count, err)
	}
}
