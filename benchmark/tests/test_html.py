import json
from pathlib import Path

from bench.report import html

DATA = Path(__file__).parent / "data"


def _cmp():
    return json.loads((DATA / "compare_sample.json").read_text())


def test_bar_svg_has_two_bars_per_label():
    svg = html.bar_svg([("a", 900.0, 990.0), ("b", 800.0, 700.0)])
    assert svg.startswith("<svg") and svg.count("<rect") == 4 and ">a<" in svg


def test_line_svg_draws_polyline_per_series():
    svg = html.line_svg({"rs": [("aaa", 0.5), ("bbb", 0.75)], "java": [("aaa", 0.8)]})
    assert svg.count("<polyline") == 2


def test_line_svg_honors_explicit_chronological_order():
    # Alphabetically "aaa" < "zzz", but order= says zzz came first.
    svg = html.line_svg({"rs": [("zzz", 0.5), ("aaa", 0.75)]}, order=["zzz", "aaa"])
    assert svg.index(">zzz<") < svg.index(">aaa<")


def test_line_svg_appends_unlisted_x_values():
    svg = html.line_svg({"rs": [("zzz", 0.5), ("aaa", 0.75)]}, order=["zzz"])
    assert svg.index(">zzz<") < svg.index(">aaa<")


def test_render_history_chart_uses_chronological_not_alphabetical_order():
    h1, h2 = _cmp(), _cmp()
    h1["created_at"] = "2026-01-01T00:00:00"
    h1["candidates"]["rs"]["sha"] = "zzz0000"
    h2["created_at"] = "2026-01-02T00:00:00"
    h2["candidates"]["rs"]["sha"] = "aaa0000"
    # history is passed already sorted by created_at (as load_history would produce)
    page = html.render(h2, history=[h1, h2])
    history_section = page.split("<h2>History", 1)[1]
    assert history_section.index(">zzz0000<") < history_section.index(">aaa0000<")


def test_render_is_self_contained():
    page = html.render(_cmp(), history=[_cmp()])
    body = page.split("<body")[1].split("</body>")[0].replace("https://schemas", "").replace("http://www.w3.org/2000/svg", "")
    assert "<script src" not in page and "http" not in body
    assert "java vs rs" in page and "win" in page and "<svg" in page


def test_bar_svg_handles_negative_values():
    svg = html.bar_svg([("a", -10.0, -50.0)])
    assert 'width="-' not in svg


def test_render_time_heading_follows_chosen_time_metric():
    cmp = _cmp()
    page = html.render(cmp, history=[])
    assert "Wall time per board (wall_s)" in page
    assert "CPU time per board" not in page

    cmp["config"]["time_metric"] = "cpu_s"
    cmp["boards"]["a"]["baseline"]["cpu_s"] = 9.0
    cmp["boards"]["a"]["against"]["rs"]["agg"]["cpu_s"] = 6.0
    page = html.render(cmp, history=[])
    assert "CPU time per board (cpu_s)" in page
    assert "Wall time per board" not in page


def test_render_escapes_untrusted_text():
    cmp = _cmp()
    cmp["failures"] = [{"board": "a", "candidate": "rs", "seed": 1, "reason": "<img src=x onerror=alert(1)>"}]
    cmp["candidates"]["rs"]["sha"] = "<b>x</b>"
    page = html.render(cmp, history=[])
    assert "<img src=x onerror=alert(1)>" not in page
    assert "<b>x</b>" not in page
    assert "&lt;img" in page


def test_chart_omits_incomparable_score_instead_of_drawing_zero(monkeypatch):
    cmp = _cmp()
    candidate = cmp["against"][0]
    row = cmp["boards"]["a"]["against"][candidate]
    row["score_comparable"] = False
    score = row["agg"]["score"]
    calls = []
    def capture(values):
        calls.append(values)
        return "<svg></svg>"
    monkeypatch.setattr(html, "bar_svg", capture)
    page = html.render(cmp, [])
    assert all(bid != "a" for bid, _, _ in calls[0])
    assert row["agg"]["score"] == score
    assert "omitted scores are not zero" in page
