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

	"github.com/kpsuperplane/noema/internal/auth"
	noemagraphql "github.com/kpsuperplane/noema/internal/graphql"
	"github.com/kpsuperplane/noema/internal/home"
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
	if err := home.RecoverTaskDocuments(root, func(taskID string) (bool, error) {
		return taskStore.TaskExists(ctx, taskID)
	}); err != nil {
		return fmt.Errorf("recover Task documents: %w", err)
	}
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
	generator, err := provider.NewOpenRouterGenerator(providerAccounts)
	if err != nil {
		return err
	}
	chatRuntime, err := noemaruntime.NewChat(taskStore, generator)
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
	))
	mux := http.NewServeMux()
	mux.Handle("/graphql", graphqlHandler)
	mux.Handle("/graphql/ws", graphqlHandler)
	mux.Handle("/provider/oauth/callback/", openRouter.CallbackHandler())
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
	}
}

func requireLoopback(address net.Addr) error {
	tcpAddress, ok := address.(*net.TCPAddr)
	if !ok || !tcpAddress.IP.IsLoopback() {
		return errors.New("migration server must listen on loopback")
	}
	return nil
}
