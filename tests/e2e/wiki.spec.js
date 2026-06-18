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

test("active session hub gathers links and appends quick notes", async ({ page }) => {
  const title = `Field Test Session ${Date.now()}`;
  await page.goto("/pages/new");
  await page.getByLabel("Title").fill(title);
  await page.getByLabel("Content type").selectOption("session_notes");
  await page.getByRole("button", { name: "Raw HTML" }).click();
  await page.locator("#raw-editor").fill("<p>Return to [[Fort Dawn]] before nightfall.</p>");
  await page.getByRole("button", { name: "Save dossier" }).click();

  await expect(page.getByRole("heading", { name: title })).toBeVisible();
  await page.getByRole("button", { name: "Make active session" }).click();
  await expect(page).toHaveURL(/\/?session=active/);
  await expect(page.getByText("Active session")).toBeVisible();
  await expect(page.locator(".session-links").getByRole("link", { name: "Fort Dawn" })).toBeVisible();

  await page.getByLabel("Add a field note").fill("The patrol returned safely.");
  await page.getByRole("button", { name: "Append note" }).click();
  await expect(page.getByRole("status")).toContainText("Field note appended");
  await expect(page.locator(".session-brief")).toContainText("The patrol returned safely.");
});

test("browser drafts can be recovered and discarded", async ({ page }) => {
  await page.goto("/pages/new");
  await page.getByLabel("Title").fill("Recovered Field Draft");
  await page.waitForTimeout(700);
  await page.reload();

  const recovery = page.locator("#draft-recovery");
  await expect(recovery).toBeVisible();
  await recovery.getByRole("button", { name: "Recover draft" }).click();
  await expect(page.getByLabel("Title")).toHaveValue("Recovered Field Draft");

  await page.getByLabel("Title").fill("Discard This Draft");
  await page.waitForTimeout(700);
  await page.reload();
  await expect(recovery).toBeVisible();
  await recovery.getByRole("button", { name: "Discard" }).click();
  await expect(recovery).toBeHidden();
  await expect(page.getByLabel("Title")).toHaveValue("");

  await page.getByLabel("Title").fill(`Saved Draft ${Date.now()}`);
  await page.waitForTimeout(700);
  await page.getByRole("button", { name: "Save dossier" }).click();
  await expect(page.getByRole("status")).toContainText("Dossier saved");
  await page.goto("/pages/new");
  await expect(page.locator("#draft-recovery")).toBeHidden();
});
