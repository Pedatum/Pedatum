import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 30000,
  use: {
    baseURL: process.env.BASE_URL || "http://localhost:1420",
    screenshot: "on",
    viewport: { width: 1280, height: 720 },
  },
  outputDir: "/output/test-results",
  snapshotDir: "/output/snapshots",
  reporter: [["html", { outputFolder: "/output/report" }], ["list"]],
});
