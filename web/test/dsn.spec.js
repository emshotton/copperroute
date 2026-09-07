import { test, expect } from "@playwright/test";
import { readFileSync } from "node:fs";

test("DSN drop previews, reroutes with embedded rules, and downloads DSN/SES/SVG", async ({ page }) => {
  const errors = [];
  page.on("pageerror", e => errors.push(e.message));
  await page.goto("/");
  await page.evaluate(text => {
    const dt = new DataTransfer();
    dt.items.add(new File([text], "design.DSN"));
    document.querySelector("#drop").dispatchEvent(new DragEvent("drop", { dataTransfer: dt, bubbles: true, cancelable: true }));
  }, readFileSync("test/example.dsn", "utf8"));
  await expect(page.locator("#filename")).toHaveText("design.DSN");
  await expect(page.locator("#preview svg")).toBeVisible();
  await expect(page.locator("#layer-legend")).toContainText("Top");
  await expect(page.locator("#layer-legend")).toContainText("Bottom");
  await expect(page.locator("#traceWidth")).toBeDisabled();
  await expect(page.locator("#project")).toBeDisabled();
  await expect(page.locator("#allow-via-in-pad")).toBeDisabled();
  await expect(page.locator("#project-name")).toContainText("embedded DSN");
  await page.locator("#route").click();
  await expect(page.locator("#pcb")).toBeVisible({ timeout: 30000 });
  await expect(page.locator("#metrics")).toContainText("0 unrouted connections");
  const outputs = {};
  for (const [id, ext] of [["pcb", "dsn"], ["ses", "ses"], ["svg", "svg"]]) {
    const pending = page.waitForEvent("download");
    await page.locator("#" + id).click();
    const file = await pending;
    expect(file.suggestedFilename()).toBe("design-routed." + ext);
    outputs[ext] = readFileSync(await file.path(), "utf8");
  }
  expect(outputs.dsn).toMatch(/path Top 450/);
  expect(outputs.dsn).not.toMatch(/path Top 800/);
  expect(outputs.dsn).toMatch(/layer_rule Bottom\s+\(active off\)/);
  expect(outputs.ses).toContain("(session");
  expect(outputs.ses).toContain("(base_design design.DSN)");
  expect(outputs.svg).toContain("<polyline");
  await page.locator("#file").setInputFiles({ name: "roundtrip.dsn", mimeType: "text/plain", buffer: Buffer.from(outputs.dsn) });
  await expect(page.locator("#preview svg polyline")).not.toHaveCount(0);
  await page.locator("#route").click();
  await expect(page.locator("#pcb")).toBeVisible({ timeout: 30000 });
  await expect(page.locator("#metrics")).toContainText("0 unrouted connections");
  await page.screenshot({ path: "/tmp/copperroute-dsn.png", fullPage: true });
  await page.locator("#demo").click();
  await expect(page.locator("#project")).toBeEnabled();
  await expect(page.locator("#traceWidth")).toBeEnabled();
  await expect(page.locator("#ses")).toBeHidden();
  expect(errors).toEqual([]);
});

test("invalid DSN reports an error and allows replacement", async ({ page }) => {
  await page.goto("/");
  await page.locator("#file").setInputFiles({ name: "bad.dsn", mimeType: "text/plain", buffer: Buffer.from("not a board") });
  await page.locator("#route").click();
  await expect(page.locator("#badge")).toHaveText("Could not route");
  await expect(page.locator("#pcb")).toBeHidden();
  await expect(page.locator("#ses")).toBeHidden();
  await expect(page.locator("#route")).toBeEnabled();
});

test("DSN worker streams routing geometry before completion", async ({ page }) => {
  await page.goto("/");
  const events = await page.evaluate(text => new Promise((resolve, reject) => {
    const worker = new Worker("./worker.js", { type: "module" });
    const frames = [];
    const timeout = setTimeout(() => { worker.terminate(); reject(Error("DSN routing timed out")); }, 15000);
    worker.onmessage = ({ data }) => {
      if (data.type === "progress") frames.push(data);
      if (data.type === "result" || data.type === "error") {
        clearTimeout(timeout);
        worker.terminate();
        if (data.type === "error") reject(Error(data.text));
        else resolve(frames);
      }
    };
    worker.postMessage({ text, name: "live.dsn", passes: 2, seconds: 10 });
  }), readFileSync("test/example.dsn", "utf8"));
  expect(events.some(frame => frame.svg.includes("<polyline"))).toBe(true);
  expect(events.at(-1).incomplete).toBe(0);
});
