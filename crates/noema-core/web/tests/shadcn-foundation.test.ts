import { readFile } from "node:fs/promises";
import { expect, test } from "bun:test";

const root = new URL("..", import.meta.url);

async function read(path: string) {
  return readFile(new URL(path, root), "utf8");
}

test("shadcn/ui is configured as the local Vite UI foundation", async () => {
  const components = JSON.parse(await read("components.json"));
  const packageJson = JSON.parse(await read("package.json"));
  const tsconfig = JSON.parse(await read("tsconfig.json"));
  const viteConfig = await read("vite.config.ts");
  const styles = await read("src/styles.css");
  const utils = await read("src/lib/utils.ts");

  expect(components.tsx).toBe(true);
  expect(components.rsc).toBe(false);
  expect(components.tailwind.css).toBe("src/styles.css");
  expect(components.tailwind.cssVariables).toBe(true);
  expect(components.aliases.components).toBe("@/components");
  expect(components.aliases.ui).toBe("@/components/ui");
  expect(components.aliases.utils).toBe("@/lib/utils");

  expect(tsconfig.compilerOptions.baseUrl).toBe(".");
  expect(tsconfig.compilerOptions.paths["@/*"]).toEqual(["./src/*"]);

  expect(viteConfig).toContain('import path from "node:path"');
  expect(viteConfig).toContain('import tailwindcss from "@tailwindcss/vite"');
  expect(viteConfig).toContain("plugins: [react(), tailwindcss()]");
  expect(viteConfig).toContain('"@": path.resolve(__dirname, "./src")');

  for (const dependency of [
    "tailwindcss",
    "@tailwindcss/vite",
    "tw-animate-css",
    "class-variance-authority",
    "clsx",
    "tailwind-merge",
    "lucide-react",
  ]) {
    expect(packageJson.dependencies[dependency]).toBeDefined();
  }
  expect(packageJson.devDependencies["@types/node"]).toBeDefined();

  for (const token of [
    '@import "tailwindcss";',
    '@import "tw-animate-css";',
    "@custom-variant dark (&:is(.dark *));",
    "@theme inline",
    "--color-background: var(--background);",
    "--color-foreground: var(--foreground);",
    "--color-card: var(--card);",
    "--color-card-foreground: var(--card-foreground);",
    "--color-popover: var(--popover);",
    "--color-popover-foreground: var(--popover-foreground);",
    "--color-primary: var(--primary);",
    "--color-primary-foreground: var(--primary-foreground);",
    "--color-secondary: var(--secondary);",
    "--color-secondary-foreground: var(--secondary-foreground);",
    "--color-muted: var(--muted);",
    "--color-muted-foreground: var(--muted-foreground);",
    "--color-accent: var(--accent);",
    "--color-accent-foreground: var(--accent-foreground);",
    "--color-destructive: var(--destructive);",
    "--color-border: var(--border);",
    "--color-input: var(--input);",
    "--color-ring: var(--ring);",
    "--radius-sm: calc(var(--radius) - 4px);",
    "--radius-md: calc(var(--radius) - 1px);",
    "--radius-lg: var(--radius);",
    "--radius-xl: calc(var(--radius) + 4px);",
    "--radius: var(--radius-md);",
    "--background: var(--surface-page);",
    "--foreground: var(--text-primary);",
    "--card: var(--surface-card);",
    "--card-foreground: var(--text-primary);",
    "--popover: var(--surface-card);",
    "--popover-foreground: var(--text-primary);",
    "--primary: var(--pine-500);",
    "--primary-foreground: var(--paper-50);",
    "--secondary: var(--paper-100);",
    "--secondary-foreground: var(--text-primary);",
    "--muted: var(--paper-100);",
    "--muted-foreground: var(--text-muted);",
    "--accent: var(--paper-100);",
    "--accent-foreground: var(--text-primary);",
    "--destructive: var(--red-700);",
    "--border: var(--border-default);",
    "--input: var(--border-default);",
    "--ring: var(--pine-500);",
  ]) {
    expect(styles).toContain(token);
  }

  expect(utils).toContain('import { clsx, type ClassValue } from "clsx"');
  expect(utils).toContain('import { twMerge } from "tailwind-merge"');
  expect(utils).toContain("export function cn");
});
