import { outlinePaths, nativeArcPoints, footprintPoint } from "./geometry.js";
import { parse } from "./kicad.js";
export function layerColor(name) {
  if (name === "F.Cu") return "#f26d78";
  if (name === "B.Cu") return "#68b8ff";
  if (name === "Edge.Cuts") return "#83c5af";
  const inner = /^In(\d+)\.Cu$/.exec(name);
  if (inner) return ["#ce9fff", "#ffa657", "#83c5af", "#ff8fc8"][(Number(inner[1]) - 1) % 4];
  return "#a6adb4";
}
export function previewLayers(text) {
  const root = parse(text);
  return (root.values.find(n => n?.values?.[0] === "layers")?.values ?? [])
    .filter(n => n?.values && /\.Cu$/.test(n.values[1]))
    .map(n => ({ name: n.values[1], label: n.values[3] ?? n.values[1],
      type: n.values[2], color: layerColor(n.values[1]) }));
}
// Display native geometry independently of the router's supported import subset.
// All coordinates are numeric and all text is XML-escaped.
export function previewBoard(text, airlines = []) {
  const root = parse(text),
    points = [],
    shapes = [],
    labels = [];
  const kids = (n, k) => n.values.filter((v) => v?.values?.[0] === k);
  const one = (n, k) => kids(n, k)[0];
  const value = (n, k, d) => one(n, k)?.values[1] ?? d;
  const num = (v, d = 0) => {
    const n = Number(v ?? d);
    if (!Number.isFinite(n) || Math.abs(n) > 100000)
      throw Error("Invalid preview coordinate.");
    return n;
  };
  const xy = (n) => ({ x: num(n?.values[1]), y: num(n?.values[2]) });
  const at = (n, k) => xy(one(n, k));
  const escape = (s) =>
    String(s).replace(
      /[&<>"']/g,
      (c) =>
        ({
          "&": "&amp;",
          "<": "&lt;",
          ">": "&gt;",
          '"': "&quot;",
          "'": "&apos;",
        })[c],
    );
  const color = layerColor;
  const path = (ps) => ps.map((p) => `${p.x},${p.y}`).join(" ");
  const collect = (ps) => points.push(...ps);
  const footprints = [...kids(root, "footprint"), ...kids(root, "module")];
  // Include outlines embedded in footprints using the same coordinate transform
  // as routing. Allow other geometry to remain previewable if an outline fails.
  try {
    for (const ps of outlinePaths(root).paths) {
      collect(ps);
      shapes.push(
        `<polyline points="${path(ps)}" fill="none" stroke="#83c5af" stroke-width="0.15"/>`,
      );
    }
  } catch {
    /* The routing importer reports outline errors separately. */
  }
  // KiCad zone fills encode holes using bridged contours; evenodd displays them.
  for (const zone of kids(root, "zone"))
    for (const fill of kids(zone, "filled_polygon")) {
      const ps = kids(one(fill, "pts") ?? { values: [] }, "xy").map(xy);
      if (!ps.length) continue;
      collect(ps);
      shapes.push(
        `<path d="M ${ps.map((p) => `${p.x} ${p.y}`).join(" L ")} Z" fill="${color(value(fill, "layer", value(zone, "layer", "F.Cu")))}" fill-rule="evenodd" opacity="0.15"/>`,
      );
    }
  for (const n of root.values.filter((v) => v?.values)) {
    const kind = n.values[0],
      layer = value(n, "layer", "");
    if (layer === "Edge.Cuts") continue;
    if (["segment", "gr_line"].includes(kind)) {
      const ps = [at(n, "start"), at(n, "end")];
      collect(ps);
      const width = num(
        value(
          n,
          "width",
          value(one(n, "stroke") ?? { values: [] }, "width", 0.15),
        ),
      );
      shapes.push(
        `<polyline points="${path(ps)}" fill="none" data-layer="${escape(layer)}" stroke="${color(layer)}" stroke-width="${width}" stroke-linecap="round"/>`,
      );
    }
    if (kind === "gr_rect") {
      const a = at(n, "start"),
        b = at(n, "end");
      collect([a, b]);
      shapes.push(
        `<rect x="${Math.min(a.x, b.x)}" y="${Math.min(a.y, b.y)}" width="${Math.abs(a.x - b.x)}" height="${Math.abs(a.y - b.y)}" fill="none" stroke="${color(layer)}" stroke-width="0.15"/>`,
      );
    }
    if (kind === "gr_circle") {
      const a = at(n, "center"),
        b = at(n, "end"),
        r = Math.hypot(b.x - a.x, b.y - a.y);
      collect([
        { x: a.x - r, y: a.y - r },
        { x: a.x + r, y: a.y + r },
      ]);
      shapes.push(
        `<circle cx="${a.x}" cy="${a.y}" r="${r}" fill="none" stroke="${color(layer)}" stroke-width="0.15"/>`,
      );
    }
    if (kind === "gr_poly") {
      const ps = kids(one(n, "pts") ?? { values: [] }, "xy").map(xy);
      collect(ps);
      shapes.push(
        `<polygon points="${path(ps)}" fill="none" stroke="${color(layer)}" stroke-width="0.15"/>`,
      );
    }
    if ((kind === "arc" || kind === "gr_arc") && !one(n, "mid")) {
      const ps = nativeArcPoints(n);
      collect(ps);
      shapes.push(`<polyline points="${path(ps)}" fill="none" stroke="${color(layer)}" stroke-width="${num(value(n, "width", 0.15))}"/>`);
      continue;
    }
    if (kind === "arc" || kind === "gr_arc") {
      const a = at(n, "start"),
        m = at(n, "mid"),
        b = at(n, "end");
      collect([a, m, b]);
      const d = 2 * (a.x * (m.y - b.y) + m.x * (b.y - a.y) + b.x * (a.y - m.y));
      if (Math.abs(d) > 1e-10) {
        const aa = a.x * a.x + a.y * a.y,
          mm = m.x * m.x + m.y * m.y,
          bb = b.x * b.x + b.y * b.y;
        const cx = (aa * (m.y - b.y) + mm * (b.y - a.y) + bb * (a.y - m.y)) / d,
          cy = (aa * (b.x - m.x) + mm * (a.x - b.x) + bb * (m.x - a.x)) / d,
          r = Math.hypot(a.x - cx, a.y - cy);
        const angle = (p) => Math.atan2(p.y - cy, p.x - cx),
          norm = (v) => (v + Math.PI * 4) % (Math.PI * 2),
          sweep = norm(angle(m) - angle(a)) < norm(angle(b) - angle(a)),
          span = sweep ? norm(angle(b) - angle(a)) : norm(angle(a) - angle(b));
        shapes.push(
          `<path d="M ${a.x} ${a.y} A ${r} ${r} 0 ${span > Math.PI ? 1 : 0} ${sweep ? 1 : 0} ${b.x} ${b.y}" fill="none" stroke="${color(layer)}" stroke-width="${num(value(n, "width", 0.15))}"/>`,
        );
      }
    }
  }
  for (const fp of footprints) {
    const origin = at(fp, "at"),
      angle = (num(one(fp, "at")?.values[3]) * Math.PI) / 180;
    for (const rect of kids(fp, "fp_rect")) {
      const layer = value(rect, "layer", "");
      if (!layer.endsWith(".Cu")) continue;
      const a = at(rect, "start"), b = at(rect, "end");
      const ps = [a, {x:b.x,y:a.y}, b, {x:a.x,y:b.y}].map(p => footprintPoint(fp,p));
      collect(ps);
      shapes.push(`<polygon points="${path(ps)}" fill="${value(rect,"fill") === "yes" ? color(layer) : "none"}" stroke="${color(layer)}" stroke-width="${num(value(one(rect,"stroke") ?? rect,"width",0))}"/>`);
    }
    for (const p of kids(fp, "pad")) {
      if (
        !one(p, "layers")?.values.some(
          (l) => typeof l === "string" && l.endsWith(".Cu"),
        )
      )
        continue;
      const local = at(p, "at"),
        size = at(p, "size"),
        x = origin.x + local.x * Math.cos(angle) + local.y * Math.sin(angle),
        y = origin.y - local.x * Math.sin(angle) + local.y * Math.cos(angle);
      collect([
        { x: x - size.x, y: y - size.y },
        { x: x + size.x, y: y + size.y },
      ]);
      const rotation = -num(one(p, "at")?.values[3]),
        type = p.values[3];
      const radius =
        type === "oval"
          ? Math.min(size.x, size.y) / 2
          : type === "roundrect"
            ? num(value(p, "roundrect_rratio", 0.25)) * Math.min(size.x, size.y)
            : 0;
      const shapeOffset = at(one(p, "drill") ?? {values: []}, "offset");
      shapes.push(
        `<g transform="translate(${x} ${y}) rotate(${rotation})" fill="#edcf86"><title>${escape(p.values[1])} · ${escape(value(p, "net", ""))}</title><g transform="translate(${shapeOffset.x} ${shapeOffset.y})">${type === "circle" || type === "custom" ? `<ellipse rx="${size.x / 2}" ry="${size.y / 2}"/>` : `<rect x="${-size.x / 2}" y="${-size.y / 2}" width="${size.x}" height="${size.y}" rx="${radius}"/>`}`,
      );
      if (type === "custom")
        for (const primitive of kids(one(p, "primitives") ?? {values: []}, "gr_poly")) {
          const ps = kids(one(primitive, "pts") ?? {values: []}, "xy").map(xy);
          shapes.push(`<polygon points="${path(ps)}" fill="#edcf86" stroke="#edcf86" stroke-width="${num(value(primitive, "width", 0))}" stroke-linejoin="round"/>`);
        }
      shapes.push("</g>");
      const drill = one(p, "drill");
      if (drill) {
        if (drill.values[1] === "oval") {
          const w = num(drill.values[2]),
            h = num(drill.values[3]);
          shapes.push(
            `<rect x="${-w / 2}" y="${-h / 2}" width="${w}" height="${h}" rx="${Math.min(w, h) / 2}" fill="#101c25"/>`,
          );
        } else
          shapes.push(
            `<circle r="${num(drill.values[1]) / 2}" fill="#101c25"/>`,
          );
      }
      shapes.push("</g>");
    }
    const reference =
      kids(fp, "property").find((n) => n.values[1] === "Reference")
        ?.values[2] ??
      kids(fp, "fp_text").find((n) => n.values[1] === "reference")?.values[2];
    if (reference)
      labels.push(
        `<text x="${origin.x}" y="${origin.y - 1.2}" fill="#d9e4e0" font-size="0.7" text-anchor="middle">${escape(reference)}</text>`,
      );
  }
  for (const v of kids(root, "via")) {
    const p = at(v, "at"),
      r = num(value(v, "size", 0.6)) / 2;
    collect([p]);
    shapes.push(
      `<circle cx="${p.x}" cy="${p.y}" r="${r}" fill="#edcf86"/><circle cx="${p.x}" cy="${p.y}" r="${num(value(v, "drill", 0.3)) / 2}" fill="#101c25"/>`,
    );
  }
  const ratsnest = airlines.map(({ from, to }) => {
    const a = { x: num(from?.[0]), y: num(from?.[1]) },
      b = { x: num(to?.[0]), y: num(to?.[1]) };
    collect([a, b]);
    return `M ${a.x} ${a.y} L ${b.x} ${b.y}`;
  });
  const ratsnestGroup = ratsnest.length
    ? `<g class="ratsnest" fill="none" stroke="#e8f0f2" stroke-width="0.06" stroke-dasharray="0.35 0.25" opacity="0.4"><path d="${ratsnest.join(" ")}"/></g>`
    : "";
  if (!points.length) throw Error("No displayable board geometry found.");
  const bounds = points.reduce(
    (b, p) => ({
      x: Math.min(b.x, p.x),
      y: Math.min(b.y, p.y),
      right: Math.max(b.right, p.x),
      bottom: Math.max(b.bottom, p.y),
    }),
    { x: Infinity, y: Infinity, right: -Infinity, bottom: -Infinity },
  );
  const x = bounds.x - 2,
    y = bounds.y - 2,
    w = bounds.right - x + 2,
    h = bounds.bottom - y + 2;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${x} ${y} ${w} ${h}" role="img" aria-label="Native KiCad board preview"><rect x="${x}" y="${y}" width="${w}" height="${h}" fill="#101c25"/>${shapes.join("")}${ratsnestGroup}${labels.join("")}</svg>`;
}
