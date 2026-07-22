import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "node:path";
import { fileURLToPath } from "node:url";

const appRoot = path.dirname(fileURLToPath(import.meta.url));
const calendarRoot = path.resolve(appRoot, "../packages/calendar");

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@buddy/calendar": calendarRoot,
      "@buddy/calendar/ui": path.join(calendarRoot, "ui"),
      "@buddy/calendar/models": path.join(calendarRoot, "models"),
      "@buddy/calendar/services": path.join(calendarRoot, "services"),
      "@buddy/calendar/api": path.join(calendarRoot, "api"),
      "@buddy/calendar/utils": path.join(calendarRoot, "utils"),
      "@buddy/calendar/notifications": path.join(calendarRoot, "notifications"),
      "@buddy/calendar/hooks": path.join(calendarRoot, "hooks"),
    },
  },
  server: {
    fs: {
      allow: [appRoot, calendarRoot],
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: ["./vitest.setup.ts"],
    include: [
      "src/**/*.test.{ts,tsx}",
      "../packages/calendar/tests/**/*.test.{ts,tsx}",
    ],
  },
});
