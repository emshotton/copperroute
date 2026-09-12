import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { EXAMPLES, modificationNotice, svgNotice } from "../examples.js";

const pcb = '(kicad_pcb (version 20221018) (title_block (title "board")))';

test("the download notice names the example's own project and licence", () => {
  const out = modificationNotice(pcb, "kfchess", "2026-09-07");
  assert.match(out, /misprit7\/real-time-chess/);
  assert.match(out, /MIT/);
  assert.doesNotMatch(out, /Easyduino/);
});

test("the SVG notice names the example's own project and licence", () => {
  const out = svgNotice("<svg><g/></svg>", "feather-ice40", "2026-09-07");
  assert.match(out, /adafruit\/Feather-ICE40-PCB/);
  assert.match(out, /CC-BY-4\.0/);
  assert.doesNotMatch(out, /Easyduino/);
});

test("the Easyduino examples keep their own attribution", () => {
  const out = modificationNotice(pcb, "uno", "2026-09-07");
  assert.match(out, /Hanqaqa\/Easyduino/);
  assert.match(out, /CERN-OHL-P-2\.0/);
});

test("every example carries the attribution its downloads need", () => {
  for (const [id, example] of Object.entries(EXAMPLES)) {
    const c = example.credit;
    assert.ok(c, `${id} has no credit`);
    for (const field of ["author", "source", "license", "commit"])
      assert.ok(c[field], `${id} credit is missing ${field}`);
  }
});

test("every example ships the files it declares, and its rules come from the board", () => {
  const web = new URL("../", import.meta.url);
  for (const [id, example] of Object.entries(EXAMPLES)) {
    const pcbPath = new URL(example.base.replace(/^\.\//, "") + ".kicad_pcb", web);
    assert.ok(existsSync(pcbPath), `${id} declares a board that is not shipped`);
    if (example.projectName) {
      const proPath = new URL(example.base.replace(/^\.\//, "") + ".kicad_pro", web);
      assert.ok(existsSync(proPath), `${id} declares projectName but ships no .kicad_pro`);
      const classes = JSON.parse(readFileSync(proPath, "utf8")).net_settings.classes;
      assert.ok(
        classes.some((c) => c.name === "Default"),
        `${id} project has no Default class for the rule fields to read`,
      );
    } else {
      // Without a project file the board has to carry its own rules, or the demo routes it
      // to whatever the rule fields happen to say rather than to the design's intent.
      const pcb = readFileSync(pcbPath, "utf8");
      assert.match(pcb, /\(net_class /, `${id} has neither a .kicad_pro nor embedded net classes`);
    }
  }
});
