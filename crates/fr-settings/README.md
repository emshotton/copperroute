# fr-settings

A behavioral Rust port of freerouting's router-configuration layer (clone HEAD):
`settings/{RouterSettings,LayerSettings,ScoringSettings,OptimizerSettings,
FanoutSettings,DesignRulesCheckerSettings,DebugSettings,SettingsSource,
SettingsMerger}.java`, the in-scope `settings/sources/**` classes,
`util/ReflectionUtil.java`, the `util/gson` JSON configuration, and
`autoroute/{BoardUpdateStrategy,ItemSelectionStrategy}.java`. It answers one
question — *given a DSN file, a `.rules` file, the environment, the command
line and a board, what settings does the router actually run with?* — and
answers it the way Java does, bug for bug. Use `fr_settings::prelude::*` to
bring in every public type.

The crate sits on `fr-dsn` (which owns the `.dsn` and `.rules` readers) and on
`fr-board` (for the one method that reads a board). It depends on `fr-dsn`,
`fr-board`, `fr-geometry`, `serde`, `serde_json` and `thiserror` — and on
nothing else (plan ruling 3). **No `tracing`:** Java's `FRLogger.warn`/`error`
calls in the merge path either become a `MergeReport::errors` entry, where Java
swallows an exception, or vanish. No GUI, no `PropertyChangeSupport`, no static
mutable state.

Deliberate Java bugs are reproduced rather than fixed; each carries a
`// Java bug:` or `// totalized:` marker at the site and a row in
`docs/java-quirks.md`. Rows **114-142** are this crate's.

## Spec §11's precedence order is wrong. Java's is what this crate implements

The spec says `defaults → DSN → SES → .rules → env → CLI`. That is not what
headless freerouting runs, and following it silently mis-orders a `.rules`
file against `--router.*` flags — a wrong-output bug with no crash to find it.
What Java runs (plan ruling 1, re-read at the clone's HEAD) is **two whole
merges** with a board-tuning pass wedged between them and a `.rules` re-apply
after the second:

| # | Java | what it does |
|---|---|---|
| 1 | `Freerouting.java:1408-1413` | the prototype merger: `DefaultSettings(0)`, `JsonFileSettings(10)`, `CliSettings(60)`, `EnvironmentVariablesSource(55)` |
| 2 | `Freerouting.java:125-136` | clone it; add `DsnFileSettings(20)`, and `RulesFileSettings(40)` for `-dr` / `-de …rules` |
| 3 | `Freerouting.java:146` → `SettingsMerger.java:133-193` | **merge #1**: sort ascending, `clone()` the first non-null source, `applyNewValuesFrom` the rest, `validate()` |
| 4 | `RoutingJobScheduler.java:93-96` → `HeadlessBoardManager.java:739-748` | `setLayerCount(board)` when it disagrees, then `applyBoardSpecificOptimizations(board)` |
| 5 | `RoutingJobScheduler.java:103-166` | clone the prototype **again**; re-add the DSN, add the rules the scheduler chose, add `new ApiSettings(job.routerSettings)` at **priority 70** — the whole result of steps 3-4 |
| 6 | `RoutingJobScheduler.java:170` | **merge #2**, with its own `validate()` |
| 7 | `:173-184` → `RulesReader.java:153-157` | re-parse the `.rules` bytes and `applyNewValuesFrom` them onto the **merged** object |
| 8 | `:186` | `applyBoardSpecificOptimizations(board)` |

Merge #1's result is *complete* — every field is non-null once
`DefaultSettings` has run — so at priority 70 it beats everything merge #2's
own `0..60` chain contributes, **except where it left a field null**. That is
the only channel merge #2's own sources have (`layers[i].preferredDirectionHorizontal`,
`layers[i].bendCost`, `resultJsonPath`, `optimizer.timeoutString`,
`fanout.timeoutString`), and it is what `fill_absent_from` expresses. Step 7 is
a different matter: a plain `applyNewValuesFrom` onto the finished object, so
for every field an `(autoroute_settings)` block can carry, **the `.rules` file
outranks the environment and the command line** — the opposite of the spec.

`resolve_headless` is that sequence as one linear pass:

```text
s = DefaultSettings                                  // 0
s.apply(dsn)                                         // 20
s.apply(parse(cli_rules))                            // 40   ( -dr / -de …rules only )
s.apply(env)                                         // 55
s.apply(cli)                                         // 60
s.validate()                                         // merge #1's validate == the priority-70 payload
s.set_layer_count(board) if it disagrees             // HeadlessBoardManager.java:741-744
s.apply_board_specific_optimizations(board)          // HeadlessBoardManager.java:745
s.board_specific_trace_costs_applied = None          // the private flag never survives merge #2 (#127)
s.fill_absent_from(parse(scheduler_rules))           // merge #2's own 0..60 chain, all of it
s.validate()                                         // merge #2's validate  (#140: not idempotent)
s.apply(parse_against(scheduler_rules, board))       // RulesReader.java:112, :153-157  (#142)
s.apply_board_specific_optimizations(board)          // RoutingJobScheduler.java:186
```

Three quirk numbers to keep in view while reading it:

- **#142 — one `.rules` file, two parses.** `parse` is
  `RulesReader.readRouterSettings`, whose layer structure is discovered from
  the file's own `(layer_rule …)` names; `parse_against` is
  `RulesReader.read`, whose structure is the **board's**. A
  two-`layer_rule` file on a four-layer board puts `B.Cu` at index 3 in one and
  index 1 in the other. That is why the two rules slots of `SettingsInputs` are
  **byte slices**, not parsed objects — handing one parsed object to both steps
  disagreed with the JVM on 13 of the differential's 84 rows.
- **#127 — `boardSpecificTraceCostsApplied` cannot cross a merge.** It is
  `private transient`, and `copyFields` skips non-`public` fields
  (`ReflectionUtil.java:226-228`), so merge #2's fresh object always carries a
  null flag — which is exactly why step 8 re-initialises the costs the merge
  just carried across. The linear pass keeps one object, so it clears the flag
  by hand at the point where Java changes objects.
- **#140 — `validate()` is not idempotent.** `max_passes == 0` becomes
  `Integer.MAX_VALUE` on the first call (`RouterSettings.java:937-940`) and
  `9999` on the second, because `MAX_VALUE > 9999`.

**Where the linearisation stops being exact.** `resolve_headless(.., None, ..)`
— no board — can disagree with Java's two merges in exactly one family: no DSN
source, and merge #1's `-dr` rules different from merge #2's `job.rules`.
`copyFields` rule 5 is first-writer-wins for primitive arrays, and merge #2
restarts that race from a fresh `DefaultSettings.clone()`. Java never sees it:
a job with no board never reaches `RoutingJobScheduler.java:173` or `:186`, and
`:186` re-derives both arrays anyway.

**`resolve_headless` models the CLI-started path and only that.** An API job
never runs merge #1, so its priority-70 `ApiSettings` payload is *sparse* and
merge #2's own chain does the real work — the opposite premise. That path needs
a different composition and is Plan 8's (`obligation:` on `resolve_headless`).

## API surface

| What you want | Call |
|---|---|
| the whole headless answer | `resolve_headless(&SettingsInputs { .. }, board, &HostEnvironment)` |
| which `.rules` the scheduler would pick | `resolve_scheduler_rules_path(job_rules, cli_rules, dsn_path)` (`_with` injects the `File.exists()` probes) |
| one merge over an arbitrary source list | `SettingsMerger::new(Vec<Box<dyn SettingsSource>>).merge(&host)` |
| one source | `DefaultSettings`, `DsnFileSettings`, `RulesFileSettings`, `SesFileSettings`, `ApiSettings`, `EnvironmentVariablesSource`, `CliSettings` |
| `--router.a.b=v` / `FREEROUTING__ROUTER__A__B=v` | `set_field_value(&mut RouterSettings, path, value) -> Result<(), MergeError>` |
| Gson-compatible JSON | `RouterSettings::from_json_str` / `::to_json_string_pretty` |
| the legacy short flags | `apply_command_line_arguments(&[String]) -> LegacyBridge`, `classify_de_arguments` |

### `SettingsInputs` is half parsed objects and half raw bytes

```rust,ignore
pub struct SettingsInputs<'a> {
    pub dsn:             Option<&'a RouterSettings>,  // priority 20 — parsed once, used once
    pub cli_rules:       Option<&'a [u8]>,            // priority 40, merge #1 only
    pub scheduler_rules: Option<&'a [u8]>,            // priority 40 in merge #2 *and* step 7
    pub env:             Option<&'a RouterSettings>,  // priority 55
    pub cli:             Option<&'a RouterSettings>,  // priority 60
}
```

The asymmetry is not an oversight, and `cli_rules` is bytes only for symmetry
with its neighbour: `scheduler_rules` **must** be bytes, because Java reads
that file twice against two different layer structures (quirk #142) and
`resolve_headless` performs both parses itself. Feed it
`std::fs::read(path)`, not a `RulesFileSettings`.

`None` means "Java registered no such source". The `dsn` slot is `None` only
for a non-DSN input, and even then merge #1 registers a `DsnFileSettings` that
happens to contribute nothing — see `tests/matrix/mod.rs`'s reachability table.

### Every field is `Option<T>`, and that is the merge protocol

Java's boxed wrappers start `null`, and `SettingsMerger.java:22-31` reads
`null` as "this source has no opinion", not as "off". Flattening the
`Option`s into defaults at the struct level would destroy the protocol, so
`RouterSettings` and its four nested value types are `Option` throughout and
the null-coalescing accessors (`get_max_passes`, `get_layer_active`,
`get_bend_cost`, …) apply Java's defaults only at the point Java applies them.

`ReflectionUtil.copyFields` is transcribed as a `CopyFields` impl per struct,
in **Java declaration order**, following its eight rules verbatim — including
the two that decide routing behaviour:

- **Primitive and `String` arrays copy only into a `null`-or-empty target**
  (`:269-290`) — first writer wins. Combined with quirk #128 (the DSN source
  seeds both `scoring` cost arrays with all-`1.0` as a side effect of seeding
  the layer count), this is why a `.rules` file's per-layer trace costs never
  reach the router through the merge.
- **Object arrays merge element-wise and are never shrunk** (`:291-327`), which
  is how a `.rules` file's per-layer `preferredDirectionHorizontal` *does* get
  through.

The `copyFields` **change count** is deliberately not reproduced field for
field (plan ruling 2, quirk #117): Java counts boxed-reference identity, which
Rust has no way to have. No caller reads it, and the differential does not
compare it.

### Defaults, clamps, and the injectable host

`DefaultSettings::new(&host)` is `DefaultSettings.getSettings()` (`:96-155`)
transcribed in Java's assignment order, every value JVM-pinned by
`tests/sources.rs::default_settings_pins_every_java_value`. The headline
constants are public (`DEFAULT_VIA_COSTS = 50`, `DEFAULT_PLANE_VIA_COSTS = 5`,
`DEFAULT_START_RIPUP_COSTS = 100`, `DEFAULT_BEND_PENALTY = 10.0`,
`DEFAULT_UNROUTED_NET_PENALTY = 5e6`,
`DEFAULT_CLEARANCE_VIOLATION_PENALTY = 1e6`,
`DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM = 500.0`,
`DEFAULT_HOLE_CLEARANCE_UM = 0.0`). `layers` and the two cost arrays are
deliberately left absent — their size depends on the board.

The clamps, all on the *setter* rather than the field:

| Setter | Clamp | Java |
|---|---|---|
| `set_bend_cost` | `[MIN_BEND_COST, MAX_BEND_COST]` = `[0.0, 9.9]` | `:675-690` |
| `get_bend_cost` | clamps the **fallback** path only, not the explicit value | `:692-703` |
| `set_via_costs`, `set_plane_via_costs`, `set_start_ripup_costs` | `max(value, 1)` | `:605-616`, `:618-629`, `:542-547` |
| `set_preferred_direction_trace_costs`, `set_against_…` | `max(value, 0.1)`, and set the applied flag | `:757-777` (`:776`), `:838-853` (`:857-858` in the sibling) |
| `validate` | `max_passes` outside `0..=9999` → `9999`; `== 0` → `i32::MAX`; `max_threads` absent/negative → default, above the core count → the core count; `trace_pull_tight_accuracy < 1` → `500` | `:932-965` |

Every machine-dependent number goes through `HostEnvironment` (plan ruling 6).
`HostEnvironment::detect()` calls `std::thread::available_parallelism()` once —
it is the only such call in the crate — and `HostEnvironment::with_processors(n)`
is what every test and the differential use. It matters: `validate()` and
`normalizeMaxThreads` disagree about `maxThreads == 0` (quirk #124), so the
processor count is observable in two different ways.

## Ported, and not

| Java | Here |
|---|---|
| `RouterSettings` (+ `clone`, `validate`, `applyNewValuesFrom`, the accessors and clamps) | `router_settings.rs` — `clone()` is `java_clone`, **not** the derived `Clone` |
| `applyBoardSpecificOptimizations` + the two companions | `board_optimizations.rs` (the only part that reads a `fr_board::Board`) |
| `LayerSettings`, `ScoringSettings`, `OptimizerSettings`, `FanoutSettings` | one module each |
| `DesignRulesCheckerSettings`, `DebugSettings` | `drc_settings.rs` |
| `ReflectionUtil.copyFields` | `copy_fields.rs` — a `CopyFields` impl per struct, plus the inverted `MergeMode::FillAbsent` |
| `ReflectionUtil.setFieldValue` and its `convertValue` grammar | `field_path.rs` |
| `SettingsSource`, `SettingsMerger`, the priority ladder | `merger.rs` |
| `DefaultSettings`, `DsnFileSettings`, `RulesFileSettings`, `SesFileSettings`, `ApiSettings`, `EnvironmentVariablesSource`, `CliSettings` | `sources/` |
| `GlobalSettings.applyCommandLineArguments` | `sources/cli.rs` — the legacy flag table, on a `LegacyBridge` **nothing reads** (ruling 8) |
| `autoroute/{BoardUpdateStrategy,ItemSelectionStrategy}` | `optimizer_settings.rs` |
| `util/gson/{GsonProvider,RouterSettingsTypeAdapterFactory}` | `json.rs` |
| `Freerouting.java` + `RoutingJobScheduler`'s merge sequence | `resolve.rs` (`resolve_headless`) |
| `JsonFileSettings` (priority 10) | **not ported** — spec §2, no persistent config file; the number and identity are reserved |
| `GuiSettingsSource` (priority **65**, not the javadoc's 50 — quirk #138) | **not ported** — no GUI; number and identity reserved |
| `SesFileSettings` | ported, and it does nothing: Java's `getSettings()` returns a bare `new RouterSettings()` and it is never registered headless (quirk #130) |
| the other fifteen `settings/**` classes (GUI, REST API, HTTP MCP, telemetry, paths, logging) | **not ported** — one `// not ported:` line each, with its reason, at the foot of `src/lib.rs` |
| `management/jobs/**`, `management/sessions/**` | **not ported** — the job queue is out of scope; the merge sequence *inside* the scheduler is what `resolve_headless` reproduces |
| `util/TextManager.parseTimespanString` | **Plan 8** — the three timeout values are carried as `String`s, as Java carries them, and parsed where Java parses them (`RoutingJobSchedulerActionThread.java:44`, capped at 24 h) |
| the five legacy short flags `-oit`/`-us`/`-is`/`-hr`/`-inc`, and `-drc`'s router switch-off | ported **dead**, exactly as Java leaves them (ruling 8, quirk #131) |
| `schemars` / MCP tool schemas | Plan 8 |

The complete roster, with a reason per class and per method, is the comment
block at the end of `crates/fr-settings/src/lib.rs` and the `GlobalSettings`
block at the end of `crates/fr-settings/src/sources/cli.rs`. It is not
decoration: `scripts/audit-port.sh` reads those markers, so a deferral is a
gate rather than a silence.

## Tests

`cargo test -p fr-settings` runs the unit tests plus eleven integration
suites: `copy_fields`, `field_path`, `router_settings`, `board_optimizations`,
`sources`, `env_source`, `cli_source`, `precedence`, `json`, `struct_shape`
and `corpus`.

- **`struct_shape.rs`** pins every struct's field order against Java's
  `getDeclaredFields()`, because `copy_fields` iterates it and `json.rs` emits
  it.
- **`precedence.rs`** is the load-bearing one. It runs a 64-case matrix
  (`tests/matrix/mod.rs`: 4 DSN shapes × 4 rules shapes × 2 environments × 2
  command lines) through **both** forms of Java's composition — the literal
  two-merge shape built out of `SettingsMerger`, and the linear
  `resolve_headless` — and asserts they agree field for field. That is ruling
  1's equivalence proved a second time, in-process; `p4t1` proves it against
  the JVM.
- **`json.rs`** holds `p4t1_mode_1_parity`, which replays 64 of the
  differential's 84 rows against the committed Java transcript in
  `tests/golden/p4t1-mode1/all.txt` — so Gson parity survives without a JVM.
- **`corpus.rs`** runs `DsnFileSettings::new` over every `.dsn` in the fixture
  corpus (148 files, ~2.7 s in debug, **not** `#[ignore]`d) and asserts the
  source's layer count is exactly what the wrapped `readMetadata` reported.
  Its `.rules` sibling is `#[cfg_attr(debug_assertions, ignore)]`; run it with
  `cargo test -p fr-settings --release --test corpus`.

### The JVM probes, and how to regenerate an expectation

Every expected value in this crate came out of a JUnit-free Java driver in
`tests/data/` (`FProbe`, `DProbe`, `TProbe`, `VProbe`, `BProbe`, `RProbe`,
`CProbe`, `PProbe`, `JProbe`, `SProbe`), run against the clone's HEAD jar.
`tests/data/README.md` says what each one probes and carries the exact command
line for each — including which ones need `-XX:ActiveProcessorCount=4` to match
`HostEnvironment::with_processors(4)`. The transcripts live in the task
reports under `.superpowers/sdd/2026-08-28-plan-4-settings/`.

The two committed `.rules` inputs (`Plan4Matrix-primary.rules`,
`Plan4Matrix-adjacent.rules`) and the trimmed
`Issue029-hw48na_reduced.rules` are described there too. They are committed
rather than generated so the JVM transcript and the Rust test read the same
bytes.

### What needs the sibling checkout

`../freerouting` (or `$FREEROUTING_JAVA_DIR`) supplies the DSN and `.rules`
fixtures; it is not vendored. Three behaviours:

- **`corpus.rs`, `precedence.rs`, `sources.rs` and `json.rs` skip with a
  printed message** — each fixture-reading test calls
  `parity::require_java_dir()` first and returns.
- **`board_optimizations.rs`'s three `jvm_golden_*` tests panic**: they read
  through `fixture_board`, which resolves via `parity::fixture` (so it *does*
  honour `FREEROUTING_JAVA_DIR`) but has no skip guard. Its synthetic-board
  tests — the majority — need nothing.
- **Everything else needs no checkout at all**: `copy_fields`, `field_path`,
  `router_settings`, `env_source`, `cli_source` and `struct_shape` are pure.

The **jar** (`../freerouting/build/libs/freerouting-current-executable.jar`)
is needed only by the probes and by `p4t1`, never by `cargo test`.

## The `p4t1` differential

`scripts/differential/java/P4T1.java` runs Java's **real** two-merge
composition — the actual `DefaultSettings`, `JsonFileSettings`, `CliSettings`,
`EnvironmentVariablesSource`, `DsnFileSettings`, `RulesFileSettings`,
`ApiSettings`, `SettingsMerger`, `RulesReader.read` and
`RouterSettings.applyBoardSpecificOptimizations` out of the clone's HEAD jar —
over the 84-row case table `scripts/differential/matrix/p4t1-cases.tsv`, and
`scripts/differential/rust/src/bin/p4t1.rs` runs `resolve_headless` over the
same table. Both print one line of state per field and the outputs are diffed
byte for byte.

```sh
./scripts/differential/run.sh p4t1 scripts/differential/matrix/p4t1-cases.tsv all 0   # 5728 lines
./scripts/differential/run.sh p4t1 scripts/differential/matrix/p4t1-cases.tsv all 1   # 4287 lines
./scripts/differential/run.sh p4t1 scripts/differential/matrix/p4t1-cases.tsv 8   2   # 5728 lines
```

- **mode 0** — the canonical dump: one sorted `path=value` line per field,
  transients included, arrays as a `.length` line plus one line per element so
  `null` and empty stay distinguishable.
- **mode 1** — the same object through `GsonProvider.GSON` versus
  `RouterSettings::to_json_string_pretty`.
- **mode 2** — every case, whatever the case index says; `all 0` and
  `<anything> 2` are the same run.

All three are **0 diffs over all 84 cases**. Needs a JDK 25 (`JAVA25_HOME`) and
the clone's HEAD jar (`FREEROUTING_JAR`); the processor count is pinned with
`-XX:ActiveProcessorCount=4` against `HostEnvironment::with_processors(4)`, and
both sides print the jar's path, size and mtime in a header line, so running
against the wrong build is a diff rather than a silent assumption. The second
header line is `JSON_SOURCE_EMPTY`: the Java side builds a real
`JsonFileSettings` on an empty temporary directory and aborts unless every leaf
is null, which turns spec §2's "no persistent config file" into a check.

`scripts/differential/README.md` has the case-table format, the transcription
risk and how to re-check it, and the golden regeneration command.

## Audit

`scripts/audit-port.sh` verifies the crate has no unaccounted-for public Java
method. Four invocations, all with the per-class map, all exit 0 with no
`MISSING` and no `UNMAPPED`:

```sh
./scripts/audit-port.sh settings crates/fr-settings/src \
    'RouterSettings.java LayerSettings.java ScoringSettings.java OptimizerSettings.java \
     FanoutSettings.java DesignRulesCheckerSettings.java DebugSettings.java \
     SettingsSource.java SettingsMerger.java GlobalSettings.java' \
    scripts/audit-map/fr-settings.map
./scripts/audit-port.sh settings/sources crates/fr-settings/src '*.java' scripts/audit-map/fr-settings.map
./scripts/audit-port.sh util           crates/fr-settings/src 'ReflectionUtil.java' scripts/audit-map/fr-settings.map
./scripts/audit-port.sh util/gson      crates/fr-settings/src '*.java' scripts/audit-map/fr-settings.map
```

`GlobalSettings.java` alone contributes eighteen deferrals — seventeen public
methods the audit gates, plus the constructor, which `audit-port.sh` skips and
the roster documents anyway. It is the whole application's configuration
object, and exactly one of its nineteen public members
(`applyCommandLineArguments`) is a router setting. They are closed by name in
`src/sources/cli.rs`, never by weakening the script or the map.
