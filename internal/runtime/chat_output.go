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
	citations []provider.Citation
}

func (s *chatOutputStream) finish(result *provider.GenerationResult, err error) error {
	s.citations = result.Citations
	return s.generationOutputStream.finish(result, err)
}

func (c *Chat) outputStream(turn store.ConversationTurn, round int, clientID *string) *chatOutputStream {
	stream := &chatOutputStream{}
	saved := make(map[string]provider.GenerationOutput)
	stream.generationOutputStream = newGenerationOutputStream(func(output []provider.GenerationOutput) error {
		offset := 0
		for _, section := range output {
			sectionOffset := offset
			if section.Kind == "message" {
				offset += utf16CodeUnitCount(section.Text)
			}
			key := fmt.Sprintf("%s:%d:%d", section.Kind, section.Index, section.SectionIndex)
			if previous, ok := saved[key]; ok && previous == section && len(stream.citations) == 0 {
				continue
			}
			normalized := normalizedProviderText{Text: section.Text}
			if section.Kind == "message" {
				var citations []provider.Citation
				for _, citation := range stream.citations {
					if citation.EndIndex == nil {
						if sectionOffset == 0 {
							citations = append(citations, citation)
						}
						continue
					}
					if *citation.EndIndex <= sectionOffset || *citation.EndIndex > offset {
						continue
					}
					end := *citation.EndIndex - sectionOffset
					citation.EndIndex = &end
					if citation.StartIndex != nil {
						start := max(0, *citation.StartIndex-sectionOffset)
						citation.StartIndex = &start
					}
					citations = append(citations, citation)
				}
				normalized = normalizeProviderText(section.Text, citations)
			}
			item, err := c.database.SaveConversationOutput(context.WithoutCancel(c.ctx), turn, round, section.Index, section.SectionIndex, section.Kind, section.Phase, section.ID, normalized.Text, section.Text, section.Status, generationCitations(normalized.Citations), time.Now())
			if err != nil {
				return err
			}
			saved[key] = section
			if item.ID != "" {
				c.publish(Event{Kind: EventConversationItem, ConversationID: turn.ConversationID, TurnID: turn.ID, ClientMessageID: clientID, Item: &item})
			}
		}
		return nil
	})
	return stream
}
