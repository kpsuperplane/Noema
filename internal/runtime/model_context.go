package runtime

import (
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"github.com/kpsuperplane/noema/internal/provider"
	"github.com/kpsuperplane/noema/internal/store"
)

// syncModelContext keeps earlier section messages intact and appends only changes.
// Compaction compares current sections with the retained history.
func (c *Chat) syncModelContext(turn store.ConversationTurn, state *store.ConversationContext, developer, retained []provider.GenerationMessage) (base, delta, snapshot []provider.GenerationMessage, err error) {
	if retained == nil {
		if state.RecentJSON != "" && json.Unmarshal([]byte(state.RecentJSON), &retained) != nil {
			return nil, nil, nil, fmt.Errorf("invalid saved model context checkpoint")
		}
		for _, item := range state.Items {
			if item.Kind != store.ConversationModelContextUpdate {
				continue
			}
			encoded, marshalErr := json.Marshal(item.Payload["model_context_update"])
			var update store.ModelContextUpdate
			if marshalErr != nil || json.Unmarshal(encoded, &update) != nil || update.SectionID == "" {
				return nil, nil, nil, fmt.Errorf("invalid saved model context update: %s", item.ID)
			}
			retained = append(retained, contextUpdateMessage(update))
		}
	}
	base, updates, snapshot, err := diffModelContext(retained, developer)
	if err != nil {
		return nil, nil, nil, err
	}
	items, err := c.database.AppendModelContextUpdates(c.ctx, turn, updates, time.Now())
	if err != nil {
		return nil, nil, nil, err
	}
	state.Items = append(state.Items, items...)
	for _, update := range updates {
		delta = append(delta, contextUpdateMessage(update))
	}
	return base, delta, snapshot, nil
}

func diffModelContext(retained, developer []provider.GenerationMessage) (base []provider.GenerationMessage, updates []store.ModelContextUpdate, snapshot []provider.GenerationMessage, err error) {
	previous := map[string]string{}
	for _, message := range retained {
		content, section := strings.CutPrefix(message.Content, "NOEMA_MODEL_CONTEXT_UPDATE\n")
		if !section || (message.Role != "developer" && message.Role != "system") {
			continue
		}
		var update store.ModelContextUpdate
		if err := json.Unmarshal([]byte(content), &update); err != nil {
			return nil, nil, nil, err
		}
		if update.Operation == "removal" {
			delete(previous, update.SectionID)
		} else {
			previous[update.SectionID] = update.Content
		}
	}
	current := map[string]store.ModelContextUpdate{}
	for _, message := range developer {
		content, section := strings.CutPrefix(message.Content, "NOEMA_MODEL_CONTEXT_UPDATE\n")
		if !section {
			base = append(base, message)
			continue
		}
		var update store.ModelContextUpdate
		if err := json.Unmarshal([]byte(content), &update); err != nil {
			return nil, nil, nil, err
		}
		current[update.SectionID] = update
	}
	for _, key := range []string{"agent.identity", "runtime.environment", "projects.catalog", "tools.visibility"} {
		update, exists := current[key]
		old, known := previous[key]
		if exists {
			snapshot = append(snapshot, contextUpdateMessage(update))
			if known && old == update.Content {
				continue
			}
			if known {
				update.Operation = "replacement"
				update.Instruction = "Replace the previous value for this section in full; do not retain omitted fields."
			}
		} else {
			if !known {
				continue
			}
			update = store.ModelContextUpdate{SectionID: key, Operation: "removal", Instruction: "Remove this section; do not use any previous value for it."}
		}
		updates = append(updates, update)
	}
	return base, updates, snapshot, nil
}

func contextUpdateMessage(update store.ModelContextUpdate) provider.GenerationMessage {
	role := "developer"
	if update.SectionID == "runtime.environment" {
		role = "system"
	}
	return provider.GenerationMessage{Role: role, Content: update.Message()}
}
