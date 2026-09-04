package graphql

import (
	"context"
	"encoding/json"
	"errors"
	"math"
	"strings"
	"time"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/graphql/model"
	noemamemory "github.com/kpsuperplane/noema/internal/memory"
	"github.com/kpsuperplane/noema/internal/store"
)

func (r *Resolver) memorySettings(ctx context.Context) (*model.GraphqlNativeMemorySettings, error) {
	assignments, err := r.Store.HostedModelAssignments(ctx)
	if err != nil {
		return nil, err
	}
	var preference *model.AgentModelPreference
	for _, assignment := range assignments {
		if assignment.Role == store.HostedModelMemoryConsolidation {
			preference = agentModelPreference(assignment)
			break
		}
	}
	options := []*model.AgentModelProviderOption{}
	if r.ProviderAccounts != nil {
		options, err = r.agentModelOptions(ctx)
		if err != nil {
			return nil, err
		}
	}
	return &model.GraphqlNativeMemorySettings{
		ModelPreference: preference, ModelOptions: options,
	}, nil
}

func (r *Resolver) memoryTree(ctx context.Context) (*model.GraphqlNativeMemoryTree, error) {
	if r.Memory == nil {
		return nil, errors.New("native Memory is unavailable")
	}
	root, err := r.Memory.ReadRoot()
	if err != nil {
		return nil, err
	}
	pages, err := r.Memory.ListPages()
	if err != nil {
		return nil, err
	}
	rootModel, err := r.memoryPageModel(ctx, root)
	if err != nil {
		return nil, err
	}
	refs := make([]*model.GraphqlNativeMemoryPageRef, 0, len(pages))
	for _, page := range pages {
		refs = append(refs, memoryPageRefModel(noemamemory.Reference(page)))
	}
	checkpoint, err := r.Memory.State()
	if err != nil {
		return nil, err
	}
	pending := 0
	if primary, primaryErr := r.Store.PrimaryConversation(ctx); primaryErr == nil && primary != nil {
		cursor := int64(0)
		if checkpoint.ConversationID == primary.ID {
			cursor = checkpoint.LastConsolidatedSequence
		}
		if source, sourceErr := r.Store.CaptureMemorySourceRange(ctx, primary.ID, cursor); sourceErr == nil {
			pending = min(len(source.Items), math.MaxInt32)
		}
	}
	return &model.GraphqlNativeMemoryTree{
		Root: rootModel, Pages: refs, PendingCount: pending,
		UpdateStatus: memoryUpdateStatus(checkpoint),
	}, nil
}

func (r *Resolver) memoryPage(ctx context.Context, selector string) (*model.GraphqlNativeMemoryPage, error) {
	if r.Memory == nil {
		return nil, nil
	}
	page, err := r.Memory.ReadPage(selector)
	if errors.Is(err, noemamemory.ErrPageNotFound) {
		return nil, nil
	}
	if err != nil {
		return nil, err
	}
	return r.memoryPageModel(ctx, page)
}

func (r *Resolver) memoryEvents(ctx context.Context) (<-chan *model.GraphqlNativeMemoryTree, error) {
	initial, err := r.memoryTree(ctx)
	if err != nil {
		return nil, err
	}
	events := make(chan *model.GraphqlNativeMemoryTree, 1)
	events <- initial
	go func() {
		<-ctx.Done()
		close(events)
	}()
	return events, nil
}

func (r *Resolver) memoryPageModel(
	ctx context.Context,
	page noemamemory.Page,
) (*model.GraphqlNativeMemoryPage, error) {
	citations := make([]*model.GraphqlNativeMemoryCitation, 0, len(page.Citations))
	for _, citation := range page.Citations {
		sources := make([]*model.GraphqlNativeMemorySourceReference, 0, len(citation.Sources))
		for _, source := range citation.Sources {
			reference, err := r.memorySourceModel(ctx, source)
			if err != nil {
				return nil, err
			}
			sources = append(sources, reference)
		}
		citations = append(citations, &model.GraphqlNativeMemoryCitation{Sources: sources})
	}
	ancestors := make([]*model.GraphqlNativeMemoryPageRef, 0, len(page.Ancestors))
	for _, ancestor := range page.Ancestors {
		ancestors = append(ancestors, memoryPageRefModel(ancestor))
	}
	children := make([]*model.GraphqlNativeMemoryPageRef, 0, len(page.Children))
	for _, child := range page.Children {
		children = append(children, memoryPageRefModel(child))
	}
	var parent *string
	if page.Parent != "" {
		parent = &page.Parent
	}
	return &model.GraphqlNativeMemoryPage{
		ID: page.ID, Path: page.Path, Title: page.Title, Icon: page.Icon,
		Body: page.Body, Hash: page.Hash, Citations: citations, Parent: parent,
		Ancestors: ancestors, Children: children,
	}, nil
}

func (r *Resolver) memorySourceModel(
	ctx context.Context,
	source string,
) (*model.GraphqlNativeMemorySourceReference, error) {
	reference := &model.GraphqlNativeMemorySourceReference{
		Source: source, Kind: model.GraphqlNativeMemorySourceKindUnavailable,
	}
	item, err := r.Store.VisibleConversationItem(ctx, source)
	if err != nil {
		return nil, err
	}
	if item == nil {
		return reference, nil
	}
	created := item.CreatedAt.UTC().Format(time.RFC3339Nano)
	reference.CreatedAt = &created
	var text string
	switch item.Kind {
	case store.ConversationUserText:
		reference.Kind = model.GraphqlNativeMemorySourceKindHumanMessage
		text = item.ContentText
	case store.ConversationToolResult:
		reference.Kind = model.GraphqlNativeMemorySourceKindToolResult
		text = memoryToolPayload(item.Payload)
	default:
		activityKind, _ := item.Payload["activity_kind"].(string)
		if string(item.Kind) == "activity" && activityKind == "tool_result" {
			reference.Kind = model.GraphqlNativeMemorySourceKindToolResult
			text = memoryToolPayload(item.Payload)
		}
	}
	if reference.Kind == model.GraphqlNativeMemorySourceKindUnavailable {
		return reference, nil
	}
	if text != "" {
		excerpt := boundedMemoryExcerpt(text)
		reference.Excerpt = &excerpt
	}
	return reference, nil
}

func memoryToolPayload(payload map[string]any) string {
	metadata, _ := payload["metadata"].(map[string]any)
	action, _ := metadata["action"].(map[string]any)
	value, exists := action["payload"]
	if !exists {
		return ""
	}
	encoded, err := json.Marshal(value)
	if err != nil {
		return ""
	}
	return string(encoded)
}

func boundedMemoryExcerpt(value string) string {
	normalized := strings.Join(strings.Fields(value), " ")
	if utf8.RuneCountInString(normalized) <= 360 {
		return normalized
	}
	runes := []rune(normalized)
	return strings.TrimSpace(string(runes[:360])) + "…"
}

func memoryPageRefModel(page noemamemory.PageRef) *model.GraphqlNativeMemoryPageRef {
	return &model.GraphqlNativeMemoryPageRef{
		ID: page.ID, Path: page.Path, Title: page.Title, Icon: page.Icon,
		Excerpt: page.Excerpt, Hash: page.Hash,
	}
}

func memoryUpdateStatus(state noemamemory.State) *model.GraphqlNativeMemoryUpdateStatus {
	var item, updated *string
	if state.LastConsolidatedItem != "" {
		item = &state.LastConsolidatedItem
	}
	if state.UpdatedAt != "" {
		updated = &state.UpdatedAt
	}
	return &model.GraphqlNativeMemoryUpdateStatus{
		State: "idle", Active: false,
		LastConsolidatedSequence: int(min(state.LastConsolidatedSequence, int64(math.MaxInt32))),
		LastConsolidatedItem:     item, UpdatedAt: updated,
	}
}
