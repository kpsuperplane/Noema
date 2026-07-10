import type { Page } from "playwright";

export async function assertExpectedBootSurface(page: Page, timeoutMs: number): Promise<void> {
  await page
    .locator('main:not([aria-label="Loading Noema"])')
    .first()
    .waitFor({ state: "visible", timeout: timeoutMs });
  const nonblank = await page.locator("#root").evaluate((element) => {
    const text = element.textContent?.trim() ?? "";
    const visibleMain = element.querySelector('main:not([aria-label="Loading Noema"])');
    return text.length > 0 && visibleMain !== null;
  });
  if (!nonblank) throw new Error("browser boot surface is blank");
  if (await page.locator('main[aria-label="Noema status"]').isVisible()) {
    throw new Error("browser boot reached error status");
  }
  await page
    .locator('section[aria-label="Noema onboarding"]')
    .waitFor({ state: "visible", timeout: timeoutMs });
}
