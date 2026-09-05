package webtool

import (
	"archive/tar"
	"archive/zip"
	"compress/gzip"
	"context"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"time"
)

const (
	obscuraRelease    = "v0.1.0"
	obscuraReleaseURL = "https://github.com/kpsuperplane/Noema/releases/download/obscura-worker-" + obscuraRelease
	obscuraArchiveMax = int64(256 << 20)
	obscuraFileMax    = int64(128 << 20)
)

type obscuraAsset struct {
	platform, architecture, target, archive, executable string
}

func obscuraAssetFor(goos, goarch string) (obscuraAsset, error) {
	asset := obscuraAsset{architecture: goarch, executable: "noema-obscura-worker"}
	switch goos + "/" + goarch {
	case "linux/amd64":
		asset.platform, asset.target = "linux", "x86_64-unknown-linux-gnu"
	case "linux/arm64":
		asset.platform, asset.target = "linux", "aarch64-unknown-linux-gnu"
	case "darwin/amd64":
		asset.platform, asset.target = "macos", "x86_64-apple-darwin"
	case "darwin/arm64":
		asset.platform, asset.target = "macos", "aarch64-apple-darwin"
	case "windows/amd64":
		asset.platform, asset.target, asset.executable = "windows", "x86_64-pc-windows-msvc", "noema-obscura-worker.exe"
	default:
		return obscuraAsset{}, errors.New("Obscura worker is unavailable for this platform")
	}
	extension := ".tar.gz"
	if goos == "windows" {
		extension = ".zip"
	}
	asset.archive = "noema-obscura-worker-" + asset.platform + "-" + asset.architecture + extension
	return asset, nil
}

// PrepareObscura installs the pinned worker before an Obscura route is saved.
func (s *Service) PrepareObscura(ctx context.Context) error {
	if s.browserPath != "" {
		return nil
	}
	return installObscura(ctx, s.obscuraHome, obscuraReleaseURL, &http.Client{Timeout: 5 * time.Minute})
}

func installObscura(ctx context.Context, home, releaseURL string, client *http.Client) error {
	asset, err := obscuraAssetFor(runtime.GOOS, runtime.GOARCH)
	if err != nil {
		return err
	}
	if path := installedObscuraPath(home, asset); path != "" {
		return nil
	}
	destination := obscuraDirectory(home, asset)
	if _, err := os.Lstat(destination); err == nil {
		return errors.New("existing Obscura installation is invalid")
	} else if !errors.Is(err, os.ErrNotExist) {
		return err
	}
	checksumText, err := downloadObscuraText(ctx, client, releaseURL, asset.archive+".sha256", 256)
	if err != nil {
		return err
	}
	checksum, err := parseObscuraChecksum(checksumText, asset.archive)
	if err != nil {
		return err
	}
	temporaryRoot := filepath.Join(home, "system", "tmp")
	if err := os.MkdirAll(temporaryRoot, 0o700); err != nil {
		return err
	}
	temporary, err := os.MkdirTemp(temporaryRoot, ".obscura-*")
	if err != nil {
		return err
	}
	defer os.RemoveAll(temporary)
	archivePath := filepath.Join(temporary, asset.archive)
	if err := downloadObscuraArchive(ctx, client, releaseURL, asset.archive, archivePath, checksum); err != nil {
		return err
	}
	stage := filepath.Join(temporary, "package")
	if err := os.Mkdir(stage, 0o700); err != nil {
		return err
	}
	if err := extractObscuraArchive(archivePath, stage, asset); err != nil {
		return err
	}
	if _, err := verifyObscuraDirectory(stage, asset); err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(destination), 0o700); err != nil {
		return err
	}
	if err := os.Rename(stage, destination); err != nil {
		return err
	}
	return nil
}
func (s *Service) obscuraWorkerPath() string {
	if s.browserPath != "" {
		return s.browserPath
	}
	asset, err := obscuraAssetFor(runtime.GOOS, runtime.GOARCH)
	if err != nil {
		return ""
	}
	return installedObscuraPath(s.obscuraHome, asset)
}

func installedObscuraPath(home string, asset obscuraAsset) string {
	path := filepath.Join(obscuraDirectory(home, asset), asset.executable)
	info, err := os.Lstat(path)
	if err != nil || !info.Mode().IsRegular() {
		return ""
	}
	return path
}
func obscuraDirectory(home string, asset obscuraAsset) string {
	return filepath.Join(home, "system", "tools", "obscura", obscuraRelease, asset.platform+"-"+asset.architecture)
}
func verifyObscuraDirectory(directory string, asset obscuraAsset) (string, error) {
	metadataBytes, err := os.ReadFile(filepath.Join(directory, "metadata.json"))
	if err != nil || int64(len(metadataBytes)) > obscuraFileMax {
		return "", errors.New("Obscura metadata is invalid")
	}
	var metadata struct {
		Target         string `json:"target"`
		Executable     string `json:"executable"`
		ExecutableSize int64  `json:"executable_size"`
	}
	if json.Unmarshal(metadataBytes, &metadata) != nil || metadata.Target != asset.target || metadata.Executable != asset.executable {
		return "", errors.New("Obscura metadata does not match this platform")
	}
	executable := filepath.Join(directory, asset.executable)
	info, err := os.Lstat(executable)
	if err != nil || !info.Mode().IsRegular() || info.Size() != metadata.ExecutableSize {
		return "", errors.New("Obscura executable does not match its release metadata")
	}
	if err := os.Chmod(executable, 0o700); err != nil {
		return "", err
	}
	return executable, nil
}
func downloadObscuraText(ctx context.Context, client *http.Client, releaseURL, name string, limit int64) ([]byte, error) {
	response, err := obscuraResponse(ctx, client, releaseURL, name)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	data, err := io.ReadAll(io.LimitReader(response.Body, limit+1))
	if err != nil || int64(len(data)) > limit {
		return nil, errors.New("Obscura release metadata is invalid")
	}
	return data, nil
}
func downloadObscuraArchive(ctx context.Context, client *http.Client, releaseURL, name, destination, checksum string) error {
	response, err := obscuraResponse(ctx, client, releaseURL, name)
	if err != nil {
		return err
	}
	defer response.Body.Close()
	file, err := os.OpenFile(destination, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
	if err != nil {
		return err
	}
	hash := sha256.New()
	written, copyErr := io.Copy(io.MultiWriter(file, hash), io.LimitReader(response.Body, obscuraArchiveMax+1))
	closeErr := file.Close()
	if copyErr != nil {
		return copyErr
	}
	if closeErr != nil {
		return closeErr
	}
	if written > obscuraArchiveMax || hex.EncodeToString(hash.Sum(nil)) != checksum {
		return errors.New("Obscura archive does not match its release metadata")
	}
	return nil
}
func obscuraResponse(ctx context.Context, client *http.Client, releaseURL, name string) (*http.Response, error) {
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, releaseURL+"/"+name, nil)
	if err != nil {
		return nil, err
	}
	request.Header.Set("User-Agent", "Noema")
	response, err := client.Do(request)
	if err != nil {
		return nil, err
	}
	if response.StatusCode != http.StatusOK {
		_ = response.Body.Close()
		return nil, errors.New("Obscura release is unavailable")
	}
	return response, nil
}
func parseObscuraChecksum(data []byte, name string) (string, error) {
	fields := strings.Fields(string(data))
	if len(fields) != 2 || fields[1] != name || len(fields[0]) != 64 {
		return "", errors.New("Obscura checksum metadata is invalid")
	}
	if _, err := hex.DecodeString(fields[0]); err != nil {
		return "", errors.New("Obscura checksum metadata is invalid")
	}
	return strings.ToLower(fields[0]), nil
}
func extractObscuraArchive(archive, destination string, asset obscuraAsset) error {
	seen := make(map[string]bool)
	total := int64(0)
	write := func(name string, size int64, reader io.Reader) error {
		name = strings.TrimPrefix(name, "./")
		if name == "" || strings.ContainsAny(name, `/\\`) || seen[name] || !obscuraArchiveName(name, asset.executable) {
			return errors.New("Obscura archive entry is invalid")
		}
		if size < 0 || size > obscuraFileMax || total+size > obscuraArchiveMax {
			return errors.New("Obscura archive content exceeds its limit")
		}
		seen[name], total = true, total+size
		file, err := os.OpenFile(filepath.Join(destination, name), os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
		if err != nil {
			return err
		}
		written, copyErr := io.Copy(file, io.LimitReader(reader, size+1))
		closeErr := file.Close()
		if copyErr != nil || closeErr != nil || written != size {
			return errors.New("Obscura archive entry is invalid")
		}
		return nil
	}
	if strings.HasSuffix(asset.archive, ".zip") {
		reader, err := zip.OpenReader(archive)
		if err != nil {
			return err
		}
		defer reader.Close()
		for _, entry := range reader.File {
			if !entry.Mode().IsRegular() || entry.UncompressedSize64 > uint64(obscuraFileMax) {
				return errors.New("Obscura archive entry is invalid")
			}
			file, err := entry.Open()
			if err != nil {
				return err
			}
			err = write(entry.Name, int64(entry.UncompressedSize64), file)
			_ = file.Close()
			if err != nil {
				return err
			}
		}
	} else {
		file, err := os.Open(archive)
		if err != nil {
			return err
		}
		defer file.Close()
		compressed, err := gzip.NewReader(file)
		if err != nil {
			return err
		}
		defer compressed.Close()
		reader := tar.NewReader(compressed)
		for {
			entry, err := reader.Next()
			if errors.Is(err, io.EOF) {
				break
			}
			if err != nil {
				return err
			}
			if entry.Typeflag == tar.TypeDir && (entry.Name == "." || entry.Name == "./") {
				continue
			}
			if entry.Typeflag != tar.TypeReg {
				return errors.New("Obscura archive entry is invalid")
			}
			if err := write(entry.Name, entry.Size, reader); err != nil {
				return err
			}
		}
	}
	if len(seen) != 5 {
		return errors.New("Obscura archive contents are incomplete")
	}
	return nil
}
func obscuraArchiveName(name, executable string) bool {
	return name == executable || name == "LICENSE-NOEMA" || name == "LICENSE-OBSCURA" || name == "SHA256SUMS" || name == "metadata.json"
}
