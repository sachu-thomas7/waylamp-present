import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: { fs: { allow: [fileURLToPath(new URL(".", import.meta.url)), fileURLToPath(new URL("../src-tauri/src", import.meta.url))] } },
  test: {
    environment: "jsdom",
    globals: true,
    setupFiles: ["./src/test/setup.ts"],
    coverage: {
      provider: "v8",
      include: ["src/**/*.{ts,tsx}"],
      exclude: ["src/test/**", "src/**/*.test.{ts,tsx}", "src/**/*.d.ts"],
      reporter: ["text-summary", "json-summary", "html"],
      reportsDirectory: "./coverage",
    },
  },
});
