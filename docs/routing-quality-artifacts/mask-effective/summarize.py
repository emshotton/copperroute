"""Summarize complete, KiCad-scored routing runs and verify saved-route identity."""

import argparse
import hashlib
import json
import pathlib
from collections import Counter


def load(root, run, name, manifest, expected):
    rows = {}
    for path in (root / "results" / run / name).glob("*/seed-1/metrics.json"):
        row = json.loads(path.read_text())
        board = row["board"]
        assert board not in rows, (name, board, "duplicate")
        assert not row.get("failed") and not row.get("unjudged"), str(path)
        assert row.get("referee") == "kicad", str(path)
        assert manifest[board]["referee"] == "kicad", board
        assert row["unrouted"] is not None and row["violations"] is not None, str(path)
        row["deadline"] = bool(row.get("timed_out")) or row.get("self", {}).get("final_state") == "TIMED_OUT"
        row["completed"] = not row["deadline"] and row.get("self", {}).get("final_state") == "COMPLETED"
        row["mask"] = row.get("violations_by_type", {}).get("solder_mask_bridge", 0)
        row["origin"] = manifest[board]["origin"]
        row["ses_sha256"] = hashlib.sha256((path.parent / "out.ses").read_bytes()).hexdigest()
        rows[board] = row
    assert set(rows) == expected, (name, "missing", sorted(expected - rows.keys()), "extra", sorted(rows.keys() - expected))
    return rows


def in_group(row, group):
    return group == "all" or (row["origin"] == "pcbench") == (group == "pcbench")


def totals(rows):
    rows = list(rows)
    return {
        "boards": len(rows),
        "unrouted": sum(r["unrouted"] for r in rows),
        "copper": sum(r["violations"] for r in rows),
        "mask": sum(r["mask"] for r in rows),
        "fully_connected": sum(r["unrouted"] == 0 for r in rows),
        "partially_connected": sum(r["unrouted"] > 0 for r in rows),
        "completed": sum(r["completed"] for r in rows),
        "deadline": sum(r["deadline"] for r in rows),
        "other_final_states": dict(Counter(r.get("self", {}).get("final_state") for r in rows if not r["completed"] and not r["deadline"])),
        "cpu_s": sum(r["cpu_s"] for r in rows),
        "max_rss_mb": max((r["peak_rss_mb"] for r in rows), default=0),
        "mean_peak_rss_mb": sum(r["peak_rss_mb"] for r in rows) / len(rows) if rows else None,
        "vias": sum(r["vias"] for r in rows),
        "wirelength_mm": sum(r["wirelength_mm"] for r in rows),
        "mask_at_or_above_cap": sum(r["mask"] >= 199 for r in rows),
    }


def compare(a, b, ids):
    cpu_a = sum(a[i]["cpu_s"] for i in ids)
    uncapped = [i for i in ids if a[i]["mask"] < 199 and b[i]["mask"] < 199]
    result = {
        "boards": len(ids),
        "improved": sum(b[i]["unrouted"] < a[i]["unrouted"] for i in ids),
        "regressed": sum(b[i]["unrouted"] > a[i]["unrouted"] for i in ids),
        "delta_unrouted": sum(b[i]["unrouted"] - a[i]["unrouted"] for i in ids),
        "copper_gainers": sum(b[i]["violations"] > a[i]["violations"] for i in ids),
        "delta_copper": sum(b[i]["violations"] - a[i]["violations"] for i in ids),
        "mask_gainers": sum(b[i]["mask"] > a[i]["mask"] for i in ids),
        "delta_mask": sum(b[i]["mask"] - a[i]["mask"] for i in ids),
        "mask_uncapped_boards": len(uncapped),
        "uncapped_mask_gainers": sum(b[i]["mask"] > a[i]["mask"] for i in uncapped),
        "uncapped_delta_mask": sum(b[i]["mask"] - a[i]["mask"] for i in uncapped),
        "same_ses_with_mask_difference": sum(b[i]["ses_sha256"] == a[i]["ses_sha256"] and b[i]["mask"] != a[i]["mask"] for i in ids),
        "same_ses_with_uncapped_mask_difference": sum(b[i]["ses_sha256"] == a[i]["ses_sha256"] and b[i]["mask"] != a[i]["mask"] for i in uncapped),
        "same_ses_with_copper_difference": sum(b[i]["ses_sha256"] == a[i]["ses_sha256"] and b[i]["violations"] != a[i]["violations"] for i in ids),
        "cpu_ratio": sum(b[i]["cpu_s"] for i in ids) / cpu_a if cpu_a else None,
        "identical_ses": sum(b[i]["ses_sha256"] == a[i]["ses_sha256"] for i in ids),
        "identical_referee_counts": sum(b[i]["unrouted"] == a[i]["unrouted"] and b[i]["violations"] == a[i]["violations"] and b[i].get("violations_by_type", {}) == a[i].get("violations_by_type", {}) for i in ids),
    }
    return result


def render(report):
    lines = ["# Routing validation", "", "KiCad only. Copper and mask findings are reported separately. A completed run may still have unrouted connections. Both-completed comparisons exclude external timeouts and require internal COMPLETED state on both sides. RSS is each board's process peak, not aggregate host memory. Mask reports at 199 findings may be capped. Single runs do not establish a speed improvement.", ""]
    for group in ["all", "pcbench", "local-kicad"]:
        lines += [f"## {group}", "", "| Candidate | Boards | Unrouted | Copper | Mask | Fully connected | Partial | Completed | Deadline | CPU s | Max RSS MiB |", "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|"]
        for name, groups in report["totals"].items():
            r = groups[group]
            values = [name, r["boards"], r["unrouted"], r["copper"], r["mask"], r["fully_connected"], r["partially_connected"], r["completed"], r["deadline"], f'{r["cpu_s"]:.2f}', f'{r["max_rss_mb"]:.1f}']
            lines.append("| " + " | ".join(map(str, values)) + " |")
        lines += ["", "| Baseline → candidate | Subset | Boards | U improved / worse | ΔU | Copper gainers / Δ | Mask gainers / Δ | Uncapped mask gainers / Δ (boards) | CPU ratio | Identical SES |", "|---|---|---:|---:|---:|---:|---:|---:|---:|---:|"]
        for r in report["comparisons"]:
            if r["group"] != group:
                continue
            ratio = f'{r["cpu_ratio"]:.4f}' if r["cpu_ratio"] is not None else "n/a"
            values = [f'{r["baseline"]} → {r["candidate"]}', "both completed" if r["completed_only"] else "all", r["boards"], f'{r["improved"]} / {r["regressed"]}', f'{r["delta_unrouted"]:+}', f'{r["copper_gainers"]} / {r["delta_copper"]:+}', f'{r["mask_gainers"]} / {r["delta_mask"]:+}', f'{r["uncapped_mask_gainers"]} / {r["uncapped_delta_mask"]:+} ({r["mask_uncapped_boards"]})', ratio, r["identical_ses"]]
            lines.append("| " + " | ".join(map(str, values)) + " |")
        lines.append("")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=pathlib.Path, required=True)
    parser.add_argument("--manifest", type=pathlib.Path, required=True)
    parser.add_argument("--boards", type=pathlib.Path, required=True)
    parser.add_argument("--spec", type=pathlib.Path, required=True, help="JSON list of {name, run}; first entry is baseline")
    parser.add_argument("--out", type=pathlib.Path, required=True)
    args = parser.parse_args()
    expected = set(args.boards.read_text().strip().split(","))
    assert len(expected) == 751, len(expected)
    manifest = {b["id"]: b for b in json.loads(args.manifest.read_text())["boards"]}
    assert Counter(manifest[i]["origin"] for i in expected) == {"pcbench": 740, "freerouting-kicad": 11}
    spec = json.loads(args.spec.read_text())
    assert len({s["name"] for s in spec}) == len(spec)
    data = {s["name"]: load(args.root, s["run"], s["name"], manifest, expected) for s in spec}
    report = {"spec": spec, "totals": {}, "comparisons": [], "regressions": {}}
    baseline = spec[0]["name"]
    for name, rows in data.items():
        report["totals"][name] = {group: totals(r for r in rows.values() if in_group(r, group)) for group in ["all", "pcbench", "local-kicad"]}
    for index, entry in enumerate(spec[1:], 1):
        name = entry["name"]
        for base in dict.fromkeys([baseline, spec[index - 1]["name"]]):
            a, b = data[base], data[name]
            for group in ["all", "pcbench", "local-kicad"]:
                for completed in [False, True]:
                    ids = [i for i in sorted(expected) if in_group(a[i], group) and (not completed or a[i]["completed"] and b[i]["completed"])]
                    report["comparisons"].append(dict(baseline=base, candidate=name, group=group, completed_only=completed, **compare(a, b, ids)))
            report["regressions"][f"{base}:{name}"] = [{"board": i, "baseline": a[i], "candidate": b[i]} for i in sorted(expected) if b[i]["unrouted"] > a[i]["unrouted"] or b[i]["violations"] > a[i]["violations"] or b[i]["mask"] > a[i]["mask"]]
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.with_suffix(".json").write_text(json.dumps(report, indent=2))
    args.out.with_suffix(".md").write_text(render(report))
    print(json.dumps({name: value["all"] for name, value in report["totals"].items()}, indent=2))


if __name__ == "__main__":
    main()
