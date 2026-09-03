# Freerouting Comparison Suite

A standalone benchmark suite that measures routing quality and speed of any number of *candidate* routers on a shared board corpus, scores every result with an independent referee, and reports differences between candidates with noise-aware verdicts.

Typical comparisons:
- Java freerouting (reference) vs. the Rust fork.
- Rust fork at commit A vs. commit B (evaluating a new routing algorithm).
- Either router at different settings (pass budget, timeout).

The suite treats routers as black-box CLIs and never imports code from either repository, so it survives internal churn in both.

## Setup

Install dependencies:
```bash
uv sync
```

Build the Java jar (in `../freerouting`):
```bash
cd ../freerouting
./gradlew executableJar
```

Download historical freerouting releases to `binaries/`:
```bash
gh release download --repo freerouting/freerouting --pattern 'freerouting-2.3.0.jar' --dir binaries
```

## Quick start

Build the jar and load the corpus (skip if already done):
```bash
(cd ../freerouting && ./gradlew executableJar -q)
uv run bench corpus init
```

Run a small, fast tier and score it against the Java referee:
```bash
uv run bench run --candidates java-current --tier canary --seeds 3 --max-passes 100 --timeout 120 --run-id baseline-canary
uv run bench compare --baseline java-current --against java-current --runs baseline-canary --out sanity
```

The second command compares `java-current` against itself as a sanity check — every board should
come back `tie`. On this machine it reports:
```
java-current vs java-current: same (0W/0L/4T)
```
with `reports/sanity.md` showing all four `canary`-tier boards (`dac2020-bm07`, `dac2020-bm08`,
`issue143-rpi_splitter_mod`, `issue558-dev-board`) as ties on every metric.

`reports/` is gitignored (its contents are machine- and timestamp-specific), so `bench compare`
output stays local by default. To share a specific report anyway, add it deliberately:
```bash
git add -f reports/sanity.json reports/sanity.md
```

Once a second candidate exists (e.g. the Rust fork, uncommented in `candidates.toml`), compare it
against the Java baseline the same way:
```bash
uv run bench run --candidates java-current,rs-main --tier canary --seeds 3 --max-passes 100 --timeout 120 --run-id fork-vs-java
uv run bench compare --baseline java-current --against rs-main --runs fork-vs-java --out fork-vs-java
```

## Reading the report

Each board/candidate pair gets a **verdict**: `win`, `loss`, or `tie`, decided by walking metrics
in a fixed priority order — `clean_pass_rate`, `unrouted`, `violations`, `score`, `wall_s`,
`peak_rss_mb` — and stopping at the first metric where the candidates differ by more than the
**noise band** (2× the stdev of that metric across the baseline's own seeds). The reported
`level` is whichever metric decided the verdict; if nothing clears the noise band the result is a
`tie` at the last level (`peak_rss_mb`). `clean_pass_rate`, `unrouted`, and `violations` are
"hard" metrics — the first three checked, in that order: a loss on any of them counts toward
`hard_losses`, and `overall.verdict` (`better`/`worse`/`same`) can only be `better` if there are
zero hard losses.

The noise band exists because wall-clock time is inherently jittery (JVM warm-up, OS scheduling)
and needs at least 3 seeds to estimate a stdev — with fewer seeds the noise floor is 0 and even
tiny timing differences will "win" or "lose" on `wall_s`. Fast fixtures (well under a second of
routing) are especially prone to this: their timing noise floor is dominated by process startup,
not the router, so a couple of them may resolve on `wall_s` where a slower board would tie. See
"Noise" below.

`disagreements` in the compare JSON (and the "Self-report disagreements" section in the Markdown)
flag cells where the candidate's own `result.json` and the referee's independent DRC pass counted
`unrouted`/`violations` differently *for the same `.ses`*. That is a referee-vs-self-report
counting difference, not a router quality issue — the scoring in `overall`/`verdict` always uses
the referee's numbers, never the candidate's self-report. A non-empty `failures` list means a
candidate crashed, timed out, or produced no output for that cell; those cells are still counted
against it but carry no metrics.

### Noise

Fixture timings from `../freerouting/docs/benchmarks.md`: `Issue558-dev-board` ~8 s,
`DAC2020 bm07` ~10 s, `bm08` ~1 s, `bm01` 5+ min. The end-to-end test (`tests/test_e2e.py`) runs
`issue143-rpi_splitter_mod` — a small, sub-second-routing board — through `java-current` twice
under two different candidate names with 3 seeds each, and asserts the two runs tie. In practice
the 3-seed noise band comfortably absorbs the wall-time jitter between the two runs even on this
fast a board (observed run: PASS in ~35s, well under the 2-minute budget), so no "noise-too-tight"
issue was observed here. If you see a fast board resolve a real Java-vs-Java comparison on
`wall_s` instead of tying, that means the board routes too quickly for JVM/process-start jitter to
average out over 3 seeds — increase `--seeds` or read that particular `wall_s` verdict with
skepticism rather than treating it as a genuine regression.

## Commands

```
bench corpus init                         # freerouting DSN fixtures → corpus/dsn, manifest entries
bench corpus pcbench --clone DIR [--boards N | --ids ...] [--jobs N] [--skip-existing/--force]
bench corpus kicad-fixtures [--fixtures DIR] [--jobs N] [--skip-existing/--force]  # ../freerouting/fixtures/*/*.kicad_pcb → corpus/kicad, manifest entries
bench corpus connections [--tier t] [--boards ids]  # measure Java's connection count per board (metrics.score's N)
bench corpus list [--tier t]
bench run --candidates a,b --tier t [--boards ...] --seeds N --max-passes N --timeout S [--threads N] [--jobs N] [--candidates-file PATH] [--run-id id] [--no-referee]
bench referee --run id [--only-missing] [--jobs N]   # (re)score outputs, e.g. after fixing the KiCad path; --jobs re-scores cells in parallel (needs only the manifest + run results, no candidates.toml)
bench compare --baseline a --against b[,c] [--runs ...] [--tier t] [--out name] [--allow-mixed] [--time-metric auto|wall|cpu]
bench report --compare id                 # regenerate .md/.html from a compare JSON
bench export --run id --candidate name [--out-dir exports]   # per-board metrics -> exports/<run>-<candidate>-<sha7>.{csv,json}
bench plot --files a.json[,b.json,...] [--out reports/plot.html]   # scatter/bar plots from bench export JSON files
```

`--boards` (on `run`, `corpus connections`) and `--ids` (on `corpus pcbench`) take a
comma-separated list of board ids; if any id itself contains a space, quote the whole
`--boards`/`--ids` value (e.g. `--boards "a board with spaces,other-board"`) so the shell
doesn't split it before `bench` sees it.

`bench run --no-referee` skips scoring entirely (only `time.json`/`out.ses`/`result.json` are
written); use `bench referee --run id` afterwards to score. `bench compare --allow-mixed` turns
an `IncompatibleRuns` error (mismatched `threads`/`jobs`/`max_passes`/`timeout_s` between the runs
being compared) into a warning in the report instead of refusing to compare.

`--candidates-file PATH` (or `$BENCH_CANDIDATES`) points `run` at a candidates TOML file other
than the default `candidates.toml`; relative `exec` paths inside it resolve relative to that
file's own directory. `candidates.remote.toml` is the file used for this: it points at jars
under `binaries/` instead of a `../freerouting` build directory, for hosts where that checkout
doesn't exist (see "Running on a remote NixOS host" below).

`bench corpus pcbench`/`kicad-fixtures --jobs N` (default 1) import up to `N` boards
concurrently -- each board is an independent KiCad subprocess pipeline (strip → export DSN →
DRC → stats) writing only into its own board directory, so there's nothing to coordinate
between them; the manifest is still read-merge-written from the main thread only as each
board finishes (same `bench.pool.run_cells` pattern as `run --jobs`), so an import killed
partway through is never left with a half-written `manifest.json`. `--skip-existing`
(default) skips boards whose manifest entry is already `status: ok` with both
`unrouted.dsn` and `ground_truth.json` on disk, so a resumed import continues where it left
off instead of re-running the whole (slow) KiCad pipeline for every board; pass `--force` to
re-import everything regardless. Both commands print one line per finished board (id,
`ok`/`excluded: reason`, elapsed seconds) as it completes -- with `--jobs > 1` these lines
may interleave across boards, since several may finish close together.

### Parallel cells (`--jobs`)

`bench run --jobs N` (default 1) runs up to `N` cells (candidate × board × seed) concurrently,
each in its own thread; every cell is still a separate subprocess with its own directory, so
`--jobs` just bounds how many run at once. Cell results are still recorded in the same
candidate → board → seed order regardless of which order they actually finish in, and
`meta.json` is still rewritten incrementally, so a run killed partway through `--jobs N` stays
just as comparable as a killed `--jobs 1` run.

**Quality metrics are unaffected by parallelism** — `unrouted`, `violations`, `score`, `vias`,
`wirelength_mm` come from routing a given cell in isolation, and `--jobs` only changes how many
of those isolated subprocesses run at the same time. **Timing is a different story**: with
`--jobs 1`, `wall_s` is a faithful measurement of how long the router took. With `--jobs > 1`,
several routers are contending for the same CPU cores at once, so `wall_s` inflates and stops
being comparable across configurations. For this reason:

- Use `--jobs 1` when you need official wall-time numbers (e.g. publishing a benchmark result).
- With `--jobs > 1`, `bench compare` automatically falls back from `wall_s` to `cpu_s` (each
  process's own user+sys CPU time, which parallel contention doesn't distort) for the level-5
  verdict metric and for `median_time_ratio`. This is the `--time-metric auto` default; pass
  `--time-metric wall` or `--time-metric cpu` to force one explicitly. The chosen metric is
  recorded in the compare JSON (`config.time_metric`) and stated in the Markdown/HTML report
  headers.
- `bench compare` refuses to compare runs with different `--jobs` values unless `--allow-mixed`
  is passed, for the same reason it refuses mismatched `--threads`.

A sensible default depends on available RAM: each Java cell uses roughly 1.5 GB RSS at peak
(`java -Xmx4g` reserves more than it typically touches, but budget for the ceiling). On a
10-core/16 GB laptop, `--jobs 4` leaves headroom for the OS and other work; on a 16-core/64 GB
workstation, `--jobs 8`–`12` uses the extra cores and memory without over-committing.

### Export and plots

`bench export --run id --candidate name` flattens one candidate's per-board `metrics.json`
files from a run into `exports/<run>-<candidate>-<sha7>.csv` and `.json` (`--out-dir` overrides
the `exports/` default). Unlike `results/` and `reports/*.{json,md,html}`, **`exports/` is
git-tracked** — it holds small, durable per-board summaries meant to be committed and diffed
across runs, not machine/timestamp-specific raw output. Every row is self-describing
(`candidate`, `sha` columns/fields), so exports for different candidates or runs can be
concatenated or read standalone without losing track of which router produced them. The `sha7`
filename suffix is the candidate's resolved git hash truncated to 7 characters (sanitised for
the filesystem); if the sha didn't resolve (`"unknown"`), the suffix is omitted and the command
prints a warning instead of guessing. A board with a `metrics.json` for that candidate in the
run but missing (or excluded) from the current `corpus/manifest.json` is skipped, with the
skipped count reported on stdout — the export always reflects the *current* corpus, not
whatever the run happened to include.

The CSV columns, in order: `candidate,sha,board,tier,nets,layers,clean_pass,unrouted,violations,
score,cpu_s,wall_s,peak_rss_mb,vias,wirelength_mm,wirelength_ratio,via_ratio,timed_out`.
`tier` is the board's `d3-*` tier if it has one, else its first tier. Booleans render as
`true`/`false`; missing values render as empty fields. The JSON file carries the same rows
(with proper types) plus `run`, `candidate` (`{name, sha, version}`), a top-level
`router_git_sha` (duplicate of `candidate.sha`, for quick access), `config` (the run's `args`),
`host`, `exported_at`, and `corpus_commit` (the short git hash of the last commit that touched
`corpus/manifest.json`, so a reader can tell which corpus revision the boards/tiers came from).

`bench plot --files a.json[,b.json,...] --out reports/plot.html` renders a single
self-contained HTML page (inline CSS, hand-drawn SVG, no external requests) comparing 1–N
`bench export` JSON files: a CPU-time-vs-score scatter (point radius ∝ √nets), nets-vs-peak-RSS
and nets-vs-CPU-time scatters (both log-log), a per-tier clean-pass/connected/zero-DRC bar
chart, and — when exactly two files share at least 10 board ids — paired log-log scatters
(cpu_s, peak_rss_mb) plus a linear score scatter, each with a y=x reference line and the median
ratio between the two candidates in the subtitle. The legend and page header identify each file
by `name @ sha7` plus its run id/config/host, matching `bench export`'s own filename scheme.

### Settings isolation

Freerouting (all versions) persists a `freerouting.json` in the user's config dir (Linux
`$XDG_CONFIG_HOME/freerouting`, default `~/.config/freerouting`; macOS `~/Library/Application
Support/freerouting`; Windows `%APPDATA%\freerouting`) and a matching data dir
(`$XDG_DATA_HOME/freerouting`). On startup it loads that file and copies its fields *over* the
built-in defaults, then re-saves it stamped with the running version. Left alone, this means
whichever router version (or candidate) ran last on a host leaks its settings — costs, ripup
costs, neckdown flags, etc. — into every later invocation of every *other* version, silently
invalidating cross-version comparisons.

To prevent that, every process this suite launches that can touch that file — a candidate
(`bench.runner.run_cell`) and the Java DRC referee (`bench.referee.java_drc.run`/
`measure_connections`) — is given its own private `HOME` (and `XDG_CONFIG_HOME`/
`XDG_DATA_HOME`/`XDG_CACHE_HOME`/`APPDATA`) pointed at a scratch `home/` directory under its
own results cell (or a temp dir, for `measure_connections`), so it never reads or writes the
real one. On macOS, Java's `user.home` follows `$HOME`, so overriding `HOME` alone isolates
that platform too; setting `$APPDATA` is harmless on non-Windows hosts.

Runs made before this isolation existed have no `isolated_config` flag in their cells'
`time.json`/`metrics.json` (`metrics.json` defaults it to `False`); `bench compare` adds a
warning — not a refusal — to the report when any compared cell isn't flagged
`isolated_config: true`, so stale results are visibly flagged rather than silently trusted.
Rerun with the current suite to clear the warning.

### Environment variables

| Variable | Overrides |
|---|---|
| `FREEROUTING_JAR` | path to the Java executable jar (default: `../freerouting/build/libs/freerouting-current-executable.jar`) |
| `FREEROUTING_JAVA` | the `java` executable (default: highest-version JDK under `~/.gradle/jdks`, else `java` on `$PATH`) |
| `FREEROUTING_KICAD_CLI` | `kicad-cli` (default: the macOS `KiCad.app` bundled copy) |
| `FREEROUTING_KICAD_PYTHON` | KiCad's bundled `python3` (default: the macOS `KiCad.app` bundled copy) |
| `FREEROUTING_TIME` | the `time` binary used to capture wall/CPU/RSS (default: `/usr/bin/time`) |

All of the above (plus `BENCH_CANDIDATES` and every `BENCH_REMOTE_*` variable used by the
remote scripts below) are documented with their defaults in `.env.example` — copy it to
`.env` and fill in whatever differs on your machine; every script and `bench/paths.py` load
`.env` from the repo root automatically if it exists, and a variable already set in your real
shell environment always wins over the same variable in `.env`. `.env` is gitignored, so
nothing host-specific needs to live in this repo.

### Running on a remote NixOS host

`--jobs` scales with cores, so a bigger box makes for faster/broader runs than a laptop.
`scripts/remote-setup.sh`, `scripts/remote-corpus.sh`, and `scripts/remote-run.sh` drive a
remote NixOS host over `ssh` without requiring the Java repo checkout (`../freerouting`) to
exist there — candidates come from `candidates.remote.toml` instead of `candidates.toml`,
pointing at jars under `binaries/`. `scripts/remote-run.sh` and `scripts/remote-corpus.sh`
share their nix-shell/env preamble via `scripts/lib/remote-env.sh`.

All three scripts take `HOST` as an optional first argument; if omitted, they fall back to
`$BENCH_REMOTE_HOST` (set it in `.env`, per above) and error out naming `.env` if neither is
given. An explicit `HOST` argument always overrides `$BENCH_REMOTE_HOST`. This suite's own
development/reference setup targets a NixOS host named `workbench` — the examples below use
`$BENCH_REMOTE_HOST` throughout so they work verbatim regardless of what you name yours.

Both scripts launch the remote job **detached** (`setsid nohup`, via
`remote-env.sh`'s `remote_launch_detached`) so a multi-hour run on the host survives this
script's own local process — or the ssh session that launched it — dying: nix/uv keep
running on the host regardless, and the job's combined stdout/stderr, PID, and (once it
finishes) exit code are recorded under `results/` on the host, not just held in an ssh
session's pipe. By default the script then polls the host every 60s (`--poll-interval N`)
until the job finishes, printing one progress line per poll, and pulls results back exactly
as before; `--no-wait` skips the polling and returns immediately after launching, and
`scripts/remote-run.sh HOST --attach RUN_ID` (re-)attaches to poll + pull an already-launched
run — see "Detached execution, `--no-wait`, and `--attach`" below for the full picture.

**Prerequisites on the remote host:**
- Nix with flakes enabled (`nix shell nixpkgs#jdk25 nixpkgs#uv nixpkgs#xvfb-run` must work).
- KiCad installed, providing both `kicad-cli` and the `kicad` wrapper script (used to locate
  `pcbnew`'s `PYTHONPATH` — only needed for the `kicad` referee, i.e. PCBench boards).
- No display: the host is expected to be headless. `bench corpus pcbench`/`kicad-fixtures`
  spawn KiCad's wx-based scripts (`vendor/kicad/*.py`), which need *some* display even to
  import `wx`; `scripts/remote-setup.sh` writes a `kicad-python` wrapper that transparently
  runs them under `xvfb-run -a` whenever `$DISPLAY` is unset, so nothing else on the host
  needs to know or care.
- `rsync` and GNU `time` (NixOS ships this at `/run/current-system/sw/bin/time`; macOS's
  built-in `time` doesn't support `-v` and won't work here).
- Passwordless (key-based) `ssh HOST` access from the machine driving the run.

**One-time setup**, once per host:
```bash
scripts/remote-setup.sh $BENCH_REMOTE_HOST
```
This creates `~/freerouting-bench-env/kicad-python` on the host — a wrapper around Nix's own
`python3` with `PYTHONPATH` pointed at KiCad's `pcbnew` module, falling back to `xvfb-run -a`
when `$DISPLAY` is unset — verifies `import pcbnew, wx` works through it, and pre-fetches the
`jdk25`/`uv`/`python312`/`xvfb-run` nix shell inputs so the first real run isn't slowed down
by a cold Nix store fetch.

**Importing the corpus on the host** (do this before the first `remote-run.sh`, and again to
pick up more boards or after a partial/interrupted import):
```bash
scripts/remote-corpus.sh $BENCH_REMOTE_HOST -- pcbench --clone '~/PCBench' --jobs 12
```
Note the *quoted* `'~/PCBench'` — see the comment at the top of `scripts/remote-corpus.sh` for
why: an unquoted `~/PCBench` is expanded by your own local shell against your local `$HOME`
before the script ever runs, not against the host's. PCBench is a several-GB checkout that
isn't kept locally; `--clone DIR` clones it into `DIR` on the host if it isn't already there
(on the reference host it's kept pre-cloned at `~/PCBench`, so this just reuses it). `--jobs N`
imports boards concurrently (see "Parallel PCBench/KiCad-fixture imports" above); the import
is resumable by default (`--skip-existing`), so an interrupted `remote-corpus.sh` run can just
be re-run to pick up where it left off. This rsyncs the suite to the host (excluding
`corpus/pcbench`/`corpus/kicad`, which live only on the host — see the exclude comments in
`scripts/remote-run.sh`/`scripts/remote-corpus.sh`), runs `bench corpus <args>` there, then
rsyncs `corpus/manifest.json` and every `corpus/pcbench/*/ground_truth.json` back locally so
`bench compare` can see the imported boards. The boards' actual KiCad/DSN files stay on the
host only — `bench run`'s kicad referee work happens there too, via `remote-run.sh`.

**Each run:**
```bash
scripts/remote-run.sh $BENCH_REMOTE_HOST --jobs 8 -- \
  --candidates java-current --tier canary --seeds 3 --max-passes 100 --timeout 120 \
  --run-id remote-canary
```
This copies the locally built jar (and the `../freerouting` commit it came from) into
`binaries/`, rsyncs the suite to the host (`--remote-dir` overrides the default
`~/freerouting-bench` directory; `corpus/pcbench`/`corpus/kicad` are excluded from this sync
since they're host-only, produced by `remote-corpus.sh` above — run that first), launches
`bench run` there inside a `nix shell`, **detached** so it survives this script (or your ssh
connection) dying, polls until it finishes, and rsyncs `results/<run-id>/` back — `bench
compare` then works on it exactly like a local run.

**Detached execution, `--no-wait`, and `--attach`.** The remote job is always launched via
`setsid nohup` on the host (see `scripts/lib/remote-env.sh`'s `remote_launch_detached`), so a
dropped ssh connection or a killed local `remote-run.sh`/`remote-corpus.sh` process does not
kill the `bench run`/`bench corpus` job on the host — only this *script's* polling/pulling
stops; the job itself keeps going under nix/uv. Three files under `results/` on the host
track it (`<job>` is the run-id for `remote-run.sh`, or `<subcommand>-<timestamp>` for
`remote-corpus.sh`, which has no run-id):
- `results/<job>.remote.log` — the job's combined stdout+stderr.
- `results/<job>.remote.pid` — the detached process's PID, used to check liveness.
- `results/<job>.remote.log.exit` — the job's exit code, written once it finishes (after
  `remote-run.sh`'s own `meta.json["status"]` flips to `"complete"`, so there's a brief
  window where the run is done but this file doesn't exist yet — the poll loop retries a
  few times before giving up on reporting an exit code).

By default, after launching, the script polls the host every 60s (override with
`--poll-interval N`) — one ssh round-trip per poll, checking whether the PID is still alive
and (for `remote-run.sh` only) whether `results/<run-id>/meta.json`'s `"status"` is
`"complete"` — and prints a one-line progress update each time, e.g.
`alive=1 status=incomplete cells=12/48` (`cells done/total` is parsed straight out of
`meta.json` with `grep`/`sed`/`awk`, no python dependency — see `remote_poll` in
`scripts/lib/remote-env.sh`). Once the job finishes (`meta.json` status `complete`, or the
PID is gone), it pulls results (or, for `remote-corpus.sh`, the manifest + ground truth)
exactly as a non-detached run would, and exits with the remote job's own exit code where
determinable.

`--no-wait` skips all of that: it launches, prints the PID/log paths, and exits 0
immediately without polling or pulling anything —
```bash
scripts/remote-run.sh $BENCH_REMOTE_HOST --jobs 8 --no-wait -- \
  --candidates java-current --tier canary --seeds 3 --max-passes 100 --timeout 120 \
  --run-id remote-canary
# >> launched: pid=12345  log=freerouting-bench/results/remote-canary.remote.log (on $BENCH_REMOTE_HOST)
# >> --no-wait: not polling. resume with: scripts/remote-run.sh $BENCH_REMOTE_HOST --attach remote-canary
```
`scripts/remote-run.sh HOST --attach RUN_ID` (only `remote-run.sh` has this — `bench corpus`
has no run-id to attach to; check its log/pid files directly, or just re-run
`remote-corpus.sh`, which is resumable via `--skip-existing` the same way it always was) skips
the rsync-up and launch entirely and just polls + pulls for an already-running (or already
finished) run-id — handy for reattaching after `--no-wait`, after a dropped connection, or
from an entirely different machine, as long as it can reach the host and `results/` locally.

Re-running `remote-run.sh` with a run-id that already has a live remote job (its
`results/<run-id>.remote.pid` on the host names a PID that's still alive, and
`results/<run-id>/meta.json` says `"incomplete"`) refuses to launch a second, competing job
under the same run-id — it prints the `--attach` command to use instead and exits 1. A
run-id whose remote job has already finished (`meta.json` status `complete`, or no live PID)
is still fine to relaunch, same as before this change.

**Timing is only comparable within a single host** — `wall_s`/`cpu_s` reflect that host's
CPU, not a portable number, so don't compare a laptop run's timing against a workstation
run's. Each run's `meta.json["host"]` records the hostname and architecture it ran on
(`platform.node()`/`platform.machine()`), so mixed-host result sets are at least traceable
even though `bench compare` doesn't currently warn about them the way it does for mismatched
`--jobs`/`--threads`.

**Admission criteria.** A reference board (`raw.kicad_pcb`) is admitted (`status: "ok"`) only
if it is *routing-DRC-clean under its own design rules* — zero errors of a routing-type
(`bench.referee.kicad.ROUTING_DRC_TYPES`: `clearance`, `track_width`, `via_diameter`,
`shorting_items`, ... — see "KiCad referee status" below) — **and** *fully connected* — zero
`unconnected_items` in its `raw-drc.json` — unless it has no routing at all (`wirelength_mm ==
0`), in which case it's admitted as an **unrouted reference**: every one of its nets is
unconnected by construction (that's expected for a bug-report fixture that was never actually
routed, not a broken reference), so the unconnected-items check doesn't apply to it, but it
carries no `wirelength_ratio`/`via_ratio` ground truth to compare a candidate against. A
routing-type DRC error excludes the board unconditionally, even if it has no routing — a
zero-routing board can still trip a genuine routing-type error unrelated to connectivity (e.g.
a zone/footprint clearance issue), and that must exclude it just as it would a routed board.
`ground_truth.json` records this as `"unconnected"` (the `raw-drc.json` count) and
`"reference_complete"` (`unconnected == 0 and wirelength_mm > 0` — true only for a routed,
fully-connected reference); the exclusion decision itself is `bench.corpus_pcbench
.reference_verdict(gt) -> (status, reason)`, used identically at import time and by `bench
corpus revalidate` (below).

`bench corpus revalidate --origin {pcbench,freerouting-kicad}` recomputes `drv_routing`/
`drv_all`/`unconnected`/`reference_complete` for every already-imported board of that origin
from what's on disk and re-applies `reference_verdict` to the manifest — without re-running
the full strip → DSN → DRC → stats import pipeline. By default it only touches boards that
already have a `raw-drc.json` (pure Python, no KiCad, safe to run alongside an in-progress
routing run on the same host); `--regenerate-projects` additionally rebuilds a project-
generated board's `.kicad_pro` from its pristine `raw.orig.kicad_pcb` backup (picking up a
`vendor/kicad/legacy_rules.py` fix, e.g. the setup-floor micrometre rounding below, without a
full re-import), and `--rerun-drc [--jobs N]` additionally re-runs `kicad-cli pcb drc` against
the (possibly just-regenerated) project and rewrites `raw-drc.json` before recomputing — both
need `kicad-cli`. It prints a summary of status changes (`ok` → `excluded`, grouped by reason;
`excluded` → `ok`).

`bench corpus pcbench` clones and imports the [PCBench](https://github.com/PCBench/PCBench)
corpus (1183 boards), which is not checked out in this repo's local development environment
(the checkout is several GB) — it's imported on a remote host instead, via
`scripts/remote-corpus.sh` above. `bench corpus kicad-fixtures` needs no external checkout —
it imports the Java repo's own `fixtures/*/*.kicad_pcb` (+ sibling `.kicad_pro`) directories
through the same strip → DSN → ground-truth pipeline, and has been run for real: of 17 boards
found across 16 fixture directories (`Issue593-BBD_Mars-64` contributes two), 11 import `ok`
and 6 are `excluded` because their reference (`raw.kicad_pcb`) has DRC errors under the
board's own project rules (`Issue069-TestSensel`, `Issue191-processor.Z80`,
`Issue230-CNH_Functional_Tester`, `Issue593-BBD_Mars-64` (both boards),
`Issue718-Allow_Net-Ties`). Several of the `ok` boards (e.g.
`Issue558-dev-board-autoroute-demo`) have an unrouted reference (0 vias, 0 wirelength) —
that's expected for a bug-report fixture that was never actually routed, and it isn't grounds
for exclusion (only nonzero DRC errors on a board that *has* routing are); it just means their
ground truth has no `wirelength_ratio`/`via_ratio` to compare against.

`bench corpus pcbench --jobs 6` has been exercised for real on the reference host (16 cores), via
`scripts/remote-corpus.sh $BENCH_REMOTE_HOST -- pcbench --clone '~/PCBench' --boards N --jobs 6`.
The KiCad pipeline itself (strip → export DSN → DRC → stats) takes ~3s/board regardless of
`--jobs` (confirmed identical DRC violation counts between `--jobs 1` and `--jobs 6` for the
same 12 boards), so `--jobs 6` gives close to a 6x wall-time speedup importing a batch. Of
the first 12 boards (alphabetically) 0/12 came back `ok` — all 12 excluded for nonzero
reference-board DRC errors (19–321 violations each); widening the sample to the first 60
boards only raised that to 3/60 (5%) `ok`. The reason: only 28 of PCBench's 1183 board
directories ship a `.kicad_pro` project file, so `bench corpus pcbench`'s reference-board DRC
check runs under KiCad's generic default design rules (not the board's own) for the other
~97.6% — which flags many violations irrelevant to any given board's actual manufacturing
constraints. This is a pre-existing property of the (previously unexercised against real
data) importer, not something `--jobs`/`--skip-existing` introduced; **the corpus's real
`ok` rate is likely closer to single digits than "most"** until the reference-DRC exclusion
criterion is revisited (e.g. relaxing it, or skipping DRC-based exclusion entirely for
project-less boards) -- out of scope here since it's an importer-design question, not a
parallel-import one.

**Follow-up: faithful design rules instead of KiCad's project-less defaults.** The above was
diagnosed further and addressed -- see "KiCad referee status" below for the generated-project /
zone-refill / routing-only-violation-counting fixes (`vendor/kicad/legacy_rules.py`,
`vendor/kicad/refill_zones.py`, `bench.referee.kicad.ROUTING_DRC_TYPES`) and the measured
before/after DRC-error counts.

**A first pass of this fix had an ordering bug: `refill_zones.py`'s `pcbnew` `board.Save()` can
silently (re)write a same-stem `.kicad_pro`/`.kicad_prl` using KiCad's own in-memory *default*
board-setup constraints.** Generating the project *before* refilling zones meant refill's `Save()`
call clobbered the freshly-generated, correctly-zeroed project with a fresh default one
(`min_hole_clearance: 0.25` etc.) -- so the reference DRC silently ran under KiCad's defaults
again, exactly the problem this fix exists to remove. It surfaced as 1Bitsy showing 35 "routing"
errors (`hole_clearance`/`copper_edge_clearance`/`starved_thermal`) instead of the expected 0.
Fixed by reordering: zone refill now runs *first*, before any project file exists on disk;
`refill_zones.py` also now cleans up any stray `.kicad_pro`/`.kicad_prl` it creates itself; and
the real/generated project is (re)written *after* refill, overwriting whatever pcbnew left
behind (`bench/corpus_pcbench.py::_import_board`, `bench/referee/kicad.py::run`). Because
`board.Save()` also stops serialising legacy `(net_class ...)` blocks as literal text once
refilled, the generated project is now built by parsing the *pre*-refill backup
(`raw.orig.kicad_pcb`), not the refilled board. Both places log (and, when the project is known
to be generated rather than the board's own, assert) that the written project's
`rules.min_hole_clearance` is 0, as a regression canary for this exact bug.

Real check on the reference host (`scripts/remote-corpus.sh $BENCH_REMOTE_HOST -- pcbench --clone ~/PCBench
--ids 1Bitsy_1bitsy,16x12-bits-I2C_I2C_Servo,4-port-usb-hub_4port-usb-hub --force --jobs 3`),
after the ordering fix: `1Bitsy` `ok` (46 total, all `courtyards_overlap`/`pth_inside_courtyard`,
0 routing -- matching the "254 -> 46" figure exactly); `16x12-bits-I2C` `ok` (26
`solder_mask_bridge`, 0 routing); `4-port-usb-hub` excluded (54 total: 52 `solder_mask_bridge` +
2 `clearance`, 2 routing errors). The generated `raw.kicad_pro`'s `board.design_settings.rules`
confirmed all-zero on the host after the fix (`min_hole_clearance: 0.0`, etc.) -- the earlier
1Bitsy discrepancy was this ordering bug, not a KiCad version effect.

## Referee

For `freerouting-fixtures` boards (DSN, no ground truth), scoring is done by the Java jar's
DRC-only mode, independent of whichever candidate produced the `.ses`:

```bash
java -jar freerouting-current-executable.jar -de in.dsn out.ses -drc report.json --gui.enabled=false
```

This works as-is (spec §15's open item is resolved, no argv changes were needed): the jar loads
the session against the DSN ("SES file loaded for DRC: N wires, M vias imported" in the log) and
writes a report with unrouted/violation counts. The report's keys are camelCase
(`unconnectedItems`, `violations`, `schematicParity`, `qualityScore`, `freeroutingVersion`);
`bench.referee.java_drc.parse_drc_report` accepts both that and the older snake_case spelling.
`--router.result_json` is not written in this mode, so via count and wirelength come from parsing
the candidate's own `.ses` file instead (`bench.referee.java_drc.parse_ses`).

### KiCad referee status

For `pcbench` boards (KiCad-sourced, no DSN ground truth of their own), scoring needs the
candidate's `.ses` routed back into the board's own KiCad file so `kicad-cli pcb drc` can check it.

**`pcbnew.ImportSpecctraSES` does not work headless on KiCad 10.0.3** (verified 2026-08-27, macOS,
KiCad 10.0.3, `kicad-cli` and the bundled `python3` at the paths in `bench/paths.py`). Every
attempt below returned `False` (or imported 0 tracks) with no error surfaced:

- `pcbnew.ImportSpecctraSES(board, path)` (two-arg form) — returns `False`.
- `pcbnew.ImportSpecctraSES(path)` (one-arg form) — returns `False`.
- Absolute paths for both the board and the `.ses` — no change.
- The stripped board copied so its filename stem matches the DSN's `(base_design ...)` name — no
  change.
- A `wx.App(False)` created before `LoadBoard`, with `wx.Log.EnableLogging(False)` — no change,
  and no wx assertion/log output pointing at a cause either.
- `kicad-cli pcb import` — this subcommand only imports *foreign PCB formats* (Eagle, Allegro,
  etc.) into a `.kicad_pcb`, not a Specctra session into an existing board; not applicable here.

Given that, the referee does not call `ImportSpecctraSES` at all. `vendor/kicad/ses_to_board.py`
is a small hand-rolled importer: it parses the SES's `(routes ...)` section itself (a ~150-line
recursive-descent S-expression parser, `parse_ses_routes`, with no pcbnew dependency and its own
unit tests in `tests/test_ses_to_board.py`) and adds `PCB_TRACK`/`PCB_VIA` objects to the loaded
board directly via the `pcbnew` API, then calls `board.Save`. `bench/referee/kicad.py` calls this
script instead of `import_specctra_ses.py`. The original `vendor/kicad/import_specctra_ses.py`
(and `export_specctra_dsn.py`) are still vendored per the original plan, as reference/for the DSN
export half of the pipeline, but `import_specctra_ses.py` itself is unused.

Coordinate mapping (confirmed against `fixtures/Issue558-dev-board-autoroute-demo/dev-board.kicad_pcb`
routed by freerouting, in `tests/data/spike/`): the SES's `(resolution <unit> <n>)` gives
nm-per-SES-unit = `{um: 1000, mm: 1e6, mil: 25400, inch: 25.4e6}[unit] / n`; board `x_nm = value *
nm_per_unit`, board `y_nm = -value * nm_per_unit` (KiCad's internal Y grows downward, Specctra's
grows upward). Verified end to end on the spike fixture (`uv run pytest tests/test_kicad_referee.py
-v -m slow`): all 35 vias imported, all 381 track segments on the correct layer (F.Cu/B.Cu), 0
skipped nets, `wirelength_mm` from `board_stats.py` agrees with a from-scratch length computed
directly off the parsed SES coordinates.

**`kicad-cli pcb drc` needs the board's own `.kicad_pro` project file, not just the `.kicad_pcb`,
to apply the board's real design rules.** Without one, KiCad falls back to its own default board
constraints (e.g. a 0.2mm minimum track width). The first pass of this referee ran DRC against a
bare `routed.kicad_pcb` with no project alongside it and got 51 `track_width` violations against
freerouting's own reported 2 clearance violations on the same route — not a coordinate bug (0
skipped nets/vias, exact via count, self-consistent wirelength) but a real DRC-engine difference:
39 of the SES's routed segments use a 0.15mm width (freerouting followed the DSN's per-net width
classes for tight-pitch escape routing near the ESP32-S3 module), and this particular board's
actual project sets `min_track_width: 0.0` (no floor at all) — it was only KiCad's *default*,
applied in the absence of a project file, that treated those as violations.

So `kicad.run` now propagates the project file: it resolves one from `board.kicad["project"]`,
falling back to a sibling of `board.kicad["raw"]` with suffix `.kicad_pro` if that exists, and (if
found) copies it — plus a sibling `.kicad_prl` if present — to `routed.kicad_pro`/`routed.kicad_prl`
next to `routed.kicad_pcb` before running DRC (`kicad-cli` picks up a same-stem, same-directory
project automatically). `referee.json` records this as `"project_used": true/false` so a manifest
entry missing a project file is visible rather than silently producing inflated violation counts.
With `tests/data/spike/stripped.kicad_pro` (copied from the dev board's real project) wired into
the slow test, the same route now reports **0 violations** under `kicad-cli pcb drc` — at or below
freerouting's own 2, confirming the 51 was purely the missing-project artifact, not a routing or
coordinate problem. If a PCBench board genuinely has no project file, the referee still runs (DRC
against KiCad's defaults rather than failing outright) but `project_used: false` flags that its
violation count may not reflect the board's actual constraints.

If `ses_to_board.py` or `kicad-cli` fails for a given board, `bench/referee/__init__.py`'s existing
fallback re-scores that cell with `java-drc` and tags the referee as `java-drc(fallback)`.

**Design-rule fidelity: generated projects, zone refill, and routing-only violation counting.**
Most PCBench boards are KiCad-4/5-era files with no `.kicad_pro` at all and zones saved with
KiCad's legacy fill strategy, which combine to make `kicad-cli pcb drc` badly unfaithful to the
board's actual design rules: (a) with no project, DRC falls back to KiCad's *default* board-setup
constraints (0.25mm hole clearance, 0.5mm copper-edge clearance, 0.2mm min track width) that these
designs never had; (b) a legacy zone fill isn't recognised as filled by KiCad 7+, so it reports a
`clearance` error per legacy-filled zone/pad interaction ("Legacy zone fill strategy is not
supported anymore") that is a stale-data artefact, not a real design problem; (c) DRC also reports
non-routing checks (`solder_mask_bridge`, `courtyards_overlap`, `pth_inside_courtyard`,
`invalid_outline`, ...) that no router touches. Together these made the reference-board exclusion
check (see the PCBench section below) far stricter than warranted — 95% of boards excluded, versus
57% for PCBench itself (which preserved each board's real design-rule settings).

Three fixes address this, all in `bench/corpus_pcbench.py::_import_board` (reference board) and
`bench/referee/kicad.py::run` (candidate-routed board):

1. **Generated project.** `vendor/kicad/legacy_rules.py` (pure Python, no `pcbnew`) parses the
   board's own legacy `(net_class NAME "desc" (clearance x) (trace_width y) (via_dia ...)
   (via_drill ...) (uvia_dia ...) (uvia_drill ...) (add_net "n") ...)` blocks and builds an
   equivalent `.kicad_pro` with one `net_settings.classes`/`netclass_patterns` entry per legacy
   class. It also parses the legacy `(setup ...)` block for board-wide DRC floors KiCad 4/5
   stored there (`trace_min`, `via_min_size`, `via_min_drill`, `uvia_min_size`,
   `uvia_min_drill`, and occasionally `clearance_min`/`trace_clearance`), mapping each onto the
   matching `rules.min_*` name (`parse_legacy_setup`). Legacy floors are carried over; rules
   legacy KiCad never had (hole clearance, hole-to-hole, copper-edge, annular width, silk, text,
   solder mask) are 0, since there is no legacy field that ever constrained them. Provenance --
   that the project is bench-generated, and which setup floors it carried over -- is recorded
   under a top-level `"_bench": {"generated": true, "setup_floors": {...}, "setup_floors_raw":
   {...}}` key; verified (2026-08-27, `kicad-cli` 10.0.3) that `kicad-cli pcb drc` ignores
   unknown top-level `.kicad_pro` keys, so this is safe to leave in the project used for DRC.
   Used only when the board ships no real `.kicad_pro`; `ground_truth.json` records
   `"project_generated": true/false`.

   **Setup floors are rounded DOWN to the nearest micrometre** (`legacy_rules._round_down_um`)
   before being written into `rules.min_*` — `"setup_floors"` is the rounded value actually
   used, `"setup_floors_raw"` the untouched parse. A legacy `(setup ...)` floor is frequently
   mil-derived and carries sub-micrometre precision (e.g. `0.3302`mm = 13 mil exactly), but
   `vendor/kicad/export_specctra_dsn.py` (via `pcbnew`) exports via/pad padstack sizes rounded
   to whole micrometres, so a routed board can never actually land on `0.3302`mm — only
   `0.330`mm or `0.331`mm. Left unrounded, re-scoring routed boards under such a floor produced
   thousands of `drill_out_of_range`/`via_diameter` violations of 0.2–0.8 µm each (e.g. "min
   diameter 0.6858 mm; actual 0.6850 mm") purely because the exporter's rounding put the
   candidate's own via a fraction of a micrometre under the (unrounded) legacy floor — not a
   real design-rule violation. Rounding down (not to nearest) means a routed via landing
   exactly on the exporter-rounded floor still clears the rule; rounding to nearest could
   instead round the floor *up* past what the exporter can ever produce, reintroducing the
   same problem from the other direction.
2. **Zone refill.** `vendor/kicad/refill_zones.py` (KiCad Python) loads a board and re-fills every
   zone with the modern `ZONE_FILLER` before DRC, clearing the legacy-fill-strategy noise. Runs on
   the reference board's `raw.kicad_pcb` in place (original kept as `raw.orig.kicad_pcb`) during
   import, and again on the candidate's `routed.kicad_pcb` before the referee's own DRC pass (so
   pours are checked against the actual routed tracks, not whatever the stripped board carried).
   Both record the zone count filled as `"zones_refilled"` (`ground_truth.json` / `referee.json`).
3. **Routing-only violation counting.** `bench/referee/kicad.py::ROUTING_DRC_TYPES` is the set of
   DRC violation types that reflect the routing itself (`clearance`, `track_width`,
   `hole_clearance`, `via_diameter`, `shorting_items`, `unconnected_items`, ...) as opposed to
   artwork/footprint/schematic-parity checks a router never touches (`silk_over_copper`,
   `courtyards_overlap`, `pth_inside_courtyard`, `solder_mask_bridge`, `invalid_outline`,
   `lib_footprint_*`, extra/missing/duplicate footprints, `net_conflict`, `schematic_parity`).
   `parse_kicad_drc` returns `violations` (routing-type errors only -- what `metrics.build` and
   the reference-board exclusion check use) alongside `violations_all` (every error) and
   `violations_by_type` (the unrestricted breakdown, for visibility). `ground_truth.json` records
   both as `"drv_routing"`/`"drv_all"`.

Measured effect (before -> after, on real PCBench boards on the reference host): `1Bitsy` 257 -> 46 DRC
errors (all 46 remaining are `courtyards_overlap`/`pth_inside_courtyard`, i.e. non-routing, so
`drv_routing` is 0 and the board is no longer excluded); `16x12-bits-I2C` 135 -> 26 (all
`solder_mask_bridge`, non-routing); `4-port-usb-hub` 84 -> 55 (52 `solder_mask_bridge` + 3
`clearance` -- the 3 routing-type errors are enough to keep it excluded under `drv_routing`). See
`scripts/remote-corpus.sh`'s real-check numbers in the PCBench section below.
