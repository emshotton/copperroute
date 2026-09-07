import { EXAMPLES } from "./examples.js";
const $ = (id) => document.getElementById(id);
let selectionRevision = 0,
  currentExample = null;
let manualRules = Object.fromEntries(
  ["traceWidth", "clearance", "viaDiameter", "viaDrill"].map((id) => [
    id,
    document.getElementById(id).value,
  ]),
);
let selected,
  embeddedRules,
  projectFile,
  previewWorker,
  worker,
  urls = [];
const status = (text) => {
  $("status").textContent = text;
};
function showLayers(layers) {
  if (!layers) return;
  const legend = $("layer-legend");
  legend.replaceChildren();
  for (const layer of layers) {
    const item = document.createElement("span"), dot = document.createElement("i");
    dot.style.backgroundColor = layer.color;
    item.append(dot, document.createTextNode(` ${layer.label}${layer.label !== layer.name ? ` (${layer.name})` : ""}${layer.type === "power" ? " · plane" : ""}`));
    legend.append(item);
  }
  const pads = document.createElement("span"), dot = document.createElement("i");
  dot.className = "pads";
  pads.append(dot, document.createTextNode(" Pads & vias"));
  legend.append(pads);
}
function showDrc(data) {
  const panel = $("drc-results");
  const list = $("drc-list");
  list.replaceChildren();
  panel.hidden = !data.drcDetails?.length;
  if (panel.hidden) return "";
  const key = (v) => JSON.stringify([v.kind, v.layer, v.items.map(i => i.id).sort()]);
  const initial = new Set((data.initialDrcDetails ?? []).map(key));
  let existing = 0;
  for (const v of data.drcDetails) {
    const preexisting = initial.has(key(v));
    if (preexisting) existing++;
    const row = document.createElement("li");
    const names = v.items.map(i => i.description).join(" ↔ ");
    row.textContent = `${preexisting ? "Existing" : "New"} · ${v.kind.replaceAll("_", " ")} · ${names} · ${v.layer ?? "all layers"} · ${v.actual.toFixed(4)} mm (required ${v.expected.toFixed(4)} mm) · at ${v.position.map(n => n.toFixed(3)).join(", ")} mm${v.estimated ? " · estimated drill" : ""}`;
    list.append(row);
  }
  const text = `${existing} existing before routing · ${data.drcDetails.length - existing} new`;
  $("drc-summary").textContent = `DRC details: ${text}`;
  return ` (${text})`;
}
function clearDownloads() {
  $("drc-results").hidden = true;
  $("drc-list").replaceChildren();
  urls.forEach(URL.revokeObjectURL);
  urls = [];
  for (const id of ["pcb", "ses", "svg"]) {
    $(id).hidden = true;
    $(id).removeAttribute("href");
  }
}
function finish() {
  worker?.terminate();
  worker = null;
  $("cancel").hidden = true;
  $("route").disabled = !selected;
}
function isDsn() { return /\.dsn$/i.test(selected?.name ?? ""); }
function updateRuleControls() {
  const embedded = isDsn();
  for (const id of Object.keys(manualRules)) $(id).disabled = embedded || !!projectFile || !!embeddedRules;
  for (const id of ["project", "clear-project", "rebuild-zones", "allow-via-in-pad"]) $(id).disabled = embedded;
  $("project-name").textContent = embedded
    ? "Using embedded DSN net classes, routing rules and layer settings."
    : projectFile ? projectFile.name : embeddedRules
      ? "Using embedded KiCad net classes. Default class values are shown below."
      : "No project selected — using the manual rules below.";
  if (!projectFile && !embedded && embeddedRules)
    for (const [id, value] of Object.entries(embeddedRules))
      if (id in manualRules) $(id).value = value;
}
function select(file) {
  selectionRevision++;
  currentExample = null;
  $("example-credit").hidden = true;
  $("example-routing").hidden = true;
  previewWorker?.terminate();
  previewWorker = null;
  finish();
  clearDownloads();
  if (!projectFile && !embeddedRules && !isDsn())
    manualRules = Object.fromEntries(Object.keys(manualRules).map((id) => [id, $(id).value]));
  if (embeddedRules && !projectFile)
    for (const [id, value] of Object.entries(manualRules)) $(id).value = value;
  embeddedRules = null;
  selected = null;
  updateRuleControls();
  $("route").disabled = true;
  $("warnings").textContent = "";
  $("preview").replaceChildren();
  $("filename").textContent = "BOARD PREVIEW";
  if (!/\.(kicad_pcb|dsn)$/i.test(file?.name ?? "")) {
    status("Choose a .kicad_pcb or .dsn file.");
    return;
  }
  if (file.size > 20 * 1024 * 1024) {
    status("Files are limited to 20 MB.");
    return;
  }
  selected = file;
  $("allow-via-in-pad").checked = false;
  updateRuleControls();
  $("filename").textContent = file.name;
  $("badge").textContent = "Board selected";
  $("route").disabled = false;
  status("Loading board preview…");
  const active = new Worker("./worker.js", { type: "module" });
  previewWorker = active;
  active.onmessage = ({ data }) => {
    if (previewWorker !== active || selected !== file) return;
    if (data.type === "preview") {
      $("preview").innerHTML = data.svg;
      showLayers(data.layers);
      $("warnings").textContent = data.warnings.join(" ");
    }
    if (data.type === "ready" && data.embeddedRules) {
      embeddedRules = data.embeddedRules;
      updateRuleControls();
    }
    if (data.type === "ready" || data.type === "error") {
      active.terminate();
      previewWorker = null;
      if (!worker)
        status(
          data.type === "error"
            ? data.text
            : "Board selected. Check the routing settings, then route.",
        );
    }
  };
  active.onerror = (e) => {
    if (previewWorker === active) {
      active.terminate();
      previewWorker = null;
      if (!worker) status(e.message);
    }
  };
  file
    .text()
    .then((text) => {
      if (previewWorker === active)
        active.postMessage({ action: "preview", text, name: file.name });
    })
    .catch((e) => {
      if (previewWorker === active) {
        active.terminate();
        previewWorker = null;
        status(e.message);
      }
    });
}
function selectProject(file) {
  selectionRevision++;
  if (worker) finish();
  clearDownloads();
  if (file && file.size > 5 * 1024 * 1024) {
    status("Project files are limited to 5 MB.");
    return;
  }
  if (file && !projectFile && !embeddedRules)
    manualRules = Object.fromEntries(
      Object.keys(manualRules).map((id) => [id, $(id).value]),
    );
  if (!file)
    for (const [id, value] of Object.entries(manualRules)) $(id).value = value;
  projectFile = file;
  updateRuleControls();
}
function selectFiles(files) {
  const list = Array.from(files),
    pcb = list.find((f) => /\.(kicad_pcb|dsn)$/i.test(f.name)),
    project = list.find((f) => f.name.toLowerCase().endsWith(".kicad_pro"));
  if (pcb) {
    selectProject(project);
    select(pcb);
  } else if (project) selectProject(project);
  else select(list[0]);
}
$("file").onchange = () => selectFiles($("file").files);
$("project").onchange = () => selectProject($("project").files[0]);
$("clear-project").onclick = () => {
  selectProject(null);
  $("project").value = "";
};
$("choose-board").onclick = () => $("file").click();
$("drop").onclick = () => $("file").click();
$("drop").onkeydown = (e) => {
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    $("file").click();
  }
};
window.addEventListener("dragover", (e) => e.preventDefault());
window.addEventListener("drop", (e) => e.preventDefault());
$("drop").ondragover = (e) => {
  e.preventDefault();
  $("drop").classList.add("drag");
};
$("drop").ondragleave = (e) => {
  if (!$("drop").contains(e.relatedTarget)) $("drop").classList.remove("drag");
};
$("drop").ondrop = (e) => {
  e.preventDefault();
  $("drop").classList.remove("drag");
  selectFiles(e.dataTransfer.files);
};
function beginExampleLoad() {
  const revision = ++selectionRevision;
  selected = null;
  finish();
  previewWorker?.terminate();
  previewWorker = null;
  clearDownloads();
  $("preview").replaceChildren();
  $("filename").textContent = "BOARD PREVIEW";
  $("example-credit").hidden = true;
  $("example-routing").hidden = true;
  $("warnings").textContent = "";
  return revision;
}
$("demo").onclick = async () => {
  const revision = beginExampleLoad();
  status("Loading small example…");
  try {
    const response = await fetch("./example.kicad_pcb");
    if (!response.ok) throw Error("Could not load example.");
    const text = await response.text();
    if (revision !== selectionRevision) return;
    selectProject(null);
    select(new File([text], "example.kicad_pcb"));
  } catch (e) {
    if (revision === selectionRevision) status(e.message);
  }
};
$("cancel").onclick = () => {
  finish();
  $("badge").textContent = "Cancelled";
  status("Routing cancelled. You can change the settings and try again.");
};
$("settings").onsubmit = async (e) => {
  e.preventDefault();
  if (!selected || worker) return;
  const rules = Object.fromEntries(
    ["traceWidth", "clearance", "viaDiameter", "viaDrill"].map((id) => [
      id,
      Number($(id).value),
    ]),
  );
  if (!isDsn() && rules.viaDrill >= rules.viaDiameter) {
    status("Via drill must be smaller than the via diameter.");
    return;
  }
  previewWorker?.terminate();
  previewWorker = null;
  clearDownloads();
  $("warnings").textContent = "";
  $("route").disabled = true;
  $("cancel").hidden = false;
  $("badge").textContent = "Working";
  const file = selected,
    active = new Worker("./worker.js", { type: "module" });
  worker = active;
  const failed = (text) => {
    if (worker !== active) return;
    finish();
    $("badge").textContent = "Could not route";
    status(text);
  };
  active.onerror = (e) =>
    failed(e.message || "The routing worker stopped unexpectedly.");
  active.onmessage = ({ data }) => {
    if (worker !== active) return;
    if (data.type === "status") status(data.text);
    if (data.type === "preview" || data.type === "result") {
      showLayers(data.layers);
      // Only SVG produced by our numeric renderer enters the DOM.
      $("preview").innerHTML = data.svg;
      $("warnings").textContent = data.warnings.join(" ");
    }
    if (data.type === "progress") {
      $("preview").innerHTML = data.svg;
      $("badge").textContent = `Live · Pass ${data.pass ?? 1}`;
      status(
        `Pass ${data.pass ?? 1} · ${data.incomplete ?? "Unknown"} connections remaining · ${data.routed ?? 0} routed this pass`,
      );
    }
    if (data.type === "warnings")
      $("warnings").textContent = data.warnings.join(" ");
    if (data.type === "error") failed(data.text);
    if (data.type === "result") {
      finish();
      $("badge").textContent = data.timedOut
        ? "Time limit reached"
        : data.incomplete === 0
          ? "Routing complete"
          : "Partial route";
      const drcSummary = showDrc(data);
      status(
        `${data.incomplete ?? "Unknown"} unrouted connections · ${data.violations} router DRC violations${drcSummary} · ${data.passes} passes${data.timedOut ? " · Time limit reached" : ""}. Review in ${data.dsn ? "the source PCB editor" : "KiCad"} and run DRC.`,
      );
      for (const [id, content, mime, ext] of [
        ["pcb", data.dsn ?? data.pcb, "text/plain", data.dsn ? ".dsn" : ".kicad_pcb"],
        ["ses", data.ses, "text/plain", ".ses"],
        ["svg", data.svg, "image/svg+xml", ".svg"],
      ]) {
        if (content == null) continue;
        if (id === "pcb") $(id).textContent = data.dsn ? "Download routed DSN ↓" : "Download routed PCB ↓";
        const url = URL.createObjectURL(new Blob([content], { type: mime }));
        urls.push(url);
        $(id).href = url;
        $(id).download = file.name.replace(/\.(kicad_pcb|dsn)$/i, "-routed") + ext;
        $(id).hidden = false;
      }
    }
  };
  try {
    const [text, project] = await Promise.all([
      file.text(),
      projectFile?.text() ?? Promise.resolve(""),
    ]);
    if (worker === active)
      active.postMessage({
        text,
        name: file.name,
        rules,
        project,
        example: currentExample,
        rebuildZones: $("rebuild-zones").checked,
        allowViaInPad: $("allow-via-in-pad").checked,
        passes: Number($("passes").value),
        seconds: Number($("seconds").value),
      });
  } catch (e) {
    failed(e.message);
  }
};

async function loadExample(id) {
  const example = EXAMPLES[id];
  if (!example) return;
  const revision = beginExampleLoad();
  status(`Loading ${example.name} and its project rules…`);
  try {
    const [pcb, project] = await Promise.all(
      [".kicad_pcb", ".kicad_pro"].map(async (ext) => {
        const response = await fetch(example.base + ext);
        if (!response.ok) throw Error(`Could not load ${example.name}.`);
        return response.text();
      }),
    );
    // User selections and newer example requests win over a slow default fetch.
    if (revision !== selectionRevision) return;
    selectProject(new File([project], example.projectName));
    select(new File([pcb], example.pcbName));
    currentExample = id;
    $("examples").value = id;
    $("example-credit").hidden = false;
    $("example-routing").hidden = !example.routingLayers;
    $("example-routing").textContent = example.routingLayers
      ? "Routes on front and back copper only (F.Cu / B.Cu). The four-layer board stack is preserved."
      : "";
    const cls = JSON.parse(project).net_settings.classes.find(
      (c) => c.name === "Default",
    );
    for (const [field, key] of [
      ["traceWidth", "track_width"],
      ["clearance", "clearance"],
      ["viaDiameter", "via_diameter"],
      ["viaDrill", "via_drill"],
    ])
      $(field).value = cls[key];
  } catch (error) {
    if (revision === selectionRevision) status(error.message);
  }
}
$("examples").onchange = () => loadExample($("examples").value);
$("reload-example").onclick = () => loadExample($("examples").value);
loadExample("uno");
