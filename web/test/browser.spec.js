import { test, expect } from "@playwright/test";
import { readFileSync } from "node:fs";
import { importBoard } from "../kicad.js";
const rules = {
  traceWidth: 0.25,
  clearance: 0.2,
  viaDiameter: 0.6,
  viaDrill: 0.3,
};
test("routes dropped PCB entirely in a worker and downloads a readable result", async ({
  page,
}) => {
  const errors = [],
    requests = [];
  page.on("pageerror", (e) => errors.push(e.message));
  page.on("request", (r) => requests.push(r));
  await page.goto("/");
  const source = readFileSync("example.kicad_pcb", "utf8");
  await page.evaluate((text) => {
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(
      new File([text], "dropped.kicad_pcb", { type: "text/plain" }),
    );
    document.querySelector("#drop").dispatchEvent(
      new DragEvent("drop", {
        dataTransfer,
        bubbles: true,
        cancelable: true,
      }),
    );
  }, source);
  await page.locator("#route").click();
  await expect(page.locator("#pcb")).toBeVisible({ timeout: 80000 });
  await expect(page.locator("#status")).toContainText("0 unrouted connections");
  await expect(page.locator("#preview svg")).toBeVisible();
  expect(await page.locator("#preview svg polyline").count()).toBeGreaterThan(
    0,
  );
  const download = page.waitForEvent("download");
  await page.locator("#pcb").click();
  const file = await download;
  expect(file.suggestedFilename()).toBe("dropped-routed.kicad_pcb");
  const output = readFileSync(await file.path(), "utf8");
  expect(
    importBoard(output, "result", rules).board.traces.length,
  ).toBeGreaterThan(0);
  await file.saveAs("/tmp/browser-routed.kicad_pcb");
  const svgDownload = page.waitForEvent("download");
  await page.locator("#svg").click();
  expect(readFileSync(await (await svgDownload).path(), "utf8")).toContain(
    "<svg",
  );
  expect(errors).toEqual([]);
  expect(
    requests.every(
      (r) => r.method() === "GET" && new URL(r.url()).hostname === "127.0.0.1",
    ),
  ).toBe(true);
  await page.setViewportSize({ width: 1440, height: 1100 });
  await page.screenshot({
    path: "/tmp/freerouting-browser.png",
    fullPage: true,
  });
  await page.setViewportSize({ width: 390, height: 844 });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
});
test("invalid input leaves no download and permits retry", async ({ page }) => {
  await page.goto("/");
  await page.locator("#file").setInputFiles({
    name: "bad.kicad_pcb",
    mimeType: "text/plain",
    buffer: Buffer.from("(kicad_pcb (zone (net 1)))"),
  });
  await page.locator("#route").click();
  await expect(page.locator("#status")).toContainText(
    "No displayable board geometry",
  );
  await expect(page.locator("#pcb")).toBeHidden();
  await expect(page.locator("#route")).toBeEnabled();
});
test("cancel terminates the worker and a new attempt succeeds", async ({
  page,
}) => {
  await page.goto("/");
  await page.locator("#demo").click();
  await expect(page.locator("#route")).toBeEnabled();
  await page.route("**/pkg/fr_web_bg.wasm", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 400));
    await route.continue();
  });
  await page.locator("#route").click();
  await page.locator("#cancel").click();
  await expect(page.locator("#badge")).toHaveText("Cancelled");
  await expect(page.locator("#pcb")).toBeHidden();
  await page.unroute("**/pkg/fr_web_bg.wasm");
  await page.locator("#route").click();
  await expect(page.locator("#pcb")).toBeVisible({ timeout: 80000 });
});

test("preview appears on selection and survives a routing compatibility error", async ({
  page,
}) => {
  await page.goto("/");
  const source = readFileSync("example.kicad_pcb", "utf8").replace(
    "(setup",
    '(zone (net 1) (keepout (tracks not_allowed)) (layer "F.Cu")) (setup',
  );
  await page.locator("#file").setInputFiles({
    name: "curved.kicad_pcb",
    mimeType: "text/plain",
    buffer: Buffer.from(source),
  });
  await expect(page.locator("#preview svg")).toBeVisible();
  await expect(page.locator("#pcb")).toBeHidden();
  await page.locator("#route").click();
  await expect(page.locator("#status")).toContainText("keepouts");
  await expect(page.locator("#preview svg")).toBeVisible();
});

test("project file reaches WASM and changes routed track width", async ({
  page,
}) => {
  await page.goto("/");
  await page.locator("#demo").click();
  await expect(page.locator("#route")).toBeEnabled();
  const project = {
    board: { design_settings: { rules: { min_clearance: 0.1 } } },
    net_settings: {
      classes: [
        {
          name: "Default",
          track_width: 0.45,
          clearance: 0.2,
          via_diameter: 0.8,
          via_drill: 0.3,
        },
      ],
    },
  };
  await page.locator("#project").setInputFiles({
    name: "example.kicad_pro",
    mimeType: "application/json",
    buffer: Buffer.from(JSON.stringify(project)),
  });
  await expect(page.locator("#traceWidth")).toBeDisabled();
  await page.locator("#route").click();
  await expect(page.locator("#pcb")).toBeVisible({ timeout: 80000 });
  const promise = page.waitForEvent("download");
  await page.locator("#pcb").click();
  const text = readFileSync(await (await promise).path(), "utf8");
  expect(
    importBoard(text, "project", rules).board.traces.every(
      (t) => t.width === 0.45,
    ),
  ).toBe(true);
  await page.locator("#project").setInputFiles({
    name: "bad.kicad_pro",
    mimeType: "application/json",
    buffer: Buffer.from("{}"),
  });
  await expect(page.locator("#pcb")).toBeHidden();
  await page.locator("#route").click();
  await expect(page.locator("#status")).toContainText(
    "no board.design_settings",
  );
  await expect(page.locator("#preview svg")).toBeVisible();
});

test("Route board discards existing tracks, arcs and vias and routes fresh copper", async ({
  page,
}) => {
  await page.goto("/");
  const source = readFileSync("example.kicad_pcb", "utf8").replace(
    "(setup",
    `
    (segment locked (start 105 105) (end 125 115) (width 0.8) (layer "F.Cu") (net 1))
    (segment (start 105 110) (end 125 120) (width 0.8) (layer "F.Cu") (net 2))
    (arc (start 101 101) (mid 102 100.5) (end 103 101) (width 0.8) (layer "F.Cu") (net 1))
    (via locked (at 128 122) (size 1.5) (drill 0.7) (layers "F.Cu" "B.Cu") (net 1))
    (setup`,
  );
  await page.locator("#file").setInputFiles({
    name: "already-routed.kicad_pcb",
    mimeType: "text/plain",
    buffer: Buffer.from(source),
  });
  await expect(
    page.locator('#preview svg polyline[stroke-width="0.8"]').first(),
  ).toBeVisible();
  let release;
  const paused = new Promise((resolve) => (release = resolve));
  await page.route("**/pkg/fr_web_bg.wasm", async (route) => {
    await paused;
    await route.continue();
  });
  await page.locator("#route").click();
  await expect(
    page.locator(
      '#preview svg polyline[stroke="#f26d78"], #preview svg polyline[stroke="#68b8ff"]',
    ),
  ).toHaveCount(0);
  await expect(page.locator("#preview svg path")).toHaveCount(0);
  release();
  await expect(page.locator("#pcb")).toBeVisible({ timeout: 80000 });
  await expect(page.locator("#status")).toContainText("0 unrouted connections");
  const download = page.waitForEvent("download");
  await page.locator("#pcb").click();
  const text = readFileSync(await (await download).path(), "utf8"),
    board = importBoard(text, "rerouted", rules).board;
  expect(board.traces.length).toBeGreaterThan(0);
  expect(board.traces.every((t) => t.width === 0.25)).toBe(true);
  expect(
    board.vias.some((v) => v.position.x === 128 && v.position.y === 122),
  ).toBe(false);
  expect(text).not.toContain("(arc ");
  expect(text).not.toContain("segment locked");
});

test("worker streams routed board frames before the final result", async ({
  page,
}) => {
  await page.goto("/");
  const text = readFileSync("example.kicad_pcb", "utf8");
  const events = await page.evaluate(
    ({ text, rules }) =>
      new Promise((resolve, reject) => {
        const worker = new Worker("./worker.js", { type: "module" }),
          events = [];
        const timeout = setTimeout(() => {
          worker.terminate();
          reject(Error("No routing result"));
        }, 20000);
        worker.onmessage = ({ data }) => {
          if (data.type === "progress")
            events.push({
              type: data.type,
              pass: data.pass,
              incomplete: data.incomplete,
              hasCopper: data.svg.includes("<polyline"),
            });
          if (data.type === "error") {
            clearTimeout(timeout);
            worker.terminate();
            reject(Error(data.text));
          }
          if (data.type === "result") {
            events.push({ type: "result" });
            clearTimeout(timeout);
            worker.terminate();
            resolve(events);
          }
        };
        worker.postMessage({
          text,
          name: "live",
          rules,
          passes: 2,
          seconds: 10,
          rebuildZones: true,
        });
      }),
    { text, rules },
  );
  expect(events.at(-1).type).toBe("result");
  expect(
    events.some((e) => e.type === "progress" && e.hasCopper && e.pass === 1),
  ).toBe(true);
  expect(
    events.filter((e) => e.type === "progress").length,
  ).toBeGreaterThanOrEqual(2);
});

test("preview accepts replacement drops while routing and opens the picker by keyboard", async ({
  page,
}) => {
  await page.goto("/");
  await page.locator("#demo").click();
  await expect(page.locator("#preview svg")).toBeVisible();
  const picker = page.waitForEvent("filechooser");
  await page.locator("#drop").focus();
  await page.keyboard.press("Enter");
  await picker;
  let release;
  const gate = new Promise((resolve) => (release = resolve));
  await page.route("**/pkg/fr_web_bg.wasm", async (route) => {
    await gate;
    await route.continue().catch(() => {});
  });
  await page.locator("#route").click();
  await expect(page.locator("#cancel")).toBeVisible();
  // Drop events must bubble correctly even when their target is the rendered SVG.
  const text = readFileSync("example.kicad_pcb", "utf8");
  await page.evaluate((text) => {
    const dt = new DataTransfer();
    dt.items.add(new File([text], "replacement.kicad_pcb"));
    document.querySelector("#preview svg").dispatchEvent(
      new DragEvent("dragover", {
        dataTransfer: dt,
        bubbles: true,
        cancelable: true,
      }),
    );
  }, text);
  await expect(page.locator("#drop")).toHaveClass(/drag/);
  await page.evaluate((text) => {
    const dt = new DataTransfer();
    dt.items.add(new File([text], "replacement.kicad_pcb"));
    document.querySelector("#preview svg").dispatchEvent(
      new DragEvent("drop", {
        dataTransfer: dt,
        bubbles: true,
        cancelable: true,
      }),
    );
  }, text);
  await expect(page.locator("#filename")).toHaveText("replacement.kicad_pcb");
  await expect(page.locator("#drop")).not.toHaveClass(/drag/);
  await expect(page.locator("#preview svg")).toBeVisible();
  release();
  await expect(page.locator("#cancel")).toBeHidden();
  await expect(page.locator("#route")).toBeEnabled();
  await expect(page.locator("#pcb")).toBeHidden();
});

test("bundled Uno and Nano load rules and export attributed routing", async ({ page }) => {
  test.setTimeout(120000);
  await page.goto("/");
  for (const [id, width] of [["uno", "0.2"], ["nano", "0.25"]]) {
    if (id === "nano") await page.locator("#examples").selectOption(id);
    await expect(page.locator("#filename")).toHaveText(`easyduino-${id}.kicad_pcb`);
    await expect(page.locator("#preview svg")).toBeVisible();
    await expect(page.locator("#project-name")).toHaveText(`easyduino-${id}.kicad_pro`);
    await expect(page.locator("#traceWidth")).toBeDisabled();
    await expect(page.locator("#traceWidth")).toHaveValue(width);
    await expect(page.locator("#example-credit")).toBeVisible();
    await page.locator("#passes").fill("1");
    await page.locator("#seconds").fill("10");
    await page.locator("#route").click();
    await expect(page.locator("#pcb")).toBeVisible({ timeout: 50000 });
    const download = page.waitForEvent("download");
    await page.locator("#pcb").click();
    const file = await download;
    const text = readFileSync(await file.path(), "utf8");
    expect(text).toContain("CERN-OHL-P-2.0");
    expect(text).toContain("tracks and vias replaced");
    const routed = importBoard(text, id, rules, { rebuildZones: true }).board;
    expect(routed.traces.length).toBeGreaterThan(0);
    expect(routed.layers.map(l => l.name)).toEqual(["F.Cu", "In1.Cu", "In2.Cu", "B.Cu"]);
    expect(routed.layers.every(l => l.type === "signal")).toBe(true);
    if (id === "uno") {
      expect(routed.traces.every(t => [0, 3].includes(t.layerIndex))).toBe(true);
      await expect(page.locator("#example-routing")).toContainText("front and back copper only");
    } else await expect(page.locator("#example-routing")).toBeHidden();
    await file.saveAs(`/tmp/easyduino-${id}-attributed.kicad_pcb`);
    const svg = page.waitForEvent("download");
    await page.locator("#svg").click();
    expect(readFileSync(await (await svg).path(), "utf8")).toContain("<desc>Easyduino");
  }
  await page.screenshot({ path: "/tmp/easyduino-browser.png", fullPage: true });
});

test("delayed default cannot replace an uploaded board", async ({ page }) => {
  let release;
  const gate = new Promise(resolve => release = resolve);
  await page.route("**/examples/easyduino/uno.*", async route => {
    await gate;
    await route.continue();
  });
  await page.goto("/");
  await page.locator("#file").setInputFiles("example.kicad_pcb");
  await expect(page.locator("#preview svg")).toBeVisible();
  release();
  await page.waitForLoadState("networkidle");
  await expect(page.locator("#filename")).toHaveText("example.kicad_pcb");
  await expect(page.locator("#traceWidth")).toBeEnabled();
  await expect(page.locator("#example-credit")).toBeHidden();
});

test("WASM respects inner power planes and preserves the native layer table", async ({ page }) => {
  await page.goto('/');
  const text = readFileSync('example.kicad_pcb', 'utf8').replace('(31 "B.Cu" signal)', '(1 "In1.Cu" power "Ground plane") (2 "In2.Cu" power) (31 "B.Cu" signal)');
  expect(text).toContain('Ground plane');
  await page.locator('#file').setInputFiles({ name: 'planes.kicad_pcb', mimeType: 'text/plain', buffer: Buffer.from(text) });
  await expect(page.locator('#layer-legend')).toContainText('Ground plane (In1.Cu) · plane');
  await page.locator('#route').click();
  await expect(page.locator('#pcb')).toBeVisible({ timeout: 30000 });
  const download = page.waitForEvent('download');
  await page.locator('#pcb').click();
  const output = readFileSync(await (await download).path(), 'utf8');
  const board = importBoard(output, 'planes', rules).board;
  expect(board.layers.map(l => l.type)).toEqual(['signal', 'plane', 'plane', 'signal']);
  expect(board.traces.length).toBeGreaterThan(0);
  expect(board.traces.every(t => [0, 3].includes(t.layerIndex))).toBe(true);
  expect(output).toContain('(1 "In1.Cu" power "Ground plane")');
});

test("Uno DRC and exported vias use actual drills before export", async ({ page }) => {
  test.setTimeout(90000);
  await page.goto('/');
  const result = await page.evaluate(async () => {
    const [text, project] = await Promise.all(['uno.kicad_pcb', 'uno.kicad_pro'].map(n => fetch('./examples/easyduino/' + n).then(r => r.text())));
    return new Promise((resolve, reject) => {
      const worker = new Worker('./worker.js', { type: 'module' });
      worker.onmessage = ({ data }) => {
        if (data.type === 'error') { worker.terminate(); reject(Error(data.text)); }
        if (data.type === 'result') { worker.terminate(); resolve(data); }
      };
      worker.postMessage({text, project, example: 'uno', name: 'uno', rules: {traceWidth: .2, clearance: .15, viaDiameter: .5, viaDrill: .3}, rebuildZones: true, passes: 5, seconds: 60});
    });
  });
  expect(result.drcDetails.filter(v => v.kind === 'drill_out_of_range')).toEqual([]);
  expect(result.initialDrcDetails.filter(v => v.kind === 'drill_out_of_range')).toEqual([]);
  expect(result.board.vias.length).toBeGreaterThan(0);
  expect(result.board.vias.every(v => v.drill === .3)).toBe(true);
  expect(importBoard(result.pcb, 'uno', rules, {rebuildZones: true}).board.vias.every(v => v.drill === .3)).toBe(true);
  const { writeFileSync } = await import('node:fs');
  writeFileSync('/tmp/uno-exact-drills.kicad_pcb', result.pcb);
  writeFileSync('/tmp/uno-exact-drills.json', JSON.stringify(result, null, 2));
});
