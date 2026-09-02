//! `HeadlessBoardManager`'s three board-mutating clearance overrides — the copper-to-edge
//! override, the drill-hole clearance override and the hole-keepout reclassification it calls.
//!
//! Java: `management/HeadlessBoardManager.java:466-552` (`applyCopperToEdgeClearanceOverride`),
//! `:346-396` (`applyHoleClearanceOverride`) and `:404-464`
//! (`assignHoleKeepoutClearanceClass`).
//!
//! # Why these live in `fr-board` and not in the loader
//!
//! Java runs all three from inside `HeadlessBoardManager`, at **one** reachable call site per DSN
//! load: `applyRouterSettingsForLoadedBoard:746-747`, on the fully loaded board. The port has no
//! manager object — `fr_dsn::read_board` answers a `Board` directly — so the three methods are
//! `Board` methods and the seam that sequences them is [`fr_router::pipeline::prepare_board`],
//! which `fr_core::apply_router_settings_for_loaded_board` calls at exactly that point.
//!
//! The paragraph above **replaced** a claim of *two* call sites (Plan 8 survey ruling AD:
//! `createBoard:342-343` plus `:746-747`). See "one call, and Java's is one too" below: Plan 8
//! Task 3 measured `HeadlessBoardManager.createBoard` to be unreachable from the DSN parser, so
//! the second call site never existed. Quirk #253.
//!
//! # Why this matters at *default* settings
//!
//! `DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM = 500.0` (`settings/sources/DefaultSettings.java:78`)
//! reads like a knob nobody turns, but the `:501-507` guard makes the default the value that
//! *fires*: it early-returns only when the configured value **is** the default **and** the
//! outline carries an explicit (non-fallback) DSN clearance class. On 15 of the 16 corpus
//! boards the DSN reader gives the outline the default AREA class, so a plain
//! `-de <dsn> -do <ses>` run appends a `board_edge` clearance class, writes 500 µm into its
//! whole row *and* column on every layer, and re-points the outline at it — which moves the
//! routed traces and therefore the SES bytes (quirk #231, and the 15 254 B vs 14 644 B
//! measurement in the Task 15b report).
//!
//! `DEFAULT_HOLE_CLEARANCE_UM = 0.0` (`DefaultSettings.java:81`) is inert on the corpus in
//! *effect* — `setHoleClearance(0)` over an existing 0 — but the method itself has no early
//! return once the setting is non-null and non-negative, so it is ported whole.
//!
//! # One call, and Java's is one too
//!
//! Plan 7 Task 15b ran the pair once, after the load, and argued that this reproduced Java's
//! *two*-call final state on the corpus with one measured exception. **Plan 8 Task 3 measured the
//! premise and it is wrong**: Java also runs the pair exactly once.
//!
//! `Structure.java:1268` calls `scopeParameter.boardHandling.createBoard(...)`, and
//! `ReadScopeParameter` has a single constructor that assigns its `final BoardParserCallback
//! boardHandling` field `new MinimalBoardManager()` (`ReadScopeParameter.java:103`).
//! `MinimalBoardManager.createBoard` (`:139-166`) constructs the `RoutingBoard` and returns; it
//! calls neither override. `HeadlessBoardManager.createBoard` (`:310-344`) has no caller outside
//! `GuiBoardManager.java:411`. Three independent measurements, all in
//! `crates/fr-core/tests/data/p8t3-clearance-overrides.txt`:
//!
//! * `[createboard] … headless_create_board_calls=0` on all three fixtures, through a counting
//!   subclass of the real `HeadlessBoardManager` driving a real `loadFromSpecctraDsn`;
//! * the `counterfactual_create_board` stage shows what `:342-343` *would* have changed on the
//!   itemless board (a `board_edge` class at index 3, the outline re-pointed) — and the real
//!   `after_parser` stage of the same load has none of it;
//! * on `Issue555-CNH_Functional_Tester_1.dsn`, Plan 7's own transcript puts `board_edge` at index
//!   **10**, after the seven `(class …)` clearance classes `Network.java:741` appends *later in
//!   the parse* than `Structure.java:1268`.
//!
//! So the "one place the two shapes differ" that quirk #232 recorded — a **non-default**
//! `router.hole_clearance_um > 0` on a board with no circular component keepouts, where Java's
//! second call was said to re-insert nothing while the port re-inserts every item — is empty:
//! there is no second call. Both sides run `changed == true` once and re-insert once, and
//! `crates/fr-core/tests/overrides.rs` replays the whole load at hole ∈ {0, 100, 500} µm on three
//! fixtures, matching the jar cell for cell **including the search tree's leaf count and its
//! `ShapeTree.toArray()` order digest**. Quirk #232 is rewritten and quirk #253 records the
//! dead-code finding.
//!
//! The Plan 8 obligation that used to sit on the next line — *"pin the quirk-#232 boundary with a
//! tree-op/order pin, or reproduce the second run"* (Plan 7 §10 row 15b, scan ruling R8) — is
//! **discharged**: the transcript's `after_second_hole_override` stage reproduces the second run
//! and `crates/fr-core/tests/overrides.rs::the_second_hole_override_leaves_the_search_tree_alone`
//! asserts it moves nothing, tree order digest included.

use fr_geometry::{Area, Shape, java_round};

use crate::Board;
use crate::board::item_ctx;
use crate::ids::ItemId;
use crate::items::Item;
use crate::rules::ItemClass;
use crate::structure::Unit;

/// `HeadlessBoardManager.BOARD_EDGE_CLEARANCE_CLASS_NAME` (HeadlessBoardManager.java:84).
pub const BOARD_EDGE_CLEARANCE_CLASS_NAME: &str = "board_edge";

/// `HeadlessBoardManager.HOLE_EDGE_CLEARANCE_CLASS_NAME` (HeadlessBoardManager.java:85).
pub const HOLE_EDGE_CLEARANCE_CLASS_NAME: &str = "hole_edge";

/// `DefaultSettings.DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM`
/// (`settings/sources/DefaultSettings.java:78`), duplicated here because
/// [`Board::apply_copper_to_edge_clearance_override`]'s `:501-507` guard compares against it and
/// `fr-board` cannot depend on `fr-settings` (the edge runs the other way).
///
/// `crates/fr-router/tests/clearance_override.rs` asserts this constant equals
/// `fr_settings::sources::DefaultSettings::DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM`, so the two
/// cannot drift apart silently.
pub const DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM: f64 = 500.0;

impl Board {
    /// The µm → board-unit conversion both overrides share
    /// (HeadlessBoardManager.java:365-372 and :508-516, character for character the same
    /// expression).
    ///
    /// Board-resolution dependent, so the *same* µm value is a different number of board units
    /// per board: 100 µm is 1 000 units on five corpus boards and 10 000 on
    /// `Issue143-rpi_splitter`.
    ///
    /// `Math.round` returns a `long` that Java then narrows to `int` with a C-style truncating
    /// cast; `as i32` truncates the same way.
    ///
    /// `pub` although Java has no such method — the expression appears inline in both overrides —
    /// so that `crates/fr-router/tests/clearance_override.rs` can pin the conversion against the
    /// probe's `copper_units` / `hole_units` columns rather than re-deriving it.
    pub fn clearance_override_board_units(&self, clearance_um: f64) -> i32 {
        let board_resolution = self.communication.resolution.max(1);
        java_round(Unit::scale(
            clearance_um * f64::from(board_resolution),
            Unit::Um,
            self.communication.unit,
        )) as i32
    }

    /// Port of `HeadlessBoardManager.applyCopperToEdgeClearanceOverride`
    /// (HeadlessBoardManager.java:466-552): append a `board_edge` clearance class holding
    /// `clearance_um`, write it into that class's whole row **and** column on every layer, and
    /// re-point the board outline at it.
    ///
    /// Returns whether the board changed — Java returns `void` and logs at `FRLogger.debug`
    /// (`:546-552`), which is invisible at the default log level.
    ///
    /// The `null` guards of `:467-472` (no board, no job, no router settings, no configured
    /// value) are the caller's: [`fr_router::pipeline::prepare_board`] only calls this when
    /// `RouterSettings::copper_to_edge_clearance_um` is `Some`. `:482-486`'s "board rules are
    /// unavailable" is not representable — a `Board` always has `rules` and a
    /// `ClearanceMatrix`.
    ///
    /// **The `:501-507` guard is the whole story of when this fires** (quirk #231): it returns
    /// early only when the value is the default *and* the outline does **not** use the fallback
    /// AREA class. Passing the default 500.0 on a board whose outline carries an explicit DSN
    /// clearance class is therefore the *only* way to leave the board alone; 500.000001 mutates
    /// it.
    pub fn apply_copper_to_edge_clearance_override(&mut self, clearance_um: f64) -> bool {
        // HeadlessBoardManager.java:474-480: negative warns and returns.
        if clearance_um < 0.0 {
            return false;
        }
        // :488-494: no outline, nothing to re-point.
        let Some(outline_id) = self.get_outline() else {
            return false;
        };
        // :496-500.
        let default_net_class = self.rules.get_default_net_class();
        let default_area_class_no = self
            .rules
            .net_classes
            .get(default_net_class)
            .default_item_clearance_classes
            .get(ItemClass::Area);
        let outline_class_no = self
            .items
            .get(&outline_id)
            .expect("Board::apply_copper_to_edge_clearance_override: get_outline just found it")
            .clearance_class();
        // :501-507 — the only early return that depends on the board.
        let uses_fallback_outline_class = outline_class_no == default_area_class_no;
        let uses_default_edge_clearance_value =
            (clearance_um - DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM).abs() < 1e-9;
        // Keep explicit DSN outline-clearance classes untouched when only the global default is
        // active (:505).
        if uses_default_edge_clearance_value && !uses_fallback_outline_class {
            return false;
        }

        // :509-516.
        let configured_clearance_board_units = self.clearance_override_board_units(clearance_um);

        // :518-528: reuse a `board_edge` class the DSN already declares, else append one. No
        // corpus board declares one, so this always appends and the new index is
        // board-dependent (3, 4 or 10 across the 16 corpus boards) — a port must append, not
        // assume 3.
        let matrix = &mut self.rules.clearance_matrix;
        let board_edge_class_no = match matrix.get_no(BOARD_EDGE_CLEARANCE_CLASS_NAME) {
            Some(class_no) => class_no,
            None => {
                matrix.append_class(BOARD_EDGE_CLEARANCE_CLASS_NAME);
                // :529-534: Java warns and returns when the append did not take. `appendClass`
                // only refuses a name that already exists, which `getNo` has just ruled out, so
                // the arm is unreachable — `expect` rather than a silent `false`.
                matrix
                    .get_no(BOARD_EDGE_CLEARANCE_CLASS_NAME)
                    .expect("ClearanceMatrix::append_class of an absent name always takes")
            }
        };

        // :536-541. Both the row and the column, on every layer, unconditionally — unlike the
        // hole path below, which floors against the existing value. The two are deliberately
        // not factored together. The loop starts at class 1, so column/row 0 (the `"null"`
        // class) keeps its zeros, and it ends at the *new* class count, so the
        // `[board_edge][board_edge]` diagonal is written too.
        for layer in 0..matrix.get_layer_count() {
            for class_no in 1..matrix.get_class_count() {
                matrix.set_value(
                    board_edge_class_no,
                    class_no,
                    layer,
                    configured_clearance_board_units,
                );
                matrix.set_value(
                    class_no,
                    board_edge_class_no,
                    layer,
                    configured_clearance_board_units,
                );
            }
        }

        // :543-549: remove from the trees, re-point, clear the derived data, insert again. Note
        // this is *not* `Item.changeClearanceClassIndex` — the remove/insert pair runs whether
        // or not clearance compensation is on, and the `clearDerivedData` sits between the
        // class write and the insert.
        let mut outline = self
            .items
            .remove(&outline_id)
            .expect("Board::apply_copper_to_edge_clearance_override: present, just read");
        self.trees.remove(&mut outline);
        outline.set_clearance_class(board_edge_class_no, &self.rules);
        outline.clear_derived_data();
        let ctx = item_ctx!(self);
        self.trees.insert(&mut outline, &ctx);
        self.items.insert(outline_id, outline);
        true
    }

    /// Port of `HeadlessBoardManager.applyHoleClearanceOverride`
    /// (HeadlessBoardManager.java:346-396): write `clearance_um` into `BoardRules.holeClearance`
    /// and, when it converts to a positive number of board units, reclassify the drill-hole
    /// keepouts through [`Board::assign_hole_keepout_clearance_class`].
    ///
    /// Returns Java's `changed || holeKeepouts > 0` (`:379`), i.e. whether anything changed —
    /// which is also the condition on `searchTreeManager.reinsertTreeItems()`.
    ///
    /// **There is no early return once the setting is non-null and non-negative**:
    /// `rules.setHoleClearance` at `:374` runs unconditionally, and only the re-insert and the
    /// `FRLogger.info` are gated. A port that returned early "because nothing changed" would
    /// diverge the moment the value is non-zero.
    ///
    /// The `:347-352` null guards and `:360-363`'s "board rules are unavailable" are the
    /// caller's / not representable, exactly as for the copper override.
    pub fn apply_hole_clearance_override(&mut self, clearance_um: f64) -> bool {
        // :354-359.
        if clearance_um < 0.0 {
            return false;
        }
        // :365-372.
        let configured_clearance_board_units = self.clearance_override_board_units(clearance_um);
        // :373-374: `changed` is read *before* the write, and the write is unconditional.
        let changed = configured_clearance_board_units != self.rules.get_hole_clearance();
        self.rules
            .set_hole_clearance(configured_clearance_board_units);
        // :375-378: the `> 0` gate lives here, not inside the reclassifier.
        let mut hole_keepouts = 0;
        if configured_clearance_board_units > 0 {
            hole_keepouts = self
                .assign_hole_keepout_clearance_class_board_units(configured_clearance_board_units);
        }
        // :379-385: tree shapes are precalculated at insert time, so every item loaded before
        // the override has to be re-inserted for its obstacle shape to include the drill-hole
        // inflation. See the module docs for how this differs from Java's two call sites.
        if changed || hole_keepouts > 0 {
            self.reinsert_tree_items();
        }
        // :387-395 is an `FRLogger.info` only.
        changed || hole_keepouts > 0
    }

    /// Port of `HeadlessBoardManager.assignHoleKeepoutClearanceClass`
    /// (HeadlessBoardManager.java:404-464), taking µm rather than board units so that all three
    /// overrides share one unit: assign every circular per-component keepout — KiCad's DSN
    /// spelling of a non-plated hole — to a dedicated `hole_edge` clearance class.
    ///
    /// Returns whether any keepout was reclassified (Java returns the count; `> 0` is the
    /// "changed anything" its caller tests at `:379`).
    ///
    /// Java's method takes the already-converted board units and has **no** `> 0` guard of its
    /// own — the guard is at the call site (`:376`). Calling this directly with a value that
    /// converts to `<= 0` therefore appends the class and reclassifies the keepouts while
    /// writing nothing but the pre-existing AREA clearances into the matrix, which is what Java
    /// would do if its call site were reached; that is deliberate, not an oversight.
    pub fn assign_hole_keepout_clearance_class(&mut self, clearance_um: f64) -> bool {
        let board_units = self.clearance_override_board_units(clearance_um);
        self.assign_hole_keepout_clearance_class_board_units(board_units) > 0
    }

    /// The Java-shaped body of [`Board::assign_hole_keepout_clearance_class`], in board units —
    /// so that [`Board::apply_hole_clearance_override`] does not convert twice.
    fn assign_hole_keepout_clearance_class_board_units(
        &mut self,
        hole_clearance_board_units: i32,
    ) -> usize {
        // :405-408: a board always has a clearance matrix, so `matrix == null` is not
        // representable.
        //
        // :409-423: `board.getItems()` in `itemList` order (descending id, quirk #63). The class
        // test is `item.getClass() != ObstacleArea.class` — the **exact** class, so
        // `ConductionArea`, `ViaObstacleArea` and `ComponentObstacleArea` are all excluded even
        // though Java has them as subclasses; the flattened `Item` enum matches only
        // `Item::ObstacleArea` and reproduces that.
        let hole_keepouts: Vec<ItemId> = {
            let ctx = item_ctx!(self);
            self.items
                .iter()
                .rev()
                .filter(|(_, item)| match item {
                    // :419-420: a package keepout belongs to a component, and a circular one is
                    // a drilled hole in the footprint.
                    Item::ObstacleArea(keepout) => {
                        keepout.hdr.get_component_id() > 0
                            && matches!(keepout.get_area(&ctx), Area::Shape(Shape::Circle(_)))
                    }
                    _ => false,
                })
                .map(|(id, _)| *id)
                .collect()
        };
        // :424-426.
        if hole_keepouts.is_empty() {
            return 0;
        }
        // :427-436, as for `board_edge` above.
        let matrix = &mut self.rules.clearance_matrix;
        let hole_edge_class_no = match matrix.get_no(HOLE_EDGE_CLEARANCE_CLASS_NAME) {
            Some(class_no) => class_no,
            None => {
                matrix.append_class(HOLE_EDGE_CLEARANCE_CLASS_NAME);
                matrix
                    .get_no(HOLE_EDGE_CLEARANCE_CLASS_NAME)
                    .expect("ClearanceMatrix::append_class of an absent name always takes")
            }
        };
        // :437-442.
        let default_net_class = self.rules.get_default_net_class();
        let default_area_class_no = self
            .rules
            .net_classes
            .get(default_net_class)
            .default_item_clearance_classes
            .get(ItemClass::Area);

        // :443-454. Two things are load-bearing and must not be "cleaned up":
        //
        //   * the value is a **floor**, `max(holeClearance, the AREA row's existing value)` —
        //     never reduce an existing requirement (:445-450). At 100 µm on the corpus the
        //     existing copper clearance always wins, so only the item reclassification is
        //     observable; at 500 µm the floor bites on the lower-valued columns.
        //   * the read and the writes **interleave**. The loop runs to the *new* class count,
        //     so on the last iteration `class_no == hole_edge_class_no` and the read
        //     `get_value(default_area_class_no, hole_edge_class_no, …)` sees the cell the
        //     `class_no == default_area_class_no` iteration already wrote through its symmetric
        //     `set_value(class_no, hole_edge_class_no, …)`. Hoisting the reads out of the loop
        //     would change the diagonal.
        let matrix = &mut self.rules.clearance_matrix;
        for layer in 0..matrix.get_layer_count() {
            for class_no in 1..matrix.get_class_count() {
                let value = hole_clearance_board_units.max(matrix.get_value(
                    default_area_class_no,
                    class_no,
                    layer,
                    false,
                ));
                matrix.set_value(hole_edge_class_no, class_no, layer, value);
                matrix.set_value(class_no, hole_edge_class_no, layer, value);
            }
        }

        // :455-462. Note there is no search-tree remove/insert here — the caller's
        // `reinsertTreeItems` covers it — and `clearDerivedData` runs only for a keepout whose
        // class actually changes.
        let mut reclassified = 0;
        for id in hole_keepouts {
            let rules = &self.rules;
            let keepout = self
                .items
                .get_mut(&id)
                .expect("Board::assign_hole_keepout_clearance_class: collected from self.items");
            if keepout.clearance_class() != hole_edge_class_no {
                keepout.set_clearance_class(hole_edge_class_no, rules);
                keepout.clear_derived_data();
                reclassified += 1;
            }
        }
        reclassified
    }
}

// -------------------------------------------------------------------------------------------------
// The rest of `management/HeadlessBoardManager.java`
// -------------------------------------------------------------------------------------------------
//
// The three methods above are the class's whole **board-mutating** surface, and all three are
// private. Its nine *public* methods are the load/save/diagnostic half, which Plan 8 Task 3 owns
// (survey §6, row 3): they build or replace the board, read it back, serialise it and check it,
// none of which `fr-board` can do — `fr-board` has no reader, no writer and no `RoutingJob`.
// **Task 3 has landed**, so the six deferral markers below now name where each one went.
// `scripts/audit-map/fr-board.map` points the class at this file so that the invocation
//
//   ./scripts/audit-port.sh management crates/fr-board/src 'HeadlessBoardManager.java' \
//       scripts/audit-map/fr-board.map
//
// gates them; before Plan 7 Task 15b no plan had ever audited `management/` at all, which is how
// the override gap survived six plans.
//
// The six markers that stood here read `added in Plan 8:`; **Plan 8 Task 3 landed all six**, and
// each is now the marker kind the tool means for what actually happened. They are repeated in
// `crates/fr-core/src/{load.rs,save.rs}` at the ported code itself, which is what
// `scripts/audit-port.sh management crates/fr-core/src` checks; these lines keep `fr-board`'s own
// `management/` audit honest about where the rest of the class went.
//
// not ported: `HeadlessBoardManager.createBoard` (:310-344) — **dead code at the pinned jar.** Its only declared caller is the `BoardParserCallback` contract `Structure.java:1268` invokes, and the sole production implementation is `ReadScopeParameter$MinimalBoardManager` (`ReadScopeParameter.java:103`, a `final` field with one constructor), which builds the `RoutingBoard` itself and calls neither override. Measured: `P8T3Probe`'s `[createboard]` rows report `headless_create_board_calls=0` for a real `loadFromSpecctraDsn` on all three fixtures. The port's equivalent construction is `fr_dsn`'s `Structure::create_board`, inside `fr_dsn::read_board`; the outline-clearance-class lookup in front of it is `crates/fr-dsn/src/parser/structure.rs`'s. Quirk #253.
// renamed: `HeadlessBoardManager.loadFromSpecctraDsn` (:673-705) -> `fr_core::load_from_specctra_dsn` (Plan 8 Task 3); `fr_dsn::read_board` is the parse half and that function is the manager wrapper around it.
// renamed: `HeadlessBoardManager.applyParsedBoardResult` (:711-737) -> `fr_core::apply_parsed_board_result` (Plan 8 Task 3): the `BoardReadResult` dispatch, then `fr_core::apply_router_settings_for_loaded_board` (whose board half is `fr_router::pipeline::prepare_board`, Task 15b) and `fr_core::apply_immediate_post_load_processing`.
// renamed: `HeadlessBoardManager.loadFromKiCadJson` (:794-823) -> `fr_core::load_from_kicad_json` (Plan 8 Task 3), over a KiCad JSON reader stub that Task 9 replaces.
// renamed: `HeadlessBoardManager.saveAsSpecctraSessionSes` (:862-877) -> `fr_core::save_as_specctra_session_ses` (Plan 8 Task 3), over `fr_dsn::ses_writer::write`.
// renamed: `HeadlessBoardManager.calculateCrc32` (:603-605) -> `fr_core::calculate_crc32_for_board` (Plan 8 Task 3): the public no-argument method is `calculateCrc32ForBoard(this.getRoutingBoard())` and the port has no `this.board`, so the two Java methods collapse into the one that takes the board.
//
// The three below were **consumed by Plan 8 Task 0**, and the reason is not the one the marker
// text predicted: `fr_core::Ctx` holds neither the board nor the job. There is no manager
// *object* in the port at all — `fr_core::RoutingPipeline::run(board, ctx)` takes the board as
// its `&mut Board` parameter, owned by whichever caller loaded it, and `RoutingJob` is passed
// around that call rather than held in a field. So none of the three is `added in Plan N:` with a
// field that was never built; each names the site that replaced it, with the marker kind the tool
// actually means (see the SF5 note below). `crates/fr-core/src/ctx.rs`'s doc comment records the
// same three.
//
// **Marker kind, corrected in Task 0's review round (SF5).** `scripts/audit-port.sh`'s own header
// defines `renamed: <Method>` as "ported under a different name" and counts it as **ported**. Only
// the third of these has a Rust item to name; the first two have a *calling convention* and no
// `fn` at all, so they are `not ported:` with the same prose. Their live non-GUI callers are
// already accounted for elsewhere in the port, which is what makes that honest:
//
//   $ grep -rn "getRoutingBoard()" src/main/java | grep -v '^src/main/java/app/freerouting/gui/'
//     io/specctra/parser/{PlaceControl:69, Wiring:345,435,658, Network:425,549,845,938,1237,1242,
//     1249,1282,1317,1318}  — all `scopeParameter.boardHandling.getRoutingBoard()`
//   $ grep -rn "replaceRoutingBoard(" src/main/java | grep -v '^src/main/java/app/freerouting/gui/'
//     management/jobs/RoutingJobSchedulerActionThread.java:284  (`boardManager.replaceRoutingBoard(job.board)`)
//
// Every `getRoutingBoard` caller outside the GUI is the DSN parser reaching the board it is
// building through `BoardParserCallback`, and `crates/fr-dsn/src/parser/scope_parameter.rs:82`
// already rosters that interface `// not ported:` — `ReadScopeParameter::board` is a plain public
// field, held directly rather than behind a callback. So the method is accounted for where it is
// actually reached, and there is nothing in `fr-board` to rename it to.
//
// not ported: `HeadlessBoardManager.getRoutingBoard` (:255-257) — a one-line field read on a manager object the port does not have. Its non-GUI callers are the DSN parser's `BoardParserCallback`, rostered at `crates/fr-dsn/src/parser/scope_parameter.rs:82`; `fr_core::RoutingPipeline::run` takes the board as its `&mut Board` parameter, owned by whichever caller loaded it.
// not ported: `HeadlessBoardManager.replaceRoutingBoard` (:276-278) — a `synchronized` field write with one non-GUI caller (`RoutingJobSchedulerActionThread.java:284`). Rust ownership does its job: the caller rebinds its own `Board`, so there is no field to replace and no `fn` to name.
// renamed: `HeadlessBoardManager.getCurrentRoutingJob` (:570-572) -> `fr_core::RoutingJob` (Plan 8 Task 1), passed to the call rather than held in a manager field. This one really is a rename — the job is a named Rust type — which is why `HeadlessBoardManager` stays off the `ROSTERED` list (see `docs/plan-7-handoff.md` §6's dated status line).
