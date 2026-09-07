package runtime

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"unicode/utf8"
)

// These tests preserve the direct file_tools contracts from the Rust runtime
// baseline. The production parser is the shared authority for Chat and Task.

func TestRustFileToolsTextParserBoundsUTF8WithoutAnyDoc(t *testing.T) {
	directory := t.TempDir()
	path := filepath.Join(directory, "large.csv")
	if err := os.WriteFile(path, []byte(strings.Repeat("rank,domain\n1,éxample.com\n", 200)), 0o600); err != nil {
		t.Fatal(err)
	}
	file, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()

	response := parseOpenFile(context.Background(), file, "large.csv", 1_000)
	if response.Status != "converted" || response.Parser == nil || *response.Parser != "utf8" ||
		response.ContentFormat == nil || *response.ContentFormat != "csv" ||
		response.ReturnedChars != 1_000 || !response.Truncated {
		t.Fatalf("bounded UTF-8 response = %#v", response)
	}
	if response.Content == nil || utf8.RuneCountInString(*response.Content) != 1_000 {
		t.Fatalf("bounded UTF-8 content = %#v", response.Content)
	}
}

func TestRustFileToolsEmailSourceIsAvailableAsBoundedText(t *testing.T) {
	path := filepath.Join(t.TempDir(), "delivery.eml")
	if err := os.WriteFile(path, []byte("Subject: Delivery\n\nOrder 8821 delivered."), 0o600); err != nil {
		t.Fatal(err)
	}
	file, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()

	response := parseOpenFile(context.Background(), file, "delivery.eml", 1_000)
	if response.Status != "converted" || response.Parser == nil || *response.Parser != "utf8" ||
		response.Format == nil || *response.Format != "eml" ||
		response.ContentFormat == nil || *response.ContentFormat != "text" ||
		response.Content == nil || !strings.Contains(*response.Content, "Order 8821") {
		t.Fatalf("email response = %#v", response)
	}
}

func TestRustFileToolsUnsupportedDocumentHasBoundedResult(t *testing.T) {
	directory := t.TempDir()
	unsupportedPath := filepath.Join(directory, "unknown.bin")
	if err := os.WriteFile(unsupportedPath, []byte("not a document"), 0o600); err != nil {
		t.Fatal(err)
	}
	unsupportedFile, err := os.Open(unsupportedPath)
	if err != nil {
		t.Fatal(err)
	}
	unsupported := parseOpenFile(context.Background(), unsupportedFile, "unknown.bin", 1_000)
	unsupportedFile.Close()
	if unsupported.Status != "unsupported" || unsupported.Error == nil || *unsupported.Error != "unsupported_format" || unsupported.Content != nil {
		t.Fatalf("unsupported document = %#v", unsupported)
	}

	rtfPath := filepath.Join(directory, "notes.rtf")
	if err := os.WriteFile(rtfPath, []byte(`{\rtf1\ansi Parsed text}`), 0o600); err != nil {
		t.Fatal(err)
	}
	rtfFile, err := os.Open(rtfPath)
	if err != nil {
		t.Fatal(err)
	}
	rtf := parseOpenFile(context.Background(), rtfFile, "notes.rtf", 1_000)
	rtfFile.Close()
	if rtf.Status != "converted" || rtf.Content == nil || !strings.Contains(*rtf.Content, "Parsed text") {
		t.Fatalf("RTF document = %#v", rtf)
	}
}

func TestRustFileToolsRasterImagesRouteToOCR(t *testing.T) {
	for _, extension := range []string{"bmp", "gif", "jpg", "jpeg", "png", "tif", "tiff", "webp"} {
		media, detected := imageMediaFromExtension(extension)
		if media == "" || detected != extension || !isImageMedia(media) {
			t.Errorf("%s image media = %q, %q", extension, media, detected)
		}
	}
	if !isImageMedia("image/png; charset=binary") {
		t.Error("image media type did not route to OCR")
	}
	if isImageMedia("application/pdf") {
		t.Error("PDF media type routed to OCR")
	}
}

func TestRustFileToolsDownloadPathsRejectTraversalAndNeverReplaceDestination(t *testing.T) {
	if _, err := normalizedFilePath("../outside.csv"); err == nil {
		t.Error("parent traversal was accepted")
	}
	if _, err := normalizedFilePath("/outside.csv"); err == nil {
		t.Error("absolute path was accepted")
	}

	directory := t.TempDir()
	target := filepath.Join(directory, "target.csv")
	if err := os.WriteFile(target, []byte("original"), 0o600); err != nil {
		t.Fatal(err)
	}
	_, err := executeFileDownload(t.Context(), directory, []byte(`{"url":"https://example.net/file.csv","path":"target.csv"}`))
	if err == nil || err.Error() != "destination already exists" {
		t.Fatalf("existing destination error = %v", err)
	}
	content, readErr := os.ReadFile(target)
	if readErr != nil || string(content) != "original" {
		t.Fatalf("existing destination content = %q, %v", content, readErr)
	}

	root, err := os.OpenRoot(directory)
	if err != nil {
		t.Fatal(err)
	}
	defer root.Close()
	if err := os.Symlink(".", filepath.Join(directory, "linked")); err != nil {
		t.Fatal(err)
	}
	if err := prepareDownloadParent(root, "linked/file.csv"); err == nil {
		t.Error("symbolic-link parent was accepted")
	}
}
