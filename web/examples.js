import { parse } from "./kicad.js";
export const EXAMPLES = {
  uno: {
    name: "Easyduino Uno",
    routingLayers: ["F.Cu", "B.Cu"],
    base: "./examples/easyduino/uno",
    pcbName: "easyduino-uno.kicad_pcb",
    projectName: "easyduino-uno.kicad_pro",
  },
  nano: {
    name: "Easyduino Nano",
    base: "./examples/easyduino/nano",
    pcbName: "easyduino-nano.kicad_pcb",
    projectName: "easyduino-nano.kicad_pro",
  },
};
export function modificationNotice(
  pcb,
  example,
  date = new Date().toISOString().slice(0, 10),
) {
  if (!EXAMPLES[example]) return pcb;
  const root = parse(pcb),
    block = root.values.find((n) => n?.values?.[0] === "title_block");
  const note = `Easyduino by Hanqaqa and contributors; CERN-OHL-P-2.0. Source: https://github.com/Hanqaqa/Easyduino at 53b14b66d64f25c55f88971c1c1b51656126bd30. Modified ${date} by CopperRoute: tracks and vias replaced; zone fill caches cleared. Refill zones and run KiCad DRC. License: https://github.com/Hanqaqa/Easyduino/blob/53b14b66d64f25c55f88971c1c1b51656126bd30/License.txt`;
  const comments =
    block?.values.filter((n) => n?.values?.[0] === "comment") ?? [];
  const slot = Array.from({ length: 9 }, (_, i) => 9 - i).find(
    (i) => !comments.some((c) => Number(c.values[1]) === i),
  );
  if (slot === undefined)
    throw Error(
      "No free title-block comment for the example modification notice.",
    );
  const comment = `(comment ${slot} ${JSON.stringify(note)})`;
  if (block)
    return (
      pcb.slice(0, block.end - 1) +
      "\n  " +
      comment +
      "\n" +
      pcb.slice(block.end - 1)
    );
  return (
    pcb.slice(0, root.end - 1) +
    "\n  (title_block " +
    comment +
    ")\n" +
    pcb.slice(root.end - 1)
  );
}
export function svgNotice(
  svg,
  example,
  date = new Date().toISOString().slice(0, 10),
) {
  if (!EXAMPLES[example]) return svg;
  return svg.replace(
    ">",
    `><desc>Easyduino by Hanqaqa and contributors. CERN-OHL-P-2.0. Modified ${date}: routed and rendered by CopperRoute. Source: https://github.com/Hanqaqa/Easyduino ; license: https://github.com/Hanqaqa/Easyduino/blob/53b14b66d64f25c55f88971c1c1b51656126bd30/License.txt</desc>`,
  );
}
