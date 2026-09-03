import json

import pytest
from click.testing import CliRunner

from bench import cli, corpus, paths, runner


@pytest.mark.slow
def test_java_vs_itself_is_all_ties(tmp_path, monkeypatch):
    try:
        paths.java_jar()
    except paths.ToolMissing:
        pytest.skip("jar not built")
    if not corpus.manifest_path().exists():
        pytest.skip("run `bench corpus init` first")
    monkeypatch.setattr(runner, "RESULTS", tmp_path / "results")
    monkeypatch.setattr(paths, "REPORTS", tmp_path / "reports")
    # A minimal candidates.toml with java-current and a second candidate name
    # (java-twin) pointing at the same jar. We don't reuse the real
    # candidates.toml verbatim because it also carries a java-2.3.0 entry
    # with a `binaries/...jar` path relative to the real ROOT; load_candidates
    # validates every candidate in the file (not just the ones requested), so
    # that entry would fail to resolve once copied into tmp_path.
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
    res = r.invoke(cli.main, ["run", "--candidates", "java-current,java-twin", "--boards", "issue143-rpi_splitter_mod",
                              "--seeds", "3", "--max-passes", "20", "--timeout", "120", "--run-id", "e2e"])
    assert res.exit_code == 0, res.output
    meta = runner.load_meta(tmp_path / "results" / "e2e")
    assert meta["status"] == "complete" and all(c["status"] == "ok" for c in meta["cells"])
    res = r.invoke(cli.main, ["compare", "--baseline", "java-current", "--against", "java-twin", "--runs", "e2e", "--out", "e2e"])
    assert res.exit_code == 0, res.output
    cmp = json.loads((tmp_path / "reports" / "e2e.json").read_text())
    v = cmp["boards"]["issue143-rpi_splitter_mod"]["against"]["java-twin"]["verdict"]
    assert v["result"] == "tie", v
    assert cmp["disagreements"] == [], cmp["disagreements"]
