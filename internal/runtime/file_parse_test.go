package runtime

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/documents"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

func TestFileParseKeepsPathsInsideConversationRoot(t *testing.T) {
	root := t.TempDir()
	cwd := filepath.Join(root, "work")
	if err := os.Mkdir(cwd, 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(cwd, "inside.txt"), []byte("inside"), 0o600); err != nil {
		t.Fatal(err)
	}
	outside := filepath.Join(root, "outside.txt")
	if err := os.WriteFile(outside, []byte("outside"), 0o600); err != nil {
		t.Fatal(err)
	}

	result, err := parseConversationFile(context.Background(), cwd, json.RawMessage(`{"path":"inside.txt"}`))
	if err != nil || result.Content == nil || *result.Content != "inside" {
		t.Fatalf("inside parse = %#v, %v", result, err)
	}
	for _, path := range []string{"../outside.txt", outside} {
		if result, err = parseConversationFile(context.Background(), cwd, json.RawMessage(`{"path":`+strconvQuote(path)+`}`)); err == nil {
			t.Fatalf("outside path %q parsed as %#v", path, result)
		}
	}
	link := filepath.Join(cwd, "linked.txt")
	if err := os.Symlink(outside, link); err != nil {
		t.Fatal(err)
	}
	if result, err = parseConversationFile(context.Background(), cwd, json.RawMessage(`{"path":"linked.txt"}`)); err == nil {
		t.Fatalf("symbolic link parsed as %#v", result)
	}
}

func TestMain(m *testing.M) {
	if handled, status := RunFileParseWorkerIfRequested(); handled {
		os.Exit(status)
	}
	os.Exit(m.Run())
}

func TestFileParseRoutesMediaAndEnforcesBounds(t *testing.T) {
	wantMedia := map[string]string{
		"xls": documents.MediaXLS, "xlsx": documents.MediaXLSX, "ods": documents.MediaODS,
		"docx": documents.MediaDOCX, "odt": documents.MediaODT,
		"pptx": documents.MediaPPTX, "odp": documents.MediaODP,
		"rtf": documents.MediaRTF, "pdf": documents.MediaPDF, "epub": documents.MediaEPUB,
	}
	for extension, want := range wantMedia {
		got, _ := documentMediaFromExtension(extension)
		if got != want {
			t.Fatalf("%s media = %q, want %q", extension, got, want)
		}
	}

	cwd := t.TempDir()
	text := strings.Repeat("é", fileParseMinimumCharacters+2)
	if err := os.WriteFile(filepath.Join(cwd, "data.csv"), []byte(text), 0o600); err != nil {
		t.Fatal(err)
	}
	result, err := parseConversationFile(context.Background(), cwd, json.RawMessage(`{"path":"data.csv","max_chars":1000}`))
	if err != nil || result.Status != "converted" || result.Parser == nil || *result.Parser != "utf8" ||
		result.ContentFormat == nil || *result.ContentFormat != "csv" || !result.Truncated ||
		result.Content == nil || utf8.RuneCountInString(*result.Content) != fileParseMinimumCharacters {
		t.Fatalf("CSV parse = %#v, %v", result, err)
	}
	if err := os.WriteFile(filepath.Join(cwd, "notes.bin"), []byte(`{\rtf1 Hello\par World}`), 0o600); err != nil {
		t.Fatal(err)
	}
	result, err = parseConversationFile(context.Background(), cwd, json.RawMessage(`{"path":"notes.bin"}`))
	if err != nil || result.Status != "converted" || result.Format == nil || *result.Format != "rtf" ||
		result.Content == nil || *result.Content != "Hello\n\nWorld" {
		t.Fatalf("detected RTF parse = %#v, %v", result, err)
	}
	large := filepath.Join(cwd, "large.pdf")
	file, err := os.Create(large)
	if err != nil {
		t.Fatal(err)
	}
	if err = file.Truncate(fileParseMaximumInput + 1); err != nil {
		file.Close()
		t.Fatal(err)
	}
	if err = file.Close(); err != nil {
		t.Fatal(err)
	}
	result, err = parseConversationFile(context.Background(), cwd, json.RawMessage(`{"path":"large.pdf"}`))
	if err != nil || result.Status != "failed" || result.Error == nil || *result.Error != "source_too_large" {
		t.Fatalf("large document parse = %#v, %v", result, err)
	}
}

func TestDocumentParsePermitHonorsDeadline(t *testing.T) {
	select {
	case documentParsePermit <- struct{}{}:
		defer func() { <-documentParsePermit }()
	default:
		t.Fatal("document parse permit is already in use")
	}
	ctx, cancel := context.WithTimeout(context.Background(), 20*time.Millisecond)
	defer cancel()
	started := time.Now()
	_, timedOut := convertDocument(ctx, []byte(`{\rtf1 waiting}`), documents.MediaRTF)
	if !timedOut || time.Since(started) > time.Second {
		t.Fatalf("permit wait timed out = %t after %s", timedOut, time.Since(started))
	}
}

func TestChatFileParseStoresAndReplaysResult(t *testing.T) {
	cwd := t.TempDir()
	if err := os.WriteFile(filepath.Join(cwd, "report.txt"), []byte("durable content"), 0o600); err != nil {
		t.Fatal(err)
	}
	chat, database, conversation := chatFixtureAt(t, cwd)
	requests := make(chan provider.GenerateRequest, 2)
	chat.openRouter = generatorFunc(func(
		_ context.Context,
		request provider.GenerateRequest,
		_ func(provider.StreamEvent),
	) (provider.GenerationResult, error) {
		requests <- request
		if len(requests) == 1 {
			return provider.GenerationResult{
				Model: "openai/gpt-5.6-luna", Text: "Reading.",
				ToolCalls: []provider.GenerationToolCall{{
					ProviderCallID: "parse_call", ProviderName: "parse", Name: fileParseName,
					Payload: json.RawMessage(`{"path":"report.txt"}`),
				}},
			}, nil
		}
		return provider.GenerationResult{Model: "openai/gpt-5.6-luna", Text: "Done."}, nil
	})
	events, err := chat.Subscribe(context.Background(), conversation.ID)
	if err != nil {
		t.Fatal(err)
	}
	<-events
	if _, err = chat.SendTurn(context.Background(), SendTurnInput{
		ConversationID: conversation.ID, Input: "Read the report",
	}); err != nil {
		t.Fatal(err)
	}
	collectCompletedTurns(t, events, 1)
	initial := <-requests
	if len(initial.Tools) != 4 || initial.Tools[0].Name != fileParseName ||
		initial.Tools[0].Description != fileParseTool().Description ||
		string(initial.Tools[0].InputSchema) != string(fileParseSchema) {
		t.Fatalf("advertised file.parse tool = %#v", initial.Tools)
	}
	continuation := <-requests
	var replay *provider.ReplayToolResult
	for _, message := range continuation.Messages {
		if message.ToolResult != nil {
			replay = message.ToolResult
		}
	}
	if replay == nil || replay.Name != fileParseName || !replay.Success {
		t.Fatalf("replayed result = %#v", replay)
	}
	var replayed fileParseResponse
	if err := json.Unmarshal(replay.Payload, &replayed); err != nil || replayed.Content == nil || *replayed.Content != "durable content" {
		t.Fatalf("replayed payload = %#v, %v", replayed, err)
	}

	page, err := database.ConversationItemPage(context.Background(), conversation.ID, "", 20)
	if err != nil {
		t.Fatal(err)
	}
	for _, item := range page.Items {
		if item.Kind != store.ConversationToolResult {
			continue
		}
		action, _ := nestedAction(item.Payload)
		payload, _ := action["payload"].(map[string]any)
		if payload["content"] != "durable content" || payload["status"] != "converted" {
			t.Fatalf("stored parse payload = %#v", payload)
		}
		return
	}
	t.Fatal("stored parse result is unavailable")
}
