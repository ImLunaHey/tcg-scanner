import { defineConfig } from "@playwright/test";

// No server or live scanner: tests fulfill a secure mock origin in-process.
export default defineConfig({
  testDir: "./tests",
  testMatch: "image-cache.spec.ts",
  workers: 1,
  timeout: 30_000,
  projects: [
    { name: "chromium", use: { browserName: "chromium" } },
    { name: "webkit", use: { browserName: "webkit" } },
  ],
});
