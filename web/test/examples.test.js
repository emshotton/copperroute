import { test } from "node:test";
import assert from "node:assert/strict";
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
