"""Render a compare JSON as Markdown."""
from __future__ import annotations


def _f(v, nd=1):
    if v is None:
        return "—"
    return f"{v:.{nd}f}" if isinstance(v, float) else str(v)


def _delta(v, nd=1):
    if v is None:
        return "—"
    sign = "+" if v > 0 else ""
    return f"{sign}{v:.{nd}f}" if isinstance(v, float) else f"{sign}{v}"


def render(cmp: dict) -> str:
    L: list[str] = []
    names = cmp["against"]
    L.append(f"# Comparison: {cmp['baseline']} vs {', '.join(names)}")
    L.append("")
    time_metric = cmp["config"].get("time_metric", "wall_s")
    L.append(f"Created {cmp['created_at']} from runs {', '.join(cmp['runs'])}. "
             f"Config: threads={cmp['config'].get('threads')} jobs={cmp['config'].get('jobs', 1)} "
             f"max_passes={cmp['config'].get('max_passes')} "
             f"timeout={cmp['config'].get('timeout_s')}s seeds={cmp['config'].get('seeds')} "
             f"time metric = {time_metric}"
             + (f" tier={cmp['tier']}" if cmp.get("tier") else ""))
    L.append("")
    L.append("| candidate | version | sha |")
    L.append("|---|---|---|")
    for n, i in cmp["candidates"].items():
        L.append(f"| {n} | {i.get('version') or '—'} | `{i.get('sha')}` |")
    L.append("")
    L.append("## Overall")
    L.append("")
    for n in names:
        o = cmp["overall"][n]
        L.append(f"- **{n}** vs {cmp['baseline']}: **{o['verdict']}** — {o['wins']} wins, {o['losses']} losses "
                 f"({o['hard_losses']} on hard metrics), {o['ties']} ties")
    L.append("")
    L.append("## Per tier")
    L.append("")
    L.append("| tier | candidate | wins | losses | ties | clean-pass rate | median Δscore | median time ratio |")
    L.append("|---|---|---|---|---|---|---|---|")
    for t, row in cmp["tiers"].items():
        for n, s in row.items():
            L.append(f"| {t} | {n} | {s['wins']} | {s['losses']} | {s['ties']} | {_f(s['clean_pass_rate'], 2)} | "
                     f"{_delta(s['median_score_delta'])} | {_f(s['median_time_ratio'], 2)} |")
    L.append("")
    L.append("## Per board")
    L.append("")
    seeds = cmp["config"].get("seeds")
    few_seeds = seeds is not None and seeds < 3
    time_label = "wall s" if time_metric == "wall_s" else "cpu s"
    L.append(f"| board | referee | candidate | clean | unrouted | viol | score | Δscore | noise | {time_label} | "
             f"Δ{time_label} | vias | length mm | verdict |")
    L.append("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|")
    for bid, e in cmp["boards"].items():
        b = e["baseline"]
        noise_val = "unmeasured" if few_seeds else _f(e["noise"].get("score"))
        L.append(f"| {bid} | {e['referee']} | {cmp['baseline']} | {_f(b['clean_pass_rate'], 2)} | {_f(b['unrouted'])} | {_f(b['violations'])} | "
                 f"{_f(b['score'])} | | {noise_val} | {_f(b.get(time_metric))} | | {_f(b['vias'])} | {_f(b['wirelength_mm'])} | baseline |")
        for n in names:
            a = e["against"].get(n)
            if not a:
                L.append(f"| {bid} | {e['referee']} | {n} | — | — | — | — | — | — | — | — | — | — | missing |")
                continue
            g, d, v = a["agg"], a["delta"], a["verdict"]
            L.append(f"| {bid} | {e['referee']} | {n} | {_f(g['clean_pass_rate'], 2)} | {_f(g['unrouted'])} | {_f(g['violations'])} | "
                     f"{_f(g['score'])} | {_delta(d['score'])} | | {_f(g.get(time_metric))} | {_delta(d.get(time_metric))} | {_f(g['vias'])} | "
                     f"{_f(g['wirelength_mm'])} | {v['result']} ({v['level']}) |")
    if few_seeds:
        L.append("")
        L.append(f"Note: seeds={seeds} (<3), so the noise floor could not be measured (stddev needs "
                 f"≥ 3 samples) and is reported as \"unmeasured\" above; verdicts on this report "
                 f"may be less reliable than one run with ≥ 3 seeds.")
    if cmp["disagreements"]:
        L += ["", "## Self-report disagreements", "", "| board | candidate | seed | self unrouted | referee unrouted | self viol | referee viol |", "|---|---|---|---|---|---|---|"]
        for d in cmp["disagreements"]:
            L.append(f"| {d['board']} | {d['candidate']} | {d['seed']} | {d['self_unrouted']} | {d['referee_unrouted']} | {d['self_violations']} | {d['referee_violations']} |")
    if cmp["failures"]:
        L += ["", "## Failures", ""]
        L += [f"- {f['board']} / {f['candidate']} / seed {f['seed']}"
              f"{' [unjudged]' if f.get('unjudged') else ''}: {f['reason']}" for f in cmp["failures"]]
    if cmp["warnings"]:
        L += ["", "## Warnings", ""]
        L += [f"- {w}" for w in cmp["warnings"]]
    return "\n".join(L) + "\n"
