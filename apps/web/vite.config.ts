import path from "node:path";
import { randomUUID } from "node:crypto";
import react from "@vitejs/plugin-react";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import { defineConfig } from "vite";
import { VitePWA } from "vite-plugin-pwa";

const stylexPlugin = [
  "@stylexjs/babel-plugin",
  {
    dev: process.env.NODE_ENV !== "production",
    // Astryx ships production property keys; keep cross-package xstyle composition compatible in dev.
    debug: false,
    runtimeInjection: true,
    treeshakeCompensation: true,
    unstable_moduleResolution: {
      type: "commonJS",
      rootDir: __dirname
    }
  }
];

const pwaReleaseId = process.env.NOEMA_PWA_RELEASE ?? randomUUID();

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
    }),
    VitePWA({
      injectRegister: null,
      registerType: "prompt",
      filename: "sw.js",
      scope: "/",
      includeAssets: ["noema-mark.svg", "apple-touch-icon.png", "push-handler.js"],
      manifest: {
        id: "/",
        name: "Noema",
        short_name: "Noema",
        description: "Your personal agent",
        start_url: "/",
        scope: "/",
        display: "standalone",
        background_color: "#fcfaf5",
        theme_color: "#176046",
        icons: [
          {
            src: "/assets/pwa-192x192.png",
            sizes: "192x192",
            type: "image/png",
            purpose: "any maskable"
          },
          {
            src: "/assets/pwa-512x512.png",
            sizes: "512x512",
            type: "image/png",
            purpose: "any maskable"
          }
        ]
      },
      workbox: {
        cacheId: `noema-${pwaReleaseId}`,
        cleanupOutdatedCaches: true,
        clientsClaim: true,
        skipWaiting: false,
        importScripts: ["/assets/push-handler.js"],
        navigateFallback: "/assets/index.html",
        navigateFallbackDenylist: [
          /^\/graphql(?:\/|$)/,
          /^\/auth(?:\/|$)/,
          /^\/__noema(?:\/|$)/,
          /^\/(?:mcp|provider|adapter)\/oauth(?:\/|$)/,
          /^\/artifacts\/versions\/[^/]+\/download$/
        ],
        globPatterns: ["**/*.{html,js,css,svg,png,ttf,webmanifest}"]
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
    outDir: "../../crates/noema-server/target/web-assets",
    manifest: true,
    emptyOutDir: true,
    cssCodeSplit: false,
    rollupOptions: {
      output: {
        entryFileNames: "app-[hash].js",
        chunkFileNames: "[name]-[hash].js",
        assetFileNames: (assetInfo) => {
          if (assetInfo.names.some((name) => name.endsWith(".css"))) {
            return "styles-[hash].css";
          }
          return "[name]-[hash][extname]";
        }
      }
    }
  }
});
