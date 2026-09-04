package home

import (
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
)

// ReadPrivateFile reads one bounded regular file after enforcing user-only access.
func ReadPrivateFile(path string, limit int64) ([]byte, error) {
	if !filepath.IsAbs(path) || limit < 0 {
		return nil, errors.New("invalid private file request")
	}
	info, err := os.Lstat(path)
	if err != nil {
		return nil, err
	}
	if !info.Mode().IsRegular() {
		return nil, errors.New("private path is not a regular file")
	}
	if err := ProtectFile(path); err != nil {
		return nil, err
	}
	file, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("open private file: %w", err)
	}
	defer file.Close()
	opened, err := file.Stat()
	if err != nil || !os.SameFile(info, opened) {
		return nil, errors.New("private file changed while opening")
	}
	data, err := io.ReadAll(io.LimitReader(file, limit+1))
	if err != nil {
		return nil, fmt.Errorf("read private file: %w", err)
	}
	if int64(len(data)) > limit {
		return nil, errors.New("private file exceeds its size limit")
	}
	return data, nil
}

// AtomicWritePrivate replaces one absolute file with user-only access.
func AtomicWritePrivate(path string, data []byte) error {
	if !filepath.IsAbs(path) {
		return errors.New("private file path must be absolute")
	}
	directory := filepath.Dir(path)
	if err := os.MkdirAll(directory, 0o700); err != nil {
		return fmt.Errorf("create private directory: %w", err)
	}
	if err := protectDirectory(directory); err != nil {
		return fmt.Errorf("protect private directory: %w", err)
	}
	temporary, err := os.CreateTemp(directory, ".noema-write-*")
	if err != nil {
		return fmt.Errorf("create private temporary file: %w", err)
	}
	temporaryName := temporary.Name()
	committed := false
	defer func() {
		_ = temporary.Close()
		if !committed {
			_ = os.Remove(temporaryName)
		}
	}()
	if err := ProtectFile(temporaryName); err != nil {
		return fmt.Errorf("protect private temporary file: %w", err)
	}
	if written, err := temporary.Write(data); err != nil || written != len(data) {
		if err == nil {
			err = io.ErrShortWrite
		}
		return fmt.Errorf("write private temporary file: %w", err)
	}
	if err := temporary.Sync(); err != nil {
		return fmt.Errorf("sync private temporary file: %w", err)
	}
	if err := temporary.Close(); err != nil {
		return fmt.Errorf("close private temporary file: %w", err)
	}
	if err := replacePrivateFile(temporaryName, path); err != nil {
		return fmt.Errorf("replace private file: %w", err)
	}
	committed = true
	return syncAbsoluteDirectory(directory)
}

// RemovePrivateFile removes one absolute regular file when it exists.
func RemovePrivateFile(path string) error {
	if !filepath.IsAbs(path) {
		return errors.New("private file path must be absolute")
	}
	info, err := os.Lstat(path)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return fmt.Errorf("inspect private file: %w", err)
	}
	if !info.Mode().IsRegular() {
		return errors.New("private path is not a regular file")
	}
	if err := os.Remove(path); err != nil {
		return fmt.Errorf("remove private file: %w", err)
	}
	return syncAbsoluteDirectory(filepath.Dir(path))
}
