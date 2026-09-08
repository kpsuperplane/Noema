package mcp

import (
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"regexp"

	"github.com/kpsuperplane/noema/internal/home"
)

const secretFileLimit = 2 << 20

var protectedID = regexp.MustCompile(`^(mcp_server|mcp_oauth):[0-9a-f]{32}$`)

type secretStore struct{ root string }

func newSecretStore(root string) (*secretStore, error) {
	if !filepath.IsAbs(root) {
		return nil, errors.New("MCP secret root must be absolute")
	}
	return &secretStore{root: filepath.Clean(root)}, nil
}

func (s *secretStore) connectionPath(id string) (string, error) {
	if !protectedID.MatchString(id) || len(id) != 43 {
		return "", errors.New("invalid MCP server id")
	}
	return filepath.Join(s.root, "mcp", id, "credentials.json"), nil
}

func (s *secretStore) attemptPath(id string) (string, error) {
	if !protectedID.MatchString(id) || len(id) != 42 {
		return "", errors.New("invalid MCP OAuth attempt id")
	}
	return filepath.Join(s.root, "mcp", "oauth", id+".json"), nil
}

func (s *secretStore) writeConnection(id string, value SecretMaterial) error {
	path, err := s.connectionPath(id)
	if err != nil {
		return err
	}
	return writeSecretMaterial(path, value)
}

func (s *secretStore) loadConnection(id string) (SecretMaterial, error) {
	path, err := s.connectionPath(id)
	if err != nil {
		return SecretMaterial{}, err
	}
	return readSecretMaterial(path)
}

func (s *secretStore) removeConnection(id string) error {
	path, err := s.connectionPath(id)
	if err != nil {
		return err
	}
	if err := home.RemovePrivateFile(path); err != nil {
		return err
	}
	// Keep the per-connection secret home ephemeral after its only file is
	// removed. The parent MCP directory contains other connections and stays.
	if err := os.Remove(filepath.Dir(path)); err != nil && !errors.Is(err, os.ErrNotExist) {
		return fmt.Errorf("remove MCP secret directory: %w", err)
	}
	return nil
}

func (s *secretStore) writeAttempt(id string, value SecretMaterial) error {
	path, err := s.attemptPath(id)
	if err != nil {
		return err
	}
	return writeSecretMaterial(path, value)
}

func (s *secretStore) loadAttempt(id string) (SecretMaterial, error) {
	path, err := s.attemptPath(id)
	if err != nil {
		return SecretMaterial{}, err
	}
	return readSecretMaterial(path)
}

func (s *secretStore) removeAttempt(id string) error {
	path, err := s.attemptPath(id)
	if err != nil {
		return err
	}
	return home.RemovePrivateFile(path)
}

func (s *secretStore) cleanupAttempts() error {
	directory := filepath.Join(s.root, "mcp", "oauth")
	entries, err := os.ReadDir(directory)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	for _, entry := range entries {
		if entry.IsDir() || filepath.Ext(entry.Name()) != ".json" {
			continue
		}
		id := entry.Name()[:len(entry.Name())-5]
		if protectedID.MatchString(id) && len(id) == 42 {
			if err := home.RemovePrivateFile(filepath.Join(directory, entry.Name())); err != nil {
				return err
			}
		}
	}
	return nil
}

func (s *secretStore) cleanupConnections(valid map[string]struct{}) error {
	directory := filepath.Join(s.root, "mcp")
	entries, err := os.ReadDir(directory)
	if errors.Is(err, os.ErrNotExist) {
		return nil
	}
	if err != nil {
		return err
	}
	for _, entry := range entries {
		id := entry.Name()
		if !entry.IsDir() || !protectedID.MatchString(id) || len(id) != 43 {
			continue
		}
		if _, exists := valid[id]; exists {
			continue
		}
		path, pathErr := s.connectionPath(id)
		if pathErr != nil {
			return pathErr
		}
		if err := home.RemovePrivateFile(path); err != nil {
			return err
		}
	}
	return nil
}

func writeSecretMaterial(path string, value SecretMaterial) error {
	if value.Revision == "" {
		return errors.New("MCP secret revision is missing")
	}
	encoded, err := json.Marshal(value)
	if err != nil || len(encoded) > secretFileLimit {
		return errors.New("MCP secret material is invalid")
	}
	return home.AtomicWritePrivate(path, encoded)
}

func readSecretMaterial(path string) (SecretMaterial, error) {
	data, err := home.ReadPrivateFile(path, secretFileLimit)
	if err != nil {
		return SecretMaterial{}, err
	}
	var value SecretMaterial
	if json.Unmarshal(data, &value) != nil || value.Revision == "" {
		return SecretMaterial{}, errors.New("MCP secret material is invalid")
	}
	return value, nil
}
