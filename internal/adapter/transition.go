package adapter

import (
	"encoding/json"
	"errors"
	"os"
	"strings"

	"github.com/kpsuperplane/noema/internal/home"
)

const transitionJournalLimit = 16 << 10

// definitionTransitionJournal records one pending reviewed definition
// replacement. It is separate from the definition projection so restart can
// recover an in-flight transition without trusting model input.
type definitionTransitionJournal struct {
	SchemaVersion   int    `json:"schema_version"`
	DefinitionID    string `json:"definition_id"`
	RequestedDigest string `json:"requested_digest"`
	ReviewedDigest  string `json:"reviewed_digest,omitempty"`
}

type definitionTransitionJournalStore struct{ root *os.Root }

func newDefinitionTransitionJournalStore(root *os.Root) *definitionTransitionJournalStore {
	return &definitionTransitionJournalStore{root: root}
}

func (s *definitionTransitionJournalStore) directory() (*os.Root, error) {
	if s == nil || s.root == nil {
		return nil, errors.New("adapter transition journal is unavailable")
	}
	if err := s.root.MkdirAll("adapters/transitions", 0o700); err != nil {
		return nil, errors.New("adapter transition journal is unavailable")
	}
	return s.root.OpenRoot("adapters/transitions")
}

func (s *definitionTransitionJournalStore) save(journal definitionTransitionJournal) error {
	if journal.SchemaVersion != 1 || !validDigest(journal.RequestedDigest) || journal.DefinitionID == "" || journal.ReviewedDigest != "" && !validDigest(journal.ReviewedDigest) {
		return errors.New("adapter transition journal is invalid")
	}
	raw, err := json.Marshal(journal)
	if err != nil || len(raw) > transitionJournalLimit {
		return errors.New("adapter transition journal is oversized")
	}
	directory, err := s.directory()
	if err != nil {
		return err
	}
	defer directory.Close()
	temporary := ".replace-" + randomHex()
	if err = writeNewFile(directory, temporary, raw); err != nil {
		return err
	}
	if err = directory.Rename(temporary, journal.RequestedDigest); err != nil {
		_ = directory.Remove(temporary)
		return errors.New("adapter transition journal could not be published")
	}
	return home.SyncRootDirectory(directory, ".")
}

func (s *definitionTransitionJournalStore) scan() ([]definitionTransitionJournal, error) {
	directory, err := s.directory()
	if err != nil {
		return nil, err
	}
	defer directory.Close()
	entries, err := readBoundedEntries(directory, ".", definitionLimit)
	if err != nil {
		return nil, err
	}
	result := make([]definitionTransitionJournal, 0, len(entries))
	for _, entry := range entries {
		if strings.HasPrefix(entry.Name(), ".") {
			_ = directory.Remove(entry.Name())
			continue
		}
		if !entry.Type().IsRegular() || entry.Type()&os.ModeSymlink != 0 {
			return nil, errors.New("adapter transition journal is invalid")
		}
		raw, readErr := readRegular(directory, entry.Name(), transitionJournalLimit)
		if readErr != nil {
			return nil, readErr
		}
		var journal definitionTransitionJournal
		if decodeExactJSON(raw, &journal) != nil || journal.SchemaVersion != 1 || journal.RequestedDigest != entry.Name() || !validDigest(journal.RequestedDigest) || journal.ReviewedDigest != "" && !validDigest(journal.ReviewedDigest) {
			return nil, errors.New("adapter transition journal is invalid")
		}
		result = append(result, journal)
	}
	return result, nil
}

func (s *definitionTransitionJournalStore) remove(requestedDigest string) error {
	if !validDigest(requestedDigest) {
		return errors.New("adapter transition journal is invalid")
	}
	directory, err := s.directory()
	if err != nil {
		return err
	}
	defer directory.Close()
	if err = directory.Remove(requestedDigest); err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	return home.SyncRootDirectory(directory, ".")
}
