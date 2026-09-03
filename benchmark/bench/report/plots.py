"""Self-contained HTML page of scatter/bar plots comparing 1..N `bench export` JSON files."""
from __future__ import annotations

import json
import math
import statistics
from pathlib import Path

from jinja2 import Environment, FileSystemLoader

from bench.report.html import _esc  # reuse the one escaping helper rather than duplicate it

TEMPLATES = Path(__file__).parent / "templates"
_env = Environment(loader=FileSystemLoader(str(TEMPLATES)), autoescape=True)

PALETTE = ["#2f7ed8", "#d84f2f", "#2fa84f", "#8a2fd8", "#d8a52f", "#2fd8c7", "#d82f7e"]

MIN_SHARED_BOARDS_FOR_PAIRED = 10


def cand_label(e: dict) -> str:
    """"name @ sha7" -- the candidate's resolved git sha is the version identity, so the
    legend always carries it rather than just a (possibly ambiguous) candidate name."""
    sha = (e.get("candidate") or {}).get("sha") or "unknown"
    return f'{e["candidate"]["name"]} @ {sha[:7]}'


def log_ticks(vmin: float, vmax: float) -> list[float]:
    """Tick values in a 1-2-5-per-decade progression spanning [vmin, vmax] (both > 0)."""
    if vmin is None or vmax is None or vmin <= 0 or vmax <= 0 or vmin > vmax:
        return []
    lo, hi = math.floor(math.log10(vmin)), math.ceil(math.log10(vmax))
    ticks = []
    for exp in range(lo, hi + 1):
        for m in (1, 2, 5):
            v = m * (10.0 ** exp)
            if vmin * 0.999 <= v <= vmax * 1.001:
                ticks.append(v)
    return ticks


def _lin_ticks(vmin: float, vmax: float, n: int = 5) -> list[float]:
    if vmax == vmin:
        return [vmin]
    step = (vmax - vmin) / n
    return [vmin + i * step for i in range(n + 1)]


def _fmt(v: float) -> str:
    if abs(v - round(v)) < 1e-9 and abs(v) < 1e6:
        return str(int(round(v)))
    return f"{v:g}"


class _Scale:
    """Maps a data value to a pixel coordinate on one axis, linearly or logarithmically."""

    def __init__(self, vmin: float, vmax: float, pmin: float, pmax: float, log: bool = False):
        self.log = log
        if log:
            vmin = max(vmin, 1e-9)
            vmax = max(vmax, vmin * 1.0001)
            self._lo, self._hi = math.log10(vmin), math.log10(vmax)
            self.vmin = vmin
        else:
            self._lo, self._hi = vmin, vmax
            if self._hi == self._lo:
                self._hi = self._lo + 1.0
            self.vmin = vmin
        self.pmin, self.pmax = pmin, pmax

    def __call__(self, v: float) -> float:
        vv = max(v, self.vmin) if self.log else v
        vv = math.log10(vv) if self.log else vv
        frac = (vv - self._lo) / (self._hi - self._lo)
        return self.pmin + frac * (self.pmax - self.pmin)


def _axes_svg(xsc: _Scale, ysc: _Scale, x_ticks: list[float], y_ticks: list[float],
             x0: float, x1: float, y0: float, y1: float) -> list[str]:
    parts = [f'<line x1="{x0:.1f}" y1="{y1:.1f}" x2="{x1:.1f}" y2="{y1:.1f}" stroke="#888"/>',
             f'<line x1="{x0:.1f}" y1="{y0:.1f}" x2="{x0:.1f}" y2="{y1:.1f}" stroke="#888"/>']
    for t in x_ticks:
        px = xsc(t)
        parts.append(f'<line x1="{px:.1f}" y1="{y1:.1f}" x2="{px:.1f}" y2="{y1 + 4:.1f}" stroke="#888"/>')
        parts.append(f'<text x="{px:.1f}" y="{y1 + 15:.1f}" text-anchor="middle" font-size="9">{_fmt(t)}</text>')
    for t in y_ticks:
        py = ysc(t)
        parts.append(f'<line x1="{x0 - 4:.1f}" y1="{py:.1f}" x2="{x0:.1f}" y2="{py:.1f}" stroke="#888"/>')
        parts.append(f'<text x="{x0 - 7:.1f}" y="{py + 3:.1f}" text-anchor="end" font-size="9">{_fmt(t)}</text>')
    return parts


def scatter_svg(rows: list[dict], x_field: str, y_field: str, *, x_log: bool = False, y_log: bool = False,
                color_field: str = "candidate", color_map: dict[str, str] | None = None,
                radius_field: str | None = None, radius_scale: float = 1.6, min_radius: float = 2.5,
                title_field: str = "board", x_label: str = "", y_label: str = "",
                width: int = 620, height: int = 380,
                x_domain: tuple[float, float] | None = None, y_domain: tuple[float, float] | None = None,
                ref_line: bool = False) -> str:
    """One <circle> per row with an x_field/y_field value, tagged with a <title> of
    row[title_field] for hover. Colour comes from color_map[row[color_field]]."""
    margin = {"left": 58, "right": 16, "top": 14, "bottom": 34}
    x0, x1 = margin["left"], width - margin["right"]
    y0, y1 = margin["top"], height - margin["bottom"]

    pts = [r for r in rows if r.get(x_field) is not None and r.get(y_field) is not None]
    if not pts:
        return f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"></svg>'
    xs, ys = [r[x_field] for r in pts], [r[y_field] for r in pts]

    if x_domain:
        x_lo, x_hi = x_domain
    else:
        x_lo, x_hi = min(xs), max(xs)
        if x_log:
            positive = [v for v in xs if v > 0]
            x_lo = min(positive) if positive else 1e-6
    if y_domain:
        y_lo, y_hi = y_domain
    else:
        y_lo, y_hi = min(ys), max(ys)
        if y_log:
            positive = [v for v in ys if v > 0]
            y_lo = min(positive) if positive else 1e-6

    xsc = _Scale(x_lo, x_hi, x0, x1, log=x_log)
    ysc = _Scale(y_lo, y_hi, y1, y0, log=y_log)  # inverted: larger value -> smaller (higher) pixel y

    x_ticks = log_ticks(x_lo, x_hi) if x_log else _lin_ticks(x_lo, x_hi)
    y_ticks = log_ticks(y_lo, y_hi) if y_log else _lin_ticks(y_lo, y_hi)

    if color_map is None:
        seen: list[str] = []
        for r in pts:
            c = r.get(color_field)
            if c not in seen:
                seen.append(c)
        color_map = {c: PALETTE[i % len(PALETTE)] for i, c in enumerate(seen)}

    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
             f'font-family="system-ui" font-size="11">']
    parts += _axes_svg(xsc, ysc, x_ticks, y_ticks, x0, x1, y0, y1)
    if x_label:
        parts.append(f'<text x="{(x0 + x1) / 2:.1f}" y="{height - 4}" text-anchor="middle">{_esc(x_label)}</text>')
    if y_label:
        cy = (y0 + y1) / 2
        parts.append(f'<text x="12" y="{cy:.1f}" text-anchor="middle" '
                     f'transform="rotate(-90 12 {cy:.1f})">{_esc(y_label)}</text>')
    if ref_line:
        lo, hi = max(x_lo, y_lo), min(x_hi, y_hi)
        if lo < hi:
            parts.append(f'<line x1="{xsc(lo):.1f}" y1="{ysc(lo):.1f}" x2="{xsc(hi):.1f}" y2="{ysc(hi):.1f}" '
                         f'stroke="#999" stroke-dasharray="4,3"/>')

    rmax = max((r.get(radius_field) or 0 for r in pts), default=0) if radius_field else 0
    for r in pts:
        cx, cy = xsc(r[x_field]), ysc(r[y_field])
        color = color_map.get(r.get(color_field), PALETTE[0])
        if radius_field and rmax > 0:
            rad = min_radius + radius_scale * math.sqrt(max(r.get(radius_field) or 0, 0))
        else:
            rad = min_radius + 1.5
        title = _esc(str(r.get(title_field, "")))
        parts.append(f'<circle cx="{cx:.1f}" cy="{cy:.1f}" r="{rad:.1f}" fill="{color}" '
                     f'fill-opacity="0.6" stroke="{color}" stroke-width="0.8"><title>{title}</title></circle>')
    parts.append("</svg>")
    return "".join(parts)


def tier_rate_svg(rows: list[dict], color_map: dict[str, str], width: int = 700, row_h: int = 18) -> str:
    """rows: [{"tier", "candidate", "clean_pass", "connected", "zero_drc"}] (each rate in
    [0, 1]). One grouped-bar row per (tier, candidate), three short bars per row."""
    if not rows:
        return f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="20"></svg>'
    label_w = 230
    bar_area = width - label_w - 20
    metric_w = bar_area // 3
    metrics = [("clean_pass", "clean-pass"), ("connected", "connected"), ("zero_drc", "zero-DRC")]

    height = 26 + row_h * (len(rows) + len({r["tier"] for r in rows})) + 10
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" '
             f'font-family="system-ui" font-size="11">']
    for i, (_, label) in enumerate(metrics):
        parts.append(f'<text x="{label_w + i * metric_w}" y="12" font-size="9" fill="#666">{_esc(label)}</text>')

    y, cur_tier = 24, None
    for row in rows:
        if row["tier"] != cur_tier:
            cur_tier = row["tier"]
            parts.append(f'<text x="0" y="{y + 12}" font-weight="bold" font-size="11">{_esc(cur_tier)}</text>')
            y += row_h
        color = color_map.get(row["candidate"], PALETTE[0])
        parts.append(f'<text x="12" y="{y + 12}" font-size="10">{_esc(row["candidate"])}</text>')
        for i, (key, _) in enumerate(metrics):
            v = row.get(key) or 0.0
            w = metric_w * 0.85 * v
            bx = label_w + i * metric_w
            parts.append(f'<rect x="{bx}" y="{y + 2}" width="{w:.1f}" height="{row_h - 6}" fill="{color}"/>')
            parts.append(f'<text x="{bx + metric_w * 0.85 + 3}" y="{y + 12}" font-size="9">{v * 100:.0f}%</text>')
        y += row_h
    parts.append("</svg>")
    return "".join(parts)


def load_export(path: Path) -> dict:
    return json.loads(Path(path).read_text())


def _paired_section(exports: list[dict], color_map: dict[str, str]) -> dict | None:
    if len(exports) != 2:
        return None
    a, b = exports
    rows_a = {r["board"]: r for r in a["rows"]}
    rows_b = {r["board"]: r for r in b["rows"]}
    shared = sorted(set(rows_a) & set(rows_b))
    if len(shared) < MIN_SHARED_BOARDS_FOR_PAIRED:
        return None
    label_a, label_b = cand_label(a), cand_label(b)
    color = color_map[label_a]
    plots = []
    for field, log in (("cpu_s", True), ("peak_rss_mb", True), ("score", False)):
        pts, ratios = [], []
        for bid in shared:
            va, vb = rows_a[bid].get(field), rows_b[bid].get(field)
            if va is None or vb is None:
                continue
            pts.append({"x": va, "y": vb, "board": bid, "candidate": label_a})
            # Ratio is label_a / label_b (numerator first, matching the subtitle text
            # rendered by the template); only points with a nonzero denominator (vb, the
            # label_b value) enter the median -- a present-but-zero measurement (e.g. a
            # cpu_s of exactly 0.0) still gets plotted (clamped on a log axis), just
            # excluded from the ratio so it can't produce a division by zero or an
            # infinite/undefined ratio skewing the median.
            if vb:
                ratios.append(va / vb)
        if not pts:
            continue
        vals = [p["x"] for p in pts] + [p["y"] for p in pts]
        if log:
            positive = [v for v in vals if v > 0]
            domain = (min(positive) if positive else 1e-6, max(vals))
        else:
            domain = (min(vals), max(vals))
        svg = scatter_svg(pts, "x", "y", x_log=log, y_log=log, color_map={label_a: color},
                          x_domain=domain, y_domain=domain, ref_line=True,
                          x_label=f"{label_a}: {field}", y_label=f"{label_b}: {field}")
        plots.append({"field": field, "svg": svg, "n": len(ratios), "total": len(pts),
                      "median_ratio": statistics.median(ratios) if ratios else None})
    return {"label_a": label_a, "label_b": label_b, "n": len(shared), "plots": plots}


def render(exports: list[dict]) -> str:
    """exports: parsed `bench export` JSON dicts (schema_version 1), in the order given on
    the command line (also the legend/colour order)."""
    color_map = {cand_label(e): PALETTE[i % len(PALETTE)] for i, e in enumerate(exports)}

    all_rows = []
    for e in exports:
        label = cand_label(e)
        for r in e["rows"]:
            rr = dict(r)
            rr["candidate"] = label
            all_rows.append(rr)

    cpu_vs_score = scatter_svg(all_rows, "cpu_s", "score", x_log=True, y_log=False,
                               color_map=color_map, radius_field="nets", y_domain=(0.0, 1000.0),
                               x_label="CPU time (s, log)", y_label="score")
    nets_vs_rss = scatter_svg(all_rows, "nets", "peak_rss_mb", x_log=True, y_log=True,
                              color_map=color_map, x_label="nets (log)", y_label="peak RSS (MB, log)")
    nets_vs_cpu = scatter_svg(all_rows, "nets", "cpu_s", x_log=True, y_log=True,
                              color_map=color_map, x_label="nets (log)", y_label="CPU time (s, log)")

    paired = _paired_section(exports, color_map)

    tier_names: list[str] = []
    for e in exports:
        for r in e["rows"]:
            t = r.get("tier") or "(none)"
            if t not in tier_names:
                tier_names.append(t)
    tier_rows = []
    for tier in sorted(tier_names):
        for e in exports:
            label = cand_label(e)
            rs = [r for r in e["rows"] if (r.get("tier") or "(none)") == tier]
            if not rs:
                continue
            n = len(rs)
            tier_rows.append({
                "tier": tier, "candidate": label,
                "clean_pass": sum(1 for r in rs if r.get("clean_pass")) / n,
                "connected": sum(1 for r in rs if r.get("unrouted") == 0) / n,
                "zero_drc": sum(1 for r in rs if r.get("violations") == 0) / n,
            })
    tier_svg = tier_rate_svg(tier_rows, color_map)

    tpl = _env.get_template("plots.html.j2")
    return tpl.render(exports=exports, cand_label=cand_label, color_map=color_map,
                      cpu_vs_score=cpu_vs_score, nets_vs_rss=nets_vs_rss, nets_vs_cpu=nets_vs_cpu,
                      paired=paired, tier_svg=tier_svg)
