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

  expect(packageJson.dependencies.tailwindcss).toBeDefined();
  expect(packageJson.devDependencies["@types/node"]).toBeDefined();
  expect(packageJson.dependencies["class-variance-authority"]).toBeDefined();
  expect(packageJson.dependencies.clsx).toBeDefined();
  expect(packageJson.dependencies["tailwind-merge"]).toBeDefined();
  expect(packageJson.dependencies["lucide-react"]).toBeDefined();

  expect(styles).toContain('@import "tailwindcss";');
  expect(styles).toContain('@import "tw-animate-css";');
  expect(styles).toContain("@theme inline");
  expect(styles).toContain("--color-background: var(--background);");
  expect(styles).toContain("--color-primary: var(--primary);");

  expect(utils).toContain('import { clsx, type ClassValue } from "clsx"');
  expect(utils).toContain('import { twMerge } from "tailwind-merge"');
  expect(utils).toContain("export function cn");
});
