package runtime

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"unicode/utf8"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

const contextSafetyTokens = uint32(128)

const (
	backgroundCompactionThresholdNumerator   = uint32(7)
	backgroundCompactionThresholdDenominator = uint32(10)
)

var errContextWindowExceeded = errors.New("model context window exceeded")

type modelContextRequest struct {
	database                       *store.Store
	generator                      provider.Generator
	accountID, providerKind, model string
	base, completed, active        []provider.GenerationMessage
	restoredContext                []provider.GenerationMessage
	tools                          []provider.GenerationTool
	hostedWeb                      bool
	outputReserve                  uint32
	persist                        func(string, []provider.GenerationMessage) error
}

func prepareModelContext(ctx context.Context, request modelContextRequest) ([]provider.GenerationMessage, bool, error) {
	continuation := NewContinuationContext(joinContextMessages(request.base, request.completed, request.active))
	messages, err := continuation.AdmissionMessages()
	if err != nil {
		return nil, false, err
	}
	window, err := contextWindow(ctx, request.database, request.accountID, request.providerKind, request.model)
	if err != nil || window == 0 {
		return messages, false, err
	}
	available := window
	if request.outputReserve >= available {
		available = 0
	} else {
		available -= request.outputReserve
	}
	if contextSafetyTokens >= available {
		available = 0
	} else {
		available -= contextSafetyTokens
	}
	estimated := CountModelContext(ctx, request.generator, messages, request.tools, request.hostedWeb)
	if estimated <= available && (!shouldCompactBackground(estimated, available) || len(request.completed) == 0) {
		return messages, false, nil
	}
	if len(request.completed) == 0 {
		return nil, false, fmt.Errorf("%w: request requires approximately %d input tokens, but the selected model allows %d; no completed history remains to compact",
			errContextWindowExceeded, estimated, available)
	}
	prefix, recent := compactionPrefix(request.completed, available, estimated > available)
	var restored []provider.GenerationMessage
	if request.restoredContext != nil {
		_, updates, _, err := diffModelContext(joinContextMessages(recent, request.active), request.restoredContext)
		if err != nil {
			return nil, false, err
		}
		for _, update := range updates {
			restored = append(restored, contextUpdateMessage(update))
		}
	}
	target := min(uint32(512), max(uint32(64), available/8))
	summary, err := summarizeModelContext(ctx, request, prefix, target, available)
	if err != nil {
		if estimated <= available && ctx.Err() == nil {
			return messages, false, nil
		}
		return nil, false, err
	}
	for attempts := 0; attempts < 4; attempts++ {
		compacted := append([]provider.GenerationMessage{{Role: "assistant", Content: "Noema compacted prior completed context:\n" + summary}}, recent...)
		messages = joinContextMessages(request.base, compacted, request.active, restored)
		if CountModelContext(ctx, request.generator, messages, request.tools, request.hostedWeb) <= available {
			if request.persist != nil {
				if err := request.persist(summary, recent); err != nil {
					return nil, false, err
				}
			}
			return messages, true, nil
		}
		if estimated <= available {
			return joinContextMessages(request.base, request.completed, request.active), false, nil
		}
		target = max(uint32(64), target/2)
		summary, err = summarizeModelContext(ctx, request,
			[]provider.GenerationMessage{{Role: "assistant", Content: summary}}, target, available)
		if err != nil {
			return nil, false, err
		}
	}
	return nil, false, fmt.Errorf("%w: compacted context does not fit the selected model", errContextWindowExceeded)
}

func shouldCompactBackground(estimated, available uint32) bool {
	return estimated >= available*backgroundCompactionThresholdNumerator/backgroundCompactionThresholdDenominator
}

func contextWindow(ctx context.Context, database *store.Store, accountID, providerKind, model string) (uint32, error) {
	fallback := map[string]uint32{
		"codex": 128_000, "openai": 128_000,
		"local_models": 8_192, "openrouter": 32_768,
	}[providerKind]
	account, err := database.ProviderAccount(ctx, accountID)
	if err != nil {
		if errors.Is(err, provider.ErrAccountNotFound) {
			return fallback, nil
		}
		return 0, err
	}
	profiles, err := account.Metadata.ModelProfiles()
	if err != nil {
		return 0, err
	}
	for _, profile := range profiles {
		if profile.ID == model && profile.ContextWindowTokens != nil {
			return *profile.ContextWindowTokens, nil
		}
	}
	return fallback, nil
}

func compactionPrefix(completed []provider.GenerationMessage, available uint32, hard bool) (
	[]provider.GenerationMessage, []provider.GenerationMessage,
) {
	if hard || len(completed) < 2 {
		return completed, nil
	}
	cap := min(uint32(8_192), available/5)
	start, tokens := len(completed), uint32(0)
	for start > 0 {
		rendered, _ := json.Marshal(completed[start-1 : start])
		next := uint32((utf8.RuneCount(rendered) + 2) / 3)
		if tokens+next > cap {
			break
		}
		start--
		tokens += next
	}
	start = completeContextStart(completed, start)
	if start <= 0 {
		return completed, nil
	}
	return completed[:start], completed[start:]
}

func completeContextStart(messages []provider.GenerationMessage, start int) int {
	if start >= len(messages) {
		return start
	}
	if messages[start].Role == "tool" {
		for start > 0 && messages[start-1].Role == "tool" {
			start--
		}
		if start > 0 && len(messages[start-1].ToolCalls) != 0 {
			start--
		}
	}
	for start > 0 && messages[start-1].Role == "hosted_web_search" {
		start--
	}
	return start
}

// CountModelContext uses the production tokenizer or conservative text estimate.
func CountModelContext(ctx context.Context, generator provider.Generator, messages []provider.GenerationMessage,
	tools []provider.GenerationTool, hostedWeb bool,
) uint32 {
	var rendered strings.Builder
	for _, message := range messages {
		switch continuationKind(message) {
		case "message", "assistant_text":
			// Text is model input, not a JSON string with an extra escape layer.
			rendered.WriteString(message.Role + ": " + message.Content)
		default:
			encoded, _ := json.Marshal(message)
			rendered.Write(encoded)
		}
		rendered.WriteByte('\n')
	}
	if len(tools) != 0 {
		encoded, _ := json.Marshal(tools)
		rendered.Write(encoded)
	}
	var tokens uint32
	if counter, ok := generator.(interface {
		CountTokens(context.Context, *string, string) (uint32, error)
	}); ok {
		tokens, _ = counter.CountTokens(ctx, nil, rendered.String())
	}
	if tokens == 0 && rendered.Len() != 0 {
		tokens = uint32((utf8.RuneCountInString(rendered.String()) + 2) / 3)
	}
	if hostedWeb {
		tokens += 256
	}
	return tokens
}

func summarizeModelContext(ctx context.Context, request modelContextRequest,
	messages []provider.GenerationMessage, target, available uint32,
) (string, error) {
	rendered, _ := json.Marshal(messages)
	current := string(rendered)
	// Start with one generous chunk. The fit check below still splits genuinely
	// oversized inputs, while ordinary single-message history gets one summary
	// request instead of an avoidable multi-pass compaction.
	chunkRunes := max(1_000, int(available)*4)
	for pass := 0; pass < 4; pass++ {
		parts := contextChunks(current, chunkRunes)
		summaries := make([]string, 0, len(parts))
		for len(parts) != 0 {
			part := parts[0]
			parts = parts[1:]
			summaryMessages := []provider.GenerationMessage{{Role: "system", Instructions: true, Content: compactionInstructions(target)}, {Role: "user", Content: part}}
			if CountModelContext(ctx, request.generator, summaryMessages, nil, false) > available && utf8.RuneCountInString(part) > 1 {
				runes := []rune(part)
				middle := len(runes) / 2
				parts = append([]string{string(runes[:middle]), string(runes[middle:])}, parts...)
				continue
			}
			if CountModelContext(ctx, request.generator, summaryMessages, nil, false) > available {
				return "", fmt.Errorf("%w: summary input does not fit", errContextWindowExceeded)
			}
			result, err := request.generator.Generate(ctx, provider.GenerateRequest{
				AccountID: request.accountID, Model: request.model,
				Messages:        summaryMessages,
				ReasoningEffort: "low", MaxOutputTokens: &target,
				ToolTransport: provider.ToolTransportNone, ToolChoice: provider.ToolChoiceNone,
				HostedWebSearch: false, StoreResponse: false,
			}, func(provider.StreamEvent) {})
			if err != nil {
				return "", errors.New("model context compaction failed")
			}
			summary := strings.TrimSpace(result.Text)
			if summary == "" || len(result.ToolCalls) != 0 {
				return "", errors.New("model context compaction returned no summary")
			}
			summaries = append(summaries, summary)
		}
		next := strings.Join(summaries, "\n\n")
		if len(summaries) == 1 {
			return next, nil
		}
		if utf8.RuneCountInString(next) >= utf8.RuneCountInString(current) {
			return "", errors.New("model context compaction did not reduce completed history")
		}
		current = next
	}
	return "", errors.New("model context compaction could not combine its summaries")
}

func contextChunks(value string, limit int) []string {
	runes := []rune(value)
	result := make([]string, 0, (len(runes)+limit-1)/limit)
	for len(runes) != 0 {
		end := min(limit, len(runes))
		result = append(result, string(runes[:end]))
		runes = runes[end:]
	}
	return result
}

func joinContextMessages(parts ...[]provider.GenerationMessage) []provider.GenerationMessage {
	count := 0
	for _, part := range parts {
		count += len(part)
	}
	result := make([]provider.GenerationMessage, 0, count)
	for _, part := range parts {
		result = append(result, part...)
	}
	return result
}

func continuationReady(generator provider.Generator, previousID string) bool {
	if session, ok := generator.(provider.ContinuationSession); ok {
		return session.ContinuationReady(previousID)
	}
	_, session := generator.(provider.GenerationSession)
	return session && previousID != ""
}

func splitActiveHistory(history, incremental []provider.GenerationMessage) (
	[]provider.GenerationMessage, []provider.GenerationMessage,
) {
	if len(history) == 0 {
		return nil, nil
	}
	start := max(0, len(history)-len(incremental))
	if len(incremental) == 0 {
		start = len(history) - 1
	}
	if start > 0 && history[start].Role == "tool" {
		if len(history[start-1].ToolCalls) != 0 {
			start--
		} else {
			start = 0
		}
	}
	start = completeContextStart(history, start)
	return history[:start], history[start:]
}

func chatContextParts(value store.ConversationContext, activeTurnID, providerKind string) (
	[]provider.GenerationMessage, []provider.GenerationMessage, int64, error,
) {
	priorItems := make([]store.ConversationItem, 0, len(value.Items))
	activeItems := make([]store.ConversationItem, 0, len(value.Items))
	through := value.ThroughSequence
	for _, item := range value.Items {
		if item.TurnID == activeTurnID {
			activeItems = append(activeItems, item)
		} else {
			priorItems = append(priorItems, item)
			through = max(through, item.Sequence)
		}
	}
	completed, err := providerMessagesFromItems(priorItems, activeTurnID, providerKind)
	if err != nil {
		return nil, nil, 0, err
	}
	if value.Summary != "" {
		var recent []provider.GenerationMessage
		if json.Unmarshal([]byte(value.RecentJSON), &recent) != nil {
			return nil, nil, 0, errors.New("conversation context checkpoint is invalid")
		}
		checkpoint := []provider.GenerationMessage{{Role: "assistant",
			Content: "Noema compacted prior completed context:\n" + value.Summary}}
		completed = append(append(checkpoint, recent...), completed...)
	}
	active, err := providerMessagesFromItems(activeItems, activeTurnID, providerKind)
	return completed, active, through, err
}

func completedTurnThrough(items []store.ConversationItem, turnID string) int64 {
	latestRound := -1
	for _, item := range items {
		if item.TurnID == turnID {
			latestRound = max(latestRound, providerRound(item))
		}
	}
	var through int64
	for _, item := range items {
		if item.Kind != store.ConversationModelContextUpdate && item.TurnID == turnID && providerRound(item) < latestRound {
			through = max(through, item.Sequence)
		}
	}
	return through
}

func compactionInstructions(target uint32) string {
	return fmt.Sprintf("Compact Noema conversation context into a durable rolling summary.\nWrite plain assistant text only. Target at most %d tokens.\nPreserve active user goals, durable decisions, unresolved references, recently active entities, projects, files, tools, and explicit uncertainty.\nDo not invent facts. Do not convert conversation-local details into memory claims.", target)
}

func compactionPrompt(content string, target uint32) string {
	return compactionInstructions(target) + "\n\n" + content
}
