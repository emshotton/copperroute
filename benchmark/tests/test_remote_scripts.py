"""Shell scripts aren't executed under pytest (they need a real remote NixOS host -- see the
"Running on a remote NixOS host" section of README.md and the real-check notes in
scripts/remote-run.sh/scripts/remote-corpus.sh's own history). This just keeps them from
bit-rotting silently: `bash -n` catches syntax errors (unbalanced quotes/heredocs -- easy to
introduce when hand-splicing shell fragments across an ssh boundary, see
scripts/lib/remote-env.sh), and the `--help` checks catch a usage message regressing or a
flag getting silently dropped.
"""
import os
import shutil
import subprocess
from pathlib import Path

SCRIPTS_DIR = Path(__file__).parent.parent / "scripts"
SCRIPTS = ["remote-run.sh", "remote-corpus.sh", "remote-setup.sh"]


def _run(*args, **kwargs):
    return subprocess.run(args, capture_output=True, text=True, **kwargs)


def _clean_env():
    """A copy of the real environment with $BENCH_REMOTE_HOST stripped, so tests of the
    "no host given" path aren't accidentally satisfied by whatever the actual calling shell
    happens to have exported."""
    env = os.environ.copy()
    env.pop("BENCH_REMOTE_HOST", None)
    return env


def _isolated_scripts(tmp_path, env_contents=None):
    """Copy scripts/ (and scripts/lib/) into a fresh tmp dir with no repo root beside it --
    in particular, no .env. This repo's own real root now legitimately carries a gitignored
    .env with BENCH_REMOTE_HOST set (see .env.example), so exercising the "no host and no
    .env" error path against SCRIPTS_DIR directly would no longer be genuinely env-less;
    tests that need that must run against this isolated copy instead. Its `../<name>` is
    still `scripts/..` == the tmp dir itself, matching each script's own `$(dirname "$0")/..`
    HERE resolution, so `HERE/.env` correctly means `tmp_path/.env` -- write `env_contents`
    there to test .env loading, or leave it None for a genuinely env-less checkout."""
    dest = tmp_path / "scripts"
    shutil.copytree(SCRIPTS_DIR, dest)
    if env_contents is not None:
        (tmp_path / ".env").write_text(env_contents)
    return dest


def test_all_remote_scripts_are_syntactically_valid():
    for name in SCRIPTS:
        script = SCRIPTS_DIR / name
        assert script.is_file(), f"missing {script}"
        result = _run("bash", "-n", str(script))
        assert result.returncode == 0, f"bash -n {name} failed:\n{result.stderr}"


def test_remote_env_lib_is_syntactically_valid():
    lib = SCRIPTS_DIR / "lib" / "remote-env.sh"
    assert lib.is_file()
    result = _run("bash", "-n", str(lib))
    assert result.returncode == 0, f"bash -n remote-env.sh failed:\n{result.stderr}"


def test_remote_run_help_prints_usage_and_exits_zero():
    result = _run(str(SCRIPTS_DIR / "remote-run.sh"), "--help")
    assert result.returncode == 0
    out = result.stdout.lower()
    assert "usage" in out
    assert "--attach" in out
    assert "--no-wait" in out
    assert "--poll-interval" in out


def test_remote_run_dash_h_also_prints_usage():
    result = _run(str(SCRIPTS_DIR / "remote-run.sh"), "-h")
    assert result.returncode == 0
    assert "usage" in result.stdout.lower()


def test_remote_corpus_help_prints_usage_and_exits_zero():
    result = _run(str(SCRIPTS_DIR / "remote-corpus.sh"), "--help")
    assert result.returncode == 0
    out = result.stdout.lower()
    assert "usage" in out
    assert "--no-wait" in out
    assert "--poll-interval" in out


def test_remote_run_requires_host_argument(tmp_path):
    scripts = _isolated_scripts(tmp_path)
    result = _run(str(scripts / "remote-run.sh"), env=_clean_env())
    assert result.returncode != 0
    assert "usage" in result.stderr.lower()


def test_remote_corpus_requires_bench_corpus_args():
    result = _run(str(SCRIPTS_DIR / "remote-corpus.sh"), "somehost", "--")
    assert result.returncode != 0


# --- HOST optional / $BENCH_REMOTE_HOST / .env -----------------------------------------


def test_remote_run_errors_helpfully_with_no_host_and_no_env(tmp_path):
    scripts = _isolated_scripts(tmp_path)
    result = _run(str(scripts / "remote-run.sh"), env=_clean_env())
    assert result.returncode != 0
    assert ".env" in result.stderr


def test_remote_corpus_errors_helpfully_with_no_host_and_no_env(tmp_path):
    scripts = _isolated_scripts(tmp_path)
    result = _run(str(scripts / "remote-corpus.sh"), "--", "pcbench", "--clone", "x",
                  env=_clean_env())
    assert result.returncode != 0
    assert ".env" in result.stderr


def test_remote_setup_errors_helpfully_with_no_host_and_no_env(tmp_path):
    scripts = _isolated_scripts(tmp_path)
    result = _run(str(scripts / "remote-setup.sh"), env=_clean_env())
    assert result.returncode != 0
    assert ".env" in result.stderr


def test_remote_run_print_host_resolves_from_dotenv(tmp_path):
    """With no HOST argument, HOST comes from $BENCH_REMOTE_HOST -- here populated by a
    tmp .env next to the (copied) scripts/ dir, exactly as scripts/remote-run.sh's own
    `HERE/.env` loading would find it in a real checkout. This is the parse-only path
    requested for the real check: `--print-host` resolves and prints HOST without touching
    ssh/rsync at all."""
    scripts = _isolated_scripts(tmp_path, env_contents="BENCH_REMOTE_HOST=dotenv-host\n")
    result = _run(str(scripts / "remote-run.sh"), "--print-host", env=_clean_env())
    assert result.returncode == 0
    assert result.stdout.strip() == "dotenv-host"


def test_remote_run_print_host_cli_argument_overrides_dotenv(tmp_path):
    scripts = _isolated_scripts(tmp_path, env_contents="BENCH_REMOTE_HOST=dotenv-host\n")
    result = _run(str(scripts / "remote-run.sh"), "cli-host", "--print-host", env=_clean_env())
    assert result.returncode == 0
    assert result.stdout.strip() == "cli-host"


def test_remote_corpus_print_host_resolves_from_dotenv(tmp_path):
    scripts = _isolated_scripts(tmp_path, env_contents="BENCH_REMOTE_HOST=dotenv-host\n")
    result = _run(str(scripts / "remote-corpus.sh"), "--print-host", "--",
                  "pcbench", "--clone", "x", env=_clean_env())
    assert result.returncode == 0
    assert result.stdout.strip() == "dotenv-host"


def test_remote_run_print_host_real_env_beats_dotenv(tmp_path):
    """The .env loader (scripts/lib/remote-env.sh's `load_dotenv`) must not clobber a
    variable already set in the real environment -- real env beats .env, matching
    bench/paths.py's own precedence. With BENCH_REMOTE_HOST exported in the real
    environment, it wins over a conflicting value in .env; with it unset, .env's value is
    used."""
    scripts = _isolated_scripts(tmp_path, env_contents="BENCH_REMOTE_HOST=envfilehost\n")

    env_with_real = _clean_env()
    env_with_real["BENCH_REMOTE_HOST"] = "realhost"
    result = _run(str(scripts / "remote-run.sh"), "--print-host", env=env_with_real)
    assert result.returncode == 0
    assert result.stdout.strip() == "realhost"

    result = _run(str(scripts / "remote-run.sh"), "--print-host", env=_clean_env())
    assert result.returncode == 0
    assert result.stdout.strip() == "envfilehost"
