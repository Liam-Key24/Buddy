import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "node:path";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss()],
  clearScreen: false,
  resolve: {
    alias: {
      "@buddy/calendar": path.resolve(__dirname, "../packages/calendar"),
      "@buddy/calendar/ui": path.resolve(__dirname, "../packages/calendar/ui"),
      "@buddy/calendar/models": path.resolve(
        __dirname,
        "../packages/calendar/models",
      ),
      "@buddy/calendar/services": path.resolve(
        __dirname,
        "../packages/calendar/services",
      ),
      "@buddy/calendar/api": path.resolve(__dirname, "../packages/calendar/api"),
      "@buddy/calendar/utils": path.resolve(
        __dirname,
        "../packages/calendar/utils",
      ),
      "@buddy/calendar/notifications": path.resolve(
        __dirname,
        "../packages/calendar/notifications",
      ),
      "@buddy/calendar/hooks": path.resolve(
        __dirname,
        "../packages/calendar/hooks",
      ),
    },
  },
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
