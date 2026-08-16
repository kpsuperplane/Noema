import path from "node:path";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  base: "/assets/",
  build: {
    outDir: "../../crates/noema-server/target/web-assets",
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
