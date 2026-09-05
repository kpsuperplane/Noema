package localmodel

import (
	"archive/tar"
	"archive/zip"
	"compress/gzip"
	"context"
	_ "embed"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"runtime"
	"strings"
)

//go:embed runtime-assets.json
var runtimeManifestJSON []byte

var runtimeManifest = func() struct {
	ReleaseTag string `json:"release_tag"`
	Commit     string `json:"commit"`
} { var manifest struct {
	ReleaseTag string `json:"release_tag"`
	Commit     string `json:"commit"`
}; if err := json.Unmarshal(runtimeManifestJSON, &manifest); err != nil {
	panic(err)
}; return manifest }()

type runtimeAsset struct {
	Target  string
	Backend string
	Name    string
	URL     string
	SHA256  string
	Size    int64
}

const githubAssetURL = "https://api.github.com/repos/ggml-org/llama.cpp/releases/assets/"

var runtimeAssets = []runtimeAsset{
	{
		Target: "darwin/arm64", Backend: "metal",
		Name: "llama-b10015-bin-macos-arm64.tar.gz", URL: githubAssetURL + "477466652",
		SHA256: "8d3144eb71a4b9b5b9ed50512f659d1cbcd5772e30aca75e6f9a0d7d68c311a7", Size: 10_769_379,
	},
	{
		Target: "darwin/amd64", Backend: "metal",
		Name: "llama-b10015-bin-macos-x64.tar.gz", URL: githubAssetURL + "477466665",
		SHA256: "295a51dad3feafaafed8aa8fba9d9429fcaca8a2e7e66c73d839076fdf3fec6f", Size: 11_047_847,
	},
	{
		Target: "linux/amd64", Backend: "vulkan",
		Name: "llama-b10015-bin-ubuntu-vulkan-x64.tar.gz", URL: githubAssetURL + "477466915",
		SHA256: "cdd2fed8d96dcb584f6f7907df67c3787454e1f44e176580ae2da34fc79d63a0", Size: 31_297_821,
	},
	{
		Target: "linux/amd64", Backend: "cpu",
		Name: "llama-b10015-bin-ubuntu-x64.tar.gz", URL: githubAssetURL + "477466926",
		SHA256: "fc9c641c5ab5ce74b01d3a95123ff76ad488e1d46122d602f20604100ceae834", Size: 15_876_053,
	},
	{
		Target: "windows/amd64", Backend: "cuda",
		Name: "llama-b10015-bin-win-cuda-12.4-x64.zip", URL: githubAssetURL + "477466978",
		SHA256: "336104b92b9b6a53a39eb7a373003d6256c1f20af3fc8213f65c2c3c11716469", Size: 248_837_340,
	},
	{
		Target: "windows/amd64", Backend: "cuda",
		Name: "cudart-llama-bin-win-cuda-12.4-x64.zip", URL: githubAssetURL + "477466175",
		SHA256: "8c79a9b226de4b3cacfd1f83d24f962d0773be79f1e7b75c6af4ded7e32ae1d6", Size: 391_443_627,
	},
	{
		Target: "windows/amd64", Backend: "vulkan",
		Name: "llama-b10015-bin-win-vulkan-x64.zip", URL: githubAssetURL + "477467395",
		SHA256: "e2b63eba0fb124e51c93159540e11a861236f3701636d417b92504209a315210", Size: 33_055_540,
	},
	{
		Target: "windows/amd64", Backend: "cpu",
		Name: "llama-b10015-bin-win-cpu-x64.zip", URL: githubAssetURL + "477466965",
		SHA256: "b850b461695b4c3e3811757fe20ae218bce06018a7f274c6c56485cf23adf980", Size: 18_271_925,
	},
}

func (s *Service) ensureRuntimeAssets(ctx context.Context, backend string) (string, error) {
	target := runtime.GOOS + "/" + runtime.GOARCH
	executable := "llama-server"
	if runtime.GOOS == "windows" {
		executable += ".exe"
	}
	if s.runtimeRoot != "" {
		path := filepath.Join(s.runtimeRoot, backend, executable)
		if info, err := os.Stat(path); err == nil && info.Mode().IsRegular() {
			if err := checkRuntimePlatform(); err != nil {
				return "", err
			}
			return path, nil
		}
	}
	var selected []runtimeAsset
	for _, asset := range runtimeAssets {
		if asset.Target == target && asset.Backend == backend {
			selected = append(selected, asset)
		}
	}
	if len(selected) == 0 && runtime.GOOS == "darwin" && backend == "cpu" {
		return s.ensureRuntimeAssets(ctx, "metal")
	}
	if len(selected) == 0 {
		return "", errors.New("llama.cpp runtime is unavailable for this platform and backend")
	}
	if err := checkRuntimePlatform(); err != nil {
		return "", err
	}

	directory := filepath.Join(s.home, "system", "tools", "llama.cpp", runtimeManifest.ReleaseTag, target, backend)
	path := filepath.Join(directory, executable)
	if info, err := os.Stat(path); err == nil && info.Mode().IsRegular() {
		return path, nil
	}

	stage := directory + ".new"
	_ = os.RemoveAll(stage)
	if err := os.MkdirAll(stage, 0o700); err != nil {
		return "", err
	}
	for _, asset := range selected {
		archivePath := filepath.Join(s.home, "system", "tmp", asset.Name)
		if err := s.downloadRuntimeAsset(ctx, asset, archivePath); err != nil {
			_ = os.RemoveAll(stage)
			return "", err
		}
		if err := extractRuntimeArchive(archivePath, stage); err != nil {
			_ = os.RemoveAll(stage)
			return "", err
		}
	}
	server := filepath.Join(stage, executable)
	if info, err := os.Stat(server); err != nil || !info.Mode().IsRegular() {
		_ = os.RemoveAll(stage)
		return "", errors.New("llama.cpp archive has no server executable")
	}
	if err := os.Chmod(server, 0o700); err != nil {
		return "", err
	}
	if err := os.MkdirAll(filepath.Dir(directory), 0o700); err != nil {
		return "", err
	}
	backup := directory + ".old"
	if err := os.RemoveAll(backup); err != nil {
		return "", err
	}
	replaced := false
	if _, err := os.Stat(directory); err == nil {
		if err = os.Rename(directory, backup); err != nil {
			return "", err
		}
		replaced = true
	} else if !errors.Is(err, os.ErrNotExist) {
		return "", err
	}
	if err := os.Rename(stage, directory); err != nil {
		_ = os.RemoveAll(stage)
		if replaced {
			if restoreErr := os.Rename(backup, directory); restoreErr != nil {
				return "", fmt.Errorf("publish llama.cpp runtime: %w; restore previous files: %v", err, restoreErr)
			}
		}
		return "", err
	}
	_ = os.RemoveAll(backup)
	return path, nil
}

func (s *Service) downloadRuntimeAsset(ctx context.Context, asset runtimeAsset, path string) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		return err
	}
	if info, err := os.Stat(path); err == nil && info.Size() == asset.Size {
		digest, _, hashErr := hashFile(ctx, path)
		if hashErr == nil && digest == asset.SHA256 {
			return nil
		}
	}

	request, err := http.NewRequestWithContext(ctx, http.MethodGet, asset.URL, nil)
	if err != nil {
		return err
	}
	request.Header.Set("Accept", "application/octet-stream")
	request.Header.Set("User-Agent", "Noema")
	response, err := s.client.Do(request)
	if err != nil {
		return err
	}
	defer response.Body.Close()
	if response.StatusCode < 200 || response.StatusCode >= 300 {
		return fmt.Errorf("runtime source returned HTTP %d", response.StatusCode)
	}

	temporary := path + ".partial"
	file, err := os.OpenFile(temporary, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, 0o600)
	if err != nil {
		return err
	}
	written, copyErr := io.Copy(file, io.LimitReader(response.Body, asset.Size+1))
	syncErr := file.Sync()
	closeErr := file.Close()
	if copyErr != nil {
		return copyErr
	}
	if syncErr != nil {
		return syncErr
	}
	if closeErr != nil {
		return closeErr
	}
	if written != asset.Size {
		return errors.New("llama.cpp runtime size does not match")
	}
	digest, _, err := hashFile(ctx, temporary)
	if err != nil {
		return err
	}
	if digest != asset.SHA256 {
		_ = os.Remove(temporary)
		return errors.New("llama.cpp runtime checksum does not match")
	}
	return os.Rename(temporary, path)
}

func extractRuntimeArchive(path, destination string) error {
	if strings.HasSuffix(path, ".zip") {
		return extractZip(path, destination)
	}
	file, err := os.Open(path)
	if err != nil {
		return err
	}
	defer file.Close()
	gzipReader, err := gzip.NewReader(file)
	if err != nil {
		return err
	}
	defer gzipReader.Close()

	type alias struct{ name, target string }
	var aliases []alias
	reader := tar.NewReader(gzipReader)
	for {
		header, err := reader.Next()
		if errors.Is(err, io.EOF) {
			break
		}
		if err != nil {
			return err
		}
		name := strings.TrimPrefix(filepath.ToSlash(header.Name), "llama-b10015/")
		if name == "" || strings.HasSuffix(name, "/") || !runtimeArchiveFile(name) {
			continue
		}
		if !safeArchivePath(name) {
			return errors.New("llama.cpp archive entry is invalid")
		}
		switch header.Typeflag {
		case tar.TypeReg:
			if err := writeArchiveFile(destination, name, reader, header.Size); err != nil {
				return err
			}
		case tar.TypeSymlink:
			aliases = append(aliases, alias{name: name, target: header.Linkname})
		default:
			return errors.New("llama.cpp archive entry is invalid")
		}
	}
	links := make(map[string]string, len(aliases))
	for _, alias := range aliases {
		links[alias.name] = filepath.Clean(filepath.Join(filepath.Dir(alias.name), filepath.FromSlash(alias.target)))
	}
	for _, alias := range aliases {
		if err := copyArchiveAlias(destination, alias.name, links); err != nil {
			return err
		}
	}
	return nil
}

func extractZip(path, destination string) error {
	reader, err := zip.OpenReader(path)
	if err != nil {
		return err
	}
	defer reader.Close()
	for _, entry := range reader.File {
		if entry.FileInfo().IsDir() || !runtimeArchiveFile(entry.Name) {
			continue
		}
		if !safeArchivePath(entry.Name) || !entry.Mode().IsRegular() {
			return errors.New("llama.cpp archive entry is invalid")
		}
		source, err := entry.Open()
		if err != nil {
			return err
		}
		err = writeArchiveFile(destination, entry.Name, source, int64(entry.UncompressedSize64))
		_ = source.Close()
		if err != nil {
			return err
		}
	}
	return nil
}

func runtimeArchiveFile(name string) bool {
	base := strings.ToLower(filepath.Base(filepath.FromSlash(name)))
	return base == "llama-server" || base == "llama-server.exe" ||
		strings.HasPrefix(base, "license") || strings.Contains(base, ".so") ||
		strings.Contains(base, ".dylib") || strings.HasSuffix(base, ".dll")
}

func safeArchivePath(name string) bool {
	clean := filepath.Clean(filepath.FromSlash(name))
	return clean != "." && clean != ".." && !filepath.IsAbs(clean) &&
		!strings.HasPrefix(clean, ".."+string(filepath.Separator))
}

func writeArchiveFile(root, name string, source io.Reader, size int64) error {
	target := filepath.Join(root, filepath.Clean(filepath.FromSlash(name)))
	if err := os.MkdirAll(filepath.Dir(target), 0o700); err != nil {
		return err
	}
	file, err := os.OpenFile(target, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0o600)
	if err != nil {
		return err
	}
	written, copyErr := io.Copy(file, io.LimitReader(source, size+1))
	closeErr := file.Close()
	if copyErr != nil {
		return copyErr
	}
	if closeErr != nil {
		return closeErr
	}
	if written != size {
		return errors.New("llama.cpp archive entry size does not match")
	}
	return nil
}

func copyArchiveAlias(root, name string, links map[string]string) error {
	resolved := links[name]
	for range links {
		next, found := links[resolved]
		if !found {
			break
		}
		resolved = next
	}
	if !safeArchivePath(resolved) {
		return errors.New("llama.cpp archive link is invalid")
	}
	if _, unresolved := links[resolved]; unresolved {
		return errors.New("llama.cpp archive link cycle is invalid")
	}
	source, err := os.Open(filepath.Join(root, resolved))
	if err != nil {
		return errors.New("llama.cpp archive link target is unavailable")
	}
	defer source.Close()
	info, err := source.Stat()
	if err != nil || !info.Mode().IsRegular() {
		return errors.New("llama.cpp archive link target is invalid")
	}
	return writeArchiveFile(root, name, source, info.Size())
}
