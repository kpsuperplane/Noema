// Command noema runs the Go Noema server.
package main

import (
	"context"
	"errors"
	"flag"
	"fmt"
	"net"
	"net/http"
	"os"
	"os/signal"
	"time"

	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/auth"
	noemagraphql "github.com/kpsuperplane/noema/internal/graphql"
	"github.com/kpsuperplane/noema/internal/home"
	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/provider"
	noemaruntime "github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/web"
)

func main() {
	listen := flag.String("listen", "127.0.0.1:3737", "loopback address for the migration server")
	migrationSpike := flag.Bool("migration-spike", false, "allow the incomplete migration server to start")
	flag.Parse()

	if !*migrationSpike {
		fmt.Fprintln(os.Stderr, "the Go server is incomplete; pass -migration-spike for development")
		os.Exit(2)
	}

	ctx, stop := signal.NotifyContext(context.Background(), shutdownSignals()...)
	defer stop()
	if err := run(ctx, *listen, os.Stdout); err != nil {
		fmt.Fprintf(os.Stderr, "Noema Go server failed: %v\n", err)
		os.Exit(1)
	}
}

func run(ctx context.Context, address string, output *os.File) error {
	paths, err := home.Resolve()
	if err != nil {
		return err
	}
	root, err := paths.Open()
	if err != nil {
		return err
	}
	defer root.Close()

	taskStore, err := store.Open(ctx, paths.Database())
	if err != nil {
		return err
	}
	defer taskStore.Close()
	nativeMemory, err := noemamemory.New(root)
	if err != nil {
		return err
	}
	defer nativeMemory.Close()
	if err := home.RecoverTaskDocuments(root, func(taskID string) (bool, error) {
		return taskStore.TaskExists(ctx, taskID)
	}); err != nil {
		return fmt.Errorf("recover Task documents: %w", err)
	}
	if err := recoverProjectDocuments(ctx, root, taskStore); err != nil {
		return err
	}
	if err := recoverRecurrenceDocuments(ctx, root, taskStore); err != nil {
		return err
	}
	artifacts, err := artifact.New(root, taskStore)
	if err != nil {
		return fmt.Errorf("open Artifact service: %w", err)
	}
	scheduleErrors := runTaskSchedules(ctx, root, taskStore)
	authConfig, recovery, err := auth.LoadConfig(paths, address)
	if err != nil {
		return err
	}
	browserAuth, err := auth.New(paths, taskStore, authConfig, recovery)
	if err != nil {
		return err
	}
	go browserAuth.RunCleanup(ctx)
	providerAccounts, err := provider.NewAccountService(paths.Root(), taskStore)
	if err != nil {
		return err
	}
	if err := providerAccounts.Initialize(ctx, time.Now()); err != nil {
		return fmt.Errorf("initialize provider accounts: %w", err)
	}
	openRouter, err := provider.NewOpenRouterService(
		providerAccounts,
		authConfig.Origin+"/provider/oauth/callback",
	)
	if err != nil {
		return err
	}
	codex, err := provider.NewCodexService(providerAccounts)
	if err != nil {
		return err
	}
	defer codex.Close()
	openRouterGenerator, err := provider.NewOpenRouterGenerator(providerAccounts)
	if err != nil {
		return err
	}
	codexGenerator, err := provider.NewCodexGenerator(providerAccounts)
	if err != nil {
		return err
	}
	chatRuntime, err := noemaruntime.NewChat(
		taskStore, openRouterGenerator, codexGenerator, root,
	)
	if err != nil {
		return err
	}
	defer chatRuntime.Close()

	listener, err := net.Listen("tcp", address)
	if err != nil {
		return fmt.Errorf("listen for HTTP: %w", err)
	}
	defer listener.Close()
	if err := requireLoopback(listener.Addr()); err != nil {
		return err
	}

	graphqlHandler := noemagraphql.NewHandler(noemagraphql.NewResolver(
		taskStore, root, browserAuth, providerAccounts, openRouter, chatRuntime, codex,
		artifacts, nativeMemory,
	))
	mux := http.NewServeMux()
	mux.Handle("/graphql", graphqlHandler)
	mux.Handle("/graphql/ws", graphqlHandler)
	mux.Handle("/provider/oauth/callback/", openRouter.CallbackHandler())
	mux.Handle("/artifacts/versions/", artifacts.Handler())
	mux.Handle("/", web.NewAssetHandler())
	server := &http.Server{
		Handler:           browserAuth.Handler(mux),
		ReadHeaderTimeout: 10 * time.Second,
		ReadTimeout:       30 * time.Second,
	}

	serveResult := make(chan error, 1)
	go func() {
		serveResult <- server.Serve(listener)
	}()
	fmt.Fprintf(output, "Noema Go migration slice listening on %s\n", listener.Addr())

	select {
	case <-ctx.Done():
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		if err := server.Shutdown(shutdownCtx); err != nil {
			return fmt.Errorf("stop HTTP server: %w", err)
		}
		return nil
	case err := <-serveResult:
		if errors.Is(err, http.ErrServerClosed) {
			return nil
		}
		return fmt.Errorf("serve HTTP: %w", err)
	case err := <-scheduleErrors:
		return fmt.Errorf("process Task schedules: %w", err)
	}
}

func recoverRecurrenceDocuments(ctx context.Context, root *os.Root, database *store.Store) error {
	values, err := database.RecurrenceTaskDocuments(ctx)
	if err != nil {
		return fmt.Errorf("list recurrence Task documents: %w", err)
	}
	for _, value := range values {
		if _, err := home.ReadRecurrenceDocument(root, value.RecurrenceID); errors.Is(err, os.ErrNotExist) {
			document, readErr := home.ReadTaskDocument(root, value.TaskID)
			if readErr != nil {
				return fmt.Errorf("read recurrence source Task %s: %w", value.TaskID, readErr)
			}
			if _, err := home.EnsureRecurrenceDocument(root, value.RecurrenceID, document.Content); err != nil {
				return fmt.Errorf("recover recurrence %s: %w", value.RecurrenceID, err)
			}
		} else if err != nil {
			return fmt.Errorf("read recurrence %s: %w", value.RecurrenceID, err)
		}
		if err := home.CopyRecurrenceDocumentToTask(root, value.RecurrenceID, value.TaskID); err != nil {
			return fmt.Errorf("recover recurrence Task %s: %w", value.TaskID, err)
		}
	}
	return nil
}

func runTaskSchedules(ctx context.Context, root *os.Root, database *store.Store) <-chan error {
	result := make(chan error, 1)
	go func() {
		if err := taskScheduleLoop(ctx, root, database); err != nil {
			result <- err
		}
	}()
	return result
}

func taskScheduleLoop(ctx context.Context, root *os.Root, database *store.Store) error {
	wake := database.SubscribeWork(ctx)
	recovering := true
	for {
		_, err := database.ProcessDueTaskSchedules(ctx, time.Now(), recovering, func(value store.DueTask) error {
			return home.CopyRecurrenceDocumentToTask(root, value.RecurrenceID, value.TaskID)
		})
		if err != nil {
			return err
		}
		recovering = false
		deadline, err := database.NextTaskScheduleDeadline(ctx)
		if err != nil {
			return err
		}
		if deadline == nil {
			select {
			case <-ctx.Done():
				return nil
			case <-wake:
				continue
			}
		}
		delay := time.Until(*deadline)
		if delay <= 0 {
			continue
		}
		timer := time.NewTimer(delay)
		select {
		case <-ctx.Done():
			if !timer.Stop() {
				<-timer.C
			}
			return nil
		case <-wake:
			if !timer.Stop() {
				<-timer.C
			}
		case <-timer.C:
		}
	}
}

func recoverProjectDocuments(ctx context.Context, root *os.Root, database *store.Store) error {
	stages, err := home.ProjectDocumentStages(root)
	if err != nil {
		return fmt.Errorf("list staged Project documents: %w", err)
	}
	for _, stage := range stages {
		result, found, err := database.ProjectReceiptResult(ctx, stage.ProjectID, stage.RequestDigest)
		if err != nil {
			return fmt.Errorf("inspect staged Project %s: %w", stage.ProjectID, err)
		}
		if !found {
			if err := home.DiscardProjectDocumentStage(root, stage.ProjectID, stage.RequestDigest); err != nil {
				return fmt.Errorf("discard uncommitted Project %s: %w", stage.ProjectID, err)
			}
			continue
		}
		if result.Project.ID != stage.ProjectID || result.DocumentDigest != stage.Document.Digest {
			return fmt.Errorf("staged Project %s does not match its command receipt", stage.ProjectID)
		}
		if _, _, err := home.CommitProjectDocumentStage(root, stage, result.Project.Folder); err != nil {
			return fmt.Errorf("recover Project %s: %w", stage.ProjectID, err)
		}
	}
	return nil
}

func requireLoopback(address net.Addr) error {
	tcpAddress, ok := address.(*net.TCPAddr)
	if !ok || !tcpAddress.IP.IsLoopback() {
		return errors.New("migration server must listen on loopback")
	}
	return nil
}
