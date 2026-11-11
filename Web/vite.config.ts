import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  build: {
    sourcemap: false,
    rollupOptions: {
      output: {
        entryFileNames: "app.js",
        assetFileNames: asset => asset.names?.some(name => name.endsWith(".css"))
          ? "app.css" : "assets/[name]-[hash][extname]",
      },
    },
  },
});
