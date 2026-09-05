package runtime

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"strings"

	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const (
	memorySubmitTool           = "noema.submit_memory_changes"
	memoryContextTokens        = 8_000
	memoryOutputTokens         = 2_048
	memoryCharsPerToken        = 3
	memoryThresholdNumerator   = 7
	memoryThresholdDenominator = 10
)

var memoryChangesSchema = json.RawMessage(`{
  "type":"object",
  "properties":{
    "upserts":{"type":"array","items":{"type":"object","properties":{
      "id":{"type":["string","null"]},"expected_hash":{"type":["string","null"]},
      "path":{"type":"string","minLength":1},"title":{"type":"string","minLength":1},
      "icon":{"type":"string","minLength":1},
      "body":{"type":"string","description":"Article Markdown without a title or footnote definitions. Use numeric markers [^1], [^2], and so on for ordered citation groups."},
      "citations":{"type":"array","items":{"type":"object","properties":{"sources":{"type":"array","minItems":1,"uniqueItems":true,"items":{"type":"string","minLength":1}}},"required":["sources"],"additionalProperties":false}}
    },"required":["path","title","icon","body","citations"],"additionalProperties":false}},
    "metadata_updates":{"type":"array","items":{"type":"object","properties":{"path":{"type":"string","minLength":1},"icon":{"type":"string","minLength":1}},"required":["path","icon"],"additionalProperties":false}},
    "deletes":{"type":"array","items":{"type":"string","minLength":1}}
  },
  "required":["upserts","metadata_updates","deletes"],"additionalProperties":false
}`)

// MemoryUpdateRuntimeStatus is the active updater state outside the durable checkpoint.
type MemoryUpdateRuntimeStatus struct {
	Active bool
	Error  string
}

// TriggerMemoryUpdate starts one update for the local primary Chat.
func (c *Chat) TriggerMemoryUpdate(ctx context.Context) (bool, error) {
	primary, err := c.database.PrimaryConversation(ctx)
	if err != nil {
		return false, err
	}
	if primary == nil {
		return false, errors.New("primary conversation is unavailable")
	}
	return c.scheduleMemoryUpdate(primary.ID), nil
}

// MemoryUpdateStatus returns the current single-flight updater state.
func (c *Chat) MemoryUpdateStatus() MemoryUpdateRuntimeStatus {
	c.memoryMu.Lock()
	defer c.memoryMu.Unlock()
	return MemoryUpdateRuntimeStatus{Active: c.memoryRun, Error: c.memoryErr}
}

func (c *Chat) publishMemoryChanged() {
	c.subMu.Lock()
	defer c.subMu.Unlock()
	for id, current := range c.subscribers {
		if current.conversationID != memoryEventChannel {
			continue
		}
		select {
		case current.events <- Event{Kind: EventMemoryChanged, ConversationID: memoryEventChannel}:
		default:
			close(current.events)
			delete(c.subscribers, id)
		}
	}
}

func (c *Chat) maybeScheduleMemoryUpdate(conversationID string) {
	primary, err := c.database.PrimaryConversation(c.ctx)
	if err != nil || primary == nil || primary.ID != conversationID {
		return
	}
	checkpoint, err := c.memory.State()
	if err != nil {
		return
	}
	cursor := int64(0)
	if checkpoint.ConversationID == conversationID {
		cursor = checkpoint.LastConsolidatedSequence
	}
	captured, err := c.database.CaptureMemorySourceRange(c.ctx, conversationID, cursor)
	if err != nil || !memorySourceReachedThreshold(captured.Items) {
		return
	}
	c.scheduleMemoryUpdate(conversationID)
}

func memorySourceReachedThreshold(items []store.ConversationItem) bool {
	characters := 0
	for _, item := range items {
		if rendered := renderMemorySourceItem(item); rendered != "" {
			characters += len([]rune(rendered)) + 1
		}
	}
	available := memoryContextTokens - memoryOutputTokens
	threshold := available * memoryThresholdNumerator / memoryThresholdDenominator
	return characters >= threshold*memoryCharsPerToken
}

func (c *Chat) scheduleMemoryUpdate(conversationID string) bool {
	c.stateMu.RLock()
	defer c.stateMu.RUnlock()
	if c.closed {
		return false
	}
	c.memoryMu.Lock()
	if c.memoryRun {
		c.memoryMu.Unlock()
		return false
	}
	c.memoryRun = true
	c.memoryErr = ""
	c.memoryWG.Add(1)
	c.memoryMu.Unlock()
	c.publishMemoryChanged()
	go func() {
		defer c.memoryWG.Done()
		err := c.runMemoryUpdate(c.ctx, conversationID)
		c.memoryMu.Lock()
		c.memoryRun = false
		if err != nil && !errors.Is(err, context.Canceled) {
			c.memoryErr = err.Error()
		}
		c.memoryMu.Unlock()
		c.publishMemoryChanged()
	}()
	return true
}

func (c *Chat) runMemoryUpdate(ctx context.Context, conversationID string) error {
	primary, err := c.database.PrimaryConversation(ctx)
	if err != nil {
		return err
	}
	if primary == nil || primary.ID != conversationID {
		return errors.New("Memory updates require the local primary conversation")
	}
	checkpoint, err := c.memory.State()
	if err != nil {
		return err
	}
	cursor := int64(0)
	if checkpoint.ConversationID == conversationID {
		cursor = checkpoint.LastConsolidatedSequence
	}
	captured, err := c.database.CaptureMemorySourceRange(ctx, conversationID, cursor)
	if err != nil || len(captured.Items) == 0 {
		return err
	}
	assignment, err := c.memoryAssignment(ctx)
	if err != nil {
		return err
	}
	generator, err := c.generatorFor(assignment.ProviderKind)
	if err != nil {
		return err
	}
	return c.consolidateMemoryRange(ctx, generator, assignment, captured)
}

func (c *Chat) memoryAssignment(ctx context.Context) (store.ModelAssignment, error) {
	assignments, err := c.database.HostedModelAssignments(ctx)
	if err != nil {
		return store.ModelAssignment{}, err
	}
	for _, assignment := range assignments {
		if assignment.Role != store.HostedModelMemoryConsolidation {
			continue
		}
		if assignment.SelectionMode == store.ModelSelectionNoemaRecommended {
			for _, recommendation := range provider.ModelRecommendations(assignment.ProviderKind) {
				if recommendation.UseCase == provider.ModelUseMemoryConsolidation {
					assignment.ModelProfile = recommendation.ModelProfile
					assignment.ReasoningEffort = store.ModelReasoningEffort(recommendation.ReasoningEffort)
					return assignment, nil
				}
			}
		}
		return assignment, nil
	}
	return store.ModelAssignment{}, errors.New("Memory consolidation model is not configured")
}

func (c *Chat) consolidateMemoryRange(
	ctx context.Context,
	generator provider.Generator,
	assignment store.ModelAssignment,
	captured store.MemorySourceRange,
) error {
	items := captured.Items
	for offset := 0; offset < len(items); {
		pages, err := c.memory.ListPages()
		if err != nil {
			return err
		}
		editable := map[string]bool{noemamemory.RootPagePath: true}
		minimal, err := memoryPromptCatalog(pages, editable)
		if err != nil {
			return err
		}
		budget := (memoryContextTokens - memoryOutputTokens) * memoryCharsPerToken
		if len([]rune(minimal)) >= budget {
			return errors.New("Memory page catalog exceeds the model context budget")
		}
		all := make(map[string]bool, len(pages))
		allowed := make(map[string]bool)
		for _, page := range pages {
			all[page.Path] = true
			for _, citation := range page.Citations {
				for _, source := range citation.Sources {
					allowed[source] = true
				}
			}
		}
		full, err := memoryPromptCatalog(pages, all)
		if err != nil {
			return err
		}
		baseCharacters := len([]rune(minimal)) + (budget-len([]rune(minimal)))/3
		if len([]rune(full)) < budget {
			baseCharacters = len([]rune(full))
		}
		end, characters := offset, baseCharacters
		for end < len(items) {
			size := len([]rune(renderMemorySourceItem(items[end]))) + 1
			if end > offset && characters+size > budget {
				break
			}
			if end == offset && characters+size > budget {
				return fmt.Errorf("conversation item %s exceeds the model context budget", items[end].ID)
			}
			characters += size
			end++
		}
		if end == offset {
			return errors.New("Memory update could not fit a conversation item")
		}
		chunk := items[offset:end]
		var sourceLines []string
		for _, item := range chunk {
			if memoryEvidencePayload(item) != "" {
				allowed[item.ID] = true
			}
			if rendered := renderMemorySourceItem(item); rendered != "" {
				sourceLines = append(sourceLines, rendered)
			}
		}
		source := strings.Join(sourceLines, "\n")
		canonical := full
		if len([]rune(full))+len([]rune(source)) > budget {
			includeRelevantMemoryPages(c.memory, pages, chunk, editable, budget-len([]rune(source)))
			canonical, err = memoryPromptCatalog(pages, editable)
			if err != nil {
				return err
			}
		} else {
			editable = all
		}
		last := chunk[len(chunk)-1]
		next := noemamemory.State{
			ConversationID: captured.ConversationID, LastConsolidatedSequence: last.Sequence,
			LastConsolidatedItem: last.ID,
		}
		if err := c.generateAndPublishMemory(ctx, generator, assignment, source, canonical, pages, editable, allowed, next); err != nil {
			return err
		}
		offset = end
	}
	return nil
}

func (c *Chat) generateAndPublishMemory(
	ctx context.Context,
	generator provider.Generator,
	assignment store.ModelAssignment,
	source, canonical string,
	pages []noemamemory.Page,
	editable, allowed map[string]bool,
	state noemamemory.State,
) error {
	var correction string
	for attempt := 0; attempt < 2; attempt++ {
		maximum := uint32(memoryOutputTokens)
		result, err := generator.Generate(ctx, provider.GenerateRequest{
			AccountID: assignment.ProviderAccountID, Model: assignment.ModelProfile,
			Messages: []provider.GenerationMessage{
				{Role: "system", Content: memoryUpdateInstructions(canonical, correction)},
				{Role: "user", Content: source},
			},
			ReasoningEffort: string(assignment.ReasoningEffort), MaxOutputTokens: &maximum,
			ConversationID: state.ConversationID,
			Tools: []provider.GenerationTool{{
				Name: memorySubmitTool, Description: "Submit one complete evidence-backed native Memory change set for runtime validation.",
				InputSchema: memoryChangesSchema,
			}},
			ToolTransport: provider.ToolTransportNative, ToolChoice: provider.ToolChoiceRequired,
			FastMode: assignment.FastMode,
		}, func(provider.StreamEvent) {})
		if err != nil {
			return err
		}
		changes, err := parseMemoryChanges(result, allowed, pages, editable)
		if err != nil {
			correction = err.Error()
			continue
		}
		err = c.memory.Publish(changes, state)
		if err == nil {
			return nil
		}
		if !errors.Is(err, noemamemory.ErrInvalidPage) {
			return err
		}
		correction = err.Error()
	}
	return fmt.Errorf("Memory update was rejected after correction: %s", correction)
}

type memoryPromptPage struct {
	ID        string                  `json:"id,omitempty"`
	Path      string                  `json:"path"`
	Title     string                  `json:"title"`
	Parent    *string                 `json:"parent"`
	Icon      string                  `json:"icon"`
	Hash      string                  `json:"hash,omitempty"`
	Body      *string                 `json:"body,omitempty"`
	Citations *[]noemamemory.Citation `json:"citations,omitempty"`
	Excerpt   *string                 `json:"excerpt,omitempty"`
}

func memoryPromptCatalog(pages []noemamemory.Page, editable map[string]bool) (string, error) {
	values := make([]memoryPromptPage, 0, len(pages))
	for _, page := range pages {
		value := memoryPromptPage{Path: page.Path, Title: page.Title, Icon: page.Icon}
		if page.Parent != "" {
			value.Parent = &page.Parent
		}
		if editable[page.Path] {
			value.ID, value.Hash = page.ID, page.Hash
			value.Body, value.Citations = &page.Body, &page.Citations
		} else {
			runes := []rune(page.Body)
			if len(runes) > 120 {
				excerpt := string(runes[:120]) + "…"
				value.Excerpt = &excerpt
			} else {
				value.Excerpt = &page.Body
			}
		}
		values = append(values, value)
	}
	encoded, err := json.Marshal(values)
	return string(encoded), err
}

func includeRelevantMemoryPages(
	memoryStore *noemamemory.Store,
	pages []noemamemory.Page,
	items []store.ConversationItem,
	selected map[string]bool,
	budget int,
) {
	terms := make(map[string]bool)
	ordered := make([]string, 0, 32)
	for _, item := range items {
		for _, term := range strings.Fields(memoryEvidencePayload(item)) {
			term = strings.Trim(term, ".,:;!?()[]{}\"'")
			if term != "" && len([]rune(term)) > 2 && !terms[term] && len(ordered) < 32 {
				terms[term] = true
				ordered = append(ordered, term)
			}
		}
	}
	for _, term := range ordered {
		results, err := memoryStore.Search(term, 8)
		if err != nil {
			continue
		}
		for _, result := range results {
			candidate := cloneStringSet(selected)
			includeMemoryPageAncestors(pages, result.Path, candidate)
			rendered, err := memoryPromptCatalog(pages, candidate)
			if err == nil && len([]rune(rendered)) <= budget {
				for path := range candidate {
					selected[path] = true
				}
			}
		}
	}
}

func includeMemoryPageAncestors(pages []noemamemory.Page, pagePath string, selected map[string]bool) {
	for pagePath != "" {
		selected[pagePath] = true
		parent := ""
		for _, page := range pages {
			if page.Path == pagePath {
				parent = page.Parent
				break
			}
		}
		pagePath = parent
	}
}

func cloneStringSet(source map[string]bool) map[string]bool {
	result := make(map[string]bool, len(source))
	for value := range source {
		result[value] = true
	}
	return result
}

func memoryEvidencePayload(item store.ConversationItem) string {
	switch item.Kind {
	case store.ConversationUserText:
		return item.ContentText
	case store.ConversationToolResult:
	case store.ConversationItemKind("activity"):
		if kind, _ := item.Payload["activity_kind"].(string); kind != "tool_result" {
			return ""
		}
	default:
		return ""
	}
	metadata, _ := item.Payload["metadata"].(map[string]any)
	action, _ := metadata["action"].(map[string]any)
	payload, exists := action["payload"]
	if !exists {
		return ""
	}
	encoded, err := json.Marshal(payload)
	if err != nil {
		return ""
	}
	return string(encoded)
}

func renderMemorySourceItem(item store.ConversationItem) string {
	switch item.Kind {
	case store.ConversationAssistantText:
		return "assistant " + item.ContentText
	case store.ConversationUserText:
		return fmt.Sprintf("human [%s] %s", item.ID, item.ContentText)
	case store.ConversationToolResult, store.ConversationItemKind("activity"):
		if payload := memoryEvidencePayload(item); payload != "" {
			return fmt.Sprintf("tool result [%s] %s", item.ID, payload)
		}
	}
	return ""
}

func memoryUpdateInstructions(canonical, correction string) string {
	if correction != "" {
		correction = "\nYour previous native tool call was rejected: " + correction + ". Correct that failure in the replacement tool call."
	}
	iconKeys := strings.Join(noemamemory.PageIconKeys, ", ")
	return fmt.Sprintf(`You are editing a compact personal encyclopedia, not recording a chronological fact list. The complete page catalog follows. Entries with body and citations are content-editable. Excerpt-only entries are discovery context. Do not content-upsert, move, overwrite, or delete excerpt-only entries. You may change their icon with metadata_updates. You may create a new page when evidence warrants one. Existing pages: %s
Call noema.submit_memory_changes exactly once through the native tool channel.
Editorial contract: root.md is a biographical overview titled with the local human's name when known. Begin each page with a natural lead. Group related material into thematic ## sections. A developed root article needs at least two sections. Keep root concise. Create focused child pages when needed. Store stable human facts, preferences, relationships, and durable decisions. Do not store connector readiness, tool counts, temporary failures, Task history, project validation, or researched facts that belong elsewhere. Each rendered page must contain at most %d Unicode words. Keep each article body at or below %d words. Do not put a # title in body.
Preference contract: Keep useful personal criteria and constraints at the narrowest supported scope. Do not require repetition or explicit memory language. Do not make inferred preferences universal. Replace contradicted preferences with current meaning. Exclude one-time criteria and assistant recommendations.
Icon contract: Every content upsert needs one supported Lucide icon from [%s]. Preserve an existing icon when it remains suitable. Use metadata_updates for icon-only changes.
Evidence contract: citations is the ordered evidence-group list. Cite groups as [^1], [^2], and so on. Use every group. Keep the smallest direct evidence set. Human messages and exact tool results can be evidence. Assistant messages are context only. Preserve stable IDs, expected hashes, hierarchy, and user meaning unless evidence requires change. Omit footnote definitions because Noema generates them. Do not copy secrets, tokens, credentials, or private keys. Use owner human:local and scope human:local.%s`,
		canonical, noemamemory.MaxWords, noemamemory.MaxWords-100, iconKeys, correction)
}
