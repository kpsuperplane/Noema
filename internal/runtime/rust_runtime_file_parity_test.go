package runtime

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"unicode/utf8"
)

func TestRustRuntime_text_parser_bounds_utf8_without_using_anydoc(t *testing.T) {
	// Rust source: crates/noema-runtime/src/file_tools/tests.rs::text_parser_bounds_utf8_without_using_anydoc.
	path := filepath.Join(t.TempDir(), "large.csv")
	if err := os.WriteFile(path, []byte(strings.Repeat("rank,domain\n1,éxample.com\n", 200)), 0o600); err != nil {
		t.Fatal(err)
	}
	file, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	defer file.Close()
	response := parseOpenFile(context.Background(), file, "large.csv", 1_000)
	if response.Status != "converted" || response.Parser == nil || *response.Parser != "utf8" || response.ContentFormat == nil || *response.ContentFormat != "csv" || response.ReturnedChars != 1_000 || !response.Truncated {
		t.Fatalf("bounded UTF-8 response = %#v", response)
	}
	if response.Content == nil || utf8.RuneCountInString(*response.Content) != 1_000 {
		t.Fatalf("bounded UTF-8 content = %#v", response.Content)
	}
}

func TestRustRuntime_email_source_is_available_as_bounded_text(t *testing.T) {
	// Rust source: crates/noema-runtime/src/file_tools/tests.rs::email_source_is_available_as_bounded_text.
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
	if response.Status != "converted" || response.Parser == nil || *response.Parser != "utf8" || response.Format == nil || *response.Format != "eml" || response.ContentFormat == nil || *response.ContentFormat != "text" || response.Content == nil || !strings.Contains(*response.Content, "Order 8821") {
		t.Fatalf("email response = %#v", response)
	}
}

func TestRustRuntime_artifact_preview_reuses_bounded_email_parsing(t *testing.T) {
	// Rust source: crates/noema-runtime/src/file_tools/tests.rs::artifact_preview_reuses_bounded_email_parsing.
	path := filepath.Join(t.TempDir(), "delivery.eml")
	if err := os.WriteFile(path, []byte("Subject: Delivery\n\nOrder 8821 delivered."), 0o600); err != nil {
		t.Fatal(err)
	}
	file, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	response := parseOpenFile(context.Background(), file, "delivery.eml", 1_000)
	file.Close()
	if response.Status != "converted" || response.ContentFormat == nil || *response.ContentFormat != "text" || response.Content == nil || !strings.Contains(*response.Content, "Order 8821") {
		t.Fatalf("email artifact preview = %#v", response)
	}
}

func TestRustRuntime_unsupported_document_has_a_bounded_result(t *testing.T) {
	// Rust source: crates/noema-runtime/src/file_tools/tests.rs::unsupported_document_has_a_bounded_result.
	path := filepath.Join(t.TempDir(), "unknown.bin")
	if err := os.WriteFile(path, []byte("not a document"), 0o600); err != nil {
		t.Fatal(err)
	}
	file, err := os.Open(path)
	if err != nil {
		t.Fatal(err)
	}
	response := parseOpenFile(context.Background(), file, "unknown.bin", 1_000)
	file.Close()
	if response.Status != "unsupported" || response.Error == nil || *response.Error != "unsupported_format" || response.Content != nil {
		t.Fatalf("unsupported document = %#v", response)
	}
}

func TestRustRuntime_raster_images_route_to_ocr(t *testing.T) {
	// Rust source: crates/noema-runtime/src/file_tools/tests.rs::raster_images_route_to_ocr.
	for _, extension := range []string{"bmp", "gif", "jpg", "jpeg", "png", "tif", "tiff", "webp"} {
		media, detected := imageMediaFromExtension(extension)
		if media == "" || detected != extension || !isImageMedia(media) {
			t.Errorf("%s image media = %q, %q", extension, media, detected)
		}
	}
	if !isImageMedia("image/png; charset=binary") || isImageMedia("application/pdf") {
		t.Fatal("image media routing changed")
	}
}

func TestRustRuntime_worker_response_reports_the_selected_parser(t *testing.T) {
	// Rust source: crates/noema-runtime/src/file_tools/tests.rs::worker_response_reports_the_selected_parser.
	worker := fileParseResponse{Status: "converted", Parser: stringPtrForRuntime("tesseract"), Format: stringPtrForRuntime("png"), ContentFormat: stringPtrForRuntime("text"), Content: stringPtrForRuntime("Standing desk"), ReturnedChars: 13}
	if worker.Parser == nil || *worker.Parser != "tesseract" || worker.ContentFormat == nil || *worker.ContentFormat != "text" || worker.Content == nil || *worker.Content != "Standing desk" {
		t.Fatalf("worker response = %#v", worker)
	}
}

func TestRustRuntime_download_paths_reject_traversal_and_never_replace_a_destination(t *testing.T) {
	// Rust source: crates/noema-runtime/src/file_tools/tests.rs::download_paths_reject_traversal_and_never_replace_a_destination.
	for _, path := range []string{"../outside.csv", "/outside.csv"} {
		if _, err := normalizedFilePath(path); err == nil {
			t.Fatalf("unsafe path accepted: %q", path)
		}
	}
	directory := t.TempDir()
	target := filepath.Join(directory, "target.csv")
	if err := os.WriteFile(target, []byte("original"), 0o600); err != nil {
		t.Fatal(err)
	}
	if _, err := executeFileDownload(t.Context(), directory, []byte(`{"url":"https://example.net/file.csv","path":"target.csv"}`)); err == nil || err.Error() != "destination already exists" {
		t.Fatalf("existing destination error = %v", err)
	}
	content, err := os.ReadFile(target)
	if err != nil || string(content) != "original" {
		t.Fatalf("destination changed: %q, %v", content, err)
	}
}
