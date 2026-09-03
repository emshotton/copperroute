//! Port of `io/kicad/KiCadJsonWriter.java` (227 lines): the KiCad board JSON writer.
//!
//! One public method, in two overloads — `write(RoutingBoard)` (`:27-31`) and
//! `write(RoutingBoard, String designName)` (`:32-226`) — which fills the [`dto`](super::dto)
//! tree and hands it to `GsonProvider.GSON.toJson` (`:225`).
//!
//! # It is not the inverse of [`read_board`](super::reader::read_board)
//!
//! The writer emits **seven** of the fourteen root keys with content — `layers`, `netClasses`,
//! `nets`, `outline`, `traces`, `vias`, `conductionAreas` — and leaves `hostCad`, `hostVersion`
//! `null` (so Gson omits them) and `clearanceRules` and `components` as empty lists. There is no
//! component section in `KiCadJsonWriter.java` at all: `grep -n 'components' <the file>` finds
//! nothing. So a `read → write` round trip **loses every component, pad and padstack**.
//!
//! A `write → read → write` round trip is a fixed point only on a board with **at most one
//! trace**, and that is measured rather than assumed: seven of the probe's nine stems are fixed
//! points and the two that are not are the two carrying more than one trace. The cause is item
//! ordering, not rounding — `Board::get_traces` walks in descending id (quirk #63), the writer
//! numbers `traceJson.id` 1..N in that order, and `read_board` re-inserts them in document order,
//! so the next walk reads them back **reversed**. `write_then_read_is_a_fixed_point` in
//! `crates/fr-dsn/tests/kicad_writer.rs` asserts both halves, including that a third write undoes
//! the reversal.
//!
//! # Gson is the serialiser, so key order is declaration order
//!
//! `GsonProvider.GSON` is `new GsonBuilder().setPrettyPrinting().disableHtmlEscaping()…` with no
//! `serializeNulls()`, which is exactly what [`to_gson_string_pretty`] reproduces (its module docs
//! carry the four JVM-verified points). The field order of each struct in
//! [`super::dto`] is Java's `getDeclaredFields()` order and therefore the wire order; a `null`
//! reference field is omitted; every `double` crosses as `Double.toString`.
//!
//! # Where this writer is reached from
//!
//! **Four call sites in the Java tree**, and all four pass a design name — which is why the
//! one-argument overload's `"KiCad_Design"` is unreachable there:
//!
//! * `management/jobs/RoutingJobSchedulerActionThread.java:277` — `setJobOutput`'s
//!   `-do out.json` arm, the only one this port reaches;
//! * `api/v1/JobOutputResource.java:293` and `:518` — **one** resource class, two call sites (the
//!   completed-job `GET …/output/json` and the in-progress snapshot);
//! * `gui/board/BoardExportActions.java:124` — the GUI's "export KiCad JSON", which spec §2 drops
//!   with the rest of the GUI. (It writes through `new java.io.FileWriter(outputFile)`, i.e. the
//!   platform default charset — the same one-argument-constructor slip quirk #290 records on the
//!   *read* side, here on the write side and on a path the port does not have.)
//!
//! The CLI arm was **quirk #289 (label T)**, and it is **fixed in Plan 9 Task 3**. In the jar
//! `setJobOutput` is registered as a board-updated listener at `:100` *and* called once more at
//! `:168`, and `setData`'s re-sniff turns the format into `KICAD_DESIGN_JSON` after the first
//! write, so only the **first** call ever reaches this function — with the board as it was
//! loaded. The port now calls this function **once, after the pipeline**, on the board the
//! pipeline finished with, and `BoardFileDetails::set_data` keeps the format it is given.
//! `crates/freerouting/src/commands/route.rs`'s step 13 carries the jar measurement and the fix;
//! `crates/freerouting/tests/cli_e2e.rs::do_out_json_writes_the_routed_board` is the test — it
//! lives there rather than beside this module's own tests because what it asserts is the
//! **binary**'s output file, cross-checked against the SES from the identical argv.
//
// not ported: KiCadJsonWriter's private constructor (KiCadJsonWriter.java:24), the
// `private KiCadJsonWriter() {}` that makes the class non-instantiable. A Rust module needs no
// such thing.

use fr_board::{Board, Item, Unit};
use fr_geometry::ShapeOps;

use super::dto::{
    ConductionAreaJson, KiCadBoardJson, LayerJson, NetClassJson, NetJson, OutlineJson, Point2D,
    TraceJson, UnitJson, ViaJson,
};
use crate::format::json::to_gson_string_pretty;

/// `KiCadJsonWriter.write(RoutingBoard)` (KiCadJsonWriter.java:27-31): the design name Java
/// defaults to when a caller supplies none.
///
/// Java's one-argument overload is `return write(board, "KiCad_Design");`. It has **no caller**
/// anywhere in the tree (`grep -rn "KiCadJsonWriter" src/` finds four call sites and all four
/// pass a name), and `:45`'s `designName != null ? designName : "KiCad_Design"` is the same
/// literal for a caller that passes `null` — which a `&str` parameter cannot. The constant is
/// kept, and named, so that the one-argument overload is `write(board, DEFAULT_DESIGN_NAME)`
/// rather than a second function nothing would call.
pub const DEFAULT_DESIGN_NAME: &str = "KiCad_Design";

/// `KiCadJsonWriter.write(RoutingBoard, String)` (KiCadJsonWriter.java:32-226): the board as
/// KiCad session JSON, UTF-8, no trailing newline.
///
/// `design_name` is Java's `designName` parameter. Java collapses a `null` to
/// [`DEFAULT_DESIGN_NAME`] at `:45`; a `&str` cannot be null, so the collapse is the caller's —
/// `write(board, DEFAULT_DESIGN_NAME)` is Java's one-argument overload.
///
/// # Panics
///
/// Never in practice, and the reason is worth stating because Java cannot fail here either.
/// `Gson.toJson` throws `IllegalArgumentException` for a non-finite `double`
/// (`GsonProvider` does not call `serializeSpecialFloatingPointValues`), and
/// [`to_gson_string_pretty`] returns `Err` at the same point. Every `double` this function writes
/// is an `int` divided by `scale_factor`, which is one of `10000.0`, `254.0` and `10.0` — none of
/// them zero — so no NaN and no infinity can arise. The `expect` is that argument, not a hope.
#[must_use]
pub fn write(board: &Board, design_name: &str) -> String {
    // `:33-42` — the unit-to-scale mapping. Java guards the whole block with
    // `board.communication != null`; `fr_board::Board` owns its `Communication` by value
    // (`board/mod.rs:249`), so the `null` arm at `:56-57` — which sets the same `10000.0`/`MM`
    // pair the `else` at `:40` and `:53` already set — is unreachable here and is not written out
    // a second time.
    let scale_factor = match board.communication.unit {
        // `:35-36`.
        Unit::Mil => 254.0,
        // `:37-38`.
        Unit::Um => 10.0,
        // `:39-41`, which `Unit.MM` and `Unit.INCH` both take.
        _ => 10_000.0,
    };

    // `:44-58`.
    let mut board_json = KiCadBoardJson {
        // `:45`.
        designName: Some(design_name.to_string()),
        // `:47`.
        resolution: scale_factor,
        // `:48-54`.
        unit: Some(match board.communication.unit {
            Unit::Mil => UnitJson::MIL,
            Unit::Um => UnitJson::UM,
            _ => UnitJson::MM,
        }),
        ..KiCadBoardJson::default()
    };
    let layers = board_json.layers.as_mut().expect("`new ArrayList<>()`");

    // ── 1. Layers (`:60-68`) ────────────────────────────────────────────────────────────────
    for i in 0..board.get_layer_count() {
        // `:62` — `board.layerStructure.layers[i]`.
        let layer = &board.layer_structure().layers[i];
        layers.push(LayerJson {
            // `:64`.
            index: i32::try_from(i).unwrap_or(i32::MAX),
            // `:65`.
            name: Some(layer.name.clone()),
            // `:66`.
            r#type: Some(if layer.is_signal { "signal" } else { "plane" }.to_string()),
        });
    }

    // ── 2. Nets (`:70-82`) ──────────────────────────────────────────────────────────────────
    for i in 1..=board.rules.nets.max_net_number() {
        // `:72-75` — `Nets.get(int)` answers `null` outside `1..=size`, which this range excludes,
        // so the `continue` is unreachable. It is written as a `let … else` all the same, because
        // that is Java's shape and the port must not depend on the range being tight.
        let Some(net) = board.rules.nets.get(i) else {
            continue;
        };
        board_json
            .nets
            .as_mut()
            .expect("`new ArrayList<>()`")
            .push(NetJson {
                // `:77`.
                id: net.net_number,
                // `:78`.
                name: Some(net.name.clone()),
                // `:79` — `net.getNetClass() != null ? …getName() : "default"`. A
                // `fr_board::Net` holds a `NetClassId`, not a nullable reference, so the
                // `"default"` arm is unreachable; `NetClasses::get` panics on an id the list does
                // not hold, which is the `ArrayIndexOutOfBoundsException` Java would raise for
                // the same corruption.
                className: Some(
                    board
                        .rules
                        .net_classes
                        .get(net.get_net_class())
                        .get_name()
                        .to_string(),
                ),
                // `:80`.
                containsPlane: net.contains_plane(),
            });
    }

    // ── 3. Net Classes (`:84-120`) ──────────────────────────────────────────────────────────
    for i in 0..board.rules.net_classes.count() {
        // `:86-89` — `NetClasses.get(int)` cannot answer `null` (it indexes an array and would
        // throw), so `:87-89`'s `continue` is dead in Java too.
        let net_class = board.rules.net_classes.get(fr_board::NetClassId(i));
        // `:92`.
        let clearance_class_index = net_class.get_trace_clearance_class();
        // `:93-95`.
        let clearance = f64::from(board.rules.clearance_matrix.get_value(
            clearance_class_index,
            clearance_class_index,
            0,
            false,
        )) / scale_factor;
        // `:96` — `(2 * netClass.getTraceHalfWidth(0))` is **int** arithmetic in Java and wraps on
        // overflow; the division to `double` happens afterwards.
        let trace_width =
            f64::from(net_class.get_trace_half_width(0).wrapping_mul(2)) / scale_factor;

        // `:98-99` — the fallback pair, then `:100-110`'s override and `:111`'s unconditional
        // recompute of the drill from whichever diameter survived.
        let mut via_diameter = 0.8;
        // `:100-110`.
        if let Some(via_rule) = net_class.get_via_rule()
            && via_rule.via_count() > 0
        {
            // `:102` — `ViaRule.getVia(0)` throws rather than answering `null`, so `:103`'s
            // `viaInfo != null` is dead. Its `getPadstack() != null` half is live: the port's
            // `PadstackId` may name a padstack the library does not hold.
            let via_info = via_rule.get_via(0);
            if let Some(via_pad) = board.library.padstacks.get(via_info.get_padstack()) {
                // `:105-108`.
                if let Some(shape) = via_pad.get_shape(0) {
                    via_diameter = f64::from(shape.bounding_box().width()) / scale_factor;
                }
            }
        }
        // `:111`.
        let via_drill = via_diameter * 0.5;

        // `:113-118` — every net whose class is **this** one, in net-number order. Java's test is
        // `net.getNetClass() == netClass`, i.e. object identity; the port compares the
        // [`fr_board::NetClassId`] index, which is the same relation because `NetClasses` stores
        // each class once.
        let mut net_names: Vec<String> = Vec::new();
        for n in 1..=board.rules.nets.max_net_number() {
            if let Some(net) = board.rules.nets.get(n)
                && net.get_net_class() == fr_board::NetClassId(i)
            {
                net_names.push(net.name.clone());
            }
        }

        // `:119`.
        board_json
            .netClasses
            .as_mut()
            .expect("`new ArrayList<>()`")
            .push(NetClassJson {
                // `:91`.
                name: Some(net_class.get_name().to_string()),
                clearance,
                traceWidth: trace_width,
                viaDiameter: via_diameter,
                viaDrill: via_drill,
                netNames: Some(net_names),
            });
    }

    // ── 4. Outline (`:122-140`) ─────────────────────────────────────────────────────────────
    //
    // `:124`'s `outline != null` replaces `boardJson.outline`, which the field initialiser
    // already set to `new OutlineJson()` (KiCadBoardJson.java:19). So a board with no outline
    // still writes `"outline": {"corners": [], "clearance": 0.0}` — not `null`, and not an
    // absent key.
    if let Some(Item::BoardOutline(outline)) = board.get_outline().and_then(|id| board.get_item(id))
    {
        // `:126`.
        let clearance_class_index = outline.hdr.clearance_class();
        let mut corners: Vec<Point2D> = Vec::new();
        // `:130-139`.
        for i in 0..outline.shape_count() {
            // `:131-132` — `getShape` answers `null` past the end, which this range excludes.
            let Some(poly_shape) = outline.get_shape(i) else {
                continue;
            };
            // `:133-137`.
            for pt in poly_shape.as_ops().bounded_corners() {
                corners.push(Point2D {
                    x: pt.to_float().x / scale_factor,
                    y: -pt.to_float().y / scale_factor,
                });
            }
        }
        // `:125-129`.
        board_json.outline = Some(OutlineJson {
            corners: Some(corners),
            clearance: f64::from(board.rules.clearance_matrix.get_value(
                clearance_class_index,
                clearance_class_index,
                0,
                false,
            )) / scale_factor,
        });
    }

    // ── 5. Traces (`:142-164`) ──────────────────────────────────────────────────────────────
    //
    // `:145`'s `trace instanceof PolylineTrace` is a **total** match in the port:
    // `fr_board::Item::Trace` holds a [`fr_board::PolylineTrace`] and there is no second trace
    // type, which is also true of the Java tree (`Trace` has exactly one concrete subclass).
    // `board.getTraces()` walks the item list in descending id (quirk #63), which
    // `Board::get_traces` reproduces, so `traceJson.id` — a **fresh** 1-based counter, not the
    // item id — numbers them in the same order on both sides.
    let mut trace_id = 1;
    for id in board.get_traces() {
        let Some(Item::Trace(poly_trace)) = board.get_item(id) else {
            continue;
        };
        // `:150-156`.
        let net_name = (poly_trace.hdr.net_count() > 0)
            .then(|| board.rules.nets.get(poly_trace.hdr.get_net_number(0)))
            .flatten()
            .map(|net| net.name.clone());
        // `:157-161`.
        let points = poly_trace
            .polyline()
            .corners()
            .iter()
            .map(|pt| Point2D {
                x: pt.to_float().x / scale_factor,
                y: -pt.to_float().y / scale_factor,
            })
            .collect();
        board_json
            .traces
            .as_mut()
            .expect("`new ArrayList<>()`")
            .push(TraceJson {
                // `:147`.
                id: trace_id,
                netName: net_name,
                // `:149` — `(2 * polyTrace.getHalfWidth())`, int arithmetic as at `:96`.
                width: f64::from(poly_trace.get_half_width().wrapping_mul(2)) / scale_factor,
                // `:148`.
                layerIndex: i32::try_from(poly_trace.get_layer()).unwrap_or(i32::MAX),
                points: Some(points),
            });
        trace_id += 1;
    }

    // ── 6. Vias (`:166-203`) ────────────────────────────────────────────────────────────────
    let layer_count = board.get_layer_count();
    let mut via_id = 1;
    // Hoisted out of the loop: `Board::ctx` builds a borrow-only view (`&self.rules`,
    // `&self.library`, `&self.components`) and nothing in this function is `&mut`, so every
    // iteration would build the identical value. Checked rather than assumed: `write` takes
    // `&Board` and the whole body is reads plus pushes into the local DTO.
    let ctx = board.ctx();
    for id in board.get_vias() {
        let Some(Item::Via(via)) = board.get_item(id) else {
            continue;
        };
        // `:171-177`.
        let net_name = (via.hdr.net_count() > 0)
            .then(|| board.rules.nets.get(via.hdr.get_net_number(0)))
            .flatten()
            .map(|net| net.name.clone());
        // `:178-181`.
        let center = via.get_center().to_float();
        let position = Point2D {
            x: center.x / scale_factor,
            y: -center.y / scale_factor,
        };

        // `:183` — `via.getPadstack()`, which Java dereferences unguarded at `:185`: a via whose
        // padstack the library does not hold is a `NullPointerException` there. The port cannot
        // panic on a missing padstack without inventing a panic Java's own callers never see on
        // a board that was read by `readBoard` (every via it inserts registers its padstack
        // first), so the two `while` walks below simply see `None` on every layer and answer
        // `firstLayer == layerCount`, `lastLayer == -1` — which is what Java's walks answer for
        // an all-`null` padstack too.
        let padstack = via.get_padstack(&ctx);
        // `:184-187`.
        let mut first_layer = 0usize;
        while first_layer < layer_count
            && padstack
                .and_then(|p| p.get_shape(i32::try_from(first_layer).unwrap_or(i32::MAX)))
                .is_none()
        {
            first_layer += 1;
        }
        // `:188-191`.
        let mut last_layer = i64::try_from(layer_count).unwrap_or(i64::MAX) - 1;
        while last_layer >= 0
            && padstack
                .and_then(|p| p.get_shape(i32::try_from(last_layer).unwrap_or(i32::MAX)))
                .is_none()
        {
            last_layer -= 1;
        }

        // `:195-200`.
        let diameter = padstack
            .and_then(|p| p.get_shape(i32::try_from(first_layer).unwrap_or(i32::MAX)))
            .map_or(0.8, |shape| {
                f64::from(shape.bounding_box().width()) / scale_factor
            });

        board_json
            .vias
            .as_mut()
            .expect("`new ArrayList<>()`")
            .push(ViaJson {
                // `:170`.
                id: via_id,
                netName: net_name,
                position: Some(position),
                diameter,
                // `:201`.
                drill: diameter * 0.5,
                // `:192`.
                startLayerIndex: i32::try_from(first_layer).unwrap_or(i32::MAX),
                // `:193`.
                endLayerIndex: i32::try_from(last_layer).unwrap_or(i32::MAX),
            });
        via_id += 1;
    }

    // ── 7. Conduction Areas (`:205-223`) ────────────────────────────────────────────────────
    let mut area_id = 1;
    for id in board.get_conduction_areas() {
        let Some(Item::ConductionArea(area)) = board.get_item(id) else {
            continue;
        };
        // `:210-216`.
        let net_name = (area.hdr.net_count() > 0)
            .then(|| board.rules.nets.get(area.hdr.get_net_number(0)))
            .flatten()
            .map(|net| net.name.clone());
        // `:219-221` — `area.getArea()` is the **absolute** area (ObstacleArea.java:119-144),
        // not the relative one, so the corners already carry the component translation.
        let polygon = area
            .get_area(&ctx)
            .corner_approx_arr()
            .into_iter()
            .map(|pt| Point2D {
                x: pt.x / scale_factor,
                y: -pt.y / scale_factor,
            })
            .collect();
        board_json
            .conductionAreas
            .as_mut()
            .expect("`new ArrayList<>()`")
            .push(ConductionAreaJson {
                // `:209`.
                id: area_id,
                netName: net_name,
                // `:217`.
                layerIndex: i32::try_from(area.get_layer()).unwrap_or(i32::MAX),
                // `:218`.
                isObstacle: area.get_is_obstacle(),
                polygon: Some(polygon),
            });
        area_id += 1;
    }

    // `:225` — `GsonProvider.GSON.toJson(boardJson)`.
    to_gson_string_pretty(&board_json).expect("every double the writer builds is finite")
}
