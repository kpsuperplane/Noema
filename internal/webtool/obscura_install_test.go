package webtool

import (
	"archive/tar"
	"archive/zip"
	"bytes"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestObscuraAssetsCoverShippedTargets(t *testing.T) {
	tests := []struct{ goos, arch, platform, archive, suffix string }{
		{"linux", "amd64", "linux", "obscura-x86_64-linux-stealth.tar.gz", ".tar.gz"},
		{"linux", "arm64", "linux", "obscura-aarch64-linux-stealth.tar.gz", ".tar.gz"},
		{"darwin", "amd64", "macos", "obscura-x86_64-macos-stealth.tar.gz", ".tar.gz"},
		{"darwin", "arm64", "macos", "obscura-aarch64-macos-stealth.tar.gz", ".tar.gz"},
		{"windows", "amd64", "windows", "obscura-x86_64-windows-stealth.zip", ".zip"},
	}
	for _, test := range tests {
		asset, err := obscuraAssetFor(test.goos, test.arch)
		if err != nil || asset.platform != test.platform || asset.archive != test.archive || !strings.HasSuffix(asset.archive, test.suffix) {
			t.Fatalf("asset %s/%s = %#v, %v", test.goos, test.arch, asset, err)
		}
	}
	if _, err := obscuraAssetFor("linux", "386"); err == nil {
		t.Fatal("unsupported target was accepted")
	}
}

func TestPrepareObscuraInstallsAndReusesVerifiedWorker(t *testing.T) {
	asset, _ := obscuraAssetFor(runtime.GOOS, runtime.GOARCH)
	archive := obscuraTestArchive(t, asset, nil)
	digest := sha256.Sum256(archive)
	asset.checksum = hex.EncodeToString(digest[:])
	requests := 0
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		requests++
		_, _ = writer.Write(archive)
	}))
	defer server.Close()
	home := t.TempDir()
	service := &Service{obscuraHome: home}
	if err := installObscura(t.Context(), home, server.URL, server.Client(), asset); err != nil {
		t.Fatal(err)
	}
	path := service.obscuraPath()
	if data, err := os.ReadFile(path); err != nil || string(data) != "worker" {
		t.Fatalf("installed worker = %q, %v", data, err)
	}
	if err := installObscura(t.Context(), home, server.URL, server.Client(), asset); err != nil || requests != 1 {
		t.Fatalf("reuse = %v, requests %d", err, requests)
	}
}

func TestPrepareObscuraAcceptsConcurrentInstallWinner(t *testing.T) {
	asset, _ := obscuraAssetFor(runtime.GOOS, runtime.GOARCH)
	archive := obscuraTestArchive(t, asset, nil)
	digest := sha256.Sum256(archive)
	asset.checksum = hex.EncodeToString(digest[:])
	downloads := make(chan struct{}, 2)
	release := make(chan struct{})
	server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
		downloads <- struct{}{}
		<-release
		_, _ = writer.Write(archive)
	}))
	defer server.Close()
	home := t.TempDir()
	results := make(chan error, 2)
	for range 2 {
		go func() { results <- installObscura(t.Context(), home, server.URL, server.Client(), asset) }()
	}
	<-downloads
	<-downloads
	close(release)
	for range 2 {
		if err := <-results; err != nil {
			t.Fatal(err)
		}
	}
}

func TestObscuraWindowsArchiveVerifiesExactWorker(t *testing.T) {
	asset, _ := obscuraAssetFor("windows", "amd64")
	archive := obscuraTestArchive(t, asset, nil)
	archivePath := filepath.Join(t.TempDir(), asset.archive)
	if err := os.WriteFile(archivePath, archive, 0o600); err != nil {
		t.Fatal(err)
	}
	destination := t.TempDir()
	if err := extractObscuraArchive(archivePath, destination, asset); err != nil {
		t.Fatal(err)
	}
	if path, err := verifyObscuraDirectory(destination, asset); err != nil || filepath.Base(path) != asset.executable {
		t.Fatalf("worker = %q, %v", path, err)
	}
}

func TestPrepareObscuraRejectsCorruptReleaseData(t *testing.T) {
	asset, _ := obscuraAssetFor(runtime.GOOS, runtime.GOARCH)
	archive := obscuraTestArchive(t, asset, nil)
	digest := sha256.Sum256(archive)
	asset.checksum = hex.EncodeToString(digest[:])
	for _, test := range []struct {
		name, checksum string
		archive        []byte
	}{
		{"partial", hex.EncodeToString(digest[:]), archive[:len(archive)-1]},
		{"checksum", strings.Repeat("0", 64), archive},
	} {
		t.Run(test.name, func(t *testing.T) {
			server := httptest.NewServer(http.HandlerFunc(func(writer http.ResponseWriter, request *http.Request) {
				_, _ = writer.Write(test.archive)
			}))
			defer server.Close()
			home := t.TempDir()
			asset.checksum = test.checksum
			if err := installObscura(t.Context(), home, server.URL, server.Client(), asset); err == nil || installedObscuraPath(home, asset) != "" {
				t.Fatalf("corrupt release accepted: %v", err)
			}
		})
	}
}

func TestObscuraArchiveRejectsUnsafeOrWrongFiles(t *testing.T) {
	asset, _ := obscuraAssetFor(runtime.GOOS, runtime.GOARCH)
	for _, test := range []struct {
		name   string
		mutate func(map[string][]byte)
		link   bool
	}{
		{name: "traversal", mutate: func(files map[string][]byte) { files["../worker"] = []byte("bad") }},
		{name: "unexpected", mutate: func(files map[string][]byte) { files["extra"] = []byte("bad") }},
		{name: "missing companion", mutate: func(files map[string][]byte) { delete(files, asset.worker) }},
		{name: "link", link: true},
	} {
		t.Run(test.name, func(t *testing.T) {
			archive := obscuraTestArchive(t, asset, test.mutate)
			if test.link {
				archive = obscuraLinkArchive(t, asset)
			}
			archivePath := filepath.Join(t.TempDir(), asset.archive)
			_ = os.WriteFile(archivePath, archive, 0o600)
			destination := t.TempDir()
			err := extractObscuraArchive(archivePath, destination, asset)
			if err == nil {
				_, err = verifyObscuraDirectory(destination, asset)
			}
			if err == nil {
				t.Fatal("unsafe archive was accepted")
			}
		})
	}
}

func TestPrepareObscuraKeepsExplicitOverride(t *testing.T) {
	service := &Service{browserPath: filepath.Join(t.TempDir(), "custom-worker")}
	if err := service.PrepareObscura(t.Context()); err != nil || service.obscuraPath() != service.browserPath {
		t.Fatalf("override = %q, %v", service.obscuraPath(), err)
	}
}

func obscuraTestArchive(t *testing.T, asset obscuraAsset, mutate func(map[string][]byte)) []byte {
	t.Helper()
	files := map[string][]byte{asset.executable: []byte("worker"), asset.worker: []byte("worker")}
	if mutate != nil {
		mutate(files)
	}
	var output bytes.Buffer
	if strings.HasSuffix(asset.archive, ".zip") {
		writer := zip.NewWriter(&output)
		for name, data := range files {
			file, _ := writer.Create(name)
			_, _ = file.Write(data)
		}
		_ = writer.Close()
		return output.Bytes()
	}
	compressed := gzip.NewWriter(&output)
	writer := tar.NewWriter(compressed)
	for name, data := range files {
		_ = writer.WriteHeader(&tar.Header{Name: name, Mode: 0o600, Size: int64(len(data)), Typeflag: tar.TypeReg})
		_, _ = writer.Write(data)
	}
	_ = writer.Close()
	_ = compressed.Close()
	return output.Bytes()
}

func obscuraLinkArchive(t *testing.T, asset obscuraAsset) []byte {
	t.Helper()
	var output bytes.Buffer
	compressed := gzip.NewWriter(&output)
	writer := tar.NewWriter(compressed)
	if err := writer.WriteHeader(&tar.Header{Name: asset.executable, Linkname: "other", Typeflag: tar.TypeSymlink}); err != nil {
		t.Fatal(err)
	}
	_ = writer.Close()
	_ = compressed.Close()
	return output.Bytes()
}
