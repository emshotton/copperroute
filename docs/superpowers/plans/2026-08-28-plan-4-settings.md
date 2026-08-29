# Plan 4 — `fr-settings` (`RouterSettings`, the merge engine, and the settings sources) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port freerouting's settings layer — `settings/RouterSettings.java` and its four nested value types (`LayerSettings`, `ScoringSettings`, `OptimizerSettings`, `FanoutSettings`), `settings/{SettingsSource,SettingsMerger,DesignRulesCheckerSettings,DebugSettings}.java`, the seven in-scope `settings/sources/**` classes, `util/ReflectionUtil.java` (the merge and path-set engine) and `GlobalSettings.applyCommandLineArguments`'s legacy flag table — as the `fr-settings` crate, producing the same effective `RouterSettings` as Java for every (DSN, `.rules`, env, argv, board) input.

**Architecture:** Spec §11 (binding for the struct shape), §3 (parity contract), §14 (tests). `fr-settings` sits on top of `fr-dsn` and `fr-board`: it converts `fr_dsn::DsnRouterSettings` (Plan 3 ruling 5's local stand-in) into the real `RouterSettings` through a `From`/`Into` pair, reads the DSN and `.rules` sources through `fr_dsn::read_metadata` / `fr_dsn::rules_reader::read_router_settings`, and takes a `&fr_board::Board` for `apply_board_specific_optimizations`. It owns no board and no I/O of its own beyond opening the two files those readers consume. Java has **no reflection analogue in Rust**: `ReflectionUtil.copyFields` and `ReflectionUtil.setFieldValue` are ported as hand-written per-struct code driven by an explicit field table, which is why Task 2 and Task 3 are the two largest tasks in the plan.

**Tech Stack:** Rust 2024; `fr-dsn` (Plan 3); `fr-board` (Plan 2); `fr-geometry` (Plan 1); `serde` + `serde_json`; `thiserror`. **No `schemars`** (ruling 3), no `tracing`, no static mutable state.

**Spec:** `docs/superpowers/specs/2026-08-27-freerouting-rust-port-design.md` (§2, §4, §11, §12, §14). Also binding: `docs/plan-1-handoff.md`, `docs/plan-2-handoff.md`, `docs/plan-3-handoff.md` (§Obligations → Plan 4), `docs/java-quirks.md`, `docs/cli-legacy-flags.md`.

**Later plans:** 5 `fr-drc`, 6–7 `fr-router`, 8 `fr-core` + surfaces (CLI wiring, MCP, `io/kicad`, `SessionToEagle`).

> ### ⚠ This plan deviates from the spec's stated merge order — read ruling 1 first
>
> Spec §11 says the merge order is `defaults → DSN → SES → .rules → env → CLI`. **Java does not do that.** Headless freerouting runs *two* merges, re-injects the first merge's complete result at priority 70, applies `.rules` **again after** the merge (so `.rules` outranks env and CLI for the fields `(autoroute_settings)` carries), and has a SES tier that is a documented no-op. Ruling 1 records the precedence Java actually implements, the single clean merge that reproduces it, and how the equivalence is proved. The spec text is wrong; the port follows Java. This is the single most important thing in the plan and the user should see it before Task 1 starts.

## Global Constraints

All Plan 1, Plan 2 and Plan 3 constraints and rulings remain in force (see `docs/plan-1-handoff.md` §Rulings, `docs/plan-2-handoff.md` §Rulings, `docs/plan-3-handoff.md` §Rulings). Additionally:

- **Java wins over plan text, and over the survey.** Every test-datum in this plan cites the Java line or the fixture it came from; never bend a test to fit a bug. Where this document quotes an expected number or string it was read out of the Java source at the clone's HEAD while writing the plan, but the implementer re-derives it from Java and reports any disagreement rather than trusting the plan. Two places where **this plan already corrects the Java survey it was written from** are called out in ruling 1 — treat that as the precedent, not as an exception.
- **Java source authority for this plan is the clone's HEAD**, and the differential jar is the clone's HEAD build (ruling 7). Plan 3's ruling-10 exception (2.3.0 for `io/specctra`) does **not** apply here: nothing in `settings/**` feeds the byte-parity references, and the three APIs Plan 3 flagged as clone-HEAD-only (`RulesReader.readRouterSettings`, `RulesReader.discoverLayerStructure`, `RouterSettings.applyNewValuesFrom`) all exist in both jars — verified with `javap` while writing this plan (ruling 7).
- Behavioral port: reproduce Java bugs, with a `// Java bug:` marker at the site and a row in `docs/java-quirks.md`. `// totalized:` for crash→value changes (only where no reachable Java caller observes the difference — Plan 1 ruling 12), `// not ported:`, `// renamed:`, `// added in Plan N:`, `obligation:` per `docs/java-quirks.md` §Process notes. The marker's Java method name must sit on the **same line** as the marker (Plan 2 ruling 13 — `audit-port.sh` is line-based).
- No GUI, no `FRLogger`, no `PropertyChangeSupport`. Java's `FRLogger.warn`/`error` calls in the merge path either (a) become a `MergeReport::errors` entry where Java swallows an exception, or (b) vanish. They never become `tracing` calls: `fr-settings` must not depend on `tracing`.
- `Result`/`Option` where Java throws or returns `null`. Every `RouterSettings` field is `Option<T>` (spec §11, ruling 4), mirroring Java's nullable boxed wrappers — that nullability *is* the merge protocol and must not be flattened into defaults at the struct level.
- **No static mutable state.** Java's `GlobalSettings.getUserDataPath`/`lockUserDataPath` are `static` mutable path state; they are out of scope and get `// not ported:` markers. Every machine-dependent default (`Runtime.getRuntime().availableProcessors()`, `DefaultSettings.java:106,134`) goes through an injectable `HostEnvironment` (ruling 6) — no `std::thread::available_parallelism()` call outside `HostEnvironment::detect`.
- `fr-settings` depends on `fr-dsn`, `fr-board`, `fr-geometry`, `serde`, `serde_json` and `thiserror` (ruling 3). Any need for a seventh dependency is a ruling that must be recorded, not a silent `Cargo.toml` edit.
- Every task ends with `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, and the three `scripts/audit-port.sh` invocations from Task 1 (with `scripts/audit-map/fr-settings.map`) all clean, verified against the committed tree — not against a report.
- Commit trailer:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_015f1ubhBBpiWHAooDsso97Z
  ```

## Rulings made while writing this plan (recorded here so they reach the user)

1. **Java's real precedence wins over spec §11's stated order; the port reproduces it with one clean merge and proves the equivalence.**
   Spec §11 says `defaults → DSN → SES → .rules → env → CLI`. What Java runs headless (all line numbers re-read at the clone's HEAD while writing this plan) is:
   - `Freerouting.java:1408-1413` builds a prototype merger from `DefaultSettings(0)`, `JsonFileSettings(10)`, `CliSettings(60)`, `EnvironmentVariablesSource(55)`.
   - **Merge #1** (`Freerouting.java:125-146`): clone the prototype, add `DsnFileSettings(20)` and — only if `-dr`/`-de …rules` gave one — `RulesFileSettings(40)`, then `merge()`. `SettingsMerger.merge` (`SettingsMerger.java:133-193`) sorts ascending by priority, `clone()`s the first non-null source as the base and `applyNewValuesFrom`s the rest, then calls `validate()` (:189).
   - **Merge #2** (`RoutingJobScheduler.java:103-170`): clone the prototype *again*, re-add `DsnFileSettings(20)`, add `RulesFileSettings(40)` from `job.rules` **or** `-dr` **or** an auto-discovered adjacent `<design>.rules` (:132-152), then add `new ApiSettings(job.routerSettings)` at **priority 70** (:163-166) — i.e. the whole result of merge #1 — and `merge()`.
   - Then `RulesReader.read(…, job.routerSettings)` (:176-180) → `RulesReader.java:153-157` calls `targetSettings.applyNewValuesFrom(parsedSettings)` on the **already-merged** object, so `.rules` `(autoroute_settings)` outranks env and CLI for every field it carries.
   - Then `job.routerSettings.applyBoardSpecificOptimizations(job.board)` (:186).
   - `SesFileSettings.getSettings()` returns `new RouterSettings()` unconditionally (`SesFileSettings.java:27-36`) and is never registered in the headless path at all — the spec's SES tier has no behaviour to port.

   Because merge #1's result is *complete* (every field non-null after `DefaultSettings`), it wins at priority 70 over everything merge #2's own sources contribute — **except where merge #1 left a field null**, which is exactly `layers[i].preferredDirectionHorizontal`, `layers[i].bendCost`, `resultJsonPath`, `optimizer.timeoutString` and `fanout.timeoutString`. **This corrects survey Q1**, which says the scheduler's auto-discovered adjacent `.rules` "contributes nothing through the merger": it contributes precisely those nullable per-layer fields. It also corrects the survey's framing of Q18: `DsnFileSettings.java:46-48` calls `setLayerCount` whenever the *extracted* settings carry no layers — which covers both "no `(autoroute_settings)` block" and "a block that named no layer rules" — not only the former.

   The port therefore implements `resolve_headless` as one linear pass (Task 8):
   ```
   s = DefaultSettings                                  // priority 0
   s.apply(dsn)                                         // 20
   s.apply(cli_rules)                                   // 40   ( -dr / -de …rules only )
   s.apply(env)                                         // 55
   s.apply(cli)                                         // 60
   s.validate()                                         // == merge #1's result, the priority-70 payload
   s.fill_absent_from(scheduler_rules)                  // Q1: the only channel merge #2's own 0..60 chain has
   s.validate()                                         // merge #2's validate()
   s.apply(scheduler_rules)                             // Q2: RulesReader's post-merge re-apply
   s.apply_board_specific_optimizations(board)          // Q4
   ```
   where `scheduler_rules` = `job.rules ?? -dr ?? adjacent <design>.rules` and `fill_absent_from` is `copy_fields` with the roles inverted (only fields still `None`, and only arrays still null-or-empty, are filled). `JsonFileSettings(10)` is out of scope (spec §2: no persistent config file) and is a no-op when the file is absent, which the differential driver asserts rather than assumes.
   *Reason:* the spec is demonstrably wrong about Java, and a port that follows the spec silently mis-orders `.rules` against `--router.*` flags — a wrong-output bug with no crash to find it. *Cost if wrong:* the linear form could disagree with the two-merge form on an input nobody thought of; the mitigation is Task 9's `p4t1` differential against the real Java classes over a 40-case matrix **plus** a Rust-side `SettingsMerger` (the literal Java class, ported for the audit anyway) that Task 8 runs in the two-merge shape over the same matrix and asserts equal. Two independent proofs, one of them against the JVM.
2. **The `copyFields` change-count is not reproduced field-for-field; the port counts copies by rule and documents the divergence.** Java compares boxed scalars with `!=` (reference identity, `ReflectionUtil.java:256`), so two equal `Integer`s above the 127 cache count as "changed" and two equal cached ones do not; the object-array arm adds `sourceArray.length` whether or not anything moved (:311, :325). Rust has no boxing and no identity to reproduce. The port's `MergeReport::fields_changed` counts one per field actually written and `source_array.len()` in the object-array arm (matching :311/:325 exactly, which *is* reproducible). *Reason:* **no caller reads the count** — `SettingsMerger.java:171` logs it and `RulesReader.java:156` discards it — so it is unobservable, and emulating the `Integer` cache would be a fiction with no test to pin it. *Cost if wrong:* nil for behaviour; the differential must therefore **not** compare the count, and Task 9's driver deliberately does not print it. Recorded as a divergence row in `docs/java-quirks.md`, not as a bug fix.
3. **`fr-settings` depends on `fr-dsn`, not only on `fr-board`; and `schemars` is not added.** Spec §4's dependency line says `fr-dsn` and `fr-settings` both depend only on `fr-board` and `fr-geometry`. But `DsnFileSettings` wraps `DsnReader.readMetadata`, `RulesFileSettings` wraps `RulesReader.readRouterSettings`, and Plan 3 ruling 5 parked the `DsnRouterSettings → RouterSettings` conversion here — all three need `fr-dsn`, and `fr-dsn` cannot depend upward without inverting spec §15's build order. `fr-settings → fr-dsn → fr-board → fr-geometry` keeps the direction strict. Separately, spec §4/§13 assume `schemars` for MCP tool schemas; the Plan 8 ruling recorded in `docs/plan-3-handoff.md` defers every surface concern, so **no `schemars` in this plan** — `serde`/`serde_json` only. *Reason:* the alternative is a fourth copy of the DSN scope readers inside `fr-settings`. *Cost if wrong:* a later plan that wants `fr-settings` without `fr-dsn` has to split out a leaf `RouterSettings` crate — mechanical, and nothing in Plans 5–8 asks for that.
4. **Every `RouterSettings` field is `Option<T>` and `copy_fields` is hand-written per struct against an explicit field table.** Java's merge protocol *is* its nullability (`SettingsMerger.java:22-31`), so the `Option`s are load-bearing, not stylistic. `ReflectionUtil.copyFields` (:215-344) is transcribed as a `CopyFields` trait impl per struct, in **Java declaration order**, following its eight rules verbatim: skip `static` (:221-223) and non-`public` (:226-228) — so `boardSpecificTraceCostsApplied` and `pcs` are never copied, and `transient` is *not* skipped; `shouldCopy = source != null` with the extra default-suppression only for **primitive** fields (:235-238); scalars copy on `!=` (:242-259); enums via `Enum.valueOf(..., toString())`, **case-sensitive** (:260-266); primitive/`String` arrays copy **only when the target is null, or empty and the source non-empty** (:269-290) — first writer wins, so `scoring.preferredDirectionTraceCost`, `scoring.undesiredDirectionTraceCost` and `ignoreNetClasses` never overwrite a populated target; object arrays merge element-wise when `target.len() >= source.len()` and are **never shrunk** (:291-327, pinned by `SettingsMergerTest.java:259-277`); any other object recurses, instantiating a null target field (:328-336); every exception is swallowed (:338-340) — in Rust, a per-field `Result` pushed onto `MergeReport::errors`, never aborting the merge. Enum *matching* is case-insensitive on `set_field_value` (`ReflectionUtil.java:158-164`) and exact on `copy_fields` (:262) — two different rules in one file; keep them apart. *Cost if wrong:* an array rule inverted turns "first writer wins" into "last writer wins" and silently changes the trace costs the router sees; Tasks 2 and 8 both pin it.
5. **`apply_board_specific_optimizations` takes `&fr_board::Board`, keeps `f64` internals, and reproduces the aspect-ratio penalty exactly.** `RouterSettings.java:266-435` needs the board bounding box and the layer structure's signal flags; `fr-board` already exposes `Board::get_bounding_box() -> IntBox`, `Board::get_layer_count()`, `Board::layer_structure()`, `LayerStructure::signal_layer_count()` and `Layer::is_signal`, and `IntBox::width()`/`height()` (verified while writing this plan). The two penalties are `0.1 * Math.round(10 * w / h)` and `0.1 * Math.round(10 * h / w)` (:299-303), computed in `double` from `int` widths widened to `double`; use `fr_geometry::java_round` and keep every intermediate `f64`. *Reason:* the alternative — a `BoardGeometry` value type — would need the port to decide what "the board's geometry" is, and the aspect ratio is the one place a rounding difference changes a routing cost. *Cost if wrong:* every routed board's costs shift by a tenth; caught by Task 5's exact expectations (a 2 000 000 × 1 000 000 board gives `undesired[0] == 3.0`, `undesired[1] == 1.5`).
6. **Machine-dependent defaults are injectable.** `DefaultSettings.java:106` and `:134` both call `Runtime.getRuntime().availableProcessors()`, and `RouterSettings.normalizeMaxThreads` (:137-149) and `validate()` (:943-955) both consult it again — with **different answers for `0`** (ruling 8 / quirk Q4). The port threads a `HostEnvironment { available_processors: usize }` through `DefaultSettings::new`, `RouterSettings::validate` and `set_max_threads`; `HostEnvironment::detect()` calls `std::thread::available_parallelism()` once, `HostEnvironment::with_processors(n)` is what every test uses. *Reason:* without it, half the plan's expected values are machine-dependent and the differential is unreproducible. *Cost if wrong:* nil — it is strictly additive.
7. **The `p4t*` differential compiles against the clone's HEAD jar, which is already built.** `javap` on both jars while writing this plan shows `SettingsMerger`, `ReflectionUtil.copyFields`, `RouterSettings.applyNewValuesFrom`, `SesFileSettings` and every `settings/sources/**` class present in **both** `tools/freerouting-2.3.0.jar` and `../freerouting/build/libs/freerouting-current-executable.jar` — so the survey's worry that they might be HEAD-only is unfounded, and the Plan 3 hand-off's "not jar-pinned" note for the three `RulesReader`/`applyNewValuesFrom` APIs applies to the *four-argument `RulesReader.read` overload*, not to these classes. The two jars differ in the `RoutingBoard` package (`app.freerouting.board.RoutingBoard` in 2.3.0 vs `app.freerouting.board.facade.RoutingBoard` at HEAD), which `applyBoardSpecificOptimizations` takes, so the driver must pick one: **HEAD**, matching the sources being ported and `run.sh`'s existing `needs_jar=1` mode (JDK 25, `$FREEROUTING_JAR`, already at `../freerouting/build/libs/freerouting-current-executable.jar`, built 2026-08-27). No `gradlew` run is needed. *Reason:* the ported source is HEAD, and 2.3.0 only matters for `io/specctra` byte parity. *Cost if wrong:* the driver would validate against a build the port is not a port of — checked by having Task 9 print the jar path and its mtime in the driver's header line.
8. **The five legacy short flags `-oit`/`-us`/`-is`/`-hr`/`-inc` are ported as dead, on a `LegacyBridge` nothing reads.** `GlobalSettings.applyCommandLineArguments` (:700-736, :810-815) writes them into the `@Deprecated public final RouterSettings routerSettings` bridge (`GlobalSettings.java:51-53`), and `CliSettings.mapFlagToProperty` (:102-110) maps **only** `mp` and `mt`. No routing path reads the bridge, so in headless Java these five flags — and `-drc`'s `routerSettings.enabled = false` (:662) — have no effect at all. The port parses them, applies the exact normalisation in `docs/cli-legacy-flags.md`, stores them on `LegacyBridge`, and **no `resolve_headless` input reads that struct**. Each carries a `// Java bug:` marker and a quirks row; `docs/plan-4-handoff.md` hands Plan 8 the decision whether to wire them (which would make the port *more capable* than Java — a deliberate choice, not a bug fix). `-mp`/`-mt` **do** reach settings through `CliSettings`, and `-mt` feeds **two different fields with different clamps** from one argv token (bridge → `optimizer.maxThreads`, clamp `<0→0`, `>1024→1024`; `CliSettings` → `RouterSettings.maxThreads`, clamped later by `validate()`), which the port reproduces as two separate writes. *Reason:* porting them live would change routing behaviour relative to Java and break metric parity in Plans 6–8 with no way to tell a port bug from a deliberate improvement. *Cost if wrong:* a user's `-oit 5` keeps being ignored exactly as Java ignores it; the hand-off makes it a one-line change later.
9. **`SettingsMerger` is ported as a real public type even though `resolve_headless` is the entry point.** It is a Java class in scope for the audit, `SettingsMergerTest` is one of the ported tests, and it is the second, independent proof of ruling 1's equivalence. It stays a thin generic merger over `Vec<Box<dyn SettingsSource>>` with `add_or_replace_sources` reproducing the `isAssignableFrom` replacement rule (`SettingsMerger.java:107-126`) via `TypeId` — Rust has no supertype relation, so the `isAssignableFrom` half is a `// not ported:` with the reason and the quirks row (Q13: it is a latent trap in Java, not live behaviour). *Cost if wrong:* trivial — the type has one call site outside tests.
10. **`crates/freerouting` is not touched by this plan.** Spec §12's `--set section.field=value`, the `-de` slot filling and the legacy shim's rewrite all live in the binary, and `docs/plan-3-handoff.md` parks every surface question for Plan 8. Task 7 ports `GlobalSettingsCommandLineTest`'s `-de` classification matrix as a **pure function in `fr-settings`** (`classify_de_arguments`) so the rule is written down and tested once, and Plan 8 calls it from `legacy.rs`; `legacy.rs` itself keeps forwarding raw values, as `docs/cli-legacy-flags.md` says it does. *Reason:* changing the binary mid-plan would break the e2e tests Plan 8 owns and put a parity-critical rule on two code paths. *Cost if wrong:* Plan 8 has a one-call rewire; the hand-off names the function and the call site.

## File Structure

```
crates/fr-settings/
  Cargo.toml                      deps: fr-dsn, fr-board, fr-geometry, serde, serde_json, thiserror
                                  dev-deps: parity
  src/lib.rs                      pub mod list + prelude + the `// not ported:` roster (Task 11)
  src/error.rs                    SettingsError, MergeError, MergeReport
  src/host.rs                     HostEnvironment (ruling 6)
  src/router_settings.rs          RouterSettings: fields, accessors, clamps, validate, set_layer_count, clone
  src/layer_settings.rs           LayerSettings
  src/scoring_settings.rs         ScoringSettings
  src/optimizer_settings.rs       OptimizerSettings, BoardUpdateStrategy, ItemSelectionStrategy
  src/fanout_settings.rs          FanoutSettings
  src/drc_settings.rs             DesignRulesCheckerSettings, DebugSettings
  src/copy_fields.rs              the ReflectionUtil.copyFields engine (ruling 4)
  src/field_path.rs               ReflectionUtil.setFieldValue: path split, name resolution, convertValue
  src/board_optimizations.rs      apply_board_specific_optimizations(&Board)
  src/merger.rs                   SettingsSource trait, SettingsMerger, PRIORITY constants
  src/sources/mod.rs
  src/sources/default_settings.rs DefaultSettings
  src/sources/dsn_file.rs         DsnFileSettings + From<DsnRouterSettings> for RouterSettings
  src/sources/rules_file.rs       RulesFileSettings
  src/sources/ses_file.rs         SesFileSettings (the explicit no-op, ruling 1)
  src/sources/api.rs              ApiSettings
  src/sources/env.rs              EnvironmentVariablesSource
  src/sources/cli.rs              CliSettings, LegacyBridge, classify_de_arguments
  src/resolve.rs                  resolve_headless, SettingsInputs (ruling 1)
  tests/*.rs                      ported Java tests + the equivalence matrix (see tasks)
scripts/audit-map/fr-settings.map per-class map
scripts/differential/java/P4T1.java
scripts/differential/rust/src/bin/p4t1.rs
scripts/differential/matrix/p4t1-cases.tsv   the 40-case matrix (committed)
```

---

### Task 1: Crate skeleton, the five settings structs, `HostEnvironment`, audit map

**Files:** `crates/fr-settings/{Cargo.toml,src/lib.rs,src/error.rs,src/host.rs,src/router_settings.rs,src/layer_settings.rs,src/scoring_settings.rs,src/optimizer_settings.rs,src/fanout_settings.rs,src/drc_settings.rs}`, workspace `Cargo.toml` (add nothing — `crates/*` is already a glob member), `scripts/audit-map/fr-settings.map`.
**Java:** `settings/{RouterSettings.java (966), LayerSettings.java (75), ScoringSettings.java (107), OptimizerSettings.java (121), FanoutSettings.java (123), DesignRulesCheckerSettings.java (30), DebugSettings.java (56)}`, `autoroute/{BoardUpdateStrategy,ItemSelectionStrategy}.java`.

**Interfaces produced.** Every field is `Option<T>` (ruling 4) and carries `#[serde(rename = "<@SerializedName value>")]` with `alias` for each `alternate`. Declaration order **must** match Java's, because `copy_fields` iterates `getDeclaredFields()` in declaration order (Task 2).

```rust
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct RouterSettings {
    #[serde(rename = "enabled")]            pub enabled: Option<bool>,              // RouterSettings.java:20-21
    #[serde(rename = "algorithm")]          pub algorithm: Option<String>,          // :23-24
    #[serde(rename = "fanout")]             pub fanout: Option<FanoutSettings>,     // :27-28
    #[serde(rename = "copper_to_edge_clearance_um")] pub copper_to_edge_clearance_um: Option<f64>, // :30-31
    #[serde(rename = "hole_clearance_um")]  pub hole_clearance_um: Option<f64>,     // :33-34
    #[serde(rename = "neck_width_um")]      pub neck_width_um: Option<f64>,         // :42-43
    #[serde(rename = "strict_drc")]         pub strict_drc: Option<bool>,           // :51-52
    #[serde(rename = "job_timeout")]        pub job_timeout_string: Option<String>, // :54-55
    #[serde(rename = "max_passes")]         pub max_passes: Option<i32>,            // :57-58
    #[serde(rename = "max_items", skip_serializing)] pub max_items: Option<i32>,    // :60-61  transient
    #[serde(rename = "layers")]             pub layers: Option<Vec<LayerSettings>>, // :63-64  transient (but see below)
    #[serde(rename = "save_intermediate_stages", skip_serializing)] pub save_intermediate_stages: Option<bool>, // :66-67 transient
    #[serde(rename = "ignore_net_classes", skip_serializing)] pub ignore_net_classes: Option<Vec<String>>,      // :69-70 transient
    #[serde(rename = "trace_pull_tight_accuracy", alias = "tracePullTightAccuracy")]
                                            pub trace_pull_tight_accuracy: Option<i32>, // :73-76
    #[serde(rename = "allowed_via_types")]  pub vias_allowed: Option<bool>,         // :78-79   ← quirk: Boolean under a plural noun
    #[serde(rename = "automatic_neckdown", alias = "automaticNeckdown")]
                                            pub automatic_neckdown: Option<bool>,   // :85-88
    #[serde(rename = "optimizer")]          pub optimizer: Option<OptimizerSettings>, // :90-91
    #[serde(rename = "scoring")]            pub scoring: Option<ScoringSettings>,   // :93-94
    #[serde(rename = "max_threads")]        pub max_threads: Option<i32>,           // :96-97
    #[serde(rename = "result_json")]        pub result_json_path: Option<String>,   // :103-104
    // private transient Boolean boardSpecificTraceCostsApplied (:111) — NOT public, so copy_fields
    // must skip it (ruling 4). Kept private here for the same reason.
    #[serde(skip)] pub(crate) board_specific_trace_costs_applied: Option<bool>,
}
```
- `LayerSettings { routable: Option<bool>, preferred_direction_horizontal: Option<bool>, bend_cost: Option<f64> }` (`LayerSettings.java:9-21`). Java's `equals`/`hashCode` (:57-74) are dropped with `// not ported:` — the survey confirms no lookup uses them; the derived `PartialEq` is structurally the same anyway.
- `ScoringSettings` in Java order (`ScoringSettings.java:29-81`): `preferred_direction_trace_cost: Option<Vec<f64>>` (transient), `undesired_direction_trace_cost: Option<Vec<f64>>` (transient), `default_preferred_direction_trace_cost: Option<f64>`, `default_undesired_direction_trace_cost: Option<f64>`, `via_costs: Option<i32>` (`rename = "via_costs"`, `alias = "viaCosts"`), `plane_via_costs: Option<i32>`, `start_ripup_costs: Option<i32>` (`alias = "startRipupCosts"`), `unrouted_net_penalty: Option<f32>`, `clearance_violation_penalty: Option<f32>`, `bend_penalty: Option<f32>`, `default_bend_cost: Option<f64>`. Note the **`f32`** on the three penalties — Java has them as `Float`, and Plan 3's `java_float_to_string` is the formatter if they ever reach text.
- `OptimizerSettings` in Java order (`OptimizerSettings.java:14-95`): `enabled`, `algorithm`, `max_passes`, `max_items`, `max_threads`, `optimization_improvement_threshold: Option<f32>` (`rename = "improvement_threshold"`), `max_consecutive_failures`, `additional_ripup_cost_factor_at_start`, `trace_ripup_cost_factor: Option<f32>`, `max_autoroute_passes`, `board_update_strategy: Option<BoardUpdateStrategy>` (transient), `hybrid_ratio: Option<String>` (transient), `item_selection_strategy: Option<ItemSelectionStrategy>` (transient), `timeout_string: Option<String>` (`rename = "timeout"`).
- `FanoutSettings` in Java order (`FanoutSettings.java:22-106`): `enabled`, `max_passes`, `max_items`, `max_milliseconds_per_pin: Option<i64>`, `ripup_allowed`, `min_escape_length_mm`, `max_escape_length_mm`, `start_via_diameter_mm`, `end_via_diameter_mm`, `pin_sorting_order: Option<String>`, `fallback_to_board_vias`, `timeout_string`.
- `#[derive(Copy, Clone, PartialEq, Eq, Debug, Serialize, Deserialize)] pub enum BoardUpdateStrategy { Greedy, GlobalOptimal, Hybrid }` and `pub enum ItemSelectionStrategy { Sequential, Random, Prioritized }` — **declaration order preserved** (`ReflectionUtil.getDefaultValue` takes `getEnumConstants()[0]` at :367-368; unreachable from `copy_fields` but preserved anyway per the survey's §8 note). Each gets `fn java_name(self) -> &'static str` returning the `SCREAMING_SNAKE` Java constant name, because `copy_fields`'s enum arm matches on `Enum.valueOf(type, sourceValue.toString())` — *exact* — and `set_field_value`'s matches case-insensitively.
- `DesignRulesCheckerSettings { enabled: bool (transient, **primitive**), include_warnings: bool = true, include_errors: bool = true }` — note these are Java `boolean`, not `Boolean`, so `copy_fields`'s primitive default-suppression applies (`ReflectionUtil.java:236-238`): a `false` never copies. Pin that in Task 2.
- `pub struct HostEnvironment { available_processors: usize }` with `detect()` (one `std::thread::available_parallelism()`, `.map(NonZero::get).unwrap_or(1)`), `with_processors(n: usize)`, `available_processors(&self) -> usize`, `default_max_threads(&self) -> i32` = `max(1, cores - 1)` (`RouterSettings.java:133-135`).
- `#[derive(Debug, thiserror::Error)] pub enum SettingsError { Dsn(#[from] fr_dsn::DsnError), Io(#[from] std::io::Error), Json(#[from] serde_json::Error) }` and `pub enum MergeError { NoSuchField { path: String }, NumberFormat { path: String, value: String }, EnumName { path: String, value: String } }`, `pub struct MergeReport { pub fields_changed: usize, pub errors: Vec<MergeError> }`.

**`scripts/audit-map/fr-settings.map`** — the per-class map, one line per class per file (the reader keeps only the first token after the class name):
```
RouterSettings              router_settings.rs
RouterSettings              board_optimizations.rs
LayerSettings               layer_settings.rs
ScoringSettings             scoring_settings.rs
OptimizerSettings           optimizer_settings.rs
FanoutSettings              fanout_settings.rs
DesignRulesCheckerSettings  drc_settings.rs
DebugSettings               drc_settings.rs
SettingsSource              merger.rs
SettingsMerger              merger.rs
ReflectionUtil              copy_fields.rs
ReflectionUtil              field_path.rs
DefaultSettings             sources/default_settings.rs
DsnFileSettings             sources/dsn_file.rs
RulesFileSettings           sources/rules_file.rs
SesFileSettings             sources/ses_file.rs
ApiSettings                 sources/api.rs
EnvironmentVariablesSource  sources/env.rs
CliSettings                 sources/cli.rs
GlobalSettings              sources/cli.rs
JsonFileSettings            lib.rs
GuiSettingsSource           lib.rs
```

**Tests (write first, `crates/fr-settings/tests/struct_shape.rs`):**
- `RouterSettings::default()` has **every** field `None` — this is Java's `new RouterSettings()` minus the three nested objects the constructor allocates (`RouterSettings.java:120-124`), which the port models as `Some(Default::default())` in a separate `RouterSettings::new()`; assert `new()` gives `Some` for `fanout`/`optimizer`/`scoring` and `None` for everything else, and that `default()` differs. Pin both, because `SettingsMergerTest.java:34-41` (`emptySourcesList`) asserts `merged.maxPasses` is `null` after `new SettingsMerger().merge()`.
- serde key round trip: `serde_json::to_value(&RouterSettings::new())` uses the key `allowed_via_types` for `vias_allowed`, `job_timeout` for `job_timeout_string`, `result_json` for `result_json_path`, `improvement_threshold` for `optimizer.optimization_improvement_threshold`, `timeout` for both `timeout_string`s; deserialising `{"trace_pull_tight_accuracy":7}` and `{"tracePullTightAccuracy":7}` both give `Some(7)`; likewise `via_costs`/`viaCosts` and `start_ripup_costs`/`startRipupCosts`.
- `HostEnvironment::with_processors(1).default_max_threads() == 1`, `with_processors(8).default_max_threads() == 7`, `with_processors(0).default_max_threads() == 1`.

Steps: tests → fail → implement → pass → fmt/clippy/test → `scripts/audit-port.sh settings crates/fr-settings/src 'RouterSettings.java LayerSettings.java ScoringSettings.java OptimizerSettings.java FanoutSettings.java DesignRulesCheckerSettings.java DebugSettings.java' scripts/audit-map/fr-settings.map` runs and prints its `MISSING` list (it will not be zero yet — record the count in the commit message so Task 11 can show it going to zero) → commit `feat(settings): crate skeleton, RouterSettings and the four nested structs, audit map`.

---

### Task 2: `ReflectionUtil.copyFields` — the merge engine

**Files:** `crates/fr-settings/src/copy_fields.rs`, `src/error.rs` (fill in `MergeReport`); `crates/fr-settings/tests/copy_fields.rs`.
**Java:** `util/ReflectionUtil.java:207-379` (`copyFields`, `getDefaultValue`), `settings/RouterSettings.java:901-929` (`applyNewValuesFrom`).

**Interfaces produced:**
```rust
pub trait CopyFields {
    /// `ReflectionUtil.copyFields(source, target)` (ReflectionUtil.java:215-344), in Java
    /// declaration order. Returns nothing; the count and any per-field failure land in `report`.
    fn copy_fields_into(&self, target: &mut Self, report: &mut MergeReport);
}
impl CopyFields for RouterSettings { … }   // and LayerSettings, ScoringSettings,
                                            // OptimizerSettings, FanoutSettings,
                                            // DesignRulesCheckerSettings
impl RouterSettings {
    /// `RouterSettings.applyNewValuesFrom` (RouterSettings.java:907-929).
    // not ported: pcs.firePropertyChange (:917-926) — GUI notification.
    // not ported: the `settings == null` guard (:908-911) — `&RouterSettings` cannot be null.
    pub fn apply_new_values_from(&mut self, source: &RouterSettings) -> MergeReport { … }
    /// Ruling 1's inverse: fill only what is still absent. Not a Java method — the derived
    /// operation that makes merge #2's own 0..60 chain expressible in one linear pass.
    // added in Plan 4: no Java analogue; see docs/superpowers/plans/…-plan-4-settings.md ruling 1.
    pub fn fill_absent_from(&mut self, source: &RouterSettings) -> MergeReport { … }
}
```

The four helper predicates that make the rules explicit and greppable:
- `fn scalar_copy<T: PartialEq + Clone>(src: &Option<T>, dst: &mut Option<T>, report: &mut MergeReport)` — rule 3 (:242-259). Copies whenever `src.is_some()`; counts one. (Ruling 2: Java's `!=` identity test is not reproducible and unobservable.)
- `fn primitive_array_copy<T: Clone>(src: &Option<Vec<T>>, dst: &mut Option<Vec<T>>, report: &mut MergeReport)` — rule 5 (:269-290). Copies **only** when `dst.is_none() || (dst.as_ref().unwrap().is_empty() && src.as_ref().is_some_and(|s| !s.is_empty()))`.
- `fn object_array_merge<T: CopyFields + Default + Clone>(src: &Option<Vec<T>>, dst: &mut Option<Vec<T>>, report: &mut MergeReport)` — rule 6 (:291-327). `target_len >= src.len()` → element-wise `copy_fields_into` in place, never shrinking; else replace with a fresh `Vec` of `src.len()` deep copies. `report.fields_changed += src.len()` in **both** arms (:311, :325).
- `fn nested_copy<T: CopyFields + Default>(src: &Option<T>, dst: &mut Option<T>, report: &mut MergeReport)` — rule 7 (:328-336): recurse, instantiating a `None` target.

Java's `boolean` primitives (`DesignRulesCheckerSettings.enabled/includeWarnings/includeErrors`) go through `fn primitive_bool_copy(src: bool, dst: &mut bool, report: &mut MergeReport)`, which **skips `false`** because `getDefaultValue` answers `false` for a `boolean` field (:362-363) and rule 2 suppresses it (:236-238). Mark it `// Java bug:` — a source that explicitly wants `include_warnings = false` cannot express it through a merge.

**Tests (write first, `tests/copy_fields.rs`) — ported from `RouterSettingsMergeTest.java` and `SettingsMergerTest.java`:**
- `merge_layers_array` (port of `RouterSettingsMergeTest.java:13-38`): source `set_layer_count(2)` with `layers[0].routable = Some(false)`, `layers[1].routable = Some(true)`, `layers[0].preferred_direction_horizontal = Some(true)`, `layers[1].preferred_direction_horizontal = Some(false)`; target `RouterSettings::new()` (layers `None`). After `apply_new_values_from`, target has 2 layers with exactly those values, and mutating `target.layers[0].routable` leaves the source at `Some(false)` (Rust's `Vec` clone gives this for free — assert it anyway, it is the Java test's point).
- `layers_array_not_shrunk_on_merge` (port of `SettingsMergerTest.java:259-277`): target `set_layer_count(6)` with layers 0–2 `routable = Some(true)`; source `set_layer_count(2)` with `[Some(false), Some(true)]`. After `copy_fields_into`: `target.get_layer_count() == 6`, `layers[0].routable == Some(false)`, `layers[1].routable == Some(true)`, `layers[2].routable == Some(true)`.
- `primitive_arrays_are_first_writer_wins`: target `scoring.preferred_direction_trace_cost = Some(vec![1.0, 1.0])`, source `Some(vec![2.5, 3.5])` → target unchanged. Target `Some(vec![])`, source `Some(vec![2.5])` → target becomes `[2.5]`. Target `None`, source `Some(vec![2.5])` → `[2.5]`. Target `Some(vec![1.0])`, source `Some(vec![])` → unchanged (the `sourceArrayLength > 0` half of :285).
- `ignore_net_classes_follows_the_same_rule`: target `Some(vec!["GND".into()])`, source `Some(vec!["VCC".into()])` → target unchanged.
- `board_specific_flag_is_never_copied`: source with `board_specific_trace_costs_applied = Some(true)`, target `None` → target still `None` after the merge (rule 1, `Modifier.isPublic` at :226-228).
- `primitive_false_does_not_copy`: `DesignRulesCheckerSettings { include_warnings: false, .. }` onto a target with `true` leaves `true`.
- `object_array_count_is_source_length`: a 2-element source onto a 6-element target with nothing actually changing still reports `fields_changed >= 2` (pins :311).
- `errors_never_abort`: a `MergeReport` with one pushed `MergeError` still leaves every later field copied — drive it by feeding an enum name Task 3 cannot resolve through `apply_new_values_from`'s nested path.

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(settings): ReflectionUtil.copyFields as a hand-written per-struct merge engine`.

---

### Task 3: `ReflectionUtil.setFieldValue` — path splitting, name resolution, value conversion

**Files:** `crates/fr-settings/src/field_path.rs`; `crates/fr-settings/tests/field_path.rs`.
**Java:** `util/ReflectionUtil.java:21-205` (`setFieldValue`, `setPropertyRecursive`, `getFieldByNameOrSerializedName`, `snakeToLowerCamel`, `convertValue`).

**Interfaces produced:**
```rust
/// `ReflectionUtil.setFieldValue(obj, propertyName, newValue)` (ReflectionUtil.java:21-25).
pub fn set_field_value(target: &mut RouterSettings, property_path: &str, value: &str)
    -> Result<(), MergeError>;
```
The path is split on `[.:\-]` (`ReflectionUtil.java:23`) — `.`, `:` **and `-`** are interchangeable separators, which is quirk Q14: a `--router.x-y=` argument silently becomes a two-segment path. Reproduce the split verbatim (`str::split(|c| matches!(c, '.' | ':' | '-'))`) with a `// Java bug:` marker.

Name resolution (`getFieldByNameOrSerializedName`, :84-115) is a **static table** per struct rather than reflection: each struct gets `const FIELDS: &[FieldSpec]` where `FieldSpec { serialized: &'static str, alternates: &'static [&'static str], java_name: &'static str, kind: FieldKind }`. Lookup tries, in Java's order and all case-insensitively: `serialized`, `snake_to_lower_camel(serialized)`, each `alternate` and its camel form, then `java_name` and `snake_to_lower_camel(java_name)` — each compared against both the raw segment and `snake_to_lower_camel(segment)`. `snake_to_lower_camel` (:117-130): returns the input unchanged when it has no `_`; otherwise lower-cases part 0 and title-cases the rest (`Character.toUpperCase(first) + rest.toLowerCase()`), **dropping empty parts** (:124). Java's superclass fallback (:111-113) is `// not ported:` — none of these structs has a superclass.

Array navigation (:46-72) is the subtle half and is quirk Q15:
- the value is split on `,` (no trim at split time; each token is `.trim()`ed only when it is handed to the recursive call at :71);
- if the array field is `None`, allocate **exactly `tokens.len()`** elements — *not* the board's real layer count;
- write `min(array_len, tokens.len())` elements — extra tokens are dropped, extra elements untouched;
- a `None` element is instantiated with `Default::default()` before recursing (:66-69).

`convert_value` (:132-205), per `FieldKind`:
- `I32`/`I64` → `Integer.parseInt`/`Long.parseLong` semantics: optional `+`/`-`, ASCII digits only, **no** underscores, no whitespace tolerance → `MergeError::NumberFormat` on failure.
- `F64`/`F32` → `Double.parseDouble`/`Float.parseDouble` semantics: accepts `1e5`, `Infinity`, `NaN`, a trailing `d`/`f`/`D`/`F` suffix and leading/trailing whitespace (Java's `parseDouble` trims). Write the parser by hand; do **not** hand this to Rust's `str::parse::<f64>()` without normalising the suffix and the `Infinity` spelling first, and pin every difference with a test.
- `Bool` → `"0"` → `false`, `"1"` → `true`, otherwise `Boolean.parseBoolean` = **case-insensitive `"true"`, everything else silently `false`** (:145-154). Quirk Q16: `--router.enabled=yes` gives `false` with no error. `// Java bug:` + quirks row.
- `Enum(&[(&str, T)])` → case-insensitive match on the **Java constant name** after `.trim()` (:158-164). **No match falls through the whole chain** and Java then `field.set`s the raw `String` into an enum field, throwing `IllegalArgumentException` which the caller logs (`EnvironmentVariablesSource.java:81-89`, `CliSettings.java:97-99`). The port returns `MergeError::EnumName`, which those two callers swallow the same way.
- `StringVec`/`F64Vec`/`I32Vec` → `.trim()`, empty → empty vec, else split on `,` with each token `.trim()`ed (:165-202).
- `String` → the value verbatim (`targetType.isInstance(value)` short-circuits at :133-135).

**Tests (write first, `tests/field_path.rs`) — ported from `ReflectionUtilArrayTest.java` (75 lines, all four cases):**
- `set_simple_property`: `set_field_value(&mut s, "enabled", "false")` → `Some(false)`; `"true"` → `Some(true)`.
- `set_nested_array_properties_when_null`: on a fresh `RouterSettings::new()` (layers `None`), `"layers.routable" = "false,true"` gives `layers.len() == 2`, `[Some(false), Some(true)]`.
- `set_nested_array_properties_when_initialized`: after `set_layer_count(2)`, `"layers.routable" = "false,true"` then `"layers.preferred_direction_horizontal" = "true,false"` gives `[Some(true), Some(false)]`.
- `case_insensitive_and_serialized_name_matching`: all four spellings from `ReflectionUtilArrayTest.java:50-73` resolve — `layers.preferred_direction_horizontal`, `layers.preferredDirectionHorizontal`, `LAYERS.PREFERRED_DIRECTION_HORIZONTAL`, `layers.routable`.
- New, pinning the quirks: `"layers.routable" = "a,b,c"` on a 2-element array writes 2 and drops `c` (Q15); the same on a `None` array allocates **3** elements (Q15's second half); `"router.x-y"`-shaped input — `set_field_value(&mut s, "optimizer-max_passes", "7")` — resolves as a two-segment path and sets `optimizer.max_passes` (Q14); `"enabled" = "yes"` gives `Some(false)` with `Ok(())` (Q16); `"optimizer.board_update_strategy" = "global_optimal"` and `"GLOBAL_OPTIMAL"` both give `GlobalOptimal`, `"globalOptimal"` gives `MergeError::EnumName`; `"optimizer.hybrid_ratio" = "1:1"` — note the value contains a **separator character**, but only the *path* is split, so it survives verbatim; `"scoring.preferred_direction_trace_cost" = "1.5, 2.0"` gives `[1.5, 2.0]`; `"max_passes" = " 7 "` gives `MergeError::NumberFormat` (Java's `parseInt` rejects the spaces) while `"copper_to_edge_clearance_um" = " 7 "` gives `Some(7.0)` (Java's `parseDouble` trims) — two different tolerances in one file, pin both; `"nope"` gives `MergeError::NoSuchField`.

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(settings): ReflectionUtil.setFieldValue — path split, name resolution, value conversion`.

---

### Task 4: `RouterSettings` accessors, clamps, `set_layer_count`, `clone`, `validate`

**Files:** `crates/fr-settings/src/router_settings.rs` (fill in the body); `crates/fr-settings/tests/router_settings.rs`.
**Java:** `settings/RouterSettings.java:120-259, 440-560, 590-900, 931-965`.

**Interfaces produced** — the null-coalescing accessors, each with its Java line and its default:

| Rust | Java | Behaviour |
|---|---|---|
| `get_run_router()` | :550-552 | `enabled.unwrap_or(true)` |
| `get_run_optimizer()` | :559-568 | `optimizer.as_ref().and_then(\|o\| o.enabled).unwrap_or(false)` |
| `get_vias_allowed()` | :594-597 | `unwrap_or(true)` |
| `get_via_costs()` / `set_via_costs(i32)` | :599-610 | get `unwrap_or(1)`; set `max(value, 1)` |
| `get_plane_via_costs()` / `set_plane_via_costs(i32)` | :612-623 | get `unwrap_or(1)`; set `max(value, 1)` |
| `get_start_ripup_costs()` / `set_start_ripup_costs(i32)` | :536-547 | get `unwrap_or(1)`; set `max(value, 1)` |
| `get_layer_active(usize)` / `set_layer_active` | :631-667 | out of range → `false` (get) / no-op (set); element `None` → `true`; else `routable.unwrap_or(true)` |
| `get_bend_cost(usize)` / `set_bend_cost` | :675-703 | set clamps to `[MIN_BEND_COST 0.0, MAX_BEND_COST 9.9]` (:17-18); get out of range → `0.0`; element or `bend_cost` `None` → the **clamped** `scoring.default_bend_cost`, else `0.0` |
| `get_preferred_direction_is_horizontal(usize)` | :733-749 | out of range → `false`; element `None` → `layer % 2 == 1`; else `unwrap_or(layer % 2 == 1)` |
| `get_preferred_direction_trace_costs(usize)` / setter | :757-790 | set out of range → no-op, else reallocate the array when its length ≠ layer count, store `max(value, 0.1)`, **and set `board_specific_trace_costs_applied = Some(true)`** (:770); get out of range → `0.0`, array absent or short → `1.0` |
| `get_against_preferred_direction_trace_costs(usize)` / setter | :795-813 | same shape, `max(value, 0.1)` at :851 |
| `get_horizontal_trace_costs(usize)` / `get_vertical_trace_costs(usize)` | :818-874 | swap the two arrays by preferred direction — **and index `scoring.preferred_direction_trace_cost[layer]` with no null/length guard** (:825-827, :869-871), unlike their siblings two methods up. Quirk Q10: the port panics with an `.expect` naming the Java line (a faithful port of Java's NPE), and a `#[should_panic]` test pins it |
| `get_trace_costs()` | :877-889 | `Vec<ExpansionCostFactor>` of `(horizontal, vertical)` per entry of `preferred_direction_trace_cost`; empty vec when `scoring` or the array is absent. `ExpansionCostFactor { horizontal: f64, vertical: f64 }` is defined **here** (`autoroute/maze/AutorouteControl.java:118` in Java) with an `obligation:` marker naming Plan 6 as the owner |
| `get_neck_width_um()` | :526-529 | `neck_width_um.filter(\|v\| *v > 0.0).unwrap_or(0.0)` |
| `is_strict_drc()`, `get_automatic_neckdown()` | :531-534, :891-894 | `unwrap_or(false)` both |
| `get_layer_count()` | :442-448 | `layers.as_ref().map_or(0, Vec::len)` |
| `set_layer_count(usize)` | :455-478 | reallocate `layers` **only** when absent or a different length, clearing the applied flag; then **unconditionally** reallocate both cost arrays and re-seed *every* layer (`routable = Some(true)`, both other fields `None`, both costs `1.0`) — quirk Q11: an unchanged layer count still wipes previously parsed costs. `// Java bug:` + row |
| `clone()` | :487-523 | Java calls `setLayerCount(layerCount)` **first** (:490-492), which resets the applied flag and both cost arrays, then replaces `scoring` wholesale (:519) and restores the flag (:521) — correct only by accident of ordering (quirk Q12). Rust's derived `Clone` is a plain deep copy, which is what Java's net effect is; a comment names the accident and a test asserts the derived clone equals a manual replay of Java's sequence |
| `validate(&HostEnvironment)` | :932-965 | `max_passes`: `< 0 \|\| > 9999 → 9999`; `== 0 → i32::MAX`. `max_threads`: `None → default_max_threads()`; `< 0 → default_max_threads()`; `> cores → cores`; **`0` stays `0`** (:951 tests `>`, not `>=`) — quirk Q4, deliberately different from `normalize_max_threads`. `trace_pull_tight_accuracy`: `< 1 → 500`. **`max_passes` and `trace_pull_tight_accuracy` are dereferenced unboxed** (:934, :958) — an NPE if a merge starts from a source that omits them. Quirk Q5: the port returns `Err(SettingsError::…)`? **No** — it panics with an `.expect` naming the Java line, because `SettingsMerger.merge` calls `validate()` unconditionally and a caller that skips `DefaultSettings` is exactly the reachable Java crash. `#[should_panic]` test |
| `set_max_threads(Option<i32>, &HostEnvironment)` | :176-186 | `normalize_max_threads`: `None → default`, `< 0 → default`, **`0 → cores`**, else `min(v, cores)` (:137-149); then mirrors the result into `optimizer.max_threads` (:183-185). Quirk Q4's other half |
| `set_max_passes`, `set_job_timeout_string`, `set_enabled`, `set_run_router`, `set_run_optimizer`, `set_vias_allowed`, `set_automatic_neckdown` | :167-238, :554-592 | plain setters; every `pcs.firePropertyChange` is `// not ported:` |

`add_property_change_listener` / `remove_property_change_listener` (:152-164) → `// not ported:` (GUI binding).

**Tests (write first, `tests/router_settings.rs`) — ported from `BendCostSettingsTest.java` (115) and `NeckWidthSettingsTest.java` (28):**
- `default_bend_cost`: `RouterSettings::new().scoring.default_bend_cost == Some(0.0)` after `DefaultSettings` seeding — **without** seeding it is `None` and `get_bend_cost(0)` on a 2-layer settings answers `0.0`.
- `set_get_bend_cost`: after `set_layer_count(2)`, `get_bend_cost(0) == 0.0` and `get_bend_cost(1) == 0.0`; `set_bend_cost(0, 2.5)` / `set_bend_cost(1, 5.0)` → `2.5` / `5.0`; `set_bend_cost(0, -1.0)` → `0.0`; `set_bend_cost(1, 99.0)` → `9.9`.
- `null_scoring_safety` (BendCostSettingsTest:62-83): `get_start_ripup_costs() == 1` before any set, `5` after `set_start_ripup_costs(5)`; `get_via_costs() == 1` then `3`; `get_preferred_direction_trace_costs(0) == 1.0` then `2.0` after the setter.
- `neck_width`: `RouterSettings::new().get_neck_width_um() == 0.0`; `neck_width_um = Some(130.0)` survives `clone()`; `neck_width_um = Some(-5.0)` gives `0.0`.
- `validate` matrix on `HostEnvironment::with_processors(4)`: `max_passes` `Some(-1) → 9999`, `Some(10_000) → 9999`, `Some(0) → i32::MAX`, `Some(50) → 50`; `max_threads` `None → 3`, `Some(-1) → 3`, `Some(0) → 0`, `Some(9) → 4`, `Some(2) → 2`; `trace_pull_tight_accuracy` `Some(0) → 500`, `Some(1) → 1`.
- `set_max_threads` on the same host: `None → 3`, `Some(-1) → 3`, `Some(0) → 4`, `Some(9) → 4`, `Some(2) → 2`, and `optimizer.max_threads` mirrors each answer — **and the `0` row differs from `validate`'s `0` row**, which is the whole point of quirk Q4; assert the two side by side in one test named `validate_and_normalize_disagree_about_zero_max_threads`.
- `set_layer_count_rewipes_costs` (Q11): `set_layer_count(2)`, `set_preferred_direction_trace_costs(0, 2.5)`, `set_layer_count(2)` again → back to `1.0`, and `board_specific_trace_costs_applied` is **still** `Some(true)` (the reallocation branch at :456 did not fire, so :457 did not clear it) — read the branch carefully, this is the subtle half.
- `set_layer_count_resets_applied_flag` (port of `Issue729TraceCostSettingsTest.java:110-118`): a *different* layer count does clear it.
- `#[should_panic]` `validate_panics_without_max_passes` and `#[should_panic]` `horizontal_trace_costs_panics_without_the_array`.

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(settings): RouterSettings accessors, clamps, setLayerCount, clone, validate`.

---

### Task 5: `apply_board_specific_optimizations`

**Files:** `crates/fr-settings/src/board_optimizations.rs`; `crates/fr-settings/tests/board_optimizations.rs`.
**Java:** `settings/RouterSettings.java:240-259, 266-435`.

**Interfaces produced:**
```rust
impl RouterSettings {
    /// `RouterSettings.applyBoardSpecificOptimizations` (RouterSettings.java:266-435).
    pub fn apply_board_specific_optimizations(&mut self, board: &Board);
    /// `RouterSettings.applyBoardSpecificOptimizationsIfNeeded` (:245-253).
    pub fn apply_board_specific_optimizations_if_needed(&mut self, board: &Board);
    /// `RouterSettings.areBoardSpecificTraceCostsApplied` (:257-259).
    pub fn are_board_specific_trace_costs_applied(&self) -> bool;
}
```
The body is transcribed in Java's exact order (`f64` throughout, ruling 5):
1. `horizontal_width = board.get_bounding_box().width() as f64`, `vertical_width = … .height() as f64` (:267-268); `layer_count = board.get_layer_count()` (:270).
2. Snapshot the originals (:272-295) — only the log line at :388-435 reads them, which is `// not ported:`; keep the snapshot out entirely and say so.
3. `horizontal_add = 0.1 * java_round(10.0 * horizontal_width / vertical_width) as f64`, `vertical_add = 0.1 * java_round(10.0 * vertical_width / horizontal_width) as f64` (:299-303).
4. Layers (:306-324): reallocate only when absent or a different length, **preserving existing elements by index** (:311-312) and clearing the applied flag (:307); otherwise fill `None` elements in place (:319-323).
5. Cost arrays (:325-334): reallocate each **when absent or its length ≠ layer count**, clearing the applied flag each time.
6. `scoring.default_preferred_direction_trace_cost` / `default_undesired_direction_trace_cost` default to `Some(1.0)` when absent (:338-343).
7. `current_preferred_is_horizontal = horizontal_width < vertical_width` (:345); `initialize_trace_costs = !matches!(self.board_specific_trace_costs_applied, Some(true))` (:346).
8. Per layer (:348-374): **toggle `current_preferred_is_horizontal` first, and only for signal layers** (:349-351); non-signal layers get `routable = Some(false)` (:352-353), else a `None` `routable` becomes `Some(true)` (:354-356); a `None` `bend_cost` becomes `scoring.default_bend_cost.unwrap_or(0.0)` — **unclamped here**, unlike `set_bend_cost` (:357-360); a `None` `preferred_direction_horizontal` takes the running flag (:361-363); when `initialize_trace_costs`, both cost entries are overwritten from the defaults and the direction-matched penalty is **added to the undesired entry only** (:365-373).
9. When `initialize_trace_costs` and `signal_layer_count() > 2` (:375-386): `outer = 0.2 * signal_layer_count as f64` added to index `0` and `layer_count - 1` of **both** arrays; then `board_specific_trace_costs_applied = Some(true)`.

**Tests (write first, `tests/board_optimizations.rs`) — ported from `Issue729TraceCostSettingsTest.java` (115) and `RouterSettingsMergeTest.java:40-84`, on the same synthetic boards those tests build.** The Rust twin uses `fr_board::Board::new` with two signal layers `"Top"`/`"Bottom"`, a default clearance matrix and an `IntBox` outline, exactly as the Java tests do (`Issue729TraceCostSettingsTest.java:28-45`).
- `apply_board_specific_optimizations_initializes_trace_costs_once` on a **2 000 000 × 1 000 000** board: `horizontal_add == 2.0`, `vertical_add == 0.5`; `current_preferred` starts `false` (2e6 < 1e6 is false), toggles to `true` on layer 0 and back to `false` on layer 1 → `get_preferred_direction_is_horizontal(0) == true`, `(1) == false`; `get_preferred_direction_trace_costs(0) == 1.0` and `(1) == 1.0`; `get_against_preferred_direction_trace_costs(0) == 3.0` and `(1) == 1.5`. `signal_layer_count() == 2`, so **no** outer-layer bonus. A second call changes nothing.
- `apply_board_specific_optimizations_preserves_user_trace_costs_on_second_call` (Java :68-79): after the first call, `set_against_preferred_direction_trace_costs(0, 4.5)`, `(1, 3.2)`, `set_preferred_direction_trace_costs(0, 2.0)`; a second call leaves all three exactly.
- `apply_board_specific_optimizations_if_needed_skips_when_already_board_tuned` (Java :81-92) and `…_runs_when_layer_count_mismatch` (Java :94-105): a fresh `RouterSettings` with `get_layer_count() == 0` gets 2 layers, the flag set, and both layers active.
- `apply_board_specific_optimizations_preserves_settings` (port of `RouterSettingsMergeTest.java:40-84`) on a **2 000 000 × 2 000 000** board: pre-set `layers[0].routable = Some(false)`, `layers[1].routable = Some(true)`, `preferred_direction_horizontal = Some(true)/Some(false)`; after the call all four survive. Note the square board makes both penalties `0.1 * java_round(10.0) == 1.0` — assert that too, it is the only fixture in the suite where they are equal.
- `outer_layer_bonus_on_a_four_signal_layer_board`: four signal layers, 2 000 000 × 1 000 000 → `outer = 0.8`; `preferred[0] == 1.8`, `preferred[3] == 1.8`, `preferred[1] == 1.0`; `undesired[0] == 3.0 + 0.8 == 3.8` (layer 0 toggles to horizontal, so it takes `horizontal_add`), `undesired[3] == 1.5 + 0.8 == 2.3`.
- `non_signal_layers_are_forced_unroutable_and_do_not_toggle_the_direction`: a `[signal, plane, signal]` stack — layer 1 gets `routable == Some(false)` and does **not** advance the running direction, so layers 0 and 2 both come out horizontal.

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(settings): applyBoardSpecificOptimizations with the exact aspect-ratio penalty`.

---

### Task 6: `SettingsSource`, `SettingsMerger`, and the five file/API sources

**Files:** `crates/fr-settings/src/merger.rs`, `src/sources/{mod.rs,default_settings.rs,dsn_file.rs,rules_file.rs,ses_file.rs,api.rs}`; `crates/fr-settings/tests/sources.rs`.
**Java:** `settings/{SettingsSource.java (42), SettingsMerger.java (199)}`, `settings/sources/{DefaultSettings.java (169), DsnFileSettings.java (69), RulesFileSettings.java (109), SesFileSettings.java (52), ApiSettings.java (38)}`.

**Interfaces produced:**
```rust
pub trait SettingsSource {
    fn get_settings(&self) -> Option<&RouterSettings>;   // Java's `null` return (SettingsMerger.java:150)
    fn get_source_name(&self) -> String;
    fn get_priority(&self) -> i32;
}
pub mod priority {                                        // SettingsSource.java:35-40
    pub const DEFAULT: i32 = 0;  pub const JSON_FILE: i32 = 10; pub const DSN_FILE: i32 = 20;
    pub const SES_FILE: i32 = 30; pub const RULES_FILE: i32 = 40; pub const GUI: i32 = 50;
    pub const ENVIRONMENT: i32 = 55; pub const CLI: i32 = 60; pub const API: i32 = 70;
}
pub struct SettingsMerger { sources: Vec<Box<dyn SettingsSource>> }
impl SettingsMerger {
    pub fn new(sources: Vec<Box<dyn SettingsSource>>) -> Self;      // SettingsMerger.java:60,72
    pub fn add_or_replace_sources(&mut self, new_sources: Vec<Box<dyn SettingsSource>>); // :107-126
    pub fn merge(&self, host: &HostEnvironment) -> RouterSettings;  // :133-193
}
```
`merge` (:133-193): empty sources → `RouterSettings::new()` **returned immediately, without `validate()`** (:134-137) — that early return is why `SettingsMergerTest.emptySourcesList` does not hit quirk Q5's NPE; sort ascending by priority with a **stable** sort (Java's `List.sort` is stable, and two sources can share a priority); the first non-`None` source's settings are `clone()`d as the base (:160-168); every later one goes through `apply_new_values_from` (:171); `validate()` at the end (:189). `add_or_replace_sources` replaces on same-`TypeId` (ruling 9); Java's extra `isAssignableFrom` arm (:116) is `// not ported:` with the quirk-Q13 reference.

Sources:
- `DefaultSettings::new(host: &HostEnvironment)` — the whole table from `DefaultSettings.java:96-155`, verbatim, in Java's assignment order. Every constant is exported as an associated `const` so nothing repeats a magic number (`DEFAULT_UNROUTED_NET_PENALTY = 5_000_000.0f32` :27, `DEFAULT_CLEARANCE_VIOLATION_PENALTY = 1_000_000.0f32` :34, `DEFAULT_BEND_PENALTY = 10.0f32` :40, `DEFAULT_VIA_COSTS = 50` :46, `DEFAULT_PLANE_VIA_COSTS = 5` :52, `DEFAULT_START_RIPUP_COSTS = 100` :60, `DEFAULT_PREFERRED_DIRECTION_TRACE_COST = 1.0` :67, `DEFAULT_UNDESIRED_DIRECTION_TRACE_COST = 1.0` :75, `DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM = 500.0` :78, `DEFAULT_HOLE_CLEARANCE_UM = 0.0` :81). `layers`, `scoring.preferred_direction_trace_cost` and `scoring.undesired_direction_trace_cost` stay `None` — deliberately, per the comment at :88-93 — and `result_json_path` is never assigned at all.
- `impl From<DsnRouterSettings> for RouterSettings` and `impl From<&RouterSettings> for DsnRouterSettings` — the Plan 3 ruling-5 obligation. Forward: `run_router → enabled`, `run_optimizer → optimizer.enabled`, `vias_allowed → vias_allowed`, `via_costs`/`plane_via_costs`/`start_ripup_costs → scoring.*`, `layer_active[i] → layers[i].routable`, `preferred_direction_is_horizontal[i] → layers[i].preferred_direction_horizontal` (**`Option<bool>` survives as `Option<bool>`** — Plan 3 kept the nullability precisely so this conversion can), the two cost vectors → `scoring.*_trace_cost`; **everything else `None`**. Reverse: apply the getters' defaults (`get_layer_active`, `get_preferred_direction_is_horizontal`'s `layer % 2 == 1`, both trace-cost getters' `1.0`) — lossy but faithful. `DsnRouterSettings` has **no `Eq`** (it holds `f64`), so the round-trip test compares field by field, not with `assert_eq!` on the whole struct. Tick the obligation in `docs/plan-3-handoff.md` in Task 12.
- `DsnFileSettings::new(dsn: impl Read, filename: &str)` — `DsnFileSettings.java:28-53`: call `fr_dsn::read_metadata`; on `BoardReadResult::Success { metadata: Some(m), .. }` take `m.router_settings` and `m.layer_count`; start from the extracted settings or a blank `RouterSettings::new()`; **then, when `layer_count > 0 && rs.get_layer_count() == 0`, call `set_layer_count(layer_count)`** (:46-48). Quirk Q18: this makes the DSN source contribute a full `layers[]` of `routable = Some(true)` **and both cost arrays of `1.0`** at priority 20 for essentially every board, which then blocks `copy_fields` rule 5 for every later source — the single most consequential quirk in the plan. `// Java bug:` + row + a cross-reference from `copy_fields.rs`'s array arm.
- `RulesFileSettings::new(rules: impl Read, file_name: &str)` — `RulesFileSettings.java:81-93`: `fr_dsn::rules_reader::read_router_settings`; `Some(s)` → `RouterSettings::from(s)`; `None` or any error → `RouterSettings::new()`, never an `Err`. Java's four constructors collapse to one plus a `from_path(&Path)` helper reproducing :38-41/:57-69's "file missing → blank settings" (`// renamed:` for the collapse).
- `SesFileSettings::new(file_name: &str)` — **the explicit no-op** (ruling 1): `get_settings` returns a blank `RouterSettings::new()`, priority 30, `get_source_name() == format!("SES file: {file_name}")`. It has no constructor argument beyond the name in Java either (`SesFileSettings.java:22-25`). Keep it, with a doc comment saying it exists so the priority ladder is complete and so nobody re-adds a SES tier believing it does something.
- `ApiSettings::new(settings: Option<RouterSettings>)` — `ApiSettings.java:20-22`: `None` → blank. Priority 70.

**Tests (`tests/sources.rs`) — ported from `SettingsMergerTest.java` (278), `DsnFileSettingsTest.java` (99), `RulesFileSettingsTest.java` (81):**
- `default_settings_only` (Java :20-31): `SettingsMerger::new(vec![DefaultSettings])` → `max_passes == Some(9999)`, `get_run_router()`, `get_vias_allowed()`, `get_run_optimizer()` all true.
- `empty_sources_list` (Java :33-41): `max_passes == None` and **no panic** (the early return, not `validate`).
- `source_priorities` (Java :43-51): `DefaultSettings.get_priority() == 0`, `get_source_name() == "Default Settings"`; each other source's priority and name string asserted verbatim (`"freerouting.json"`, `"DSN file: <name>"`, `"SES file: <name>"`, `"RULES file: <name>"`, `"Environment Variables"`, `"CLI Arguments"`, `"API Settings"`).
- `null_value_handling` (Java :92-121): a source returning `None` at priority 100 leaves `max_passes == Some(9999)`.
- `rules_file_settings_priority_is_40` (`RulesFileSettingsTest.java:17-22`) plus both fixture goldens, which are this plan's **golden inputs**:
  - `../freerouting/fixtures/Issue191-processor.Z80/processor.rules`: `get_vias_allowed()` true, `get_via_costs() == 50`, `get_plane_via_costs() == 5`, `get_start_ripup_costs() == 100`, `get_run_router()` true, `get_run_optimizer()` true, `get_layer_count() == 2`, both layers active; layer 0 horizontal with costs `1.0`/`2.5`; layer 1 vertical with costs `1.0`/`1.7`.
  - `../freerouting/fixtures/Issue029-hw48na_valid.rules`: vias allowed, `50`/`5`/`100`, `get_layer_count() == 2`; layer 0 **vertical** with `1.0`/`2.0`; layer 1 **horizontal** with `1.0`/`2.0`.
  Both go through `parity::require_java_dir()` so they skip with a message when the sibling checkout is absent (Plan 3 ruling I).
- `dsn_file_settings_seeds_the_layer_count` (port of `DsnFileSettingsTest.java`): a 2-layer and a 4-layer fixture **with no `(autoroute_settings)` block** each give `get_layer_count()` equal to the DSN's layer count, every layer `routable == Some(true)`, and both cost arrays present and all-`1.0` — the Q18 evidence.

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(settings): SettingsSource, SettingsMerger, and the DSN/rules/SES/API/default sources`.

---

### Task 7: `EnvironmentVariablesSource`, `CliSettings`, and the dead `LegacyBridge`

**Files:** `crates/fr-settings/src/sources/{env.rs,cli.rs}`; `crates/fr-settings/tests/{env_source.rs,cli_source.rs}`.
**Java:** `settings/sources/{EnvironmentVariablesSource.java (133), CliSettings.java (135)}`, `settings/GlobalSettings.java:474-509, 521-838`.

**Interfaces produced:**
- `EnvironmentVariablesSource::new(environment: &BTreeMap<String, String>)` — `EnvironmentVariablesSource.java:42-99`. `EnvironmentVariablesSource::from_process_env()` is the `System.getenv()` overload (:33-35). Per entry: **upper-case the key first** (:56), so variable names are case-insensitive (pinned by `EnvironmentVariablesSourceTest.java:245-257`); skip anything not starting with `FREEROUTING__ROUTER__` (:59-61); strip that prefix and replace `__` with `.` (:64-65); hand the result to `set_field_value`. `MergeError::NoSuchField` and every other error are warned about and **skipped**, never fatal (:74-89). `get_parsed_variables()` and `get_parsed_count()` (:121-132) are ported (they have test callers). Java iterates a `HashMap` in unspecified order; the port takes a `BTreeMap` so the parse order is deterministic — a `// renamed:` deviation, and a comment noting that two variables resolving to the same field would then differ from Java (no test does; say so).
- `CliSettings::new(args: &[String])` — `CliSettings.java:25-110`, exactly two shapes:
  - `--name=value`: `split("=", 2)` on the text after `--`; `"router.enabled"` sets `has_explicit_router_enabled` (:50-52); the setting is applied **only when `name` starts with `router.`** (:54-56), with that prefix stripped (:91-92) before `set_field_value`.
  - `-flag [value]`: the value is consumed only when the next arg exists **and does not start with `-`** (:61), otherwise the value is the empty string. `de` and `do` set their flags (:63-67); `map_flag_to_property` maps **only** `mp → "router.max_passes"` and `mt → "router.max_threads"` (:104-109), everything else `None`.
  - After the loop: `-de` **and** `-do` present **and** no explicit `--router.enabled` → `enabled = Some(true)` (:79-83). This is a source-level forcing at priority 60, so it beats a priority-10 `enabled = false`.
  - Every parse failure is caught and warned (:97-99), never fatal.
- `pub struct LegacyBridge { pub optimizer_max_threads: Option<i32>, pub optimization_improvement_threshold: Option<f32>, pub board_update_strategy: Option<BoardUpdateStrategy>, pub item_selection_strategy: Option<ItemSelectionStrategy>, pub hybrid_ratio: Option<String>, pub ignore_net_classes: Option<Vec<String>>, pub router_enabled: Option<bool>, pub drc_enabled: Option<bool> }` and `pub fn apply_command_line_arguments(args: &[String]) -> LegacyBridge` — ruling 8. **Nothing in `resolve.rs` reads it.** Every field carries a `// Java bug:` line naming the `GlobalSettings.java` site and the fact that the value never reaches the router. Normalisation is exactly `docs/cli-legacy-flags.md`, re-derived from the Java while implementing:
  - `-mp`: `Integer.decode` (so `0x10` and `010` parse as 16 and 8 — `decode`, not `parseInt`); `< 0 → 1`; `> 9999 → 9999`; `0` kept (:675-686).
  - `-mt`: `Integer.decode`; `< 0 → 0`; `> 1024 → 1024` (:688-698). Feeds `optimizer.max_threads` on the bridge **while the same argv token feeds `RouterSettings.max_threads` through `CliSettings`** — quirk Q4; assert both in one test.
  - `-oit`: `Float.parseFloat(v) / 100` **as `f32`**, then `<= 0.0 → 0.0` (:700-708). Keep the `f32` division — an `f64` divide by 100 gives a different bit pattern.
  - `-us`: `to_lowercase().trim()`, `"global" → GlobalOptimal`, `"hybrid" → Hybrid`, **anything else → Greedy** (:710-719).
  - `-is`: `to_lowercase().trim()`, **prefix** match `starts_with("seq") → Sequential`, `starts_with("rand") → Random`, else `Prioritized` (:721-731).
  - `-hr`: `.trim()`, stored as a raw `String` (:732-736).
  - `-inc`: `split(',')` with **no trim** (:810-815) — `-inc "GND, VCC"` gives `["GND", " VCC"]`.
  - `-drc`: sets `router_enabled = Some(false)` and `drc_enabled = Some(true)` **before** looking for a value, and is matched before `-dr` (:660-669).
  - Every branch uses Java's `args[i].startsWith("-mp")` prefix test, not equality — so `-mpx 5` also matches `-mp`. Reproduce it and pin one case.
- `pub fn classify_de_arguments(args: &[String]) -> DeSlots` (ruling 10) — `GlobalSettings.java:564-648`: consume **every** following argument that does not start with `-`; `.trim()` each; take it verbatim if the path exists, else split on `+` dropping empty parts; classify by **lower-cased extension** — `.dsn` → design input, `.ses` → session, `.rules` → rules, `.json` → design input when no `.dsn` has been seen yet, else session (:609-621); refilling a slot keeps the **last** (:601-637); any other extension is warned about and **dropped** (:638-644).

**Tests (`tests/env_source.rs`, `tests/cli_source.rs`) — ported from `EnvironmentVariablesSourceTest.java` (279, all 15 cases) and `GlobalSettingsCommandLineTest.java` (260):**
- Env: `FREEROUTING__ROUTER__MAX_PASSES=50 → Some(50)`; the same key **lower-cased** works (:245-257); `FREEROUTING__ROUTER__OPTIMIZER__MAX_THREADS=8 → optimizer.max_threads == Some(8)`; `…__OPTIMIZER__BOARD_UPDATE_STRATEGY=GREEDY` and `=greedy` both give `Greedy`; `…__OPTIMIZER__HYBRID_RATIO=1:1` gives the string `"1:1"` (the value is not path-split); `…__SCORING__PREFERRED_DIRECTION_TRACE_COST=1.5,2.0` gives `[1.5, 2.0]`; `…__ALGORITHM=freerouting-router-v19` gives that string; `…__SAVE_INTERMEDIATE_STAGES=true`; an unknown property warns and leaves the settings untouched with `get_parsed_count() == 0`; a bad value (`MAX_PASSES=abc`) likewise; a non-`FREEROUTING__ROUTER__` key is ignored; priority is 55 and the name is `"Environment Variables"`.
- CLI: `--router.enabled=false` + `--router.optimizer.enabled=false` merged over `DefaultSettings` gives `get_run_router() == false` and `get_run_optimizer() == false` (`SettingsMergerTest.java:177-189`); `-de a.dsn -do a.ses` over a priority-10 source with `enabled = Some(false)` gives `get_run_router() == true` (:191-224); adding `--router.enabled=false` flips it back (:226-241); `-de a.dsn --router.layers.routable=false,true` still leaves the router enabled (:243-256); `--router.neck_width_um=250` gives `get_neck_width_um() == 250.0` (`NeckWidthSettingsTest.java:13-17`); `-mp` at the end of argv (no value) is a no-op; `-mp -5` consumes nothing because `-5` starts with `-`.
- `-de` matrix from `GlobalSettingsCommandLineTest.java`: `+`-separated lists, spaces around entries, case-insensitive extensions (`.DSN`, `.Ses`), multiple files of one kind keeping the last, a real file whose name contains `+`, and an unknown extension being dropped rather than falling back to the design slot.
- `legacy_bridge_is_dead`: `apply_command_line_arguments(&["-oit", "5", "-us", "global", "-is", "seq", "-hr", "2:3", "-inc", "GND, VCC"])` gives `optimization_improvement_threshold == Some(0.05f32)`, `board_update_strategy == Some(GlobalOptimal)`, `item_selection_strategy == Some(Sequential)`, `hybrid_ratio == Some("2:3")`, `ignore_net_classes == Some(vec!["GND", " VCC"])` — **and** the `CliSettings` built from the same argv has all of `optimizer.optimization_improvement_threshold`, `optimizer.board_update_strategy`, `optimizer.item_selection_strategy`, `optimizer.hybrid_ratio` and `ignore_net_classes` still `None`. That second half is the assertion that makes ruling 8 a test rather than a comment.
- `mt_feeds_two_fields_with_different_clamps`: `["-mt", "2000"]` gives `LegacyBridge::optimizer_max_threads == Some(1024)` and `CliSettings.get_settings().max_threads == Some(2000)`, which `validate` on `with_processors(4)` then caps at `4`.
- `oit_divides_before_it_clamps`: `["-oit", "-5"]` gives `Some(0.0f32)`.

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(settings): env and CLI sources, plus the dead legacy-flag bridge`.

---

### Task 8: `resolve_headless` — the single clean merge, and the two-merge equivalence matrix

**Files:** `crates/fr-settings/src/resolve.rs`; `crates/fr-settings/tests/precedence.rs`.
**Java:** `Freerouting.java:125-146, 1408-1413`, `management/jobs/RoutingJobScheduler.java:103-186`, `io/specctra/RulesReader.java:153-157`.

**Interfaces produced:**
```rust
/// Everything the headless path can feed the merge. Every field is optional; `None` means the
/// corresponding Java source was never registered.
pub struct SettingsInputs<'a> {
    pub dsn: Option<&'a RouterSettings>,            // DsnFileSettings, priority 20
    pub cli_rules: Option<&'a RouterSettings>,      // RulesFileSettings from -dr / -de …rules  (merge #1)
    pub scheduler_rules: Option<&'a RouterSettings>,// RulesFileSettings the scheduler resolved  (merge #2)
    pub env: Option<&'a RouterSettings>,            // EnvironmentVariablesSource, priority 55
    pub cli: Option<&'a RouterSettings>,            // CliSettings, priority 60
}
/// Ruling 1: the linear form of Java's two merges plus the post-merge rules re-apply.
pub fn resolve_headless(
    inputs: &SettingsInputs<'_>,
    board: Option<&Board>,
    host: &HostEnvironment,
) -> RouterSettings;
/// How the scheduler picks `scheduler_rules` (RoutingJobScheduler.java:111-152): job rules,
/// else `-dr`, else an adjacent `<design>.rules` next to the DSN.
pub fn resolve_scheduler_rules_path(
    job_rules: Option<&Path>, cli_rules: Option<&Path>, dsn_path: Option<&Path>,
) -> Option<PathBuf>;
```
The body is ruling 1's nine steps, each line carrying the Java site it stands for. Two details that are easy to get backwards and each get their own test:
- **`validate()` runs twice** — once as merge #1's (`SettingsMerger.java:189`) and once as merge #2's — and the `fill_absent_from(scheduler_rules)` step sits **between** them. Since `validate` only touches `max_passes`, `max_threads` and `trace_pull_tight_accuracy`, and `.rules` carries none of them, the second call is idempotent; assert that rather than assuming it.
- **`apply_board_specific_optimizations` runs last** (`RoutingJobScheduler.java:186`) and can overwrite per-layer trace costs the merge produced, because `board_specific_trace_costs_applied` is `private` and therefore never copied by `copy_fields` (quirk Q9). The consequence — a `.rules` file's explicit per-layer trace costs are discarded in the headless path — is **the single most surprising behaviour in this plan** and gets its own named test.

**Tests (`tests/precedence.rs`).** Two groups.

*(a) The equivalence matrix (ruling 1's Rust-side proof).* A table-driven test builds, for each of **40** cases, both forms:
- the **two-merge form**: `SettingsMerger::new(vec![DefaultSettings, DsnFileSettings, CliRules?, Cli, Env]).merge()` → merged1; then `SettingsMerger::new(vec![DefaultSettings, DsnFileSettings, SchedulerRules?, Cli, Env, ApiSettings(merged1)]).merge()`; then `apply_new_values_from(scheduler_rules)`; then `apply_board_specific_optimizations(board)`;
- the **linear form**: `resolve_headless(...)`;
and asserts they are equal field-for-field. The 40 cases are the cross product of: `{no DSN, 2-layer DSN with no autoroute block, 2-layer DSN with an autoroute block, 4-layer DSN}` × `{no rules, cli rules only, adjacent rules only, cli rules ≠ adjacent rules}` × `{no env, env setting max_passes + a per-layer direction}` × `{no cli, cli setting max_passes + optimizer.enabled}`, pruned to the 40 reachable combinations (the pruning rule and the list are written into the test file, not left implicit).

*(b) The precedence assertions themselves, each named after the quirk it pins:*
- `rules_outrank_env_and_cli_for_autoroute_fields` (Q2): DSN gives `via_costs = 10`, env gives `--router.scoring.via_costs=20`, CLI gives `30`, `.rules` gives `40` → the answer is **40**. The same test with `max_passes` (which `.rules` does not carry) gives the CLI's value, proving the effect is scoped to `(autoroute_settings)`'s fields.
- `adjacent_rules_reach_only_the_fields_merge_one_left_null` (Q1, and the correction to the survey): merge #1 has no rules; the adjacent rules set `layers[0].preferred_direction_horizontal = Some(true)` **and** `via_costs = 99`. The direction survives (merged1 left it `None`); `via_costs` also ends at 99 — but through step 8's post-merge re-apply, not step 7's fill. Prove the distinction by running the same case with the post-merge apply disabled in a helper and asserting `via_costs == 50` there. Write that helper as `#[cfg(test)]` in `resolve.rs`, not as public API.
- `ses_tier_is_a_no_op` (Q3): adding `SesFileSettings` at priority 30 to either form changes nothing.
- `rules_per_layer_trace_costs_are_discarded_in_the_headless_path` (Q9 + Q18): a `.rules` with `(preferred_direction_trace_costs 2.5)` on layer 0, a DSN with 2 layers and no autoroute block, and a 2 000 000 × 1 000 000 board → the final `get_preferred_direction_trace_costs(0)` is **1.0**, not 2.5, because the DSN seeded the array at priority 20 (blocking rule 5 for the rules source and for the post-merge re-apply) and `apply_board_specific_optimizations` then re-initialised it. The test's doc comment carries the whole chain, because anyone reading it will assume a bug.
- `dsn_layer_seeding_blocks_every_later_cost_array` (Q18) in isolation: the same setup without the board call still gives `1.0`.
- `merge_with_no_default_source_panics_in_validate` (Q5): `SettingsMerger::new(vec![CliSettings::new(&["--router.enabled=true"])]).merge()` panics — `#[should_panic]`, with the Java line in the message.

Steps: tests → fail → implement → pass → fmt/clippy/test/audit → commit `feat(settings): resolve_headless — Java's real precedence as one linear merge`.

---

### Task 9: The `p4t1` differential driver and the 40-case matrix

**Files:** `scripts/differential/java/P4T1.java`, `scripts/differential/rust/src/bin/p4t1.rs`, `scripts/differential/matrix/p4t1-cases.tsv`, `scripts/differential/run.sh` (+`p4t1`), `scripts/differential/README.md`.

**`P4T1.java <cases.tsv> <case-index|all> <mode>`** — compiled and run in `run.sh`'s existing `needs_jar=1` mode (ruling 7): `$FREEROUTING_JAR` = `../freerouting/build/libs/freerouting-current-executable.jar`, `$JAVA25_HOME`, `-Djava.awt.headless=true`. It declares `package app.freerouting.settings;` so it can read the package-private state it needs, and it **transcribes** `Freerouting.java:1408-1413` + `:125-146` + `RoutingJobScheduler.java:103-186` in ~50 lines, quoting each Java line number in a comment block at the top — every merge-relevant class it calls (`DefaultSettings`, `JsonFileSettings`, `CliSettings`, `EnvironmentVariablesSource`, `DsnFileSettings`, `RulesFileSettings`, `ApiSettings`, `SettingsMerger`, `RulesReader.read`, `RouterSettings.applyBoardSpecificOptimizations`) is the **real** class. The transcription is the one risk (ruling 1's cost-if-wrong); the mitigation is that the quoted line numbers are checkable by hand and Task 11's review re-reads them.

Driver details:
- The first output line is a header: the jar path, its mtime, `Runtime.getRuntime().availableProcessors()`, and the case index. The Rust twin prints the same header with `HostEnvironment::with_processors(<the value Java printed>)`, so the machine-dependent defaults are pinned rather than assumed equal.
- `JsonFileSettings` is constructed as `new JsonFileSettings(Path.of(tempDir, "freerouting.json"))` on a directory the driver creates empty, and the driver **aborts with `JSON_SOURCE_NOT_EMPTY`** unless every field of its `getSettings()` is null. That turns spec §2's "no persistent config file" from an assumption into a check.
- Environment is passed as a `Map` to `new EnvironmentVariablesSource(map)` (the public testing constructor, `EnvironmentVariablesSource.java:42`), because `System.getenv` cannot be set from inside the JVM.
- `mode 0` — the canonical dump: every field of the merged `RouterSettings` reached by reflection over the declared fields of `RouterSettings`, `ScoringSettings`, `OptimizerSettings`, `FanoutSettings` and each `LayerSettings`, emitted as `path=value` lines **sorted by path**, with `null` for null, `Double.toString`/`Float.toString` for the two float types (Plan 3's `java_double_to_string`/`java_float_to_string` on the Rust side) and the enum's `name()` for enums. Transients are included; `boardSpecificTraceCostsApplied` is included too (it is observable through `areBoardSpecificTraceCostsApplied()`). The change count is **not** printed (ruling 2).
- `mode 1` — the Gson JSON of the same object through `GsonProvider.GSON` with `RouterSettingsTypeAdapterFactory`, for Task 10's serde key parity. Only the fields Gson round-trips appear, which is itself the thing being compared.
- `mode 2` — `case-index` is ignored and every case runs, printing `CASE <i>` separators; this is what `sweep` uses.

**`p4t1-cases.tsv`** — one case per line, tab-separated: `name`, `dsn-fixture-or-"-"`, `cli-rules-or-"-"`, `scheduler-rules-or-"-"`, `env` (`K=V;K=V`), `argv` (space-separated), `board-fixture-or-"-"`. The 40 rows are exactly Task 8's matrix, so the two proofs cover the same ground. Fixtures come from `../freerouting/fixtures`: at minimum `Issue191-processor.Z80/processor.dsn` + `processor.rules`, `Issue029-hw48na_valid.rules`, `Issue593-BBD_Mars-64.{dsn,rules}` and a 4-layer board (pick one from the corpus and name it in the file).

**Acceptance:** `scripts/differential/run.sh p4t1 scripts/differential/matrix/p4t1-cases.tsv all 0` reports `MATCH` on all 40 cases, and mode 1 likewise. Any diff is either a port bug or a new quirk row — never a silently relaxed comparison. Record the case count and the runtime in `scripts/differential/README.md`'s table, alongside a paragraph on the transcription risk and how to re-check it.

Steps: write `P4T1.java` → write `p4t1.rs` → write the matrix → run mode 0 → fix → run mode 1 → README table → fmt/clippy/test/audit → commit `test(settings): p4t1 differential — the real Java two-merge over a 40-case matrix`.

---

### Task 10: JSON in and out — serde parity with Gson

**Files:** `crates/fr-settings/src/router_settings.rs` (serde attributes finalised), `src/lib.rs` (re-exports); `crates/fr-settings/tests/json.rs`.
**Java:** `util/gson/{GsonProvider.java (20), RouterSettingsTypeAdapterFactory.java (70)}`, `settings/sources/JsonFileSettings.java (79)`.

**What is ported and what is not.** Spec §2 drops the persistent `freerouting.json`, so `JsonFileSettings` itself is **not** ported (`// not ported: JsonFileSettings — spec §2, no persistent config file; priority 10 stays reserved in `priority::JSON_FILE``). What *is* needed is `Serialize`/`Deserialize` on `RouterSettings` that agrees with Gson's key set, because Plan 8's MCP `settings?` input, `list_settings` and `--set section.field=value` all round-trip through it (spec §13).

**Interfaces produced:**
- `RouterSettings: Serialize + Deserialize` with the rename/alias table from Task 1, plus `#[serde(default)]` on every field so a partial object deserialises with the rest `None` — that is what makes a JSON object a *settings source* rather than a whole settings value.
- `pub fn from_json_str(s: &str) -> Result<RouterSettings, SettingsError>` and `pub fn to_json_string_pretty(&self) -> Result<String, SettingsError>`. Gson is configured pretty-printing, HTML-escaping off, lenient (`GsonProvider.java:12-20`); `serde_json::to_string_pretty` uses two-space indent, which matches, but the **key order** is declaration order in both — assert it.
- The `transient` gap is real and must be reproduced on the *write* side: Gson drops `maxItems`, `saveIntermediateStages`, `ignoreNetClasses`, `scoring.preferredDirectionTraceCost`, `scoring.undesiredDirectionTraceCost`, `optimizer.boardUpdateStrategy`, `optimizer.hybridRatio` and `optimizer.itemSelectionStrategy`, and re-reads **only `layers`** through the custom factory (`RouterSettingsTypeAdapterFactory.java:59-64`). The port marks each of those `#[serde(skip_serializing)]` and keeps them deserialisable, which is exactly what the factory achieves for `layers` and what plain `transient` achieves for the rest. Each carries a comment naming the Java line, and `tests/json.rs` asserts the emitted key set **exactly**.

**Tests (`tests/json.rs`):**
- `emitted_key_set_matches_gson`: serialise a fully-populated `RouterSettings` and assert the top-level keys are exactly `["enabled","algorithm","fanout","copper_to_edge_clearance_um","hole_clearance_um","neck_width_um","strict_drc","job_timeout","max_passes","layers","trace_pull_tight_accuracy","allowed_via_types","automatic_neckdown","optimizer","scoring","max_threads","result_json"]` in that order, and that `max_items`, `save_intermediate_stages` and `ignore_net_classes` are **absent**.
- `scoring_and_optimizer_key_sets` likewise, with `preferred_direction_trace_cost`/`undesired_direction_trace_cost` absent from `scoring` and `board_update_strategy`/`hybrid_ratio`/`item_selection_strategy` absent from `optimizer`.
- `partial_object_deserialises_as_a_source`: `{"max_passes": 42}` gives `max_passes == Some(42)` and every other field `None`.
- `aliases_round_trip`: `{"viaCosts": 7}` inside `scoring` gives `Some(7)`; serialising emits `via_costs`.
- `p4t1_mode_1_parity`: a `#[cfg_attr(debug_assertions, ignore)]` test that reads the mode-1 golden the differential wrote (committed under `tests/golden/p4t1-mode1/`) and asserts the port's JSON matches after `parity::normalize_whitespace`.

Steps: tests → fail → implement → pass → run `p4t1` mode 1 → fmt/clippy/test/audit → commit `feat(settings): serde JSON in/out with Gson-compatible keys`.

---

### Task 11: Audit to zero, the `// not ported:` roster, README

**Files:** `crates/fr-settings/src/lib.rs`, `crates/fr-settings/README.md`, `scripts/audit-map/fr-settings.map`, `scripts/differential/README.md`.

**Audit to zero.** All three invocations exit 0 with the map supplied and no `UNMAPPED` line:
```
scripts/audit-port.sh settings crates/fr-settings/src \
  'RouterSettings.java LayerSettings.java ScoringSettings.java OptimizerSettings.java FanoutSettings.java DesignRulesCheckerSettings.java DebugSettings.java SettingsSource.java SettingsMerger.java GlobalSettings.java' \
  scripts/audit-map/fr-settings.map
scripts/audit-port.sh settings/sources crates/fr-settings/src '*.java' scripts/audit-map/fr-settings.map
scripts/audit-port.sh util crates/fr-settings/src 'ReflectionUtil.java' scripts/audit-map/fr-settings.map
```
`GlobalSettings.java` has 19 public methods (counted while writing this plan); exactly one — `applyCommandLineArguments` (:521-838) — is ported (as `sources/cli.rs::apply_command_line_arguments`, `// renamed:`). The other eighteen get `// not ported:` markers with reasons: `load`/`saveAsJson`/`getConfigurationFilePath`/`getUserDataPath`/`setUserDataPath`/`lockUserDataPath` (spec §2, no persistent config or user-data dir), `getReleaseSafeVersion` (version check, spec §2), `getCurrentLocale`/`applyNonRouterEnvironmentVariables`/`setValue`/`setDefaultValue` (non-router settings, out of scope — but `setValue`'s path handling is the same `ReflectionUtil` code Task 3 already ports, say so), `getDesignDir`/`getMaxPasses`/`getNumThreads`/`getHybridRatio`/`getBoardUpdateStrategy`/`getItemSelectionStrategy` (readers of the dead bridge, ruling 8), and the constructor.

**The out-of-scope settings roster in `lib.rs`** (R9). One `// not ported:` line per class, each naming the reason, so nobody re-derives them:
`JsonFileSettings` (spec §2, no persistent config file — priority 10 reserved), `GuiSettingsSource` (GUI, priority 50/65), `RuntimeEnvironment`, `AppPaths`, `LoggingSettings`, `StatisticsSettings`, `UserProfileSettings`, `UsageAndDiagnosticDataSettings` (telemetry, spec §2), `FeatureFlagsSettings`, `GuiApplicationSettings` (GUI), `ApiServerSettings`, `ApiAuthenticationSettings`, `NetworkSettings`, `RateLimitSettings` (REST API, spec §2), `McpServerSettings` (HTTP MCP transport — the **stdio** transport is Plan 8 and needs none of this), `GoogleSheetsProviderSettings` (telemetry). Also `management/jobs/RoutingJobScheduler.java` and `management/sessions/**` (`// not ported:` — the job queue is out of scope per spec §2; the *merge sequence* inside the scheduler is what `resolve_headless` reproduces, and the marker says so), and `util/TextManager.parseTimespanString` (`// added in Plan 8:` — `job_timeout_string` is carried as a `String` here and parsed where Java parses it, `RoutingJobSchedulerActionThread.java:44`, with the 24 h `MAX_TIMEOUT` cap at :47-52).

**`crates/fr-settings/README.md`** — the precedence diagram from ruling 1 (both the Java two-merge shape and the linear form), the field table with defaults and clamps, the golden fixtures, how to run `p4t1`, and a prominent note that spec §11's stated order is wrong.

**`crates/fr-settings/tests/corpus.rs`** — a non-`#[ignore]`d test that runs `DsnFileSettings::new` over **every** `.dsn` in `../freerouting/fixtures` (skipping with a printed message via `parity::require_java_dir()`) and asserts no panic and a layer count matching `read_metadata`'s; plus a `#[cfg_attr(debug_assertions, ignore)]` sibling that also runs `RulesFileSettings::new` over every `.rules` in the corpus.

Steps: run the three audits → iterate to zero **without weakening the script** → write the roster and the README → fmt/clippy/test → commit `test(settings): audit to zero, the not-ported roster, README`.

---

### Task 12: Docs — quirks rows #114–#131+, hand-off, obligation ticks

**Files:** `docs/java-quirks.md`, `docs/plan-4-handoff.md`, `docs/plan-3-handoff.md` (tick the three Plan 4 obligations), `docs/cli-legacy-flags.md` (fix the plan numbering).

**`docs/java-quirks.md` rows**, numbering continuing from **#113** (the last row on `plan-3-dsn`, verified while writing this plan) — so Plan 4's first row is **#114**. At minimum, the survey's Q1–Q18 in order:

| new # | survey | what |
|---|---|---|
| #114 | Q1 | merge #2 re-injects merge #1's complete result at priority 70, so the scheduler's own DSN/rules sources reach **only** the fields merge #1 left null. **Include this plan's correction**: `layers[i].preferred_direction_horizontal` and `bend_cost` *do* get through, and the adjacent `<design>.rules` reaches everything else through the post-merge re-apply, not the merger. `RoutingJobScheduler.java:163-170` |
| #115 | Q2 | `.rules` `(autoroute_settings)` is applied after the merge and outranks env/CLI. `RoutingJobScheduler.java:172-181` → `RulesReader.java:153-157` |
| #116 | Q3 | `-oit/-us/-is/-hr/-inc` and `-drc`'s `routerSettings.enabled = false` write only the `@Deprecated` bridge. `GlobalSettings.java:51-53, 662, 700-736, 810-815`; `CliSettings.java:102-110` |
| #117 | Q4 | two `max_threads` fields from one `-mt`, with different clamps; `validate()` and `normalizeMaxThreads` disagree about `0`. `RouterSettings.java:137-149` vs `:943-955`; `GlobalSettings.java:688-698` |
| #118 | Q5 | `validate()` dereferences `maxPasses` and `tracePullTightAccuracy` unboxed. `RouterSettings.java:934, 958` |
| #119 | Q6 | `viasAllowed` serialises as `allowed_via_types` — a `Boolean` under a plural noun; `SettingsMergerTest.java:171-174` has a commented-out assertion admitting it. `RouterSettings.java:78-79` |
| #120 | Q7 | `copyFields` compares boxed scalars with `!=`; the change count is noise (**this port's divergence**, ruling 2). `ReflectionUtil.java:256` |
| #121 | Q8 | object-array merge increments the count by `sourceArray.length` regardless. `ReflectionUtil.java:311, 325` |
| #122 | Q9 | `boardSpecificTraceCostsApplied` is `private transient`, so a file's explicit per-layer trace costs are re-initialised from board geometry. `RouterSettings.java:111, 346, 365-374, 521` |
| #123 | Q10 | `getHorizontalTraceCosts`/`getVerticalTraceCosts` index the cost array with no guard. `RouterSettings.java:825-827, 869-871` |
| #124 | Q11 | `setLayerCount` re-seeds both cost arrays to `1.0` even when the layer count is unchanged. `RouterSettings.java:463-477` |
| #125 | Q12 | `clone()` is correct only by accident of ordering. `RouterSettings.java:487-523` |
| #126 | Q13 | `addOrReplaceSources` replaces on `isAssignableFrom` — safe today, a trap tomorrow. `SettingsMerger.java:107-126` |
| #127 | Q14 | `setFieldValue` splits the path on `-` as well as `.`/`:`. `ReflectionUtil.java:23` |
| #128 | Q15 | array-path values silently truncate, and a null array is allocated at token count. `ReflectionUtil.java:52-72` |
| #129 | Q16 | `Boolean` conversion never fails: `=yes` silently gives `false`. `ReflectionUtil.java:145-154` |
| #130 | Q17 | `SesFileSettings` is a documented no-op — the spec's SES tier has no behaviour. `SesFileSettings.java:27-36` |
| #131 | Q18 | `DsnFileSettings` seeds `layers[]` and both cost arrays at priority 20 whenever the extracted settings carry no layers, blocking rule 5 for every later source. `DsnFileSettings.java:41-50`; `ReflectionUtil.java:285` |

Plus, from this plan's own work: the primitive-`boolean` default suppression that makes `include_warnings = false` unmergeable (`ReflectionUtil.java:236-238` + `DesignRulesCheckerSettings.java:14-19`); `applyBoardSpecificOptimizations`'s **unclamped** `bendCost` default at :357-360 versus `setBendCost`'s clamp at :683; `GlobalSettings.applyCommandLineArguments`'s `startsWith` flag matching (so `-mpx 5` sets `maxPasses`); `RouterSettings.setMaxThreads` silently rewriting `optimizer.maxThreads` (:183-185), which is a *third* writer of that field; and anything found while porting Tasks 2–10.

**`docs/plan-4-handoff.md`:** delivered surface with every public signature; the ten rulings above with what execution confirmed or corrected — ruling 1 in particular must say whether the 40-case equivalence held and whether the survey's Q1 correction survived contact with the JVM; parked residuals per task; obligations for later plans:
- **Plan 5 (`fr-drc`)** — `DesignRulesCheckerSettings` lives here; `enabled` is a primitive `boolean` and therefore unmergeable when `false` (the new quirk row). The DRC report's own JSON schema is Plan 5's, not this crate's.
- **Plans 6/7 (`fr-router`)** — `max_passes == 0` means **unlimited** (`validate` turns it into `i32::MAX`), never "no passes"; `optimizer.max_threads` and `max_threads` are distinct fields with distinct normalisations and `-mt 0` selects the **single-threaded** optimizer (`BatchOptimizer.java:58`); `get_trace_costs()`'s `ExpansionCostFactor` is defined in `fr-settings` with an `obligation:` marker and Plan 6 owns `AutorouteControl`; quirk #122 means the router sees board-derived trace costs, not the `.rules` file's.
- **Plan 8 (`fr-core` + surfaces)** — wire `resolve_headless` and `classify_de_arguments` into `crates/freerouting/src/{cli,legacy}.rs` (ruling 10); **decide whether to wire the five dead legacy flags** (ruling 8) — doing so makes the port more capable than Java and must be a recorded decision, not a drift; `job_timeout_string` still needs `TextManager.parseTimespanString` and the 24 h cap; `schemars` for MCP tool schemas is still unadded (ruling 3) and spec §13's `list_settings` needs it or a hand-written schema.

**`docs/plan-3-handoff.md` ticks:** the three Plan 4 obligations — `DsnRouterSettings → RouterSettings` (Task 6), the three clone-HEAD-only APIs (ruling 7 re-derived them against the HEAD jar; say which are now differentially covered and which remain Rust-tests-only), and legacy-CLI value normalisation (Task 7, ruling 8) — each marked discharged with the commit that did it, or restated as still open with why.

**`docs/cli-legacy-flags.md`:** its header and every "Plan 5" reference say the normalisation is `fr-settings`'s job in **Plan 5**; the Plan 3+ numbering makes `fr-settings` Plan **4**. Fix the numbers and add a line pointing at `sources/cli.rs::apply_command_line_arguments` and ruling 8's "parsed but dead" status.

Commit `docs(settings): quirks rows #114+, Plan 4 hand-off, Plan 3 obligation ticks`.

---

## Dispatch order and sizing

1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10 → 11 → 12. Strictly sequential: Task 2's `CopyFields` and Task 3's `set_field_value` are consumed by every source from Task 6 on, and Task 8 cannot be written before both merge halves and all seven sources exist. The one place the order could be relaxed is 9 before 8 — writing the Java driver first and letting it *derive* the linear form — and the controller may choose that; the plan's order assumes the linear form is derived from the Java source (as ruling 1 does) and then checked, not guessed and then fitted.

Sizing: **Task 2** (the merge engine — eight rules, six structs, and the array semantics every later task depends on) and **Task 8** (the precedence itself, plus a 40-case equivalence matrix) are the two largest — opus, and worth reviewing twice. **Tasks 3, 4, 9** are large — opus (Task 3 because two number grammars and four name-resolution passes hide silent misses; Task 4 because the clamps are where a typo becomes a routing-cost change; Task 9 because the Java transcription is the plan's one unverifiable-by-inspection artefact). **Tasks 5, 6, 7** medium-large — opus for 5 and 7 (the aspect-ratio arithmetic; the dead-flag table), sonnet for 6. **Tasks 1, 10, 11, 12** small-to-medium — sonnet.

Reviewers: opus for 2, 3, 4, 5, 8, 9; sonnet otherwise. From Task 8 on, every review runs `cargo test -p fr-settings --test precedence` on the committed tree, not on the implementer's word; from Task 9 on, every review runs `run.sh p4t1 … all 0` and reads the header line to confirm the jar it actually used.

## Plan self-review

**Spec coverage — including the deviation.** Spec §11's three bullets map to: `RouterSettings` with every field `Option<T>` and derived `Serialize`/`Deserialize` → Tasks 1 and 10 (`JsonSchema` is **deliberately not** derived — ruling 3 defers `schemars` to Plan 8, and the hand-off says so); "merge order (later wins, only `Some` values override)" → Tasks 2, 6, 8 — **but not in the order §11 states**, which ruling 1 replaces with Java's real precedence and which the plan header flags in a call-out box so the user sees it before any code is written; "no persistent config file" → Task 11's `// not ported: JsonFileSettings` with the priority-10 slot reserved, and Task 9's `JSON_SOURCE_NOT_EMPTY` check that turns the assumption into an assertion. Spec §4's dependency line is deviated from by ruling 3 (`fr-settings → fr-dsn`) with the reason recorded. Spec §12's CLI surface and §13's MCP surface are **not** touched (ruling 10); the two pure functions those surfaces will need — `resolve_headless` and `classify_de_arguments` — are built and tested here and named in the hand-off. Spec §14.1 (ported Java tests) → all ten of R8's suites land: `SettingsMergerTest` (Tasks 6, 8), `RouterSettingsMergeTest` (2, 5), `EnvironmentVariablesSourceTest` (7), `GlobalSettingsCommandLineTest` (7), `ReflectionUtilArrayTest` (3), `BendCostSettingsTest` (4), `Issue729TraceCostSettingsTest` (4, 5), `NeckWidthSettingsTest` (4, 7), `DsnFileSettingsTest` (6), `RulesFileSettingsTest` (6, with both `.rules` goldens). Spec §3's bit-parity clauses belong to Plans 3/5/8 and are untouched; the parity contract this plan is measured against is Task 9's differential.
**Deliberately excluded, with citation:** the sixteen out-of-scope `settings/**` classes (Task 11's roster, each with a spec-§2 or GUI/API/telemetry reason), `JsonFileSettings` (spec §2), `GuiSettingsSource` (GUI), eighteen of `GlobalSettings`'s nineteen public methods (Task 11), `PropertyChangeSupport` throughout (no GUI), `management/jobs/**` and `management/sessions/**` (spec §2 drops sessions and the job queue — the merge *sequence* inside the scheduler is reproduced, the scheduler is not), `util/TextManager.parseTimespanString` (Plan 8, marked), `AutorouteControl` beyond the `ExpansionCostFactor` value type (Plan 6, marked `obligation:`).
**Placeholder scan.** No "TBD", no "add error handling", no "similar to Task N". Every task names its Java files with line counts, its produced signatures, its test expectations as literal numbers with their provenance, and its commit message. Three places name a decision the implementer must *make* rather than one this plan makes, each with the default already chosen and the deciding evidence named: the 40-case matrix's pruning rule (Task 8 — the cross product is 64, the plan says write the pruning rule into the test file rather than leaving it implicit), which 4-layer corpus fixture the differential uses (Task 9 — "pick one and name it in the file"), and whether `p4t1` runs before Task 8 (Dispatch order — stated as the controller's call, with the plan's assumption spelled out). All three are decisions-with-a-default, not gaps.
**Type consistency across tasks.** `RouterSettings`, `LayerSettings`, `ScoringSettings`, `OptimizerSettings`, `FanoutSettings`, `BoardUpdateStrategy`, `ItemSelectionStrategy`, `HostEnvironment`, `MergeReport`/`MergeError`/`SettingsError` are all fixed in Task 1 and used unchanged after; `CopyFields` in Task 2; `set_field_value` + `FieldSpec`/`FieldKind` in Task 3; `ExpansionCostFactor` in Task 4 (the one forward reference, to Plan 6, marked at its definition); `SettingsSource`/`SettingsMerger`/`priority::*` and the seven source types in Task 6; `SettingsInputs`/`resolve_headless` in Task 8. The plan makes **no edits to `fr-dsn`, `fr-board` or `fr-geometry`** — the `From<DsnRouterSettings>` impl lives on `fr-settings`'s side of the dependency edge (orphan-rule-safe, since `RouterSettings` is local), and `fr-board` is read through the five accessors Task 5 names, all of which already exist and were verified while writing this plan. Every Plan 1/2/3 test therefore stays green by construction, and `crates/freerouting` is untouched (ruling 10).
