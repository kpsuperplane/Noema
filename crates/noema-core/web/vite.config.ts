import path from "node:path";
import react from "@vitejs/plugin-react";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import { defineConfig } from "vite";

const stylexPlugin = [
  "@stylexjs/babel-plugin",
  {
    dev: process.env.NODE_ENV !== "production",
    runtimeInjection: true,
    treeshakeCompensation: true,
    unstable_moduleResolution: {
      type: "commonJS",
      rootDir: __dirname
    }
  }
];

export default defineConfig({
  plugins: [
    tanstackRouter({
      target: "react",
      autoCodeSplitting: true
    }),
    react({
      babel: {
        plugins: [stylexPlugin]
      }
    })
  ],
  base: "/assets/",
  publicDir: "public",
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src")
    }
  },
  build: {
    outDir: "../target/web-assets",
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
