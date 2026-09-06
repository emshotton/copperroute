import { test } from "node:test";
import assert from "node:assert/strict";
import { applyProject } from "../project.js";
const input = () => ({
  warnings: [],
  board: {
    outline: { clearance: 0.2 },
    nets: [{ name: "VCC" }, { name: "USB_D+" }],
    netClasses: [
      {
        name: "Default",
        clearance: 0.2,
        traceWidth: 0.25,
        viaDiameter: 0.6,
        viaDrill: 0.3,
      },
    ],
  },
});
const project = {
  board: {
    design_settings: {
      rules: { min_clearance: 0.15, min_copper_edge_clearance: 0.5 },
    },
  },
  net_settings: {
    classes: [
      {
        name: "Default",
        track_width: 0.2,
        clearance: 0.2,
        via_diameter: 0.6,
        via_drill: 0.3,
      },
      {
        name: "Power",
        track_width: 0.8,
        clearance: 0.4,
        via_diameter: 0.9,
        via_drill: 0.4,
      },
    ],
    netclass_assignments: { VCC: ["Power"] },
    netclass_patterns: [],
  },
};
test("project class assignments drive routing widths and clearances", () => {
  const board = input();
  applyProject(board, JSON.stringify(project));
  assert.equal(board.board.nets[0].className, "Power");
  assert.equal(board.board.nets[1].className, "Default");
  assert.equal(board.board.netClasses[0].traceWidth, 0.2);
  assert.equal(board.board.netClasses[1].traceWidth, 0.8);
  assert.equal(board.board.outline.clearance, 0.5);
});
test("invalid and composite project rules fail explicitly", () => {
  assert.throws(() => applyProject(input(), "{}"));
  const p = structuredClone(project);
  p.net_settings.netclass_assignments.VCC = ["Default", "Power"];
  assert.throws(() => applyProject(input(), JSON.stringify(p)), /Composite/);
});
