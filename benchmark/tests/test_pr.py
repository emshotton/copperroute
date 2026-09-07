import copy
import json
from pathlib import Path

from bench import compare
from bench.report import markdown, pr

DATA = Path(__file__).parent / "data"


def _cmp():
    return json.loads((DATA / "compare_sample.json").read_text())


def _with_gate(cmp, **overall):
    cmp = copy.deepcopy(cmp)
    cmp["overall"]["rs"].update(overall)
    return cmp


def test_render_leads_with_the_verdict_and_a_passing_gate():
    md = pr.render(_cmp())
    assert md.startswith("## Benchmark: rs vs java")
    assert "**better** — the regression gate passes." in md


def test_render_names_every_gate_failure():
    cmp = _with_gate(_cmp(), quality_losses=2, performance_losses=1, performance_unmeasured=3)
    md = pr.render(cmp)
    assert "the regression gate **fails**" in md
    assert "- rs: 2 routing-quality losses" in md
    assert "- rs: 1 performance losses" in md
    assert "- rs: 3 boards with too few performance samples to judge" in md


def test_gate_falls_back_to_hard_losses_for_reports_without_quality_losses():
    cmp = _cmp()
    assert "quality_losses" not in cmp["overall"]["rs"]
    cmp["overall"]["rs"]["hard_losses"] = 4
    assert compare.gate_failures(cmp) == ["rs: 4 routing-quality losses"]


def test_gate_passes_on_a_clean_comparison():
    assert compare.gate_failures(_cmp()) == []


def test_require_complete_reports_excluded_boards():
    cmp = _cmp()
    cmp["coverage"] = {"rs": {"shared_boards": ["a", "b"], "compared_boards": 1,
                              "incomplete_boards": ["b"], "skipped_boards": {},
                              "baseline_only_boards": [], "candidate_only_boards": []}}
    assert compare.gate_failures(cmp) == []
    assert compare.gate_failures(cmp, require_complete=True) == [
        "rs: 1 incomplete and 0 skipped boards"]


def test_summary_table_carries_the_headline_numbers():
    md = pr.render(_cmp())
    assert "| Wins / losses / ties | 1 / 0 / 0 |" in md
    assert "| Clean-pass rate | 1.00 → 1.00 |" in md
    assert "| Median Δscore | +90.0 |" in md
    assert "| Median time ratio | 0.50 |" in md


def test_changed_boards_are_listed_and_ties_are_not():
    cmp = _cmp()
    cmp["boards"]["b"]["against"]["rs"] = {
        "agg": {"clean_pass_rate": 1.0}, "delta": {"score": 0.0},
        "verdict": {"result": "tie", "level": "score"}}
    md = pr.render(cmp)
    assert "<details><summary>1 board changed</summary>" in md
    assert "| a | win (score) |" in md
    assert "| b |" not in md


def test_board_rows_are_capped_with_a_remainder_line():
    cmp = _cmp()
    board = cmp["boards"]["a"]
    for i in range(pr.MAX_BOARD_ROWS + 5):
        cmp["boards"][f"x{i}"] = copy.deepcopy(board)
    md = pr.render(cmp)
    rows = [line for line in md.splitlines() if line.startswith("| x")]
    assert len(rows) < pr.MAX_BOARD_ROWS + 5
    assert "| … | and 6 more | | | | |" in md


def test_a_delta_that_rounds_to_zero_carries_no_sign():
    cmp = _cmp()
    cmp["boards"]["a"]["against"]["rs"]["delta"]["score"] = 0.04
    md = pr.render(cmp)
    assert "| 0.0 |" in md
    assert "+0.0" not in md


def test_identity_line_names_both_shas_and_the_settings():
    md = pr.render(_cmp())
    assert "Baseline first: java `aaa` · rs `bbb`" in md
    assert "threads=1 jobs=1 max_passes=100 timeout=300s seeds=3" in md
    assert "deciding on wall s" in md


def test_time_metric_follows_the_config():
    cmp = _cmp()
    cmp["config"]["time_metric"] = "cpu_s"
    cmp["boards"]["a"]["against"]["rs"]["delta"]["cpu_s"] = -2.0
    md = pr.render(cmp)
    assert "deciding on cpu s" in md
    assert "Δcpu s" in md


def test_low_seed_count_is_called_out_as_a_caveat():
    cmp = _cmp()
    cmp["config"]["seeds"] = 1
    md = pr.render(cmp)
    assert "seeds=1 (<3)" in md and "not evidence" in md

    cmp["config"]["seeds"] = 3
    assert "seeds=3 (<3)" not in pr.render(cmp)


def test_excluded_boards_and_warnings_appear_as_caveats():
    cmp = _cmp()
    cmp["coverage"] = {"rs": {"shared_boards": ["a", "b"], "compared_boards": 1,
                              "incomplete_boards": ["b"], "skipped_boards": {"c": "no referee"},
                              "baseline_only_boards": [], "candidate_only_boards": []}}
    cmp["failures"] = [{"board": "a", "candidate": "rs", "seed": 1, "reason": "no out.ses"}]
    md = pr.render(cmp)
    assert "| Boards compared | 1 of 2 shared |" in md
    assert "1 incomplete and 1 skipped boards are excluded" in md
    assert "1 failed cells" in md
    assert "- w1" in md


def test_performance_losses_are_labelled_advisory_until_a_tolerance_is_set():
    cmp = _with_gate(_cmp(), performance_losses=0)
    assert "| Performance losses | 0 (advisory) |" in pr.render(cmp)

    cmp["config"]["performance_regression_percent"] = 10
    assert "| Performance losses | 0 (gated above 10%) |" in pr.render(cmp)


def test_reproduction_footer_names_the_command_when_an_id_is_given():
    assert "pr-summary --compare run7" in pr.render(_cmp(), "run7")
    assert "pr-summary" not in pr.render(_cmp())


def test_summary_is_far_shorter_than_the_full_report():
    cmp = _cmp()
    board = cmp["boards"]["a"]
    for i in range(60):
        cmp["boards"][f"x{i}"] = copy.deepcopy(board)
    assert len(pr.render(cmp).splitlines()) < len(markdown.render(cmp).splitlines()) / 2
