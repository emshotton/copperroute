# Copperroute Comparison Suite — Design Spec

**Date:** 2026-08-27
**Status:** Approved for planning
**Location:** `freerouting-bench/` (sibling of `freerouting/` (Java) and `copperroute/` (Rust fork))

## 1. Goal

A standalone benchmark suite that measures routing quality and speed of any
number of *candidate* routers on a shared board corpus, scores every result
with an independent referee, and reports differences between candidates with
noise-aware verdicts.

Typical comparisons:

- Java freerouting (reference) vs. the Rust fork.
- Rust fork at commit A vs. commit B (evaluating a new routing algorithm).
- Either router at different settings (pass budget, timeout).

The suite treats routers as black-box CLIs. It never imports code from either
repository, so it survives internal churn in both.

## 2. Non-goals

- Bit-parity testing between Java and Rust — that is `copperroute/tests/parity`.
- Autopilot / auto-commit loops — that is `copperroute/scripts/autopilot`.
- Reproducing PCBWorld's RL/LLM agents. Only its board splits and Clean Pass
  metric are adopted so numbers are comparable with the paper (arXiv 2607.05915).
- GUI. Multi-threaded router measurement is opt-in and reported separately.

## 3. Candidate contract

Both routers accept the Java legacy CLI flags. A candidate is invoked as:

```
<exec...> -de <in.dsn> -do <out.ses> \
  -mp <max_passes> \
  --router.job_timeout=<HH:MM:SS> \
  --router.max_threads=1 \
  --router.result_json=<result.json> \
  --gui.enabled=false --api_server.enabled=false --mcp_server.enabled=false \
  [extra args from candidates.toml]
```

The candidate must write `out.ses` and a `result.json` following the Java
`RoutingResultManifest` schema (schema_version 1): `app_version`, `git_sha`,
`fixture`, `settings_snapshot`, `phases.{fanout,autorouter,optimizer}.{duration_seconds,passes_completed}`,
`board_statistics`, `normalized_score`, `resource_usage`, `final_state`,
`exit_code`, `output_written`. Exit code 0 iff COMPLETED or TIMED_OUT with
output written.

The fork's legacy shim rewrites these flags to `freerouting route ...`; no
fork-specific code path exists in the suite.

`candidates.toml`:

```toml
[candidates.java-2.3]
kind = "java"
exec = ["java", "-Xmx4g", "-jar", "../freerouting/build/libs/freerouting-executable.jar"]
sha_from = "../freerouting"          # git rev-parse HEAD in that dir

[candidates.rs-main]
kind = "rust"
exec = ["../copperroute/target/release/copperroute"]
sha_from = "../copperroute"

[candidates.rs-negotiated]
kind = "rust"
exec = ["/path/to/another/build/copperroute"]
sha = "abc1234"                      # explicit when the binary is not a live checkout
extra_args = ["--router.algorithm=negotiated"]
```

`kind` affects only how version/sha are discovered; invocation is identical.
Unknown `exec` (missing file) fails fast before any board is routed.

## 4. Layout

```
freerouting-bench/
  pyproject.toml            # uv project, Python >=3.11; deps: click, jinja2, tomli (3.11 only)
  README.md
  candidates.toml
  bench/
    __init__.py
    cli.py                  # click entry point: corpus, run, referee, compare, report
    candidates.py           # load candidates.toml, resolve exec/sha/version
    corpus.py               # corpus/manifest.json model, tier selection
    runner.py               # execute candidate × board × seed, capture resources
    referee/
      __init__.py           # pick referee by board.referee
      kicad.py              # SES import + kicad-cli drc
      java_drc.py           # Java jar in DRC-only mode
    metrics.py              # Metrics record, score formula, clean_pass
    compare.py              # medians, noise floor, deltas, lexicographic verdict
    report/
      markdown.py
      html.py               # single-file dashboard (inline CSS/JS, no CDN)
      templates/dashboard.html.j2
  vendor/kicad/             # copied from copperroute/scripts/pcbench (KiCad-python scripts)
    strip_kicad_routing.py
    export_specctra_dsn.py
    import_specctra_ses.py
  corpus/
    manifest.json           # list of boards (see §5); committed
    dsn/                    # copies of copperroute/fixtures/*.dsn (gitignored; `corpus init` recreates)
    pcbench/<board>/        # raw.kicad_pcb, stripped.kicad_pcb, unrouted.dsn, ground_truth.json (gitignored)
  results/<run-id>/         # gitignored
    meta.json               # run args, candidates with resolved sha/version, start/end, status
    <candidate>/<board>/seed-<n>/
      in.dsn                # copy of the input actually routed
      out.ses
      result.json           # candidate self-report
      referee.json          # independent scoring
      metrics.json          # normalised Metrics record
      stdout.log, stderr.log
      time.json             # wall_s, cpu_s, peak_rss_mb, exit_code, timed_out
  reports/<compare-id>.{md,json,html}   # committed selectively by the user
  tests/
```

## 5. Corpus

`corpus/manifest.json` entries:

```json
{
  "id": "dac2020-bm01",
  "source": "dsn/Issue508-DAC2020_bm01.dsn",
  "origin": "freerouting-fixtures | pcbench",
  "referee": "java-drc | kicad",
  "tiers": ["hard", "dac2020"],
  "nets": 195,
  "layers": 2,
  "expected_duration_s": 300,
  "kicad": { "raw": "pcbench/<board>/raw.kicad_pcb", "stripped": "...", "ground_truth": "..." }
}
```

Sources:

1. **freerouting DSN fixtures** (105 boards from `../freerouting/fixtures/*.dsn`).
   Referee `java-drc`. `bench corpus init` copies them and fills `nets`/`layers`
   by running the Java jar with `-mp 0` once and reading `board_statistics`.
2. **PCBench** (KiCad boards, https://github.com/PCBench/PCBench).
   `bench corpus pcbench --clone <dir> [--boards N | --ids ...]`:
   strip routing → export unrouted DSN → export reference metrics + KiCad DRC of
   the original into `ground_truth.json` (`wirelength_mm`, `vias`, `drv_count`,
   `nets`). Referee `kicad`. Boards whose original already has DRVs, or that
   fail DSN export, are recorded with `"status": "excluded"` and a reason.

Tiers (a board may be in several):

| Tier | Membership rule |
|---|---|
| `canary` | expected duration ≤ 10 s with the Java reference at `-mp 100`; used for quick checks |
| `hard` | DAC2020 bm01/bm05/bm06, BBD_Mars-64, CNH_Functional_Tester |
| `dac2020` | all `Issue508-DAC2020_bm*.dsn` |
| `regression` | every freerouting fixture (issue reproductions) |
| `d3-a`, `d3-b`, `d3-c` | PCBench boards by net count 2–13 / 5–42 / 6–451 (PCBWorld splits; overlapping ranges are the paper's; a board is assigned to the smallest range that contains it) |

Tier membership is data in the manifest, not a filename convention.

## 6. Runner

`bench run --candidates a,b --tier canary [--boards id,...] --seeds 3 --max-passes 100 --timeout 300 [--threads 1] [--run-id name]`

For each candidate, board, seed (nested loop in that order so partial runs
contain complete candidate blocks):

1. Create `results/<run-id>/<candidate>/<board>/seed-<n>/`, copy the input DSN.
2. Invoke the candidate through `/usr/bin/time -l` (macOS) or `-v` (Linux)
   to capture wall clock, user+sys CPU, and peak RSS. Seed is passed as
   `--router.seed=<n>` if the candidate advertises support (`extra_args`
   template `{seed}`), else the seed only varies run repetition.
3. Enforce `timeout + 60 s` from the suite side with `subprocess.run(timeout=...)`;
   on expiry kill the process group and record `timed_out: true`.
4. Write `time.json`, `stdout.log`, `stderr.log`.
5. Run the referee immediately (see §7) and write `referee.json` and `metrics.json`.
6. Update `meta.json` status after every cell so a killed run is still comparable
   (`status: incomplete`, with the list of finished cells).

Default `--threads 1` for determinism. `--threads N` is allowed and stored in
`meta.json`; `compare` refuses to compare runs with different thread counts
unless `--allow-mixed-threads`.

Resolved candidate sha/version are recorded in `meta.json` at run start;
`compare` warns if two runs claim the same candidate name with different shas.

## 7. Referee

Purpose: score every output with the same judge regardless of which router
produced it, and detect self-report errors.

### 7.1 `java-drc` (freerouting DSN fixtures)

Invoke the Java jar (path from `[referee.java] exec` in `candidates.toml`) in
its DRC-only mode, which loads a board plus a session file and runs
`DesignRulesChecker` without routing (`Freerouting.java`, `-drc` path;
`GlobalSettings` accepts several files after `-de` and treats the `.ses` as
`designSessionFilename`):

```
-de in.dsn out.ses -drc referee-drc.json --gui.enabled=false
```

Read from the KiCad-schema DRC report: clearance violations (count, by type)
and unconnected items. Via count and total trace length (mm) are read from the
board statistics that the same invocation logs; if that proves unavailable in
DRC-only mode, the referee parses `out.ses` directly (via count = `(via ...)`
entries; length = sum of `(wire (path ...))` segment lengths scaled by the DSN
resolution). If the jar cannot load the SES, the referee result is
`{"status": "referee_failed", "reason": ...}` and the cell is excluded from
verdicts with a visible warning.

### 7.2 `kicad` (PCBench boards)

1. `vendor/kicad/import_specctra_ses.py stripped.kicad_pcb out.ses routed.kicad_pcb`
   using KiCad's bundled Python (`/Applications/KiCad/KiCad.app/Contents/Frameworks/Python.framework/Versions/Current/bin/python3`, overridable via `COPPERROUTE_KICAD_PYTHON`).
2. `kicad-cli pcb drc --format json --all-track-errors --units mm -o drc.json routed.kicad_pcb`
   (`kicad-cli` from `/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli`, overridable via `COPPERROUTE_KICAD_CLI`).
3. Read violations (by type), unconnected items, and compute via count and
   wirelength from `routed.kicad_pcb` via `pcbnew`.

**Known risk:** the Java repo's PoC found headless `pcbnew.ImportSpecctraSES`
returned false on KiCad 10.0.2 with no tracks imported. The first
implementation task is a bounded spike (≤ 2 h) on one PCBench board with
KiCad 10.0.3. Outcomes:

- Works → `kicad` referee as designed.
- Fails → PCBench boards are scored by `java-drc` with `referee.json.actual = "java-drc"`
  and the report's referee column shows the fallback. The `kicad` path remains
  a tracked TODO in the README; it does not block the suite.

Most PCBench boards ship no `.kicad_pro` and carry legacy zone fills, so both the reference
board's DRC (`bench/corpus_pcbench.py`) and the candidate's (`kicad.run`) first generate a
project from the board's own legacy `net_class` rules when none exists and re-fill zones with
`ZONE_FILLER` before DRC-ing; violation counts used for exclusion and scoring are then
restricted to routing-type DRC checks (`ROUTING_DRC_TYPES`) rather than every check DRC runs.
See README.md "KiCad referee status" for the measured before/after DRC-error counts.

### 7.3 Disagreement detection

`metrics.json` stores both `self.*` (from `result.json`) and `referee.*`.
`disagreement` is true when unrouted or violation counts differ. Verdicts use
referee numbers only; disagreements are listed in a dedicated report section.

## 8. Metrics

Per cell (`metrics.json`):

| Field | Source | Notes |
|---|---|---|
| `clean_pass` | referee | all connections complete AND zero violations (PCBWorld CP) |
| `unrouted` | referee | incomplete connections |
| `violations` | referee | clearance/DRC violations |
| `vias` | referee | |
| `wirelength_mm` | referee | |
| `score` | computed by suite | Java formula on referee numbers: `max(0, (N·P_u − unrouted·P_u − violations·P_v − length_mm·c_t − vias·c_v) / (N·P_u)) · 1000` with the Java defaults for `P_u, P_v, c_t, c_v` copied into `metrics.py` and versioned |
| `passes` | self | `phases.autorouter.passes_completed` |
| `wall_s`, `cpu_s`, `peak_rss_mb` | `time.json` | |
| `timed_out`, `exit_code` | runner | |
| `wirelength_ratio`, `via_ratio` | referee ÷ ground truth | PCBench boards only |
| `disagreement` | self vs referee | |

A cell with `referee_failed` or a crash (non-zero exit without output) has
`clean_pass=false`, `unrouted=N` and is flagged `failed`. **Ruling (2026-08-27):** when the
candidate produced no `out.ses` at all, that charge stands as above, against the candidate.
When the candidate *did* produce `out.ses` but the referee itself failed to score it, the
cell is additionally flagged `unjudged=true`; `compare` excludes `unjudged` cells from
aggregation (so a referee hiccup is never counted as a routing failure), and if every seed of
a (candidate, board) is unjudged the board's `against` entry for that candidate is `None`,
with the underlying per-seed failures still listed (and marked `unjudged`) in the report.

## 9. Comparison

`bench compare --baseline <cand> --against <cand>[,<cand>...] [--runs id,...] [--tier t] [--out name]`

1. Collect cells for the named candidates across the given runs (default: latest
   run containing each candidate).
2. Per (candidate, board): median over seeds of every numeric metric;
   `clean_pass_rate` = fraction of seeds with clean pass.
3. **Noise floor** per board = stddev over the baseline's seeds of each metric
   (requires ≥ 3 seeds; otherwise noise = 0 and the report says "unmeasured").
4. Per-board verdict for each `against` candidate, lexicographic:
   1. `clean_pass_rate` (higher wins)
   2. `unrouted` (lower)
   3. `violations` (lower)
   4. `score` (higher)
   5. `wall_s` (lower)
   6. `peak_rss_mb` (lower)

   At each level a difference within `2 × noise` counts as a tie and falls
   through to the next level. Result: `win | loss | tie` plus the deciding level.
5. Per-tier summary: win/loss/tie counts, mean and median deltas, and
   aggregate Clean Pass rate (boards with rate 1.0), so a tier row reads like a
   PCBWorld table row.
6. Overall verdict: `against` is "better" only if it has no losses on any
   board at levels 1–3 (hard metrics) and more wins than losses overall.
   "One board destroyed, average up" is therefore a loss.

Outputs `reports/<compare-id>.json` (all of the above), `.md` (tables), and
`.html` (dashboard).

## 10. Reports

**Markdown:** header (candidates, shas, versions, run ids, threads, pass budget,
timeout, seeds), overall verdict, per-tier summary table, per-board table
(baseline value, candidate value, delta, noise, verdict, referee, flags),
disagreements section, failures section.

**HTML dashboard:** one self-contained file (inline CSS/JS, no external
requests). Contents: summary cards per tier (Clean Pass rate, wins/losses,
median score delta, median time ratio); win/loss/tie matrix (boards × candidates);
bar charts of score and wall time per board; a history line chart of Clean Pass
rate and median score per candidate over all `reports/*.json` present, keyed
by sha, so progress over commits is visible. Charts drawn with a small
hand-written SVG helper — no charting library.

## 11. CLI summary

```
bench corpus init                         # freerouting DSN fixtures → corpus/dsn, manifest entries
bench corpus pcbench --clone DIR [--boards N | --ids ...]
bench corpus list [--tier t]
bench run --candidates a,b --tier t [--boards ...] --seeds N --max-passes N --timeout S [--threads N] [--run-id id]
bench referee --run id [--only-missing]   # (re)score outputs, e.g. after fixing the KiCad path
bench compare --baseline a --against b[,c] [--runs ...] [--tier t] [--out name]
bench report --compare id [--html|--md]   # regenerate from the compare JSON
```

## 12. Error handling

- Missing executable, jar, `kicad-cli`, or KiCad Python: fail at startup with
  the path that was tried and the env var that overrides it.
- Candidate crash/timeout: recorded per cell; run continues.
- Referee failure: recorded per cell; cell excluded from verdicts, listed in report.
- `compare` across incompatible runs (different threads, pass budget, or
  timeout): refuse unless `--allow-mixed`; always print both configurations.
- All subprocess invocations log the exact argv to the cell's log.

## 13. Testing

- **Unit:** `metrics.py` score formula against values copied from a Java
  `result.json`; `compare.py` verdicts and noise-tie logic on synthetic cells;
  manifest/tier selection; `candidates.py` template rendering.
- **Runner integration without routers:** a fake candidate
  (`tests/fake_router.py`) that copies a canned SES and manifest and can be
  told to crash, hang, or misreport; exercises timeouts, partial runs, and
  disagreement flags.
- **Referee integration:** `java-drc` on a recorded SES (marked `slow`,
  skipped when the jar is absent); `kicad` on one PCBench board (marked
  `slow`, skipped when KiCad or the board is absent).
- **End-to-end:** `bench run` with the Java jar on
  `Issue143-rpi_splitter_mod.dsn` (routes in ~1 s), then `compare` of the
  jar against itself must yield all ties.

## 14. Build order

1. Project skeleton, `candidates.toml`, `corpus init` for the DSN fixtures.
2. Runner + `time.json` capture + fake-router tests.
3. `java-drc` referee + metrics + score formula.
4. `compare` + Markdown report; first real Java-vs-Java run on `canary`.
5. KiCad SES-import spike; `kicad` referee or documented fallback.
6. PCBench corpus commands, tiers d3-a/b/c, ground truth.
7. HTML dashboard with history.

Steps 1–4 deliver a usable suite for Java-vs-Java (settings experiments) and
for the fork the moment its CLI routes a board.

## 15. Open items recorded, not blocking

- Whether the Java DRC-only invocation emits via/length statistics, or the
  referee must parse the SES (step 3 confirms; both paths are specified in §7.1).
- Whether the fork will expose `--router.seed`; until then seeds only repeat runs.
- Linux support for `time -v` parsing is designed in but only macOS is tested.
