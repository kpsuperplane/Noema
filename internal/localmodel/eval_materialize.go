package localmodel

import (
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"

	"github.com/kpsuperplane/noema/internal/provider"
)

// VerifiedEvalModelSource identifies one immutable public evaluation artifact.
type VerifiedEvalModelSource struct {
	Repo, Revision, File string
}

// MaterializeVerifiedEvalModelRequest describes one store-free evaluation cache entry.
type MaterializeVerifiedEvalModelRequest struct {
	Source        VerifiedEvalModelSource
	ExpectedBytes int64
	SHA256        string
	CacheRoot     string
}

// MaterializeVerifiedEvalModel returns a verified content-addressed evaluation
// blob without creating or changing a local-model installation.
func (s *Service) MaterializeVerifiedEvalModel(ctx context.Context, request MaterializeVerifiedEvalModelRequest) (string, error) {
	if err := validateEvalMaterializeRequest(request); err != nil {
		return "", err
	}
	s.evalMu.Lock()
	defer s.evalMu.Unlock()
	return s.materializeVerifiedEvalModelFromURL(ctx, request, s.huggingFaceURL(request.Source.Repo, request.Source.Revision, request.Source.File))
}

func validateEvalMaterializeRequest(request MaterializeVerifiedEvalModelRequest) error {
	if request.ExpectedBytes <= 0 {
		return errors.New("expected byte count must be greater than zero")
	}
	if err := provider.ValidateLocalModelSHA256(request.SHA256); err != nil {
		return errors.New("SHA-256 must be lowercase hexadecimal")
	}
	if !filepath.IsAbs(request.CacheRoot) || strings.TrimSpace(request.CacheRoot) == "" {
		return errors.New("cache root is required")
	}
	parts := strings.Split(strings.Trim(request.Source.Repo, "/"), "/")
	if len(parts) != 2 || parts[0] == "" || parts[1] == "" {
		return errors.New("Hugging Face repository must use owner/repository form")
	}
	if err := provider.ValidateLocalModelRevision(request.Source.Revision); err != nil {
		return err
	}
	if !safeGGUFPath(request.Source.File) {
		return errors.New("Hugging Face artifact path is invalid")
	}
	return nil
}

func (s *Service) materializeVerifiedEvalModelFromURL(ctx context.Context, request MaterializeVerifiedEvalModelRequest, source string) (string, error) {
	if err := ctx.Err(); err != nil {
		return "", err
	}
	if err := os.MkdirAll(request.CacheRoot, 0o700); err != nil {
		return "", fmt.Errorf("evaluation materialization: %w", err)
	}
	destination := filepath.Join(request.CacheRoot, request.SHA256+".gguf")
	if valid, err := verifiedEvalCacheEntry(ctx, destination, request); err != nil {
		return "", err
	} else if valid {
		return destination, nil
	}
	_ = os.Remove(destination)
	partial := filepath.Join(request.CacheRoot, "."+request.SHA256+".partial")
	_ = os.Remove(partial)
	if err := s.downloadVerifiedEvalModel(ctx, request, source, partial, destination); err != nil {
		_ = os.Remove(partial)
		return "", err
	}
	return destination, nil
}

func verifiedEvalCacheEntry(ctx context.Context, path string, request MaterializeVerifiedEvalModelRequest) (bool, error) {
	info, err := os.Stat(path)
	if errors.Is(err, os.ErrNotExist) {
		return false, nil
	}
	if err != nil {
		return false, fmt.Errorf("evaluation materialization: %w", err)
	}
	if !info.Mode().IsRegular() || info.Size() != request.ExpectedBytes {
		return false, nil
	}
	digest, _, err := hashGGUF(ctx, path)
	if err != nil {
		if errors.Is(err, context.Canceled) {
			return false, context.Canceled
		}
		return false, nil
	}
	return digest == request.SHA256, nil
}

func (s *Service) downloadVerifiedEvalModel(ctx context.Context, request MaterializeVerifiedEvalModelRequest, source, partial, destination string) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, source, nil)
	if err != nil {
		return fmt.Errorf("evaluation materialization: %w", err)
	}
	response, err := s.client.Do(req)
	if err != nil {
		if errors.Is(err, context.Canceled) {
			return context.Canceled
		}
		return fmt.Errorf("evaluation materialization: %w", err)
	}
	defer response.Body.Close()
	if response.StatusCode < http.StatusOK || response.StatusCode >= http.StatusMultipleChoices {
		return fmt.Errorf("evaluation materialization: source returned HTTP %d", response.StatusCode)
	}
	if response.ContentLength >= 0 && response.ContentLength != request.ExpectedBytes {
		return errors.New("evaluation materialization: source content length did not match the pinned byte count")
	}
	output, err := os.OpenFile(partial, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, 0o600)
	if err != nil {
		return fmt.Errorf("evaluation materialization: %w", err)
	}
	written, copyErr := io.Copy(output, io.LimitReader(response.Body, request.ExpectedBytes+1))
	syncErr := output.Sync()
	closeErr := output.Close()
	if copyErr != nil {
		return fmt.Errorf("evaluation materialization: %w", copyErr)
	}
	if syncErr != nil {
		return fmt.Errorf("evaluation materialization: %w", syncErr)
	}
	if closeErr != nil {
		return fmt.Errorf("evaluation materialization: %w", closeErr)
	}
	if written > request.ExpectedBytes {
		return errors.New("evaluation materialization: source exceeded the pinned byte count")
	}
	if written != request.ExpectedBytes {
		return fmt.Errorf("evaluation materialization: download ended at %d bytes instead of %d", written, request.ExpectedBytes)
	}
	digest, _, err := hashGGUF(ctx, partial)
	if err != nil {
		return fmt.Errorf("evaluation materialization: %w", err)
	}
	if digest != request.SHA256 {
		return errors.New("downloaded artifact did not match its pinned SHA-256")
	}
	if err := os.Rename(partial, destination); err != nil {
		return fmt.Errorf("evaluation materialization: %w", err)
	}
	return nil
}
