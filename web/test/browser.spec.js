import { test, expect } from "@playwright/test";
import { readFileSync } from "node:fs";
import { importBoard } from "../kicad.js";
const rules = {
  traceWidth: 0.25,
  clearance: 0.2,
  viaDiameter: 0.6,
  viaDrill: 0.3,
};
// The board rules panel is collapsed on load, so its inputs need it opened.
const openRules = async (page) => {
  const panel = page.locator("#rules-panel");
  if (!(await panel.evaluate((el) => el.open)))
    await page.locator("#rules-panel > summary").click();
};
// Routing uses enclosing rectangles for rounded pads. Check the entire via
// copper disk, including off-centre overlaps, against that routing envelope.
function smdViaOverlaps(board) {
  const overlaps = [];
  for (const via of board.vias) for (const component of board.components) {
    for (const pad of component.pads) {
      if (pad.layers.length !== 1 || pad.drill > 0) continue;
      const layer = board.layers.findIndex(l => l.name === pad.layers[0]);
      if (layer < via.startLayerIndex || layer > via.endLayerIndex) continue;
      // Adapter rotations already use SVG screen coordinates.
      const angle = component.rotation * Math.PI / 180;
      const dx = via.position.x - component.position.x;
      const dy = via.position.y - component.position.y;
      const x = Math.abs(dx * Math.cos(angle) + dy * Math.sin(angle));
      const y = Math.abs(-dx * Math.sin(angle) + dy * Math.cos(angle));
      const hx = pad.size.x / 2, hy = pad.size.y / 2;
      let gap;
      if (pad.shape === "circle") gap = Math.hypot(x, y) - hx;
      else if (pad.shape === "oval") {
        const radius = Math.min(hx, hy);
        gap = Math.hypot(Math.max(0, x - hx + radius), Math.max(0, y - hy + radius)) - radius;
      } else gap = Math.hypot(Math.max(0, x - hx), Math.max(0, y - hy));
      if (gap < via.diameter / 2 - 0.001)
        overlaps.push({via: via.position, pad: component.reference});
    }
  }
  return overlaps;
}
test("via overlap checks use the displayed orientation of diagonal pads", () => {
  for (const [rotation, inside, outside] of [
    [45, {x: .6, y: .6}, {x: .6, y: -.6}],
    [-45, {x: .6, y: -.6}, {x: .6, y: .6}],
  ]) {
    const board = {
      layers: [{name: "F.Cu"}, {name: "B.Cu"}],
      components: [{reference: "U", position: {x: 0, y: 0}, rotation,
        pads: [{shape: "rect", size: {x: 2, y: .5}, layers: ["F.Cu"]}]}],
      vias: [{position: inside, diameter: .2, startLayerIndex: 0, endLayerIndex: 1}],
    };
    expect(smdViaOverlaps(board)).toHaveLength(1);
    board.vias[0].position = outside;
    expect(smdViaOverlaps(board)).toEqual([]);
  }
});

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
  await expect(page.locator("#metrics")).toContainText("0 unrouted connections");
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
    path: "/tmp/copperroute-browser.png",
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
  await page.route("**/pkg/copper_web_bg.wasm", async (route) => {
    await new Promise((resolve) => setTimeout(resolve, 400));
    await route.continue();
  });
  await page.locator("#route").click();
  await page.locator("#cancel").click();
  await expect(page.locator("#badge")).toHaveText("Cancelled");
  await expect(page.locator("#pcb")).toBeHidden();
  await page.unroute("**/pkg/copper_web_bg.wasm");
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
  await page.route("**/pkg/copper_web_bg.wasm", async (route) => {
    await paused;
    await route.continue();
  });
  await page.locator("#route").click();
  await expect(
    page.locator(
      '#preview svg polyline[stroke="#f26d78"], #preview svg polyline[stroke="#68b8ff"]',
    ),
  ).toHaveCount(0);
  await expect(page.locator("#preview svg > path")).toHaveCount(0);
  release();
  await expect(page.locator("#pcb")).toBeVisible({ timeout: 80000 });
  await expect(page.locator("#metrics")).toContainText("0 unrouted connections");
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
  await page.route("**/pkg/copper_web_bg.wasm", async (route) => {
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
    await openRules(page);
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
    expect(smdViaOverlaps(routed)).toEqual([]);
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
  expect(smdViaOverlaps(importBoard(result.pcb, "uno", rules, {rebuildZones: true}).board)).toEqual([]);
  expect(result.initialDrcDetails).toHaveLength(8);
  expect(result.drcDetails).toHaveLength(8);
  expect(result.drcDetails.map(v => Number(v.actual.toFixed(4))).sort()).toEqual(
    [0.1751, 0.1751, 0.1751, 0.1751, 0.2099, 0.2099, 0.2099, 0.2099]);
  expect(result.drcDetails.filter(v => v.kind === 'drill_out_of_range')).toEqual([]);
  expect(result.initialDrcDetails.filter(v => v.kind === 'drill_out_of_range')).toEqual([]);
  expect(result.board.vias.length).toBeGreaterThan(0);
  expect(result.board.vias.every(v => v.drill === .3)).toBe(true);
  expect(importBoard(result.pcb, 'uno', rules, {rebuildZones: true}).board.vias.every(v => v.drill === .3)).toBe(true);
  const { writeFileSync } = await import('node:fs');
  writeFileSync('/tmp/uno-exact-drills.kicad_pcb', result.pcb);
  writeFileSync('/tmp/uno-exact-drills.json', JSON.stringify(result, null, 2));
});

test("Nano rounded pads retain real footprint violations and explain them in the UI", async ({ page }) => {
  await page.goto("/");
  await page.locator("#examples").selectOption("nano");
  await expect(page.locator("#filename")).toHaveText("easyduino-nano.kicad_pcb");
  await expect(page.locator("#allow-via-in-pad")).not.toBeChecked();
  await page.locator("#route").click();
  await expect(page.locator("#pcb")).toBeVisible({ timeout: 80000 });
  const download = page.waitForEvent("download");
  await page.locator("#pcb").click();
  const output = readFileSync(await (await download).path(), "utf8");
  expect(smdViaOverlaps(importBoard(output, "nano", rules, {rebuildZones: true}).board)).toEqual([]);
  await expect(page.locator("#metrics")).toContainText("4 router DRC violations");
  await expect(page.locator("#drc-summary")).toHaveText("DRC details: 4 existing before routing · 0 new");
  await page.locator("#drc-summary").click();
  await expect(page.locator("#drc-list li")).toHaveCount(4);
  for (const row of await page.locator("#drc-list li").all()) {
    await expect(row).toContainText("0.1944 mm (required 0.2500 mm)");
    await expect(row).toContainText("Pin [GND]");
  }
  await page.locator("#demo").click();
  await expect(page.locator("#drc-results")).toBeHidden();
});


test("via-in-pad requires a browser opt-in and resets for the next board", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator("#filename")).toHaveText("easyduino-uno.kicad_pcb");
  await openRules(page);
  await expect(page.locator("#allow-via-in-pad")).not.toBeChecked();
  await page.locator("#allow-via-in-pad").check();
  await page.locator("#passes").fill("1");
  await page.locator("#route").click();
  await expect(page.locator("#pcb")).toBeVisible({timeout: 80000});
  const download = page.waitForEvent("download");
  await page.locator("#pcb").click();
  const output = readFileSync(await (await download).path(), "utf8");
  expect(smdViaOverlaps(importBoard(output, "uno", rules, {rebuildZones: true}).board).length).toBeGreaterThan(0);
  await page.locator("#demo").click();
  await expect(page.locator("#allow-via-in-pad")).not.toBeChecked();
});

test("legacy KiCad rules are shown, used by WASM, and cleared with the board", async ({ page }) => {
  await page.goto('/');
  const source = readFileSync('example.kicad_pcb', 'utf8').replace('(setup', '(net_class Default "" (clearance 0.08) (trace_width 0.15) (via_dia 0.6) (via_drill 0.35) (add_net "Signal") (add_net "Return")) (setup');
  await page.locator('#file').setInputFiles({name: 'legacy.kicad_pcb', mimeType: 'text/plain', buffer: Buffer.from(source)});
  await expect(page.locator('#project-name')).toContainText('embedded KiCad net classes');
  await expect(page.locator('#traceWidth')).toHaveValue('0.15');
  await expect(page.locator('#traceWidth')).toBeDisabled();
  await page.locator('#route').click();
  await expect(page.locator('#pcb')).toBeVisible({timeout: 80000});
  await expect(page.locator('#metrics')).toContainText('0 unrouted connections');
  const downloadPromise = page.waitForEvent('download');
  await page.locator('#pcb').click();
  const output = readFileSync(await (await downloadPromise).path(), 'utf8');
  const board = importBoard(output, 'routed', rules).board;
  expect(board.traces.length).toBeGreaterThan(0);
  expect(board.traces.every(t => t.width === .15)).toBe(true);
  await page.locator('#file').setInputFiles('example.kicad_pcb');
  await expect(page.locator('#traceWidth')).toBeEnabled();
  await expect(page.locator('#project-name')).toContainText('manual rules');
  await expect(page.locator('#traceWidth')).toHaveValue('0.25');
});

test("WASM routes around cutouts with a distinct edge clearance class", async ({page}) => {
  await page.goto('/');
  const source=readFileSync('example.kicad_pcb','utf8').replace('(setup','(gr_rect (start 113 104) (end 117 116) (layer "Edge.Cuts")) (setup');
  await page.locator('#file').setInputFiles({name:'cutout.kicad_pcb',mimeType:'text/plain',buffer:Buffer.from(source)});
  await page.locator('#route').click();
  await expect(page.locator('#pcb')).toBeVisible({timeout:80000});
  await expect(page.locator('#metrics')).toContainText('0 router DRC violations');
  const download=page.waitForEvent('download');
  await page.locator('#pcb').click();
  const board=importBoard(readFileSync(await (await download).path(),'utf8'),'cutout',rules).board;
  expect(board.outline.cutouts).toHaveLength(1);
  expect(board.traces.length).toBeGreaterThan(0);
  for(const trace of board.traces)for(let i=1;i<trace.points.length;i++) {
    const a=trace.points[i-1],b=trace.points[i];
    for(let step=0;step<=20;step++) {
      const x=a.x+(b.x-a.x)*step/20,y=a.y+(b.y-a.y)*step/20;
      expect(x>113 && x<117 && y>104 && y<116).toBe(false);
    }
  }
});

test("routes only the nets left selected in the panel", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/");
  const source = readFileSync("example.kicad_pcb", "utf8");
  await page.evaluate((text) => {
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(
      new File([text], "dropped.kicad_pcb", { type: "text/plain" }),
    );
    document
      .querySelector("#drop")
      .dispatchEvent(
        new DragEvent("drop", { dataTransfer, bubbles: true, cancelable: true }),
      );
  }, source);
  await expect(page.locator("#net-summary")).toHaveText(
    "Nets to route — all 2 selected",
  );
  await page.locator("#net-panel > summary").click();
  await page.locator('#net-list input[value="Return"]').uncheck();
  await expect(page.locator("#net-summary")).toHaveText(
    "Nets to route — 1 of 2 selected",
  );

  await page.locator("#route").click();
  await expect(page.locator("#pcb")).toBeVisible({ timeout: 80000 });
  const download = page.waitForEvent("download");
  await page.locator("#pcb").click();
  const output = readFileSync(await (await download).path(), "utf8");

  const routedNets = new Set(
    [...output.matchAll(/\(segment\b[^\n]*\(net (\d+)\)/g)].map((m) => m[1]),
  );
  expect(routedNets).toEqual(new Set(["1"]));
  await expect(page.locator("#metrics")).toContainText(
    "on nets you did not select",
  );
  expect(errors).toEqual([]);
});

test("board rules collapse into a panel naming the active rule source", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.locator("#filename")).toHaveText("easyduino-uno.kicad_pcb");
  await expect(page.locator("#rules-summary")).toHaveText(
    "Board rules — easyduino-uno.kicad_pro",
  );
  await expect(page.locator("#passes")).toBeHidden();

  await openRules(page);
  await expect(page.locator("#passes")).toBeVisible();

  await page.locator("#clear-project").click();
  await expect(page.locator("#rules-summary")).toHaveText(
    "Board rules — manual",
  );
});

test("routing is blocked while no net is selected", async ({ page }) => {
  await page.goto("/");
  const source = readFileSync("example.kicad_pcb", "utf8");
  await page.evaluate((text) => {
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(
      new File([text], "dropped.kicad_pcb", { type: "text/plain" }),
    );
    document
      .querySelector("#drop")
      .dispatchEvent(
        new DragEvent("drop", { dataTransfer, bubbles: true, cancelable: true }),
      );
  }, source);
  await page.locator("#net-panel > summary").click();
  await page.locator("#nets-none").click();
  await expect(page.locator("#route")).toBeDisabled();
  await page.locator("#nets-all").click();
  await expect(page.locator("#route")).toBeEnabled();
});

test("keeps a deselected net's existing copper in the routed download", async ({
  page,
}) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/");
  const source = readFileSync("example.kicad_pcb", "utf8").replace(
    /\)\s*$/,
    `  (segment (start 105 110) (end 125 120) (width 0.25) (layer "B.Cu") (net 2))
)`,
  );
  await page.evaluate((text) => {
    const dataTransfer = new DataTransfer();
    dataTransfer.items.add(
      new File([text], "prerouted.kicad_pcb", { type: "text/plain" }),
    );
    document
      .querySelector("#drop")
      .dispatchEvent(
        new DragEvent("drop", { dataTransfer, bubbles: true, cancelable: true }),
      );
  }, source);
  await page.locator("#net-panel > summary").click();
  await page.locator('#net-list input[value="Return"]').uncheck();

  await page.locator("#route").click();
  await expect(page.locator("#pcb")).toBeVisible({ timeout: 80000 });
  const download = page.waitForEvent("download");
  await page.locator("#pcb").click();
  const output = readFileSync(await (await download).path(), "utf8");

  const board = importBoard(output, "result", rules).board;
  const returnTraces = board.traces.filter((t) => t.netName === "Return");
  expect(returnTraces).toHaveLength(1);
  expect(returnTraces[0].layerIndex).toBe(
    board.layers.findIndex((l) => l.name === "B.Cu"),
  );
  expect(board.traces.some((t) => t.netName === "Signal")).toBe(true);
  expect(errors).toEqual([]);
});

test("the drop hint covers the loaded board until the first interaction", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.locator("#drop-hint")).toContainText(
    "Drag and drop a KiCad PCB or Specctra DSN here to route",
  );
  await expect(page.locator("#preview svg")).toBeVisible();

  await page.locator("header").click();

  await expect(page.locator("#drop-hint")).toBeHidden();
  await expect(page.locator("#preview svg")).toBeVisible();
});

test("the logo uses the self-hosted Bakbak One face, with no third-party requests", async ({
  page,
}) => {
  const hosts = new Set();
  page.on("request", (r) => hosts.add(new URL(r.url()).hostname));
  await page.goto("/");

  await expect
    .poll(() =>
      page.evaluate(async () => {
        await document.fonts.ready;
        return document.fonts.check('16px "Bakbak One"');
      }),
    )
    .toBe(true);
  expect(
    await page.evaluate(() =>
      getComputedStyle(document.querySelector(".brand")).fontFamily,
    ),
  ).toContain("Bakbak One");
  expect([...hosts]).toEqual(["127.0.0.1"]);
});

test("the install terminal shows the clone and build commands", async ({
  page,
}) => {
  await page.goto("/");
  const terminal = page.locator(".terminal-body");
  await expect(terminal).toContainText(
    "git clone https://github.com/emshotton/copperroute.git",
  );
  await expect(terminal).toContainText("cargo build --release");
  await expect(terminal).toContainText("copperroute route board.dsn");
  await expect(terminal).toContainText("copperroute mcp");
  await expect(terminal).toContainText("claude mcp add copperroute");
});

test("the header links out to the GitHub project", async ({ page }) => {
  await page.goto("/");
  const link = page.locator("header .repo-link");
  await expect(link).toHaveAttribute(
    "href",
    "https://github.com/emshotton/copperroute",
  );
  await expect(link).toHaveAttribute("rel", /noopener/);
  await expect(link).toBeVisible();
});

test("the install terminal spans the same width as the panel and preview above it", async ({
  page,
}) => {
  await page.goto("/");
  const edges = await page.evaluate(() => {
    const w = document.querySelector(".workspace").getBoundingClientRect();
    const t = document.querySelector(".terminal").getBoundingClientRect();
    return {
      workspace: [Math.round(w.left), Math.round(w.right)],
      terminal: [Math.round(t.left), Math.round(t.right)],
    };
  });
  expect(edges.terminal).toEqual(edges.workspace);
});

test("every example loads with its own attribution and reachable licence files", async ({
  page,
}) => {
  const errors = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await page.goto("/");
  await expect(page.locator("#net-panel")).toBeVisible();

  for (const [id, project, license] of [
    ["jacks", "Hubble by wntrblm", "CERN-OHL-P-2.0"],
    ["vca", "CATs-Eurosynth by mzuelch", "MIT"],
    ["uno", "Easyduino by Hanqaqa", "CERN-OHL-P-2.0"],
    ["feather-ice40", "Feather-ICE40-PCB by adafruit", "CC-BY-4.0"],
    ["kfchess", "real-time-chess by misprit7", "MIT"],
  ]) {
    await page.locator("#examples").selectOption(id);
    await expect(page.locator("#example-credit")).toContainText(project, {
      timeout: 60000,
    });
    await expect(page.locator("#example-credit")).toContainText(
      `${license} license`,
    );
    for (const href of await page
      .locator("#example-credit a")
      .evaluateAll((links) => links.map((a) => a.getAttribute("href"))))
      if (href.startsWith("./")) {
        const response = await page.request.get(href.replace("./", "/"));
        expect(response.status(), href).toBe(200);
      }
  }
  expect(errors).toEqual([]);
});

test("a routed example download carries that example's own licence notice", async ({
  page,
}) => {
  await page.goto("/");
  await page.locator("#examples").selectOption("jacks");
  await expect(page.locator("#example-credit")).toContainText("wntrblm", {
    timeout: 60000,
  });

  await page.locator("#route").click();
  await expect(page.locator("#pcb")).toBeVisible({ timeout: 80000 });
  const download = page.waitForEvent("download");
  await page.locator("#pcb").click();
  const output = readFileSync(await (await download).path(), "utf8");

  expect(output).toContain("wntrblm/Hubble");
  expect(output).not.toContain("Easyduino");
});

test("switching to an example without a project drops the previous board's rules", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.locator("#rules-summary")).toHaveText(
    "Board rules — easyduino-uno.kicad_pro",
  );

  await page.locator("#examples").selectOption("jacks");
  await expect(page.locator("#example-credit")).toContainText("wntrblm", {
    timeout: 60000,
  });

  await expect(page.locator("#rules-summary")).not.toContainText("easyduino");
  await expect(page.locator("#project-name")).not.toContainText("easyduino");
});

test("live frames carry a ratsnest for exactly as long as connections remain", async ({
  page,
}) => {
  await page.goto("/");
  const messages = await page.evaluate(
    ({ text, rules }) =>
      new Promise((resolve, reject) => {
        const worker = new Worker("./worker.js", { type: "module" }),
          seen = [];
        const timeout = setTimeout(() => {
          worker.terminate();
          reject(Error("No routing result"));
        }, 20000);
        worker.onmessage = ({ data }) => {
          if (data.svg)
            seen.push({
              type: data.type,
              incomplete: data.incomplete,
              ratsnest: data.svg.includes("ratsnest"),
            });
          if (data.type === "result" || data.type === "error") {
            clearTimeout(timeout);
            worker.terminate();
            data.type === "error" ? reject(Error(data.text)) : resolve(seen);
          }
        };
        worker.postMessage({
          text,
          name: "live.kicad_pcb",
          rules,
          passes: 3,
          seconds: 15,
        });
      }),
    { text: readFileSync("example.kicad_pcb", "utf8"), rules },
  );
  expect(messages.some((m) => m.type === "preview" && m.ratsnest)).toBe(true);
  expect(messages.filter((m) => m.incomplete > 0).length).toBeGreaterThan(0);
  for (const frame of messages.filter((m) => m.incomplete !== undefined))
    expect(frame.ratsnest).toBe(frame.incomplete > 0);
});

test("the unrouted connections of a selected board are drawn and can be hidden", async ({
  page,
}) => {
  await page.goto("/");
  const source = readFileSync("example.kicad_pcb", "utf8");
  await page.locator("#file").setInputFiles({
    name: "unrouted.kicad_pcb",
    mimeType: "text/plain",
    buffer: Buffer.from(source.replace(/\n\s*\(segment [^\n]*\)/g, "")),
  });
  await expect(page.locator("#preview .ratsnest")).toBeVisible();

  await page.locator("#show-ratsnest").uncheck();
  await expect(page.locator("#preview .ratsnest")).toBeHidden();
  await page.locator("#show-ratsnest").check();
  await expect(page.locator("#preview .ratsnest")).toBeVisible();
});

test("routing clears the ratsnest it started with", async ({ page }) => {
  await page.goto("/");
  await page.locator("#demo").click();
  await expect(page.locator("#preview .ratsnest")).toBeVisible();

  await page.locator("#route").click();
  await expect(page.locator("#badge")).toHaveText("Routing complete", {
    timeout: 60000,
  });
  await expect(page.locator("#preview .ratsnest")).toHaveCount(0);
});

test("hiding the unrouted connections survives the live frames of a route", async ({
  page,
}) => {
  await page.goto("/");
  await page.locator("#demo").click();
  await expect(page.locator("#preview .ratsnest")).toBeVisible();
  await page.locator("#show-ratsnest").uncheck();

  await page.locator("#route").click();
  await expect(page.locator("#badge")).toHaveText("Routing complete", {
    timeout: 60000,
  });
  await expect(page.locator("#show-ratsnest")).not.toBeChecked();
});
