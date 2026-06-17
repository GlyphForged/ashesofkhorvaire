const { defineConfig, devices } = require("@playwright/test");

module.exports = defineConfig({
  testDir: "./tests/e2e",
  fullyParallel: false,
  forbidOnly: Boolean(process.env.CI),
  retries: process.env.CI ? 2 : 0,
  workers: 1,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://127.0.0.1:3100",
    trace: "retain-on-failure",
    screenshot: "only-on-failure"
  },
  projects: [
    {
      name: "chromium",
      use: {
        ...devices["Desktop Chrome"],
        channel: process.platform === "win32" ? "msedge" : undefined
      }
    }
  ],
  webServer: {
    command: "cargo run",
    url: "http://127.0.0.1:3100/healthz",
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
    env: {
      DATABASE_URL: "sqlite://playwright.db",
      BIND_ADDRESS: "127.0.0.1:3100",
      RUST_LOG: "ashes_wiki=warn"
    }
  }
});
