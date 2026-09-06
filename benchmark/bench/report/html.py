"""Self-contained HTML dashboard for a compare JSON, with history across reports/."""
from __future__ import annotations

import json
from pathlib import Path

from jinja2 import Environment, FileSystemLoader

TEMPLATES = Path(__file__).parent / "templates"
_env = Environment(loader=FileSystemLoader(str(TEMPLATES)), autoescape=True)


def _esc(s: str) -> str:
    return s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def bar_svg(values: list[tuple[str, float, float]], width: int = 640, row_h: int = 22) -> str:
    """Horizontal grouped bars: for each (label, baseline, candidate)."""
    if not values:
        return "<svg></svg>"
    vmax = max(max(abs(b), abs(c)) for _, b, c in values) or 1.0
    label_w, h = 200, row_h * len(values) + 10
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{h}" font-family="system-ui" font-size="11">']
    for i, (label, b, c) in enumerate(values):
        y = 5 + i * row_h
        bw, cw = (width - label_w - 10) * abs(b) / vmax, (width - label_w - 10) * abs(c) / vmax
        parts.append(f'<text x="{label_w - 6}" y="{y + 14}" text-anchor="end">{_esc(label)}</text>')
        parts.append(f'<rect x="{label_w}" y="{y}" width="{bw:.1f}" height="8" fill="#8a8a8a"/>')
        parts.append(f'<rect x="{label_w}" y="{y + 10}" width="{cw:.1f}" height="8" fill="#2f7ed8"/>')
    parts.append("</svg>")
    return "".join(parts)


def line_svg(series: dict[str, list[tuple[str, float]]], width: int = 640, height: int = 200,
             order: list[str] | None = None) -> str:
    colors = ["#2f7ed8", "#d84f2f", "#2fa84f", "#8a2fd8", "#d8a52f"]
    all_xs = {x for pts in series.values() for x, _ in pts}
    if order:
        # Chronological (created_at) order, as given; any x not in `order` (shouldn't
        # normally happen) is appended, alphabetically, so nothing is silently dropped.
        xs = [x for x in order if x in all_xs] + sorted(all_xs - set(order))
    else:
        xs = sorted(all_xs)
    if not xs:
        return "<svg></svg>"
    ys = [y for pts in series.values() for _, y in pts]
    ymin, ymax = min(ys), max(ys)
    span = (ymax - ymin) or 1.0
    pad = 30
    def px(i): return pad + (width - 2 * pad) * (i / max(len(xs) - 1, 1))
    def py(v): return height - pad - (height - 2 * pad) * ((v - ymin) / span)
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" font-family="system-ui" font-size="10">']
    for i, x in enumerate(xs):
        parts.append(f'<text x="{px(i):.1f}" y="{height - 8}" text-anchor="middle">{_esc(x[:7])}</text>')
    for k, (name, pts) in enumerate(series.items()):
        idx = {x: i for i, x in enumerate(xs)}
        coords = " ".join(f"{px(idx[x]):.1f},{py(y):.1f}" for x, y in pts)
        parts.append(f'<polyline fill="none" stroke="{colors[k % len(colors)]}" stroke-width="2" points="{coords}"/>')
        parts.append(f'<text x="{pad}" y="{12 + 12 * k}" fill="{colors[k % len(colors)]}">{_esc(name)}</text>')
    parts.append("</svg>")
    return "".join(parts)


def load_history(reports_dir: Path) -> list[dict]:
    items = []
    for p in reports_dir.glob("*.json"):
        try:
            d = json.loads(p.read_text())
        except ValueError:
            continue
        if d.get("schema_version") == 1 and "overall" in d:
            items.append(d)
    return sorted(items, key=lambda d: d.get("created_at", ""))


def render(cmp: dict, history: list[dict]) -> str:
    names = cmp["against"]
    time_metric = cmp["config"].get("time_metric", "wall_s")
    time_heading = ("Wall time per board" if time_metric == "wall_s" else "CPU time per board") \
        + f" ({time_metric})"
    def pairs(name: str, metric: str) -> list[tuple[str, float, float]]:
        values = []
        for bid, entry in cmp["boards"].items():
            other = entry["against"].get(name)
            if not other or (metric == "score" and not other.get("score_comparable", True)):
                continue
            base_value, other_value = entry["baseline"].get(metric), other["agg"].get(metric)
            if base_value is not None and other_value is not None:
                values.append((bid, base_value, other_value))
        return values

    score_bars = {n: bar_svg(pairs(n, "score")) for n in names}
    time_bars = {n: bar_svg(pairs(n, time_metric)) for n in names}
    cp_series: dict[str, list[tuple[str, float]]] = {}
    score_series: dict[str, list[tuple[str, float]]] = {}
    # `history` is already sorted by created_at (see load_history); the x-axis order for
    # both charts must follow that chronological order, not an alphabetical sort of shas.
    sha_order: list[str] = []
    seen_shas: set[str] = set()
    for h in history:
        for n in h["against"]:
            sha = h["candidates"].get(n, {}).get("sha", "?")
            if sha not in seen_shas:
                seen_shas.add(sha)
                sha_order.append(sha)
            t = h["tiers"].get("all", {}).get(n)
            if t:
                cp_series.setdefault(n, []).append((sha, t["clean_pass_rate"]))
                score_series.setdefault(n, []).append((sha, t["median_score_delta"] or 0.0))
    tpl = _env.get_template("dashboard.html.j2")
    return tpl.render(cmp=cmp, names=names, score_bars=score_bars, time_bars=time_bars,
                      time_heading=time_heading,
                      cp_history=line_svg(cp_series, order=sha_order),
                      score_history=line_svg(score_series, order=sha_order))
