import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 30000,
  use: {
    baseURL: "http://localhost:1420",
    screenshot: "on",
    viewport: { width: 1280, height: 720 },
  },
  webServer: {
    command: "npm run dev",
    port: 1420,
    reuseExistingServer: true,
  },
  snapshotDir: "../tests/snapshots",
  snapshotPathTemplate: "{snapshotDir}/{testFileDir}/{arg}{ext}",
});
