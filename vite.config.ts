import { defineConfig } from "vite";
import { fileURLToPath } from "node:url";

const src = fileURLToPath(new URL("./src", import.meta.url));

export default defineConfig({
  root: src,
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
    target: "es2021",
    rollupOptions: {
      input: { main: `${src}/index.html`, prefs: `${src}/prefs.html` },
    },
  },
});
