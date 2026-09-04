package memory

import (
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"path"
	"slices"
	"strconv"
	"strings"
	"time"
	"unicode"

	noemahome "github.com/kpsuperplane/noema/internal/home"
)

type stagedPage struct {
	Path  string `json:"path"`
	Bytes []byte `json:"bytes"`
}

type pendingPublication struct {
	Pages   []stagedPage `json:"pages"`
	Deletes []string     `json:"deletes"`
	State   State        `json:"state"`
}

// Publish validates and durably applies one complete Memory change set.
func (s *Store) Publish(changes ChangeSet, state State) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if err := s.recoverPending(); err != nil {
		return fmt.Errorf("recover prior Memory publication: %w", err)
	}
	pending, err := s.stage(changes, state)
	if err != nil {
		return err
	}
	operation, err := randomName()
	if err != nil {
		return err
	}
	directory := ".pending/" + operation
	if err := ensureRootDirectory(s.root, directory); err != nil {
		return fmt.Errorf("create Memory publication stage: %w", err)
	}
	payload, err := json.Marshal(pending)
	if err != nil {
		return fmt.Errorf("encode Memory publication: %w", err)
	}
	if err := writeRootFile(s.root, directory+"/changes.json", payload); err != nil {
		return fmt.Errorf("stage Memory publication: %w", err)
	}
	if err := s.apply(pending); err != nil {
		return err
	}
	if err := writeRootFile(s.root, ".state.md", renderState(pending.State)); err != nil {
		return fmt.Errorf("save Memory checkpoint: %w", err)
	}
	if err := s.root.RemoveAll(directory); err != nil {
		return fmt.Errorf("remove Memory publication stage: %w", err)
	}
	return noemahome.SyncRootDirectory(s.root, ".pending")
}

func (s *Store) stage(changes ChangeSet, state State) (pendingPublication, error) {
	if err := validateState(state); err != nil {
		return pendingPublication{}, err
	}
	now := time.Now().UTC().Format(time.RFC3339Nano)
	if state.UpdatedAt == "" {
		state.UpdatedAt = now
	}
	current, err := s.pages()
	if err != nil {
		return pendingPublication{}, err
	}
	byPath := make(map[string]parsedPage, len(current))
	byID := make(map[string]parsedPage, len(current))
	for _, page := range current {
		parsed, err := s.parsePage(page.Path)
		if err != nil {
			return pendingPublication{}, err
		}
		byPath[page.Path], byID[page.ID] = parsed, parsed
	}
	seenPaths := make(map[string]bool)
	deleteSet := make(map[string]bool)
	for _, candidate := range changes.Deletes {
		pagePath, err := normalizePagePath(candidate)
		if err != nil {
			return pendingPublication{}, err
		}
		if pagePath == RootPagePath {
			return pendingPublication{}, fmt.Errorf("%w: root.md cannot be deleted", ErrInvalidPage)
		}
		if seenPaths[pagePath] {
			return pendingPublication{}, fmt.Errorf("%w: duplicate change for %s", ErrInvalidPage, pagePath)
		}
		seenPaths[pagePath], deleteSet[pagePath] = true, true
	}
	staged := make([]stagedPage, 0, len(changes.Upserts))
	stagedIDs := make(map[string]bool)
	for _, change := range changes.Upserts {
		pagePath, err := normalizePagePath(change.Path)
		if err != nil {
			return pendingPublication{}, err
		}
		if seenPaths[pagePath] {
			return pendingPublication{}, fmt.Errorf("%w: duplicate change for %s", ErrInvalidPage, pagePath)
		}
		seenPaths[pagePath] = true
		if err := validatePageChange(change, pagePath); err != nil {
			return pendingPublication{}, err
		}
		currentPage, exists := byPath[pagePath]
		if !exists && change.ID != "" {
			currentPage, exists = byID[change.ID]
		}
		if exists {
			if change.ID != currentPage.ID || change.ExpectedHash != currentPage.Hash {
				return pendingPublication{}, fmt.Errorf("%w: exact ID and hash are required for %s", ErrInvalidPage, currentPage.Path)
			}
			if occupied, occupiedExists := byPath[pagePath]; occupiedExists && occupied.ID != currentPage.ID {
				return pendingPublication{}, fmt.Errorf("%w: %s already belongs to another page", ErrInvalidPage, pagePath)
			}
			if currentPage.Path != pagePath {
				if currentPage.Path == RootPagePath {
					return pendingPublication{}, fmt.Errorf("%w: root.md cannot be moved", ErrInvalidPage)
				}
				deleteSet[currentPage.Path] = true
			}
		} else if change.ID != "" || change.ExpectedHash != "" {
			return pendingPublication{}, fmt.Errorf("%w: new page %s cannot specify an ID or hash", ErrInvalidPage, pagePath)
		}
		pageID := change.ID
		createdAt := ""
		if exists {
			pageID, createdAt = currentPage.ID, currentPage.CreatedAt
		}
		if pageID == "" {
			pageID = "memory:human:" + pagePath
		}
		if stagedIDs[pageID] {
			return pendingPublication{}, fmt.Errorf("%w: duplicate page ID %s", ErrInvalidPage, pageID)
		}
		stagedIDs[pageID] = true
		rendered, err := renderNewPage(change, pagePath, pageID, createdAt, now)
		if err != nil {
			return pendingPublication{}, err
		}
		staged = append(staged, stagedPage{Path: pagePath, Bytes: rendered})
	}

	finalIDs := make(map[string]string)
	finalPaths := make(map[string]bool)
	for _, page := range current {
		if deleteSet[page.Path] || seenPaths[page.Path] {
			continue
		}
		finalPaths[page.Path], finalIDs[page.ID] = true, page.Path
	}
	for _, page := range staged {
		parsed, err := parseRenderedPage(page.Path, page.Bytes)
		if err != nil {
			return pendingPublication{}, err
		}
		if other, exists := finalIDs[parsed.ID]; exists {
			return pendingPublication{}, fmt.Errorf("%w: ID %s is shared by %s and %s", ErrInvalidPage, parsed.ID, other, page.Path)
		}
		finalPaths[page.Path], finalIDs[parsed.ID] = true, page.Path
	}
	if !finalPaths[RootPagePath] {
		return pendingPublication{}, fmt.Errorf("%w: root.md is required", ErrInvalidPage)
	}
	for pagePath := range finalPaths {
		if parent := parentPagePath(pagePath); parent != "" && !finalPaths[parent] {
			return pendingPublication{}, fmt.Errorf("%w: %s requires %s", ErrInvalidPage, pagePath, parent)
		}
	}
	deletes := make([]string, 0, len(deleteSet))
	for pagePath := range deleteSet {
		deletes = append(deletes, pagePath)
	}
	slices.Sort(deletes)
	return pendingPublication{Pages: staged, Deletes: deletes, State: state}, nil
}

func validatePageChange(change PageChange, pagePath string) error {
	if invalidFrontmatterValue(change.Title) || !slices.Contains(PageIconKeys, change.Icon) {
		return fmt.Errorf("%w: %s has invalid title or icon", ErrInvalidPage, pagePath)
	}
	if change.ID != "" && invalidFrontmatterValue(change.ID) {
		return fmt.Errorf("%w: %s has an invalid ID", ErrInvalidPage, pagePath)
	}
	for _, line := range strings.Split(change.Body, "\n") {
		if strings.HasPrefix(line, "# ") {
			return fmt.Errorf("%w: %s repeats its generated title", ErrInvalidPage, pagePath)
		}
	}
	if err := validateCitations(change.Body, change.Citations); err != nil {
		return fmt.Errorf("%w: %s: %v", ErrInvalidPage, pagePath, err)
	}
	if pagePath == RootPagePath && unicodeWordCount(change.Body) >= 120 {
		sections := 0
		for _, line := range strings.Split(change.Body, "\n") {
			if strings.HasPrefix(line, "## ") {
				sections++
			}
		}
		if sections < 2 {
			return fmt.Errorf("%w: developed root.md needs two sections", ErrInvalidPage)
		}
	}
	return nil
}

func renderNewPage(change PageChange, pagePath, pageID, createdAt string, timestamps ...string) ([]byte, error) {
	now := time.Now().UTC().Format(time.RFC3339Nano)
	if len(timestamps) != 0 {
		now = timestamps[0]
	}
	if pageID == "" {
		pageID = "memory:human:" + pagePath
	}
	if createdAt == "" {
		createdAt = now
	}
	body := strings.TrimSpace(change.Body)
	definitions := make([]string, 0, len(change.Citations))
	for index, citation := range change.Citations {
		definitions = append(definitions, fmt.Sprintf("[^%d]: %s", index+1, strings.Join(citation.Sources, " ")))
	}
	if len(definitions) != 0 {
		body += "\n\n" + strings.Join(definitions, "\n")
	}
	rendered := fmt.Sprintf(
		"---\nschema: noema.memory.page/v2\nid: %s\nowner: %s\nscope: %s\ntitle: %s\nicon: %s\ncreated_at: %s\nupdated_at: %s\n---\n\n# %s\n\n%s\n",
		pageID, Owner, Scope, change.Title, change.Icon, createdAt, now, change.Title, body,
	)
	if unicodeWordCount(rendered[strings.Index(rendered, "# "):]) > MaxWords {
		return nil, fmt.Errorf("%w: %s exceeds %d words", ErrInvalidPage, pagePath, MaxWords)
	}
	return []byte(rendered), nil
}

func parseRenderedPage(pagePath string, data []byte) (parsedPage, error) {
	fields, article, err := splitFrontmatter(string(data))
	if err != nil {
		return parsedPage{}, err
	}
	digest := sha256Bytes(data)
	return parsedPage{ID: fields["id"], Path: pagePath, Title: fields["title"], Hash: digest, Body: article}, nil
}

func (s *Store) apply(pending pendingPublication) error {
	for _, pagePath := range pending.Deletes {
		if _, err := normalizePagePath(pagePath); err != nil {
			return err
		}
		if pagePath == RootPagePath {
			return fmt.Errorf("%w: root.md cannot be deleted", ErrInvalidPage)
		}
		if err := s.root.Remove(pagePath); err != nil && !errors.Is(err, os.ErrNotExist) {
			return fmt.Errorf("delete Memory page %s: %w", pagePath, err)
		}
		if err := noemahome.SyncRootDirectory(s.root, path.Dir(pagePath)); err != nil {
			return err
		}
	}
	for _, page := range pending.Pages {
		if _, err := normalizePagePath(page.Path); err != nil {
			return err
		}
		if err := writeRootFile(s.root, page.Path, page.Bytes); err != nil {
			return fmt.Errorf("publish Memory page %s: %w", page.Path, err)
		}
	}
	return nil
}

func (s *Store) recoverPending() error {
	entries, err := fsReadDirectory(s.root, ".pending")
	if err != nil {
		return err
	}
	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}
		directory := ".pending/" + entry.Name()
		data, err := s.root.ReadFile(directory + "/changes.json")
		if errors.Is(err, os.ErrNotExist) {
			if err := s.root.RemoveAll(directory); err != nil {
				return err
			}
			continue
		}
		if err != nil {
			return err
		}
		var pending pendingPublication
		if err := json.Unmarshal(data, &pending); err != nil {
			return fmt.Errorf("decode pending Memory publication: %w", err)
		}
		if err := validateState(pending.State); err != nil {
			return err
		}
		if err := s.apply(pending); err != nil {
			return err
		}
		if err := writeRootFile(s.root, ".state.md", renderState(pending.State)); err != nil {
			return err
		}
		if err := s.root.RemoveAll(directory); err != nil {
			return err
		}
	}
	return noemahome.SyncRootDirectory(s.root, ".pending")
}

func (s *Store) state() (State, error) {
	data, err := s.root.ReadFile(".state.md")
	if errors.Is(err, os.ErrNotExist) {
		return State{}, nil
	}
	if err != nil {
		return State{}, err
	}
	fields, _, err := splitFrontmatter(string(data))
	if err != nil || fields["schema"] != "noema.memory.state/v1" || fields["owner"] != Owner || fields["scope"] != Scope {
		return State{}, fmt.Errorf("%w: Memory checkpoint is invalid", ErrInvalidPage)
	}
	sequence, err := strconv.ParseInt(fields["last_consolidated_sequence"], 10, 64)
	if err != nil || sequence < 0 {
		return State{}, fmt.Errorf("%w: Memory checkpoint sequence is invalid", ErrInvalidPage)
	}
	return State{
		ConversationID: fields["conversation_id"], LastConsolidatedSequence: sequence,
		LastConsolidatedItem: fields["last_consolidated_item"], UpdatedAt: fields["updated_at"],
	}, nil
}

func renderState(state State) []byte {
	updated := state.UpdatedAt
	if updated == "" {
		updated = time.Now().UTC().Format(time.RFC3339Nano)
	}
	return []byte(fmt.Sprintf(
		"---\nschema: noema.memory.state/v1\nowner: %s\nscope: %s\nconversation_id: %s\nlast_consolidated_sequence: %d\nlast_consolidated_item: %s\nupdated_at: %s\n---\n",
		Owner, Scope, state.ConversationID, state.LastConsolidatedSequence,
		state.LastConsolidatedItem, updated,
	))
}

func validateState(state State) error {
	if state.LastConsolidatedSequence < 0 {
		return fmt.Errorf("%w: Memory checkpoint sequence is invalid", ErrInvalidPage)
	}
	for _, value := range []string{state.ConversationID, state.LastConsolidatedItem, state.UpdatedAt} {
		if strings.IndexFunc(value, unicode.IsControl) >= 0 {
			return fmt.Errorf("%w: Memory checkpoint value is invalid", ErrInvalidPage)
		}
	}
	return nil
}

func writeRootFile(root *os.Root, name string, data []byte) error {
	if err := ensureRootDirectory(root, path.Dir(name)); err != nil {
		return err
	}
	suffix, err := randomName()
	if err != nil {
		return err
	}
	temporary := path.Join(path.Dir(name), "."+path.Base(name)+".tmp-"+suffix)
	file, err := root.OpenFile(temporary, os.O_WRONLY|os.O_CREATE|os.O_EXCL, 0o600)
	if err != nil {
		return err
	}
	committed := false
	defer func() {
		_ = file.Close()
		if !committed {
			_ = root.Remove(temporary)
		}
	}()
	if written, err := file.Write(data); err != nil || written != len(data) {
		if err == nil {
			err = io.ErrShortWrite
		}
		return err
	}
	if err := file.Sync(); err != nil {
		return err
	}
	if err := file.Close(); err != nil {
		return err
	}
	if err := noemahome.ReplaceRootFile(root, temporary, name); err != nil {
		return err
	}
	committed = true
	return noemahome.SyncRootDirectory(root, path.Dir(name))
}

func ensureRootDirectory(root *os.Root, name string) error {
	if name == "." {
		return nil
	}
	current := "."
	for _, component := range strings.Split(name, "/") {
		if component == "" || component == "." || component == ".." {
			return errors.New("Memory directory path is invalid")
		}
		next := path.Join(current, component)
		if err := root.Mkdir(next, 0o700); err != nil {
			if !errors.Is(err, os.ErrExist) {
				return err
			}
		} else if err := noemahome.SyncRootDirectory(root, current); err != nil {
			return err
		}
		current = next
	}
	return nil
}

func randomName() (string, error) {
	var value [12]byte
	if _, err := rand.Read(value[:]); err != nil {
		return "", fmt.Errorf("create Memory operation ID: %w", err)
	}
	return hex.EncodeToString(value[:]), nil
}

func sha256Bytes(data []byte) string {
	digest := sha256.Sum256(data)
	return hex.EncodeToString(digest[:])
}

func fsReadDirectory(root *os.Root, name string) ([]os.DirEntry, error) {
	directory, err := root.Open(name)
	if err != nil {
		return nil, err
	}
	defer directory.Close()
	entries, err := directory.ReadDir(-1)
	if err != nil {
		return nil, err
	}
	slices.SortFunc(entries, func(left, right os.DirEntry) int {
		return strings.Compare(left.Name(), right.Name())
	})
	return entries, nil
}
