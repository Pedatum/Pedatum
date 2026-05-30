import { test, expect } from "@playwright/test";

test.describe("Shinra Engine IDE", () => {
  test("initial layout shows all panels", async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector('[data-testid="ide-layout"]');

    await expect(page.getByTestId("hierarchy-panel")).toBeVisible();
    await expect(page.getByTestId("inspector-panel")).toBeVisible();
    await expect(page.getByTestId("viewport-panel")).toBeVisible();
    await expect(page.getByTestId("project-panel")).toBeVisible();
    await expect(page.getByTestId("terminal-panel")).toBeVisible();
    await expect(page.getByTestId("console-panel")).toBeVisible();

    await page.screenshot({
      path: "../tests/snapshots/initial.gui.png",
      fullPage: true,
    });
  });

  test("selecting a node updates inspector", async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector('[data-testid="ide-layout"]');

    await page.getByTestId("hierarchy-node-0").click();

    const name = page.getByTestId("inspector-name");
    await expect(name).toHaveText("cube");

    const position = page.getByTestId("inspector-position");
    await expect(position).toHaveText("0.00, 0.00, 0.00");

    await page.screenshot({
      path: "../tests/snapshots/after-select-node.gui.png",
      fullPage: true,
    });
  });
});
