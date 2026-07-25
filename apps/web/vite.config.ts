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
    react()
  ],
  base: "/assets/",
  publicDir: "public",
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src")
    }
  },
  build: {
    outDir: "../../crates/noema-server/target/web-assets",
    manifest: true,
    emptyOutDir: true,
    cssCodeSplit: false,
    rollupOptions: {
      output: {
        entryFileNames: "app.js",
        chunkFileNames: "[name].js",
        assetFileNames: (assetInfo) => {
          if (assetInfo.names.some((name) => name.endsWith(".css"))) {
            return "styles.css";
          }
          return "[name][extname]";
        }
      }
    }
  }
});
