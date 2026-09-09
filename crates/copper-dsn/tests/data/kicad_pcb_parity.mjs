// Emits the JS adapter's KiCadBoardJson for one board so the Rust port can be compared to it.
import { readFileSync } from "node:fs";
import { importBoard } from "../../../../web/kicad.js";

const rules = { clearance: 0.2, traceWidth: 0.25, viaDiameter: 0.6, viaDrill: 0.3 };
const text = readFileSync(process.argv[2], "utf8");
const { board } = importBoard(text, process.argv[3], rules, { rebuildZones: true });
process.stdout.write(JSON.stringify(board));
