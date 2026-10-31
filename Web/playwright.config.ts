import { defineConfig, devices } from "@playwright/test";

if (!process.env.SCANNER_BROWSER_HOST)
  throw new Error(
    "Set SCANNER_BROWSER_HOST to this Mac’s Tailscale IPv4 for browser tests.",
  );
export default defineConfig({
  testDir: "./tests",
  workers: 1,
  timeout: 45_000,
  use: {
    baseURL: `http://${process.env.SCANNER_BROWSER_HOST}:8171`,
    trace: "retain-on-failure",
  },
  projects: [
    {
      name: "desktop",
      use: {
        ...devices["Desktop Chrome"],
        viewport: { width: 1440, height: 1000 },
      },
    },
    {
      name: "mobile",
      use: { ...devices["iPhone 13"], defaultBrowserType: "chromium" },
    },
    {
      name: "tablet-webkit",
      use: { ...devices["iPad (gen 7)"], defaultBrowserType: "webkit" },
    },
  ],
});
