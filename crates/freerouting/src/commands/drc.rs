//! `freerouting drc` — `Freerouting.initializeDrc` (`Freerouting.java:246-374`) end to end.
//!
//! # The thirteen steps, and where each one is
//!
//! Unlike `initializeCli`, this function is the whole program: there is no scheduler, no action
//! thread and no job queue on this path — **the job is never enqueued** (`:260` builds a
//! `RoutingJob` and nothing ever hands it to `RoutingJobScheduler`). So the transcription is
//! one-to-one, in Java's own order:
//!
//! | # | Java | here |
//! |---|---|---|
//! | 1 | the input guard `:247-250` | `// not reachable:` below — clap and `legacy::rewrite` both guarantee the slot |
//! | 2 | the session create `:253-257` | `// not ported:` below |
//! | 3 | `new RoutingJob(drcSession.id)` `:260` | [`RoutingJob::new`] |
//! | 4 | `drcJob.drc = globalSettings.drcReportFile` `:261` | [`report_file_details`] |
//! | 5 | `FRLogger.info` + `drcJob.setInput` `:262-268` | [`RoutingJob::set_input`] — **catch ⇒ exit 1** |
//! | 6 | `BoardLoader.loadBoardIfNeeded` `:271-274` | [`fr_core::load_board_if_needed`] — **false ⇒ exit 1** |
//! | 7 | the `.rules` load `:277-294` | [`load_rules_file`] |
//! | 8 | the `.json`-vs-`.ses` session branch `:296-329` | [`load_session_file`] |
//! | 9 | `new DesignRulesChecker(board, drcSettings)` `:332-333` | [`fr_drc::DesignRulesChecker::new`] |
//! | 10 | `coordinateUnit = "mm"` `:336`, `sourceFileName` `:339`, `generateReport` `:340` | [`DrcReportOptions`] + [`fr_drc::DesignRulesChecker::generate_report`] |
//! | 11 | the quality-score sub-merge `:342-352` | [`quality_score`] — **quirk #272** |
//! | 12 | `GsonProvider.GSON.toJson(report)` `:354` | `fr_drc::KiCadDrcReport::to_json` |
//! | 13 | the file-vs-stdout write `:356-371` (**IOException ⇒ exit 1**), `return true` `:373` | [`write_report`] |
//!
//! # The load order is DSN → `.rules` → session, and it is load-bearing (quirk #273)
//!
//! Steps 6, 7 and 8 are three writes to the same board, and their order changes the answer.
//! **The channel is `defaultItemClearanceClasses`, and it is narrower than "the rules change the
//! clearances".** Most of what a `(rules …)` scope writes — the clearance matrix's values, the
//! padstacks, the via rules, the snap angle — is read at *check* time and so is order-blind. What
//! is not: `RulesReader`'s `(rule …)` arm reaches `Structure.setClearanceRule`, which calls
//! `appendClearanceClass` for either half of a clearance-class **pair** that is not already in the
//! matrix (`Structure.java:756`, `:765`), and `appendClearanceClass` (`:826-840`) writes the
//! **default net class's `defaultItemClearanceClasses`** for the four names `via`, `pin`, `smd`,
//! `area`. `SesReader.processViaScope` reads that field at **via-creation** time
//! (`SesReader.java:395-400`), and `processWireScope` the `TRACE` slot (`:316-321`) — so a via made
//! in step 8 takes the class step 7 installed, and one made before it would not.
//!
//! Measured, on `Issue593-BBD_Mars-64.dsn` + its `.ses` with a one-rule
//! `(rule (clearance 400.0 (type smd_via)))`: **15** clearance violations rules-first, **0**
//! session-first. The HEAD jar's own run on those three files reports 15, i.e. the rules-first
//! board. A clearance with **no** `(type …)` is the control — `setClearanceRule` returns at
//! `:684-707` after `setDefaultValue`, having touched no item class — and gives 463 either way.
//!
//! `crates/freerouting/tests/cli_e2e.rs::the_session_is_imported_after_the_rules` is that pair of
//! measurements: it requires the two orders to **differ** on the typed file, to **agree** on the
//! class-blind control, and then requires the binary's own report to carry the rules-first count
//! and its log to show `:281` before `:309`. `tests/reference/drc-issue593-rules` and
//! `drc-issue593-ses` are the two committed references that exercise the two optional slots
//! singly; `p8t3 e2e`'s `rules-and-session` row is the only run that fills both at once.
//!
//! # `-drc` exits 0 whatever it finds (quirk #271)
//!
//! `initializeDrc` returns `true` unconditionally (`:373`). A missing `.rules` file (`:289`), a
//! missing session file (`:324`), a failed quality score (`:350`) and **any number of violations**
//! all leave `globalSettings.cliExitCode` at its `0` default (`GlobalSettings.java:122`). Exactly
//! three sites produce a `1`, and all three are `System.exit(1)` inside this function: `:267`
//! (the input is unreadable), `:273` (the board will not load) and `:366` (the report cannot be
//! written). A `-drc` run is therefore **not** a pass/fail gate on the design, and a script that
//! treats it as one is reading a number that never moves.
//!
//! # What is deliberately absent
//!
// not ported: Freerouting.initializeDrc's SessionManager bookkeeping (:253-257) —
//   `SessionManager.getInstance().createSession(UUID.fromString(userProfileSettings.userId),
//   "Freerouting/" + version)`. The same singleton `initializeCli` registers into, with the same
//   readers (the REST API's `/sessions` resources, rostered in `crates/fr-core/src/lib.rs` §1).
//   The port builds the [`RoutingJob`] directly. **Note what does *not* happen next**: unlike
//   `initializeCli:114`, this path never calls `session.addJob(job)` and never reaches
//   `RoutingJobScheduler.enqueueJob` — the job exists only as a carrier for `input`/`rules`/`drc`.
// not ported: Freerouting.initializeDrc's `drcJob.routerSettings` (:285's fourth argument to
//   `RulesReader.read`) — Java hands the job's settings to the reader so the file's
//   `(autoroute_settings …)` block can be written into them, and then **nothing ever reads them
//   again**: the quality score at :342-352 builds its own `RouterSettings` from a fresh merger
//   (quirk #272) and no router runs on this path. The port passes `None` for
//   `fr_dsn::rules_reader::read`'s `target_settings`, which is the same observable program.
// not ported: Freerouting.initializeDrc's `globalSettings.drcSettings` (:333, the
//   `DesignRulesChecker` constructor's second argument) — plan-5 ruling 12 measured that the
//   field is stored and never read by any method of the class, so `fr_drc::DesignRulesChecker`
//   has no settings parameter at all. The marker on that struct carries the evidence.

use std::path::Path;

use fr_core::{BoardFileDetails, BoardStatistics, FileFormat, RoutingJob, SessionId};
use fr_drc::DesignRulesChecker;
use fr_drc::report::{DrcCoordinates, DrcJsonFlavor, DrcReportOptions};
use fr_settings::sources::{
    CliSettings, DefaultSettings, DsnFileSettings, EnvironmentVariablesSource, JsonFileSettings,
};
use fr_settings::{HostEnvironment, RouterSettings, SettingsMerger, SettingsSource};

use crate::cli::{DrcArgs, DrcSchema};
use crate::legacy::ExitCode;

/// `Freerouting.initializeDrc` (`Freerouting.java:246-374`).
///
/// `settings_argv` is the **raw** argv, not the rewritten one — see [`crate::run`]'s "The raw
/// argv is the settings argv". Only [`quality_score`] reads it, because the DRC path has no other
/// settings consumer: `drcJob.routerSettings` stays `new RouterSettings()` throughout.
pub fn run(args: &DrcArgs, settings_argv: &[String]) -> ExitCode {
    // ── 1. the input guard (`:247-250`) ───────────────────────────────────────────────────────
    //
    // not reachable: Freerouting.initializeDrc:247-250 — `globalSettings.initialInputFile ==
    // null`. The slot is non-optional here: clap requires the positional input, and
    // `legacy::rewrite` emits the `FRLogger.error` at `:248` itself (keyed `Freerouting.java:248`
    // in `crate::logging::MESSAGE_MAP`) and answers an **empty** argv, which `crate::run` turns
    // into exit 1 — the same `System.exit(1)` `:1469-1474` reaches for `cliResult == false`.
    // `crates/freerouting/src/legacy.rs::drc_without_an_input_is_javas_other_refusal` pins it.

    // ── 3-4. the job, and the report file it carries (`:260-261`) ─────────────────────────────
    let mut job = RoutingJob::new(SessionId::NIL);
    job.drc = args.output.as_deref().map(report_file_details);

    // ── 5. `drcJob.setInput` (`:262-268`) ─────────────────────────────────────────────────────
    //
    // `:263` — `FRLogger.info("Loading DSN file for DRC: " + …)`. It comes **before** the call,
    // so it is emitted even for an input that cannot be read.
    tracing::info!("Loading DSN file for DRC: {}", args.input.display());
    if let Err(error) = job.set_input(&args.input) {
        // `:266` — `FRLogger.error("Couldn't load the input file '" + … + "'", e)`, then `:267`'s
        // `System.exit(1)`. Note the difference from `initializeCli`: there is no second
        // `job.input == null` warning here (`:108-112` has no counterpart), because `System.exit`
        // ends the process inside the catch.
        tracing::error!(
            "Couldn't load the input file '{}': {error}",
            args.input.display()
        );
        return ExitCode::Failure;
    }

    // ── 6. `BoardLoader.loadBoardIfNeeded` (`:271-274`) ───────────────────────────────────────
    //
    // **The whole loader, not the parse half.** `initializeCli` has to split it (see
    // `commands::route`'s module docs) because merge #1 needs the parsed board; this path never
    // merges before the load, so `drcJob.routerSettings` is still `new RouterSettings()` and Java
    // calls `BoardLoader.loadBoardIfNeeded(drcJob)` whole. That matters:
    // `applyRouterSettingsForLoadedBoard`'s two board mutations read
    // `copperToEdgeClearanceUm`/`holeClearanceUm`, which `new RouterSettings()` leaves **null**
    // (only `DefaultSettings.getSettings` fills them, `:107-108`), so both overrides no-op — and
    // that is why the eight committed `tests/reference/drc-*` documents match a plain
    // `fr_dsn::read_board`. Calling the parse half instead would be the same board today and a
    // divergence the moment either default changed.
    let loaded = match fr_core::load_board_if_needed(&mut job) {
        Ok(loaded) => loaded,
        Err(error) => {
            // `BoardLoader.java:32-35`'s "Only DSN and JSON formats are supported" (quirk #274,
            // label S) and `:52`'s `FRLogger.error("Failed to load board", e)` both arrive as this
            // message; Task 3 owns its text.
            tracing::error!("{error}");
            // `:272` — `FRLogger.error("Failed to load board for DRC check", null)`, then `:273`'s
            // `System.exit(1)`.
            tracing::error!("Failed to load board for DRC check");
            return ExitCode::Failure;
        }
    };
    let mut board = loaded.board;
    let transform = loaded.transform;
    for warning in &loaded.warnings {
        tracing::warn!("{warning}");
    }

    // ── 7. the `.rules` load (`:277-294`) ─────────────────────────────────────────────────────
    load_rules_file(args.rules.as_deref(), &job, &mut board, &transform);

    // ── 8. the session load (`:296-329`) ──────────────────────────────────────────────────────
    load_session_file(args.ses.as_deref(), &mut board, &transform);

    // ── 9-10. the checker and the report (`:332-340`) ─────────────────────────────────────────
    let coords = DrcCoordinates {
        board_unit: board.communication.unit,
        transform,
    };
    let options = DrcReportOptions {
        // `:339` — `new File(globalSettings.initialInputFile).getName()`, i.e. the base name.
        source: base_name(&args.input),
        // `:335-336` — hard-coded, quirk #151. **The CLI exposes no unit flag and this is a
        // recorded decision, not an omission**: adding one would make the port strictly more
        // capable than the jar on a surface (`convertCoordinate`'s five arms,
        // `DesignRulesChecker.java:512-525`) whose other four are unreachable from `-drc`. The
        // MCP `check_drc` tool does not expose one either (Task 12). `fr-drc` ports all five
        // regardless, because the crate is not the CLI.
        coordinate_unit: "mm".to_string(),
        // `KiCadDrcReport.java:70` — `ZonedDateTime.now().format(ISO_OFFSET_DATE_TIME)`, which
        // plan-5 ruling 5 made an injected string because `fr-drc` has no clock.
        date: report_date(std::time::SystemTime::now()),
        // `DesignRulesChecker.java:212-213` — `Constants.FREEROUTING_VERSION`, with
        // `generate_report` adding the `"Freerouting "` prefix. Ruling AT/5: the DRC report's
        // `freeroutingVersion` is one of exactly three FILE-FORMAT fields that carry
        // [`fr_core::PARITY_VERSION`], never `CARGO_PKG_VERSION`.
        freerouting_version: fr_core::PARITY_VERSION.to_string(),
        // `:349` assigns the score **after** `generateReport` returns, and so does this runner —
        // see step 11. Java's own field is `null` at construction (`KiCadDrcReport.java:73`).
        quality_score: None,
    };
    let mut checker = DesignRulesChecker::new(&mut board);
    let mut report = checker.generate_report(&coords, &options);

    // ── 11. the quality-score sub-merge (`:342-352`) — quirk #272 ─────────────────────────────
    //
    // Java's order, and it is not interchangeable: `generateReport` has already run, and it
    // **mutates the board** (plan-5 ruling 8 — every clearance query lowers an item's
    // `smallestClearance` and advances the search tree's entry counter). `:348`'s
    // `drcJob.board.getStatistics()` therefore reads the post-report board, and computing the
    // score first would be a different program.
    report.quality_score = job
        .get_input()
        .and_then(|input| quality_score(&mut board, input, settings_argv))
        .map(f64::from);

    // ── 12. `GsonProvider.GSON.toJson(report)` (`:354`) ───────────────────────────────────────
    //
    // **Ruling W, applied explicitly.** `DrcJsonFlavor::default()` is `FreeroutingHead` — the
    // *parity* default `crates/fr-drc/tests/report_json.rs` pins against the jar's own Gson bytes
    // — and the CLI's default is [`DrcSchema::Kicad`], the spelling the document's own `$schema`
    // promises (quirk #154). Passing the flavor by name rather than by `Default` is the point:
    // a later change to the enum's `Default` cannot move the CLI silently.
    let flavor = match args.schema {
        DrcSchema::Kicad => DrcJsonFlavor::KiCad,
        DrcSchema::Freerouting => DrcJsonFlavor::FreeroutingHead,
    };
    let json = match report.to_json(flavor) {
        Ok(json) => json,
        // totalized: GsonProvider.GSON.toJson (Freerouting.java:354) — Gson throws
        //   `IllegalArgumentException` for a non-finite `Double`, and `:354` is **outside** the
        //   `try` that guards the score (`:343-352`), so the exception leaves `initializeDrc`,
        //   leaves `main`, and the JVM dies with a stack trace and exit status 1. The only input
        //   that reaches it is a `qualityScore` of NaN or ±Infinity, which
        //   `BoardStatistics::normalized_score` can produce when `maximumCount *
        //   unroutedNetPenalty` overflows `float` (see its own doc). The port logs and returns 1
        //   — **the same exit code**, so the divergence is the stack trace and nothing else.
        Err(error) => {
            tracing::error!("Couldn't serialise the DRC report: {error}");
            return ExitCode::Failure;
        }
    };

    // ── 13. the write, and the unconditional `return true` (`:356-373`) ───────────────────────
    write_report(job.drc.as_ref(), &json)
}

/// `Freerouting.initializeDrc:261` — `drcJob.drc = globalSettings.drcReportFile`, which
/// `GlobalSettings.java:665-667` built as a `BoardFileDetails` with `format = DRC_JSON` and the
/// `-drc` argument as its filename.
///
/// It is carried as a [`BoardFileDetails`] rather than as the `&Path` this runner already has,
/// because `:358` reads `drcJob.drc.getAbsolutePath()` and that method is **not**
/// `java.io.File.getAbsolutePath()`: it is `Path.of(directoryPath, filename).toString()`
/// (`BoardFileDetails.java:96-98`), i.e. `setFilename`'s split rejoined. For `-drc /a/b/r.json`
/// the two agree; for `-drc r.json` Java answers `"r.json"` where `File.getAbsolutePath()` would
/// answer `"<cwd>/r.json"`. The log line at `:363` is where that is observable.
fn report_file_details(path: &Path) -> BoardFileDetails {
    let mut details = BoardFileDetails::default();
    // `:666`.
    details.format = FileFormat::DrcJson;
    // `:667` — `setFilename(args[i + 1])`, which splits the string into directory and name.
    details.set_filename(Some(&path.to_string_lossy()));
    details
}

/// `new File(globalSettings.initialInputFile).getName()` (`Freerouting.java:339`).
///
/// `File.getName()` is the text after the last separator, with no filesystem access and no
/// normalisation; `Path::file_name` is the same for every input the CLI can be given. A path that
/// ends in a separator (`-de dir/`) answers `""` in Java and `Some("dir")` here — unreachable,
/// because such an input never survives `set_input` (quirk #257 records the jar's own NPE on
/// `-de /`).
pub(crate) fn base_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// `Freerouting.initializeDrc:277-294` — the optional `.rules` file, read **onto the board**
/// before the session import.
///
/// Java's shape, arm for arm: the outer `if` is `initialRulesFile != null` (`:277`), the inner one
/// is `rulesFile.exists()` (`:280`), the `else` warns (`:289`) and the `catch (Exception)`
/// errors (`:292`) — and **none of the three stops the run** (quirk #271).
///
/// # This is not the route path's `.rules` resolution, and the difference is real
///
/// `RoutingJobScheduler.java:115-152` picks the job's own bytes, else `-dr` **if it exists**, else
/// an adjacent `<design>.rules` beside the DSN (quirk #269, label V). `initializeDrc` does none of
/// that: it reads `globalSettings.initialRulesFile` and nothing else, so a `<design>.rules` sitting
/// next to the DSN is picked up by `freerouting route` and **ignored** by `freerouting drc`. That
/// is Java, transcribed, not an omission here.
pub fn load_rules_file(
    rules: Option<&Path>,
    job: &RoutingJob,
    board: &mut fr_board::Board,
    transform: &fr_dsn::CoordinateTransform,
) {
    // `:277`.
    let Some(rules) = rules else {
        return;
    };
    // `:279-280` — `new java.io.File(...)` then `exists()`.
    if !rules.exists() {
        // `:289` — `FRLogger.warn("RULES file for DRC not found: " + …)`, and the run continues to
        // a full report. Quirk #271.
        tracing::warn!("RULES file for DRC not found: {}", rules.display());
        return;
    }
    // `:281`.
    tracing::info!("Loading RULES file for DRC: {}", rules.display());
    // `:283` — `drcJob.name != null ? drcJob.name : "board"`. `RoutingJob::name` is not optional
    // in the port (Task 1: the constructor always derives `J-<short6>`, and `setInput` replaces it
    // with the input's base name **without** the extension, `RoutingJob.java:457`), so the `:
    // "board"` arm is unreachable — and it matters that the name has no `.dsn` on it: `RulesReader`
    // compares it against the file's `(rules PCB <name>)` header and warns on a mismatch
    // (`RulesReader.java:100-110`). `tests/reference/drc-issue593-rules/java.log` records the jar
    // taking that warning branch, because the fixture's header spells the name *with* `.dsn`.
    let design_name = job.name.clone();
    let file = match std::fs::File::open(rules) {
        Ok(file) => file,
        Err(error) => {
            // `:292` — the `catch (Exception e)` around the whole block; `new FileInputStream`
            // is what throws for an unreadable file that `exists()` accepted.
            tracing::error!("Failed to load RULES file for DRC: {error}");
            return;
        }
    };
    // `:284-285` — `RulesReader.read(rulesStream, designName, drcJob.board, drcJob.routerSettings)`.
    // The fourth argument is `None` here; see the `// not ported:` marker in the module docs.
    match fr_dsn::rules_reader::read(file, &design_name, board, transform, None) {
        Ok(_) => {
            // `:286`.
            tracing::info!("RULES file loaded for DRC successfully");
        }
        Err(error) => {
            // `:292`.
            tracing::error!("Failed to load RULES file for DRC: {error}");
        }
    }
}

/// `Freerouting.initializeDrc:296-329` — the optional session file, read **after** the `.rules`
/// (quirk #273).
///
/// The extension test is `toLowerCase().endsWith(".json")` (`:301`), so the two arms are the KiCad
/// JSON session reader and `SesReader`. As with the `.rules` block, a missing file warns
/// (`:324`), a failure errors (`:327`) and neither stops the run (quirk #271).
///
/// # The `.json` arm, and quirk #290 (label U)
///
/// `:304` opens the file with `new java.io.FileReader(sessionFile)` — the one-argument
/// constructor, i.e. `Charset.defaultCharset()` — while every other JSON path in the tree names
/// UTF-8 explicitly. The port decodes UTF-8. **Measured** (docs/java-quirks.md #290): on JDK 18+
/// the two agree, because JEP 400 made the default charset UTF-8 independently of the locale, so
/// the divergence is reachable only under an older JVM or an explicit `-Dfile.encoding`. Pinned
/// by `crates/freerouting/tests/cli_e2e.rs::a_non_ascii_session_file_is_read_as_utf8`.
///
/// Java's `FileReader` replaces an undecodable byte with `U+FFFD` rather than throwing, and so
/// does [`String::from_utf8_lossy`] — so the two also agree on a file that is not valid UTF-8.
pub fn load_session_file(
    session: Option<&Path>,
    board: &mut fr_board::Board,
    transform: &fr_dsn::CoordinateTransform,
) {
    // `:297`.
    let Some(session) = session else {
        return;
    };
    // `:299-300`.
    if !session.exists() {
        // `:324`.
        tracing::warn!("Session file for DRC not found: {}", session.display());
        return;
    }
    // `:301`.
    if session.to_string_lossy().to_lowercase().ends_with(".json") {
        // `:302-303`.
        tracing::info!(
            "Loading KiCad JSON session file for DRC: {}",
            session.display()
        );
        // `:304` — `new FileReader(sessionFile)`, quirk #290's charset. See the doc comment.
        let bytes = match std::fs::read(session) {
            Ok(bytes) => bytes,
            Err(error) => {
                // `:327` — the `catch (Exception e)`; `new FileReader` is what throws for an
                // unreadable file that `exists()` accepted.
                tracing::error!("Failed to load session file for DRC: {error}");
                return;
            }
        };
        // `:305` — `KiCadJsonReader.importSession(jsonReader, drcJob.board)`.
        match fr_dsn::kicad::import_session(&String::from_utf8_lossy(&bytes), board) {
            Ok(()) => {
                // `:306`.
                tracing::info!("KiCad JSON session file loaded for DRC successfully");
            }
            Err(error) => {
                // `:327`.
                tracing::error!("Failed to load session file for DRC: {error}");
            }
        }
        return;
    }
    // `:309`.
    tracing::info!("Loading SES file for DRC: {}", session.display());
    let file = match std::fs::File::open(session) {
        Ok(file) => file,
        Err(error) => {
            // `:327` — the `catch (Exception e)`.
            tracing::error!("Failed to load session file for DRC: {error}");
            return;
        }
    };
    // `:311` — `SesReader.read(sesStream, drcJob.board)`.
    match fr_dsn::ses_reader::read(file, board, transform) {
        Ok(summary) => {
            // `:312-320` — the same three counters and the same parenthesised suffix.
            let errors = if summary.errors_encountered > 0 {
                format!(" ({} errors)", summary.errors_encountered)
            } else {
                String::new()
            };
            tracing::info!(
                "SES file loaded for DRC: {} wires, {} vias imported{errors}",
                summary.wires_imported,
                summary.vias_imported
            );
        }
        Err(error) => {
            // `:327`.
            tracing::error!("Failed to load session file for DRC: {error}");
        }
    }
}

/// `Freerouting.initializeDrc:342-352` — the quality score, and **quirk #272: it is a different
/// settings merge from the router's**.
///
/// ```java
///   var settingsMerger = globalSettings.settingsMergerProtype.clone();          // :344
///   settingsMerger.addOrReplaceSources(
///       new DsnFileSettings(drcJob.input.getData(), drcJob.input.getFilename()));  // :345-346
///   var routerSettings = settingsMerger.merge();                                // :347
///   var finalStats = drcJob.board.getStatistics();                              // :348
///   report.qualityScore = (double) finalStats.getNormalizedScore(routerSettings.scoring);  // :349
/// ```
///
/// The prototype is `new SettingsMerger(new DefaultSettings(), new JsonFileSettings(),
/// new CliSettings(args), new EnvironmentVariablesSource())` (`Freerouting.java:1408-1413`), so
/// the whole chain is **0, 10, 20, 55, 60** — and that is the *entire* difference from the router
/// path, which is why it is a quirk rather than a detail:
///
/// | step the router takes | here |
/// |---|---|
/// | `RulesFileSettings` at priority 40 (`Freerouting.java:129-144`) | **absent** — `-dr x.rules` never reaches the score |
/// | `applyRouterSettingsForLoadedBoard` between the merges (`HeadlessBoardManager.java:741-745`) | **absent** |
/// | merge #2 (`RoutingJobScheduler.java:103-170`) | **absent** |
/// | the post-merge `RulesReader.read` re-apply (`:173-184`) | **absent** |
///
/// So `freerouting drc board.dsn --rules x.rules` scores with weights the rules file never
/// influenced, while the *violations* it reports were computed against the clearances that same
/// file installed. `crates/freerouting/tests/cli_e2e.rs::the_quality_score_uses_a_dsn_only_merge`
/// pins both halves: a `.rules` file carrying `(via_costs 999)` moves the router's
/// `settings_snapshot` and leaves this score untouched.
///
/// # Why this composes `SettingsMerger` by hand instead of calling `resolve_headless`
///
/// [`fr_settings::resolve_headless`] is the **linearised two-merge ladder**, and its premise is
/// that merge #1's result is complete so that merge #2's chain only fills gaps. This path has no
/// merge #2 and no board pass, so feeding it a `SettingsInputs` would answer a different number.
/// The `obligation:` at `crates/fr-settings/src/resolve.rs:204` already says the API path must
/// compose the merger directly for the same structural reason; **this is a second, independent
/// user of that rule**, and the marker there now names both so Task 12 does not read it as its
/// own.
///
/// # The score is an `f32`, widened once
///
/// `BoardStatistics.getNormalizedScore` returns a Java `float` (`BoardStatistics.java:623`) and
/// `KiCadDrcReport.qualityScore` is a `Double` (`:56-57`), so `:349`'s `(double)` cast is the only
/// widening in the program — which is why a score serialises as `902.078369140625` rather than
/// `902.0784`. The caller performs it with `f64::from`, on an `f32` this function never widens.
pub fn quality_score(
    board: &mut fr_board::Board,
    input: &BoardFileDetails,
    settings_argv: &[String],
) -> Option<f32> {
    // `:343-352` is one `try` whose `catch (Exception e)` warns and leaves `qualityScore` null.
    // The port has no exceptions, so Java's one reachable throw site is the `else` below, with
    // the same `:351` warning.
    //
    // `:344-347`.
    let router_settings = quality_score_settings(input, settings_argv);

    // `:348` — `drcJob.board.getStatistics()`, i.e. `new BoardStatistics(this)`
    // (`RoutingBoard.java:1410-1412`).
    let stats = BoardStatistics::new(board);

    // `:349`. `routerSettings.scoring` is null only when the merge produced no `scoring` object,
    // which `merge`'s `validate()` rules out; Java would throw an NPE into `:350`'s
    // `catch (Exception e)` and warn, and so does this arm.
    let Some(scoring) = router_settings.scoring.as_ref() else {
        // `:351`.
        tracing::warn!(
            "Failed to calculate quality score for DRC report: routerSettings.scoring is null"
        );
        return None;
    };
    Some(stats.normalized_score(scoring))
}

/// `Freerouting.initializeDrc:344-347` on its own — the sub-merge, without the board.
///
/// Split out of [`quality_score`] so that `scripts/differential/rust/src/bin/p8t3.rs` can print
/// the **weights** beside the score: a score that diverges is otherwise a single number with no
/// way to tell a merge difference from a `BoardStatistics` difference. It is the `p8t5`
/// convention — the driver links the binary's own library, so what the JVM is compared against is
/// the code the program runs, not a second copy written for the driver.
///
/// # Panics
///
/// Through `RouterSettings::validate` (`SettingsMerger.java:189`), if `DefaultSettings` did not
/// run — which cannot happen here, since the chain starts from it. Quirk #125 is the
/// merger-level version.
#[must_use]
pub fn quality_score_settings(
    input: &BoardFileDetails,
    settings_argv: &[String],
) -> RouterSettings {
    let host = HostEnvironment::detect();

    // `:344` — the prototype merger (`Freerouting.java:1408-1413`), rebuilt rather than cloned:
    // `SettingsMerger::clone` is `not ported:` in `fr_settings::merger` because a
    // `Vec<Box<dyn SettingsSource>>` cannot share instances, and a merge never mutates a source,
    // so a second merger over the same inputs is the same value.
    let mut sources: Vec<Box<dyn SettingsSource>> = vec![Box::new(DefaultSettings::new(&host))];
    // Priority 10. Java's no-argument `new JsonFileSettings()` (`:1410`) resolves the OS user-data
    // path; controller ruling BG measured that the port has **no** default file at all, so the
    // tier is reachable only through `--settings <file>` on the native form. See
    // [`super::json_settings_path`]. `p8t3`'s Java half prints the resolved path and whether it
    // exists on stderr, so an environment where that file *does* carry `router.*` values shows up
    // as a diff rather than as a silent asymmetry.
    if let Some(path) = super::json_settings_path(settings_argv) {
        sources.push(Box::new(JsonFileSettings::new(&path)));
    }
    // Priorities 60 and 55, in `Freerouting.java:1411-1412`'s registration order (the merge sorts
    // by priority, so the order here is cosmetic — kept as Java writes it).
    sources.push(Box::new(CliSettings::new(settings_argv)));
    let environment: std::collections::BTreeMap<String, String> = std::env::vars().collect();
    sources.push(Box::new(EnvironmentVariablesSource::new(&environment)));
    let mut merger = SettingsMerger::new(sources);

    // `:345-346` — the DSN at priority 20, and nothing else. **No `RulesFileSettings` at 40**,
    // which is the whole of quirk #272.
    merger.add_or_replace_sources(vec![Box::new(DsnFileSettings::new(
        input.get_data(),
        input.get_filename(),
    ))]);

    // `:347` — one `merge()`, which ends in one `validate()` (`SettingsMerger.java:189`). The
    // router path calls `validate` twice, which is quirk #140's `max_passes == 0` ladder; nothing
    // the score reads is affected, and the count is stated here so the difference is visible.
    merger.merge(&host)
}

/// `Freerouting.initializeDrc:356-373` — the report goes to the file `-drc` named, or to stdout.
///
/// # Ruling 6: the stdout branch is unreachable in Java, and live here (quirk #275)
///
/// `:368-371`'s `IO.println(drcReportJson)` cannot run in the jar. `main:1462` enters DRC mode on
/// `globalSettings.drcReportFile != null`, and the only assignment of that field is
/// `GlobalSettings.java:664-668`, which runs **only** when `-drc` was followed by a value (a bare
/// `-drc` is quirk #263, label A: two dead booleans and no DRC mode at all). So `drcJob.drc` is
/// non-null on every reachable entry, and the `else` is dead code.
///
/// Spec §12 asks for `drc` to write JSON to stdout when no `-o` is given, so the port makes the
/// branch live — reachable **only from the native subcommand form**, because `legacy::rewrite`
/// always emits `-o <report>` for a `-drc` argv (it has the path, or there was no DRC mode). The
/// halves are therefore cleanly split: everything `p8t3` compares against the jar goes through
/// the file branch, and `crates/freerouting/tests/cli_e2e.rs::drc_with_no_output_prints_to_stdout`
/// is a **port-only** test of a branch no jar run can reach.
fn write_report(drc: Option<&BoardFileDetails>, json: &str) -> ExitCode {
    // `:357`.
    let Some(drc) = drc else {
        // `:368-371` — ruling 6's live branch. `IO.println` is `System.out.println`, i.e. the
        // JSON **plus a newline**; `to_gson_string_pretty` writes no trailing newline of its own
        // (`GsonProvider`'s pretty printer does not), so `println!` is the byte-exact equivalent.
        println!("{json}");
        // `:373`.
        return ExitCode::Ok;
    };
    // `:358` — `drcJob.drc.getAbsolutePath()`; see [`report_file_details`] for why that is not
    // `File.getAbsolutePath()`.
    let output_file_name = drc.get_absolute_path();
    // `:361-362` — `Files.write(Path.of(name), json.getBytes(StandardCharsets.UTF_8))`. A Rust
    // `&str` is already UTF-8, so `as_bytes` is the same bytes.
    match std::fs::write(&output_file_name, json.as_bytes()) {
        Ok(()) => {
            // `:363`.
            tracing::info!("DRC report written to: {output_file_name}");
            // `:373` — `return true`, which `main:1462-1474` turns into `System.exit(0)`.
            ExitCode::Ok
        }
        Err(error) => {
            // `:365` — `FRLogger.error("Couldn't save the DRC report to '" + … + "'", e)`, then
            // `:366`'s `System.exit(1)`. One of exactly three sites that can (quirk #271).
            tracing::error!("Couldn't save the DRC report to '{output_file_name}': {error}");
            ExitCode::Failure
        }
    }
}

/// `KiCadDrcReport`'s `date` (`io/kicad/KiCadDrcReport.java:70`):
/// `ZonedDateTime.now().format(DateTimeFormatter.ISO_OFFSET_DATE_TIME)`.
///
/// # Two rules, and one recorded divergence
///
/// `ISO_OFFSET_DATE_TIME` is `ISO_LOCAL_DATE_TIME` plus an offset id, and it differs from the
/// `ISO_INSTANT` that [`fr_core::format_utc_iso8601`] renders (the manifest's `generated_at`,
/// Task 4) in exactly two ways, both reproduced below:
///
/// 1. **the fraction is minimal, not 3/6/9.** `ISO_LOCAL_TIME` is built with
///    `appendFraction(NANO_OF_SECOND, 0, 9, true)`, which trims trailing zeros — `100_000_000` ns
///    is `.1`, where `Instant.toString()` writes `.100`. The committed
///    `tests/reference/drc-*/drc.json` dates (`…:26.406471-07:00`) are that rule at a
///    microsecond clock.
/// 2. **`:ss` is omitted when second and nanosecond are both zero** — `10:15+01:00` is a legal
///    `ISO_OFFSET_DATE_TIME`, where `Instant.toString()` always prints seconds.
///
/// **The divergence: the port renders UTC, so the offset is always `Z`** — `docs/java-quirks.md`
/// **#276**, registered because both normalisers drop `date` and therefore no gate can see it.
/// Java uses the JVM's
/// default zone, and there is no way to read the host's UTC offset from `std` — every route to it
/// is a dependency (forbidden) or a `libc` call (`#![forbid(unsafe_code)]`). `Z` is a valid
/// `ISO_OFFSET_DATE_TIME` offset id, so the field stays parseable by any consumer; what changes is
/// which wall clock it names. Nothing compares it: `parity::normalize_drc_json` and
/// `scripts/normalize-drc.py` both **drop** `date` (plan-5 ruling 3), because a timestamp cannot be
/// a parity surface, and `fr-drc` takes it as an injected string for the same reason.
pub(crate) fn report_date(time: std::time::SystemTime) -> String {
    // `fr_core::format_utc_iso8601` is `ISO_INSTANT`: `YYYY-MM-DDTHH:MM:SS[.fff|.ffffff|.fffffffff]Z`.
    // Rules 1 and 2 above are the two edits that turn it into `ISO_OFFSET_DATE_TIME` at offset
    // zero. Re-deriving the calendar here instead would be a second copy of
    // `civil_from_days` — the arithmetic Task 4 already pinned against eleven jar-measured rows.
    let instant = fr_core::format_utc_iso8601(time);
    let body = instant.strip_suffix('Z').unwrap_or(&instant);
    // Rule 1: trim the fraction's trailing zeros, and drop the `.` if nothing survives.
    let body = match body.split_once('.') {
        Some((head, fraction)) => {
            let trimmed = fraction.trim_end_matches('0');
            if trimmed.is_empty() {
                head.to_string()
            } else {
                format!("{head}.{trimmed}")
            }
        }
        None => body.to_string(),
    };
    // Rule 2: `HH:MM:00` with no fraction prints as `HH:MM`.
    let body = match body.strip_suffix(":00") {
        Some(without_seconds) if !body.contains('.') => without_seconds.to_string(),
        _ => body,
    };
    format!("{body}Z")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, UNIX_EPOCH};

    /// `ISO_OFFSET_DATE_TIME`'s two rules, at offset zero — see [`report_date`].
    ///
    /// The expectations are `DateTimeFormatter.ISO_OFFSET_DATE_TIME`'s documented composition
    /// (`ISO_LOCAL_DATE_TIME` + `appendOffsetId`, which writes `Z` for a zero offset), applied to
    /// the same instants Task 4 measured `Instant.toString()` on — so the two renderings can be
    /// read side by side.
    #[test]
    fn the_date_is_iso_offset_date_time_not_iso_instant() {
        let base = 1_756_800_000u64; // 2025-09-02T08:00:00Z
        for (nanos, expected) in [
            // Rule 2: no seconds, no fraction.
            (0u32, "2025-09-02T08:00Z"),
            // Rule 1: minimal fraction, where `Instant.toString()` writes `.100`.
            (100_000_000, "2025-09-02T08:00:00.1Z"),
            (120_000_000, "2025-09-02T08:00:00.12Z"),
            (123_000_000, "2025-09-02T08:00:00.123Z"),
            // The committed references' shape: a microsecond clock, six digits, no trailing zero.
            (406_471_000, "2025-09-02T08:00:00.406471Z"),
            (1, "2025-09-02T08:00:00.000000001Z"),
        ] {
            assert_eq!(
                report_date(UNIX_EPOCH + Duration::new(base, nanos)),
                expected,
                "nanos {nanos}"
            );
        }
        // A second that is not zero keeps `:ss` even with no fraction.
        assert_eq!(
            report_date(UNIX_EPOCH + Duration::new(base + 7, 0)),
            "2025-09-02T08:00:07Z"
        );
    }

    /// `File.getName()` (`Freerouting.java:339`).
    #[test]
    fn the_source_is_the_inputs_base_name() {
        assert_eq!(base_name(Path::new("/a/b/board.dsn")), "board.dsn");
        assert_eq!(base_name(Path::new("board.dsn")), "board.dsn");
    }

    /// `BoardFileDetails.getAbsolutePath` is `Path.of(directoryPath, filename)`, **not**
    /// `File.getAbsolutePath()` — so a relative `-drc` argument stays relative.
    #[test]
    fn the_report_path_is_the_argument_rejoined() {
        assert_eq!(
            report_file_details(Path::new("/a/b/r.json")).get_absolute_path(),
            "/a/b/r.json"
        );
        assert_eq!(
            report_file_details(Path::new("r.json")).get_absolute_path(),
            "r.json"
        );
        assert_eq!(
            report_file_details(Path::new("/a/b/r.json")).format,
            FileFormat::DrcJson
        );
    }
}
