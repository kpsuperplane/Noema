package runtime

import (
	"errors"
	"sort"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
)

// generationOutputStream reconciles provider deltas with durable section snapshots.
// Chat and Tasks share the same ordering, completion, and write cadence.
type generationOutputStream struct {
	output    []provider.GenerationOutput
	save      func([]provider.GenerationOutput) error
	lastFlush time.Time
	dirty     bool
	err       error
}

func newGenerationOutputStream(save func([]provider.GenerationOutput) error) *generationOutputStream {
	return &generationOutputStream{save: save}
}

func (s *generationOutputStream) event(event provider.StreamEvent) {
	if s.err != nil {
		return
	}
	kind := "message"
	completed := false
	switch event.Kind {
	case provider.ToolCallStarted:
		s.flush(true)
		return
	case provider.MessageStarted:
	case provider.TextDelta:
	case provider.MessageCompleted:
		completed = true
	case provider.ReasoningDelta:
		kind = "reasoning"
	case provider.ReasoningCompleted:
		kind, completed = "reasoning", true
	default:
		return
	}
	index := s.section(kind, event.Index, event.SectionIndex)
	item := &s.output[index]
	if event.ID != "" {
		item.ID = event.ID
	}
	if event.Phase != "" {
		item.Phase = event.Phase
	}
	if completed {
		item.Text = event.Text
		item.Status = "completed"
	} else {
		item.Text += event.Delta
	}
	s.dirty = true
	s.flush(completed)
}

func (s *generationOutputStream) section(kind string, index, section int) int {
	for i, item := range s.output {
		if item.Kind == kind && item.Index == index && item.SectionIndex == section {
			return i
		}
	}
	s.output = append(s.output, provider.GenerationOutput{Kind: kind, Index: index, SectionIndex: section, Status: "running"})
	return len(s.output) - 1
}

func (s *generationOutputStream) flush(force bool) {
	if s.err != nil || !s.dirty || (!force && time.Since(s.lastFlush) < 100*time.Millisecond) {
		return
	}
	size := 0
	visible := false
	for _, item := range s.output {
		size += len(item.Text)
		visible = visible || strings.TrimSpace(item.Text) != ""
	}
	if size > 512<<10 {
		s.err = errors.New("provider display output exceeds transcript limit")
		return
	}
	if !visible {
		return
	}
	sort.SliceStable(s.output, func(i, j int) bool {
		if s.output[i].Index != s.output[j].Index {
			return s.output[i].Index < s.output[j].Index
		}
		return s.output[i].SectionIndex < s.output[j].SectionIndex
	})
	s.err = s.save(append([]provider.GenerationOutput(nil), s.output...))
	s.lastFlush, s.dirty = time.Now(), false
}

func (s *generationOutputStream) finish(result *provider.GenerationResult, generationErr error) error {
	for _, item := range result.Output {
		index := s.section(item.Kind, item.Index, item.SectionIndex)
		s.output[index] = item
	}
	if len(result.Output) == 0 && result.Text != "" {
		message, count, nextIndex := -1, 0, 0
		for i, item := range s.output {
			if item.Index >= nextIndex {
				nextIndex = item.Index + 1
			}
			if item.Kind == "message" {
				message = i
				count++
			}
		}
		if count == 0 {
			s.output = append(s.output, provider.GenerationOutput{Kind: "message", Index: nextIndex, Text: result.Text})
		}
		if count == 1 && generationErr == nil {
			s.output[message].Text = result.Text
		}
	}
	for i := range s.output {
		if generationErr == nil {
			s.output[i].Status = "completed"
		} else if s.output[i].Status != "completed" {
			s.output[i].Status = "failed"
		}
	}
	s.dirty = true
	s.flush(true)
	result.Output = append([]provider.GenerationOutput(nil), s.output...)
	return s.err
}
