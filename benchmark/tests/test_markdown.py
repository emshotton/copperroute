import json
from pathlib import Path

from bench.report import markdown

DATA = Path(__file__).parent / "data"


def _cmp():
    return json.loads((DATA / "compare_sample.json").read_text())


def test_render_contains_verdict_tables():
    cmp = _cmp()
    md = markdown.render(cmp)
    assert "# Comparison: java vs rs" in md
    assert "| all |" in md and "| a |" in md
    assert "**better**" in md and "win (score)" in md
    assert "w1" in md


def test_render_shows_unmeasured_noise_and_note_when_seeds_below_three():
    cmp = _cmp()
    cmp["config"]["seeds"] = 1
    cmp["boards"]["a"]["baseline"]["n"] = 1
    md = markdown.render(cmp)
    assert "| a | java-drc | java |" in md
    # the baseline row's noise column shows "unmeasured" instead of a number
    assert any(line.startswith("| a | java-drc | java |") and "unmeasured" in line
              for line in md.splitlines())
    assert "seeds=1 (<3)" in md


def test_render_shows_numeric_noise_when_seeds_at_least_three():
    cmp = _cmp()
    cmp["config"]["seeds"] = 3
    md = markdown.render(cmp)
    assert "unmeasured" not in md
    assert "seeds=1 (<3)" not in md


def test_render_states_time_metric_used():
    cmp = _cmp()
    md = markdown.render(cmp)
    assert "time metric = wall_s" in md

    cmp["config"]["time_metric"] = "cpu_s"
    cmp["boards"]["a"]["baseline"]["cpu_s"] = 9.0
    cmp["boards"]["a"]["against"]["rs"]["agg"]["cpu_s"] = 6.0
    cmp["boards"]["a"]["against"]["rs"]["delta"]["cpu_s"] = -3.0
    md = markdown.render(cmp)
    assert "time metric = cpu_s" in md
    assert "cpu s" in md


def test_render_flags_unjudged_failures():
    cmp = _cmp()
    cmp["failures"] = [
        {"board": "a", "candidate": "rs", "seed": 1, "reason": "referee crashed", "unjudged": True},
        {"board": "a", "candidate": "rs", "seed": 2, "reason": "no out.ses", "unjudged": False},
    ]
    md = markdown.render(cmp)
    lines = [ln for ln in md.splitlines() if "a / rs / seed" in ln]
    assert any("[unjudged]" in ln and "seed 1" in ln for ln in lines)
    assert not any("[unjudged]" in ln and "seed 2" in ln for ln in lines)


def test_noise_label_uses_actual_baseline_samples_with_mixed_run_counts():
    cmp = _cmp()
    cmp["config"]["seeds"] = None
    cmp["boards"]["a"]["baseline"]["n"] = 1
    md = markdown.render(cmp)
    assert any(line.startswith("| a | java-drc | java |") and "unmeasured" in line
               for line in md.splitlines())
