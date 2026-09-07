"""Render a compare JSON as a short Markdown block for pasting into a pull request.

`markdown.render` writes the full report — every board, every metric — which is the artifact
to keep. This renders only what a reviewer needs in a comment: the verdict, whether the
regression gate passes, and the boards whose verdict is not a tie.
"""
from __future__ import annotations

from bench.compare import gate_failures

MAX_BOARD_ROWS = 20


def _num(value, nd=1):
    if value is None:
        return "—"
    return f"{value:.{nd}f}" if isinstance(value, float) else str(value)


def _delta(value, nd=1):
    if value is None:
        return "—"
    if isinstance(value, float):
        # A value that rounds to zero prints without a sign: "+0.0" reads as a change.
        return f"{value:.{nd}f}" if round(value, nd) == 0 else f"{value:+.{nd}f}"
    if isinstance(value, int):
        return "0" if value == 0 else f"{value:+d}"
    return str(value)


def _clean_pass_pair(cmp: dict, name: str) -> tuple[float | None, float | None]:
    """The baseline's and the candidate's clean-pass rate over the boards they both routed."""
    rows = [(e["baseline"], e["against"][name]) for e in cmp["boards"].values()
            if e.get("against", {}).get(name)]
    if not rows:
        return None, None
    total = len(rows)
    base = sum(1 for b, _ in rows if b.get("clean_pass_rate") == 1.0) / total
    cand = sum(1 for _, a in rows if a["agg"].get("clean_pass_rate") == 1.0) / total
    return base, cand


def _compared(cmp: dict, name: str) -> str:
    coverage = (cmp.get("coverage") or {}).get(name)
    if coverage:
        return f"{coverage['compared_boards']} of {len(coverage['shared_boards'])} shared"
    compared = sum(1 for e in cmp["boards"].values() if e.get("against", {}).get(name))
    return f"{compared} of {len(cmp['boards'])}"


def _changed_rows(cmp: dict, name: str) -> list[tuple[str, dict, dict]]:
    """(board, delta, verdict) for every non-tie board, worst losses first."""
    rows = []
    for bid, entry in cmp["boards"].items():
        against = entry.get("against", {}).get(name)
        if not against or not against.get("verdict"):
            continue
        if against["verdict"]["result"] == "tie":
            continue
        rows.append((bid, against["delta"], against["verdict"]))
    rows.sort(key=lambda r: (r[2]["result"] != "loss", -abs(r[1].get("score") or 0.0)))
    return rows


def render(cmp: dict, compare_id: str | None = None) -> str:
    baseline = cmp["baseline"]
    names = cmp["against"]
    config = cmp.get("config", {})
    time_metric = config.get("time_metric", "wall_s")
    time_label = "wall s" if time_metric == "wall_s" else "cpu s"
    failures = gate_failures(cmp)

    L: list[str] = [f"## Benchmark: {', '.join(names)} vs {baseline}", ""]

    verdicts = ", ".join(f"**{cmp['overall'][n]['verdict']}**" for n in names)
    if failures:
        L.append(f"{verdicts} — the regression gate **fails**:")
        L.append("")
        L += [f"- {reason}" for reason in failures]
    else:
        L.append(f"{verdicts} — the regression gate passes.")
    L.append("")

    for name in names:
        overall = cmp["overall"][name]
        tier_stats = (cmp.get("tiers", {}).get("all") or {}).get(name, {})
        base_rate, cand_rate = _clean_pass_pair(cmp, name)
        if len(names) > 1:
            L += [f"### {name}", ""]
        L += ["| metric | result |", "|---|---|",
              f"| Boards compared | {_compared(cmp, name)} |",
              f"| Wins / losses / ties | {overall['wins']} / {overall['losses']} / {overall['ties']} |",
              f"| Quality losses | {overall.get('quality_losses', overall.get('hard_losses', 0))} |"]
        performance_losses = overall.get("performance_losses")
        if performance_losses is not None:
            gated = config.get("performance_regression_percent")
            note = f"gated above {gated:g}%" if gated is not None else "advisory"
            L.append(f"| Performance losses | {performance_losses} ({note}) |")
        if base_rate is not None:
            L.append(f"| Clean-pass rate | {base_rate:.2f} → {cand_rate:.2f} |")
        if tier_stats:
            L += [f"| Median Δscore | {_delta(tier_stats.get('median_score_delta'))} |",
                  f"| Median time ratio | {_num(tier_stats.get('median_time_ratio'), 2)} |"]
        L.append("")

        rows = _changed_rows(cmp, name)
        if rows:
            shown = rows[:MAX_BOARD_ROWS]
            plural = "board" if len(rows) == 1 else "boards"
            L += [f"<details><summary>{len(rows)} {plural} changed</summary>", "",
                  f"| board | verdict | Δunrouted | Δviol | Δscore | Δ{time_label} |",
                  "|---|---|---|---|---|---|"]
            for bid, delta, verdict in shown:
                L.append(f"| {bid} | {verdict['result']} ({verdict['level']}) | "
                         f"{_delta(delta.get('unrouted'))} | {_delta(delta.get('violations'))} | "
                         f"{_delta(delta.get('score'))} | {_delta(delta.get(time_metric))} |")
            if len(rows) > len(shown):
                L.append(f"| … | and {len(rows) - len(shown)} more | | | | |")
            L += ["", "</details>", ""]

    candidates = cmp.get("candidates", {})
    identity = " · ".join(
        f"{n} `{(candidates.get(n) or {}).get('sha') or '—'}`" for n in [baseline, *names])
    L.append(f"Baseline first: {identity}")
    settings = (f"threads={config.get('threads')} jobs={config.get('jobs', 1)} "
                f"max_passes={config.get('max_passes')} timeout={config.get('timeout_s')}s "
                f"seeds={config.get('seeds')}")
    if cmp.get("tier"):
        settings += f" tier={cmp['tier']}"
    L += ["", f"{settings}, deciding on {time_label}. Runs {', '.join(cmp.get('runs', []))}."]

    caveats: list[str] = []
    seeds = config.get("seeds")
    if seeds is not None and seeds < 3:
        caveats.append(f"seeds={seeds} (<3), so the noise floor is unmeasured and timing "
                       f"differences here are not evidence.")
    for name, coverage in (cmp.get("coverage") or {}).items():
        excluded = len(coverage["incomplete_boards"]) + len(coverage["skipped_boards"])
        if excluded:
            caveats.append(f"{name}: {len(coverage['incomplete_boards'])} incomplete and "
                           f"{len(coverage['skipped_boards'])} skipped boards are excluded, "
                           f"not counted as ties.")
    if cmp.get("failures"):
        caveats.append(f"{len(cmp['failures'])} failed cells; see the full report.")
    for warning in cmp.get("warnings", []):
        caveats.append(warning)
    if caveats:
        L += ["", "**Caveats**", ""] + [f"- {c}" for c in caveats]

    if compare_id:
        L += ["", f"<sub>Generated by `uv run bench pr-summary --compare {compare_id}` "
                  f"from `reports/{compare_id}.json`.</sub>"]
    return "\n".join(L) + "\n"
