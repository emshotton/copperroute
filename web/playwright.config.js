import { defineConfig } from "@playwright/test";
export default defineConfig({
  testDir: "./test",
  testMatch: "*.spec.js",
  timeout: 90000,
  use: { baseURL: "http://127.0.0.1:8080" },
  webServer: {
    command: "npm start",
    url: "http://127.0.0.1:8080",
    reuseExistingServer: true,
  },
});
