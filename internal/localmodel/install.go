package localmodel

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const transferNoticeBytes = int64(8 << 20)

var errLocalModelChecksum = errors.New("local model checksum does not match")

func (s *Service) download(ctx context.Context, value store.LocalModelInstallation, source, expected string, prepare bool) error {
	partial := s.partialPath(value.ID)
	defer func() {
		if ctx.Err() != nil {
			_ = os.Remove(partial)
		}
	}()
	if err := os.MkdirAll(filepath.Dir(partial), 0700); err != nil {
		return err
	}
	start := int64(0)
	if info, err := os.Stat(partial); err == nil {
		start = info.Size()
	}
	request, err := http.NewRequestWithContext(ctx, http.MethodGet, source, nil)
	if err != nil {
		return err
	}
	if start > 0 {
		request.Header.Set("Range", fmt.Sprintf("bytes=%d-", start))
	}
	response, err := s.client.Do(request)
	if err != nil {
		return err
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK && response.StatusCode != http.StatusPartialContent {
		return fmt.Errorf("model source returned HTTP %d", response.StatusCode)
	}
	if start > 0 && response.StatusCode == http.StatusOK {
		start = 0
	}
	total := response.ContentLength
	if total >= 0 {
		total += start
	} else {
		total = 0
	}
	flags := os.O_CREATE | os.O_WRONLY
	if start == 0 {
		flags |= os.O_TRUNC
	} else {
		flags |= os.O_APPEND
	}
	if response.ContentLength > 0 {
		if err = ensureFreeSpace(filepath.Dir(partial), response.ContentLength); err != nil {
			return err
		}
	}
	file, err := os.OpenFile(partial, flags, 0600)
	if err != nil {
		return err
	}
	if _, err = s.database.UpdateLocalModel(context.Background(), value.ID, "downloading", start, total, 0, "", "", "", "", time.Now()); err != nil {
		_ = file.Close()
		return err
	}
	s.publishDurable(context.Background())
	completed, last := start, start
	buffer := make([]byte, 1<<20)
	for {
		read, readErr := response.Body.Read(buffer)
		if read > 0 {
			written, writeErr := file.Write(buffer[:read])
			completed += int64(written)
			if writeErr != nil || written != read {
				_ = file.Close()
				if writeErr == nil {
					writeErr = io.ErrShortWrite
				}
				return writeErr
			}
			if completed-last >= transferNoticeBytes {
				_, _ = s.database.UpdateLocalModel(context.Background(), value.ID, "downloading", completed, total, 0, "", "", "", "", time.Now())
				s.publishDurable(context.Background())
				last = completed
			}
		}
		if readErr != nil {
			if errors.Is(readErr, io.EOF) {
				break
			}
			_ = file.Close()
			return readErr
		}
		if err := ctx.Err(); err != nil {
			_ = file.Close()
			return err
		}
	}
	if err = file.Sync(); err != nil {
		_ = file.Close()
		return err
	}
	if err = file.Close(); err != nil {
		return err
	}
	return s.verifyAndPublish(ctx, value, partial, expected, completed, total, prepare)
}

func (s *Service) copyLocal(ctx context.Context, value store.LocalModelInstallation, source string) error {
	input, err := os.Open(source)
	if err != nil {
		return err
	}
	defer input.Close()
	partial := s.partialPath(value.ID)
	defer func() {
		if ctx.Err() != nil {
			_ = os.Remove(partial)
		}
	}()
	if err = os.MkdirAll(filepath.Dir(partial), 0700); err != nil {
		return err
	}
	if err = ensureFreeSpace(filepath.Dir(partial), value.TotalBytes); err != nil {
		return err
	}
	output, err := os.OpenFile(partial, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, 0600)
	if err != nil {
		return err
	}
	if _, err = s.database.UpdateLocalModel(context.Background(), value.ID, "downloading", 0, value.TotalBytes, 0, "", "", "", "", time.Now()); err != nil {
		_ = output.Close()
		return err
	}
	completed, last := int64(0), int64(0)
	buffer := make([]byte, 1<<20)
	for {
		read, readErr := input.Read(buffer)
		if read > 0 {
			written, writeErr := output.Write(buffer[:read])
			completed += int64(written)
			if writeErr != nil || written != read {
				_ = output.Close()
				if writeErr == nil {
					writeErr = io.ErrShortWrite
				}
				return writeErr
			}
			if completed-last >= transferNoticeBytes {
				_, _ = s.database.UpdateLocalModel(context.Background(), value.ID, "downloading", completed, value.TotalBytes, 0, "", "", "", "", time.Now())
				s.publishDurable(context.Background())
				last = completed
			}
		}
		if readErr != nil {
			if errors.Is(readErr, io.EOF) {
				break
			}
			_ = output.Close()
			return readErr
		}
		if err = ctx.Err(); err != nil {
			_ = output.Close()
			return err
		}
	}
	if err = output.Sync(); err != nil {
		_ = output.Close()
		return err
	}
	if err = output.Close(); err != nil {
		return err
	}
	return s.verifyAndPublish(ctx, value, partial, "", completed, value.TotalBytes, false)
}

func (s *Service) verifyAndPublish(ctx context.Context, value store.LocalModelInstallation, partial, expected string, completed, total int64, prepare bool) error {
	if _, err := s.database.UpdateLocalModel(context.Background(), value.ID, "verifying", completed, total, 0, "", "", "", "", time.Now()); err != nil {
		return err
	}
	s.publishDurable(context.Background())
	digest, size, err := hashGGUF(ctx, partial)
	if err != nil {
		return err
	}
	if expected != "" && digest != expected {
		if removeErr := os.Remove(partial); removeErr != nil && !errors.Is(removeErr, os.ErrNotExist) {
			return fmt.Errorf("%w; remove corrupt partial: %v", errLocalModelChecksum, removeErr)
		}
		return errLocalModelChecksum
	}
	directory := filepath.Join(s.home, "models", "blobs")
	if err = os.MkdirAll(directory, 0700); err != nil {
		return err
	}
	target := filepath.Join(directory, digest+".gguf")
	if _, err = os.Stat(target); err == nil {
		existing, _, hashErr := hashGGUF(ctx, target)
		if hashErr == nil && existing == digest {
			_ = os.Remove(partial)
		} else {
			if err = ctx.Err(); err != nil {
				return err
			}
			installations, listErr := s.database.LocalModelInstallations(ctx)
			if listErr != nil {
				return listErr
			}
			referenced := false
			for _, installation := range installations {
				if installation.BlobPath == filepath.ToSlash(filepath.Join("models", "blobs", digest+".gguf")) {
					referenced = true
					break
				}
			}
			if referenced {
				return errors.New("blob digest conflict")
			}
			if removeErr := os.Remove(target); removeErr != nil {
				return removeErr
			}
			if err = os.Rename(partial, target); err != nil {
				return err
			}
		}
		_ = os.Remove(partial)
	} else if !errors.Is(err, os.ErrNotExist) {
		return err
	} else if err = os.Rename(partial, target); err != nil {
		return err
	}
	relative := filepath.ToSlash(filepath.Join("models", "blobs", digest+".gguf"))
	installed, err := s.database.UpdateLocalModel(context.Background(), value.ID, "installed", size, size, size, digest, relative, "", "", time.Now())
	if err != nil {
		return err
	}
	s.publishDurable(context.Background())
	if prepare {
		items, _ := s.database.LocalModelInstallations(context.Background())
		active := false
		for _, item := range items {
			active = active || item.Active
		}
		if !active {
			if startErr := s.startRuntime(ctx, installed); startErr == nil {
				_, err = s.database.ActivateLocalModel(context.Background(), installed.ID, false, time.Now())
				s.publishDurable(context.Background())
			}
		}
	}
	return err
}

func hashGGUF(ctx context.Context, path string) (string, int64, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", 0, err
	}
	defer file.Close()
	info, err := file.Stat()
	if err != nil || !info.Mode().IsRegular() || info.Size() < 4 {
		return "", 0, errors.New("model artifact is not a regular GGUF")
	}
	magic := make([]byte, 4)
	if _, err = io.ReadFull(file, magic); err != nil || string(magic) != "GGUF" {
		return "", 0, errors.New("model artifact does not have a GGUF header")
	}
	hash := sha256.New()
	_, _ = hash.Write(magic)
	if _, err = io.Copy(hash, &contextReader{ctx: ctx, reader: file}); err != nil {
		return "", 0, err
	}
	return hex.EncodeToString(hash.Sum(nil)), info.Size(), nil
}

func hashFile(ctx context.Context, path string) (string, int64, error) {
	file, err := os.Open(path)
	if err != nil {
		return "", 0, err
	}
	defer file.Close()
	info, err := file.Stat()
	if err != nil || !info.Mode().IsRegular() {
		return "", 0, errors.New("artifact is not a regular file")
	}
	hash := sha256.New()
	if _, err := io.Copy(hash, &contextReader{ctx: ctx, reader: file}); err != nil {
		return "", 0, err
	}
	return hex.EncodeToString(hash.Sum(nil)), info.Size(), nil
}

type contextReader struct {
	ctx    context.Context
	reader io.Reader
}

func (r *contextReader) Read(p []byte) (int, error) {
	if err := r.ctx.Err(); err != nil {
		return 0, err
	}
	return r.reader.Read(p)
}

func regularGGUF(path string) (os.FileInfo, error) {
	if !filepath.IsAbs(path) || !strings.EqualFold(filepath.Ext(path), ".gguf") {
		return nil, errors.New("local import must reference an absolute .gguf file")
	}
	info, err := os.Lstat(path)
	if err != nil {
		return nil, err
	}
	if !info.Mode().IsRegular() || info.Size() < 4 {
		return nil, errors.New("local import must reference a non-empty regular GGUF")
	}
	file, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer file.Close()
	magic := make([]byte, 4)
	_, err = io.ReadFull(file, magic)
	if err != nil || string(magic) != "GGUF" {
		return nil, errors.New("model artifact does not have a GGUF header")
	}
	return info, nil
}

func validateRemote(repo, revision, file, digest string) error {
	parts := strings.Split(repo, "/")
	if len(parts) != 2 || parts[0] == "" || parts[1] == "" {
		return errors.New("Hugging Face repository must use owner/repository form")
	}
	if err := provider.ValidateLocalModelRevision(revision); err != nil {
		return err
	}
	if !safeGGUFPath(file) {
		return errors.New("Hugging Face artifact path is invalid")
	}
	if err := provider.ValidateLocalModelSHA256(digest); err != nil {
		return err
	}
	return nil
}
func safeGGUFPath(file string) bool {
	if !strings.HasSuffix(strings.ToLower(file), ".gguf") {
		return false
	}
	for _, part := range strings.Split(filepath.ToSlash(file), "/") {
		if part == "" || part == "." || part == ".." {
			return false
		}
	}
	return true
}
func validDigest(value string) bool {
	return provider.ValidateLocalModelSHA256(value) == nil
}
func (s *Service) huggingFaceURL(repo, revision, file string) string {
	parts := strings.Split(repo, "/")
	segments := []string{"https://huggingface.co", url.PathEscape(parts[0]), url.PathEscape(parts[1]), "resolve", url.PathEscape(revision)}
	for _, part := range strings.Split(filepath.ToSlash(file), "/") {
		segments = append(segments, url.PathEscape(part))
	}
	return strings.Join(segments, "/") + "?download=true"
}
func (s *Service) partialPath(id string) string {
	sum := sha256.Sum256([]byte(id))
	return filepath.Join(s.home, "system", "tmp", "local-model-"+hex.EncodeToString(sum[:8])+".partial")
}
func modelID(name string) string {
	var b strings.Builder
	dash := false
	for _, r := range strings.ToLower(strings.TrimSpace(name)) {
		if (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') {
			b.WriteRune(r)
			dash = false
		} else if !dash && b.Len() > 0 {
			b.WriteByte('-')
			dash = true
		}
	}
	value := strings.Trim(b.String(), "-")
	if value == "" {
		return "imported-model"
	}
	return value
}
func localFileID(path string, info os.FileInfo) string {
	sum := sha256.Sum256([]byte(filepath.Clean(path) + ":" + strconv.FormatInt(info.Size(), 10) + ":" + strconv.FormatInt(info.ModTime().UnixNano(), 10)))
	return "local_model_installation:local:" + hex.EncodeToString(sum[:6])
}

func installationErrorCode(err error) string {
	if errors.Is(err, errLocalModelChecksum) {
		return "checksum_mismatch"
	}
	return "installation_failed"
}
