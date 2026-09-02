# `freerouting` — the command-line program

Two command lines reach the same program.

* The **legacy** form is Java's, reproduced bug for bug:
  `freerouting -de board.dsn -do board.ses -mp 100`.
* The **native** form is this port's own: `freerouting route board.dsn -o board.ses`.

`legacy::is_legacy_form` decides which: **the native form starts with a subcommand name; every
other argv — including an empty one — is legacy.** So `freerouting` with no arguments is a *legacy*
command line, and it answers what the jar answers: `Both an input file and an output file must be
specified …` and **exit 1**, not a usage screen.

> **Status.** Plan 8 Task 5 landed the command line: the parse, the mode ladder, the exit ladder
> and the log surface. **Task 6 landed `route`** — the sixteen steps of `Freerouting.initializeCli`
> end to end, with SES byte parity against the HEAD jar on eleven boards (see the acceptance table
> below). **Task 7 landed `drc`** — the thirteen steps of `Freerouting.initializeDrc`, with report
> byte parity on seven of the eight committed `drc-*` stems and the eighth an `XDIFF` the jar
> cannot win either (see its own table below). `info` is the last stub answering exit 3, until
> Task 12. Task 13 expands this file into the full reference (every accepted flag, the
> `p8t1`-`p8t7` acceptance table, the MCP delta table).

---

## The exit ladder (plan ruling AR)

| code | when | Java |
|---|---|---|
| **0** | a completed run; `--help`; `drc` **unless** the input is unreadable, the board will not load or the report cannot be written (quirk **#271** — `initializeDrc` returns `true` whatever else happens: a missing `.rules`, a missing session, a failed quality score and **any number of violations** all leave the code at 0) | `Freerouting.java:1495`, `:1397`, `:373` |
| **1** | any failure — **including every refusal on the legacy path** | `Freerouting.java:1474` |
| **2** | **port only:** a usage error on the *native* subcommand form (clap's own) | — |
| **3** | **port only, reserved:** a subcommand not wired up yet | — |

Ruling AR is why 2 and 3 are native-form-only: *a command line the jar accepts must never come
back with a code the jar cannot produce.* `legacy::rewrite` therefore never fails — it warns — and
every argv it builds parses, so clap can only refuse an argv the user typed in the native form.

The same rule decided `--version`: the jar **refuses** `--version` and `-V`
(`Unknown command line argument: --version`, then `initializeCli`'s refusal, **exit 1** — measured
on the HEAD jar, and pinned by `p8t5`'s `version-long`/`version-short` rows). So the legacy path has
no `--version` arm, and clap's lives behind a subcommand: `freerouting route --version`.

---

## Port-only spellings

Java has none of these. Each is marked "Port only" in its own `--help` text too, so that a reader
who learns this CLI does not type them at the jar.

| spelling | what it is | Java's equivalent |
|---|---|---|
| `-v` / `-vv` / `--verbose` | raise the log level | **none** — the jar's log-level flag is `-ll <level>` (quirk #260) |
| `--log-level <level>` | set it outright | `-ll <level>`, or `--logging.console.level=<level>` |
| `--settings <file>` | name a `freerouting.json` for the priority-10 tier. **Native form only** (rulings R7/BG) | **none** — the jar only reads the file under its OS-standard user-data path, and warns at `--settings` as an unknown argument |
| `--version` / `-V` | print the version (native form only) | **none**; the jar prints its version in the startup banner (`Freerouting.java:1120`) |
| `--kicad-json <file>` | a KiCad board file in its own slot (native form only, ruling 14) | **none** — on the legacy form a `.json` takes Java's own slot |
| `--set <section.field=value>` | a generic settings override | `--section.field=value`, which the legacy form still accepts |
| `--schema <kicad\|freerouting>` | which spelling of the KiCad DRC schema `drc` writes; **default `kicad`**, native form only (ruling W, quirk #154) | **none** — each jar hard-codes one spelling, and HEAD's disagrees with the `$schema` it advertises |
| `drc` with no `-o` | write the report to **stdout** (quirk #275, spec §12) | **none** — `Freerouting.java:368-371` is dead code, because a bare `-drc` is not DRC mode (quirk #263) |

`--settings` was accepted and unread through Task 5; **Task 6 wired it** — `SettingsInputs::json_file`
now carries the priority-10 tier into both of `resolve_headless`'s chains, and without the flag the
working directory's `freerouting.json` stands in for Java's OS-standard user-data path.

**Native form only, and there is no default file** — controller ruling **BG**, on the measurement
below. Round 1 of the Task 6 review left the flag working on the legacy form too and recorded that
as accepted; ~~"honouring a port-only flag on both command lines was judged less surprising than
refusing it on one"~~ was ruled out: scan ruling R7 scoped the flag to the native form and ruling
AR makes the legacy path bug-for-bug, so the legacy form now warns exactly as the jar does and
applies nothing.

| jar input | `scoring.via_costs` in its manifest |
|---|---|
| `--settings s.json` | **50** — two `Unknown command line argument` warnings (`GlobalSettings.java:833`, once for the flag and once for its argument — it is not a value-consuming arm), file ignored |
| started **in** a directory holding `freerouting.json` | **50** — ignored |
| `-Duser.home` at a home whose *user-data* `freerouting.json` sets `via_costs 77` | **77** — applied at priority 10 |
| the same, with no such file (control) | 50 |

The jar reads exactly one location, `GlobalSettings.getUserDataPath().resolve("freerouting.json")`
(macOS: `~/Library/Application Support/freerouting/freerouting.json`), which is `static` mutable
state spec §2 does not port. Task 5 had made the **working directory** stand in for it; the middle
two rows show it stood in for nothing the jar does, so ruling BG removed that default as well.
`--settings <file>` on the native form is now the whole surface.

Pinned by `cli_e2e.rs::a_settings_file_reaches_the_run` (five runs across both forms) and by
`p8t1`'s `settings-on-legacy` row, which compares the two warnings against the **live jar**.

---

## The dead legacy knobs, and `--set` (plan ruling AQ)

Seven legacy writes are **parsed and dead**, in Java and therefore here:

| flag | what it writes | who reads it |
|---|---|---|
| `-oit` | `routerSettings.optimizer.optimizationImprovementThreshold` | **nobody** |
| `-us` | `routerSettings.optimizer.boardUpdateStrategy` | **nobody** |
| `-is` | `routerSettings.optimizer.itemSelectionStrategy` | **nobody** |
| `-hr` | `routerSettings.optimizer.hybridRatio` | **nobody** |
| `-inc` | `routerSettings.ignoreNetClasses` | **nobody** |
| `-drc`'s `routerSettings.enabled = false` | the same object | **nobody** |
| `-mt` | `routerSettings.optimizer.maxThreads` | **nobody** (quirk #143: the whole multithreaded path is unreachable) |

All seven write Java's `@Deprecated public final RouterSettings routerSettings` bridge
(`GlobalSettings.java:51-53`), which no routing path consults. `docs/java-quirks.md` #131 and #143
are the rows; `crates/fr-settings/src/sources/cli.rs`'s `LegacyBridge` carries the writes with a
`// Java bug:` marker per field, and nothing in `resolve_headless` reads it.

**Ruling AQ: the port keeps them dead, and keeps parsing them.** Keeping the parse is not
pedantry — it is observable, because the value decides how far the cursor moves and therefore which
*other* arguments get warned about, and `p8t5` compares that. Wiring them up would make the port
**more capable than the jar**, which is a product decision and not a parity fix.

**The same knobs are reachable through `--set`.** Every one of them is a field of `RouterSettings`,
so the supported spellings are the two generic ones, which go through `CliSettings` at priority 60
— the parser that actually reaches the router:

```sh
# native form
freerouting route board.dsn -o board.ses \
    --set router.optimizer.optimization_improvement_threshold=0.005 \
    --set router.optimizer.board_update_strategy=GLOBAL_OPTIMAL \
    --set router.optimizer.item_selection_strategy=SEQUENTIAL \
    --set router.optimizer.hybrid_ratio=1:2 \
    --set router.optimizer.max_threads=4

# legacy form — Java's own `--section.field=value`, accepted unchanged
freerouting -de board.dsn -do board.ses --router.optimizer.max_threads=4
```

Two warnings that are not hedges:

* **`--set` is not wired to the run path yet** (Task 6). The spelling and the decision are settled;
  the plumbing is not.
* **The two spellings are not equivalent to the dead flags.** `--router.optimizer.max_threads=4`
  goes through `ReflectionUtil.setFieldValue` and therefore carries **no clamp**, where `-mt`
  clamps to `[0, 1024]` on the bridge (`GlobalSettings.java:692-697`). `-mt 99999` is 1024 there;
  `--router.optimizer.max_threads=99999` is 99999. Both are measured — `p8t5`'s `mt` and
  `router-optimizer-max-threads` rows.

---

## Logging

Everything goes to **stderr**, and there is no log file. That is a deliberate divergence from the
jar, which writes `INFO`/`WARN` to **stdout** and an `ERROR` three times over (stdout, the log
file, and stderr) — `docs/java-quirks.md` #261. The port also emits no timestamp, so its output is
byte-reproducible; `normalize_log` strips the jar's before comparing.

The level ladder is Java's: the console default is `INFO`, `-ll <level>` moves it (matched by
prefix, **last** occurrence wins), and an unrecognised name silently means `INFO`
(`Log4j2ConfigurationFactory.parseLevel:130-135`).

---

## Where the rest of it is written down

| what | where |
|---|---|
| every legacy flag's value normalisation, and the `-de` rule | `docs/cli-legacy-flags.md` |
| the parse rules, arm by arm, with the Java line for each | `src/legacy.rs`'s module docs |
| the log level ladder, the stderr divergence, `MESSAGE_MAP`, and the `logger/**` roster | `src/logging.rs` |
| the Java behaviours reproduced on purpose | `docs/java-quirks.md` #131, #143, #259-#264 |
| the differential that pins all of it against the jar | `scripts/differential/{run.sh p8t5, sweep-p8t5.sh}`, 86 argv shapes |


---

## `route`: the acceptance table (Plan 8 Task 6, controller ruling AV)

`freerouting route` is measured against the **HEAD jar as a whole program**, not against a method.
Two harnesses, one comparison:

* `scripts/differential/run.sh p8t1 [all]` runs `java -jar <jar> <argv>` and `freerouting <argv>`
  live, on the argv recorded in each `tests/reference/cli-<stem>/argv.txt`;
* `crates/freerouting/tests/cli_e2e.rs` runs the port against the **committed** outputs of the
  same jar runs (`scripts/gen-cli-reference.sh`), so a machine with no JDK checks the same thing.

Four rungs per stem: byte-identical SES, equal exit code, equal `parity::normalize_log`, and —
`p8t2 e2e`'s — equal `parity::normalize_manifest` for a second run with `--router.result_json=<f>`.

| stem | lane | argv beyond `-de`/`-do` | `p8t1` | `p8t2 e2e` |
|---|---|---|---|---|
| `router-rpi-splitter` | ci | `-mp 8` | MATCH | MATCH |
| `router-j2-reference` | ci | `-mp 99` | MATCH | MATCH |
| `router-ecc83-input` | ci | `-mp 8` | MATCH | MATCH |
| `router-empty-board` | ci | `-mp 1`, both stages off | MATCH | MATCH |
| `router-dac2020-bm01` | slow | `-mp 2` | MATCH | MATCH |
| `router-tutorial-board` | slow | `-mp 8` | MATCH | MATCH |
| `router-fanout-bm11` | slow | `-mp 2`, optimizer off | MATCH | MATCH |
| `router-strict-drc-cnh` | slow | `-mp 2` | MATCH | MATCH |
| `tutorial_board` | slow | *(bare)* | MATCH | MATCH |
| `Issue026-J2_reference` | slow | *(bare)* | MATCH | MATCH |
| `large-outline` | slow | `-mp 2`, optimizer off | MATCH | MATCH |

Plus five **argv rows**, which is where the exit-code and log rungs earn their keep — every stem
above succeeds and emits no message `logging::MESSAGE_MAP` names, so on the stems the log rung
compares two empty projections.

| row | argv | `p8t1` | what it pins |
|---|---|---|---|
| `missing-input` | `-de <missing>.dsn -do a.ses` | MATCH | `Freerouting.java:105` + `:109`, and quirk #261's duplicated `ERROR` folding back into one |
| `no-files` | `-mp 1` | MATCH | `Freerouting.java:81`'s refusal, exit 1 |
| `do-out-dsn` | `-de <dsn> -do b.dsn -mp 1` | MATCH | quirk #268: a 0-byte file, exit 1 |
| `settings-on-legacy` | `-de <dsn> -do d.ses -mp 1 --settings s.json` | MATCH | ruling BG — two `GlobalSettings.java:562` warnings on both sides, exit 0, and the file ignored on both |
| `invalid-input-java-hangs` | session bytes under a `.dsn` name | **XDIFF** | quirk #244 / plan ruling 7 — **the jar hangs for ever**; the port exits 1. Not run against the jar, for the obvious reason |

### XDIFF rows

**One**, and it is the row above. `invalid-input-java-hangs` is a *totalisation* the plan asked
for (ruling 7), not a defect: `Freerouting.isCliTerminalState` (`:189-194`) omits
`RoutingJobState.INVALID`, which `RoutingJobScheduler.java:83`/`:253` assigns for an input that is
neither DSN nor KiCad JSON, so the jar sits in `:151-158`'s `while (…) Thread.sleep(500)` at 0 %
CPU with no output and no message. The port exits **1**. Pinned by
`cli_e2e.rs::de_a_ses_exits_1_instead_of_hanging`.

**No stem row is an XDIFF**, and there is no tolerance anywhere in the ladder. The one
normalisation the SES comparison applies is quirk **#92**'s closed set of four `(parser …)`
keyword literals, rewritten on the *jar* side — the same rewrite `crates/fr-router/tests/
batch_parity.rs` applies to `batch.ses`, and for the same reason (Plan 3 ruling 1 pins the port's
writer to the 2.3.0 spelling because HEAD writes Specctra its own lexer cannot read back).
Measured on `router-rpi-splitter`: those two lines are the **only** difference between the jar's
3 654 bytes and the port's 3 656.

### The budget is live on both sides

Every `p7t*` driver runs the port with `RouterBudget::disabled()` against a jar whose
`optChangedArea` limit is a javac-inlined constant. **This gate does not**: the port's CLI runs
`fr_core::RouterBudget::default()` — Java's own 1000 / 10000 / 250 / 1000 literals — because that
is what a user gets, and the comparison is between two whole programs. The cost is that a live
wall clock is a machine-speed dependency; the bound on it is
`scripts/gen-cli-reference.sh`'s `batch.ses` cross-check, which requires each CLI reference to be
byte-identical to Plan 7's independently generated one and fails loudly if it is not.

---

## `drc`: the acceptance table (Plan 8 Task 7)

`freerouting drc` is measured the same way `route` is — two whole programs on the same argv —
by `scripts/differential/run.sh p8t3 e2e`, over the eight rows of `tests/reference/drc-fixtures.txt`
plus six argv rows. Rungs per stem:

| rung | assertion |
|---|---|
| (a) | the two reports are byte-identical after `parity::normalize_drc_json` (which drops `date` and sorts each `unconnectedItems` entry's `items` by numeric uuid — plan-5 ruling 3, quirk #144) |
| (b) | `quality_score` is equal — asserted *before* the document, because it is the value Task 7 newly **computes** where Plan 5 injected it |
| (c) | the two exit codes are equal |
| (d) | `parity::normalize_log` of both sides is equal |
| (e) | the port's shipped default is the **KiCad** spelling: `quality_score`, not `qualityScore` (ruling W) |

Rung (a) needs the jar's key spelling, so the port is run a second time on the native form with
`--schema freerouting`; rungs (b)-(e) use the **legacy** argv, so the shim, the message set and the
exit ladder are compared on the command line a user actually types.

| stem | argv beyond `-de` | `p8t3 e2e` | `quality_score` |
|---|---|---|---|
| `drc-dev-board` | — | MATCH (20 374 B) | `902.078369140625` |
| `drc-bbd-mars-64` | — | MATCH (62 393 B) | `828.276123046875` |
| `drc-natural-tone-preamp` | — | **XDIFF** | `334.8545227050781` — **MATCH** |
| `drc-issue593-rules` | `-dr <rules>` | MATCH (42 010 B) | `0.0` |
| `drc-issue593-ses` | `<ses>` in the `-de` slot list | MATCH (44 501 B) | `556.5908203125` |
| `drc-issue753-cpu85` | — | MATCH (189 817 B) | `331.1068115234375` |
| `drc-issue110-relay` | — | MATCH (34 269 B) | `0.0` |
| `drc-tutorial-board` | — | MATCH (290 B) | `0.0` |

**All eight quality scores match the committed references exactly**, which is the acceptance the
plan asked for: Plan 5 *injected* that number from the reference, and Task 7 *computes* it from
`fr_router::score::BoardStatistics::normalized_score`, so the eight references became eight free
assertions the moment the injection was removed. They are available **without a JDK** too:
`cli_e2e.rs::every_committed_reference_score_is_recomputed` walks the same eight rows, runs the
binary, and compares its computed `quality_score` to the value the committed `drc.json` records —
the `route` reference lanes' pattern.

Plus six **argv rows**, which is where rungs (c) and (d) earn their keep — the stems all succeed,
and three of the five things that can go wrong on this path do not move the exit code (quirk #271):

| row | argv | `p8t3 e2e` | what it pins |
|---|---|---|---|
| `missing-rules` | `-dr <missing>.rules` | MATCH | `Freerouting.java:289` warns; **exit 0**, report written |
| `missing-session` | `<missing>.ses` in the `-de` slot list | MATCH | `:324` warns; **exit 0**, report written |
| `rules-and-session` | `<dsn> <ses> -dr <rules>` | MATCH | quirk #273 — the only run that fills both optional slots; the six-line log shows `:281`/`:286` before `:309`/`:312` on both sides |
| `missing-input` | `-de <missing>.dsn` | MATCH | `:266` + `:267`, **exit 1**, no report. The driver's row prints `Freerouting.java:105`, not `:266`: the two messages are byte-identical and `logging::MESSAGE_MAP` folds a line onto the **first** template that matches, so both programs resolve it to `initializeCli`'s site — which is what makes the row a comparison rather than a coincidence |
| `ses-input` | session bytes under a `.dsn` name | MATCH | quirk #274 — the **loader** refuses (`BoardLoader.java:33`), `:272` + `:273`, **exit 1** |
| `unwritable-report` | `-drc <missing-dir>/r.json` | MATCH | `:365` + `:366`, **exit 1** — the whole check ran and the run still fails |

`scripts/differential/run.sh p8t3` (no argument) is the **other** mode: a genuine Java-vs-Rust pair
over quirk #272's separate settings merge (`P8T3.java` against
`commands::drc::{quality_score_settings, quality_score}`), printing the seven scoring weights, the
six board counters and the score in raw IEEE bits for each of the eight stems. 25 lines, MATCH.

### The one XDIFF, and why no port can remove it

`drc-natural-tone-preamp` is quirk **#146**, and it is the case where *the jar does not match
itself*. `generateReport` folds `getAllUnconnectedItems`' `track_dangling` entries into
`violations` (`DesignRulesChecker.java:271-276`), and that phase's dedup (`:160`) drops whichever
dangling trace a net entry's **identity-hash-ordered** `firstItem` happens to be. Across
`-XX:hashCode=0..4` the jar produces 113-115 violations on this board; `-XX:hashCode=2` — the only
mode that reproduces run to run without being derived from an object address — gives 115, which is
the committed reference. The port's ascending-id representatives (plan-5 ruling 3) give **112**.

The three that differ are pinned **by uuid, never by count** — `1909`, `1696`, `1242` — here, in
`p8t3`'s row detail, and in `crates/fr-drc/tests/reference_parity.rs::natural_tone_preamp_is_the_
reference_minus_three_dangling_tracks`. **The grant is checked, not waived**, and it is that
test's shape: `p8t3` deletes exactly those three entries from the **jar's** document, requires all
three to have been present, and then requires everything left to be byte-identical to the port's —
so a **port**-side extra entry, or any other difference anywhere in the document, is still a
`DIFF`. Everything else, including the whole 44-entry `unconnectedItems` block **and the
`quality_score`**, is identical.

