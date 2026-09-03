#!/usr/bin/env bash
# One-time setup for a remote NixOS host that will run `bench run` via scripts/remote-run.sh.
#
#   scripts/remote-setup.sh [HOST]
#
# HOST is optional if $BENCH_REMOTE_HOST is set (directly, or via .env -- see .env.example);
# a HOST given on the command line always wins over it.
#
# Creates ~/freerouting-bench-env/kicad-python on HOST: a small wrapper script that runs
# Nix's own python3 with PYTHONPATH pointed at KiCad's pcbnew module (extracted from the
# `kicad` wrapper script that KiCad itself installs), so bench's kicad referee can
# `import pcbnew` without needing KiCad's bundled Python distribution. On a headless host,
# KiCad's wx-based scripts (vendor/kicad/*.py, used by `bench corpus pcbench`/`kicad-fixtures`)
# need a display -- the wrapper falls back to `xvfb-run -a` whenever $DISPLAY is unset and
# `xvfb-run` is on PATH, so it works both on a host with a real/virtual display already
# (leaves it alone) and one with neither (fails with the normal wx error). Also verifies the
# pcbnew/wx import actually works, and pre-fetches the nix shell inputs scripts/remote-run.sh
# and scripts/remote-corpus.sh need (jdk25, uv, python312, xvfb-run) so the first real
# `bench run`/`bench corpus` isn't slowed down by a cold nix store fetch.
#
# Requirements on HOST: nix with flakes enabled, and kicad installed (providing both the
# `kicad` wrapper script and a nix-store python3 3.14.x env alongside it).
set -euo pipefail

HERE="$(cd "$(dirname "$0")/.." && pwd)"
cd "$HERE"
source "$HERE/scripts/lib/remote-env.sh"
# Load .env from the repo root if present -- see .env.example. A variable already set in the
# real environment always wins (see load_dotenv in scripts/lib/remote-env.sh), same as
# bench/paths.py's own loader.
load_dotenv "$HERE/.env"

if [[ $# -gt 0 && "$1" != -* ]]; then
  HOST="$1"; shift
else
  HOST="${BENCH_REMOTE_HOST:-}"
fi
if [[ -z "$HOST" ]]; then
  echo "error: no remote host given and \$BENCH_REMOTE_HOST is not set." >&2
  echo "usage: scripts/remote-setup.sh HOST" >&2
  echo "       (or set BENCH_REMOTE_HOST in .env -- copy .env.example to .env first)" >&2
  exit 1
fi

echo ">> setting up $HOST"
ssh "$HOST" bash -s <<'REMOTE'
set -euo pipefail

ENV_DIR="$HOME/freerouting-bench-env"
mkdir -p "$ENV_DIR"

KICAD="$(which kicad)"
if [[ -z "$KICAD" ]]; then
  echo "error: 'kicad' not found on PATH on this host" >&2
  exit 1
fi

PP=$(grep -o "PYTHONPATH=.[^ ]*" "$KICAD" | head -1 | sed "s/^PYTHONPATH=//; s/^.//; s/.$//")
if [[ -z "$PP" ]]; then
  echo "error: could not extract PYTHONPATH from $KICAD" >&2
  exit 1
fi

PY=$(ls -d /nix/store/*-python3-3.14.*-env/bin/python3 2>/dev/null | head -1)
if [[ -z "$PY" ]]; then
  echo "error: no /nix/store/*-python3-3.14.*-env/bin/python3 found on this host" >&2
  exit 1
fi

WRAPPER="$ENV_DIR/kicad-python"
cat > "$WRAPPER" <<WRAP
#!/usr/bin/env bash
export PYTHONPATH="$PP"
if [[ -z "\${DISPLAY:-}" ]] && command -v xvfb-run >/dev/null 2>&1; then exec xvfb-run -a "$PY" "\$@"; fi
exec "$PY" "\$@"
WRAP
chmod +x "$WRAPPER"
echo ">> wrote $WRAPPER (PYTHONPATH=$PP, python=$PY)"

echo ">> verifying pcbnew/wx import via $WRAPPER"
"$WRAPPER" -c "import pcbnew, wx; print('pcbnew OK:', pcbnew.GetBuildVersion())"

echo ">> pre-fetching nix shell inputs (jdk25, uv, python312, xvfb-run)"
# python312 satisfies pyproject.toml's requires-python and gives `uv run` a system
# interpreter to use -- without it, uv tries to download its own standalone CPython build,
# which is a dynamically-linked generic-linux binary that NixOS can't run (see
# https://nix.dev/permalink/stub-ld). scripts/remote-run.sh sets UV_PYTHON_DOWNLOADS=never
# to make sure it never tries. xvfb-run backs the kicad-python wrapper's headless fallback
# above, used by `bench corpus pcbench`/`kicad-fixtures` (scripts/remote-corpus.sh).
nix shell nixpkgs#jdk25 nixpkgs#uv nixpkgs#python312 nixpkgs#xvfb-run --command true

echo ">> remote-setup.sh done: $WRAPPER"
REMOTE
