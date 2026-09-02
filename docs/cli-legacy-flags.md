# Legacy CLI flags: Java's per-flag value normalisation

`crates/freerouting/src/legacy.rs` rewrites Java-freerouting command lines into
the port's subcommand form. Java normalises most flag values *while parsing* —
clamping, dividing, lower-casing, or falling back to a default for an
unrecognised word. Reproducing those rules is `fr-settings`' job — **Plan 4**
Task 7 in the event, not the "Plan 5" this file guessed at when it was written
(`fr-settings` is Plan **4** and `fr-drc` is Plan 5 in the Plan 3+ numbering; the
cells below have been renumbered). This file records the rules once, from the
Java, so the later plans do not re-derive them (and get them wrong).

**Plan 8 Task 5 rewired the shim.** Where this file used to say ~~"It forwards
raw values"~~, `legacy.rs` now forwards *nothing*: it is a mode and slot
resolver, and every value goes to `fr-settings` off the **raw** argv. The
consequences are recorded in place below — the `-de` section's "one deliberate
divergence" is gone (ruling 14), and the `-mp`/`-mt`/`-oit`/`-us`/`-is`/`-hr`/
`-inc` rows now say where the value actually lands.

**Ported, and five of them are dead on purpose.**
`crates/fr-settings/src/sources/cli.rs` carries all of this:

- **`apply_command_line_arguments(&[String]) -> LegacyBridge`** — the flag table
  below, with every normalisation applied exactly as Java applies it. **Nothing
  reads the result.** `-oit`, `-us`, `-is`, `-hr`, `-inc` and `-drc`'s
  `routerSettings.enabled = false` write only Java's `@Deprecated` bridge
  (`GlobalSettings.java:51-53`), which no routing path consults, so they are
  **parsed but dead** — in Java, and therefore here (plan 4 ruling 8,
  `docs/java-quirks.md` row 131). Making them live would make the port *more
  capable than Java*; `docs/plan-4-handoff.md` §10 handed that decision to Plan 8.

  **Plan 8 ruling AQ closed it: they stay dead, and they stay parsed.** Keeping
  the parse is not pedantry — the value decides how far the cursor moves and
  therefore which *other* arguments get warned about, and `p8t5` compares that.
  **The same knobs are reachable through `--set`**, which is the port's spelling
  of Java's own `--section.field=value` and goes through `CliSettings` at
  priority 60 — the parser that actually reaches the router:

  ```sh
  freerouting route board.dsn -o board.ses \
      --set router.optimizer.optimization_improvement_threshold=0.005 \
      --set router.optimizer.max_threads=4
  freerouting -de board.dsn -do board.ses --router.optimizer.max_threads=4
  ```

  The two are **not** equivalent to the dead flags: the generic path goes through
  `ReflectionUtil.setFieldValue` and carries **no clamp**, where `-mt` clamps to
  `[0, 1024]` on the bridge (`GlobalSettings.java:692-697`). `-mt 99999` is 1024
  there; `--router.optimizer.max_threads=99999` is 99999 (`p8t5` rows `mt` and
  `router-optimizer-max-threads`). `crates/freerouting/README.md` carries the
  whole table and the same warning; **`--set` is not wired to the run path until
  Plan 8 Task 6** — the spelling and the decision are what is settled here.
- **`CliSettings`** — the only two flags that actually reach the router, `-mp`
  and `-mt` (`CliSettings.mapFlagToProperty`, `:102-110`).
- **`classify_de_arguments(&[String]) -> DeSlots`** — the `-de` rule below (plan 4
  ruling 10). `crates/freerouting/src/legacy.rs` **calls it** since Plan 8 Task 5;
  the binary's own copy of the rule is deleted.
  `classify_de_arguments_reporting` is the same function with the five
  `FRLogger.warn` lines the `-de` arm emits, which the CLI needs and this crate
  may not log itself.
- **`legacy_flag_value_is_consumed(&str, &str) -> bool`** — the half of the loop
  that decides `i++`. `:686`, `:698`, `:708` and `:822` each run *after* an
  assignment that can throw, so `-mp abc` logs at `:836`, consumes nothing, and
  lets `abc` reach the unknown-argument warning at `:833`. It lives here so the
  CLI can walk the same argv without re-deriving `Integer.decode`,
  `Float.parseFloat` or `Integer.parseInt`.
- **`JsonFileSettings`** (`crates/fr-settings/src/sources/json_file.rs`) — the
  `freerouting.json` tier at **priority 10**, ported by Plan 8 Task 5 under scan
  ruling R7. Plan 4 had rostered it `// not ported:` on spec §2 ("no persistent
  config file") and reserved the number; the Plan 8 controller ruled it in,
  because it is the only rung between `DefaultSettings` (0) and the DSN file (20)
  and because the CLI is where `--settings <file>` belongs. Two notes:

  - Java resolves the file under the OS-standard **user-data** directory
    (`GlobalSettings.getUserDataPath().resolve("freerouting.json")`,
    `JsonFileSettings.java:27-29`). That path is `static` mutable state and is
    not ported, so the port looks for `freerouting.json` in the **working
    directory**, and `--settings <file>` names one explicitly. `--settings` is a
    **port-only** flag: Java has none.
  - **Quirk label AJ still holds.** `mcp_server.stdio=true` read from
    `freerouting.json` is silently ignored by Java — the stdout redirect has to
    happen before logging is initialised, so the JSON setting arrives 250 lines
    too late and only earns a warning (`Freerouting.java:1191-1203`). The port's
    `mcp` **subcommand** is the supported spelling; the legacy
    `--mcp_server.stdio=true` is accepted and rewritten to it.

Baseline: freerouting **v2.3.0**. All line numbers in this file are re-derived
against the Java clone's HEAD as of Plan 4 (plan ruling 7's convention), not
against the v2.3.0 tag — re-check them against a later clone before reusing
this table. Primary source `app/freerouting/settings/GlobalSettings.java`,
method `applyCommandLineArguments` (the flag table runs :521-838; the
numeric/strategy flags cited below are :675-731). Secondary sources
`app/freerouting/settings/RouterSettings.java` and
`app/freerouting/settings/sources/CliSettings.java`.

A blanket rule applies to every flag below: a value is only consumed when the
next argument exists **and does not start with `-`** (`args.length > i + 1 &&
!args[i + 1].startsWith("-")`). Otherwise the flag is silently a no-op and the
field keeps its previous value — Java never errors on a missing value. The
whole loop body is additionally wrapped in `try { … } catch (Exception e)`
(`GlobalSettings.java:835-837`), so a malformed number logs an error and the
flag is skipped rather than aborting the run.

## Router / optimizer flags

| Flag | Java field | Java normalisation | Java location | Where the port must apply it |
|---|---|---|---|---|
| `-mp` | `routerSettings.maxPasses` | `Integer.decode(v)`; then `< 0 → 1`, `> 9999 → 9999`. **`0` is deliberately allowed and means *unlimited*.** A second, *different* clamp runs later in `RouterSettings.validate()`: there `< 0 \|\| > 9999 → 9999` and `== 0 → Integer.MAX_VALUE`. | `GlobalSettings.java:675-686`; `RouterSettings.java:932-941` | `fr-settings` (Plan 4), at both the parse step and a `validate()` equivalent. Plan 6/7's pass loop must treat `max_passes == 0` as "no limit", never "no passes". |
| `-mt` | `routerSettings.optimizer.maxThreads` | `Integer.decode(v)`; then `< 0 → 0`, `> 1024 → 1024`. **No further normalisation on this path** — see the quirk below. | `GlobalSettings.java:688-698` | `fr-settings` (Plan 4). **Not** Plan 6's optimizer thread pool — headless Java reads this field nowhere (row 143, and the correction below). |
| `-oit` | `routerSettings.optimizer.optimizationImprovementThreshold` | `Float.parseFloat(v) / 100`; then `<= 0 → 0.0f`. Note the value is a **percentage** on the command line and a fraction in the settings, and the division happens before the clamp. Parsed as `float`, not `double`. **Correction (Plan 4 Task 7, JVM-verified):** an earlier revision of this table said `-oit -5` becomes `0.0f`. It does not — `-5` starts with `-`, so the blanket rule above never consumes it and the field keeps its previous value. The `<= 0` clamp is reachable only from a literal zero. | `GlobalSettings.java:700-708` | `fr-settings` (Plan 4). Keep the `f32` rounding — a `f64` division by 100 gives a different bit pattern. |
| `-us` | `routerSettings.optimizer.boardUpdateStrategy` | `v.toLowerCase().trim()`; then `"global" → GLOBAL_OPTIMAL`, `"hybrid" → HYBRID`, **anything else → `GREEDY`**. There is no error for an unrecognised word. | `GlobalSettings.java:710-719` | `fr-settings` (Plan 4). Must be a total function with a `GREEDY` fallback, not a `FromStr` that fails. |
| `-is` | `routerSettings.optimizer.itemSelectionStrategy` | `v.toLowerCase().trim()`; then **prefix** match `indexOf("seq") == 0 → SEQUENTIAL`, `indexOf("rand") == 0 → RANDOM`, **anything else → `PRIORITIZED`**. Prefix, not equality: `sequential`, `seq`, `sequestered` all give `SEQUENTIAL`. | `GlobalSettings.java:721-731` | `fr-settings` (Plan 4). Prefix match with a `PRIORITIZED` fallback. |
| `-hr` | `routerSettings.optimizer.hybridRatio` | `v.trim()` only — stored as a raw `String` and parsed later. | `GlobalSettings.java:732-736` | `fr-settings` (Plan 4): keep it a string here, parse where Java parses it. |
| `-inc` | `routerSettings.ignoreNetClasses` | `v.split(",")` — **the individual entries are not trimmed and not lower-cased**, unlike `debug.filter_by_net` (`GlobalSettings.java:552-557`), which does both. `-inc "GND, VCC"` yields `["GND", " VCC"]` and the second never matches a net class. | `GlobalSettings.java:810-815` | `fr-settings` (Plan 4). Reproduce the missing trim; do not "fix" it before parity. |

## Non-router flags the port currently drops

The shim discards these, but the normalisation is recorded so a later plan that
adopts one gets it right.

| Flag | Java field | Java normalisation | Java location |
|---|---|---|---|
| `-l` | `currentLocale` | `v.toLowerCase().replace("-", "_")`, then a **prefix** chain (`zh_tw` before `zh`, `pt_br` before `pt`, …). An unmatched string leaves the previous locale untouched. | `GlobalSettings.java:737-798` |
| `-ll` | `logging.console.level` | `v.toUpperCase()`, unvalidated. | `GlobalSettings.java:825-830` |
| `-host` | `runtimeEnvironment.host` | `v.trim()`. | `GlobalSettings.java:803-807` |
| `-dct` | `guiSettings.dialogConfirmationTimeout` | `Integer.parseInt(v)`; then `<= 0 → 0`. | `GlobalSettings.java:816-824` |
| `-dl` | `logging.file.enabled` | Switch: sets `false`. No value consumed. | `GlobalSettings.java:799-800` |
| `-da` | `usageAndDiagnosticData.disableAnalytics` | Switch: sets `true`. No value consumed. | `GlobalSettings.java:801-802` |
| `-drc` | `routerSettings.enabled`, `drcSettings.enabled` | Switch plus optional report path: sets `routerSettings.enabled = false` and `drcSettings.enabled = true` **before** looking for a value, so a bare `-drc` still switches to DRC-only mode. Matched before `-dr` on purpose. | `GlobalSettings.java:660-669` |

## Quirks worth pinning

1. **`-mp 0` means unlimited, not "no passes".** `GlobalSettings.java:684`
   carries the comment "Note: 0 is allowed and means no limit", and
   `RouterSettings.validate()` (`RouterSettings.java:937-940`) turns it into
   `Integer.MAX_VALUE`. `scripts/gen-reference.sh` originally used `-mp 0` to
   generate *unrouted* references and would in fact have generated fully routed
   ones; it now uses `--router.enabled=false`.

2. **`-mt 0` does *not* mean "all cores" on the CLI path.** There are two
   different max-thread normalisations in `RouterSettings.java` and the CLI
   reaches neither:
   - `normalizeMaxThreads` (`RouterSettings.java:137-149`) maps `null →
     max(1, cores - 1)`, `< 0 → max(1, cores - 1)`, **`0 → cores`**, else
     `min(v, cores)`. It is reached only through `setMaxThreads`
     (`RouterSettings.java:175-186`), i.e. the config/API path.
   - `validate()` (`RouterSettings.java:943-955`) normalises
     `RouterSettings.maxThreads` and leaves `0` as `0` (0 is neither `< 0` nor
     `> cores`) — already inconsistent with `normalizeMaxThreads`.
   - `-mt` writes `routerSettings.optimizer.maxThreads` **directly**
     (`GlobalSettings.java:690`), a different field from
     `RouterSettings.maxThreads`, so neither routine touches it. A *third*
     writer, `RouterSettings.setMaxThreads` (`:183-185`), mirrors
     `RouterSettings.maxThreads` into it.
   - **Correction (Plan 4 Task 12, re-derived from the HEAD source): the
     headless path reads neither field.** An earlier revision of this table said
     `optimizer.maxThreads`' only consumer is `BatchOptimizer.java:58` and that
     `-mt 0` therefore "selects the single-threaded optimizer". That line
     `:58` is inside **`createForGui`** (`:56-66`) and is additionally gated on
     `globalSettings.featureFlags.multiThreading`. Headless goes
     `RoutingJobSchedulerActionThread.java:99` →
     `RoutingPipeline.createForHeadless` (`:45-47`, doc: "headless
     single-threaded optimizer policy") → `BatchOptimizer.createForHeadless`
     (`:51-53`), which returns `new BatchOptimizer(job)` **unconditionally** and
     never reads the field. `GlobalSettings.getNumThreads()` (`:851-854`) returns
     it and has no caller. The sibling `RouterSettings.maxThreads` is no
     livelier: its only non-GUI reader, `AutoroutePassRunner.runMultiThread`
     (`:50,53,91`), is reached only from
     `BatchAutorouter.autoroutePassMultiThread` (`:411-413`), which has **no
     caller anywhere in the tree** — the live pass path is
     `AutorouteBatchLoop.java:293` → `:598-600` →
     `BatchAutorouter.autoroutePass` (`:419-421`) → `runSingleThread`. So `-mt`
     is normalised arithmetic on a value headless Java never consumes
     (`docs/java-quirks.md` row 143).

   Plans 6/7 must therefore keep `optimizer.max_threads` and `router.max_threads`
   as distinct fields with distinct normalisations, must not "helpfully" map
   `-mt 0` to the core count, and must **not** build a headless multi-threading
   policy on `-mt` — it has no Java counterpart, and inventing one makes the port
   more capable than Java (see `docs/plan-4-handoff.md` §10).

3. **`-oit` divides before it clamps**, and parses as `float`. A zero
   percentage collapses to exactly `0.0f`. A *negative* one never gets that
   far: the blanket value rule refuses any argument starting with `-`, so
   `-oit -5` is a silent no-op and the clamp's negative arm is dead code
   (JVM-verified, Plan 4 Task 7 — `docs/java-quirks.md` row 135).

4. **`-us` / `-is` never reject a value.** Both are total functions onto an
   enum with a fixed default (`GREEDY`, `PRIORITIZED`). A typo silently
   changes the routing strategy.

5. **`-inc` does not trim its comma-separated entries**, unlike the otherwise
   parallel `debug.filter_by_net` handling three hundred lines above it.

## `-de` file classification

`-de` is the one flag whose *shape* the shim reproduces rather than forwards
(`GlobalSettings.java:564-648`):

- It consumes **every** following argument that does not start with `-`, not
  just one.
- Each argument is `trim()`ed. If it names an **existing** path it is taken
  verbatim; otherwise, if it contains `+`, it is split on `+` (legacy
  concatenation) with empty parts dropped. So a real file whose name contains
  `+` survives (`GlobalSettings.java:570-580`).
- Each resulting file is classified by lower-cased extension: `.dsn` → design
  input, `.ses` → session, `.rules` → rules, `.json` → session *or* design
  input (below). Filling a slot twice logs "Only the last one will be used" and
  keeps the last (`:601-608`, `:609-621`, `:622-629`, `:630-637`).
- **Any other extension is warned about and dropped** (`:638-644`) — it does
  *not* fall back to the design-input slot.

**The `.json` slot: ruling 14, and the divergence that is gone.** Java has no
dedicated KiCad-JSON slot. A `.json` goes to `initialInputFile` (the design
input) when no `.dsn` has been seen yet, and to `designSessionFilename` (the
session) otherwise (`GlobalSettings.java:609-621`). **The port now follows that
rule exactly**, on the legacy form.

Until Plan 8 Task 5 it did not. This file used to say:

> ~~**One deliberate divergence.** … The port gives it its own `--kicad-json`
> option on `route`/`drc` so a KiCad board file never silently poses as a SES
> session. Two consequences for Plan 8, which owns the loader: `-de board.json
> -do out.ses` routes in Java but currently fails in the port with `-de input
> must include a .dsn file`, because `--kicad-json` does not yet feed the
> design-input slot. `-de a.dsn prev.json` yields `--kicad-json prev.json` where
> Java would treat `prev.json` as the previous session. Both are
> unimplemented-loader gaps, not silent misroutes; resolve them when the KiCad
> JSON reader lands.~~

The **argument** half landed in Plan 8 Task 3 (`fr_core::load_board_if_needed`
sniffs the format from the bytes), so the reason for the divergence expired, and
**plan ruling 14 closed it**: on the legacy form a `.json` fills Java's slot, and
both of those rows are `MATCH` in the `p8t5` differential (`de-json-first`,
`de-json-after-dsn`). `--kicad-json` survives on the **native** subcommand form
only, where it is the port's own spelling and nothing has to guess.

**The loader half landed in Plan 8 Task 9**, and this is where the sentence above
was overdrawn until then. Task 3's `fr_core::load::kicad_read_board` was an inert
stub that answered `ParseError("(kicad_json", "the KiCad JSON reader is not
ported yet (Plan 8 Task 9)")`, so a `.json` reached the right slot and *then*
failed in the reader: `-de board.json -do out.ses` still routed in the jar and
still failed in the port, one layer further in than the strikethrough above
describes. Task 9 completed `fr_dsn::kicad::read_board`'s sections 9-11 and
pointed the stub at it, and the gate is now permanent — two stems in
`tests/reference/cli-fixtures.txt` run the whole program on a KiCad export:

| stem | lane | argv | verdict |
|---|---|---|---|
| `kicad-ecc83-json` | ci | `-de fixtures/Issue649-kicad_ecc83-pp_input_board_v1.json -do <out>.ses -mp 3 --router.fanout.enabled=true --router.optimizer.enabled=true` | `MATCH  exit 0, 3841 B SES, log 0 lines` |
| `kicad-complex-hierarchy-json` | slow | `-de fixtures/Issue733-kicad_complex_hierarchy_input_design.json -do <out>.ses` (bare) | `MATCH  exit 0, 28409 B SES, log 0 lines` |

Byte-identical SES, equal exit code, equal `parity::normalize_log`, on the same
`scripts/differential/run.sh p8t1` rungs every DSN stem uses. There is no
`--kicad-json` gap left on either form.

**The output half landed in Plan 8 Task 10.** `-do out.json` is accepted by
`tryToSetOutputFile:384-388` (a `.json` extension classifies as
`KICAD_DESIGN_JSON`, which `:391` rewrites to `KICAD_SESSION_JSON`), so unlike
`-do out.dsn`/`out.scr` it really is serialised and the run exits 0. What it
serialises is **quirk #289 (label T)**: `setJobOutput` is both a board-updated
listener and a once-only call after the pipeline, and only the *first* of those
calls ever writes, because `setData` re-sniffs the JSON back to
`KICAD_DESIGN_JSON` and every later call then matches neither branch. Measured
on three boards at the pinned jar — see the register row — the file therefore
holds the board **as loaded, before any routing**. The port reproduces that
exactly: `commands/route.rs` takes the `fr_dsn::kicad::write` snapshot before
`RoutingPipeline::run`, and
`crates/freerouting/tests/cli_e2e.rs::do_out_json_writes_the_pre_routing_board`
pins the jar's own 1 540 bytes as a literal.

**The `.json` *session* slot landed with it.** A second `.json` on a `-de` line
(`GlobalSettings.java:609-621`) is the previous session, and its `.json` arm now
goes through `fr_dsn::kicad::import_session` on both the router path
(`RoutingJobScheduler.java:197-207`) and the DRC path
(`Freerouting.java:301-307`). Quirk **#290** (label U) records that Java opens
it with the platform default charset; on JDK 18+ that is UTF-8 and the port
agrees byte for byte.

## What `legacy.rs` does and does not do, after Plan 8 Task 5

`crates/freerouting/src/legacy.rs` answers three questions and nothing else:
which subcommand the line names, which files fill Java's four slots, and which
`FRLogger` lines Java would have emitted. In particular:

- **It forwards no values.** `-mp`, `-mt`, `-oit`, `-us`, `-is`, `-hr` and
  `-inc` are consumed exactly the way Java consumes them and then dropped. The
  settings that reach the router come from `fr_settings::CliSettings` over the
  **raw** argv, because that is Java's *second* parser and it matches `-mp`/`-mt`
  with an exact `switch` where the flag table matches by prefix (scan ruling
  R19). Translating `-mpx 5` into `--max-passes 5` here would set `max_passes`,
  which **no Java parser does**.
- **Nothing fails.** An unknown flag warns and continues (`:561`, `:833`); a
  malformed number logs at `:836` and leaves its own value token to be warned
  about again; a missing value is a silent no-op. Every refusal on this path is
  Java's own — `initializeCli`'s "Both an input file and an output file must be
  specified…" (`Freerouting.java:80-86`) or `initializeDrc`'s "An input file
  must be specified with -de argument in DRC mode." (`:247-250`) — and both are
  **exit 1**. Exit **2** is clap's usage error on the *native* form and exit
  **3** is the not-yet-wired subcommand; neither can be reached from a Java
  command line (ruling AR).
- **A bare `-drc` is not DRC mode.** `:660-663` sets two dead booleans before it
  looks for a report path, but `main:1462` enters DRC mode on `drcReportFile !=
  null` alone, so `-drc` with no path falls through to the CLI branch and dies
  there (`docs/java-quirks.md` #263).
- **`-di` is accepted, consumed and warned about.** It names a GUI input
  directory; there is no GUI. Consuming it matters — Java consumes it, and the
  rest of the line parses differently otherwise.
- **There is no `-v`.** Java's log-level flag is `-ll` (`docs/java-quirks.md`
  #260). `-v`/`--verbose` and `--log-level` exist on the **native** form only,
  and the help text says so.
- **There is no `--version` either, and the port does not invent one on this
  path.** The jar answers `--version` and `-V` with
  `Unknown command line argument: …` and then `initializeCli`'s refusal — **exit
  1**, measured on the HEAD jar and pinned by `p8t5`'s `version-long` and
  `version-short` rows. Controller ruling BF applied ruling AR: a command line
  the jar refuses must not come back with a code the jar cannot produce, so the
  legacy path has no `--version` arm and clap's lives behind a subcommand
  (`freerouting route --version`).
- **The port-only flag list lives in `crates/freerouting/README.md`**, one table,
  with the exit ladder and the `--set` note beside it.
