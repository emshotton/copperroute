# Routing benchmarks

This suite answers two questions:

1. How does freerouting-rs compare with the original Java freerouting router?
2. Does a proposed change improve routing relative to a pinned HEAD of this repository,
   without regressions on individual boards?

Candidates run as separate CLI processes on the same board inputs. A shared referee scores
connectivity, routing violations, wirelength and vias; the suite also measures process wall
time, CPU time and peak RSS. Use the same suite checkout, corpus, referee and machine for
both sides of a comparison.

## Setup

Run suite commands from `benchmark/`:

```bash
cd benchmark
uv sync
```

Build the Rust release binary and the Java reference from their respective checkouts:

```bash
cargo build --release --locked --manifest-path ../Cargo.toml
(cd ../../freerouting && ./gradlew executableJar)
uv run bench corpus init
uv run bench corpus list --tier canary
```

The Java checkout defaults to `freerouting/` alongside the Rust repository, **two levels up
from `benchmark/`**. It must provide `build/libs/freerouting-current-executable.jar`, including
DRC-only mode. The Java candidate and the Java referee are separate roles: keep the referee
fixed when comparing routers or revisions. A downloaded upstream release may be a candidate
without supporting the referee's DRC interface.

Copy [.env.example](.env.example) to `.env` for machine-specific tool paths. Python and the
remote scripts load `benchmark/.env`; shell environment values take precedence. `{java}` in
candidate commands uses `FREEROUTING_JAVA`, otherwise a Gradle-downloaded JDK or `java` on PATH.
`FREEROUTING_JAR` selects the default **referee** jar; it does not rewrite candidate commands.

The manifest is tracked, but board input files, binaries and raw results are gitignored.
A fresh worktree needs corpus import or a copy of the corresponding inputs before routing.
Historical jars are optional: only candidates named by `--candidates` are loaded and checked.

## Compare Rust with Java freerouting

The default [candidates.toml](candidates.toml) provides `java-current` from the sibling Java
checkout and `rs-main` from this checkout's `target/release/freerouting`. `rs-main` is a label:
it runs the binary you built, not an automatic checkout of `main`.

```bash
uv run bench run --candidates java-current,rs-main --tier canary \
  --seeds 3 --threads 1 --jobs 1 --max-passes 100 --timeout 120 --run-id java-vs-rust
uv run bench compare --baseline java-current --against rs-main \
  --runs java-vs-rust --out java-vs-rust
```

Start with canary boards to check the pipeline, then repeat on a broader shared tier such as
`pcbench` with the same settings for both routers. Use a new run ID each time. Existing run
directories are never reused, so old sessions or metrics cannot leak into new measurements.

Record which Java revision or release you are comparing against. The candidate table also
contains historical releases and experimental Java variants; those are optional investigations,
not the default performance baseline. `sha_from` reads the checkout's HEAD at run time; it
cannot prove an existing binary was built from that commit. Build from a clean, pinned checkout
and keep it unchanged during the run. For a copied binary, set `sha` or `sha_file` explicitly.

[candidates.rs.toml](candidates.rs.toml) provides just Rust and a copied Java jar under
`binaries/`. [candidates.remote.toml](candidates.remote.toml) adds optional Java variants for
remote hosts. Both read the copied Java commit from `binaries/freerouting-current.sha`.

## Compare a change with current HEAD

Freeze HEAD **before starting the change**. If work has already started, choose its intended
baseline commit explicitly (usually the current `origin/main`). Run both binaries from one
suite checkout so they share the same corpus and scoring code.

From the Rust repository root, create and build a separate baseline worktree:

```bash
git worktree add --detach ../benchmark-head HEAD
cargo build --release --locked --manifest-path ../benchmark-head/Cargo.toml \
  --target-dir ../benchmark-head/target
```

Make and commit the proposed change in your development branch, then build it:

```bash
cargo build --release --locked --target-dir target
cd benchmark
```

Create `candidates.local.toml` in `benchmark/`:

```toml
[candidates.rs-head]
kind = "rust"
exec = ["../../benchmark-head/target/release/freerouting"]
sha_from = "../../benchmark-head"

[candidates.rs-change]
kind = "rust"
exec = ["../target/release/freerouting"]
sha_from = ".."
```

Use distinct candidate names and separate target directories. Do not rebuild one candidate's
binary with the other revision before both runs finish. Relative paths resolve against the
candidate TOML file's directory.

```bash
uv run bench run --candidates-file candidates.local.toml --candidates rs-head,rs-change \
  --tier canary --seeds 3 --threads 1 --jobs 1 --max-passes 100 --timeout 120 \
  --run-id head-vs-change
uv run bench compare --baseline rs-head --against rs-change --runs head-vs-change \
  --out head-vs-change --fail-on-regression
```

`--fail-on-regression` writes the reports first, then exits nonzero for routing-quality losses
(clean-pass rate, unrouted connections, violations or score), or if no boards can be compared.
Timing and RSS verdicts remain advisory by default. To gate those too, add
`--performance-regression-percent 10`: each side needs at least three measured repetitions,
and a loss must exceed both the specified relative margin and three times the root-sum-square
of the two samples' standard deviations. This is a conservative noise allowance, not a
statistical significance guarantee; repeat borderline results under controlled conditions.

Comparisons use the intersection of the candidates' selected boards. Missing repetitions,
unjudged outputs and incompatible referees are reported explicitly; available comparable
measurements still contribute. Add `--require-complete` alongside `--fail-on-regression` when
every shared board and its planned repetitions must be present. A replacement run can complete
an interrupted plan under a new run ID; repeated plans are not added together as new obligations.
Boards selected by only one side are excluded and listed in coverage.

You can also compare separate run IDs with `--runs baseline-run,change-run`. Keep the candidate
names distinct across revisions. Never combine multiple commits under `rs-main`: the suite
rejects that ambiguity. Explicit run IDs avoid accidentally selecting an unrelated latest run.

## Reading results

`reports/<name>.json`, `.md` and `.html` contain candidate identities, settings, per-board
metrics, failures, warnings and aggregate verdicts. Per-board verdicts check, in order:

| Metric | Better direction |
|---|---|
| Clean-pass rate | Higher; connected with zero routing violations |
| Unrouted connections | Lower |
| Routing violations | Lower |
| Normalized routing score | Higher |
| Wall time, or CPU time for concurrent runs | Lower |
| Peak RSS | Lower |

The first difference outside twice the baseline's sample standard deviation decides the
verdict. The first three metrics are hard quality metrics; a loss on one prevents an overall
“better” verdict. `same` means differences did not decide a net win or loss under this rule;
it is not proof of equivalence. Per-board rows remain essential.

`--seeds` defaults to 1 **repetition**; use `--seeds 3` or more to estimate noise. By default no random-seed flag is sent to either router;
`{seed}` is available in candidate `extra_args` for a router that supports one. Below three
baseline measurements the suite uses a zero noise band. This is a noise heuristic, not a significance
test; short routes can be dominated by process/JVM startup. Repeating both candidates under
quiet, comparable conditions is necessary for timing claims.

Coverage is shown near the top of each report. A candidate with no comparable shared boards
gets `inconclusive`; otherwise aggregate verdicts describe the comparable subset. A candidate
that produces no session is scored as a routing failure. An output the referee cannot judge is
excluded from routing aggregates and listed in failures. `--no-referee` runs require
`bench referee` before comparison. The JSON `coverage` field includes shared, unmatched,
incomplete and skipped boards, with reasons for skipped comparisons.

Comparisons reject different thread counts, job counts, pass limits or timeouts unless
`--allow-mixed` is passed for exploratory analysis. Host differences produce a warning: network
hostnames can change on the same machine, so verify the physical hardware yourself. Repetition
count differences are reported, and each board's noise estimate uses its actual baseline
sample count, regardless of `--runs` order. Different commits/settings under one candidate
name remain an error: they cannot be pooled into one identity.

Cells judged by different referees or known Java referee jars are excluded for that board and
candidate pair, without discarding other boards. Re-score those outputs with one referee to
include them. Different score versions omit the score comparison but retain other metrics.

For current-version metrics, comparison recomputes scores in memory using one shared denominator
per board: its manifest connection count, falling back to its manifest net count. Candidate
self-reported counts and missing `result.json` files therefore cannot change the denominator
between candidates. Candidate failures are charged the same shared connection count. Raw
metrics and exports retain their original scores; the compare JSON records `config.score_basis`
and each board's `score_n`. `bench corpus connections` measures manifest connection counts
with the Java referee. Keep the manifest fixed across comparisons and inspect connectivity,
violations, length and vias alongside the normalized score.

### Timing and parallelism

Use `--jobs 1 --threads 1` for controlled comparisons. `--threads` is passed to both routers
(the current Rust implementation does not use that setting to create workers);
`--jobs` controls how many candidate/board/repetition processes run concurrently. Time includes
process startup and the candidate's routing/optimization/output work, but excludes referee work.

With `--jobs > 1`, automatic comparison uses CPU time instead of wall time. CPU time still
changes with host load, cache contention and scheduling; it is not a correction for contention.
Time-limited routing may also produce different quality under contention. Keep host, jobs,
threads and budgets identical, and confirm claimed improvements with serial runs. You can force
`--time-metric wall` or `--time-metric cpu` for analysis.

## Corpus and referees

```bash
uv run bench corpus init
uv run bench corpus kicad-fixtures
uv run bench corpus pcbench --clone ~/PCBench --licensed-only --jobs 4
uv run bench corpus list --tier pcbench
```

`corpus init` imports the Java checkout's DSN fixtures. KiCad fixtures and PCBench imports
strip routing, export DSN inputs and measure the reference board. PCBench license metadata
is retained in the manifest; `--licensed-only` selects classified licensed sources. Review
individual license terms before sharing inputs. `--skip-existing` is the import default;
`--force` regenerates boards. Use `--ids` for PCBench directory names and `--boards` on `run`
for manifest board IDs; both accept comma-separated values.

DSN fixtures use the Java jar's DRC-only mode. KiCad boards use `kicad-cli pcb drc` after the
suite imports the session with `vendor/kicad/ses_to_board.py`. The referee preserves project
rules or reconstructs legacy rules, refills zones, and counts routing-related DRC errors.
KiCad failures remain unjudged; there is no automatic Java fallback. KiCad requires both
`kicad-cli` and a Python interpreter with `pcbnew`, configured through `.env` when necessary.

```bash
uv run bench referee --run head-vs-change --only-missing --jobs 1
uv run bench corpus revalidate --origin pcbench --regenerate-projects --rerun-drc --jobs 4
```

Java-scored runs record the resolved referee command and jar SHA-256 in `meta.json`, and Java
measurements record the same identity in `metrics.json`. `bench referee --only-missing` reuses
that recorded referee and refuses a changed jar. To change referees, pass `--candidates-file`
and rescore the full run without `--only-missing`. For older runs or runs made with
`--no-referee`, use their original candidates file when first scoring them. KiCad-only runs
and rescoring do not require a Java referee jar.

Revalidation can change corpus membership and ground truth. Finish it before benchmarking and
keep the corpus fixed between candidates. Compare/export currently use the current manifest;
archive it along with inputs if a result needs to remain reproducible after corpus updates.
Candidate and Java referee processes receive isolated HOME/XDG directories to prevent saved
router settings leaking between invocations. Their `JAVA_TOOL_OPTIONS` also includes
`-Djava.awt.headless=true -Dapple.awt.UIElement=true` to disable AWT windows and suppress
normal macOS GUI activation, including for referee and connection-count probes. Existing
`JAVA_TOOL_OPTIONS` are preserved; only subprocess environments are changed. Reports warn about results lacking isolation data.

## Reports, exports and history

```bash
uv run bench report --compare head-vs-change
uv run bench export --run head-vs-change --candidate rs-head
uv run bench export --run head-vs-change --candidate rs-change
uv run bench plot --files exports/BASELINE.json,exports/CHANGE.json --out reports/change.html
```

Use the filenames printed by `export` in the last command. Exports contain compact per-board
measurements, candidate SHA, host and configuration; they are tracked and can be deliberately
committed. Raw `results/` and generated `reports/` remain local. To share a report, explicitly
add the chosen files with `git add -f reports/<name>.{json,md,html}` and retain the source run
and corpus elsewhere. Exports alone cannot reconstruct every raw cell or rerun the referee.

Existing baseline files and exports are fixed historical measurements, not an updating HEAD
baseline. New comparisons should pin and measure the intended baseline. The archive utility
`scripts/make-java-view.sh` can extract the specific Java result set it names from existing
local result data.

## Remote runs

The `scripts/remote-*.sh` helpers launch jobs on a configured NixOS host. See `.env.example`
for `BENCH_REMOTE_*` settings and each script's usage header. `remote-setup.sh HOST` prepares
the KiCad Python wrapper; `remote-corpus.sh` imports corpus data; `remote-run.sh` syncs the
suite and Java jar, launches a detached job, and pulls results back.

```bash
scripts/remote-setup.sh "$BENCH_REMOTE_HOST"
scripts/remote-run.sh "$BENCH_REMOTE_HOST" --remote-dir freerouting-rs/benchmark --jobs 1 -- \
  --candidates java-current,rs-main --seeds 3 --threads 1 --tier canary \
  --max-passes 100 --timeout 120 --run-id remote-java-vs-rust
```

Provision the Rust repository and build its release binary **on the remote host** first.
These helpers sync the benchmark directory, not a Rust worktree or its build output. For branch
comparisons, build both pinned revisions there and provide a candidates file with remote paths
and known SHAs; `sha_from` only works when the referenced remote Git checkout exists. Keep each
worktree's remote directory separate. Local binaries may target a different architecture.

Remote run IDs cannot be reused, including completed or interrupted runs. Use a new ID for
a new measurement; the script refuses existing IDs before syncing and reserves launches
against concurrent reuse. A successful retrieval requires the remote job's recorded exit status.
`--no-wait` returns after launch; `remote-run.sh HOST --attach RUN_ID` resumes polling and result
retrieval. Corpus import does not pull all board files back automatically; copy the corpus
inputs separately if you need local routing or rescoring. Do not mix local and remote timings.

## Suite checks

```bash
uv run pytest -m 'not slow'
uv run pytest -m slow
```

The first command exercises the suite with synthetic router outputs. Slow tests require the
external Java/KiCad tools and provide integration checks. Neither replaces measuring actual
routing performance on the chosen corpus.
