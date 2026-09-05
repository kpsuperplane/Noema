package runtime

import (
	"archive/zip"
	"bytes"
	"context"
	"encoding/json"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	goruntime "runtime"
	"strings"
	"testing"
	"time"
	"unicode/utf8"

	"github.com/abemedia/go-cfb"
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
	if len(os.Args) == 3 && os.Args[1] == "noema-ocr-grandchild" {
		time.Sleep(300 * time.Millisecond)
		if err := os.WriteFile(os.Args[2], []byte("survived"), 0o600); err != nil {
			os.Exit(2)
		}
		os.Exit(0)
	}
	if len(os.Args) == 4 && os.Args[1] == "noema-ocr-timeout" {
		child := exec.Command(os.Args[0], "noema-ocr-grandchild", os.Args[3])
		if child.Start() != nil || os.WriteFile(os.Args[2], []byte("ready"), 0o600) != nil {
			os.Exit(2)
		}
		time.Sleep(10 * time.Second)
		os.Exit(2)
	}
	if len(os.Args) == 5 && strings.Join(os.Args[1:], " ") == "stdin stdout -l eng" {
		content, err := io.ReadAll(os.Stdin)
		if err != nil {
			os.Exit(2)
		}
		if strings.HasPrefix(string(content), "NOEMA_OCR_SUCCESS") {
			_, _ = os.Stdout.WriteString(strings.Repeat("é", fileParseMaximumCharacters+2))
			os.Exit(0)
		}
		os.Exit(2)
	}
	if handled, status := RunFileParseWorkerIfRequested(); handled {
		os.Exit(status)
	}
	os.Exit(m.Run())
}

func TestFileParseRoutesMediaAndEnforcesBounds(t *testing.T) {
	wantMedia := map[string]string{
		"xls": documents.MediaXLS, "xlsx": documents.MediaXLSX, "ods": documents.MediaODS,
		"docx": documents.MediaDOCX, "odt": documents.MediaODT,
		"pptx": documents.MediaPPTX, "ppt": documents.MediaPPT, "pps": documents.MediaPPT, "pot": documents.MediaPPT, "odp": documents.MediaODP,
		"rtf": documents.MediaRTF, "pdf": documents.MediaPDF, "epub": documents.MediaEPUB,
	}
	for extension, want := range wantMedia {
		got, _ := documentMediaFromExtension(extension)
		if got != want {
			t.Fatalf("%s media = %q, want %q", extension, got, want)
		}
	}
	wantImages := map[string]string{
		"bmp": "image/bmp", "gif": "image/gif", "jpg": "image/jpeg", "jpeg": "image/jpeg",
		"png": "image/png", "tif": "image/tiff", "tiff": "image/tiff", "webp": "image/webp",
	}
	for extension, want := range wantImages {
		got, detected := imageMediaFromExtension(extension)
		if got != want || detected != extension || !isImageMedia(got) {
			t.Fatalf("%s image media = %q, %q, want %q", extension, got, detected, want)
		}
	}
	if !isImageMedia(" image/custom; parameter=value") {
		t.Fatal("image media type did not route to OCR")
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
	large := filepath.Join(cwd, "large.png")
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
	result, err = parseConversationFile(context.Background(), cwd, json.RawMessage(`{"path":"large.png"}`))
	if err != nil || result.Status != "failed" || result.Error == nil || *result.Error != "source_too_large" {
		t.Fatalf("large image parse = %#v, %v", result, err)
	}
}

func TestImageOCRUnavailable(t *testing.T) {
	t.Setenv("PATH", t.TempDir())
	cwd := t.TempDir()
	if err := os.WriteFile(filepath.Join(cwd, "scan.png"), []byte("image"), 0o600); err != nil {
		t.Fatal(err)
	}
	result, err := parseConversationFile(context.Background(), cwd, json.RawMessage(`{"path":"scan.png"}`))
	if err != nil || result.Status != "failed" || result.Error == nil || *result.Error != "ocr_unavailable" {
		t.Fatalf("OCR without Tesseract = %#v, %v", result, err)
	}
}

func TestImageOCRConvertsAndTruncates(t *testing.T) {
	installFakeTesseract(t)
	conversion := convertImageOCR([]byte("NOEMA_OCR_SUCCESS"))
	if !conversion.Converted || conversion.ErrorCode != "" ||
		utf8.RuneCountInString(conversion.Content) != fileParseMaximumCharacters || !conversion.Truncated {
		t.Fatalf("OCR conversion = %#v", conversion)
	}
	bounded := truncateRunes(conversion.Content, fileParseMinimumCharacters)
	if utf8.RuneCountInString(bounded) != fileParseMinimumCharacters ||
		utf8.RuneCountInString(conversion.Content) <= utf8.RuneCountInString(bounded) {
		t.Fatalf("bounded OCR content has %d characters", utf8.RuneCountInString(bounded))
	}
}

func TestImageOCRTimeoutStopsProcessTree(t *testing.T) {
	directory := t.TempDir()
	ready := filepath.Join(directory, "ready")
	survived := filepath.Join(directory, "survived")
	ctx, cancel := context.WithCancel(context.Background())
	done := make(chan bool, 1)
	go func() {
		executable, err := os.Executable()
		if err != nil {
			done <- false
			return
		}
		_, _, timedOut := runFileParseCommand(
			ctx, exec.Command(executable, "noema-ocr-timeout", ready, survived), 1_024,
		)
		done <- timedOut
	}()
	deadline := time.Now().Add(5 * time.Second)
	for {
		if _, err := os.Stat(ready); err == nil {
			break
		}
		if time.Now().After(deadline) {
			cancel()
			t.Fatal("fake Tesseract did not start its descendant")
		}
		time.Sleep(10 * time.Millisecond)
	}
	cancel()
	select {
	case timedOut := <-done:
		if !timedOut {
			t.Fatal("OCR conversion did not report its canceled deadline")
		}
	case <-time.After(3 * time.Second):
		t.Fatal("OCR process tree did not stop")
	}
	time.Sleep(500 * time.Millisecond)
	if _, err := os.Stat(survived); !os.IsNotExist(err) {
		t.Fatalf("OCR descendant survived cancellation: %v", err)
	}
}

func installFakeTesseract(t *testing.T) {
	t.Helper()
	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	name := "tesseract"
	if goruntime.GOOS == "windows" {
		name += ".exe"
	}
	directory := t.TempDir()
	target, err := os.OpenFile(filepath.Join(directory, name), os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o700)
	if err != nil {
		t.Fatal(err)
	}
	source, err := os.Open(executable)
	if err != nil {
		target.Close()
		t.Fatal(err)
	}
	_, copyError := io.Copy(target, source)
	closeError := target.Close()
	source.Close()
	if copyError != nil || closeError != nil {
		t.Fatalf("copy fake Tesseract: %v, %v", copyError, closeError)
	}
	t.Setenv("PATH", directory)
	if goruntime.GOOS == "windows" {
		t.Setenv("PATHEXT", ".EXE")
	}
}

func TestDocumentMediaPrefersContentOverLegacyExtension(t *testing.T) {
	pptx := minimalPPTXSignature(t)
	if media, format := documentMedia(pptx, "ppt"); media != documents.MediaPPTX || format != "pptx" {
		t.Fatalf("PPTX named .ppt = %q, format = %q", media, format)
	}

	compound := func(streamName string) []byte {
		t.Helper()
		file, createError := os.CreateTemp(t.TempDir(), "*.ppt")
		if createError != nil {
			t.Fatal(createError)
		}
		writer := cfb.NewWriterV3(file)
		stream, createError := writer.CreateStream(streamName)
		if createError == nil {
			_, createError = stream.Write([]byte("content"))
		}
		if createError == nil {
			createError = stream.Close()
		}
		if createError == nil {
			createError = writer.Close()
		}
		if closeError := file.Close(); createError == nil {
			createError = closeError
		}
		if createError != nil {
			t.Fatal(createError)
		}
		content, readError := os.ReadFile(file.Name())
		if readError != nil {
			t.Fatal(readError)
		}
		return content
	}
	for _, test := range []struct {
		stream, mediaType, format string
	}{
		{"WordDocument", documents.MediaDOC, "doc"},
		{"PowerPoint Document", documents.MediaPPT, "ppt"},
		{"Workbook", documents.MediaXLS, "excel"},
		{"Book", documents.MediaXLS, "excel"},
	} {
		media, format := documentMedia(compound(test.stream), "ppt")
		if media != test.mediaType || format != test.format {
			t.Fatalf("%s CFB named .ppt = %q, format = %q", test.stream, media, format)
		}
	}
}

func TestFileParsePrefersPPTXSignatureOverImageExtension(t *testing.T) {
	cwd := t.TempDir()
	if err := os.WriteFile(filepath.Join(cwd, "deck.png"), minimalPPTXSignature(t), 0o600); err != nil {
		t.Fatal(err)
	}
	result, err := parseConversationFile(context.Background(), cwd, json.RawMessage(`{"path":"deck.png"}`))
	if err != nil || result.Parser == nil || *result.Parser != "anydoc" ||
		result.Format == nil || *result.Format != "pptx" {
		t.Fatalf("PPTX named .png = %#v, %v", result, err)
	}
}

func TestFileParseRejectsGrowthAfterStat(t *testing.T) {
	path := filepath.Join(t.TempDir(), "growing.png")
	if err := os.WriteFile(path, []byte("small"), 0o600); err != nil {
		t.Fatal(err)
	}
	file, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	before, err := file.Stat()
	if err != nil || before.Size() != int64(len("small")) {
		t.Fatalf("initial file metadata = %#v, %v", before, err)
	}
	if err = os.Truncate(path, fileParseMaximumInput+1); err != nil {
		t.Fatal(err)
	}
	content, tooLarge, err := readFileParseInput(file)
	if err != nil || !tooLarge || len(content) != fileParseMaximumInput+1 {
		t.Fatalf("grown file read = %d bytes, too large = %t, error = %v", len(content), tooLarge, err)
	}
}

func minimalPPTXSignature(t *testing.T) []byte {
	t.Helper()
	var content bytes.Buffer
	archive := zip.NewWriter(&content)
	part, err := archive.Create("ppt/presentation.xml")
	if err == nil {
		_, err = part.Write([]byte("<presentation/>"))
	}
	if err == nil {
		err = archive.Close()
	}
	if err != nil {
		t.Fatal(err)
	}
	return content.Bytes()
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
	if len(initial.Tools) != 5 || initial.Tools[0].Name != fileParseName ||
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
