import { readFile } from "node:fs/promises";
import { expect, test } from "bun:test";

const root = new URL("..", import.meta.url);

async function read(path: string) {
  return readFile(new URL(path, root), "utf8");
}

test("initial shadcn primitives are source-owned in components/ui", async () => {
  const expectedFiles = [
    "src/components/ui/button.tsx",
    "src/components/ui/textarea.tsx",
    "src/components/ui/badge.tsx",
    "src/components/ui/card.tsx",
    "src/components/ui/separator.tsx",
    "src/components/ui/sheet.tsx",
    "src/components/ui/tabs.tsx",
    "src/components/ui/dropdown-menu.tsx",
    "src/components/ui/sonner.tsx",
  ];

  for (const path of expectedFiles) {
    const source = await read(path);
    expect(source.length).toBeGreaterThan(200);
    expect(source).toContain("@/lib/utils");
  }

  expect(await read("src/components/ui/button.tsx")).toContain("buttonVariants");
  expect(await read("src/components/ui/textarea.tsx")).toContain(
    'ComponentProps<"textarea">',
  );
  expect(await read("src/components/ui/badge.tsx")).toContain("badgeVariants");
  expect(await read("src/components/ui/card.tsx")).toContain("CardContent");
  expect(await read("src/components/ui/sheet.tsx")).toContain("SheetContent");
  expect(await read("src/components/ui/tabs.tsx")).toContain("TabsContent");
  expect(await read("src/components/ui/dropdown-menu.tsx")).toContain(
    "DropdownMenuContent",
  );
  expect(await read("src/components/ui/sonner.tsx")).toContain("Toaster");
});

test("radix-backed primitives use radix data attributes", async () => {
  const tabs = await read("src/components/ui/tabs.tsx");
  const separator = await read("src/components/ui/separator.tsx");
  const sheet = await read("src/components/ui/sheet.tsx");
  const dropdownMenu = await read("src/components/ui/dropdown-menu.tsx");

  expect(tabs).toContain("orientation={orientation}");
  expect(tabs).toContain("data-[orientation=horizontal]");
  expect(tabs).toContain("data-[orientation=vertical]");
  expect(tabs).toContain("data-[state=active]");
  expect(tabs).not.toContain("data-horizontal:");
  expect(tabs).not.toContain("data-vertical:");
  expect(tabs).not.toContain("data-active:");

  expect(separator).toContain("data-[orientation=horizontal]");
  expect(separator).toContain("data-[orientation=vertical]");
  expect(separator).not.toContain("data-horizontal:");
  expect(separator).not.toContain("data-vertical:");

  expect(sheet).toContain("data-[state=open]");
  expect(sheet).toContain("data-[state=closed]");
  expect(sheet).not.toContain("data-open:");
  expect(sheet).not.toContain("data-closed:");

  expect(dropdownMenu).toContain("data-[state=open]");
  expect(dropdownMenu).toContain("data-[state=closed]");
  expect(dropdownMenu).not.toContain("data-open:");
  expect(dropdownMenu).not.toContain("data-closed:");
});
