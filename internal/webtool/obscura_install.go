package webtool

import (
	"archive/tar"
	"archive/zip"
	"compress/gzip"
	"context"
	"crypto/sha256"
	"encoding/hex"
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
	obscuraRelease    = "v0.2.2"
	obscuraReleaseURL = "https://github.com/h4ckf0r0day/obscura/releases/download/" + obscuraRelease
	obscuraArchiveMax = int64(256 << 20)
	obscuraFileMax    = int64(128 << 20)
)

type obscuraAsset struct {
	platform, architecture, archive, executable, worker, checksum string
}

func obscuraAssetFor(goos, goarch string) (obscuraAsset, error) {
	asset := obscuraAsset{architecture: goarch, executable: "obscura", worker: "obscura-worker"}
	arch := "x86_64"
	if goarch == "arm64" {
		arch = "aarch64"
	}
	switch goos + "/" + goarch {
	case "linux/amd64":
		asset.platform, asset.checksum = "linux", "faf46c28948c10c6d44d6f46faad577adba43d63bb19b83cdb92a5e22bdd5da1"
	case "linux/arm64":
		asset.platform, asset.checksum = "linux", "5fc7e90393e38dc60288381a523eaf8dddb9a1e2925874844c8433a69bdf75c1"
	case "darwin/amd64":
		asset.platform, asset.checksum = "macos", "4a27584576eeca532813e0fa87a3f9e79b128bcd3e7a459a683c35a8989d2616"
	case "darwin/arm64":
		asset.platform, asset.checksum = "macos", "ae462d3518f5683464a53d00d1703effc57e128faba6706a82ec6033348eddd8"
	case "windows/amd64":
		asset.platform, asset.checksum = "windows", "4b4ce93b80de134bd6dbb775e701538ba09e53f36c32e2f5b5084cbe65a16a5b"
		asset.executable, asset.worker = "obscura.exe", "obscura-worker.exe"
	default:
		return obscuraAsset{}, errors.New("Obscura is unavailable for this platform")
	}
	extension := ".tar.gz"
	if goos == "windows" {
		extension = ".zip"
	}
	asset.archive = "obscura-" + arch + "-" + asset.platform + "-stealth" + extension
	return asset, nil
}

// PrepareObscura installs the pinned upstream release before an Obscura route is saved.
func (s *Service) PrepareObscura(ctx context.Context) error {
	if s.browserPath != "" {
		return nil
	}
	asset, err := obscuraAssetFor(runtime.GOOS, runtime.GOARCH)
	if err != nil {
		return err
	}
	return installObscura(ctx, s.obscuraHome, s.endpoints["obscura"], &http.Client{Timeout: 5 * time.Minute}, asset)
}

func installObscura(ctx context.Context, home, releaseURL string, client *http.Client, asset obscuraAsset) error {
	if path := installedObscuraPath(home, asset); path != "" {
		return nil
	}
	destination := obscuraDirectory(home, asset)
	if _, err := os.Lstat(destination); err == nil {
		return errors.New("existing Obscura installation is invalid")
	} else if !errors.Is(err, os.ErrNotExist) {
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
	if err := downloadObscuraArchive(ctx, client, releaseURL, asset.archive, archivePath, asset.checksum); err != nil {
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
		if installedObscuraPath(home, asset) != "" {
			return nil
		}
		return err
	}
	return nil
}
func (s *Service) obscuraPath() string {
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
	path, err := verifyObscuraDirectory(obscuraDirectory(home, asset), asset)
	if err != nil {
		return ""
	}
	return path
}
func obscuraDirectory(home string, asset obscuraAsset) string {
	return filepath.Join(home, "system", "tools", "obscura", obscuraRelease, asset.platform+"-"+asset.architecture)
}
func verifyObscuraDirectory(directory string, asset obscuraAsset) (string, error) {
	for _, name := range []string{asset.executable, asset.worker} {
		path := filepath.Join(directory, name)
		info, err := os.Lstat(path)
		if err != nil || !info.Mode().IsRegular() || info.Size() == 0 {
			return "", errors.New("Obscura release executable is missing or invalid")
		}
		if err := os.Chmod(path, 0o700); err != nil {
			return "", err
		}
	}
	return filepath.Join(directory, asset.executable), nil
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
func extractObscuraArchive(archive, destination string, asset obscuraAsset) error {
	seen := make(map[string]bool)
	total := int64(0)
	write := func(name string, size int64, reader io.Reader) error {
		name = strings.TrimPrefix(name, "./")
		if name == "" || strings.ContainsAny(name, `/\\`) || seen[name] || (name != asset.executable && name != asset.worker) {
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
	if len(seen) != 2 {
		return errors.New("Obscura archive contents are incomplete")
	}
	return nil
}
