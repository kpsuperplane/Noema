import fs from "node:fs/promises";
import path from "node:path";
import react from "@vitejs/plugin-react";
import stylex from "@stylexjs/unplugin";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import { defineConfig } from "vite";

const stylexPlugin = stylex.vite({
  unstable_moduleResolution: {
    type: "commonJS",
    rootDir: __dirname
  }
});

export default defineConfig({
  plugins: [
    stylexPlugin,
    tanstackRouter({
      target: "react",
      autoCodeSplitting: true
    }),
    react(),
    {
      name: "noema-desktop-public-assets",
      transformIndexHtml(html) {
        return html
          .replaceAll('href="/assets/', 'href="./assets/')
          .replaceAll('src="/assets/', 'src="./assets/');
      },
      async writeBundle() {
        await fs.mkdir(path.resolve(__dirname, "dist-tauri/assets"), {
          recursive: true
        });
        await fs.copyFile(
          path.resolve(__dirname, "public/noema-mark.svg"),
          path.resolve(__dirname, "dist-tauri/assets/noema-mark.svg")
        );
      }
    }
  ],
  base: "./",
  publicDir: "public",
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src")
    }
  },
  build: {
    outDir: "dist-tauri",
    emptyOutDir: true,
    cssCodeSplit: false,
    rollupOptions: {
      output: {
        entryFileNames: "assets/app.js",
        chunkFileNames: "assets/[name].js",
        assetFileNames: (assetInfo) => {
          if (assetInfo.names.some((name) => name.endsWith(".css"))) {
            return "assets/styles.css";
          }
          return "assets/[name][extname]";
        }
      }
    }
  }
});
