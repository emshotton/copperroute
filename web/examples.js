import { parse } from "./kicad.js";
const EASYDUINO = {
  project: "Easyduino",
  author: "Hanqaqa and contributors",
  source: "https://github.com/Hanqaqa/Easyduino",
  commit: "53b14b66d64f25c55f88971c1c1b51656126bd30",
  license: "CERN-OHL-P-2.0",
  licenseUrl:
    "https://github.com/Hanqaqa/Easyduino/blob/53b14b66d64f25c55f88971c1c1b51656126bd30/License.txt",
  licenseFile: "./examples/easyduino/LICENSE.txt",
  noticeFile: "./examples/easyduino/NOTICE.txt",
};
export const EXAMPLES = {
  jacks: {
    name: "Hubble jacks (9 nets)",
    base: "./examples/hubble/jacks",
    pcbName: "hubble-jacks.kicad_pcb",
    credit: {
      project: "Hubble",
      author: "wntrblm",
      source: "https://github.com/wntrblm/Hubble",
      commit: "ca05a51e91eda40766f9106236731e7dd67fb8e8",
      license: "CERN-OHL-P-2.0",
      licenseFile: "./examples/hubble/LICENSE.txt",
      noticeFile: "./examples/hubble/NOTICE.md",
    },
  },
  vca: {
    name: "Slimline VCA (27 nets)",
    base: "./examples/cats-eurosynth/slimline-vca",
    pcbName: "cats-slimline-vca.kicad_pcb",
    credit: {
      project: "CATs-Eurosynth",
      author: "mzuelch",
      source: "https://github.com/mzuelch/CATs-Eurosynth",
      commit: "ee70506817f1",
      license: "MIT",
      licenseFile: "./examples/cats-eurosynth/LICENSE.txt",
      noticeFile: "./examples/cats-eurosynth/NOTICE.md",
    },
  },
  uno: {
    name: "Easyduino Uno (55 nets)",
    routingLayers: ["F.Cu", "B.Cu"],
    base: "./examples/easyduino/uno",
    pcbName: "easyduino-uno.kicad_pcb",
    projectName: "easyduino-uno.kicad_pro",
    credit: EASYDUINO,
  },
  nano: {
    name: "Easyduino Nano (62 nets)",
    base: "./examples/easyduino/nano",
    pcbName: "easyduino-nano.kicad_pcb",
    projectName: "easyduino-nano.kicad_pro",
    credit: EASYDUINO,
  },
  "feather-ice40": {
    name: "Feather ICE40 (80 nets)",
    base: "./examples/feather-ice40/feather-ice40",
    pcbName: "feather-ice40.kicad_pcb",
    credit: {
      project: "Feather-ICE40-PCB",
      author: "adafruit",
      source: "https://github.com/adafruit/Feather-ICE40-PCB",
      commit: "d2a4cf7ad1d9",
      license: "CC-BY-4.0",
      licenseFile: "./examples/feather-ice40/LICENSE.txt",
      noticeFile: "./examples/feather-ice40/NOTICE.md",
    },
  },
  kfchess: {
    name: "Real-time chess (225 nets)",
    base: "./examples/kfchess/kfchess",
    pcbName: "kfchess.kicad_pcb",
    projectName: "kfchess.kicad_pro",
    credit: {
      project: "real-time-chess",
      author: "misprit7",
      source: "https://github.com/misprit7/real-time-chess",
      commit: "2f62f9f60c3d",
      license: "MIT",
      licenseFile: "./examples/kfchess/LICENSE.txt",
      noticeFile: "./examples/kfchess/NOTICE.md",
    },
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
  const c = EXAMPLES[example].credit;
  const note = `${c.project} by ${c.author}; ${c.license}. Source: ${c.source} at ${c.commit}. Modified ${date} by CopperRoute: tracks and vias replaced; zone fill caches cleared. Refill zones and run KiCad DRC. License: ${c.licenseUrl ?? c.source}`;
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
  const c = EXAMPLES[example].credit;
  return svg.replace(
    ">",
    `><desc>${c.project} by ${c.author}. ${c.license}. Modified ${date}: routed and rendered by CopperRoute. Source: ${c.source} ; license: ${c.licenseUrl ?? c.source}</desc>`,
  );
}
