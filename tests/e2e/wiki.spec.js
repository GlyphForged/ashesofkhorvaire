const { test, expect } = require("@playwright/test");

test("dashboard supports live campaign search", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Fort Dawn Command Desk" })).toBeVisible();
  await page.locator("#nav-search").fill("purifier");

  const results = page.locator("#nav-results");
  await expect(results.getByRole("link", { name: /Redwater/ })).toBeVisible();
});

test("visual and raw HTML editor modes stay synchronized", async ({ page }) => {
  await page.goto("/pages/fort-dawn/edit");

  const visualEditor = page.locator("#visual-editor .pell-content");
  const rawEditor = page.locator("#raw-editor");
  await expect(visualEditor).toContainText("A fortified settlement");

  await page.getByRole("button", { name: "Raw HTML" }).click();
  await expect(rawEditor).toBeVisible();
  await expect(rawEditor).toHaveValue(/A fortified settlement/);

  await rawEditor.fill("<p>Round-trip field note</p>");
  await page.getByRole("button", { name: "Visual" }).click();
  await expect(visualEditor).toHaveText("Round-trip field note");

  await page.getByRole("button", { name: "Raw HTML" }).click();
  await expect(rawEditor).toHaveValue("<p>Round-trip field note</p>");
});

test("new dossier form previews a generated slug", async ({ page }) => {
  await page.goto("/pages/new");
  await page.getByLabel("Title").fill("Sharn's Last Beacon");

  await expect(page.locator("#slug-preview code")).toHaveText("sharn-s-last-beacon");
});

test("revision history loads as an HTMX enhancement", async ({ page }) => {
  await page.goto("/pages/fort-dawn");
  await page.getByRole("button", { name: "Load revision history" }).click();

  const panel = page.locator("#revision-panel");
  await expect(panel.getByRole("heading", { name: "Revision history" })).toBeVisible();
  await expect(panel).toContainText("Initial campaign seed");
});
