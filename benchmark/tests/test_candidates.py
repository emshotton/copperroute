from pathlib import Path

import pytest

from bench.candidates import Candidate, load_candidates

TOML = """
[candidates.java-x]
kind = "java"
exec = ["java", "-jar", "{jar}"]
sha = "deadbeef"

[candidates.rs-x]
kind = "rust"
exec = ["{rs}"]
sha = "cafe0000"
extra_args = ["--router.seed={seed}"]
"""


@pytest.fixture
def toml_path(tmp_path: Path) -> Path:
    jar = tmp_path / "fr.jar"
    jar.write_text("")
    rs = tmp_path / "freerouting"
    rs.write_text("")
    p = tmp_path / "candidates.toml"
    # Use replace instead of format to avoid conflict with {seed} placeholder
    content = TOML.replace("{jar}", str(jar)).replace("{rs}", str(rs))
    p.write_text(content)
    return p


def test_load_candidates_resolves_exec_and_sha(toml_path):
    cands = load_candidates(toml_path)
    assert set(cands) == {"java-x", "rs-x"}
    assert cands["java-x"].kind == "java"
    assert cands["java-x"].sha == "deadbeef"
    assert cands["java-x"].exec[-1].endswith("fr.jar")


def test_argv_is_legacy_for_java_and_native_for_rust(toml_path):
    cands = load_candidates(toml_path)
    common = dict(in_dsn=Path("a.dsn"), out_ses=Path("a.ses"), result_json=Path("r.json"),
                  max_passes=100, timeout_s=300, threads=1, seed=2)
    j = cands["java-x"].argv(**common)
    assert j[3:] == ["-de", "a.dsn", "-do", "a.ses", "-mp", "100",
                     "--router.job_timeout=00:05:00", "--router.max_threads=1",
                     "--router.result_json=r.json", "--gui.enabled=false",
                     "--api_server.enabled=false", "--mcp_server.enabled=false"]
    r = cands["rs-x"].argv(**common)
    assert r[1:] == ["route", "a.dsn", "-o", "a.ses", "--max-passes", "100",
                     "--timeout", "00:05:00", "--result-json", "r.json",
                     "--set", "router.seed=2"]


def test_rust_extra_args_keep_the_dotted_spelling_in_the_table(toml_path):
    cands = load_candidates(toml_path)
    assert cands["rs-x"].extra_args == ["--router.seed={seed}"]


def test_missing_exec_fails_fast(tmp_path):
    p = tmp_path / "c.toml"
    p.write_text('[candidates.bad]\nkind="rust"\nexec=["/nonexistent/bin"]\nsha="x"\n')
    with pytest.raises(FileNotFoundError):
        load_candidates(p)


def test_sha_file_is_read_and_stripped(tmp_path):
    jar = tmp_path / "fr.jar"
    jar.write_text("")
    (tmp_path / "current.sha").write_text("abc123def456\n")
    p = tmp_path / "c.toml"
    p.write_text(f'[candidates.java-current]\nkind="java"\nexec=["java","-jar","{jar}"]\nsha_file="current.sha"\n')
    cands = load_candidates(p)
    assert cands["java-current"].sha == "abc123def456"


def test_sha_file_falls_back_to_unknown_when_missing(tmp_path):
    jar = tmp_path / "fr.jar"
    jar.write_text("")
    p = tmp_path / "c.toml"
    p.write_text(f'[candidates.java-current]\nkind="java"\nexec=["java","-jar","{jar}"]\nsha_file="does-not-exist.sha"\n')
    cands = load_candidates(p)
    assert cands["java-current"].sha == "unknown"


def test_timeout_formatting():
    assert Candidate.hms(3661) == "01:01:01"


def test_java_placeholder_is_resolved(tmp_path, monkeypatch):
    java_bin = tmp_path / "java"
    java_bin.write_text("")
    monkeypatch.setattr("bench.paths.java_exe", lambda: str(java_bin))

    jar = tmp_path / "test.jar"
    jar.write_text("")
    p = tmp_path / "cands.toml"
    p.write_text(f'[candidates.test]\nkind="java"\nexec=["{{java}}","jar","--","{jar}"]\nsha="x"\n')

    cands = load_candidates(p)
    assert cands["test"].exec[0] == str(java_bin)


def test_java_exe_prefers_env(monkeypatch):
    monkeypatch.setenv("FREEROUTING_JAVA", "/custom/java")

    from bench.paths import java_exe
    assert java_exe() == "/custom/java"
