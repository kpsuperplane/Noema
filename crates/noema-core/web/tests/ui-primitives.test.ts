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
