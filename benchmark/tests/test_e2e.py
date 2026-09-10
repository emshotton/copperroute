import json

import pytest
from click.testing import CliRunner

from bench import cli, corpus, paths, runner


@pytest.mark.slow
def test_java_candidate_vs_itself_with_kicad_is_all_ties(tmp_path, monkeypatch):
    try:
        paths.java_jar()
    except paths.ToolMissing:
        pytest.skip("jar not built")
    if not corpus.manifest_path().exists():
        pytest.skip("run `bench corpus init` first")
    available = [b for b in corpus.load_manifest()
                 if b.referee == "kicad" and b.status == "ok" and b.path.exists()]
    if not available:
        pytest.skip("import a KiCad reference board first")
    board = min(available, key=lambda b: b.nets)
    monkeypatch.setattr(runner, "RESULTS", tmp_path / "results")
    monkeypatch.setattr(paths, "REPORTS", tmp_path / "reports")
    extra = (
        '[candidates.java-current]\n'
        'kind = "java"\n'
        'exec = ["{java}", "-Xmx4g", "-jar", "../freerouting/build/libs/freerouting-current-executable.jar"]\n'
        'sha_from = "../freerouting"\n'
        '\n'
        '[candidates.java-twin]\n'
        'kind="java"\n'
        'exec=["{java}","-Xmx4g","-jar","../freerouting/build/libs/freerouting-current-executable.jar"]\n'
        'sha_from="../freerouting"\n'
    )
    twin = tmp_path / "candidates.toml"
    twin.write_text(extra.replace('"../freerouting/', f'"{paths.JAVA_REPO}/'))
    monkeypatch.setattr(paths, "ROOT", tmp_path)
    r = CliRunner()
    res = r.invoke(cli.main, ["run", "--candidates", "java-current,java-twin", "--boards", board.id,
                              "--seeds", "3", "--max-passes", "20", "--timeout", "120", "--run-id", "e2e"])
    assert res.exit_code == 0, res.output
    meta = runner.load_meta(tmp_path / "results" / "e2e")
    assert meta["status"] == "complete" and all(c["status"] == "ok" for c in meta["cells"])
    res = r.invoke(cli.main, ["compare", "--baseline", "java-current", "--against", "java-twin", "--runs", "e2e", "--out", "e2e"])
    assert res.exit_code == 0, res.output
    cmp = json.loads((tmp_path / "reports" / "e2e.json").read_text())
    v = cmp["boards"][board.id]["against"]["java-twin"]["verdict"]
    assert v["result"] == "tie", v
    assert cmp["disagreements"] == [], cmp["disagreements"]
