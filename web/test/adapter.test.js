import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  importBoard,
  exportBoard,
  parse,
  renderSvg,
} from "../kicad.js";
const source = readFileSync(
  new URL("../example.kicad_pcb", import.meta.url),
  "utf8",
);
const rules = {
  traceWidth: 0.25,
  clearance: 0.2,
  viaDiameter: 0.6,
  viaDrill: 0.25,
};
const load = (text) => importBoard(text, "example", rules);
test("imports net identities, ordered copper layers, pads, and outline", () => {
  const { board } = load(source);
  assert.equal(board.components.length, 4);
  assert.deepEqual(
    board.layers.map((l) => l.name),
    ["F.Cu", "B.Cu"],
  );
  assert.equal(board.nets[0].name, "Signal");
  assert.equal(board.outline.corners.length, 4);
});
test("rotated back footprint keeps global pad placement and angle", () => {
  const input = load(
    source
      .replace('(layer "F.Cu") (at 105 105)', '(layer "B.Cu") (at 105 105 90)')
      .replace("(at 0 5) (size 1.8 1.8)", "(at 0 5 90) (size 1.8 1.8)"),
  );
  assert.deepEqual(input.board.components[1].position, { x: 110, y: 105 });
  assert.equal(input.board.components[1].rotation, -90);
});
test("replaces routing and preserves all unrelated source bytes", () => {
  const text = source.replace(
    '(net 0 "")',
    '(segment (start 105 105) (end 106 105) (width 0.25) (layer "F.Cu") (net 1))\n  (net 0 "")',
  );
  const input = load(text),
    routed = {
      ...input.board,
      traces: [
        {
          netName: "Signal",
          width: 0.25,
          layerIndex: 1,
          points: [
            { x: 105, y: 105 },
            { x: 125, y: 115 },
          ],
        },
      ],
    };
  const exported = exportBoard(input, routed),
    again = load(exported);
  assert.equal(again.board.traces.length, 1);
  assert.equal(again.board.traces[0].layerIndex, 1);
  const objects = (s) =>
    parse(s)
      .values.filter(
        (n) => n?.values && !["segment", "via"].includes(n.values[0]),
      )
      .map((n) => s.slice(n.start, n.end));
  assert.deepEqual(objects(exported), objects(text));
});
test("exports the core's actual drill dimensions without a browser repair", () => {
  const input = load(source);
  const routed = {...input.board, vias: [{
    netName: "Signal", position: {x: 110, y: 110}, diameter: 0.6,
    drill: 0.35, startLayerIndex: 0, endLayerIndex: 1,
  }]};
  assert.equal(load(exportBoard(input, routed)).board.vias[0].drill, 0.35);
});
test("rejects unsupported geometry and malformed source", () => {
  for (const item of [
    "(zone (net 1))",
    "(arc (start 1 2))",
    '(gr_circle (center 1 2) (end 3 4) (layer "Edge.Cuts"))',
    '(gr_line (start 1 2) (end 3 4) (layer "F.Cu"))',
  ])
    assert.throws(() => load(source.replace("(setup", item + " (setup")));
  assert.throws(() => load(source.slice(0, -3)));
  assert.throws(() => load(source + " garbage"));
  assert.throws(() => load(source.replace("circle (at", "custom (at")));
});
test("does not embed untrusted file metadata in SVG", () => {
  const input = load(source.replace('"Signal"', '"<script>alert(1)</script>"'));
  assert.ok(!renderSvg(input).includes("<script>"));
});

test("KiCad 10 named nets round trip and paste apertures are excluded", () => {
  const modern = source
    .replace("20240108", "20260206")
    .replace('(net 0 "") (net 1 "Signal") (net 2 "Return")', "")
    .replaceAll('(net 1 "Signal")', '(net "Signal")')
    .replaceAll('(net 2 "Return")', '(net "Return")');
  const input = load(modern);
  assert.equal(input.board.nets.length, 2);
  const routed = {
    ...input.board,
    traces: [
      {
        netName: "Signal",
        width: 0.25,
        layerIndex: 0,
        points: [
          { x: 105, y: 105 },
          { x: 125, y: 115 },
        ],
      },
    ],
  };
  const text = exportBoard(input, routed);
  assert.equal(load(text).board.traces[0].netName, "Signal");
  assert.ok(text.includes('(net "Signal")'));
  const aperture = source.replace(
    '(layers "*.Cu" "*.Mask")',
    '(layers "F.Paste")',
  );
  assert.equal(load(aperture).board.components.length, 3);
});
test("zone rebuild preserves zone definitions and removes only cached fill", () => {
  const zone =
    '(zone (net 1) (layer "F.Cu") (polygon (pts (xy 101 101) (xy 129 101) (xy 129 124))) (filled_polygon (layer "F.Cu") (pts (xy 102 102) (xy 128 102) (xy 128 123))))';
  const text = source.replace("(setup", zone + " (setup");
  const input = importBoard(text, "zones", rules, { rebuildZones: true });
  const output = exportBoard(input, input.board);
  assert.ok(output.includes("(zone (net 1)"));
  assert.ok(output.includes("(polygon (pts (xy 101 101)"));
  assert.ok(!output.includes("filled_polygon"));
  assert.equal(input.board.conductionAreas.length, 0);
  assert.throws(() => load(text), /does not yet support/);
});

test("native preview escapes text and displays copper without a routing import", async () => {
  const { previewBoard } = await import("../preview.js");
  const text = source
    .replace('"J1"', '"<script>bad</script>"')
    .replace(
      "(setup",
      '(arc (start 105 105) (mid 110 100) (end 115 105) (width 0.25) (layer "F.Cu") (net 1)) (setup',
    );
  assert.throws(() => load(text));
  const svg = previewBoard(text);
  assert.ok(svg.includes("&lt;script&gt;"));
  assert.ok(!svg.includes("<script>"));
  assert.ok(svg.includes(" A "));
});

test("pour-only keepouts allow routing and remain byte-identical in the download", () => {
  const zone =
    '(zone (layers "F.Cu" "B.Cu") (keepout (tracks allowed) (vias allowed) (pads allowed) (copperpour not_allowed) (footprints allowed)) (polygon (pts (xy 110 110) (xy 112 110) (xy 112 112))))';
  const text = source.replace("(setup", zone + " (setup");
  for (const rebuildZones of [false, true]) {
    const input = importBoard(text, "keepout", rules, {
      ripUpRouting: true,
      rebuildZones,
    });
    assert.ok(exportBoard(input, input.board).includes(zone));
    assert.equal(input.board.conductionAreas.length, 0);
    assert.ok(input.warnings.some((w) => w.includes("1 keepout areas")));
    assert.ok(!input.warnings.some((w) => w.includes("1 copper zones")));
  }
  for (const restricted of ["tracks", "vias"])
    assert.throws(
      () =>
        importBoard(
          text.replace(
            `(${restricted} allowed)`,
            `(${restricted} not_allowed)`,
          ),
          "keepout",
          rules,
          { ripUpRouting: true, rebuildZones: true },
        ),
      /restrict tracks or vias/,
    );
});

test("KiCad 10 copper IDs, aliases and plane restrictions survive export", async () => {
  const uno = readFileSync(new URL('../examples/easyduino/uno.kicad_pcb', import.meta.url), 'utf8')
    .replace('(4 "In1.Cu" signal)', '(4 "In1.Cu" power "Ground plane")');
  const input = importBoard(uno, 'uno', rules, { ripUpRouting: true, rebuildZones: true });
  assert.deepEqual(input.board.layers, [
    { index: 0, name: 'F.Cu', type: 'signal' },
    { index: 1, name: 'In1.Cu', type: 'plane' },
    { index: 2, name: 'In2.Cu', type: 'signal' },
    { index: 3, name: 'B.Cu', type: 'signal' },
  ]);
  const exported = exportBoard(input, input.board);
  const layerBlock = text => { const n = parse(text).values.find(n => n?.values?.[0] === 'layers'); return text.slice(n.start, n.end); };
  assert.equal(layerBlock(exported), layerBlock(uno));
  const changed = structuredClone(input.board);
  changed.layers[0].name = 'F.SilkS';
  changed.traces = [{ layerIndex: 0, netName: input.board.nets[0].name, width: 0.2, points: [{ x: 1, y: 1 }, { x: 2, y: 2 }] }];
  assert.throws(() => exportBoard(input, changed), /layer mapping/);
  changed.traces[0].layerIndex = 1;
  assert.throws(() => exportBoard(input, changed), /non-routing layer/);
  const { previewLayers, layerColor } = await import('../preview.js');
  assert.equal(previewLayers(uno)[1].label, 'Ground plane');
  assert.notEqual(layerColor('In1.Cu'), layerColor('In2.Cu'));
  assert.notEqual(layerColor('In1.Cu'), layerColor('F.SilkS'));
});

test("KiCad via attachment is opt-in independently of project net classes", () => {
  assert.equal(load(source).board.viaInPadAllowed, false);
  assert.equal(importBoard(source, "example", rules, {allowViaInPad: true}).board.viaInPadAllowed, true);
});
