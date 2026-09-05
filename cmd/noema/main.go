// Command noema runs the Go Noema server.
package main

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"io"
	"net"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/acp"
	noemaadapter "github.com/kpsuperplane/noema/internal/adapter"
	"github.com/kpsuperplane/noema/internal/artifact"
	"github.com/kpsuperplane/noema/internal/auth"
	"github.com/kpsuperplane/noema/internal/diagnostics"
	noemagraphql "github.com/kpsuperplane/noema/internal/graphql"
	"github.com/kpsuperplane/noema/internal/home"
	"github.com/kpsuperplane/noema/internal/localmodel"
	noemamcp "github.com/kpsuperplane/noema/internal/mcp"
	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/notification"
	"github.com/kpsuperplane/noema/internal/provider"
	noemaruntime "github.com/kpsuperplane/noema/internal/runtime"
	"github.com/kpsuperplane/noema/internal/store"
	"github.com/kpsuperplane/noema/internal/web"
	"github.com/kpsuperplane/noema/internal/webtool"
)

func main() {
	if handled, status := acp.RunTaskMCPIfRequested(os.Args, os.Stdin, os.Stdout); handled {
		os.Exit(status)
	}
	if handled, status := noemaruntime.RunFileParseWorkerIfRequested(); handled {
		os.Exit(status)
	}
	listen := flag.String("listen", "", "override the configured web bind address")
	desktopSidecar := flag.Bool("desktop-sidecar", false, "run as the packaged desktop child")
	flag.Parse()

	if err := releaseRootError(*desktopSidecar); err != nil {
		fmt.Fprintf(os.Stderr, "Noema server failed: %v\n", err)
		os.Exit(1)
	}

	ctx, stop := signal.NotifyContext(context.Background(), shutdownSignals()...)
	defer stop()
	var desktop *desktopOptions
	if *desktopSidecar {
		var reader *bufio.Reader
		desktop, reader = readDesktopOptions(os.Stdin)
		if desktop == nil {
			fmt.Fprintln(os.Stderr, "Noema Go server failed: invalid desktop startup input")
			os.Exit(2)
		}
		var cancel context.CancelFunc
		ctx, cancel = context.WithCancel(ctx)
		defer cancel()
		go func() {
			_, _ = io.Copy(io.Discard, reader)
			cancel()
		}()
	}
	if err := run(ctx, *listen, os.Stdout, desktop); err != nil {
		fmt.Fprintf(os.Stderr, "Noema Go server failed: %v\n", err)
		os.Exit(1)
	}
}

type desktopOptions struct {
	Token       string `json:"token"`
	RuntimeRoot string `json:"runtimeRoot"`
}

func readDesktopOptions(input io.Reader) (*desktopOptions, *bufio.Reader) {
	reader := bufio.NewReaderSize(input, 8*1024)
	line, err := reader.ReadSlice('\n')
	if err != nil || len(line) > 8*1024 {
		return nil, reader
	}
	var value desktopOptions
	decoder := json.NewDecoder(strings.NewReader(string(line)))
	decoder.DisallowUnknownFields()
	if decoder.Decode(&value) != nil || decoder.Decode(&struct{}{}) != io.EOF || value.Token == "" || value.RuntimeRoot != "" && !filepath.IsAbs(value.RuntimeRoot) {
		return nil, reader
	}
	return &value, reader
}

func run(ctx context.Context, address string, output io.Writer, desktop *desktopOptions) error {
	paths, err := home.Resolve()
	if err != nil {
		return err
	}
	root, err := paths.Open()
	if err != nil {
		return err
	}
	defer root.Close()
	errorLog, _ := diagnostics.Open(paths.ErrorsLog())
	if errorLog != nil {
		defer errorLog.Close()
	}
	var listener net.Listener
	if desktop != nil {
		listener, err = net.Listen("tcp4", "127.0.0.1:0")
		if err != nil {
			return fmt.Errorf("listen for desktop HTTP: %w", err)
		}
		defer listener.Close()
		address = listener.Addr().String()
	}

	taskStore, err := store.Open(ctx, paths.Database())
	if err != nil {
		return err
	}
	defer taskStore.Close()
	adapterService, err := noemaadapter.NewService(root, taskStore)
	if err != nil {
		return fmt.Errorf("open adapter service: %w", err)
	}
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
	if err := home.ReconcileTaskDocumentStages(root, 4096, func(taskID, digest string) (string, bool, error) {
		return taskStore.TaskDocumentReceipt(ctx, taskID, digest)
	}); err != nil {
		return fmt.Errorf("recover Task document updates: %w", err)
	}
	if err := recoverProjectDocuments(ctx, root, taskStore); err != nil {
		return err
	}
	if err := recoverRecurrenceDocuments(ctx, root, taskStore); err != nil {
		return err
	}
	artifacts, err := artifact.New(root, taskStore, errorLog)
	if err != nil {
		return fmt.Errorf("open Artifact service: %w", err)
	}
	scheduleOutput := output
	if desktop != nil {
		scheduleOutput = os.Stderr
	}
	runTaskSchedules(ctx, root, taskStore, scheduleOutput)
	var authConfig auth.Config
	var recovery *auth.Recovery
	authConfig, recovery, err = auth.LoadConfig(paths, address)
	if desktop != nil && err == nil {
		authConfig.Authority = address
		authConfig.Origin = "http://" + address
		authConfig.RPID = "localhost"
		authConfig.Secure = false
		authConfig.DevNoAuth = false
	}
	if err != nil {
		return err
	}
	if err := adapterService.SetOAuthCallback(authConfig.Origin + "/adapter/oauth/callback"); err != nil {
		return err
	}
	browserAuth, err := auth.New(paths, taskStore, authConfig, recovery)
	if err != nil {
		return err
	}
	go browserAuth.RunCleanup(ctx)
	notifications, err := notification.New(paths, taskStore, authConfig.Origin)
	if err != nil {
		return fmt.Errorf("open notification service: %w", err)
	}
	providerAccounts, err := provider.NewAccountService(paths.Root(), taskStore)
	if err != nil {
		return err
	}
	stdioEnabled, err := noemamcp.StdioEnabled(paths)
	if err != nil {
		return err
	}
	mcpService, err := noemamcp.NewService(paths, taskStore, stdioEnabled, errorLog, authConfig.Origin+"/mcp/oauth/callback")
	if err != nil {
		return fmt.Errorf("open MCP service: %w", err)
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
	openAIGenerator, err := provider.NewOpenAIGenerator(providerAccounts)
	if err != nil {
		return err
	}
	foundationGenerator, err := provider.NewFoundationGenerator(providerAccounts)
	if err != nil {
		return err
	}
	if _, err := foundationGenerator.RefreshAccount(ctx, time.Now()); err != nil {
		return err
	}
	runtimeRoot := ""
	if desktop != nil {
		runtimeRoot = desktop.RuntimeRoot
	}
	localModels, err := localmodel.New(taskStore, paths.Root(), runtimeRoot)
	if err != nil {
		return err
	}
	defer localModels.Close()
	go func() {
		if _, err := localModels.Retry(ctx); err != nil && ctx.Err() == nil {
			_ = errorLog.Write("local_model.retry_failed", diagnostics.Text("detail", err.Error()))
		}
	}()
	webTools, err := webtool.New(taskStore, providerAccounts, map[string]provider.Generator{
		"openrouter": openRouterGenerator, "codex": codexGenerator, "openai": openAIGenerator,
		"foundation_local": foundationGenerator, "local_models": localModels,
	}, artifacts, paths.Root(), os.Getenv("NOEMA_OBSCURA_WORKER_PATH"), authConfig.BrowserMaxSessions, authConfig.BrowserMaxOldSpaceMB)
	if err != nil {
		return err
	}
	defer webTools.Close()
	chatRuntime, err := noemaruntime.NewChat(
		taskStore, openRouterGenerator, codexGenerator, openAIGenerator, root, nativeMemory,
		mcpService, adapterService, foundationGenerator, localModels, webTools, errorLog,
	)
	if err != nil {
		return err
	}
	mcpService.SetToolClassifier(chatRuntime.MCPToolClassifier())
	defer chatRuntime.Close()
	defer mcpService.Close()
	taskExecution, err := noemaruntime.NewTaskExecution(
		ctx, taskStore, openRouterGenerator, codexGenerator, openAIGenerator, root,
		mcpService, adapterService, artifacts, foundationGenerator, localModels, webTools, errorLog,
	)
	if err != nil {
		return fmt.Errorf("start Task execution: %w", err)
	}
	defer taskExecution.Close()
	mcpService.SetOAuthCompletionHandler(func(attemptID string) {
		handled, _ := taskExecution.ResumeMCPAuthentication(context.Background(), attemptID)
		if !handled {
			_ = chatRuntime.ResumeMCPAuthentication(context.Background(), attemptID)
		}
	})
	adapterService.SetOAuthCompletionHandler(func(event noemaadapter.OAuthAttemptEvent) {
		ctx := context.Background()
		requests, err := taskStore.AdapterAuthRequestsForAttempt(ctx, event.AttemptID)
		if err != nil {
			return
		}
		if event.Status == "superseded" {
			for _, request := range requests {
				if request.TaskID != "" {
					_ = taskExecution.SupersedeAdapterAuthentication(ctx, request)
				} else {
					_ = chatRuntime.SupersedeAdapterAuthentication(ctx, request.ID, request.Revision)
				}
			}
			return
		}
		if event.Status != "completed" {
			for _, request := range requests {
				_ = taskStore.RetryAdapterOAuthAuthentication(ctx, request, "oauth_attempt_"+event.Status, time.Now())
				if request.ConversationID != "" {
					chatRuntime.NotifyHumanInterventionsChanged(request.ConversationID)
				}
			}
			return
		}
		if taskStore.CompleteAdapterOAuthAuthentication(ctx, event.AttemptID, time.Now()) != nil {
			return
		}
		connections, err := adapterService.ConnectionIDsForGrant(event.GrantID)
		if err != nil {
			return
		}
		for _, connectionID := range connections {
			_, _ = taskExecution.ResumeAdapterAuthentication(ctx, connectionID)
			_ = chatRuntime.ResumeAdapterAuthentication(ctx, connectionID)
		}
	})
	go notifications.Run(ctx, chatRuntime.SubscribeAll(ctx))

	if listener == nil {
		listener, err = net.Listen("tcp", authConfig.ListenAddress)
		if err != nil {
			return fmt.Errorf("listen for HTTP: %w", err)
		}
		defer listener.Close()
	}
	resolver := noemagraphql.NewResolver(
		taskStore, root, browserAuth, providerAccounts, openRouter, chatRuntime, codex,
		artifacts, nativeMemory, notifications,
		mcpService,
	)
	resolver.TaskExecution = taskExecution
	resolver.SetFoundation(foundationGenerator)
	resolver.SetLocalModels(localModels)
	resolver.SetAdapters(adapterService)
	resolver.WebTools = webTools
	graphqlHandler := noemagraphql.NewHandler(resolver)
	webGraphQL := web.NewGraphQLHandler(graphqlHandler, noemagraphql.Schema(), authConfig.GraphiQL)
	mux := http.NewServeMux()
	mux.Handle("/graphql", webGraphQL)
	mux.Handle("/graphql/ws", webGraphQL)
	mux.Handle("/graphql/schema.graphql", webGraphQL)
	mux.Handle("/provider/oauth/callback/", openRouter.CallbackHandler())
	mux.Handle("/mcp/oauth/callback", mcpService.CallbackHandler())
	mux.Handle("/adapter/oauth/callback", adapterService.OAuthCallbackHandler())
	mux.Handle("/artifacts/versions/", artifacts.Handler())
	mux.Handle("GET /favicons/{hostname}", web.NewFaviconHandler())
	mux.Handle("/", web.NewAssetHandler())
	server := &http.Server{
		Handler:           browserAuth.Handler(mux),
		ReadHeaderTimeout: 10 * time.Second,
		ReadTimeout:       30 * time.Second,
	}
	if desktop != nil {
		server.Handler = desktopHandler(desktop.Token, mux, server.Handler)
	}
	serverContext, stopServers := context.WithCancel(ctx)
	defer stopServers()
	var localResult <-chan error
	if authConfig.LocalGraphQLSocket {
		local, err := web.NewLocalGraphQLServer(filepath.Join(paths.Root(), "run", "graphql.sock"), graphqlHandler)
		if err != nil {
			return err
		}
		result := make(chan error, 1)
		localResult = result
		go func() { result <- local.Serve(serverContext) }()
	}

	serveResult := make(chan error, 1)
	go func() {
		serveResult <- server.Serve(listener)
	}()
	if desktop == nil {
		fmt.Fprintf(output, "Noema listening on %s\n", listener.Addr())
	} else if err := json.NewEncoder(output).Encode(map[string]string{"type": "ready", "origin": authConfig.Origin}); err != nil {
		return fmt.Errorf("write desktop ready line: %w", err)
	}

	select {
	case <-ctx.Done():
		stopServers()
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		shutdownErr := server.Shutdown(shutdownCtx)
		if localResult != nil {
			if err := <-localResult; err != nil {
				return fmt.Errorf("stop local GraphQL: %w", err)
			}
		}
		if shutdownErr != nil {
			return fmt.Errorf("stop HTTP server: %w", shutdownErr)
		}
		return nil
	case err := <-serveResult:
		stopServers()
		if localResult != nil {
			_ = <-localResult
		}
		if errors.Is(err, http.ErrServerClosed) {
			return nil
		}
		return fmt.Errorf("serve HTTP: %w", err)
	case err := <-localResult:
		shutdownCtx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
		defer cancel()
		_ = server.Shutdown(shutdownCtx)
		if err == nil {
			return errors.New("local GraphQL server stopped")
		}
		return fmt.Errorf("serve local GraphQL: %w", err)
	}
}

func desktopHandler(token string, application, fallback http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		callbackPath := strings.HasPrefix(r.URL.Path, "/provider/oauth/callback/") ||
			r.URL.Path == "/mcp/oauth/callback" || r.URL.Path == "/adapter/oauth/callback"
		if callbackPath {
			application.ServeHTTP(w, r)
			return
		}
		candidate, ok := strings.CutPrefix(r.Header.Get("Authorization"), "Bearer ")
		desktopPath := r.URL.Path == "/graphql" || r.URL.Path == "/graphql/ws" ||
			strings.HasPrefix(r.URL.Path, "/artifacts/versions/") || strings.HasPrefix(r.URL.Path, "/favicons/")
		if ok && desktopPath && candidate == token {
			application.ServeHTTP(w, r.WithContext(auth.WithDesktopAccess(r.Context())))
			return
		}
		fallback.ServeHTTP(w, r)
	})
}

func recoverRecurrenceDocuments(ctx context.Context, root *os.Root, database *store.Store) error {
	stages, err := home.RecurrenceDocumentStages(root)
	if err != nil {
		return fmt.Errorf("list staged recurrence documents: %w", err)
	}
	for _, stage := range stages {
		result, found, err := database.TaskRecurrenceReceiptResult(ctx, stage.RecurrenceID, stage.RequestDigest)
		if err != nil {
			return fmt.Errorf("inspect staged recurrence %s: %w", stage.RecurrenceID, err)
		}
		if !found {
			if err := home.DiscardRecurrenceDocumentStage(root, stage.RequestDigest); err != nil {
				return fmt.Errorf("discard uncommitted recurrence %s: %w", stage.RecurrenceID, err)
			}
			continue
		}
		if result.RecurrenceID != stage.RecurrenceID {
			return fmt.Errorf("staged recurrence %s does not match its command receipt", stage.RecurrenceID)
		}
		if _, err := home.CommitRecurrenceDocumentStage(root, stage); err != nil {
			return fmt.Errorf("recover recurrence %s: %w", stage.RecurrenceID, err)
		}
	}
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

func runTaskSchedules(ctx context.Context, root *os.Root, database *store.Store, output io.Writer) {
	go func() { _ = taskScheduleLoop(ctx, root, database, output) }()
}

func taskScheduleLoop(ctx context.Context, root *os.Root, database *store.Store, output io.Writer) error {
	wake := database.SubscribeWork(ctx)
	recovering := true
	retryDelay := 100 * time.Millisecond
	needsRecovery := false
	for {
		unlockSchedules := database.LockTaskSchedules()
		var err error
		if needsRecovery {
			err = home.RecoverTaskDocuments(root, func(taskID string) (bool, error) {
				return database.TaskExists(ctx, taskID)
			})
			if err == nil {
				err = recoverRecurrenceDocuments(ctx, root, database)
			}
		}
		var created []store.DueTask
		var changed bool
		if err == nil {
			created, changed, err = database.ProcessDueTaskSchedules(ctx, time.Now(), recovering, func(value store.DueTask) error {
				return home.StageRecurrenceDocumentToTask(root, value.RecurrenceID, value.TaskID)
			})
		}
		if err == nil {
			for _, value := range created {
				if err = home.CommitTaskDocument(root, value.TaskID); err != nil {
					break
				}
			}
		}
		if err == nil && changed {
			database.NotifyWork()
		}
		var deadline *time.Time
		if err == nil {
			deadline, err = database.NextTaskScheduleDeadline(ctx)
		}
		unlockSchedules()
		if err != nil {
			needsRecovery = true
			fmt.Fprintf(output, "Task schedule processing failed; retrying in %s: %v\n", retryDelay, err)
			timer := time.NewTimer(retryDelay)
			select {
			case <-ctx.Done():
				if !timer.Stop() {
					select {
					case <-timer.C:
					default:
					}
				}
				return nil
			case <-timer.C:
			}
			if retryDelay < 5*time.Second {
				retryDelay *= 2
				if retryDelay > 5*time.Second {
					retryDelay = 5 * time.Second
				}
			}
			continue
		}
		needsRecovery = false
		retryDelay = 100 * time.Millisecond
		recovering = false
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
