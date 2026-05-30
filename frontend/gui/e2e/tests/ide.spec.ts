import { test, expect } from "@playwright/test";

test.describe("Shinra Engine IDE — Full Layout", () => {
  test("initial layout — all 6 panels visible", async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector('[data-testid="ide-layout"]');

    await expect(page.getByTestId("hierarchy-panel")).toBeVisible();
    await expect(page.getByTestId("inspector-panel")).toBeVisible();
    await expect(page.getByTestId("viewport-panel")).toBeVisible();
    await expect(page.getByTestId("project-panel")).toBeVisible();
    await expect(page.getByTestId("terminal-panel")).toBeVisible();
    await expect(page.getByTestId("console-panel")).toBeVisible();

    await page.screenshot({
      path: "/output/snapshots/initial.gui.png",
      fullPage: true,
    });
  });

  test("select node — inspector updates", async ({ page }) => {
    await page.goto("/");
    await page.waitForSelector('[data-testid="ide-layout"]');

    await page.getByTestId("hierarchy-node-0").click();

    await expect(page.getByTestId("inspector-name")).toHaveText("cube");
    await expect(page.getByTestId("inspector-position")).toHaveText(
      "0.00, 0.00, 0.00"
    );

    await page.screenshot({
      path: "/output/snapshots/after-select-node.gui.png",
      fullPage: true,
    });
  });
});
