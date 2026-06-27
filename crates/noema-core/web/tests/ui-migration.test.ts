import { readFile } from "node:fs/promises";
import { expect, test } from "bun:test";

const root = new URL("..", import.meta.url);

async function read(path: string) {
  return readFile(new URL(path, root), "utf8");
}

test("Noema shell uses shadcn primitives without moving domain state into ui components", async () => {
  const app = await read("src/App.tsx");
  const header = await read("src/components/shell/AppHeader.tsx");
  const composer = await read("src/components/Composer.tsx");
  const emptyState = await read("src/components/EmptyState.tsx");
  const onboarding = await read("src/components/Onboarding.tsx");
  const statusCluster = await read("src/components/StatusCluster.tsx");
  const styles = await read("src/styles.css");

  expect(app).toContain('import { AppHeader } from "@/components/shell/AppHeader"');
  expect(app).not.toContain("function Header(");
  expect(header).toContain('import { StatusCluster } from "@/components/StatusCluster"');
  expect(header).toContain("Local chat");

  expect(composer).toContain('import { Button } from "@/components/ui/button"');
  expect(composer).toContain('import { Textarea } from "@/components/ui/textarea"');
  expect(composer).toContain("<Textarea");
  expect(composer).toContain("<Button");

  expect(emptyState).toContain('import { Button } from "@/components/ui/button"');
  expect(emptyState).toContain('import { Card, CardContent } from "@/components/ui/card"');

  expect(onboarding).toContain('import { Badge } from "@/components/ui/badge"');
  expect(onboarding).toContain('import { Button } from "@/components/ui/button"');
  expect(onboarding).toContain('import { Card, CardContent } from "@/components/ui/card"');

  expect(statusCluster).toContain('import { Badge } from "@/components/ui/badge"');
  expect(statusCluster).toContain('import { cn } from "@/lib/utils"');

  expect(styles).toContain('.onboarding-panel [data-slot="button"]');
  expect(styles).toContain('.auth-attempt [data-slot="button"]');
  expect(styles).toContain("min-height: 40px");
  expect(styles).toMatch(/\.starter-card\s*\{[^}]*padding: 0;/s);
  expect(styles).toMatch(/\.starter-button\s*\{[^}]*width: 100%;[^}]*height: 100%;/s);

  expect(app).toContain("StartPrimaryConversationDocument");
  expect(app).toContain("ConversationEventsDocument");
  expect(app).toContain("sendConversationTurn");
  expect(app).not.toContain("@ai-sdk/react");
});

test("Transcript uses MessageScroller while preserving Noema transcript entry ownership", async () => {
  const transcript = await read("src/components/Transcript.tsx");

  expect(transcript).toContain('from "@/components/ui/message-scroller"');
  expect(transcript).toContain("MessageScrollerProvider");
  expect(transcript).toContain('defaultScrollPosition="last-anchor"');
  expect(transcript).toContain("MessageScrollerItem");
  expect(transcript).toContain("messageId={entry.id}");
  expect(transcript).toContain('scrollAnchor={entry.type === "user"}');
  expect(transcript).toContain('entry.type === "activity"');
  expect(transcript).toContain('entry.type === "card"');
  expect(transcript).toContain("memoryCardsFromStructuredItem");
  expect(transcript).not.toContain("@ai-sdk/react");
  expect(transcript).not.toContain("useChat(");
});
