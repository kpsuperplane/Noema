package store

import "github.com/kpsuperplane/noema/internal/toolmarker"

// decorateConversationToolMarker adds the marker used by a live transcript
// event. Replay computes the same marker from the stored action envelope.
func decorateConversationToolMarker(item *ConversationItem, marker map[string]any) {
	if item == nil || item.Kind != ConversationToolCall && item.Kind != ConversationToolResult {
		return
	}
	activityKind, _ := item.Payload["activity_kind"].(string)
	if activityKind != "tool_call" && activityKind != "tool_result" {
		return
	}
	if marker == nil {
		metadata, _ := item.Payload["metadata"].(map[string]any)
		action, _ := metadata["action"].(map[string]any)
		marker, _ = toolmarker.ForAction(activityKind, item.Status, action)
	}
	metadata, _ := item.Payload["metadata"].(map[string]any)
	if metadata == nil {
		metadata = map[string]any{}
		item.Payload["metadata"] = metadata
	}
	display, _ := metadata["display"].(map[string]any)
	if display == nil {
		display = map[string]any{}
		metadata["display"] = display
	}
	action, _ := metadata["action"].(map[string]any)
	if name, _ := action["name"].(string); name != "" {
		display["name"] = toolmarker.ReadableName(name)
	}
	if marker != nil {
		display["marker"] = marker
	}
}
