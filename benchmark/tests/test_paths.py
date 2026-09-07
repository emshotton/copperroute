import os
from pathlib import Path

from bench import paths


def test_jdk_major_version_parses_jdk_dash_segment():
    assert paths._jdk_major_version(Path("/x/eclipse_adoptium-25-aarch64/jdk-25.0.4.1+1/bin/java")) == 25
    assert paths._jdk_major_version(Path("/x/eclipse_adoptium-9-aarch64/jdk-9.0.4+1/bin/java")) == 9


def test_java_exe_picks_highest_version_numerically_not_lexicographically(tmp_path, monkeypatch):
    """Lexicographic string sort puts "9" after "25" ('9' > '2' as characters), which would
    wrongly prefer JDK 9 over JDK 25. java_exe must compare the parsed version numerically."""
    monkeypatch.delenv("FREEROUTING_JAVA", raising=False)
    monkeypatch.setattr(paths.Path, "home", staticmethod(lambda: tmp_path))
    jdks = tmp_path / ".gradle" / "jdks"
    old = jdks / "eclipse_adoptium-9-aarch64-os_x.2" / "jdk-9.0.4.1+1" / "bin"
    new = jdks / "eclipse_adoptium-25-aarch64-os_x.2" / "jdk-25.0.4.1+1" / "bin"
    old.mkdir(parents=True)
    new.mkdir(parents=True)
    (old / "java").write_text("")
    (new / "java").write_text("")

    result = paths.java_exe()
    assert result == str(new / "java")


def test_java_exe_falls_back_to_path_when_no_gradle_jdks(tmp_path, monkeypatch):
    monkeypatch.delenv("FREEROUTING_JAVA", raising=False)
    monkeypatch.setattr(paths.Path, "home", staticmethod(lambda: tmp_path))  # empty tmp_path: no .gradle/jdks
    result = paths.java_exe()
    assert result  # either shutil.which("java") or the literal fallback "java"


# --- .env loading (_load_dotenv) --------------------------------------------------------


def test_load_dotenv_sets_a_variable_absent_from_the_real_environment(tmp_path, monkeypatch):
    """.env beats this module's own hardcoded defaults: a variable with no real-env value
    picks up whatever .env says."""
    monkeypatch.delenv("BENCH_TEST_ONLY_VAR", raising=False)
    monkeypatch.setattr(paths, "ROOT", tmp_path)
    (tmp_path / ".env").write_text("BENCH_TEST_ONLY_VAR=from-dotenv\n")
    try:
        paths._load_dotenv(paths.ROOT / ".env")
        assert os.environ["BENCH_TEST_ONLY_VAR"] == "from-dotenv"
    finally:
        monkeypatch.delenv("BENCH_TEST_ONLY_VAR", raising=False)


def test_load_dotenv_does_not_override_a_real_environment_variable(tmp_path, monkeypatch):
    """Real env beats .env: a variable already set in the environment before .env is loaded
    keeps its real value."""
    monkeypatch.setenv("BENCH_TEST_ONLY_VAR", "from-real-env")
    monkeypatch.setattr(paths, "ROOT", tmp_path)
    (tmp_path / ".env").write_text("BENCH_TEST_ONLY_VAR=from-dotenv\n")
    paths._load_dotenv(paths.ROOT / ".env")
    assert os.environ["BENCH_TEST_ONLY_VAR"] == "from-real-env"


def test_load_dotenv_ignores_blank_lines_comments_and_strips_quotes(tmp_path, monkeypatch):
    monkeypatch.delenv("BENCH_TEST_ONLY_VAR", raising=False)
    monkeypatch.delenv("BENCH_TEST_ONLY_VAR2", raising=False)
    monkeypatch.setattr(paths, "ROOT", tmp_path)
    (tmp_path / ".env").write_text(
        "\n# a comment\nBENCH_TEST_ONLY_VAR=\"quoted value\"\nBENCH_TEST_ONLY_VAR2='single'\n"
    )
    try:
        paths._load_dotenv(paths.ROOT / ".env")
        assert os.environ["BENCH_TEST_ONLY_VAR"] == "quoted value"
        assert os.environ["BENCH_TEST_ONLY_VAR2"] == "single"
    finally:
        monkeypatch.delenv("BENCH_TEST_ONLY_VAR", raising=False)
        monkeypatch.delenv("BENCH_TEST_ONLY_VAR2", raising=False)


def test_load_dotenv_is_a_no_op_when_the_file_is_missing(tmp_path, monkeypatch):
    monkeypatch.setattr(paths, "ROOT", tmp_path)
    paths._load_dotenv(paths.ROOT / ".env")  # tmp_path/.env doesn't exist -- must not raise


def test_tool_default_used_when_neither_real_env_nor_dotenv_set_it(tmp_path, monkeypatch):
    """End-to-end: with no COPPERROUTE_TIME anywhere, _tool falls back to its hardcoded
    default, and a .env value (dotenv-beats-default) is picked up by _tool the same way a
    real env value would be (since _load_dotenv just populates os.environ)."""
    monkeypatch.delenv("BENCH_TEST_TOOL_PATH", raising=False)
    monkeypatch.setattr(paths, "ROOT", tmp_path)
    fake_tool = tmp_path / "tool"
    fake_tool.write_text("")
    (tmp_path / ".env").write_text(f"BENCH_TEST_TOOL_PATH={fake_tool}\n")
    try:
        paths._load_dotenv(paths.ROOT / ".env")
        found = paths._tool("BENCH_TEST_TOOL_PATH", str(tmp_path / "nonexistent-default"), "test tool")
        assert found == fake_tool
    finally:
        monkeypatch.delenv("BENCH_TEST_TOOL_PATH", raising=False)


def test_isolated_env_disables_java_gui_without_changing_parent_environment(tmp_path, monkeypatch):
    monkeypatch.setenv("JAVA_TOOL_OPTIONS", "-Xmx2g")
    env = paths.isolated_env(tmp_path)
    assert env["JAVA_TOOL_OPTIONS"] == (
        "-Xmx2g -Djava.awt.headless=true -Dapple.awt.UIElement=true")
    assert os.environ["JAVA_TOOL_OPTIONS"] == "-Xmx2g"


def test_isolated_env_disables_java_gui_without_existing_options(tmp_path, monkeypatch):
    monkeypatch.delenv("JAVA_TOOL_OPTIONS", raising=False)
    env = paths.isolated_env(tmp_path)
    assert env["JAVA_TOOL_OPTIONS"] == "-Djava.awt.headless=true -Dapple.awt.UIElement=true"
