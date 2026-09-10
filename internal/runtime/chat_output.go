package runtime

import (
	"context"
	"fmt"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

type chatOutputStream struct {
	*generationOutputStream
	citations   []provider.Citation
	finalAnswer bool
}

func (s *chatOutputStream) finish(result *provider.GenerationResult, err error) error {
	s.citations = result.Citations
	s.finalAnswer = err == nil && len(result.ToolCalls) == 0
	return s.generationOutputStream.finish(result, err)
}

func (c *Chat) outputStream(turn store.ConversationTurn, round int, clientID *string) *chatOutputStream {
	stream := &chatOutputStream{}
	saved := make(map[string]provider.GenerationOutput)
	stream.generationOutputStream = newGenerationOutputStream(func(output []provider.GenerationOutput) error {
		lastMessage := -1
		for i, section := range output {
			if section.Kind == "message" {
				lastMessage = i
			}
		}
		offset := 0
		for i, section := range output {
			segments := []provider.MarkdownSegment{{Text: section.Text, SourceUTF16: [2]int{0, utf16CodeUnitCount(section.Text)}}}
			if section.Kind == "message" && section.Status == "completed" {
				segments = provider.SplitMarkdownSegments(section.Text)
			}
			if stream.finalAnswer && i == lastMessage && section.Phase == "" && len(segments) > 1 {
				section.Phase = "final_answer"
			}
			sectionOffset := offset
			if section.Kind == "message" {
				offset += utf16CodeUnitCount(section.Text)
			}
			key := fmt.Sprintf("%s:%d:%d", section.Kind, section.Index, section.SectionIndex)
			if previous, ok := saved[key]; ok && previous == section && len(stream.citations) == 0 {
				continue
			}
			for paragraph, segment := range segments {
				startOffset := sectionOffset + segment.SourceUTF16[0]
				endOffset := sectionOffset + segment.SourceUTF16[1]
				citationLimit := offset
				if paragraph+1 < len(segments) {
					citationLimit = sectionOffset + segments[paragraph+1].SourceUTF16[0]
				}
				normalized := normalizedProviderText{Text: segment.Text}
				if section.Kind == "message" {
					var citations []provider.Citation
					for _, citation := range stream.citations {
						if citation.EndIndex == nil {
							if sectionOffset == 0 && paragraph == 0 {
								citations = append(citations, citation)
							}
							continue
						}
						if *citation.EndIndex <= startOffset || *citation.EndIndex > citationLimit {
							continue
						}
						end := min(*citation.EndIndex, endOffset) - startOffset
						citation.EndIndex = &end
						if citation.StartIndex != nil {
							start := max(0, *citation.StartIndex-startOffset)
							citation.StartIndex = &start
						}
						citations = append(citations, citation)
					}
					normalized = normalizeProviderText(segment.Text, citations)
				}
				// The first paragraph retains the whole native message for exact replay.
				providerText := ""
				if paragraph == 0 {
					providerText = section.Text
				}
				item, err := c.database.SaveConversationOutput(context.WithoutCancel(c.ctx), turn, round, section.Index, section.SectionIndex, paragraph, section.Kind, section.Phase, section.ID, normalized.Text, providerText, section.Status, generationCitations(normalized.Citations), time.Now())
				if err != nil {
					return err
				}
				if item.ID != "" {
					c.publish(Event{Kind: EventConversationItem, ConversationID: turn.ConversationID, TurnID: turn.ID, ClientMessageID: clientID, Item: &item})
				}
			}
			saved[key] = section
		}
		return nil
	})
	return stream
}
