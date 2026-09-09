package localmodel

import (
	"archive/tar"
	"archive/zip"
	"bytes"
	"compress/gzip"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"runtime"
	"slices"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestLocalModelTransferVerifiesGGUFAndResumes(t *testing.T) {
	service, database, home := newTestService(t)
	model := append([]byte("GGUF"), bytes.Repeat([]byte{7}, 128)...)
	sum := sha256.Sum256(model)
	digest := hex.EncodeToString(sum[:])

	localPath := filepath.Join(home, "source.gguf")
	if err := os.WriteFile(localPath, model, 0o600); err != nil {
		t.Fatal(err)
	}
	queued, err := service.Import(t.Context(), ImportInput{
		Name: "Imported model", SourceKind: "local_file", LocalPath: localPath,
	})
	if err != nil {
		t.Fatal(err)
	}
	installed := waitForInstallation(t, database, queued.ID)
	if installed.SHA256 != digest || installed.DiskBytes != int64(len(model)) {
		t.Fatalf("installed local model = %#v", installed)
	}
	if _, err := os.Stat(filepath.Join(home, filepath.FromSlash(installed.BlobPath))); err != nil {
		t.Fatalf("verified blob is unavailable: %v", err)
	}

	resumed := store.LocalModelInstallation{
		ID: "installation:resumed", ModelID: "resumed", Name: "Resumed",
		File: "resumed.gguf", SourceKind: "public_gguf", Repo: "owner/repo",
		Revision: strings.Repeat("a", 40), SHA256: digest, Backend: "cpu",
		CreatedAt: time.Now(),
	}
	resumed, err = database.QueueLocalModel(t.Context(), resumed)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(service.partialPath(resumed.ID), model[:17], 0o600); err != nil {
		t.Fatal(err)
	}
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		if request.Header.Get("Range") != "bytes=17-" {
			t.Errorf("range = %q", request.Header.Get("Range"))
		}
		writer.WriteHeader(http.StatusPartialContent)
		_, _ = writer.Write(model[17:])
	}))
	defer server.Close()
	if err := service.download(t.Context(), resumed, server.URL, digest, false); err != nil {
		t.Fatal(err)
	}
	resumed, err = database.LocalModelInstallation(t.Context(), resumed.ID)
	if err != nil || resumed.Status != "installed" || resumed.SHA256 != digest {
		t.Fatalf("resumed installation = %#v, %v", resumed, err)
	}

	corrupt := store.LocalModelInstallation{
		ID: "installation:corrupt", ModelID: "corrupt", Name: "Corrupt", File: "corrupt.gguf",
		SourceKind: "local_file", Backend: "cpu", TotalBytes: int64(len(model)), CreatedAt: time.Now(),
	}
	corrupt, err = database.QueueLocalModel(t.Context(), corrupt)
	if err != nil {
		t.Fatal(err)
	}
	corruptPartial := service.partialPath(corrupt.ID)
	if err = os.WriteFile(corruptPartial, model, 0o600); err != nil {
		t.Fatal(err)
	}
	if err = service.verifyAndPublish(t.Context(), corrupt, corruptPartial, strings.Repeat("0", 64), int64(len(model)), int64(len(model)), false); !errors.Is(err, errLocalModelChecksum) {
		t.Fatalf("corrupt model verification = %v", err)
	}
	if _, err = os.Stat(corruptPartial); !os.IsNotExist(err) {
		t.Fatalf("corrupt partial remains: %v", err)
	}
	if err = ensureFreeSpace(home, int64(^uint64(0)>>1)); err == nil {
		t.Fatal("impossible model size passed the free-space check")
	}

	removable := store.LocalModelInstallation{
		ID: "installation:remove", ModelID: "remove", Name: "Remove", File: "remove.gguf",
		SourceKind: "local_file", Backend: "cpu", CreatedAt: time.Now(),
	}
	removable, err = database.QueueLocalModel(t.Context(), removable)
	if err != nil {
		t.Fatal(err)
	}
	jobStarted := make(chan struct{})
	service.startJob(removable, func(ctx context.Context) error {
		close(jobStarted)
		<-ctx.Done()
		return ctx.Err()
	})
	<-jobStarted
	if err = os.WriteFile(service.partialPath(removable.ID), model, 0o600); err != nil {
		t.Fatal(err)
	}
	if removed, removeErr := service.Remove(t.Context(), removable.ID); removeErr != nil || !removed {
		t.Fatalf("remove active transfer = %v, %v", removed, removeErr)
	}
	if _, err = os.Stat(service.partialPath(removable.ID)); !os.IsNotExist(err) {
		t.Fatalf("removed transfer partial remains: %v", err)
	}

	live := make(chan Event, 1)
	service.subscribers[99] = live
	service.emit(Event{Kind: "old"})
	service.emit(Event{Kind: "new"})
	if event := <-live; event.Kind != "new" {
		t.Fatalf("buffer retained %q event", event.Kind)
	}
	service.unsubscribe(99)
}

func TestRuntimeAssetsKeepRequiredAliasesAndUsePinnedRequest(t *testing.T) {
	root := t.TempDir()
	archivePath := filepath.Join(root, "runtime.tar.gz")
	archive, err := os.Create(archivePath)
	if err != nil {
		t.Fatal(err)
	}
	gzipWriter := gzip.NewWriter(archive)
	tarWriter := tar.NewWriter(gzipWriter)
	entries := []tar.Header{
		{Name: "llama-b10015/libggml.so", Typeflag: tar.TypeSymlink, Linkname: "libggml.so.0"},
		{Name: "llama-b10015/libggml.so.0", Typeflag: tar.TypeSymlink, Linkname: "libggml.so.0.9.0"},
		{Name: "llama-b10015/libggml.so.0.9.0", Typeflag: tar.TypeReg, Size: 7, Mode: 0o600},
		{Name: "llama-b10015/llama-server", Typeflag: tar.TypeReg, Size: 6, Mode: 0o700},
		{Name: "llama-b10015/examples/ignored", Typeflag: tar.TypeReg, Size: 7, Mode: 0o600},
	}
	for _, entry := range entries {
		if err := tarWriter.WriteHeader(&entry); err != nil {
			t.Fatal(err)
		}
		if entry.Typeflag == tar.TypeReg {
			_, _ = tarWriter.Write(bytes.Repeat([]byte{'x'}, int(entry.Size)))
		}
	}
	_ = tarWriter.Close()
	_ = gzipWriter.Close()
	_ = archive.Close()
	destination := filepath.Join(root, "tar")
	if err := os.Mkdir(destination, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := extractRuntimeArchive(archivePath, destination); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"libggml.so", "libggml.so.0", "libggml.so.0.9.0", "llama-server"} {
		if info, err := os.Stat(filepath.Join(destination, name)); err != nil || !info.Mode().IsRegular() {
			t.Fatalf("retained runtime file %s = %v, %v", name, info, err)
		}
	}
	if _, err := os.Stat(filepath.Join(destination, "examples", "ignored")); !os.IsNotExist(err) {
		t.Fatalf("unneeded archive member was retained: %v", err)
	}

	zipPath := filepath.Join(root, "runtime.zip")
	zipFile, _ := os.Create(zipPath)
	zipWriter := zip.NewWriter(zipFile)
	serverEntry, _ := zipWriter.Create("llama-server.exe")
	_, _ = serverEntry.Write([]byte("server"))
	dllEntry, _ := zipWriter.Create("ggml.dll")
	_, _ = dllEntry.Write([]byte("library"))
	_ = zipWriter.Close()
	_ = zipFile.Close()
	zipDestination := filepath.Join(root, "zip")
	_ = os.Mkdir(zipDestination, 0o700)
	if err := extractRuntimeArchive(zipPath, zipDestination); err != nil {
		t.Fatal(err)
	}

	service, _, home := newTestService(t)
	if service.client.Timeout != 0 {
		t.Fatalf("whole-download timeout = %v", service.client.Timeout)
	}
	assetBody := []byte("pinned asset")
	assetSum := sha256.Sum256(assetBody)
	var accepted atomic.Bool
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		accepted.Store(request.Header.Get("Accept") == "application/octet-stream")
		_, _ = writer.Write(assetBody)
	}))
	defer server.Close()
	asset := runtimeAsset{
		URL: server.URL, Size: int64(len(assetBody)), SHA256: hex.EncodeToString(assetSum[:]),
	}
	if err := service.downloadRuntimeAsset(t.Context(), asset, filepath.Join(home, "asset")); err != nil {
		t.Fatal(err)
	}
	if !accepted.Load() {
		t.Fatal("runtime asset request omitted the GitHub media header")
	}
	badAsset := asset
	badAsset.SHA256 = strings.Repeat("0", 64)
	badPath := filepath.Join(home, "bad-asset")
	if err := service.downloadRuntimeAsset(t.Context(), badAsset, badPath); err == nil {
		t.Fatal("corrupt runtime asset passed verification")
	}
	if _, err := os.Stat(badPath + ".partial"); !os.IsNotExist(err) {
		t.Fatalf("corrupt runtime partial remains: %v", err)
	}
	bundledBackend := "cpu"
	if runtime.GOOS == "darwin" {
		bundledBackend = "metal"
	}
	bundledName := "llama-server"
	if runtime.GOOS == "windows" {
		bundledName += ".exe"
	}
	bundledServer := filepath.Join(root, "packaged", bundledBackend, bundledName)
	if err := os.MkdirAll(filepath.Dir(bundledServer), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(bundledServer, []byte("packaged"), 0o700); err != nil {
		t.Fatal(err)
	}
	service.runtimeRoot = filepath.Join(root, "packaged")
	if path, err := service.ensureRuntimeAssets(t.Context(), "cpu"); err != nil || path != bundledServer {
		t.Fatalf("packaged runtime = %q, %v", path, err)
	}
	service.runtimeRoot = ""

	archiveBody, err := os.ReadFile(archivePath)
	if err != nil {
		t.Fatal(err)
	}
	archiveSum := sha256.Sum256(archiveBody)
	archiveServer := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, _ *http.Request) {
		_, _ = writer.Write(archiveBody)
	}))
	defer archiveServer.Close()
	previousAssets := runtimeAssets
	runtimeAssets = []runtimeAsset{{
		Target: runtime.GOOS + "/" + runtime.GOARCH, Backend: "cpu", Name: "replacement.tar.gz",
		URL: archiveServer.URL, SHA256: hex.EncodeToString(archiveSum[:]), Size: int64(len(archiveBody)),
	}}
	defer func() { runtimeAssets = previousAssets }()
	runtimeDirectory := filepath.Join(home, "system", "tools", "llama.cpp", runtimeManifest.ReleaseTag, runtime.GOOS+"/"+runtime.GOARCH, "cpu")
	if err = os.MkdirAll(runtimeDirectory, 0o700); err != nil {
		t.Fatal(err)
	}
	marker := filepath.Join(runtimeDirectory, "incomplete")
	if err = os.WriteFile(marker, []byte("old"), 0o600); err != nil {
		t.Fatal(err)
	}
	runtimePath, err := service.ensureRuntimeAssets(t.Context(), "cpu")
	if err != nil {
		t.Fatal(err)
	}
	if _, err = os.Stat(runtimePath); err != nil {
		t.Fatalf("published runtime is unavailable: %v", err)
	}
	if _, err = os.Stat(marker); !os.IsNotExist(err) {
		t.Fatalf("incomplete runtime destination remains: %v", err)
	}
	args := runtimeArguments("/model.gguf", "model", 3210, "vulkan", 2048)
	for _, pair := range [][]string{
		{"--host", "127.0.0.1"}, {"--ctx-size", "8192"}, {"--parallel", "1"},
		{"--cache-ram", "2048"}, {"--n-gpu-layers", "999"},
	} {
		if !containsArgumentPair(args, pair) {
			t.Fatalf("runtime arguments omit %v: %v", pair, args)
		}
	}
}

func TestPackagedRuntimePrecedesCuratedAssetSupport(t *testing.T) {
	root := t.TempDir()
	executable := "llama-server"
	if runtime.GOOS == "windows" {
		executable += ".exe"
	}
	previous := runtimeAssets
	runtimeAssets = nil
	t.Cleanup(func() { runtimeAssets = previous })

	service := &Service{runtimeRoot: root}
	for _, backend := range []string{"cpu", "vulkan"} {
		path := filepath.Join(root, backend, executable)
		if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte("packaged"), 0o700); err != nil {
			t.Fatal(err)
		}
		if actual, err := service.ensureRuntimeAssets(t.Context(), backend); err != nil || actual != path {
			t.Fatalf("packaged %s runtime = %q, %v", backend, actual, err)
		}
	}
}

func TestRuntimeProcessExcludesNoemaEnvironment(t *testing.T) {
	if len(os.Args) > 1 && os.Args[len(os.Args)-1] == "NOEMA_LOCALMODEL_ENV_HELPER" {
		fmt.Printf("%s:%s:%s", os.Getenv("NOEMA_HOME"), os.Getenv("NOEMA_OPENAI__API_KEY"),
			os.Getenv("NOEMA_TEST_ORDINARY"))
		os.Exit(0)
	}
	t.Setenv("NOEMA_HOME", "private-home")
	t.Setenv("NOEMA_OPENAI__API_KEY", "private-key")
	t.Setenv("NOEMA_TEST_ORDINARY", "ordinary-value")
	command := runtimeCommand(t.Context(), os.Args[0], "-test.run=^TestRuntimeProcessExcludesNoemaEnvironment$",
		"--", "NOEMA_LOCALMODEL_ENV_HELPER")
	output, err := command.Output()
	if err != nil || string(output) != "::ordinary-value" {
		t.Fatalf("llama-server environment = %q, %v", output, err)
	}
}

func TestLocalGenerationPreservesToolsReplayAndTokenization(t *testing.T) {
	service, database, _ := newTestService(t)
	installation := installActiveModel(t, database, "local-model")
	var streamedName string
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		switch request.URL.Path {
		case "/v1/chat/completions":
			var body map[string]any
			if err := json.NewDecoder(request.Body).Decode(&body); err != nil {
				t.Error(err)
			}
			encoded, _ := json.Marshal(body)
			if bytes.Contains(encoded, []byte("pattern")) || bytes.Contains(encoded, []byte("minLength")) {
				t.Errorf("llama schema constraints were not removed: %s", encoded)
			}
			messages := body["messages"].([]any)
			if len(messages) != 2 || messages[0].(map[string]any)["content"] != "system\n\ndeveloper" {
				t.Errorf("local messages = %#v", messages)
			}
			writer.Header().Set("Content-Type", "text/event-stream")
			_, _ = io.WriteString(writer, `data: {"choices":[{"delta":{"reasoning_content":"The work is complete."}}]}`+"\n\n")
			_, _ = io.WriteString(writer, "data: {\"id\":\"response\",\"model\":\"local-model\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call-1\",\"function\":{\"name\":\"task_finish_execution\",\"arguments\":\"{}\"}}]}}]}\n\n")
			_, _ = io.WriteString(writer, "data: [DONE]\n\n")
		case "/tokenize":
			writer.Header().Set("Content-Type", "application/json")
			_, _ = io.WriteString(writer, `{"tokens":[1,2,3,4]}`)
		default:
			http.NotFound(writer, request)
		}
	}))
	defer server.Close()
	service.runtime.status = RuntimeRunning
	service.runtime.endpoint = server.URL
	service.runtime.modelID = installation.ModelID

	schema := json.RawMessage(`{"type":"object","properties":{"summary":{"type":"string","pattern":"\\S","minLength":1}}}`)
	result, err := service.Generate(t.Context(), provider.GenerateRequest{
		AccountID: accountID, Model: installation.ModelID,
		Messages: []provider.GenerationMessage{
			{Role: "system", Content: "system"},
			{Role: "developer", Content: "developer"},
			{Role: "user", Content: "finish"},
		},
		Tools: []provider.GenerationTool{{
			Name: "task.finish_execution", Description: "Finish the Task.", InputSchema: schema,
		}},
		ToolTransport: provider.ToolTransportNative,
		ToolChoice:    provider.ToolChoiceRequired,
	}, func(event provider.StreamEvent) {
		if event.Kind == provider.ToolCallStarted {
			streamedName = event.Name
		}
	})
	if err != nil || len(result.ToolCalls) != 1 || result.ToolCalls[0].Name != "task.finish_execution" {
		t.Fatalf("local generation = %#v, %v", result, err)
	}
	if len(result.Reasoning) != 1 || !bytes.Contains(result.Reasoning[0].ProviderDetails[0], []byte("The work is complete.")) {
		t.Fatal("local reasoning lost")
	}
	if streamedName != "task.finish_execution" {
		t.Fatalf("streamed tool name = %q", streamedName)
	}
	tokens, err := service.CountTokens(t.Context(), nil, "count me")
	if err != nil || tokens != 4 {
		t.Fatalf("token count = %d, %v", tokens, err)
	}
}

func newTestService(t *testing.T) (*Service, *store.Store, string) {
	t.Helper()
	home := t.TempDir()
	database, err := store.Open(context.Background(), filepath.Join(home, "noema.sqlite3"))
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { _ = database.Close() })
	service, err := New(database, home, "")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(service.Close)
	return service, database, home
}

func waitForInstallation(t *testing.T, database *store.Store, id string) store.LocalModelInstallation {
	t.Helper()
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		value, err := database.LocalModelInstallation(t.Context(), id)
		if err == nil && value.Status == "installed" {
			return value
		}
		if err == nil && value.Status == "failed" {
			t.Fatalf("installation failed: %s", value.ErrorMessage)
		}
		time.Sleep(10 * time.Millisecond)
	}
	t.Fatal("installation did not finish")
	return store.LocalModelInstallation{}
}

func installActiveModel(t *testing.T, database *store.Store, model string) store.LocalModelInstallation {
	t.Helper()
	digest := strings.Repeat("a", 64)
	value, err := database.QueueLocalModel(t.Context(), store.LocalModelInstallation{
		ID: "installation:" + model, ModelID: model, Name: model, File: model + ".gguf",
		SourceKind: "local_file", Backend: "cpu", TotalBytes: 4, CreatedAt: time.Now(),
	})
	if err != nil {
		t.Fatal(err)
	}
	for _, status := range []string{"downloading", "verifying"} {
		value, err = database.UpdateLocalModel(t.Context(), value.ID, status, 4, 4, 0, "", "", "", "", time.Now())
		if err != nil {
			t.Fatal(err)
		}
	}
	value, err = database.UpdateLocalModel(
		t.Context(), value.ID, "installed", 4, 4, 4, digest,
		"models/blobs/"+digest+".gguf", "", "", time.Now(),
	)
	if err != nil {
		t.Fatal(err)
	}
	value, err = database.ActivateLocalModel(t.Context(), value.ID, false, time.Now())
	if err != nil {
		t.Fatal(err)
	}
	return value
}

func containsArgumentPair(arguments, pair []string) bool {
	for index := 0; index+1 < len(arguments); index++ {
		if slices.Equal(arguments[index:index+2], pair) {
			return true
		}
	}
	return false
}
