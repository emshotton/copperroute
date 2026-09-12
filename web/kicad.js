import {
  footprintPoint,
  outlinePaths,
  OUTLINE_TOLERANCE,
  nativeArcPoints,
  boundingBox,
} from "./geometry.js";
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
// Matches KiCad's own DEFAULT_CHAINING_EPSILON_MM (board.h): the max distance between two
// endpoints for KiCad to treat them as connected when chaining a board outline.
const CHAINING_EPSILON_MM = 0.01;
const equal = (a, b) => Math.hypot(a.x - b.x, a.y - b.y) < CHAINING_EPSILON_MM;
const strokeMargin = (n) => number(val(child(n, "stroke") ?? n, "width", 0)) / 2;
const obstacleNoun = (kind) =>
  ({ rect: "rectangles", line: "lines", circle: "circles", arc: "arcs", poly: "polygons" })[kind];
const circleBounds = (n, margin) => {
  const center = point(n, "center"),
    end = point(n, "end"),
    radius = Math.hypot(end.x - center.x, end.y - center.y);
  return boundingBox(
    [
      { x: center.x - radius, y: center.y - radius },
      { x: center.x + radius, y: center.y + radius },
    ],
    margin,
  );
};
const polyPoints = (n, emptyMessage) => {
  const pts = children(child(n, "pts") ?? { values: [] }, "xy").map(xy);
  if (!pts.length) throw Error(emptyMessage);
  return pts;
};
const textRectangle = (n, text, at, angle) => {
  const effects = child(n, "effects"),
    font = effects && child(effects, "font");
  if (!font || child(font, "face"))
    throw Error("Custom copper text fonts are not supported yet.");
  const size = point(font, "size");
  const lines = String(text).split("\n");
  const w = Math.max(...lines.map((line) => line.length)) * size.x * 1.5 + size.y;
  const h = lines.length * size.y * 2;
  const justify = child(effects, "justify")?.values ?? [];
  const left = justify.includes("left") ? -size.y / 2 : justify.includes("right") ? -w : -w / 2;
  const top = justify.includes("top") ? -size.y / 2 : justify.includes("bottom") ? -h : -h / 2;
  const mirror = justify.includes("mirror") ? -1 : 1;
  return [
    [left, top],
    [left + w, top],
    [left + w, top + h],
    [left, top + h],
  ].map(([x, y]) => ({
    x: at.x + mirror * x * Math.cos(angle) - y * Math.sin(angle),
    y: at.y + mirror * x * Math.sin(angle) + y * Math.cos(angle),
  }));
};

export function embeddedNetClasses(root) {
  return children(root, "net_class").map((node) => ({
    name: node.values[1],
    clearance: number(val(node, "clearance")),
    traceWidth: number(val(node, "trace_width")),
    viaDiameter: number(val(node, "via_dia")),
    viaDrill: number(val(node, "via_drill")),
    netNames: children(node, "add_net").map((net) => net.values[1]),
  }));
}

export function importBoard(text, name, rules, options = {}) {
  const root = parse(text),
    warnings = [];
  if (!children(root, "net_class").some(n => n.values[1] === "Default")) {
    for (const key of ["traceWidth", "clearance", "viaDiameter", "viaDrill"]) {
      if (!(rules[key] > 0 && rules[key] <= 5))
        throw Error("Invalid routing rules.");
    }
    if (rules.viaDrill >= rules.viaDiameter)
      throw Error("Via drill must be smaller than diameter.");
  }
  if (
    !options.rebuildZones &&
    children(root, "zone").some((zone) => !child(zone, "keepout"))
  )
    throw Error(
      "Copper zones and keepouts are not supported yet.",
    );
  const layers = child(root, "layers")
    ?.values.filter((v) => v?.values && (/\.Cu$/.test(v.values[1]) || ["signal", "mixed", "power"].includes(v.values[2])))
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
  const isCopperLayer = (name) => layers.some((l) => l.name === name);
  // A `.Cu`-suffixed name is copper-shaped even when undeclared, so callers using this to
  // decide whether to validate an object still reject it instead of silently skipping it.
  const couldBeCopper = (name) => isCopperLayer(name) || /\.Cu$/.test(name);
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
  const rippedUp = (n) =>
    !!options.ripUpRouting &&
    (!options.ripUpNets || options.ripUpNets.has(netName(n)));
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
  const conductionAreas = [];
  const outline = outlinePaths(root);
  const edges = outline.paths.flatMap((path) =>
    path.slice(1).map((p, i) => [path[i], p]),
  );
  for (const n of root.values.filter((v) => v?.values)) {
    const layer = val(n, "layer", "");
    if (!couldBeCopper(layer)) continue;
    const kind = n.values[0];
    if (kind === "gr_text") {
      const at = point(n, "at"),
        angle = (-number(child(n, "at").values[3] ?? 0) * Math.PI) / 180;
      const polygon = textRectangle(n, n.values[1], at, angle);
      conductionAreas.push({netName: "", layerIndex: layerIndex(layer), isObstacle: true, polygon});
      warnings.push("Copper text is preserved and reserved as conservative rectangular routing obstacles; check text clearances in KiCad.");
      continue;
    }
    if (kind === "gr_line" || kind === "gr_rect") {
      const polygon = boundingBox([point(n, "start"), point(n, "end")], strokeMargin(n));
      conductionAreas.push({netName: "", layerIndex: layerIndex(layer), isObstacle: true, polygon});
      warnings.push(`Copper ${obstacleNoun(kind === "gr_line" ? "line" : "rect")} are reserved as solid routing obstacles and preserved in downloads.`);
      continue;
    }
    if (kind === "gr_circle") {
      const polygon = circleBounds(n, strokeMargin(n));
      conductionAreas.push({netName: "", layerIndex: layerIndex(layer), isObstacle: true, polygon});
      warnings.push(`Copper ${obstacleNoun("circle")} are reserved as solid routing obstacles and preserved in downloads.`);
      continue;
    }
    if (kind === "gr_arc") {
      const polygon = boundingBox(nativeArcPoints(n), strokeMargin(n));
      conductionAreas.push({netName: "", layerIndex: layerIndex(layer), isObstacle: true, polygon});
      warnings.push(`Copper ${obstacleNoun("arc")} are reserved as solid routing obstacles and preserved in downloads.`);
      continue;
    }
    if (kind === "gr_poly") {
      const polygon = boundingBox(polyPoints(n, "Empty board polygon."), strokeMargin(n));
      conductionAreas.push({netName: "", layerIndex: layerIndex(layer), isObstacle: true, polygon});
      warnings.push(`Copper ${obstacleNoun("poly")} are reserved as solid routing obstacles and preserved in downloads.`);
      continue;
    }
    // A `generated` node is a length-tuning recipe whose `members` are segments and arcs
    // that already appear in the file, so it carries no copper of its own.
    if (!["segment", "footprint", "module", "zone", "arc", "generated"].includes(kind))
      throw Error(`Unsupported copper object: ${kind}`);
  }
  if (outline.curved)
    warnings.push(
      "Curved board edges are approximated within 0.005 mm for routing; the original outline is preserved in downloads.",
    );
  if (!edges.length) throw Error("A closed Edge.Cuts outline is required.");
  const loops = [];
  let strayEdges = 0;
  while (edges.length) {
    const [first, last] = edges.shift(), loop = [first];
    let end = last, consumed = 0, closed = true;
    while (!equal(end, first)) {
      loop.push(end);
      const i = edges.findIndex(([a, b]) => equal(a, end) || equal(b, end));
      if (i < 0) {
        closed = false;
        break;
      }
      const [a, b] = edges.splice(i, 1)[0];
      consumed++;
      end = equal(a, end) ? b : a;
    }
    if (!closed) {
      strayEdges += 1 + consumed;
      continue;
    }
    if (loop.length < 3) throw Error("Degenerate Edge.Cuts outline.");
    loops.push(loop);
  }
  if (!loops.length) throw Error("A closed Edge.Cuts outline is required.");
  if (strayEdges)
    warnings.push(`${strayEdges} Edge.Cuts edges could not be closed into a loop and were ignored.`);
  const area = ps => Math.abs(ps.reduce((sum, p, i) => {
    const q = ps[(i + 1) % ps.length];
    return sum + p.x * q.y - p.y * q.x;
  }, 0));
  loops.sort((a, b) => area(b) - area(a));
  const corners = loops.shift();
  const inside = (p, poly) => {
    let result = false;
    for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
      const a = poly[i], b = poly[j];
      if ((a.y > p.y) !== (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x) result = !result;
    }
    return result;
  };
  if (loops.some(loop => loop.some(p => !inside(p, corners))))
    throw Error("Separate board outlines are not supported yet.");
  if (loops.length) warnings.push(`${loops.length} internal cutouts are reserved on every copper layer.`);
  const components = [];
  const setup = child(root, "setup");
  const boardMaskMargin = setup ? number(val(setup, "pad_to_mask_clearance", 0)) : 0;
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
      if (!couldBeCopper(layer)) continue;
      const kind = n.values[0];
      if (kind === "fp_rect" || kind === "fp_line") {
        const polygon = boundingBox([point(n, "start"), point(n, "end")], strokeMargin(n)).map((p) => footprintPoint(fp, p));
        conductionAreas.push({netName: "", layerIndex: layerIndex(layer), isObstacle: true, polygon});
        warnings.push(`Footprint copper ${obstacleNoun(kind === "fp_rect" ? "rect" : "line")} are reserved as solid routing obstacles and preserved in downloads.`);
        continue;
      }
      if (kind === "fp_circle") {
        const polygon = circleBounds(n, strokeMargin(n)).map((p) => footprintPoint(fp, p));
        conductionAreas.push({netName: "", layerIndex: layerIndex(layer), isObstacle: true, polygon});
        warnings.push(`Footprint copper ${obstacleNoun("circle")} are reserved as solid routing obstacles and preserved in downloads.`);
        continue;
      }
      if (kind === "fp_arc") {
        const polygon = boundingBox(nativeArcPoints(n), strokeMargin(n)).map((p) => footprintPoint(fp, p));
        conductionAreas.push({netName: "", layerIndex: layerIndex(layer), isObstacle: true, polygon});
        warnings.push(`Footprint copper ${obstacleNoun("arc")} are reserved as solid routing obstacles and preserved in downloads.`);
        continue;
      }
      if (kind === "fp_poly") {
        const polygon = boundingBox(polyPoints(n, "Empty footprint polygon."), strokeMargin(n)).map((p) => footprintPoint(fp, p));
        conductionAreas.push({netName: "", layerIndex: layerIndex(layer), isObstacle: true, polygon});
        warnings.push(`Footprint copper ${obstacleNoun("poly")} are reserved as solid routing obstacles and preserved in downloads.`);
        continue;
      }
      if (kind === "fp_text") {
        const origin = footprintPoint(fp, point(n, "at")),
          angle = (-number(child(n, "at").values[3] ?? 0) * Math.PI) / 180;
        const polygon = textRectangle(n, n.values[2], origin, angle);
        conductionAreas.push({netName: "", layerIndex: layerIndex(layer), isObstacle: true, polygon});
        warnings.push("Footprint copper text is preserved and reserved as conservative rectangular routing obstacles; check text clearances in KiCad.");
        continue;
      }
      if (kind !== "pad" && kind !== "layer")
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
      const shape = pad.values[3];
      const type = pad.values[2];
      if (
        !["smd", "connect", "thru_hole", "np_thru_hole"].includes(type) ||
        !["circle", "rect", "oval", "roundrect", "custom"].includes(shape)
      )
        throw Error(
          `Unsupported pad ${reference}.${pad.values[1]}: ${type}/${shape}`,
        );
      let copperPolygon;
      if (shape === "custom") {
        const primitives = child(pad, "primitives")?.values.slice(1) ?? [];
        if (primitives.length !== 1 || primitives[0]?.values?.[0] !== "gr_poly" || val(primitives[0], "fill") !== "yes")
          throw Error("Custom pads require one filled convex polygon.");
        const poly = children(child(primitives[0], "pts") ?? {values: []}, "xy").map(xy);
        const cross = (a, b, c) => (b.x-a.x)*(c.y-a.y)-(b.y-a.y)*(c.x-a.x);
        const signs = poly.map((p,i) => Math.sign(cross(p,poly[(i+1)%poly.length],poly[(i+2)%poly.length]))).filter(Boolean);
        if (poly.length < 3 || signs.some(s => s !== signs[0])) throw Error("Concave custom pads are not supported yet.");
        const radius = number(val(primitives[0], "width", 0)) / 2;
        const size = point(pad, "size");
        if (val(child(pad, "options") ?? {values: []}, "anchor") !== "circle" || size.x !== size.y || radius < 0)
          throw Error("Custom polygon pads require a circular anchor and nonnegative stroke.");
        if (poly.some((a,i) => {
          const b = poly[(i+1)%poly.length];
          return signs[0] * cross(a,b,{x:0,y:0}) / Math.hypot(b.x-a.x,b.y-a.y) + radius < size.x/2;
        })) throw Error("Custom pad polygon must cover its circular anchor.");
        const count = radius ? Math.max(24, Math.ceil(Math.PI / Math.acos(1 / (1 + OUTLINE_TOLERANCE / radius)))) : 1;
        const samples = poly.flatMap(p => Array.from({length: count}, (_,i) => {
          const r = radius ? radius / Math.cos(Math.PI/count) : 0, angle = i * 2*Math.PI/count;
          return {x:p.x+r*Math.cos(angle),y:p.y+r*Math.sin(angle)};
        })).sort((a,b) => a.x-b.x || a.y-b.y);
        const half = points => {const out=[];for(const p of points){while(out.length>1 && cross(out.at(-2),out.at(-1),p)<=0)out.pop();out.push(p);}return out.slice(0,-1);};
        copperPolygon = [...half(samples), ...half([...samples].reverse())];
        warnings.push("Convex custom pad outlines include their stroke, approximated within 0.005 mm; original pad definitions are preserved.");
      } else if (children(pad, "primitives").length) throw Error("Unexpected custom pad primitives.");
      const drillNode = child(pad, "drill"),
        slotted = drillNode?.values[1] === "oval";
      const drill = slotted
        ? Math.min(number(drillNode.values[2]), number(drillNode.values[3]))
        : number(val(pad, "drill", 0));
      if (slotted) {
        if (
          number(drillNode.values[2]) <= 0 ||
          number(drillNode.values[3]) <= 0
        )
          throw Error("Only slots with positive dimensions are supported.");
        warnings.push(
          "Slots retain their copper pad geometry; slot-specific drill checks require KiCad DRC. Original slots are preserved in downloads.",
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
      let unknownCopperLayer;
      const padLayers =
        child(pad, "layers")
          ?.values.slice(1)
          .flatMap((l) => {
            if (l === "*.Cu" || l === "F&B.Cu") return layers.map((l) => l.name);
            if (isCopperLayer(l)) return [l];
            if (/\.Cu$/.test(l) && unknownCopperLayer === undefined) unknownCopperLayer = l;
            return [];
          }) ?? [];
      padLayers.forEach(layerIndex);
      if (!padLayers.length) {
        if (unknownCopperLayer !== undefined)
          throw Error(`Unknown copper layer: ${unknownCopperLayer}`);
        continue; // Paste-only apertures are not copper obstacles.
      }
      const localClearance = (node) => {
        const field = child(node, "clearance");
        if (!field) return undefined;
        const value = number(field.values[1]);
        // KiCad changed explicit zero from inheritance to an override after this format version.
        return value === 0 && Number(val(root, "version", 0)) <= 20240201 ? undefined : value;
      };
      const copperClearance = localClearance(pad) ?? localClearance(fp);
      const maskMargin = number(val(pad, "solder_mask_margin", val(fp, "solder_mask_margin", boardMaskMargin)));
      const rawLayers = child(pad, "layers")?.values.slice(1) ?? [];
      const solderMaskExpansion = Object.fromEntries(
        ["F.Mask", "B.Mask"]
          .filter((layer) => rawLayers.includes(layer) || rawLayers.includes("*.Mask"))
          .map((layer) => [layer, maskMargin]),
      );
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
            sourceFootprint: String(fi),
            sourcePadNumber: String(pad.values[1]),
            netName: netName(pad),
            shape,
            ...(shape === "roundrect" ? { roundRectRatio: number(val(pad, "roundrect_rratio", 0.25)) } : {}),
            size,
            ...(copperPolygon ? { copperPolygon } : {}),
            ...(drillNode && child(drillNode, "offset") ? { shapeOffset: point(drillNode, "offset") } : {}),
            offset: { x: 0, y: 0 },
            position,
            drill,
            nonPlated: type === "np_thru_hole",
            drillEstimated: slotted,
            layers: padLayers,
            solderMaskExpansion,
            effectiveSolderMaskExpansion: { "F.Mask": maskMargin, "B.Mask": maskMargin },
            ...(copperClearance !== undefined ? { copperClearance } : {}),
            allowSolderMaskBridges: child(fp, "attr")?.values.includes("allow_soldermask_bridges") ?? false,
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
  const keptSegments = children(root, "segment").filter((n) => !rippedUp(n));
  const traces = keptSegments.map(
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
  const keptVias = children(root, "via").filter((n) => !rippedUp(n));
  const vias = keptVias.map(
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
  if (children(root, "arc").some((n) => !rippedUp(n)))
    throw Error("Curved tracks are not supported yet.");
  const embedded = children(root, "net_class");
  const netClasses = embeddedNetClasses(root);
  const classNames = new Set();
  const assignments = new Map();
  for (const cls of netClasses) {
    if (typeof cls.name !== "string" || !cls.name || classNames.has(cls.name))
      throw Error("Embedded net class names must be nonempty and unique.");
    classNames.add(cls.name);
    if (cls.clearance < 0 || cls.traceWidth <= 0 || cls.viaDrill <= 0 || cls.viaDiameter <= cls.viaDrill)
      throw Error(`Invalid embedded routing rules for ${cls.name}.`);
    for (const name of cls.netNames) {
      if (assignments.has(name) && assignments.get(name) !== cls.name)
        throw Error(`Net ${name} belongs to multiple embedded net classes.`);
      assignments.set(name, cls.name);
    }
    cls.netNames = [];
  }
  if (!classNames.has("Default"))
    netClasses.unshift({ name: "Default", ...rules, netNames: [] });
  else netClasses.sort((a, b) => (b.name === "Default") - (a.name === "Default"));
  for (const net of nets.filter((n) => n.id > 0)) {
    net.className = assignments.get(net.name) ?? "Default";
    netClasses.find((cls) => cls.name === net.className).netNames.push(net.name);
  }
  if (embedded.length)
    warnings.push(`Imported ${embedded.length} embedded KiCad net classes, including trace widths, clearances and via dimensions.`);
  return {
    text,
    root,
    namedNets,
    ripUpRouting: !!options.ripUpRouting,
    rebuildZones: !!options.rebuildZones,
    warnings: [...new Set(warnings)],
    board: {
      allowSolderMaskBridgesInFootprints: !!setup && val(setup, "allow_soldermask_bridges_in_footprints", "no") === "yes",
      solderMaskMinWidth: setup && child(setup, "solder_mask_min_width") ? number(val(setup, "solder_mask_min_width", 0)) : undefined,
      viaInPadAllowed: !!options.allowViaInPad,
      designName: name,
      unit: "MM",
      resolution: 10000,
      layers,
      nets: nets.filter((n) => n.id > 0),
      netClasses,
      components,
      outline: {
        corners,
        ordered: true,
        cutouts: loops,
        clearance: netClasses[0].clearance + (outline.curved ? OUTLINE_TOLERANCE : 0),
      },
      traces,
      vias,
      conductionAreas,
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

// Returns null when the board cannot be imported, so the preview can still
// render a board the router would reject.
export function netsForSelection(text, name, rules) {
  try {
    const { board } = importBoard(text, name, rules, {
      ripUpRouting: true,
      rebuildZones: true,
    });
    return board.nets.map((net) => ({
      name: net.name,
      className: net.className,
    }));
  } catch {
    return null;
  }
}
