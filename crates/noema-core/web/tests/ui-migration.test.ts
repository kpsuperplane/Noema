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

  expect(app).toContain("StartPrimaryConversationDocument");
  expect(app).toContain("ConversationEventsDocument");
  expect(app).toContain("sendConversationTurn");
  expect(app).not.toContain("@ai-sdk/react");
});
