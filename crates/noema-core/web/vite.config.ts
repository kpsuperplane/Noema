import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  base: "/assets/",
  publicDir: "public",
  build: {
    outDir: "../src/daemon/web/assets",
    emptyOutDir: true,
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
