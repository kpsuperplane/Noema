// Package memory owns the local human's native Markdown Memory tree.
package memory

import (
	"crypto/sha256"
	"encoding/hex"
	"errors"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path"
	"slices"
	"strconv"
	"strings"
	"sync"
	"unicode"
	"unicode/utf8"

	noemahome "github.com/kpsuperplane/noema/internal/home"
	"github.com/rivo/uniseg"
)

const (
	Owner          = "human:local"
	Scope          = "human:local"
	RootPagePath   = "root.md"
	MaxWords       = 750
	memoryRootPath = "memory/human"
	pageReadLimit  = 1 << 20
)

var (
	ErrInvalidPage  = errors.New("Memory page is invalid")
	ErrPageNotFound = errors.New("Memory page was not found")
)

var PageIconKeys = []string{
	"brain", "file-text", "user", "users", "heart", "house",
	"briefcase-business", "graduation-cap", "book-open", "lightbulb",
	"target", "calendar-days", "map-pin", "plane", "heart-pulse",
	"dumbbell", "utensils", "music", "palette", "camera", "gamepad-2",
	"mountain", "paw-print", "code-2", "wallet-cards", "sparkles",
	"compass", "notebook-pen",
}

type Citation struct {
	Sources []string `json:"sources"`
}

type PageRef struct {
	ID, Path, Title, Icon, Excerpt, Hash string
}

type Page struct {
	ID, Path, Title, Icon, Body, Hash string
	Citations                         []Citation
	Parent                            string
	Ancestors, Children               []PageRef
}

type State struct {
	ConversationID           string `json:"conversation_id,omitempty"`
	LastConsolidatedSequence int64  `json:"last_consolidated_sequence"`
	LastConsolidatedItem     string `json:"last_consolidated_item,omitempty"`
	UpdatedAt                string `json:"updated_at,omitempty"`
}

type PageChange struct {
	ID, ExpectedHash, Path, Title, Icon, Body string
	Citations                                 []Citation
}

type ChangeSet struct {
	Upserts []PageChange
	Deletes []string
}

type Store struct {
	root *os.Root
	mu   sync.RWMutex
}

func New(home *os.Root) (*Store, error) {
	if home == nil {
		return nil, errors.New("Memory home is unavailable")
	}
	if err := home.MkdirAll(memoryRootPath+"/.pending", 0o700); err != nil {
		return nil, fmt.Errorf("create Memory root: %w", err)
	}
	for _, directory := range []string{memoryRootPath, "memory", "."} {
		if err := noemahome.SyncRootDirectory(home, directory); err != nil {
			return nil, fmt.Errorf("sync Memory root: %w", err)
		}
	}
	root, err := home.OpenRoot(memoryRootPath)
	if err != nil {
		return nil, fmt.Errorf("open Memory root: %w", err)
	}
	store := &Store{root: root}
	if err := store.initialize(); err != nil {
		_ = root.Close()
		return nil, err
	}
	return store, nil
}

func (s *Store) Close() error {
	if s == nil || s.root == nil {
		return nil
	}
	return s.root.Close()
}

func (s *Store) ReadRoot() (Page, error) {
	return s.ReadPage(RootPagePath)
}

func (s *Store) ReadPage(selector string) (Page, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	pages, err := s.pages()
	if err != nil {
		return Page{}, err
	}
	for _, page := range pages {
		if page.Path == selector || page.ID == selector {
			return page, nil
		}
	}
	return Page{}, ErrPageNotFound
}

func (s *Store) ListPages() ([]Page, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.pages()
}

func (s *Store) State() (State, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return s.state()
}

func (s *Store) initialize() error {
	s.mu.Lock()
	defer s.mu.Unlock()
	if _, err := s.root.Stat(RootPagePath); errors.Is(err, os.ErrNotExist) {
		blank, renderErr := renderNewPage(PageChange{
			Path: RootPagePath, Title: "Human memory", Icon: "user",
		}, RootPagePath, "", "")
		if renderErr != nil {
			return renderErr
		}
		if err := writeRootFile(s.root, RootPagePath, blank); err != nil {
			return fmt.Errorf("create root Memory page: %w", err)
		}
	} else if err != nil {
		return fmt.Errorf("inspect root Memory page: %w", err)
	}
	if err := s.recoverPending(); err != nil {
		return fmt.Errorf("recover Memory publication: %w", err)
	}
	_, err := s.pages()
	return err
}

type parsedPage struct {
	ID, Path, Title, Icon, Body, Hash, CreatedAt string
	Citations                                    []Citation
}

func (s *Store) pages() ([]Page, error) {
	paths := make([]string, 0)
	err := fs.WalkDir(s.root.FS(), ".", func(name string, entry fs.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.IsDir() && name != "." && strings.HasPrefix(entry.Name(), ".") {
			return fs.SkipDir
		}
		if entry.IsDir() || path.Ext(name) != ".md" || entry.Name() == ".state.md" {
			return nil
		}
		paths = append(paths, name)
		return nil
	})
	if err != nil {
		return nil, fmt.Errorf("walk Memory pages: %w", err)
	}
	slices.Sort(paths)
	parsed := make([]parsedPage, 0, len(paths))
	byPath := make(map[string]parsedPage, len(paths))
	ids := make(map[string]string, len(paths))
	for _, pagePath := range paths {
		page, err := s.parsePage(pagePath)
		if err != nil {
			return nil, err
		}
		if other, exists := ids[page.ID]; exists {
			return nil, fmt.Errorf("%w: ID %s is shared by %s and %s", ErrInvalidPage, page.ID, other, page.Path)
		}
		ids[page.ID] = page.Path
		byPath[page.Path] = page
		parsed = append(parsed, page)
	}
	if _, exists := byPath[RootPagePath]; !exists {
		return nil, fmt.Errorf("%w: %s is missing", ErrInvalidPage, RootPagePath)
	}
	result := make([]Page, 0, len(parsed))
	for _, current := range parsed {
		parent := parentPagePath(current.Path)
		if parent != "" {
			if _, exists := byPath[parent]; !exists {
				return nil, fmt.Errorf("%w: %s requires %s", ErrInvalidPage, current.Path, parent)
			}
		}
		ancestors := make([]PageRef, 0)
		for ancestor := parent; ancestor != ""; ancestor = parentPagePath(ancestor) {
			ancestors = append(ancestors, pageReference(byPath[ancestor]))
		}
		slices.Reverse(ancestors)
		children := make([]PageRef, 0)
		for _, candidate := range parsed {
			candidateParent := parentPagePath(candidate.Path)
			if candidate.Path == RootPagePath {
				continue
			}
			if candidateParent == current.Path || current.Path == RootPagePath && candidateParent == "" {
				children = append(children, pageReference(candidate))
			}
		}
		result = append(result, Page{
			ID: current.ID, Path: current.Path, Title: current.Title, Icon: current.Icon,
			Body: current.Body, Hash: current.Hash, Citations: current.Citations,
			Parent: parent, Ancestors: ancestors, Children: children,
		})
	}
	return result, nil
}

func (s *Store) parsePage(pagePath string) (parsedPage, error) {
	if _, err := normalizePagePath(pagePath); err != nil {
		return parsedPage{}, err
	}
	info, err := s.root.Lstat(pagePath)
	if err != nil || !info.Mode().IsRegular() {
		return parsedPage{}, fmt.Errorf("%w: %s is not a regular file", ErrInvalidPage, pagePath)
	}
	file, err := s.root.Open(pagePath)
	if err != nil {
		return parsedPage{}, err
	}
	defer file.Close()
	content, err := io.ReadAll(io.LimitReader(file, pageReadLimit+1))
	if err != nil {
		return parsedPage{}, err
	}
	if len(content) > pageReadLimit || !utf8.Valid(content) {
		return parsedPage{}, fmt.Errorf("%w: %s is too large or invalid UTF-8", ErrInvalidPage, pagePath)
	}
	fields, article, err := splitFrontmatter(string(content))
	if err != nil {
		return parsedPage{}, fmt.Errorf("%w: %s: %v", ErrInvalidPage, pagePath, err)
	}
	if fields["schema"] != "noema.memory.page/v2" || fields["owner"] != Owner || fields["scope"] != Scope {
		return parsedPage{}, fmt.Errorf("%w: %s has an invalid schema, owner, or scope", ErrInvalidPage, pagePath)
	}
	id, title, icon := fields["id"], fields["title"], fields["icon"]
	if id == "" || invalidFrontmatterValue(title) || !slices.Contains(PageIconKeys, icon) {
		return parsedPage{}, fmt.Errorf("%w: %s has invalid metadata", ErrInvalidPage, pagePath)
	}
	article = strings.TrimSpace(article)
	heading := "# " + title
	if article == heading {
		article = ""
	} else if strings.HasPrefix(article, heading+"\n") {
		article = strings.TrimSpace(strings.TrimPrefix(article, heading+"\n"))
	} else {
		return parsedPage{}, fmt.Errorf("%w: %s has no matching title heading", ErrInvalidPage, pagePath)
	}
	body, citations, err := splitCitations(article)
	if err != nil {
		return parsedPage{}, fmt.Errorf("%w: %s: %v", ErrInvalidPage, pagePath, err)
	}
	if unicodeWordCount("# "+title+"\n\n"+article) > MaxWords {
		return parsedPage{}, fmt.Errorf("%w: %s exceeds %d words", ErrInvalidPage, pagePath, MaxWords)
	}
	digest := sha256.Sum256(content)
	return parsedPage{
		ID: id, Path: pagePath, Title: title, Icon: icon, Body: body,
		Hash: hex.EncodeToString(digest[:]), CreatedAt: fields["created_at"], Citations: citations,
	}, nil
}

func splitFrontmatter(content string) (map[string]string, string, error) {
	lines := strings.Split(strings.ReplaceAll(content, "\r\n", "\n"), "\n")
	if len(lines) == 0 || lines[0] != "---" {
		return nil, "", errors.New("frontmatter is missing")
	}
	fields := make(map[string]string)
	for index := 1; index < len(lines); index++ {
		if lines[index] == "---" {
			return fields, strings.Join(lines[index+1:], "\n"), nil
		}
		key, value, ok := strings.Cut(lines[index], ":")
		if ok {
			fields[strings.TrimSpace(key)] = strings.TrimSpace(value)
		}
	}
	return nil, "", errors.New("frontmatter is not terminated")
}

func splitCitations(article string) (string, []Citation, error) {
	lines := strings.Split(article, "\n")
	definitions := make(map[int]Citation)
	retained := make([]string, 0, len(lines))
	for _, line := range lines {
		label, targets, ok := citationDefinition(line)
		if !ok {
			retained = append(retained, line)
			continue
		}
		index, err := strconv.Atoi(label)
		sources := strings.Fields(targets)
		if err != nil || index < 1 || len(sources) == 0 {
			return "", nil, errors.New("citation definition is invalid")
		}
		if _, exists := definitions[index]; exists {
			return "", nil, errors.New("citation definition is duplicated")
		}
		definitions[index] = Citation{Sources: sources}
	}
	citations := make([]Citation, len(definitions))
	for index := 1; index <= len(definitions); index++ {
		citation, exists := definitions[index]
		if !exists {
			return "", nil, errors.New("citation definitions are not consecutive")
		}
		citations[index-1] = citation
	}
	body := strings.TrimSpace(strings.Join(retained, "\n"))
	if err := validateCitations(body, citations); err != nil {
		return "", nil, err
	}
	return body, citations, nil
}

func citationDefinition(line string) (string, string, bool) {
	if !strings.HasPrefix(line, "[^") {
		return "", "", false
	}
	label, rest, ok := strings.Cut(strings.TrimPrefix(line, "[^"), "]:")
	return label, rest, ok
}

func validateCitations(body string, citations []Citation) error {
	references := make(map[int]bool)
	for remaining := body; ; {
		start := strings.Index(remaining, "[^")
		if start < 0 {
			break
		}
		remaining = remaining[start+2:]
		end := strings.IndexByte(remaining, ']')
		if end < 0 {
			return errors.New("citation marker is not terminated")
		}
		index, err := strconv.Atoi(remaining[:end])
		if err != nil || index < 1 {
			return errors.New("citation marker is invalid")
		}
		references[index] = true
		remaining = remaining[end+1:]
		if strings.HasPrefix(remaining, ":") {
			return errors.New("article contains a citation definition")
		}
	}
	if len(references) != len(citations) {
		return errors.New("every citation group needs one marker")
	}
	for index, citation := range citations {
		if !references[index+1] || len(citation.Sources) == 0 {
			return errors.New("citation groups and markers do not match")
		}
		seen := make(map[string]bool)
		for _, source := range citation.Sources {
			if source == "" || strings.TrimSpace(source) != source || strings.ContainsAny(source, " \t\r\n") || seen[source] {
				return errors.New("citation source is invalid or duplicated")
			}
			seen[source] = true
		}
	}
	return nil
}

func normalizePagePath(value string) (string, error) {
	if value == "" || strings.Contains(value, "\\") || path.IsAbs(value) || path.Clean(value) != value || path.Ext(value) != ".md" {
		return "", fmt.Errorf("%w: path must be a normalized relative .md path", ErrInvalidPage)
	}
	for _, component := range strings.Split(value, "/") {
		if component == "" || strings.HasPrefix(component, ".") || strings.Contains(component, ":") || strings.IndexFunc(component, unicode.IsControl) >= 0 {
			return "", fmt.Errorf("%w: path contains a reserved component", ErrInvalidPage)
		}
	}
	return value, nil
}

func parentPagePath(value string) string {
	parent := path.Dir(value)
	if parent == "." {
		return ""
	}
	return parent + ".md"
}

func pageReference(page parsedPage) PageRef {
	return PageRef{
		ID: page.ID, Path: page.Path, Title: page.Title, Icon: page.Icon,
		Excerpt: pageExcerpt(page.Body), Hash: page.Hash,
	}
}

// Reference creates the public summary for one loaded page.
func Reference(page Page) PageRef {
	return PageRef{
		ID: page.ID, Path: page.Path, Title: page.Title, Icon: page.Icon,
		Excerpt: pageExcerpt(page.Body), Hash: page.Hash,
	}
}

func pageExcerpt(body string) string {
	lead := ""
	for _, paragraph := range strings.Split(body, "\n\n") {
		paragraph = strings.TrimSpace(paragraph)
		if paragraph != "" && !strings.HasPrefix(paragraph, "#") {
			lead = strings.Join(strings.Fields(paragraph), " ")
			break
		}
	}
	for {
		start := strings.Index(lead, "[^")
		if start < 0 {
			break
		}
		end := strings.IndexByte(lead[start+2:], ']')
		if end < 0 {
			break
		}
		lead = lead[:start] + lead[start+end+3:]
	}
	runes := []rune(lead)
	if len(runes) <= 180 {
		return lead
	}
	return strings.TrimSpace(string(runes[:180])) + "…"
}

func unicodeWordCount(value string) int {
	count, state := 0, -1
	for value != "" {
		word, rest, next := uniseg.FirstWordInString(value, state)
		if strings.IndexFunc(word, func(r rune) bool { return unicode.IsLetter(r) || unicode.IsNumber(r) }) >= 0 {
			count++
		}
		value, state = rest, next
	}
	return count
}

func invalidFrontmatterValue(value string) bool {
	return value == "" || strings.TrimSpace(value) != value || strings.IndexFunc(value, unicode.IsControl) >= 0
}
