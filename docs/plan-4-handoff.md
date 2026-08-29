# Plan 4 hand-off — router settings, the merge engine and the settings sources (`fr-settings`)

Branch `plan-4-settings`, 22 implementation commits on top of `main` (`462d744`,
Plan 3 merged 2026-08-28) plus the plan document itself (`fb15aac`) — see `git log`
for the tip, which includes the whole-branch final-review fix wave this document
is itself part of.

**Read this before Plans 5–8.** Plan 5 owns `DesignRulesCheckerSettings`'
consumer side, Plans 6/7 read every routing cost this crate computes, and Plan 8
wires the two pure functions this plan built for it and inherits three product
decisions it must make rather than inherit. Section 1 is the one thing to read
even if nothing else here is read: **the spec's stated merge order is wrong, and
this crate deliberately does not implement it.**

---

## 1. Spec §11's merge order is wrong. This is what Java does

Spec §11 says the merge order is `defaults → DSN → SES → .rules → env → CLI`.
**Java does not do that**, and a port that follows the spec silently mis-orders a
`.rules` file against `--router.*` flags — a wrong-output bug with no crash to
find it. Plan ruling 1 replaced the spec's order with Java's, re-read at the
clone's HEAD; Task 9's JVM differential then proved the replacement over 84
cases, and controller ruling N corrected it once more.

What headless freerouting actually runs is **two whole merges**, with a
board-tuning pass wedged between them and a `.rules` re-apply after the second:

| # | Java | what it does |
|---|---|---|
| 1 | `Freerouting.java:1408-1413` | the prototype merger: `DefaultSettings(0)`, `JsonFileSettings(10)`, `CliSettings(60)`, `EnvironmentVariablesSource(55)` |
| 2 | `Freerouting.java:125-136` | clone it; add `DsnFileSettings(20)`, and `RulesFileSettings(40)` for `-dr` / `-de …rules` |
| 3 | `Freerouting.java:146` → `SettingsMerger.java:133-193` | **merge #1**: sort ascending by priority, `clone()` the first non-null source as the base, `applyNewValuesFrom` the rest, `validate()` |
| 4 | `RoutingJobScheduler.java:93-96` → `HeadlessBoardManager.java:739-748` | `setLayerCount(board)` when it disagrees, then `applyBoardSpecificOptimizations(board)` |
| 5 | `RoutingJobScheduler.java:103-166` | clone the prototype **again**; re-add the DSN, add the rules the scheduler chose, add `new ApiSettings(job.routerSettings)` at **priority 70** — the whole result of steps 3–4 |
| 6 | `RoutingJobScheduler.java:170` | **merge #2**, with its own `validate()` |
| 7 | `:173-184` → `RulesReader.java:153-157` | re-parse the `.rules` bytes and `applyNewValuesFrom` them onto the **already-merged** object |
| 8 | `:186` | `applyBoardSpecificOptimizations(board)` again |

`fr_settings::resolve_headless` is that sequence as one linear pass:

```text
s = DefaultSettings                                  // priority 0
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

Merge #1's result is *complete* — every field is non-null once `DefaultSettings`
has run — so at priority 70 it beats everything merge #2's own `0..60` chain
contributes, **except where merge #1 left a field null**. That set is exactly
`layers[i].preferredDirectionHorizontal`, `layers[i].bendCost`, `resultJsonPath`,
`optimizer.timeoutString` and `fanout.timeoutString`, and it is what
`fill_absent_from` expresses. (**This corrects the Java survey's Q1**, which said
the scheduler's auto-discovered adjacent `.rules` "contributes nothing through
the merger": it contributes precisely those fields.)

### The five consequences, plainly

1. **A `.rules` file outranks the environment and the command line.** Step 7 is a
   plain `applyNewValuesFrom` onto the finished object, so for every field an
   `(autoroute_settings)` block can carry — `vias`, `via_costs`,
   `plane_via_costs`, `start_ripup_costs`, `autoroute`/`postroute` and the
   per-layer rules — the file wins over `FREEROUTING__ROUTER__…` and over
   `--router.…`. The spec has it the other way round. The mechanism is quirk
   #142's second `.rules` parse (`RulesReader.java:153-157`); #127 and #128
   describe what a merge can and cannot overwrite once a value has landed.
2. **The spec's SES tier has no behaviour.** `SesFileSettings.getSettings()`
   returns a bare `new RouterSettings()` unconditionally
   (`SesFileSettings.java:27-36`) and the class is never registered in the
   headless path at all. It is ported, and it does nothing (quirk #130).
   `priority::SES_FILE == 30` is reserved so nobody reuses the number.
3. **Five legacy short flags are dead in Java, and dead here.**
   `-oit`, `-us`, `-is`, `-hr`, `-inc` — and `-drc`'s
   `routerSettings.enabled = false` — write only the `@Deprecated public final
   RouterSettings routerSettings` bridge on `GlobalSettings` (`:51-53`, `:662`,
   `:700-736`, `:810-815`), and `CliSettings.mapFlagToProperty` maps **only**
   `mp` and `mt` (`:102-110`). No routing path reads the bridge. The port parses
   them, applies the exact normalisation in `docs/cli-legacy-flags.md`, stores
   them on `LegacyBridge`, and **no `resolve_headless` input reads that struct**
   (plan ruling 8, quirk #131). Wiring them live would make the port *more
   capable than Java* — a product decision handed to Plan 8, not a bug fix.
4. **The scheduler's `.rules` file is parsed twice, against two different layer
   structures** (quirk #142, controller ruling N). At priority 40 it goes through
   `RulesReader.readRouterSettings`, whose layer structure is discovered from the
   file's own `(layer_rule …)` names (`RulesReader.java:238-273`); after the merge
   it goes through `RulesReader.read(…, board, settings)`, whose structure is the
   **board's** (`:112`). A two-`layer_rule` file on a four-layer board puts `B.Cu`
   at index 1 in the first parse and index 3 in the second. This is why
   `SettingsInputs`'s two rules slots are **byte slices**, not parsed objects:
   handing one parsed object to both steps diverged from the JVM on 13 of the
   differential's 84 rows.
5. **`validate()` is not idempotent, and the headless path calls it twice**
   (quirk #140). `max_passes == 0` becomes `Integer.MAX_VALUE` on the first call
   (`RouterSettings.java:937-940`) and `9999` on the second, because
   `MAX_VALUE > 9999`. So `--router.max_passes=0` — documented as "unlimited" —
   ends the headless path as 9999.

### Where the linearisation stops being exact

`resolve_headless(.., None, ..)` — no board — can disagree with Java's two merges
in exactly one family: **no DSN source, and merge #1's `-dr` rules different from
merge #2's `job.rules`**. `copyFields` rule 5 is first-writer-wins for primitive
arrays and merge #2 restarts that race from a fresh `DefaultSettings.clone()`.
Java never sees it: a job with no board never reaches
`RoutingJobScheduler.java:173` or `:186`, and `:186` re-derives both arrays
anyway. Documented at `resolve.rs` and in `crates/fr-settings/README.md`.

### `resolve_headless` models the CLI-started path and only that

An **API job never runs merge #1**: `job.routerSettings` is
`new RouterSettings()` (`core/RoutingJob.java:105`), the request body
(`api/v1/JobInputResource.java:203-211`), or a bare `setLayerCount`
(`:337-339`, `:540-542`). So `ApiSettings` at priority 70 is a *sparse* override
and merge #2's own `0..60` chain does the real work — the opposite premise from
the one the linear form is built on. Plan 8 owns the API surface and must compose
that path out of `SettingsMerger` directly. Carried as an `obligation:` marker on
`resolve.rs:184` and repeated under Plan 8's obligations below.

---

## 2. Delivered

- **`fr-settings`**: 7.2k lines of source across `router_settings`,
  `layer_settings`, `scoring_settings`, `optimizer_settings`, `fanout_settings`,
  `drc_settings`, `copy_fields` (the `ReflectionUtil.copyFields` engine),
  `field_path` (`ReflectionUtil.setFieldValue` and its `convertValue` grammar),
  `board_optimizations`, `merger`, `sources/**` (seven sources plus the
  `DsnRouterSettings` conversion pair), `json` (Gson-compatible in/out),
  `resolve` (`resolve_headless`), `host`, `error`. Dependencies are exactly the
  six plan ruling 3 allowed: `fr-dsn`, `fr-board`, `fr-geometry`, `serde`,
  `serde_json`, `thiserror` (dev: `parity`, for `require_java_dir`). **No
  `tracing`, no `schemars`, no static mutable state.**
- **JVM parity**: `p4t1` runs Java's **real** two-merge composition — the actual
  `DefaultSettings`, `JsonFileSettings`, `CliSettings`,
  `EnvironmentVariablesSource`, `DsnFileSettings`, `RulesFileSettings`,
  `ApiSettings`, `SettingsMerger`, `RulesReader.read` and
  `RouterSettings.applyBoardSpecificOptimizations` out of the clone's HEAD jar —
  against `resolve_headless` over 84 cases in three modes: **0 diffs in all
  three** (5 728 / 4 287 / 5 728 lines).
- **In-process second proof**: `tests/precedence.rs` runs a 64-case matrix
  through **both** shapes of Java's composition — the literal two-merge form
  built out of the ported `SettingsMerger`, and the linear `resolve_headless` —
  and asserts they agree field for field. Ruling 1's equivalence is therefore
  proved twice, once against the JVM and once in-process.
- **`docs/java-quirks.md`**: 30 new pinned quirks (**#114–#143**), 2 new
  totalization rows, **two** obligation-register rows struck through as
  discharged (`DsnRouterSettings → RouterSettings`; the three clone-HEAD-only
  APIs), one re-labelled (legacy-CLI normalisation: the `fr-settings` half
  discharged, the wiring half open for Plan 8), and two new obligations filed
  (`AutorouteControl.ExpansionCostFactor` for Plan 6;
  `RoutingJobScheduler.scheduleJob`'s API-path composition for Plan 8).
- **Differential harness**: one new driver pair, `p4t1`, plus the 84-row case
  table `scripts/differential/matrix/p4t1-cases.tsv` and the committed mode-1
  golden `crates/fr-settings/tests/golden/p4t1-mode1/all.txt`.
- **`scripts/audit-map/fr-settings.map`**; all four `fr-settings` audit
  invocations exit 0 with no `MISSING` and no `UNMAPPED`.
- **One `fr-dsn` change**, controller ruling L (see below): `DsnRouterSettings`
  gained `Option` fields, absence tracking and `*_raw` accessors so a merge
  engine can tell "the file omitted this" from "the file wrote the default". No
  emitted byte moved — `p3t15` mode 3 and the whole 530-pair sweep stay MATCH.
- **1 354 tests across the workspace** (4 ignored), of which **233 in
  `fr-settings`** (1 ignored).
- **`crates/freerouting` is untouched** (plan ruling 10), verified: `git diff
  --name-only main..HEAD` names no file under it.

---

## 3. Public API surface (what Plans 5–8 call)

```rust
// crates/fr-settings/src/resolve.rs
pub struct SettingsInputs<'a> {
    pub dsn:             Option<&'a RouterSettings>,  // priority 20 — parsed once, used once
    pub cli_rules:       Option<&'a [u8]>,            // priority 40, merge #1 only
    pub scheduler_rules: Option<&'a [u8]>,            // priority 40 in merge #2 *and* step 7
    pub env:             Option<&'a RouterSettings>,  // priority 55
    pub cli:             Option<&'a RouterSettings>,  // priority 60
}

pub fn resolve_headless(
    inputs: &SettingsInputs<'_>,
    board: Option<&Board>,
    host: &HostEnvironment,
) -> RouterSettings;

pub fn resolve_scheduler_rules_path(
    job_rules: Option<&Path>, cli_rules: Option<&Path>, dsn_path: Option<&Path>,
) -> Option<PathBuf>;
pub fn resolve_scheduler_rules_path_with(          // the two File.exists() probes injected
    job_rules: Option<&Path>, cli_rules: Option<&Path>, dsn_path: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf>;

// crates/fr-settings/src/merger.rs
pub trait SettingsSource {
    fn get_settings(&self) -> Option<&RouterSettings>;   // None == Java's null: "no opinion"
    fn get_source_name(&self) -> String;
    fn get_priority(&self) -> i32;
    fn kind(&self) -> SourceKind;                        // added by the port; Java uses getClass()
}
pub struct SettingsMerger { … }
impl SettingsMerger {
    pub fn new(sources: Vec<Box<dyn SettingsSource>>) -> Self;
    pub fn add_or_replace_sources(&mut self, new_sources: Vec<Box<dyn SettingsSource>>);
    pub fn sources(&self) -> &[Box<dyn SettingsSource>];
    pub fn merge(&self, host: &HostEnvironment) -> RouterSettings;
}
pub mod priority { DEFAULT=0, JSON_FILE=10, DSN_FILE=20, SES_FILE=30,
                   RULES_FILE=40, ENVIRONMENT=55, CLI=60, GUI=65, API=70 }

// crates/fr-settings/src/sources/  — the seven sources
DefaultSettings, DsnFileSettings, RulesFileSettings, SesFileSettings,
ApiSettings, EnvironmentVariablesSource, CliSettings
impl From<DsnRouterSettings> for RouterSettings;      // and From<&DsnRouterSettings>
impl From<&RouterSettings> for DsnRouterSettings;     // Plan 3 ruling 5, discharged
pub fn apply_rules_file_against_board(…);             // RulesReader.read, step 7

// crates/fr-settings/src/field_path.rs
pub fn set_field_value(target: &mut RouterSettings, property_path: &str, value: &str)
    -> Result<(), MergeError>;   // path splits on `.` `:` AND `-` (quirk #118)

// crates/fr-settings/src/json.rs
impl RouterSettings {
    pub fn from_json_str(s: &str) -> Result<Self, SettingsError>;
    pub fn to_json_string_pretty(&self) -> Result<String, SettingsError>;
}

// crates/fr-settings/src/sources/cli.rs
pub fn apply_command_line_arguments(args: &[String]) -> LegacyBridge;  // dead by design
pub fn classify_de_arguments(args: &[String]) -> DeSlots;              // Plan 8 calls this

// crates/fr-settings/src/copy_fields.rs
impl RouterSettings {
    pub fn apply_new_values_from(&mut self, source: &RouterSettings) -> MergeReport;
    pub fn fill_absent_from(&mut self, source: &RouterSettings) -> MergeReport;
}

// crates/fr-settings/src/router_settings.rs / board_optimizations.rs
impl RouterSettings {
    pub fn java_clone(&self) -> Self;                  // NOT the derived Clone — see below
    pub fn validate(&mut self, host: &HostEnvironment);
    pub fn set_layer_count(&mut self, layer_count: usize);
    pub fn get_trace_costs(&self) -> Vec<ExpansionCostFactor>;
    pub fn is_fanout_enabled(&self) -> bool;
    pub fn apply_board_specific_optimizations(&mut self, board: &Board);
    pub fn apply_board_specific_optimizations_if_needed(&mut self, board: &Board);  // GUI only
    pub fn are_board_specific_trace_costs_applied(&self) -> bool;
}
pub struct ExpansionCostFactor { pub horizontal: f64, pub vertical: f64 }  // obligation: Plan 6

// crates/fr-settings/src/host.rs
impl HostEnvironment {
    pub fn detect() -> Self;                 // the crate's ONLY available_parallelism() call
    pub fn with_processors(n: usize) -> Self;
    pub fn available_processors(&self) -> usize;
    pub fn default_max_threads(&self) -> i32;
}
```

Everything public is re-exported from `fr_settings::prelude`.

**Four things a caller must know:**

- **`SettingsInputs` is half parsed objects and half raw bytes, and that is not
  an oversight.** `scheduler_rules` **must** be bytes because Java parses that
  file twice against two different layer structures (quirk #142) and
  `resolve_headless` performs both parses itself; `cli_rules` is bytes for
  symmetry with its neighbour. Feed them `std::fs::read(path)`, not a
  `RulesFileSettings`. `None` means "Java registered no such source".
- **`resolve_headless` is the CLI-started path only** — §1's last subsection.
- **`java_clone` is not the derived `Clone`.** Java's
  `RouterSettings.clone()` (`:487-523`) never assigns `resultJsonPath` (quirk
  #114) and re-copies its nested objects explicitly, so it differs from a
  structural clone in `result_json_path` and in null nested objects. **Every site
  where Java calls `clone()` must call `java_clone`** — `SettingsMerger`'s merge
  base and `resolve_headless`'s priority-70 payload both do. The derived `Clone`
  exists for ordinary Rust copying and carries a warning comment pointing at
  `java_clone`.
- **`HostEnvironment` is injectable and load-bearing.** `DefaultSettings.java:106`
  and `:134` both call `Runtime.getRuntime().availableProcessors()`, and
  `validate()` and `normalizeMaxThreads` consult it again **with different
  answers for `0`** (quirk #124). Tests and the differential all use
  `with_processors(4)`, pinned on the Java side with
  `-XX:ActiveProcessorCount=4`.

---

## 4. Rulings

### Plan rulings 1–10 — and what execution did with them

1. **Java's real precedence wins over spec §11; one clean merge reproduces it,
   and the equivalence is proved.** *Held, and was extended twice.* The
   40-case matrix the plan asked for became **64** in
   `tests/matrix/mod.rs` (4 DSN shapes × 4 rules shapes × 2 environments ×
   2 command lines — the honest cross product, with the pruning rule written into
   the file rather than left implicit), and `p4t1` added **20** more rows for a
   total of **84**. Both equivalence proofs pass: the in-process two-merge-vs-
   linear comparison over the 64, and the JVM differential over the 84. The
   survey's **Q1 correction survived contact with the JVM** — the adjacent
   `.rules` does reach `layers[i].preferredDirectionHorizontal` and
   `layers[i].bendCost`, which is why `fill_absent_from` exists at all.
   **Two extensions the plan did not foresee:** (a) the linear form needed
   `HeadlessBoardManager.java:741-745`'s `setLayerCount` +
   `applyBoardSpecificOptimizations` *between* the two merges (Task 8), and the
   explicit clearing of `board_specific_trace_costs_applied` at the point where
   Java changes objects (quirk #127); (b) controller ruling N split the single
   `scheduler_rules` parse into two (quirk #142). Without (b) the port diverged
   from the JVM on 13 of 84 rows.
   *Reason:* the spec is demonstrably wrong about Java. *Cost if wrong:* a wrong
   `.rules`-versus-flag order with no crash to find it — which is why it has two
   independent proofs, one against the JVM.
2. **The `copyFields` change-count is not reproduced field for field.** *Held.*
   Java compares boxed scalars with `!=` (reference identity,
   `ReflectionUtil.java:256`), so equal `Integer`s above the 127 cache count as
   changed; the object-array arm adds `sourceArray.length` regardless
   (`:311`, `:325`). The port counts one per field actually written and
   `source_array.len()` in the array arm. Recorded as quirk #117.
   *Reason:* no caller reads the count (`SettingsMerger.java:171` logs it,
   `RulesReader.java:156` discards it), and emulating the `Integer` cache would be
   a fiction with no test to pin it. *Cost if wrong:* nil for behaviour — and the
   differential deliberately does not print the count.
3. **`fr-settings` depends on `fr-dsn`; `schemars` is not added.** *Held.* The
   dependency list is exactly the six named. Spec §4's dependency line is
   deviated from, with the reason recorded here.
   *Reason:* the alternative is a fourth copy of the DSN scope readers.
   *Cost if wrong:* a later plan wanting `fr-settings` without `fr-dsn` has to
   split out a leaf `RouterSettings` crate — mechanical, and nothing in Plans 5–8
   asks for it.
4. **Every field is `Option<T>`; `copy_fields` is hand-written per struct against
   an explicit field table.** *Held, with one addition.* All eight `copyFields`
   rules are transcribed in Java declaration order, and `MergeMode::FillAbsent`
   was added as the inverted mode ruling 1's `fill_absent_from` needs. Each
   struct's table is pinned by a `*_table_covers_every_field` test with a
   fully-populated fixture (Task 2 fix round), mutation-verified by deleting a
   line from each table and watching all seven fail.
   *Reason:* Java's merge protocol *is* its nullability
   (`SettingsMerger.java:22-31`). *Cost if wrong:* an inverted array rule turns
   first-writer-wins into last-writer-wins and silently changes the trace costs
   the router sees; Tasks 2 and 8 both pin it.
5. **`apply_board_specific_optimizations` takes `&fr_board::Board`, keeps `f64`
   internals, reproduces the aspect-ratio penalty exactly.** *Held.* The five
   `fr-board` accessors the plan named all existed and were used unchanged; the
   plan's expectations (a 2 000 000 × 1 000 000 board gives `undesired[0] == 3.0`,
   `undesired[1] == 1.5`) verified against `BProbe`.
   *Correction found in execution:* plane (non-signal) layers **inherit the
   running direction without advancing it** — Java-wins over the brief's reading —
   and `Issue143-rpi_splitter` is a 2-layer board, so the 4-layer JVM golden uses
   `Issue145-smoothieboard` instead. Quirk #127 was found here.
   *Cost if wrong:* every routed board's costs shift by a tenth.
6. **Machine-dependent defaults are injectable (`HostEnvironment`).** *Held,
   unchanged.* One `available_parallelism()` call in the whole crate.
   *Cost if wrong:* nil — strictly additive; without it half the plan's expected
   values would be machine-dependent.
7. **The `p4t*` differential compiles against the clone's HEAD jar.** *Held.*
   `run.sh`'s `needs_jar=1` mode, `$FREEROUTING_JAR`, JDK 25. Both sides print the
   jar's path, size and mtime in a header line, so running against the wrong build
   is a diff rather than a silent assumption; the Rust side additionally prints
   its own binary's path and mtime to stderr.
   **What this discharged:** Plan 3's three clone-HEAD-only APIs are no longer
   Rust-tests-only — see §8's tick.
   *Cost if wrong:* the driver would validate against a build the port is not a
   port of; the header line is the guard.
8. **The five legacy short flags are ported dead, on a `LegacyBridge` nothing
   reads.** *Held.* Every flag carries a `// Java bug:` marker and a quirks row
   (#131–#137). `-mp`/`-mt` **do** reach settings through `CliSettings`, and `-mt`
   feeds **two different fields with different clamps** from one argv token —
   reproduced as two separate writes (quirk #132), with
   `RouterSettings.setMaxThreads` (`:183-185`) silently rewriting
   `optimizer.maxThreads` as a *third* writer.
   *Correction found in execution (JVM-verified):* `-oit -5` does **not** become
   `0.0f`. `-5` starts with `-`, so the blanket value rule never consumes it and
   the field keeps its previous value — the `<= 0` clamp is unreachable from a
   command line (quirk #135). `docs/cli-legacy-flags.md` carries the correction.
   *Cost if wrong:* a user's `-oit 5` keeps being ignored exactly as Java ignores
   it; Plan 8 can make it live with a one-line change and a recorded decision.
9. **`SettingsMerger` is a real public type.** *Held.* It is the second,
   independent proof of ruling 1. The `isAssignableFrom` half of
   `addOrReplaceSources` is `// not ported:` with quirk #129 — Rust has no
   supertype relation — and identity is a `SourceKind` enum rather than `TypeId`
   (clearer, and it lets the reserved `JsonFile`/`Gui` tiers keep an identity with
   no source behind them).
   *Cost if wrong:* trivial — one call site outside tests.
10. **`crates/freerouting` is not touched.** *Held, verified.* `classify_de_arguments`
    and `resolve_headless` are built and tested here; `legacy.rs` still forwards
    raw values, as `docs/cli-legacy-flags.md` says it does. Plan 8 rewires.
    *Cost if wrong:* Plan 8 has a one-call rewire; the function and the call site
    are named under Plan 8's obligations.

### Controller rulings J–P (made during execution)

- **J — the plan's rulings 1–10 are accepted as written**, and ruling 1 (Java's
  precedence over spec §11's wording) is surfaced to the user rather than buried.
  *Reason:* the plan's pre-flight scan found no producer/consumer mismatch worth
  re-opening. *Cost if wrong:* the deviation goes unnoticed and a later plan
  "fixes" the port back to the spec — which is why it is §1 of this document, the
  first section of `crates/fr-settings/README.md`, and a call-out box in the plan.
- **K — `SettingsMerger::clone` stays not ported.** Java clones a merger to fork
  the prototype (`Freerouting.java:125`, `RoutingJobScheduler.java:103`); the port
  rebuilds a merger from the same source list instead, because sources are pure
  value producers with no per-merger state. *Cost if wrong:* if a future source
  ever becomes stateful, a rebuilt merger and a cloned one diverge — bounded, and
  `SettingsMerger` has one non-test call site.
- **L — `DsnRouterSettings` could not express absence; fixed at the root, in
  `fr-dsn`.** Plan 3 defined it to hold "exactly the fields the DSN/rules scopes
  read and write, **with the same defaults**", stored eagerly as plain
  `bool`/`i32`. That is lossless for Plan 3's own readers and writers but destroys
  the one thing a merge engine needs: a `.rules` file that omits `(via_costs …)`
  must leave `scoring.viaCosts` null and let `DefaultSettings`' 50 stand — the
  eager default pushed `1` over it. JVM-measured on a reduced fixture:
  `merged.getViaCosts = 50` and `areBoardSpecificTraceCostsApplied = false` where
  the port answered 1 and `true`. The fix made the four scalars `Option`, added
  `board_specific_trace_costs_applied` and `*_raw` accessors, and made
  `apply_new_values_from` conditional; the coalescing getters were **kept**
  because the writers call them unconditionally, so no emitted byte moved
  (`p3t15` mode 3 and the full 530-pair sweep re-run MATCH).
  *Reason:* patching it on the `fr-settings` side would have left the trap in
  `fr-dsn` for the next consumer. *Cost if wrong:* silently wrong routing costs
  with no crash. **Lesson for the next cross-plan subset type: a subset type that
  a merge engine will consume must carry absence, not defaults.**
  The same review round corrected `priority::GUI` from the javadoc's 50 to the
  constant's **65** (quirk #138) — Java-wins over javadoc.
- **M — Task 7's commit is accepted although it swept in two quirks rows and one
  register row from Task 6's then-uncommitted fix.** The code those rows describe
  landed in `826756a`, so the ledger is consistent at HEAD and nothing was
  reverted. *Cost if wrong:* a quirks row could have described code that never
  landed; checked at HEAD instead of assumed.
- **N — Java parses the scheduler's `.rules` file twice, against two different
  layer structures.** Found by `p4t1`: an earlier `SettingsInputs` took one
  pre-parsed `RouterSettings` for both slots and diverged from the JVM on **13 of
  84 rows** — every `dsn4-*` row with a `.rules` file — in `layers[1]` versus
  `layers[3]`. Fixed at the root: both rules slots became bytes and
  `resolve_headless` performs both parses itself (Task 8 fix round 2, quirk #142).
  *Reason:* the two parses are not an implementation detail of the driver, they
  are the behaviour. *Cost if wrong:* per-layer routability and preferred
  direction land on the wrong layers on any board with more layers than the
  `.rules` file names — silent, and exactly the kind of thing a 2-layer test
  cannot see.
- **O — Gson's `Strictness.LENIENT` *reader* dialect stays not ported.**
  `GsonProvider.GSON` sets LENIENT, which governs only the textual pass:
  unquoted keys, single quotes, `NaN`/`Infinity` literals and so on are accepted
  by Java and rejected by `serde_json`. The port refuses them. *Reason:* nothing
  in the port writes such JSON, and reproducing Gson's dialect means a hand-rolled
  lenient parser. *Cost if wrong:* a hand-written config file that Java would have
  accepted is rejected with an error, never silently misread — an error where Java
  gave a value, never a different value. Recorded as quirk #141, which was
  **rewritten in Task 10's fix round** after the JVM refuted its first version:
  non-finite floats are refused in **both** directions, not accepted on read.
- **P — `JsonFileSettings`' not-ported roster line belongs to Task 11**, not to
  Task 10's JSON work. It is a *source* (priority 10), not a serialiser, and the
  roster is Task 11's deliverable. *Cost if wrong:* nil — a line in one file or
  another.

---

## 5. Corrections to the plan discovered during execution

1. **Quirk #119's consequence half was wrong** (Task 3 fix round, `RProbe` L1–L4).
   A `--router.layers.*` (or `FREEROUTING__ROUTER__LAYERS__*`) whose token count
   differs from the board's layer count is **discarded wholesale**, not merely
   mis-sized: `HeadlessBoardManager.java:742-744` calls
   `setLayerCount(boardLayerCount)` on the mismatch and
   `RouterSettings.setLayerCount` reallocates and resets every element. A matching
   token count survives, because the guard never fires.
2. **`DsnFileSettings.java:46-48` is conditional, not unconditional** (Task 3 fix
   round, correcting a review note): it calls `setLayerCount` only when
   `layerCount > 0 && rs.getLayerCount() == 0`. The seeding it causes is still
   real (quirk #128) — it seeds both per-layer cost arrays as a side effect — but
   the trigger is narrower than the survey's Q18 framing.
3. **`ExpansionCostFactor` is declared at `AutorouteControl.java:287`**, not
   `:118` as the Task 4 brief said; `:118` is inside the constructor.
4. **`EnvironmentVariablesSourceTest` has 18 cases**, and the merge test
   `env_source.rs` mirrors is `SettingsMergerTest.complexMerging` (`:152-175`),
   not `.environmentVariablesPriority` (`:141-150`), which asserts only the
   priority.
5. **Nine `transient` fields, not seven**, are absent from the Gson dump
   (`RouterSettings.java:61,64,67,70`, `OptimizerSettings.java:80,84,91`,
   `ScoringSettings.java:30,35`).
6. **`-oit -5` keeps the previous value** (ruling 8 above; quirk #135).
7. **Plane layers inherit the running preferred direction without advancing it**
   in `applyBoardSpecificOptimizations` (ruling 5 above).

---

## 6. Quirks pinned in Plan 4 (`docs/java-quirks.md` #114–#143)

| # | One line |
|---|---|
| 114 | `RouterSettings.clone` never copies `resultJsonPath` — the one scalar its ~15 explicit re-copies skip |
| 115 | `copyFields` can never merge a `false` or `0` **primitive** field, so `DesignRulesCheckerSettings.enabled = false` is unmergeable |
| 116 | `copyFields` silently drops a `Set`/`List` field's contents — rule 7 recurses into an object with no copyable fields |
| 117 | `copyFields`' return value counts boxed-reference identity, not equality (**this port's recorded divergence**, ruling 2) |
| 118 | `setFieldValue` treats `-` as a property-path separator, alongside `.` and `:` |
| 119 | `setPropertyRecursive` sizes an object array by the value's token count and silently drops surplus tokens |
| 120 | `convertValue` turns any non-boolean text into `false` without an error — `enabled=yes` is `false` |
| 121 | `getFieldByNameOrSerializedName` does not filter by modifier, so `private`/`static` fields are reachable by path |
| 122 | `convertValue`'s numeric arms accept more than the port does (Unicode digits, hex floats) — an error where Java gave a number |
| 123 | `getHorizontalTraceCosts`/`getVerticalTraceCosts` index the cost arrays with no null or bounds guard |
| 124 | `validate` and `normalizeMaxThreads` disagree about `maxThreads == 0` |
| 125 | `validate` dereferences `maxPasses` and `tracePullTightAccuracy` unboxed |
| 126 | `setLayerCount` wipes every per-layer cost on an **unchanged** layer count, but only clears the applied flag when it reallocates |
| 127 | `boardSpecificTraceCostsApplied` is `private transient`, so `copyFields` cannot carry it across a merge — a file's explicit trace costs are always re-derived from board geometry |
| 128 | `DsnFileSettings` seeds both per-layer trace-cost arrays as a side effect of seeding the layer count, blocking rule 5 for every later source |
| 129 | `SettingsMerger.addOrReplaceSources` also replaces when the **existing** source's class is a supertype of the new one — a latent trap, not live behaviour |
| 130 | `SesFileSettings` claims to read SES files and reads nothing — the spec's SES tier has no behaviour |
| 131 | `-oit`/`-us`/`-is`/`-hr`/`-inc` and `-drc`'s router switch write a bridge nothing reads |
| 132 | One `-mt` token writes two different fields with two different clamps |
| 133 | `GlobalSettings` matches every legacy flag by **prefix** while `CliSettings` matches exactly — so `-mpx 5` sets `maxPasses` |
| 134 | `-mp` is `Integer.decode` on one path and `Integer.parseInt` on the other |
| 135 | `-oit`'s `<= 0` clamp is unreachable from a command line |
| 136 | `CliSettings` treats a valueless `--router.enabled=` as an explicit choice, then reads it as `false` |
| 137 | `-de` drops a file whose extension it does not know rather than treating it as the design |
| 138 | `SettingsSource`'s javadoc says the GUI tier is priority 50; the constant is 65 |
| 139 | `getRunFanout` and `isFanoutEnabled` carry the same javadoc and opposite defaults |
| 140 | `validate` is not idempotent, and the headless path calls it twice — `max_passes == 0` ends as 9999 |
| 141 | `GsonProvider.GSON`'s `Strictness.LENIENT` governs only the *textual* pass, and non-finite floats are refused in **both** directions |
| 142 | The scheduler's `.rules` file is parsed **twice**, against two different layer structures |
| 143 | **Neither** thread-count field is read anywhere in the headless path — every reader of `optimizer.maxThreads` and of `maxThreads` is GUI-only or dead code, so `-mt` is arithmetic on a value nothing consumes |

Plus **two totalization rows**: `ReflectionUtil.setFieldValue(obj, ".", v)` (or
any path of nothing but separators) throws `ArrayIndexOutOfBoundsException` in
Java and returns an error here; and
`RouterSettings.applyBoardSpecificOptimizations` dereferences
`board.boundingBox` with no null check, where the port takes `Option<&Board>`.

Also worth carrying forward, though it is a port determinism choice rather than a
Java defect: **`EnvironmentVariablesSource` takes a `BTreeMap`, not a `HashMap`**
(`// renamed:` at the type). Java's iteration order is unspecified; the port's is
lexicographic. Observable only if two raw keys upper-case to the same property
path, which no real environment produces.

---

## 7. Test and fixture inventory

**233 tests in `fr-settings`** across eleven integration suites plus unit tests:
`copy_fields`, `field_path`, `router_settings`, `board_optimizations`, `sources`,
`env_source`, `cli_source`, `precedence`, `json`, `struct_shape`, `corpus`.

The load-bearing three:

- **`precedence.rs`** — the 64-case matrix through both composition shapes
  (§2). The pruning rule is written into `tests/matrix/mod.rs` rather than left
  implicit, and the file also carries the reachability table for each column.
  **The matrix alone cannot discriminate every mutation** — step-level tests do
  that; Task 8's fix round corrected a mutation table that had claimed otherwise.
- **`struct_shape.rs`** — pins every struct's field order against Java's
  `getDeclaredFields()`, because `copy_fields` iterates it and `json.rs` emits it.
- **`json.rs::p4t1_mode_1_parity`** — replays 64 of the differential's 84 rows
  against the committed Java transcript, so Gson parity survives without a JVM.

**All ten Java suites spec §14.1 named are ported**, each cited by name at its
Rust site: `SettingsMergerTest`, `RouterSettingsMergeTest`,
`EnvironmentVariablesSourceTest`, `GlobalSettingsCommandLineTest`,
`ReflectionUtilArrayTest`, `BendCostSettingsTest`,
`Issue729TraceCostSettingsTest`, `NeckWidthSettingsTest`, `DsnFileSettingsTest`,
`RulesFileSettingsTest`.

### The ten JVM probes

Every expected value in this crate came out of a JUnit-free Java driver in
`crates/fr-settings/tests/data/`, run against the clone's HEAD jar.
`tests/data/README.md` is the index and carries the **exact command line for
each**, including which need `-XX:ActiveProcessorCount=4` to match
`HostEnvironment::with_processors(4)`.

| Probe | What it pins |
|---|---|
| `FProbe` | 44 `setFieldValue` cases: separators, `Boolean.parseBoolean`, array navigation, name resolution, the numeric arms, enums, `String[]`/`double[]` leaves, every failure mode |
| `DProbe` | `Double.parseDouble` / `Integer.parseInt` grammar edges (no jar needed) |
| `TProbe` | the array-token vs scalar-leaf trim asymmetry |
| `VProbe` | Task 4's accessors, clamps, `setLayerCount`, `clone`, `validate` |
| `BProbe` | `applyBoardSpecificOptimizations` over four synthetic stacks plus named DSN fixtures |
| `RProbe` | quirk #119's real consequence, and Java's array branch with a bogus next segment |
| `CProbe` | `EnvironmentVariablesSource`, `CliSettings`' consumption and flag mapping, the merged `SettingsMergerTest` CLI cases, the whole dead `LegacyBridge` table, the `-de` matrix |
| `PProbe` | the two `validate()` calls — quirk #140's evidence |
| `JProbe` | Gson parity: write, read, the `@SerializedName` alternates, LENIENT's dialect and coercions, `U+2028`/`U+2029` |
| `SProbe` | `SettingsMerger`, `SettingsSource`, the five in-scope sources, `DefaultSettings` field by field, and ruling L's absence-vs-default evidence |

Their transcripts live in the task reports under
`.superpowers/sdd/2026-08-28-plan-4-settings/task-{3,4,5,6,7,8,10}-report.md`.

**Committed inputs** (deliberately committed, not generated, so the JVM
transcript and the Rust test read the same bytes):
`Plan4Matrix-primary.rules` and `Plan4Matrix-adjacent.rules` (the matrix's `-dr`
file and the scheduler's file — they name `F.Cu`/`B.Cu` and disagree on every
value they carry, so a case where merge #1 and merge #2 see different rules
cannot pass by coincidence), and `Issue029-hw48na_reduced.rules` (the real
fixture with eight lines deleted, which is what distinguishes "absent" from "the
coalesced default").

**The one golden:** `crates/fr-settings/tests/golden/p4t1-mode1/all.txt`.
Regenerate with

```sh
./scripts/differential/run.sh p4t1 scripts/differential/matrix/p4t1-cases.tsv all 1
tail -n +3 scripts/differential/build/p4t1.j.out \
  > crates/fr-settings/tests/golden/p4t1-mode1/all.txt
```

**What needs the sibling checkout** (`../freerouting`, or `$FREEROUTING_JAVA_DIR`
— not vendored):

- `corpus.rs`, `precedence.rs`, `sources.rs` and `json.rs` **skip with a printed
  message** (each fixture-reading test calls `parity::require_java_dir()` first).
- `board_optimizations.rs`'s three `jvm_golden_*` tests **panic** — they read
  through `fixture_board`, which honours `FREEROUTING_JAVA_DIR` but has no skip
  guard. Its synthetic-board tests, the majority, need nothing. Adding the guard
  is three lines; it was left alone rather than changed mid-task.
- Everything else is pure: `copy_fields`, `field_path`, `router_settings`,
  `env_source`, `cli_source`, `struct_shape`.

The **jar** is needed only by the probes and by `p4t1`, never by `cargo test`.

---

## 8. The `p4t1` differential

`scripts/differential/java/P4T1.java` (Java's real two-merge composition out of
the clone's HEAD jar) against `scripts/differential/rust/src/bin/p4t1.rs`
(`resolve_headless`), over the 84-row table
`scripts/differential/matrix/p4t1-cases.tsv`.

| Run | Lines | Diffs | What it covers |
|---|---|---|---|
| `p4t1 … all 0` | 5 728 | **0** | the canonical dump: one sorted `path=value` line per field, transients included, arrays as a `.length` line plus one line per element so `null` and empty stay distinguishable |
| `p4t1 … all 1` | 4 287 | **0** | the same objects through `GsonProvider.GSON` vs `RouterSettings::to_json_string_pretty`, byte for byte |
| `p4t1 … 8 2` | 5 728 | **0** | mode 2 ignores the case index and runs the whole table as mode 0 |

```sh
./scripts/differential/run.sh p4t1                                                    # all 84, mode 0
./scripts/differential/run.sh p4t1 scripts/differential/matrix/p4t1-cases.tsv all 1   # the Gson dump
./scripts/differential/run.sh p4t1 scripts/differential/matrix/p4t1-cases.tsv 8   0   # one case
```

The first 64 rows are exactly `tests/matrix/mod.rs`'s cross product with the same
ids, so the in-process proof and the JVM proof cover the same ground; the 20
`x-*` rows are Task 9 additions (the `validate()` non-idempotence rows, four real
corpus `.rules` files, and env/CLI shapes the 64 do not carry).

Two header lines guard the run: the jar's real path, size and mtime plus
`availableProcessors` (so the wrong build is a diff, not a silent assumption),
and `JSON_SOURCE_EMPTY` — the Java side builds a real `JsonFileSettings` on an
**empty temporary directory** and aborts unless every leaf is null, which turns
spec §2's "no persistent config file" from an assumption into a check.

**The transcription risk.** `P4T1.java` is the only driver whose ground truth is
a *sequence* of Java calls rather than one method, so a mis-transcribed step
would make both sides agree on the wrong answer. The mitigation: every statement
carries the Java line it stands for, over five short ranges
(`Freerouting.java:1408-1413`, `:125-146`, `HeadlessBoardManager.java:739-748`,
`RoutingJobScheduler.java:103-170`, `:172-186`), and only the ~50 lines of
plumbing are transcribed — every merge-relevant class is the real one out of the
jar. Two substitutions are deliberate and marked at the site (the synthetic
board, and `job.name == null` making the design name the literal `"board"`).
`scripts/differential/README.md` says how to re-check it.

**`p3t15` re-run after ruling L.** Ruling L changed `fr-dsn`, so Task 11 re-ran
the full sweep: **530 pairs, 525 MATCH, 0 unexpected diffs, 5 XDIFF** — exactly
the Plan 3 baseline, both XDIFFs being the two already documented (ruling E's
2.3.0-vs-HEAD `readStringScope`, and `RulesWriter`'s NPE on a library-less
board). Ruling L cost nothing.

---

## 9. Parked residuals, by task

- **Task 1** — the audit baseline is **46** `MISSING`, not the 44 the original
  commit message says (miscounted from a scrolled terminal; corrected in the
  report, the commit message could not be amended because Task 2 was already
  building on it). The trajectory 46 → 43 → 8 → 5 → 0 is in the ledger.
- **Task 2** — `MergeMode::FillAbsent` has exactly one production caller
  (`resolve_headless`). If a future differential disagrees with the two-merge
  form, this mode's rule-5 and rule-6 behaviours are the first place to look:
  they are the two rules where "inverted" is a judgment call rather than a
  transcription.
- **Task 3** — Java's `parseInt(s, radix)` accepts any Unicode decimal digit and
  `Double.parseDouble` accepts the hexadecimal form; the port does not (quirk
  #122). Always an error where Java gave a number, never a different number.
- **Task 4** — test-file citation ranges are approximate. Five `RouterSettings`
  methods were left `MISSING` at Task 4 and closed by name in Task 11:
  `setAlgorithm` (no non-test caller), `setOptimizerEnabled` / `setFanoutEnabled`
  / `getRunFanout` (GUI only), and `isFanoutEnabled` — which **was** ported,
  because Plan 6's pipeline needs it.
- **Task 5** — `apply_board_specific_optimizations_if_needed` is ported because
  it is in the audit's scope and the GUI uses it. **Nothing in the headless
  pipeline should call it**; `RoutingJobScheduler.java:186` calls the unguarded
  form, and quirk #127 is what makes that safe.
- **Task 6** — `apply_new_values_from` copies `run_router`/`run_optimizer`
  unconditionally (documented at the site, unreachable in practice).
- **Task 7** — `apply_command_line_arguments` walks Java's *whole* dispatch
  chain, including branches that write objects this crate does not model, because
  `-drc` must be matched before `-dr` and the `-de` arm's multi-argument run has
  to be skipped as one unit. Provably equivalent to omitting them.
- **Task 8** — the no-board boundary family (§1) is documented, not closed. The
  matrix cannot discriminate every mutation on its own.
- **Task 9** — a synthetic board cannot read a real `.rules` file, so the rows
  that pair a corpus `.rules` with a board read the fixture back through the real
  DSN reader (`board = dsn`).
- **Task 10** — Gson's LENIENT reader dialect is not ported (ruling O).
- **Task 11** — `tests/corpus.rs`'s always-on sweep costs 2.7 s of a ~4 s
  `cargo test -p fr-settings`; its `.rules` sibling is
  `#[cfg_attr(debug_assertions, ignore)]` and needs
  `cargo test -p fr-settings --release --test corpus`.

---

## 10. Obligations for later plans

### Plan 5 (`fr-drc`) — all three answered; see `docs/plan-5-handoff.md`

- ~~**`DesignRulesCheckerSettings` lives in this crate, and its `enabled` field is
  a primitive `boolean`.**~~ — **moot: Plan 5 does not consume it at all**
  (plan-5 ruling 12, Task 3 `6a7b5d0`). Rule 2 of `copyFields` suppresses
  primitive defaults
  (`ReflectionUtil.java:235-238`), so **`enabled = false` and
  `include_warnings = false` are unmergeable** — a source that wants to turn
  either off cannot, in Java or here (quirk #115). That would have mattered if
  anything read the settings; **nothing does, in Java either**.
  `DesignRulesChecker` stores the object in a `private final` field
  (`DesignRulesChecker.java:32`, assigned at `:45`) and never mentions it again,
  and `includeWarnings`/`includeErrors` are read nowhere in the tree — recorded
  as **quirk #155**, with the re-count that 12 of the 14 constructions in
  `src/main` pass `null`. So `DesignRulesChecker::new(&mut Board)` takes **no
  settings parameter**, and `fr-drc` has **no `fr-settings` dependency**. If a
  later upstream release starts reading the field, `fr-drc` gains one dependency
  and one parameter, and quirk #115 becomes live again.
- ~~**The DRC report's own JSON schema is Plan 5's, not this crate's.**~~ —
  **done** (Tasks 7-8, `b127bd3`/`f73d5fb`).
  `fr-settings`' `json.rs` reproduces `GsonProvider` for `RouterSettings` only;
  the DRC report's four DTOs and its two key tables live in
  `crates/fr-drc/src/report/`. (Plan 3 also ruled that the DRC report must not
  reuse `IndentFileWriter`, and it does not.) What *is* shared is the number
  formatter: plan-5 ruling 7 moved `JavaNumberFormatter` out of
  `fr-settings::json` and down into `fr_dsn::format::json` (Task 1, `1a8f0a9`),
  so both crates render Gson bytes from one copy. `fr-settings`' entry points
  and its whole `tests/json.rs` were left untouched, which is the proof the move
  was behaviour-preserving.
- `-drc`'s `routerSettings.enabled = false` writes only the dead `LegacyBridge`
  (quirk #131). If Plan 5 wants `-drc` to actually disable the router it is
  making the port more capable than Java — Plan 8's decision, below. —
  **restated for Plan 8, unchanged.** Plan 5 never touched the `-drc` argument
  path (plan-5 ruling 13 puts `Freerouting.initializeDrc` in Plan 8) and
  `crates/freerouting` has no Plan 5 commit, so the decision arrives at Plan 8
  exactly as Plan 4 left it.

### Plans 6/7 (`fr-router`)

- **`max_passes == 0` means *unlimited*, never "no passes."** `validate` turns it
  into `i32::MAX` on the first call — and into `9999` on the second, which is
  what the headless path actually produces (quirk #140). The pass loop must treat
  the value as a limit, not a count.
- **`optimizer.max_threads` and `max_threads` are two distinct fields with
  distinct normalisations** (quirk #132) — **and neither drives anything in
  Java's headless path** (quirk #143). Do not collapse them, and do **not** build
  headless multi-threaded selection on `-mt`: it has no Java counterpart.
  The complete consumer list, from a grep of every `maxThreads` occurrence in
  `src/main/java` at the clone's HEAD:

  | Field | Reader | Reachable headless? |
  |---|---|---|
  | `optimizer.maxThreads` | `BatchOptimizer.createForGui:58` — `featureFlags.multiThreading && … > 1` | **No** — `createForGui` is called only from `RoutingPipeline.createForGui:41`, itself only from `GuiRoutingJobWorker.java:212` |
  | `optimizer.maxThreads` | `BatchOptimizerMultiThreaded.java:41` (the pool size) | **No** — constructed only inside that GUI branch |
  | `optimizer.maxThreads` | `GlobalSettings.getNumThreads():851-854` | **No** — the method has no caller anywhere in the tree |
  | `RouterSettings.maxThreads` | `AutoroutePassRunner.runMultiThread:50,53,91` | **No** — reached only from `BatchAutorouter.autoroutePassMultiThread:411-413`, which **has no caller anywhere in the tree** |
  | `RouterSettings.maxThreads` | `GuiManager:147`, `GuiRoutingJobWorker:416,603`, `WorkspacePortAdapter:121`, `WindowAutorouteParameter:1084,1110` | **No** — all GUI |

  The headless chain is `RoutingJobSchedulerActionThread.java:99` →
  `RoutingPipeline.createForHeadless` (`:45-47`, whose doc says "headless
  single-threaded optimizer policy") → `BatchOptimizer.createForHeadless`
  (`:51-53`), which returns `new BatchOptimizer(job)` **unconditionally** and
  never looks at the field. The live pass path is
  `AutorouteBatchLoop.java:293` → `:598-600` → `BatchAutorouter.autoroutePass`
  (`:419-421`) → `AutoroutePassRunner.runSingleThread`.

  So in headless Java `-mt` is **parsed, clamped, mirrored into a second field,
  and read by nothing** — dead in the same way the five legacy flags of §1 are
  dead, and for the same reason: the only readers are GUI. Plans 6/7 must
  therefore keep both fields (they are part of the settings object's observable
  state, and `p4t1` compares them) but must **not** invent a headless
  multi-threading policy from them. Doing so would make the port more capable
  than Java — the same class of product decision as the dead legacy flags, and it
  belongs in a recorded decision, not in a router task.
- **`ExpansionCostFactor` is declared here, with an `obligation:` marker**
  (`router_settings.rs:934`), because `fr-settings` is its only producer and
  cannot depend on a router crate that does not exist. `AutorouteControl.java:287`
  is where Java declares it; when Plan 6 ports `AutorouteControl` it must
  **re-export `fr_settings::ExpansionCostFactor`, not declare a second record**.
  `RouterSettings::get_trace_costs()` is the producer.
- **`is_fanout_enabled` is ported for you** (`router_settings.rs:472`, with an
  `obligation:` marker naming Plan 6). Its sibling `getRunFanout` is **not**
  ported (GUI only) and carries the opposite default — quirk #139 exists because
  the two share a javadoc and disagree.
- **Quirk #127 means the router sees board-derived trace costs, not the `.rules`
  file's.** `boardSpecificTraceCostsApplied` is `private transient`, cannot cross
  a merge, and `RoutingJobScheduler.java:186` therefore always re-derives both
  per-layer cost arrays from board geometry. Combined with quirk #128 (the DSN
  source seeds both arrays with all-`1.0` as a side effect of seeding the layer
  count) and rule 5's first-writer-wins, a `.rules` file's per-layer trace costs
  **never reach the router through the merge**. That is Java's behaviour; do not
  "fix" it while porting the router.
- **`apply_board_specific_optimizations` has exactly two headless call sites**
  (`HeadlessBoardManager.java:745` and `RoutingJobScheduler.java:186`), both
  inside `resolve_headless`; the **three** GUI call sites (`BoardFrame.java:331`,
  `:581`, `:783`) are out of scope, and the separate
  `applyBoardSpecificOptimizationsIfNeeded` entry point has exactly one call
  site, `BoardToolbar.java:209`, also GUI. **If Plan 6/7 ever re-derives settings
  mid-run** — a
  board change, a re-load — it must decide which variant to call, and quirks #126
  and #127 govern the answer: the `_if_needed` guard consults a flag that
  `setLayerCount` clears only on reallocation.
- **Via-info / via-rule re-pointing (carried unchanged from Plan 3, still open).**
  Nothing in Plan 4 touched it; the register row stands.
- Everything Plan 2 and Plan 3 filed for these plans remains open and unchanged
  (the ladder hang's second half, quirk #106, pass-level recovery, quirk #82,
  `scripts/audit-map/fr-board.map`).

### Plan 8 (`fr-core` + surfaces)

- **The API-job path needs a different composition** — the largest of the four.
  `resolve_headless` linearises the CLI-started two-merge path and its premise is
  that merge #1's result is *complete* when it is re-injected at priority 70. An
  API job never runs merge #1, so its priority-70 `ApiSettings` payload is
  **sparse** and merge #2's own `0..60` chain does the work. Plan 8 must compose
  that path out of `SettingsMerger` directly rather than calling
  `resolve_headless`. Marker: `obligation: RoutingJobScheduler.scheduleJob` at
  `crates/fr-settings/src/resolve.rs:184`.
- **Wire the two pure functions this plan built for you** (ruling 10).
  `fr_settings::classify_de_arguments` is
  `GlobalSettingsCommandLineTest`'s `-de` classification rule, tested once here;
  `crates/freerouting/src/legacy.rs` currently reproduces the rule itself and
  should call this instead. `fr_settings::apply_command_line_arguments` →
  `LegacyBridge` is the legacy flag table with Java's exact value normalisation;
  `legacy.rs` forwards raw values on purpose so the rule is applied once, here.
  `resolve_headless` goes into `crates/freerouting/src/cli.rs`. **Feed the rules
  slots `std::fs::read(path)`, not a parsed object** (quirk #142).
- **Decide whether to wire the five dead legacy flags — this is a product
  decision, not a defect.** `-oit`, `-us`, `-is`, `-hr`, `-inc` and `-drc`'s
  router switch-off are parsed, normalised, stored on `LegacyBridge`, and read by
  nothing (ruling 8, quirk #131). Making them live makes the port **more capable
  than Java**, which will show up as a parity difference in Plans 6–8 with no way
  to tell a port bug from a deliberate improvement. Whatever is decided must be
  recorded as a decision, not allowed to drift. It is a one-line change either
  way.
- **`job_timeout_string` still needs `TextManager.parseTimespanString`** and the
  24 h cap. This crate carries all three timeout values as `String`s, as Java
  does; the only Java reader is
  `RoutingJobSchedulerActionThread.threadAction` (`:44`), which parses
  `jobTimeoutString` then caps at `MAX_TIMEOUT` (`:24`, 24 hours) in `:45-52`.
  Marker: `added in Plan 8: TextManager.parseTimespanString` in `lib.rs`'s roster.
- **`schemars` is still unadded** (ruling 3). Spec §13's `list_settings` MCP tool
  needs either it or a hand-written schema. `RouterSettings` derives
  `Serialize`/`Deserialize` but deliberately not `JsonSchema`.
- **`JsonFileSettings` if a persistent config file is ever wanted.** Priority 10
  is reserved (`priority::JSON_FILE`), the identity is reserved
  (`SourceKind::JsonFile`), and `P4T1.java` *proves* the tier currently
  contributes nothing. Spec §2 says there is no config file; if that changes,
  this is the slot and `GlobalSettings.load`'s `copyFields(loadedSettings,
  defaultSettings)` call (`GlobalSettings.java:351`) is the behaviour to port —
  which is also the one call site where `MergeMode` on
  `DesignRulesCheckerSettings`/`DebugSettings` would become observable
  (`copy_fields.rs:578` carries the note).
- **The `.json` design-input divergence stands** (`docs/cli-legacy-flags.md`):
  Java has no dedicated KiCad-JSON slot and the port gives it one, so
  `-de board.json -do out.ses` routes in Java and currently fails in the port.
  An unimplemented-loader gap, not a silent misroute; resolve it when the KiCad
  JSON reader lands.
- MCP concurrency, `io/kicad/**`, `SessionToEagle` and the KiCad reader's quirk
  #83 warning are carried unchanged from Plans 1–3.

---

## 11. Known limitations

1. **`board_optimizations.rs`'s three `jvm_golden_*` tests panic rather than skip
   without the sibling Java checkout.** They read through `fixture_board`, which
   honours `$FREEROUTING_JAVA_DIR` but has no `require_java_dir()` guard. Same
   shape `fr-dsn`'s README documents for its own suites. Three lines to fix if a
   later plan wants it.
2. **`audit-port.sh` cannot distinguish two classes mapped to the same file.**
   `JsonFileSettings.getSettings` and `GuiSettingsSource.getSettings` satisfy each
   other's marker, because the regex matches the method name inside the union of
   the class's mapped files. The roster writes both lines anyway. A real fix is a
   class-aware marker regex (`not ported: <Class>.<method>`), which changes
   `audit-port.sh` **and every existing marker in three crates** — a plan-level
   decision, not a task edit. (`fr-board`'s crate-wide audit caveat from Plan 2 is
   still open and unrelated.)
3. **Gson's LENIENT reader coercions are not ported** (ruling O, quirk #141).
   Unquoted keys, single quotes and non-finite literals are refused where Java
   accepted them. An error where Java gave a value, never a different value.
4. **The `copyFields` change count diverges by design** (ruling 2, quirk #117) and
   the differential deliberately does not compare it.
5. **The no-board linearisation boundary** (§1) is documented and unreachable from
   Java, not closed.

---

## 12. Evidence

Verified on the committed tree at `50e5195` with a working tree carrying nothing
but this document's own edits:

```
cargo fmt --all --check                                clean
cargo clippy --workspace --all-targets -- -D warnings  clean          (at 50e5195, Task 11)
cargo test --workspace                                 1354 passed, 0 failed, 4 ignored
cargo test -p fr-settings                               233 passed, 0 failed, 1 ignored

./scripts/audit-port.sh settings         <10 files> scripts/audit-map/fr-settings.map -> 0
./scripts/audit-port.sh settings/sources '*.java'   scripts/audit-map/fr-settings.map -> 0
./scripts/audit-port.sh util  'ReflectionUtil.java' scripts/audit-map/fr-settings.map -> 0
./scripts/audit-port.sh util/gson        '*.java'   scripts/audit-map/fr-settings.map -> 0
   (no MISSING line, no UNMAPPED line, on all four)

./scripts/differential/run.sh p4t1 … all 0             MATCH (5728 lines)   (Task 11)
./scripts/differential/run.sh p4t1 … all 1             MATCH (4287 lines)   (Task 11)
./scripts/differential/run.sh p4t1 … 8   2             MATCH (5728 lines)   (Task 11)
./scripts/differential/sweep-p3t15.sh                  530 pairs, 0 unexpected  (Task 11)
```

The 4 ignored tests: `fr-settings`' `corpus.rs` `.rules` sibling, `fr-board`'s
non-terminating ladder reproduction (quirk #76 — it exists precisely because it
cannot pass), and two in `fr-dsn/tests/dsn_reader.rs` gated on
`debug_assertions`.

**Read the audit zero precisely.** For `fr-settings` it is per-class evidence —
`scripts/audit-map/fr-settings.map` pins every ported class to its Rust file(s)
and an unmapped class prints `UNMAPPED` — with limitation 2 above as the one
caveat. `GlobalSettings.java` alone contributes eighteen deferrals (seventeen
public methods plus the constructor), closed by name in `src/sources/cli.rs`,
never by weakening the script or the map.

**Nothing in Plan 4 changes `docs/geometry-library-survey.md`.**

---

## 13. Open items for the user

- **Spec §11 is wrong and stays wrong.** This port follows Java (§1). If the spec
  is ever revised, §11's merge-order bullet is the line to rewrite; the port
  should not be changed to match it.
- **Three product decisions are handed to Plan 8**, each of which would make the
  port more capable than Java rather than fix a bug: wiring the five dead legacy
  flags, wiring `-drc`'s router switch-off, and giving `.json` design inputs a
  first-class slot. None should be taken silently.
- **Plan 4's final whole-branch review has not run.** This task's own
  verification is not a substitute for it.
