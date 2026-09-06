import { outlinePaths, OUTLINE_TOLERANCE } from "./geometry.js";
// Native KiCad adapter. Source spans let us replace routing while preserving
// every other byte (footprints, graphics, properties, and unknown metadata).
export function parse(text) {
  let i = 0;
  function space() {
    while (/\s/.test(text[i] ?? "") && i < text.length) i++;
  }
  function node(depth = 0) {
    space();
    const start = i;
    if (depth > 100) throw Error("File nesting is too deep.");
    if (text[i++] !== "(") throw Error("Expected a KiCad S-expression.");
    const values = [];
    while (true) {
      space();
      if (i >= text.length) throw Error("Unclosed expression.");
      if (text[i] === ")") {
        i++;
        return { values, start, end: i };
      }
      if (text[i] === "(") {
        values.push(node(depth + 1));
        continue;
      }
      if (text[i] === '"') {
        i++;
        let value = "",
          closed = false;
        while (i < text.length) {
          const c = text[i++];
          if (c === '"') {
            closed = true;
            break;
          }
          if (c === "\\") {
            if (i >= text.length) break;
            value += text[i++];
          } else value += c;
        }
        if (!closed) throw Error("Unclosed string.");
        values.push(value);
      } else {
        const begin = i;
        while (i < text.length && !/[\s()]/.test(text[i])) i++;
        values.push(text.slice(begin, i));
      }
    }
  }
  const root = node();
  space();
  if (i !== text.length || root.values[0] !== "kicad_pcb")
    throw Error("Expected one kicad_pcb board.");
  return root;
}
const children = (n, key) => n.values.filter((v) => v?.values?.[0] === key);
const child = (n, key) => children(n, key)[0];
const val = (n, key, fallback) => child(n, key)?.values[1] ?? fallback;
const number = (v) => {
  const n = Number(v);
  if (!Number.isFinite(n) || Math.abs(n) > 100000)
    throw Error("Invalid or excessive board coordinate.");
  return n;
};
const xy = (n) => {
  if (!n) throw Error("Missing coordinate.");
  return { x: number(n.values[1]), y: number(n.values[2]) };
};
const point = (n, key) => xy(child(n, key));
const equal = (a, b) => Math.hypot(a.x - b.x, a.y - b.y) < 0.00001;

export function importBoard(text, name, rules, options = {}) {
  const root = parse(text),
    warnings = [];
  for (const key of ["traceWidth", "clearance", "viaDiameter", "viaDrill"]) {
    if (!(rules[key] > 0 && rules[key] <= 5))
      throw Error("Invalid routing rules.");
  }
  if (rules.viaDrill >= rules.viaDiameter)
    throw Error("Via drill must be smaller than diameter.");
  if (
    (!options.rebuildZones &&
      children(root, "zone").some((zone) => !child(zone, "keepout"))) ||
    (!options.ripUpRouting && children(root, "arc").length)
  )
    throw Error(
      "This prototype does not yet support copper zones, keepouts, or curved tracks.",
    );
  if (children(root, "net_class").length)
    throw Error("Legacy embedded net classes are not supported yet.");
  const layers = child(root, "layers")
    ?.values.filter((v) => v?.values && /\.Cu$/.test(v.values[1]))
    .map((v, index) => {
      const kind = v.values[2];
      if (!["signal", "mixed", "power"].includes(kind))
        throw Error(`Unsupported copper layer type: ${kind}`);
      return { index, name: v.values[1], type: kind === "power" ? "plane" : "signal" };
    });
  if (!layers?.length || layers.length > 32)
    throw Error("Expected 1–32 copper layers.");
  const layerIndex = (name) => {
    const layer = layers.find((l) => l.name === name);
    if (!layer) throw Error(`Unknown copper layer: ${name}`);
    return layer.index;
  };
  const nets = children(root, "net").map((n) => ({
    id: number(n.values[1]),
    name: n.values[2],
    className: "Default",
  }));
  const namedNets = Number(val(root, "version", 0)) >= 20260101;
  const netName = (n) => {
    if (namedNets) {
      const name = val(n, "net", "");
      if (name && !nets.some((net) => net.name === name))
        nets.push({ id: nets.length + 1, name, className: "Default" });
      return name;
    }
    const id = number(val(n, "net", 0));
    if (!id) return "";
    const net = nets.find((n) => n.id === id);
    if (!net) throw Error(`Unknown net ${id}`);
    return net.name;
  };
  const zones = children(root, "zone");
  let copperZones = 0,
    preservedKeepouts = 0;
  for (const zone of zones) {
    const keepout = child(zone, "keepout");
    if (keepout) {
      // These rule areas do not constrain track/via routing. Preserve the source
      // so KiCad applies their pour/placement restrictions during refill.
      if (
        val(keepout, "tracks") !== "allowed" ||
        val(keepout, "vias") !== "allowed"
      )
        throw Error(
          "Zone keepouts that restrict tracks or vias are not supported for routing yet.",
        );
      preservedKeepouts++;
      continue;
    }
    if (!netName(zone))
      throw Error("Netless copper zones are not supported for routing yet.");
    copperZones++;
  }
  if (copperZones)
    warnings.push(
      `${copperZones} copper zones will be preserved with their fill cache removed. Routing uses tracks only; refill zones in KiCad (B), then run DRC.`,
    );
  if (preservedKeepouts)
    warnings.push(
      `${preservedKeepouts} keepout areas allow tracks and vias and are preserved for KiCad zone refill.`,
    );
  const outline = outlinePaths(root);
  const edges = outline.paths.flatMap((path) =>
    path.slice(1).map((p, i) => [path[i], p]),
  );
  for (const n of root.values.filter((v) => v?.values)) {
    const layer = val(n, "layer", "");
    if (
      layer.endsWith(".Cu") &&
      ![
        "segment",
        "footprint",
        "module",
        "zone",
        ...(options.ripUpRouting ? ["arc"] : []),
      ].includes(n.values[0])
    )
      throw Error(`Unsupported copper object: ${n.values[0]}`);
  }
  if (outline.curved)
    warnings.push(
      "Curved board edges are approximated within 0.005 mm for routing; the original outline is preserved in downloads.",
    );
  if (!edges.length) throw Error("A closed Edge.Cuts outline is required.");
  const [first, last] = edges.shift(),
    corners = [first];
  let end = last;
  while (!equal(end, first)) {
    corners.push(end);
    const i = edges.findIndex(([a, b]) => equal(a, end) || equal(b, end));
    if (i < 0) throw Error("The Edge.Cuts outline is not closed.");
    const [a, b] = edges.splice(i, 1)[0];
    end = equal(a, end) ? b : a;
  }
  if (edges.length || corners.length < 3)
    throw Error("Multiple outlines or cutouts are not supported yet.");
  const components = [];
  for (const [fi, fp] of [
    ...children(root, "footprint"),
    ...children(root, "module"),
  ].entries()) {
    if (children(fp, "net_tie_pad_groups").length)
      throw Error("Net ties are not supported yet.");
    if (children(fp, "zone").length)
      throw Error("Footprint zones are not supported yet.");
    for (const n of fp.values.filter((v) => v?.values)) {
      const layer = val(n, "layer", "");
      if (
        layer.endsWith(".Cu") &&
        n.values[0] !== "pad" &&
        n.values[0] !== "layer"
      )
        throw Error("Footprint copper graphics are not supported yet.");
    }
    const origin = point(fp, "at"),
      rotation = (number(child(fp, "at").values[3] ?? 0) * Math.PI) / 180;
    const reference =
      children(fp, "property").find((n) => n.values[1] === "Reference")
        ?.values[2] ??
      children(fp, "fp_text").find((n) => n.values[1] === "reference")
        ?.values[2] ??
      `FP${fi}`;
    for (const [pi, pad] of children(fp, "pad").entries()) {
      const shape = pad.values[3],
        type = pad.values[2];
      if (
        !["smd", "connect", "thru_hole", "np_thru_hole"].includes(type) ||
        !["circle", "rect", "oval", "roundrect"].includes(shape)
      )
        throw Error(
          `Unsupported pad ${reference}.${pad.values[1]}: ${type}/${shape}`,
        );
      if (
        children(pad, "primitives").length ||
        child(child(pad, "drill") ?? { values: [] }, "offset")
      )
        throw Error("Custom pads and offset drills are not supported yet.");
      const drillNode = child(pad, "drill"),
        slotted = drillNode?.values[1] === "oval";
      const drill = slotted
        ? Math.min(number(drillNode.values[2]), number(drillNode.values[3]))
        : number(val(pad, "drill", 0));
      if (slotted) {
        if (
          type !== "thru_hole" ||
          number(drillNode.values[2]) <= 0 ||
          number(drillNode.values[3]) <= 0
        )
          throw Error(
            "Only plated slots with positive dimensions are supported.",
          );
        warnings.push(
          "Plated slots retain their copper pad geometry; slot-specific drill checks require KiCad DRC. Original slots are preserved in downloads.",
        );
      }
      const local = point(pad, "at"),
        size = point(pad, "size");
      if (shape === "circle" && size.x !== size.y)
        throw Error("Circular pads must have equal dimensions.");
      if (size.x <= 0 || size.y <= 0)
        throw Error("Pad sizes must be positive.");
      const position = {
        x:
          origin.x +
          local.x * Math.cos(rotation) +
          local.y * Math.sin(rotation),
        y:
          origin.y -
          local.x * Math.sin(rotation) +
          local.y * Math.cos(rotation),
      };
      const angle = number(child(pad, "at").values[3] ?? 0);
      const padLayers =
        child(pad, "layers")
          ?.values.slice(1)
          .flatMap((l) =>
            l === "*.Cu" || l === "F&B.Cu"
              ? layers.map((l) => l.name)
              : l.endsWith(".Cu")
                ? [l]
                : [],
          ) ?? [];
      padLayers.forEach(layerIndex);
      if (!padLayers.length) continue; // Paste-only apertures are not copper obstacles.
      // The JSON reader has component rotation but no individual pad rotation.
      // Give each pad a component with its absolute placement and angle.
      components.push({
        reference: `${reference}:${fi}:${pi}`,
        position,
        rotation: -angle,
        layer: "F.Cu",
        pads: [
          {
            name: String(pi),
            netName: netName(pad),
            shape,
            ...(shape === "roundrect" ? { roundRectRatio: number(val(pad, "roundrect_rratio", 0.25)) } : {}),
            size,
            offset: { x: 0, y: 0 },
            position,
            drill,
            nonPlated: type === "np_thru_hole",
            drillEstimated: slotted,
            layers: padLayers,
          },
        ],
      });
      if (shape === "roundrect")
        warnings.push(
          "Rounded pads retain their corner radius for hole DRC; routing uses enclosing rectangles.",
        );
    }
  }
  if (!components.length) throw Error("No pads were found.");
  const traces = (options.ripUpRouting ? [] : children(root, "segment")).map(
    (n, id) => {
      if (n.values.includes("locked") || child(n, "locked"))
        throw Error("Locked tracks are not supported yet.");
      if (!netName(n))
        throw Error("Tracks without a net are not supported yet.");
      return {
        id,
        netName: netName(n),
        width: number(val(n, "width")),
        layerIndex: layerIndex(val(n, "layer")),
        points: [point(n, "start"), point(n, "end")],
      };
    },
  );
  const vias = (options.ripUpRouting ? [] : children(root, "via")).map(
    (n, id) => {
      if (
        n.values.includes("locked") ||
        child(n, "locked") ||
        n.values.includes("blind") ||
        n.values.includes("micro")
      )
        throw Error("Locked, blind, and micro vias are not supported yet.");
      const span = child(n, "layers")?.values.slice(1);
      if (span?.length !== 2) throw Error("Missing via layer span.");
      if (
        !netName(n) ||
        span[0] !== layers[0].name ||
        span[1] !== layers.at(-1).name
      )
        throw Error("Only through vias assigned to a net are supported.");
      return {
        id,
        netName: netName(n),
        position: point(n, "at"),
        diameter: number(val(n, "size")),
        drill: number(val(n, "drill")),
        startLayerIndex: layerIndex(span[0]),
        endLayerIndex: layerIndex(span[1]),
      };
    },
  );
  return {
    text,
    root,
    namedNets,
    ripUpRouting: !!options.ripUpRouting,
    rebuildZones: !!options.rebuildZones,
    warnings: [...new Set(warnings)],
    board: {
      viaInPadAllowed: !!options.allowViaInPad,
      designName: name,
      unit: "MM",
      resolution: 10000,
      layers,
      nets: nets.filter((n) => n.id > 0),
      netClasses: [
        {
          name: "Default",
          ...rules,
          netNames: nets.filter((n) => n.id > 0).map((n) => n.name),
        },
      ],
      components,
      outline: {
        corners,
        clearance: rules.clearance + (outline.curved ? OUTLINE_TOLERANCE : 0),
      },
      traces,
      vias,
      conductionAreas: [],
    },
  };
}

export function exportBoard(input, routed) {
  const q = (s) => JSON.stringify(s);
  const net = (name) => {
    if (input.namedNets) return q(name || "");
    if (!name) return 0;
    const n = input.board.nets.find((n) => n.name === name);
    if (!n) throw Error(`Unknown output net: ${name}`);
    return n.id;
  };
  const layer = (i) => {
    const l = input.board.layers.find((l) => l.index === i);
    const output = routed.layers.find((l) => l.index === i);
    if (!l || !output || output.name !== l.name)
      throw Error("Router output changed the copper layer mapping.");
    return q(l.name);
  };
  const f = (n) => {
    if (!Number.isFinite(n)) throw Error("Invalid routed coordinate.");
    return Number(n.toFixed(6));
  };
  const added = [];
  for (const t of routed.traces) {
    if (input.routingLayers && !input.routingLayers.includes(input.board.layers.find(l => l.index === t.layerIndex)?.name))
      throw Error("Router output contains a trace outside the selected routing layers.");
    if (input.board.layers.find((l) => l.index === t.layerIndex)?.type !== "signal")
      throw Error("Router output contains a trace on a non-routing layer.");
    for (let i = 1; i < t.points.length; i++) {
      const a = t.points[i - 1],
        b = t.points[i];
      added.push(
        `  (segment (start ${f(a.x)} ${f(a.y)}) (end ${f(b.x)} ${f(b.y)}) (width ${f(t.width)}) (layer ${layer(t.layerIndex)}) (net ${net(t.netName)}))`,
      );
    }
  }
  for (const v of routed.vias)
    added.push(
      `  (via (at ${f(v.position.x)} ${f(v.position.y)}) (size ${f(v.diameter)}) (drill ${f(v.drill)}) (layers ${layer(v.startLayerIndex)} ${layer(v.endLayerIndex)}) (net ${net(v.netName)}))`,
    );
  const removed = [
    ...children(input.root, "segment"),
    ...children(input.root, "via"),
    ...(input.ripUpRouting ? children(input.root, "arc") : []),
    ...(input.rebuildZones
      ? children(input.root, "zone").flatMap((zone) => [
          ...children(zone, "filled_polygon"),
          ...children(zone, "fill_segments"),
        ])
      : []),
  ].sort((a, b) => a.start - b.start);
  let text = "",
    cursor = 0;
  for (const n of removed) {
    text += input.text.slice(cursor, n.start);
    cursor = n.end;
  }
  return (
    text +
    input.text.slice(cursor, input.root.end - 1) +
    "\n" +
    added.join("\n") +
    "\n" +
    input.text.slice(input.root.end - 1)
  );
}

export function renderSvg(input, routed = input.board) {
  const pts = input.board.outline.corners;
  const xs = pts.map((p) => p.x),
    ys = pts.map((p) => p.y),
    x = Math.min(...xs) - 2,
    y = Math.min(...ys) - 2,
    w = Math.max(...xs) - x + 2,
    h = Math.max(...ys) - y + 2;
  const colors = ["#f26d78", "#68b8ff", "#ce9fff", "#71dbc1"];
  const path = (p) => p.map((p) => `${p.x},${p.y}`).join(" ");
  let svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${x} ${y} ${w} ${h}" role="img" aria-label="PCB copper routing preview"><rect x="${x}" y="${y}" width="${w}" height="${h}" fill="#101c25"/><polygon points="${path(pts)}" fill="#183b37" stroke="#83c5af" stroke-width="0.15"/>`;
  for (const t of routed.traces)
    svg += `<polyline points="${path(t.points)}" fill="none" stroke="${colors[t.layerIndex % colors.length]}" stroke-width="${t.width}" stroke-linecap="round" stroke-linejoin="round"/>`;
  for (const c of input.board.components)
    for (const p of c.pads) {
      const { x, y } = c.position,
        rx = p.size.x / 2,
        ry = p.size.y / 2;
      svg += `<g transform="translate(${x} ${y}) rotate(${c.rotation})" fill="#edcf86">`;
      svg +=
        p.shape === "circle"
          ? `<ellipse rx="${rx}" ry="${ry}"/>`
          : `<rect x="${-rx}" y="${-ry}" width="${rx * 2}" height="${ry * 2}" rx="${p.shape === "oval" ? Math.min(rx, ry) : 0}"/>`;
      if (p.drill) svg += `<circle r="${p.drill / 2}" fill="#101c25"/>`;
      svg += "</g>";
    }
  for (const v of routed.vias)
    svg += `<circle cx="${v.position.x}" cy="${v.position.y}" r="${v.diameter / 2}" fill="#edcf86"/><circle cx="${v.position.x}" cy="${v.position.y}" r="${v.drill / 2}" fill="#101c25"/>`;
  return svg + "</svg>";
}
