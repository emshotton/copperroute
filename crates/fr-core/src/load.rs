//! The board **load sequence** — `HeadlessBoardManager.loadFromSpecctraDsn` (:673-705) →
//! `applyParsedBoardResult` (:711-737) → `applyRouterSettingsForLoadedBoard` (:739-749) →
//! `applyImmediatePostLoadProcessing` (:751-757), plus `BoardLoader.loadBoardIfNeeded` (:19-56).
//!
//! **Free functions: there is no manager object.** `HeadlessBoardManager`'s other ≈ 640 lines are
//! diagnostics and are rostered in [`crate`]'s `// not ported:` block; the three accessors
//! (`getRoutingBoard`, `replaceRoutingBoard`, `getCurrentRoutingJob`) are answered by Rust
//! ownership and by [`crate::RoutingJob`], as `crates/fr-board/src/board/clearance_override.rs`
//! records at its marker site.
//!
//! # The four steps of `applyRouterSettingsForLoadedBoard`, and who owns which
//!
//! ```java
//!   int boardLayerCount = this.board.getLayerCount();                                          // :740
//!   if (routerSettings.getLayerCount() != boardLayerCount) routerSettings.setLayerCount(...);   // :741-744
//!   this.routingJob.routerSettings.applyBoardSpecificOptimizations(this.board);                 // :745
//!   applyCopperToEdgeClearanceOverride();                                                       // :746
//!   applyHoleClearanceOverride();                                                               // :747
//! ```
//!
//! The first two **mutate the settings**, the last two **mutate the board**. The port keeps the
//! same split: [`apply_router_settings_for_loaded_board`] writes `:741-744` and `:745` into the
//! `&mut RouterSettings` it is handed — the port's stand-in for Java's `job.routerSettings`,
//! which is exactly the object Java mutates in place — and then calls
//! [`fr_router::pipeline::prepare_board`], Plan 7 Task 15b's driver over
//! [`fr_board::Board::apply_copper_to_edge_clearance_override`] and
//! [`fr_board::Board::apply_hole_clearance_override`], for `:746-747`. Nothing here
//! re-implements an override.
//!
//! `fr_settings::resolve_headless` runs the *same two settings steps* at
//! `crates/fr-settings/src/resolve.rs:274-284`, because it models the whole scheduler flow
//! (merge #1 → this pass → merge #2). It is **not** called from here: its signature needs the
//! `SettingsInputs` ladder and a `HostEnvironment`, neither of which a loader has, and running it
//! here would re-run both merges. The CLI (Plan 8 Tasks 5-6) is where the two meet: it resolves
//! the ladder against the loaded board and this loader performs Java's in-place pass on the
//! merge-#1 object it is handed. Both cite the same four Java lines, and
//! `crates/fr-core/tests/load.rs::the_settings_pass_is_the_same_two_steps_resolve_headless_runs`
//! asserts they agree.
//!
//! # The overrides run ONCE per load, and the survey said twice
//!
//! Plan 8 survey ruling AD and quirk register row #232 both state that the two clearance
//! overrides run **twice** on a DSN load — once from `HeadlessBoardManager.createBoard:342-343`,
//! "which the parser invokes at `io/specctra/parser/Structure.java:1268`", and again from
//! `applyRouterSettingsForLoadedBoard:746-747`. **Measured at the pinned jar, that is false**, and
//! this task's probe closes it three ways:
//!
//! * **Source.** `Structure.java:1268` calls `scopeParameter.boardHandling.createBoard(...)`.
//!   `ReadScopeParameter` has one constructor and it assigns its `final BoardParserCallback
//!   boardHandling` field `new MinimalBoardManager()` (`ReadScopeParameter.java:103`).
//!   `MinimalBoardManager.createBoard` (`:139-166`) builds the `RoutingBoard` and returns —
//!   it calls neither override, and its `getCurrentRoutingJob()` answers `null`.
//!   `HeadlessBoardManager.createBoard` has no caller outside `GuiBoardManager.java:411`.
//! * **Runtime.** `P8T3Probe`'s `[createboard]` rows load each fixture through a counting
//!   subclass of the real `HeadlessBoardManager`:
//!   `headless_create_board_calls=0` on all three.
//! * **Transcript, from Plan 7's own evidence.** On `Issue555-CNH_Functional_Tester_1.dsn` the
//!   `board_edge` class lands at index **10**, after the seven `(class …)` clearance classes
//!   `Network.java:741` appends *later in the parse* than `Structure.java:1268`. Had the copper
//!   override run at `createBoard`, `board_edge` would be index 3.
//!
//! So the port calls [`fr_router::pipeline::prepare_board`] **once**, at the point
//! `applyRouterSettingsForLoadedBoard:746-747` reaches it, and a second call would be the
//! divergence. `crates/fr-core/tests/overrides.rs::the_override_runs_once_on_a_dsn_load` pins it.
//! Quirk #232 is rewritten and quirk #253 records the dead-code finding.

use fr_board::Board;
use fr_dsn::{BoardMetadata, BoardReadResult, CoordinateTransform, DsnReadOptions};
use fr_router::pipeline::prepare_board;
use fr_settings::RouterSettings;

use crate::{Error, FileFormat, RoutingJob};

/// What a completed load answers.
///
/// # Why the `CoordinateTransform` travels with the board
///
/// Plan 3 ruling A: Java's writers re-derive the DSN↔board transform from the board, the port's
/// (`fr_dsn::ses_writer::write`, `fr_dsn::dsn_writer::write`) take a `&CoordinateTransform`, and
/// only the one `Structure.createBoard` produced round-trips a file's coordinates unchanged. The
/// Plan 3 hand-off's *"whatever holds a `Board` between a read and a write must also hold the
/// `CoordinateTransform`"* obligation is discharged by this struct — [`crate::save`]'s two
/// functions take exactly the pair this holds.
#[derive(Debug)]
pub struct LoadedBoard {
    /// The loaded board, after all four steps.
    pub board: Board,
    /// The transform `Structure.createBoard` built between DSN and board coordinates.
    pub transform: CoordinateTransform,
    /// `BoardReadResult.Success.metadata()`.
    ///
    /// **Always `None` on this path**, and that is Java's: `DsnReader.readBoard` constructs its
    /// `Success` with a `null` metadata (`DsnReader.java:146`) — only `readMetadata` fills one.
    /// The task brief drafted a non-optional `BoardMetadata`; the field is `Option` because the
    /// only value the reader can put here is Java's null (Plan 3, `fr_dsn::BoardReadResult`).
    pub metadata: Option<BoardMetadata>,
    /// `job.routerSettings` after `applyRouterSettingsForLoadedBoard`'s first two steps
    /// (`:741-744`, `:745`).
    pub settings: RouterSettings,
    /// `ReadScopeParameter.warnings` — the reader's non-fatal complaints, which Java logs and
    /// drops. Kept so a caller (the CLI's `-de` path, Task 6) can report them.
    pub warnings: Vec<String>,
}

/// Port of `HeadlessBoardManager.loadFromSpecctraDsn` (HeadlessBoardManager.java:673-705).
///
/// Java's `inputStream` is `InputStream`; the port takes the bytes because
/// [`crate::BoardFileDetails`] already holds them (`job.input.getData()` is a byte array in Java
/// too) and because the load is retried per settings variant in the tests.
///
/// `settings` is Java's `this.routingJob.routerSettings` — mutated in place by
/// [`apply_router_settings_for_loaded_board`], exactly as Java's is.
//
// not ported: the `inputStream == null` guard (:674-676) — `&[u8]` cannot be null; an empty slice
// fails the DSN header check instead and gives the same `ParseError` arm.
// not ported: the three `Loading board file '<name>'...` log lines (:684-696) and the
// `routingJob.logError("There was an error while reading DSN file.", e)` of the `catch` (:702-704)
// — `fr-core` has no logger, and the port's reader answers a `BoardReadResult` rather than
// throwing, so the `catch` arm has nothing to catch. The messages are preserved in
// [`apply_parsed_board_result`]'s error text, which is the only thing a caller can observe.
pub fn load_from_specctra_dsn(
    bytes: &[u8],
    job: &mut RoutingJob,
    settings: &mut RouterSettings,
) -> Result<LoadedBoard, Error> {
    // :677-683 — Java's `inputFilename`, used for the log line and handed to the reader as its
    // `designName`.
    let input_filename = job
        .get_input()
        .map(|input| input.get_filename().to_string())
        .filter(|name| !name.is_empty());
    // :697-698.
    let result = fr_dsn::read_board(
        bytes,
        None,
        input_filename.as_deref(),
        &DsnReadOptions::default(),
    );
    // :700.
    apply_parsed_board_result(result, settings)
}

/// Port of `HeadlessBoardManager.loadFromKiCadJson` (HeadlessBoardManager.java:794-823).
///
/// Java takes the stream and wraps it in a UTF-8 `InputStreamReader` (`:812-813`); the port takes
/// the decoded text for the same reason [`load_from_specctra_dsn`] takes bytes.
// renamed: `HeadlessBoardManager.loadFromKiCadJson` -> `load_from_kicad_json` — Rust's
// `KiCad` -> `ki_cad` transliteration is not a word; the crate spells the vendor `kicad`
// everywhere (`FileFormat::KicadDesignJson`, `fr_dsn::kicad`).
// not ported: the `inputStream == null` guard (:795-797) and the four log lines (:799-810,
// :819-822), for the reasons [`load_from_specctra_dsn`] gives.
pub fn load_from_kicad_json(
    text: &str,
    job: &mut RoutingJob,
    settings: &mut RouterSettings,
) -> Result<LoadedBoard, Error> {
    let _ = job;
    // :814 — `KiCadJsonReader.readBoard(reader, boardObservers, idGenerator)`.
    let result = kicad_read_board(text);
    // :815.
    apply_parsed_board_result(result, settings)
}

// not ported: `HeadlessBoardManager.createBoard` (:310-344) — **dead code at the pinned jar**, and
// the root cause of quirk #232's wrong text (quirk #253). `Structure.java:1268` calls
// `scopeParameter.boardHandling.createBoard(...)`, but `ReadScopeParameter`'s single constructor
// always assigns its `final BoardParserCallback boardHandling` field
// `new MinimalBoardManager()` (`ReadScopeParameter.java:103`), and that class's own `createBoard`
// (`:139-166`) constructs the `RoutingBoard` without either clearance-override call. Measured:
// `P8T3Probe`'s `[createboard]` rows report `headless_create_board_calls=0` for a real
// `loadFromSpecctraDsn` on all three fixtures, through a counting subclass of the real manager.
// The port's board construction is `fr_dsn`'s `Structure::create_board`, inside
// `fr_dsn::read_board`; the outline-clearance-class lookup in front of it
// (`HeadlessBoardManager.java:319-331`, duplicated at `ReadScopeParameter.java:146-157`) is
// `crates/fr-dsn/src/parser/structure.rs`'s.
// not ported: `BoardManager.createBoard` (`management/BoardManager.java:142`) — the interface
// method the above overrides, with the same two implementations and the same reachability.

/// `io/kicad/KiCadJsonReader.readBoard` — **stubbed**.
///
/// The KiCad JSON reader is Plan 8 Task 9's; nothing in the tree reads that format yet. The stub
/// is inert: it answers the `ParseError` variant `BoardReadResult` already has, mutates nothing,
/// and cannot be mistaken for a working reader by a caller that ignores the error.
///
/// The Plan 7 scan's ruling 6 precedent — *no unannounced forward references* — is why this is a
/// named function with a marker rather than a `todo!()` inside [`load_from_kicad_json`].
// obligation: Task 9 (`io/kicad/KiCadJsonReader`) replaces this body with the real reader, which
// lands as `fr_dsn::kicad::read_board`. Until then `-de <board>.json` fails here, with the message
// below, and `crates/fr-core/tests/load.rs::the_kicad_json_reader_is_a_stub` pins that.
fn kicad_read_board(text: &str) -> BoardReadResult {
    let _ = text;
    BoardReadResult::ParseError {
        location: "(kicad_json".to_string(),
        detail: "the KiCad JSON reader is not ported yet (Plan 8 Task 9)".to_string(),
    }
}

/// Port of `HeadlessBoardManager.applyParsedBoardResult` (HeadlessBoardManager.java:711-737): the
/// `BoardReadResult` dispatch, then the two post-load passes.
///
/// Java's `analyticsFormat` argument feeds only `scheduleDeferredPostLoadProcessing`, which is
/// rostered `// not ported:` in [`crate`] (quirk label AE), so the port's signature drops it
/// along with `inputFilename`.
//
// not ported: `scheduleDeferredPostLoadProcessing` (:736 / :759-784) — the virtual thread that
// computes analytics, the CRC32 and the counterpart-board comparison. Rostered in `lib.rs` §7.
pub fn apply_parsed_board_result(
    result: BoardReadResult,
    settings: &mut RouterSettings,
) -> Result<LoadedBoard, Error> {
    // :712-715: `Success` and `OutlineMissing` both hand the board over; every other variant is
    // logged and returned unchanged. Java's `(RoutingBoard) success.board()` is an unguarded cast
    // of a possibly-null field — `BoardReadResult.Success(null, …)` is constructible and
    // `DsnReader` reaches it on a `(pcb name)` whose body produced no board — so a null board
    // leaves the manager with `this.board = null` and the two passes below no-op through their
    // own null guards (`:740`, `:752-754`). The port answers the error instead, because a
    // `LoadedBoard` with no board is not a value this crate can produce.
    let (board, metadata, warnings, coordinate_transform) = match result {
        BoardReadResult::Success {
            board,
            metadata,
            warnings,
            coordinate_transform,
        }
        | BoardReadResult::OutlineMissing {
            board,
            metadata,
            warnings,
            coordinate_transform,
        } => (board, metadata, warnings, coordinate_transform),
        // :720-732 — Java logs and returns the result; the port carries the same two messages.
        BoardReadResult::IoError(error) => {
            return Err(Error::Load(format!(
                "There was an IO error while reading board file.: {error}"
            )));
        }
        BoardReadResult::ParseError { location, detail } => {
            return Err(Error::Load(format!(
                "There was a parse error while reading board file at '{location}': {detail}"
            )));
        }
    };
    let Some(board) = board else {
        return Err(Error::Load(
            "There was a parse error while reading board file at '(pcb': the file produced no board"
                .to_string(),
        ));
    };
    let mut board = *board;
    let Some(transform) = coordinate_transform else {
        // Not reachable: `Structure.createBoard` builds the transform in the same statement that
        // builds the board (Plan 3 ruling A), so a board without one cannot come out of the
        // reader. Answered rather than unwrapped because the field is an `Option`.
        return Err(Error::Load(
            "the reader produced a board without a coordinate transform".to_string(),
        ));
    };

    // :734.
    apply_router_settings_for_loaded_board(&mut board, settings);
    // :735.
    apply_immediate_post_load_processing(&mut board);

    Ok(LoadedBoard {
        board,
        transform,
        metadata,
        settings: settings.clone(),
        warnings,
    })
}

/// Port of `HeadlessBoardManager.applyRouterSettingsForLoadedBoard`
/// (HeadlessBoardManager.java:739-749), all four steps, in Java's order.
///
/// Returns whether [`fr_router::pipeline::prepare_board`] changed the board — Java returns
/// `void`; the flag exists so `crates/fr-core/tests/overrides.rs` can assert the override fired
/// without re-deriving the whole board state.
///
/// Java's `:740` guard is `this.board != null && this.routingJob != null`; both are the caller's
/// here, since a `&mut Board` and a `&mut RouterSettings` cannot be null.
pub fn apply_router_settings_for_loaded_board(
    board: &mut Board,
    settings: &mut RouterSettings,
) -> bool {
    // :741-744. Quirk #119's teeth: a `--router.layers.*` whose token count disagrees with the
    // board sizes `layers` from the tokens and `setLayerCount` then discards the whole array.
    let board_layer_count = board.get_layer_count();
    if settings.get_layer_count() != board_layer_count {
        settings.set_layer_count(board_layer_count);
    }
    // :745 — `applyBoardSpecificOptimizations`, not the `IfNeeded` variant.
    settings.apply_board_specific_optimizations(board);
    // :746-747 — copper first, then hole, which is why `board_edge` always takes the lower class
    // index of the two. This is the **only** call: see the module docs on survey ruling AD.
    prepare_board(board, settings)
}

/// Port of `HeadlessBoardManager.applyImmediatePostLoadProcessing`
/// (HeadlessBoardManager.java:751-757).
///
/// Returns `board.reduceNetsOfRouteItems()`'s answer, which is Java's always-`false`
/// (`RoutingBoard.java:1285,1355` computes `result` and never assigns it — quirk #71's family;
/// the `fn` reproduces it).
///
/// **This is the first caller `fr_board::Board::reduce_nets_of_route_items` has ever had.** It was
/// ported in Plan 2 (`crates/fr-board/src/board/query.rs:1096`) and nothing in Plans 2-7 reached
/// it, because nothing in Plans 2-7 was a *loader*.
//
// not ported: `validatePowerPlanes()` (:756 / :914-1010) — 97 lines of `FRLogger.warn`, with the
// two helpers it calls. Rostered in `lib.rs` §7.
pub fn apply_immediate_post_load_processing(board: &mut Board) -> bool {
    // :752-754's `this.board == null` guard is the caller's.
    // :755.
    board.reduce_nets_of_route_items()
}

/// Port of `BoardLoader.loadBoardIfNeeded` (management/BoardLoader.java:19-56) — the
/// DSN-vs-KiCad-JSON dispatch and the **"only DSN and JSON formats are supported"** guard
/// (`:31-37`).
///
/// This guard is why `-de prev.ses -drc r.json` fails *in the loader* rather than at the argument
/// (quirk label S): the CLI accepts the path, `RoutingJob::set_input` sniffs it as
/// [`FileFormat::Ses`], and only here does the run stop. DRC mode is the caller; the route path
/// loads through the pipeline.
///
/// Java answers `boolean` and writes the board into `job.board`. The port's [`RoutingJob`] has no
/// `board` field — Rust ownership hands the board to the caller instead — so this answers the
/// [`LoadedBoard`] and the three `false` exits become [`Error::Load`] carrying Java's own message
/// text.
//
// not reachable: `BoardLoader.loadBoardIfNeeded:20-23`'s `if (job.board != null) return true`
// short-circuit. `RoutingJob` has no board field in the port (Task 1), so "already loaded" is not
// a state a job can be in; the caller holds the `LoadedBoard` and simply does not call again.
// not ported: `BoardLoader`'s three `FRLogger.error(…, null)` calls (:26, :32-35, :52) — the
// messages survive as [`Error::Load`]'s text, which is the observable half.
pub fn load_board_if_needed(job: &mut RoutingJob) -> Result<LoadedBoard, Error> {
    // :25-29.
    let Some(input) = job.get_input() else {
        return Err(Error::Load(
            "Cannot load board: job has no input".to_string(),
        ));
    };
    // :31-37 — the `//` comment on :31 reads "Only DSN and JSON/Native format are supported for
    // now"; the message is `:33`'s, with Java's `FileFormat.toString()` (the enum constant name).
    let format = input.format;
    if format != FileFormat::Dsn && format != FileFormat::KicadDesignJson {
        return Err(Error::Load(format!(
            "Cannot load board: only DSN and JSON formats are supported, got {}",
            format.java_name()
        )));
    }
    let data = input.get_data().to_vec();
    let mut settings = job.router_settings.clone();
    // :40-50 — the two arms, each building a fresh `HeadlessBoardManager(job)` around the same
    // job. The port has no manager, so the two arms are the two loaders.
    let loaded = if format == FileFormat::KicadDesignJson {
        let text = String::from_utf8_lossy(&data).into_owned();
        load_from_kicad_json(&text, job, &mut settings)
    } else {
        load_from_specctra_dsn(&data, job, &mut settings)
    };
    // :51-54 — Java's `catch (Exception e) { FRLogger.error("Failed to load board", e); return
    // false; }`. The port's loaders answer `Err` rather than throwing, so the arm is the `?`
    // below; the message is kept on the way out so the observable text is Java's.
    let loaded = loaded.map_err(|error| Error::Load(format!("Failed to load board: {error}")))?;
    // :44 / :49 — `job.board = boardManager.getRoutingBoard(); return job.board != null`. The
    // board is in the answer instead of in a field.
    job.router_settings = loaded.settings.clone();
    Ok(loaded)
}
