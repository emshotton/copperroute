import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { previewBoard } from "../preview.js";

const source = readFileSync(
  new URL("../example.kicad_pcb", import.meta.url),
  "utf8",
);

test("a board with no airlines draws no ratsnest group", () => {
  assert.ok(!previewBoard(source).includes("ratsnest"));
});

test("airlines are drawn as one dashed path over the copper", () => {
  const svg = previewBoard(source, [
    { net: 1, from: [2, 2], to: [12, 2] },
    { net: 2, from: [3, 4], to: [5, 6] },
  ]);
  assert.match(svg, /class="ratsnest"/);
  assert.match(svg, /d="M 2 2 L 12 2 M 3 4 L 5 6"/);
});

test("the ratsnest sits above the copper it crosses", () => {
  const svg = previewBoard(source, [{ net: 1, from: [2, 2], to: [12, 2] }]);
  assert.ok(svg.indexOf("ratsnest") > svg.lastIndexOf("<polyline"));
});

test("an airline reaching past the copper widens the viewBox to fit", () => {
  const far = previewBoard(source, [{ net: 1, from: [0, 0], to: [900, 900] }]);
  const [, , width] = /viewBox="([-\d.]+) ([-\d.]+) ([\d.]+)/.exec(far).slice(1);
  assert.ok(Number(width) > 900, `viewBox width was ${width}`);
});

test("a malformed airline coordinate is rejected rather than drawn", () => {
  assert.throws(() =>
    previewBoard(source, [{ net: 1, from: [0, Number.NaN], to: [1, 1] }]),
  );
});
