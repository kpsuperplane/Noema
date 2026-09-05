import path from "node:path";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const assetOutDir = process.env.NOEMA_DEV_ASSET_DIR ?? "../../target/web-assets";

export default defineConfig({
  plugins: [react()],
  base: "/assets/",
  publicDir: false,
  build: {
    outDir: assetOutDir,
    emptyOutDir: false,
    cssCodeSplit: false,
    rollupOptions: {
      input: path.resolve(__dirname, "graphiql.html"),
      output: {
        entryFileNames: "graphiql-[hash].js",
        chunkFileNames: "graphiql-[name]-[hash].js",
        assetFileNames: "graphiql-[name]-[hash][extname]"
      }
    }
  }
});
