from bench.report import plots


def _row(board, nets=10, cpu_s=5.0, score=900.0, peak_rss_mb=200.0, unrouted=0, violations=0,
        clean_pass=True, tier="d3-a", candidate="cand"):
    return {"board": board, "nets": nets, "layers": 2, "cpu_s": cpu_s, "wall_s": cpu_s,
            "score": score, "peak_rss_mb": peak_rss_mb, "unrouted": unrouted,
            "violations": violations, "clean_pass": clean_pass, "vias": 1, "wirelength_mm": 10.0,
            "wirelength_ratio": None, "via_ratio": None, "timed_out": False, "tier": tier,
            "candidate": candidate, "sha": "sha0000000"}


def _export(candidate="cand", sha="sha0000000", run="r1", rows=None):
    return {"schema_version": 1, "run": run,
            "candidate": {"name": candidate, "sha": sha, "version": "v1"},
            "router_git_sha": sha, "config": {"seeds": 1, "max_passes": 100, "timeout_s": 300,
                                              "jobs": 1, "tier": "d3-a"},
            "host": {"node": "host1", "machine": "x86_64"}, "exported_at": "2026-01-01T00:00:00+00:00",
            "corpus_commit": "abc1234", "rows": rows or [_row(f"b{i}") for i in range(5)]}


def test_log_ticks_1_2_5_progression():
    assert plots.log_ticks(1, 100) == [1, 2, 5, 10, 20, 50, 100]


def test_log_ticks_handles_non_positive_or_inverted_range():
    assert plots.log_ticks(0, 10) == []
    assert plots.log_ticks(10, 1) == []
    assert plots.log_ticks(None, 10) == []


def test_scatter_svg_has_one_circle_per_row_with_title():
    rows = [_row(f"board-{i}", nets=i + 1, cpu_s=float(i + 1), score=100.0 * i) for i in range(4)]
    svg = plots.scatter_svg(rows, "cpu_s", "score", x_log=True, y_log=False, radius_field="nets")
    assert svg.count("<circle") == 4
    for i in range(4):
        assert f">board-{i}<" in svg


def test_scatter_svg_skips_rows_missing_either_field():
    rows = [_row("a"), {**_row("b"), "cpu_s": None}]
    svg = plots.scatter_svg(rows, "cpu_s", "score")
    assert svg.count("<circle") == 1
    assert ">a<" in svg and ">b<" not in svg


def test_cand_label_uses_short_sha():
    e = _export(candidate="rs-main", sha="abcdef0123456789")
    assert plots.cand_label(e) == "rs-main @ abcdef0"


def test_cand_label_handles_unresolved_sha():
    e = _export(candidate="cand", sha="unknown")
    assert plots.cand_label(e) == "cand @ unknown"


def test_paired_mode_requires_at_least_ten_shared_boards():
    rows_a = [_row(f"b{i}") for i in range(9)]
    rows_b = [_row(f"b{i}") for i in range(9)]
    page = plots.render([_export(candidate="a", sha="sha000aaaa", rows=rows_a),
                         _export(candidate="b", sha="sha000bbbb", rows=rows_b)])
    assert "shared board" not in page


def test_paired_mode_activates_with_ten_shared_boards():
    rows_a = [_row(f"b{i}") for i in range(10)]
    rows_b = [_row(f"b{i}") for i in range(10)]
    page = plots.render([_export(candidate="a", sha="sha000aaaa", rows=rows_a),
                         _export(candidate="b", sha="sha000bbbb", rows=rows_b)])
    assert "10 shared board" in page
    # Subtitle states the ratio orientation explicitly using both candidate labels
    # (label_a / label_b), and n (all 10 pairs have a nonzero denominator here, so no
    # "of N" suffix is expected).
    assert "median a @ sha000a / b @ sha000b ratio 1" in page
    assert "n=10)" in page


def test_paired_render_handles_zero_values_clamped_and_excluded_from_ratio():
    rows_a = [_row(f"b{i}", cpu_s=5.0, peak_rss_mb=100.0) for i in range(10)]
    rows_b = [_row(f"b{i}", cpu_s=5.0, peak_rss_mb=100.0) for i in range(10)]
    # b0's candidate-b measurements are present but exactly zero: the point must still be
    # plotted (clamped onto the log axis) but dropped from the ratio median (nonzero
    # denominator only) for both log-scaled fields (cpu_s, peak_rss_mb); score is linear
    # and untouched, so its n stays the full 10.
    rows_b[0] = {**rows_b[0], "cpu_s": 0.0, "peak_rss_mb": 0.0}
    page = plots.render([_export(candidate="a", sha="sha000aaaa", rows=rows_a),
                         _export(candidate="b", sha="sha000bbbb", rows=rows_b)])
    assert page.count("n=9 of 10") == 2  # cpu_s and peak_rss_mb subtitles
    assert "n=10)" in page  # score subtitle: no exclusion
    assert page.count("<circle") > 0  # the zero-valued point still renders


def test_render_single_file_has_no_paired_section():
    page = plots.render([_export()])
    assert "shared board" not in page


def test_render_is_self_contained_no_script_or_external_http():
    page = plots.render([_export(candidate="a", sha="sha000aaaa"),
                         _export(candidate="b", sha="sha000bbbb",
                                 rows=[_row(f"b{i}") for i in range(5)])])
    assert "<script src" not in page
    body_wo_xmlns = page.replace("http://www.w3.org/2000/svg", "")
    assert "http" not in body_wo_xmlns


def test_render_shows_run_config_host_and_legend():
    page = plots.render([_export()])
    assert "r1" in page
    assert "host1" in page
    assert "cand @ sha0000" in page
