package runtime

import (
	"encoding/json"
	"errors"
	"fmt"
	"slices"
	"strconv"
	"strings"

	"github.com/google/jsonschema-go/jsonschema"

	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/provider"
)

type modelMemoryChangeSet struct {
	Upserts         *[]modelMemoryPageChange `json:"upserts"`
	MetadataUpdates *[]memoryMetadataUpdate  `json:"metadata_updates"`
	Deletes         *[]string                `json:"deletes"`
}

type modelMemoryPageChange struct {
	ID           *string                 `json:"id"`
	ExpectedHash *string                 `json:"expected_hash"`
	Path         *string                 `json:"path"`
	Title        *string                 `json:"title"`
	Icon         *string                 `json:"icon"`
	Body         *string                 `json:"body"`
	Citations    *[]noemamemory.Citation `json:"citations"`
}

type memoryMetadataUpdate struct {
	Path *string `json:"path"`
	Icon *string `json:"icon"`
}

func parseMemoryChanges(
	result provider.GenerationResult,
	allowed map[string]bool,
	pages []noemamemory.Page,
	editable map[string]bool,
) (noemamemory.ChangeSet, error) {
	if len(result.ToolCalls) != 1 || result.ToolCalls[0].Name != memorySubmitTool {
		return noemamemory.ChangeSet{}, errors.New("expected exactly one native Memory change call")
	}
	var schema jsonschema.Schema
	if err := json.Unmarshal(memoryChangesSchema, &schema); err != nil {
		return noemamemory.ChangeSet{}, fmt.Errorf("invalid Memory schema: %w", err)
	}
	resolved, err := schema.Resolve(nil)
	if err != nil {
		return noemamemory.ChangeSet{}, fmt.Errorf("invalid Memory schema: %w", err)
	}
	var value any
	if err := json.Unmarshal(result.ToolCalls[0].Payload, &value); err != nil {
		return noemamemory.ChangeSet{}, fmt.Errorf("invalid Memory change set: %w", err)
	}
	if err := resolved.Validate(value); err != nil {
		return noemamemory.ChangeSet{}, fmt.Errorf("invalid Memory change set: %w", err)
	}
	var proposed modelMemoryChangeSet
	if err := json.Unmarshal(result.ToolCalls[0].Payload, &proposed); err != nil {
		return noemamemory.ChangeSet{}, fmt.Errorf("invalid Memory change set: %w", err)
	}
	if proposed.Upserts == nil || proposed.MetadataUpdates == nil || proposed.Deletes == nil {
		return noemamemory.ChangeSet{}, errors.New("invalid Memory change set: operation lists are required")
	}
	changes := noemamemory.ChangeSet{Deletes: *proposed.Deletes}
	for _, candidate := range *proposed.Upserts {
		if candidate.Path == nil || candidate.Title == nil || candidate.Icon == nil ||
			candidate.Body == nil || candidate.Citations == nil {
			return noemamemory.ChangeSet{}, errors.New("Memory page change is missing a required field")
		}
		change := noemamemory.PageChange{
			Path: *candidate.Path, Title: *candidate.Title, Icon: *candidate.Icon,
			Body: *candidate.Body, Citations: *candidate.Citations,
		}
		if candidate.ID != nil {
			change.ID = *candidate.ID
		}
		if candidate.ExpectedHash != nil {
			change.ExpectedHash = *candidate.ExpectedHash
		}
		if err := normalizeMemoryCitations(&change, allowed); err != nil {
			return noemamemory.ChangeSet{}, err
		}
		changes.Upserts = append(changes.Upserts, change)
	}
	metadataPaths := make(map[string]bool)
	for _, update := range *proposed.MetadataUpdates {
		if update.Path == nil || update.Icon == nil {
			return noemamemory.ChangeSet{}, errors.New("Memory metadata update is missing a required field")
		}
		var page *noemamemory.Page
		for index := range pages {
			if pages[index].Path == *update.Path {
				page = &pages[index]
				break
			}
		}
		if page == nil {
			return noemamemory.ChangeSet{}, fmt.Errorf("metadata update references unknown page %s", *update.Path)
		}
		if !slices.Contains(noemamemory.PageIconKeys, *update.Icon) {
			return noemamemory.ChangeSet{}, fmt.Errorf("unsupported Memory icon %s", *update.Icon)
		}
		if metadataPaths[page.Path] {
			return noemamemory.ChangeSet{}, fmt.Errorf("duplicate metadata update for %s", page.Path)
		}
		metadataPaths[page.Path] = true
		if slices.Contains(changes.Deletes, page.Path) || slices.ContainsFunc(changes.Upserts, func(change noemamemory.PageChange) bool {
			return change.Path == page.Path || change.ID == page.ID
		}) {
			return noemamemory.ChangeSet{}, fmt.Errorf("page %s has both content and metadata changes", page.Path)
		}
		if page.Icon != *update.Icon {
			changes.Upserts = append(changes.Upserts, noemamemory.PageChange{
				ID: page.ID, ExpectedHash: page.Hash, Path: page.Path, Title: page.Title,
				Icon: *update.Icon, Body: page.Body, Citations: page.Citations,
			})
		}
	}
	scoped := cloneStringSet(editable)
	for path := range metadataPaths {
		scoped[path] = true
	}
	if err := validateMemoryChangeScope(changes, pages, scoped); err != nil {
		return noemamemory.ChangeSet{}, err
	}
	return changes, nil
}

func validateMemoryChangeScope(
	changes noemamemory.ChangeSet,
	pages []noemamemory.Page,
	editable map[string]bool,
) error {
	for _, change := range changes.Upserts {
		var current *noemamemory.Page
		for index := range pages {
			if (change.ID != "" && pages[index].ID == change.ID) || pages[index].Path == change.Path {
				current = &pages[index]
				break
			}
		}
		if current != nil && !editable[current.Path] {
			return fmt.Errorf("page %s was catalog-only and cannot be changed", current.Path)
		}
		for _, destination := range pages {
			if destination.Path == change.Path && !editable[destination.Path] {
				return fmt.Errorf("page %s was catalog-only and cannot be overwritten", destination.Path)
			}
		}
	}
	for _, pagePath := range changes.Deletes {
		for _, page := range pages {
			if page.Path == pagePath && !editable[pagePath] {
				return fmt.Errorf("page %s was catalog-only and cannot be deleted", pagePath)
			}
		}
	}
	return nil
}

func normalizeMemoryCitations(change *noemamemory.PageChange, allowed map[string]bool) error {
	for citationIndex := range change.Citations {
		citation := &change.Citations[citationIndex]
		if len(citation.Sources) == 0 {
			return fmt.Errorf("Memory page %s has an empty citation group", change.Path)
		}
		seen := make(map[string]bool)
		for sourceIndex, source := range citation.Sources {
			canonical := canonicalMemorySource(source, allowed)
			if canonical == "" {
				return fmt.Errorf("Memory page %s cites ineligible source %s", change.Path, source)
			}
			if seen[canonical] {
				return fmt.Errorf("Memory page %s repeats source %s", change.Path, canonical)
			}
			seen[canonical] = true
			citation.Sources[sourceIndex] = canonical
		}
	}
	articleLines := make([]string, 0)
	for _, line := range strings.Split(change.Body, "\n") {
		if !isNumericFootnoteDefinition(line) {
			articleLines = append(articleLines, line)
		}
	}
	article := strings.Join(articleLines, "\n")
	references, err := numericFootnoteReferences(article)
	if err != nil {
		return err
	}
	if len(references) != len(change.Citations) {
		return fmt.Errorf("Memory page %s must use every citation group", change.Path)
	}
	for index := 1; index <= len(change.Citations); index++ {
		if !references[index] {
			return fmt.Errorf("Memory page %s is missing citation marker [^%d]", change.Path, index)
		}
	}
	change.Body = strings.TrimSpace(article)
	return nil
}

func isNumericFootnoteDefinition(line string) bool {
	if !strings.HasPrefix(line, "[^") {
		return false
	}
	end := strings.Index(line, "]:")
	if end < 2 {
		return false
	}
	index, err := strconv.Atoi(line[2:end])
	return err == nil && index > 0
}

func numericFootnoteReferences(body string) (map[int]bool, error) {
	references := make(map[int]bool)
	for remainder := body; ; {
		start := strings.Index(remainder, "[^")
		if start < 0 {
			return references, nil
		}
		remainder = remainder[start+2:]
		end := strings.IndexByte(remainder, ']')
		if end < 0 {
			return nil, errors.New("Memory body contains an unterminated footnote marker")
		}
		index, err := strconv.Atoi(remainder[:end])
		if err != nil || index < 1 {
			return nil, errors.New("Memory body footnote marker is not a positive source index")
		}
		remainder = remainder[end+1:]
		if strings.HasPrefix(remainder, ":") {
			return nil, errors.New("Memory body must omit footnote definitions")
		}
		if references[index] {
			return nil, fmt.Errorf("Memory body repeats citation marker [^%d]", index)
		}
		references[index] = true
	}
}

func canonicalMemorySource(source string, allowed map[string]bool) string {
	if allowed[source] {
		return source
	}
	qualified := "item:" + source
	if allowed[qualified] {
		return qualified
	}
	if suffix, ok := strings.CutPrefix(source, "item:"); ok {
		paired := "item:tool_result:" + suffix
		if allowed[paired] {
			return paired
		}
	}
	for _, prefix := range []string{"human [", "tool result ["} {
		if value, ok := strings.CutPrefix(source, prefix); ok {
			if value, ok = strings.CutSuffix(value, "]"); ok && allowed[value] {
				return value
			}
		}
	}
	return ""
}
