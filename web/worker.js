import { EXAMPLES, modificationNotice, svgNotice } from "./examples.js";
import { importBoard, exportBoard, netsForSelection, parse, embeddedNetClasses } from "./kicad.js";
import { previewBoard, previewLayers } from "./preview.js";
import { applyProject } from "./project.js";
self.onmessage = async ({ data }) => {
  try {
    if (/\.dsn$/i.test(data.name ?? "")) {
      const { default: init, preview_dsn, route_dsn } = await import("./pkg/copper_web.js");
      await init();
      self.postMessage({ type: "preview", ...JSON.parse(preview_dsn(data.text, data.name)) });
      if (data.action === "preview") {
        self.postMessage({ type: "ready" });
        return;
      }
      self.postMessage({ type: "status", text: "Routing from scratch with embedded DSN rules…" });
      const result = JSON.parse(route_dsn(data.text, data.name, data.passes, data.seconds,
        (json) => self.postMessage(JSON.parse(json))));
      self.postMessage({ type: "result", ...result });
      return;
    }
    self.postMessage({
      type: "preview",
      svg: previewBoard(data.text),
      layers: previewLayers(data.text),
      warnings: [],
    });
    if (data.action === "preview") {
      self.postMessage({
        type: "ready",
        embeddedRules: embeddedNetClasses(parse(data.text)).find((c) => c.name === "Default"),
        nets: netsForSelection(data.text, data.name, data.rules),
      });
      return;
    }
    self.postMessage({
      type: "status",
      text: "Importing board and project rules…",
    });
    const input = importBoard(data.text, data.name, data.rules, {
      rebuildZones: data.rebuildZones,
      ripUpRouting: true,
      ripUpNets: data.nets ? new Set(data.nets) : null,
      allowViaInPad: data.allowViaInPad,
    });
    applyProject(input, data.project);
    if (data.nets)
      input.warnings.push(
        `Routing restricted to ${data.nets.length} of ${input.board.nets.length} nets; the unselected nets keep their existing copper and stay unrouted.`,
      );
    input.routingLayers = EXAMPLES[data.example]?.routingLayers;
    if (input.routingLayers)
      input.warnings.push(`Routing restricted to ${input.routingLayers.join(" and ")}; the original layer stack is preserved.`);
    // Show the same unrouted board that the WASM pipeline will receive.
    self.postMessage({
      type: "preview",
      svg: previewBoard(exportBoard(input, input.board)),
      warnings: input.warnings,
    });
    const { default: init, route_board } = await import("./pkg/copper_web.js");
    await init();
    self.postMessage({
      type: "status",
      text: "Existing tracks and vias removed. Routing from scratch…",
    });
    const result = JSON.parse(
      route_board(
        JSON.stringify(input.board),
        data.passes,
        data.seconds,
        data.project ?? "",
        (json) => {
          const frame = JSON.parse(json);
          self.postMessage({
            type: "progress",
            svg: previewBoard(exportBoard(input, frame.board)),
            pass: frame.pass,
            incomplete: frame.incomplete,
            routed: frame.routed,
          });
        },
        input.routingLayers ? JSON.stringify(input.routingLayers) : "",
        data.nets ? JSON.stringify(data.nets) : "",
      ),
    );
    const pcb = modificationNotice(
      exportBoard(input, result.board),
      data.example,
    );
    self.postMessage({
      type: "result",
      ...result,
      pcb,
      svg: svgNotice(previewBoard(pcb), data.example),
      warnings: input.warnings,
    });
  } catch (error) {
    self.postMessage({ type: "error", text: error.message ?? String(error) });
  }
};
