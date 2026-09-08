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
function showMetrics(text) {
  $("metrics").textContent = text ?? "";
  $("metrics").hidden = !text;
}
const currentRules = () =>
  Object.fromEntries(
    ["traceWidth", "clearance", "viaDiameter", "viaDrill"].map((id) => [
      id,
      Number($(id).value),
    ]),
  );
function netCheckboxes() {
  return [...$("net-list").querySelectorAll("input")];
}
function netSelection() {
  const boxes = netCheckboxes();
  return boxes.length && boxes.some((box) => !box.checked)
    ? boxes.filter((box) => box.checked).map((box) => box.value)
    : null;
}
function updateNetSummary() {
  const boxes = netCheckboxes(),
    chosen = boxes.filter((box) => box.checked).length;
  $("route").disabled = !selected || (!!boxes.length && chosen === 0);
  if (!boxes.length) return;
  $("net-summary").textContent =
    chosen === boxes.length
      ? `Nets to route — all ${boxes.length} selected`
      : `Nets to route — ${chosen} of ${boxes.length} selected`;
}
function showNets(nets) {
  const list = $("net-list");
  list.replaceChildren();
  $("net-filter").value = "";
  $("net-panel").hidden = !nets?.length;
  $("net-panel").open = false;
  if (!nets?.length) return;
  for (const net of nets) {
    const item = document.createElement("li"),
      label = document.createElement("label"),
      box = document.createElement("input"),
      name = document.createElement("span"),
      className = document.createElement("em");
    box.type = "checkbox";
    box.checked = true;
    box.value = net.name;
    name.textContent = net.name;
    className.textContent = net.className;
    label.append(box, name, className);
    item.append(label);
    list.append(item);
  }
  updateNetSummary();
}
function showCredit(credit) {
  const box = $("example-credit");
  box.replaceChildren();
  box.hidden = !credit;
  if (!credit) return;
  const link = (href, text, download) => {
    const a = document.createElement("a");
    a.href = href;
    a.textContent = text;
    if (download) a.download = "";
    return a;
  };
  box.append(
    document.createTextNode(`${credit.project} by ${credit.author} · `),
    link(credit.source, "Source"),
    document.createTextNode(" · "),
    link(credit.licenseFile, `${credit.license} license`, true),
    document.createTextNode(" · "),
    link(credit.noticeFile, "Notices"),
  );
}
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
  const unrouted = document.createElement("label"),
    box = document.createElement("input"),
    dash = document.createElement("i");
  box.type = "checkbox";
  box.id = "show-ratsnest";
  box.checked = !$("preview").classList.contains("no-ratsnest");
  box.onchange = () =>
    $("preview").classList.toggle("no-ratsnest", !box.checked);
  unrouted.append(box, dash, document.createTextNode(" Unrouted"));
  legend.append(unrouted);
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
  updateNetSummary();
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
  $("rules-summary").textContent = `Board rules — ${
    embedded
      ? "embedded DSN"
      : projectFile
        ? projectFile.name
        : embeddedRules
          ? "embedded KiCad classes"
          : "manual"
  }`;
  if (!projectFile && !embedded && embeddedRules)
    for (const [id, value] of Object.entries(embeddedRules))
      if (id in manualRules) $(id).value = value;
}
function select(file) {
  selectionRevision++;
  currentExample = null;
  showCredit(null);
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
  showNets(null);
  showMetrics(null);
  updateRuleControls();
  $("route").disabled = true;
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
    }
    if (data.type === "ready" && data.embeddedRules) {
      embeddedRules = data.embeddedRules;
      updateRuleControls();
    }
    if (data.type === "ready") showNets(data.nets);
    if (data.type === "done" || data.type === "error") {
      active.terminate();
      previewWorker = null;
      if (!worker) status(data.type === "error" ? data.text : "");
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
        active.postMessage({
          action: "preview",
          text,
          name: file.name,
          rules: currentRules(),
        });
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
  showCredit(null);
  $("example-routing").hidden = true;
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
  const rules = currentRules();
  if (!isDsn() && rules.viaDrill >= rules.viaDiameter) {
    status("Via drill must be smaller than the via diameter.");
    return;
  }
  previewWorker?.terminate();
  previewWorker = null;
  clearDownloads();
  $("route").disabled = true;
  $("cancel").hidden = false;
  $("badge").textContent = "Working";
  showMetrics(null);
  const file = selected,
    active = new Worker("./worker.js", { type: "module" });
  worker = active;
  const failed = (text) => {
    if (worker !== active) return;
    finish();
    $("badge").textContent = "Could not route";
    showMetrics(null);
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
    }
    if (data.type === "progress") {
      $("preview").innerHTML = data.svg;
      $("badge").textContent = `Live · Pass ${data.pass ?? 1}`;
      status(
        `Pass ${data.pass ?? 1} · ${data.incomplete ?? "Unknown"} connections remaining · ${data.routed ?? 0} routed this pass`,
      );
    }
    if (data.type === "error") failed(data.text);
    if (data.type === "result") {
      finish();
      $("badge").textContent = data.timedOut
        ? "Time limit reached"
        : data.incomplete === 0
          ? "Routing complete"
          : "Partial route";
      const drcSummary = showDrc(data);
      showMetrics(
        `${data.incomplete ?? "Unknown"} unrouted connections${data.incompleteOnUnselectedNets ? ` (${data.incompleteOnUnselectedNets} on nets you did not select)` : ""} · ${data.violations} router DRC violations${drcSummary}`,
      );
      status(
        `${data.passes} passes${data.timedOut ? " · Time limit reached" : ""}. Review in ${data.dsn ? "the source PCB editor" : "KiCad"} and run DRC.`,
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
        nets: netSelection(),
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
    const pcbResponse = await fetch(example.base + ".kicad_pcb");
    if (!pcbResponse.ok) throw Error(`Could not load ${example.name}.`);
    const pcb = await pcbResponse.text();
    let project = null;
    if (example.projectName) {
      const response = await fetch(example.base + ".kicad_pro");
      if (!response.ok) throw Error(`Could not load ${example.name}.`);
      project = await response.text();
    }
    // User selections and newer example requests win over a slow default fetch.
    if (revision !== selectionRevision) return;
    selectProject(
      project === null ? null : new File([project], example.projectName),
    );
    select(new File([pcb], example.pcbName));
    currentExample = id;
    $("examples").value = id;
    showCredit(example.credit);
    $("example-routing").hidden = !example.routingLayers;
    $("example-routing").textContent = example.routingLayers
      ? "Routes on front and back copper only (F.Cu / B.Cu). The four-layer board stack is preserved."
      : "";
    if (project !== null) {
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
    }
  } catch (error) {
    if (revision === selectionRevision) status(error.message);
  }
}
const setAllNets = (checked) => {
  for (const box of netCheckboxes())
    if (!box.closest("li").hidden) box.checked = checked;
  updateNetSummary();
};
$("nets-all").onclick = () => setAllNets(true);
$("nets-none").onclick = () => setAllNets(false);
$("net-list").onchange = updateNetSummary;
$("net-filter").oninput = () => {
  const needle = $("net-filter").value.trim().toLowerCase();
  for (const box of netCheckboxes())
    box.closest("li").hidden = !box.value.toLowerCase().includes(needle);
};
function dismissDropHint() {
  const hint = $("drop-hint");
  if (!hint || hint.hidden) return;
  hint.classList.add("leaving");
  // transitionend never fires when the fade is skipped, so time it out too.
  const hide = () => (hint.hidden = true);
  hint.addEventListener("transitionend", hide, { once: true });
  setTimeout(hide, 400);
}
for (const event of ["pointerdown", "keydown", "dragenter", "wheel"])
  addEventListener(event, dismissDropHint, { once: true, passive: true });
$("examples").onchange = () => loadExample($("examples").value);
$("reload-example").onclick = () => loadExample($("examples").value);
loadExample("uno");
