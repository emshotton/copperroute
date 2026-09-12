// KiCad stores footprint geometry in footprint-local coordinates, including
// already-flipped coordinates for back-side footprints. Rotation uses PCB Y-down.
const kids = (n, key) => n.values.filter((v) => v?.values?.[0] === key);
const child = (n, key) => kids(n, key)[0];
const val = (n, key, fallback) => child(n, key)?.values[1] ?? fallback;
const num = (v) => {
  const n = Number(v);
  if (!Number.isFinite(n) || Math.abs(n) > 100000)
    throw Error("Invalid outline coordinate.");
  return n;
};
const xy = (n) => {
  if (!n) throw Error("Missing outline coordinate.");
  return { x: num(n.values[1]), y: num(n.values[2]) };
};
const point = (n, key) => xy(child(n, key));
export const OUTLINE_TOLERANCE = 0.005; // Maximum chord deviation in mm.
export function footprintPoint(fp, p) {
  const a = point(fp, "at"),
    angle = (num(child(fp, "at").values[3] ?? 0) * Math.PI) / 180;
  return {
    x: a.x + p.x * Math.cos(angle) + p.y * Math.sin(angle),
    y: a.y - p.x * Math.sin(angle) + p.y * Math.cos(angle),
  };
}
function sample(center, radius, start, sweep) {
  if (!(radius > 0)) throw Error("Degenerate outline arc.");
  const step = Math.min(
    Math.PI / 12,
    2 * Math.acos(Math.max(-1, 1 - OUTLINE_TOLERANCE / radius)),
  );
  const count = Math.ceil(Math.abs(sweep) / step);
  if (!Number.isFinite(count) || count > 4096)
    throw Error("Outline curve is too large to import.");
  return Array.from({ length: count + 1 }, (_, i) => ({
    x: center.x + radius * Math.cos(start + (sweep * i) / count),
    y: center.y + radius * Math.sin(start + (sweep * i) / count),
  }));
}
export function arcPoints(a, m, b) {
  const d = 2 * (a.x * (m.y - b.y) + m.x * (b.y - a.y) + b.x * (a.y - m.y));
  if (Math.abs(d) < 1e-10) throw Error("Degenerate outline arc.");
  const aa = a.x * a.x + a.y * a.y,
    mm = m.x * m.x + m.y * m.y,
    bb = b.x * b.x + b.y * b.y;
  const c = {
    x: (aa * (m.y - b.y) + mm * (b.y - a.y) + bb * (a.y - m.y)) / d,
    y: (aa * (b.x - m.x) + mm * (a.x - b.x) + bb * (m.x - a.x)) / d,
  };
  const angle = (p) => Math.atan2(p.y - c.y, p.x - c.x),
    norm = (v) => (v + Math.PI * 4) % (Math.PI * 2);
  const start = angle(a),
    span = norm(angle(b) - start),
    sweep = norm(angle(m) - start) < span ? span : span - Math.PI * 2;
  const points = sample(c, Math.hypot(a.x - c.x, a.y - c.y), start, sweep);
  points[0] = a;
  points[points.length - 1] = b;
  return points;
}
export function boundingBox(points, margin) {
  const xs = points.map((p) => p.x),
    ys = points.map((p) => p.y);
  const x0 = Math.min(...xs) - margin,
    x1 = Math.max(...xs) + margin;
  const y0 = Math.min(...ys) - margin,
    y1 = Math.max(...ys) + margin;
  return [
    { x: x0, y: y0 },
    { x: x1, y: y0 },
    { x: x1, y: y1 },
    { x: x0, y: y1 },
  ];
}
export function nativeArcPoints(node) {
  if (child(node, "mid")) return arcPoints(point(node, "start"), point(node, "mid"), point(node, "end"));
  const center = point(node, "start"), start = point(node, "end");
  const points = sample(center, Math.hypot(start.x - center.x, start.y - center.y),
    Math.atan2(start.y - center.y, start.x - center.x), num(val(node, "angle")) * Math.PI / 180);
  points[0] = start;
  return points;
}
export function outlinePaths(root) {
  const paths = [];
  let curved = false;
  const collect = (node, transform = (p) => p) => {
    if (val(node, "layer", "") !== "Edge.Cuts") return;
    const kind = node.values[0].replace(/^fp_/, "gr_");
    if (kind === "target" || kind === "gr_target") return;
    let points;
    if (kind === "gr_line") points = [point(node, "start"), point(node, "end")];
    else if (kind === "gr_rect") {
      const a = point(node, "start"),
        b = point(node, "end");
      points = [a, { x: b.x, y: a.y }, b, { x: a.x, y: b.y }, a];
    } else if (kind === "gr_poly") {
      points = kids(child(node, "pts") ?? { values: [] }, "xy").map(xy);
      if (points.length) points.push(points[0]);
    } else if (kind === "gr_arc") {
      points = nativeArcPoints(node);
      curved = true;
    } else if (kind === "gr_circle") {
      const c = point(node, "center"),
        e = point(node, "end");
      points = sample(c, Math.hypot(e.x - c.x, e.y - c.y), 0, Math.PI * 2);
      points[points.length - 1] = points[0];
      curved = true;
    } else throw Error(`Unsupported board outline object: ${kind}`);
    if (points.length < 2) throw Error("Empty outline path.");
    paths.push(points.map(transform));
  };
  for (const n of root.values.filter((v) => v?.values)) collect(n);
  for (const fp of [...kids(root, "footprint"), ...kids(root, "module")])
    for (const n of fp.values.filter((v) => v?.values))
      collect(n, (p) => footprintPoint(fp, p));
  return { paths, curved };
}
