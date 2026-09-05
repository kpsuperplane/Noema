package runtime

import (
	"testing"

	"github.com/kpsuperplane/noema/internal/provider"
)

func TestFoundationGeneratorIsReachableFromChatAndTask(t *testing.T) {
	foundation := &provider.FoundationGenerator{}
	chat := &Chat{foundation: foundation}
	if got, err := chat.generatorFor("foundation_local"); err != nil || got != foundation {
		t.Fatalf("Chat Foundation generator = %T, %v", got, err)
	}
	tasks := &TaskExecution{foundation: foundation}
	if got, err := tasks.generator("foundation_local"); err != nil || got != foundation {
		t.Fatalf("Task Foundation generator = %T, %v", got, err)
	}
}
