import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import {
  importBoard,
  exportBoard,
  netsForSelection,
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
test("effective mask expansion survives on copper pads without mask openings", () => {
  const text = source
    .replace('(pad "1" thru_hole circle', '(pad "1" thru_hole circle (solder_mask_margin 0.2)')
    .replace('(layers "*.Cu" "*.Mask")', '(layers "*.Cu")');
  const pad = load(text).board.components[0].pads[0];
  assert.deepEqual(pad.solderMaskExpansion, {});
  assert.deepEqual(pad.effectiveSolderMaskExpansion, { "F.Mask": 0.2, "B.Mask": 0.2 });
});
test("mask margins inherit by scope while explicit zero and exposed sides survive", () => {
  const input = source
    .replace("(pad_to_mask_clearance 0)", "(pad_to_mask_clearance 0.2)")
    .replace("(at 105 105)", "(at 105 105) (solder_mask_margin 0.15)")
    .replace('(pad "1" thru_hole circle', '(pad "1" thru_hole circle (solder_mask_margin 0)')
    .replace('(layers "*.Cu" "*.Mask")', '(layers "*.Cu" "F.Mask")');
  const pads = load(input).board.components.map((c) => c.pads[0]);
  assert.deepEqual(pads[0].solderMaskExpansion, { "F.Mask": 0 });
  assert.deepEqual(pads[1].solderMaskExpansion, { "F.Mask": 0.15, "B.Mask": 0.15 });
  assert.deepEqual(pads[2].solderMaskExpansion, { "F.Mask": 0.2, "B.Mask": 0.2 });
});
test("negative mask margins on the back remain negative and on the back", () => {
  const input = source
    .replace('(pad "1" thru_hole circle', '(pad "1" thru_hole circle (solder_mask_margin -0.05)')
    .replace('(layers "*.Cu" "*.Mask")', '(layers "*.Cu" "B.Mask")');
  assert.deepEqual(load(input).board.components[0].pads[0].solderMaskExpansion, { "B.Mask": -0.05 });
});
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
  assert.throws(() => load(text), /Copper zones and keepouts/);
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

const legacyClasses = `
  (net_class Power "" (clearance 0.3) (trace_width 0.7) (via_dia 0.8) (via_drill 0.4) (add_net "Return"))
  (net_class Default "" (clearance 0.08) (trace_width 0.15) (via_dia 0.6) (via_drill 0.35) (add_net "Signal"))`;
const legacySource = source.replace('(setup', legacyClasses + ' (setup').replace('20240108', '20171130').replaceAll('(footprint ', '(module ');
test("legacy embedded classes assign exact net rules and survive routing export", () => {
  const input = load(legacySource);
  assert.deepEqual(input.board.netClasses, [
    {name: 'Default', clearance: .08, traceWidth: .15, viaDiameter: .6, viaDrill: .35, netNames: ['Signal']},
    {name: 'Power', clearance: .3, traceWidth: .7, viaDiameter: .8, viaDrill: .4, netNames: ['Return']},
  ]);
  assert.equal(input.board.nets.find(n => n.name === 'Return').className, 'Power');
  assert.equal(input.board.outline.clearance, .08);
  const output = exportBoard(input, input.board);
  assert.ok(output.includes(legacyClasses));
  assert.deepEqual(load(output).board.netClasses, input.board.netClasses);
  assert.equal(load(legacySource.replace('(add_net "Return")', '')).board.nets.find(n => n.name === 'Return').className, 'Default');
});
test("malformed or ambiguous embedded classes fail explicitly", () => {
  assert.throws(() => load(legacySource.replace('(via_drill 0.35)', '(via_drill 0.65)')), /Invalid embedded/);
  assert.throws(() => load(legacySource.replace('(trace_width 0.15)', '(trace_width NaN)')));
  assert.throws(() => load(legacySource.replace('(add_net "Return")', '(add_net "Signal")')), /multiple embedded/);
  assert.throws(() => load(legacySource.replace('net_class Power', 'net_class Default')), /unique/);
});
test("an accompanying project overrides embedded class assignments and dimensions", async () => {
  const {applyProject} = await import('../project.js');
  const input = load(legacySource);
  applyProject(input, JSON.stringify({board: {design_settings: {}}, net_settings: {classes: [{name: 'Default', track_width: .4}]}}));
  assert.equal(input.board.netClasses[0].traceWidth, .4);
  assert.ok(input.board.nets.every(n => n.className === 'Default'));
});

test("offset pad copper retains the drill anchor and local displacement", async () => {
  const input = load(source.replace('(drill 0.8)', '(drill 0.8 (offset 0.25 -0.1))'));
  const pad = input.board.components.flatMap(c=>c.pads).find(p=>p.shapeOffset);
  assert.deepEqual(pad.shapeOffset,{x:.25,y:-.1});
  assert.equal(pad.drill,.8);
});

test("internal cutouts remain separate from ordered outer edges", () => {
  const text = source.replace('(setup','(gr_rect (start 112 110) (end 114 112) (layer "Edge.Cuts")) (setup');
  const input = load(text);
  assert.equal(input.board.outline.cutouts.length,1);
  assert.equal(input.board.outline.ordered,true);
  assert.deepEqual(input.board.outline.corners,load(source).board.outline.corners);
  assert.ok(exportBoard(input,input.board).includes('(gr_rect (start 112 110)'));
  assert.throws(()=>load(text.replace('(start 112 110) (end 114 112)','(start 212 110) (end 214 112)')),/Separate board/);
});

test("legacy signed arcs match the modern three-point geometry", async () => {
  const {nativeArcPoints} = await import('../geometry.js');
  const arc = (text) => parse('(kicad_pcb '+text+')').values[1];
  for(const angle of [90,-90,180,-180]) {
    const ps = nativeArcPoints(arc(`(gr_arc (start 10 10) (end 12 10) (angle ${angle}))`));
    assert.deepEqual(ps[0],{x:12,y:10});
    assert.ok(Math.abs(ps.at(-1).x-(10+2*Math.cos(angle*Math.PI/180)))<1e-9);
    assert.ok(Math.abs(ps.at(-1).y-(10+2*Math.sin(angle*Math.PI/180)))<1e-9);
  }
});

test("copper text and footprint rectangles become layer-specific fixed obstacles", () => {
  const text = source.replace('(setup', '(gr_text "A" (at 112 110 90) (layer "F.Cu") (effects (font (size 1 1) (thickness 0.2)))) (setup')
    .replace('(at 105 105)', '(at 105 105) (fp_rect (start 1 1) (end 2 2) (layer "B.Cu") (stroke (width 0.2)) (fill yes))');
  const input = load(text);
  assert.equal(input.board.conductionAreas.length,2);
  assert.ok(input.board.conductionAreas.every(a=>a.isObstacle && a.netName === ''));
  assert.deepEqual(input.board.conductionAreas.map(a=>a.layerIndex),[0,1]);
  const xs = input.board.conductionAreas[1].polygon.map(p=>p.x);
  assert.equal(Math.min(...xs),105.9);
  assert.equal(Math.max(...xs),107.1);
  assert.ok(exportBoard(input,input.board).includes('(gr_text "A"'));
});

test("a stroked convex custom pad includes its copper, while unsupported primitives fail", () => {
  const pad = '(pad "1" thru_hole circle (at 0 0) (size 1.8 1.8) (drill 0.8)';
  const custom = '(pad "1" thru_hole custom (at 0 0) (size 0.8 0.8) (options (anchor circle)) (primitives (gr_poly (pts (xy -0.8 -0.8) (xy 0 -0.8) (xy 0.8 0) (xy 0.8 0.8) (xy -0.8 0.8)) (width 0.2) (fill yes))) (drill 0.3)';
  const text = source.replace(pad,custom);
  const polygon = load(text).board.components[0].pads[0].copperPolygon;
  assert.ok(polygon.length>5);
  assert.ok(Math.max(...polygon.map(p=>p.y))>=.9);
  assert.ok(Math.max(...polygon.map(p=>p.y))<=.905);
  assert.throws(()=>load(text.replace('(fill yes)','(fill no)')),/filled convex/);
  assert.throws(()=>load(text.replace('(anchor circle)','(anchor rect)')),/circular anchor/);
});

const routedSource = source.replace(
  /\)\s*$/,
  `  (segment (start 105 105) (end 125 115) (width 0.25) (layer "F.Cu") (net 1))
  (segment (start 105 110) (end 125 120) (width 0.25) (layer "F.Cu") (net 2))
)`,
);

test("rips up only the nets named in ripUpNets", () => {
  const { board } = importBoard(routedSource, "example", rules, {
    ripUpRouting: true,
    ripUpNets: new Set(["Signal"]),
  });
  assert.deepEqual(
    board.traces.map((t) => t.netName),
    ["Return"],
    "the deselected net keeps its copper as an obstacle",
  );
});

test("rips up every net when ripUpNets is absent", () => {
  const { board } = importBoard(routedSource, "example", rules, {
    ripUpRouting: true,
  });
  assert.deepEqual(board.traces, []);
});

test("offers every board net with its class for selection", () => {
  assert.deepEqual(netsForSelection(source, "example", rules), [
    { name: "Signal", className: "Default" },
    { name: "Return", className: "Default" },
  ]);
});

test("offers no net selection for a board that cannot be imported", () => {
  assert.equal(netsForSelection("(kicad_pcb)", "broken", rules), null);
});

const arcSource = source.replace(
  /\)\s*$/,
  `  (arc (start 105 105) (mid 112 108) (end 125 115) (width 0.25) (layer "F.Cu") (net 1))
)`,
);

test("refuses a curved track that a deselected net would keep", () => {
  assert.throws(
    () =>
      importBoard(arcSource, "example", rules, {
        ripUpRouting: true,
        ripUpNets: new Set(["Return"]),
      }),
    /Curved tracks are not supported/,
  );
});

test("accepts a curved track on a net that is being ripped up", () => {
  const { board } = importBoard(arcSource, "example", rules, {
    ripUpRouting: true,
    ripUpNets: new Set(["Signal"]),
  });
  assert.deepEqual(board.traces, []);
});

 test("footprint solder-mask bridge permission survives import without losing apertures", () => {
  const text = source.replace("(at 105 105)", "(at 105 105) (attr allow_soldermask_bridges)");
  const pads = load(text).board.components.map(c => c.pads[0]);
  assert.equal(pads[0].allowSolderMaskBridges, true);
  assert.equal(pads[1].allowSolderMaskBridges, true);
  assert.equal(pads[2].allowSolderMaskBridges, false);
  assert.deepEqual(pads[0].solderMaskExpansion, {"F.Mask": 0, "B.Mask": 0});
});

test("local copper clearance inherits with the file version's zero semantics", () => {
  const inherited = source.replace("(at 105 105)", "(at 105 105) (clearance 0.3)")
    .replace('(pad "1" thru_hole circle', '(pad "1" thru_hole circle (clearance 0)');
  const oldPads = load(inherited).board.components.map(c => c.pads[0]);
  assert.equal(oldPads[0].copperClearance, 0.3);
  assert.equal(oldPads[1].copperClearance, 0.3);
  assert.equal(oldPads[2].copperClearance, undefined);
  const newPads = load(inherited.replace("20240108", "20241229")).board.components.map(c => c.pads[0]);
  assert.equal(newPads[0].copperClearance, 0);
  assert.equal(newPads[1].copperClearance, 0.3);
  const own = load(inherited.replace("(clearance 0)", "(clearance 0.4)")).board.components[0].pads[0];
  assert.equal(own.copperClearance, 0.4);
});

 test("negative local copper clearance remains a finite override", () => {
  const text = source.replace('(pad "1" thru_hole circle', '(pad "1" thru_hole circle (clearance -0.1)');
  assert.equal(load(text).board.components[0].pads[0].copperClearance, -0.1);
});

test("board mask web width survives the browser adapter", () => {
  const text = source.replace("(pad_to_mask_clearance 0)", "(pad_to_mask_clearance 0) (solder_mask_min_width 0.2)");
  assert.equal(load(text).board.solderMaskMinWidth, 0.2);
});

test("board-wide footprint mask permission is imported", () => {
 const text=source.replace("(pad_to_mask_clearance 0)","(pad_to_mask_clearance 0) (allow_soldermask_bridges_in_footprints yes)");
 assert.equal(load(text).board.allowSolderMaskBridgesInFootprints,true);
});

test("flattened pads retain their source footprint and pad numbers", () => {
  const pads = load(source).board.components.map(c => c.pads[0]);
  assert.equal(pads[0].sourceFootprint, pads[1].sourceFootprint);
  assert.notEqual(pads[0].sourceFootprint, pads[2].sourceFootprint);
  assert.equal(pads[0].sourcePadNumber, "1");
  assert.equal(pads[1].sourcePadNumber, "2");
});
