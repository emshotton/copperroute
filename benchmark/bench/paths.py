"""Filesystem locations and external tool paths (env-overridable)."""
from __future__ import annotations

import os
import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CORPUS = ROOT / "corpus"
RESULTS = ROOT / "results"
REPORTS = ROOT / "reports"
VENDOR_KICAD = ROOT / "vendor" / "kicad"
JAVA_REPO = ROOT.parent.parent / "freerouting"
RUST_REPO = ROOT.parent
TEST_CORPUS = RUST_REPO / "tests" / "corpus" / "fixtures"


def _load_dotenv(path: Path) -> None:
    """Minimal `.env` loader (no new dependency): `KEY=VALUE` per line, blank lines and `#`
    comments ignored, optional matching quotes around VALUE stripped. Never overrides a
    variable already present in the real environment -- real env beats `.env`, and `.env`
    beats this module's own hardcoded defaults (see `_tool`/`java_exe` below, which only
    consult `os.environ` after this has run)."""
    if not path.is_file():
        return
    for line in path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, _, value = line.partition("=")
        key = key.strip()
        value = value.strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
            value = value[1:-1]
        if key:
            os.environ.setdefault(key, value)


_load_dotenv(ROOT / ".env")


class ToolMissing(RuntimeError):
    pass


def _tool(env: str, default: str, what: str) -> Path:
    p = Path(os.environ.get(env, default))
    if not p.exists():
        raise ToolMissing(f"{what} not found at {p} (override with ${env})")
    return p


def java_jar() -> Path:
    return _tool(
        "FREEROUTING_JAR",
        str(JAVA_REPO / "build" / "libs" / "freerouting-current-executable.jar"),
        "freerouting jar (build with `./gradlew executableJar` in the Java clone, sibling of this repo)",
    )


_JDK_MAJOR = re.compile(r"jdk-(\d+)")
_FIRST_NUMBER_AFTER_HYPHEN = re.compile(r"-(\d+)")


def _jdk_major_version(path: Path) -> int:
    """The JDK major version implied by a `.../bin/java` path, for picking the highest
    installed JDK. Prefers the number after a `jdk-` segment (e.g. `jdk-25.0.4.1+1` -> 25);
    falls back to the first integer after any `-` in the path. String-sorting paths breaks
    on version jumps past one digit (lexicographically "9" > "25"), so this parses the
    version out and compares numerically instead. Unparseable paths sort last (-1)."""
    m = _JDK_MAJOR.search(str(path))
    if m:
        return int(m.group(1))
    m = _FIRST_NUMBER_AFTER_HYPHEN.search(str(path))
    return int(m.group(1)) if m else -1


def java_exe() -> str:
    """The `java` to run freerouting with -- used to resolve a candidate's `{java}` exec
    placeholder. freerouting needs a JDK new enough to be uncertain the system `java` on
    $PATH qualifies, so this prefers the highest-version JDK Gradle already downloaded to
    `~/.gradle/jdks` (e.g. `eclipse_adoptium-25-aarch64-os_x.2/jdk-25.0.4.1+1`) over PATH."""
    if env := os.environ.get("FREEROUTING_JAVA"):
        return env
    gradle_jdk = Path.home() / ".gradle" / "jdks"
    if gradle_jdk.exists():
        matches = list(gradle_jdk.rglob("bin/java"))
        if matches:
            return str(max(matches, key=_jdk_major_version))
    return shutil.which("java") or "java"


def kicad_cli() -> Path:
    return _tool(
        "COPPERROUTE_KICAD_CLI",
        "/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli",
        "kicad-cli",
    )


def kicad_python() -> Path:
    return _tool(
        "COPPERROUTE_KICAD_PYTHON",
        "/Applications/KiCad/KiCad.app/Contents/Frameworks/Python.framework/Versions/Current/bin/python3",
        "KiCad bundled python3",
    )


def time_exe() -> Path:
    return _tool("COPPERROUTE_TIME", "/usr/bin/time", "GNU/BSD time")


def isolated_env(home: Path) -> dict[str, str]:
    """Env overrides that redirect freerouting's persisted `freerouting.json`/data dir
    under `home` instead of the real user config/data dir (Linux `$XDG_CONFIG_HOME`,
    macOS `~/Library/Application Support`, Windows `%APPDATA%`) -- freerouting loads that
    file, copies its fields OVER the built-in defaults, and re-saves it with the running
    version, so without this, whichever candidate/referee invocation saved last leaks its
    settings (costs, ripup costs, neckdown flags, ...) into every later invocation of every
    other version on the same host. On macOS, Java's `user.home` follows `$HOME`, so
    setting `HOME` alone isolates that platform too; `$APPDATA` is harmless to set on
    non-Windows hosts (nothing reads it there) and covers Windows the same way. Caller is
    responsible for creating `home` on disk before launching a process with this env."""
    return {
        "HOME": str(home),
        "XDG_CONFIG_HOME": str(home / ".config"),
        "XDG_DATA_HOME": str(home / ".local" / "share"),
        "XDG_CACHE_HOME": str(home / ".cache"),
        "APPDATA": str(home / "AppData" / "Roaming"),
        "FREEROUTING_ISOLATED_HOME": "1",
        # Java can initialize AWT on macOS even when freerouting disables its GUI.
        "JAVA_TOOL_OPTIONS": " ".join(filter(None, [
            os.environ.get("JAVA_TOOL_OPTIONS", ""),
            "-Djava.awt.headless=true", "-Dapple.awt.UIElement=true",
        ])),
    }
