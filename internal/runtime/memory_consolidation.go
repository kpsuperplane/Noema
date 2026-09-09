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
	memorySubmitTool    = "noema.submit_memory_changes"
	memoryContextTokens = 8_000
	memoryOutputTokens  = 2_048
	memoryCharsPerToken = 3
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
	for _, current := range c.subscribers {
		if current.conversationID != memoryEventChannel {
			continue
		}
		select {
		case current.events <- Event{Kind: EventMemoryChanged, ConversationID: memoryEventChannel}:
		default:
			// One pending invalidation is sufficient because subscribers reload the current snapshot.
		}
	}
}

func (c *Chat) schedulePrimaryMemoryUpdate(conversationID string) {
	primary, err := c.database.PrimaryConversation(c.ctx)
	if err == nil && primary != nil && primary.ID == conversationID {
		c.scheduleMemoryUpdate(conversationID)
	}
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
	window, err := contextWindow(ctx, c.database, assignment.ProviderAccountID, assignment.ProviderKind, assignment.ModelProfile)
	if err != nil {
		return err
	}
	if window == 0 {
		window = memoryContextTokens
	}
	budget := max((int(window)-memoryOutputTokens)*memoryCharsPerToken, 1)
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
				{Role: "system", Instructions: true, Content: memoryUpdateInstructions(canonical, correction)},
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
	queryParts := make([]string, 0, len(items))
	for _, item := range items {
		value := memoryEvidencePayload(item)
		if value == "" {
			value = item.ContentText
		}
		if value != "" {
			queryParts = append(queryParts, value)
		}
	}
	results, err := memoryStore.SearchRelevant(strings.Join(queryParts, " "), 8)
	if err != nil {
		return
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
	case store.ConversationUserText, store.ConversationMultipleChoiceSelection:
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
	name, _ := action["name"].(string)
	if strings.HasPrefix(name, "task.") {
		return ""
	}
	payload, exists := action["payload"]
	if !exists {
		return ""
	}
	if item.Kind == store.ConversationToolResult {
		if strings.HasPrefix(name, "web.browse.") {
			payload = withoutBrowserScreenshots(payload)
		}
	}
	encoded, err := json.Marshal(payload)
	if err != nil {
		return ""
	}
	return string(encoded)
}

// withoutBrowserScreenshots copies a browser result without its image data.
// Screenshots can be nested below result, while the other fields remain useful
// evidence for memory updates.
func withoutBrowserScreenshots(value any) any {
	switch current := value.(type) {
	case map[string]any:
		visible := make(map[string]any, len(current))
		for key, nested := range current {
			if key == "screenshot" {
				continue
			}
			visible[key] = withoutBrowserScreenshots(nested)
		}
		return visible
	case []any:
		visible := make([]any, len(current))
		for index, nested := range current {
			visible[index] = withoutBrowserScreenshots(nested)
		}
		return visible
	default:
		return value
	}
}

func renderMemorySourceItem(item store.ConversationItem) string {
	switch item.Kind {
	case store.ConversationAssistantText:
		return "assistant " + item.ContentText
	case store.ConversationUserText, store.ConversationMultipleChoiceSelection:
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
	return fmt.Sprintf(`You are editing a compact personal encyclopedia, not recording a chronological fact list. The complete page catalog is below. Entries with body and citations are content-editable and include stable ids and exact hashes; excerpt-only entries are discovery context and must not be content-upserted, moved, overwritten, or deleted, though their icon may be changed with metadata_updates. Keep new facts in the current article. Create another file only when retaining the content in that article would exceed its word limit. Existing pages are: %s
Call noema.submit_memory_changes exactly once through the provider's native tool channel. Do not encode the tool call or its arguments in ordinary assistant text.
Editorial contract: root.md is a biographical overview titled with the local human's name whenever known, never "Human memory" in that case. Begin each page with a natural human-language lead, then group related material into thematic ## sections. A developed root article must have at least two sections. Merge related claims into multi-sentence prose; never emit a sequence of one-sentence fact paragraphs, a field inventory, or a chronology of messages. Treat memory as evolving documentation. Start with root.md and organize facts into sections within the current article. A distinct topic alone does not justify another file. Before splitting, merge related claims and remove repetition without losing useful facts. Split only when the resulting article would exceed the 750-word limit, including its title and generated footnotes. Move a coherent section into a child article and retain a concise overview in the parent. Apply this rule at every depth. Merge small existing child articles back into their parent when the combined article fits within the limit. Preserve evidence when moving or merging content. Store stable human facts, preferences, relationships, and durable decisions. Do not store current connector readiness, enabled-tool counts, temporary failures, task execution history, project validation records, or researched subject facts that belong in their live object, task, project, document, or artifact. A rendered page includes its title and generated footnote definitions. It must contain at most %d Unicode words. Aim for %d body words to leave space for generated content; this target is not a reason to split. Do not put a # title in body because Noema generates it. Rewrite any existing page that violates this structure even when its facts remain correct.
Preference recall contract: Prefer to retain useful personal criteria and constraints. Store criteria that could improve future help as current preferences. Use the narrowest scope supported by the evidence. Store them even when they appear inside one concrete request or lack explicit memory language. Do not require repetition or words such as always or usually. Do not make an inferred preference permanent or universal. When newer human evidence contradicts a preference, rewrite or remove it. Keep the current meaning, not a history of changes. Do not store criteria clearly limited to one occasion or another person's needs. Do not store researched options or assistant recommendations.
Icon contract: every content upsert must include exactly one semantically specific Lucide icon key from [%s]. Preserve an existing icon when it remains the clearest fit. When only an existing page's icon should change, emit one metadata_updates entry instead of reproducing its content; use this whenever another allowed key represents the stable page subject more clearly. Treat file-text as a generic fallback and replace it whenever a more specific key fits.
Evidence contract: citations is the ordered list of evidence groups. Each group contains one or more exact source ids supporting one nearby claim. Cite the first group as [^1], the second as [^2], and so on. Use every group at least once. Reuse an exact source across groups only when it supports several claims. Keep the smallest direct evidence set. Do not retain an old source only because an earlier page used it. Do not write footnote definitions because Noema generates them. Human messages and exact tool results can be evidence. Assistant messages are context rather than independent evidence. Preserve stable ids, expected hashes, hierarchy, and user-authored meaning unless evidence requires a change. To move a page, retain its id and expected hash and change its path. Do not copy secrets, tokens, credentials, or private keys. Use owner human:local and scope human:local.%s`,
		canonical, noemamemory.MaxWords, noemamemory.MaxWords-100, iconKeys, correction)
}
