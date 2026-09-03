//! Port of `io/kicad/KiCadJsonReader.java`'s `readBoard` — the KiCad board-JSON reader.
//!
//! **Plan 8 Task 8 landed the signature and sections 1-8 (`KiCadJsonReader.java:63-497`); Task 9
//! extended the same function body with sections 9-11 (`:498-755`)**, which build the library
//! templates, the components and their pins, the conduction areas, the traces and the vias, and
//! with the two private helpers only they call. The reader is complete, and
//! `fr_core::load::kicad_read_board` calls it — Task 3's stub obligation is discharged.
//!
//! not ported: the private `KiCadJsonReader()` constructor (KiCadJsonReader.java:55) — a
//! utility-class no-op; this is a Rust module.
//! **Plan 8 Task 10 added [`import_session`]** — `KiCadJsonReader.importSession` (`:757-855`),
//! the *session* half of the same class, which imports traces, vias and conduction areas onto a
//! board that already exists. It lives here rather than beside [`crate::kicad::writer`] because
//! it is a reader and it shares this module's helpers (`java_nets_get`, `via_shape`,
//! `java_drill_item_tile_shape_count`, `java_format_fixed`).

use std::cmp::Ordering;

use fr_board::{
    Board, BoardLibrary, BoardRules, ClearanceMatrix, Communication, Components, FixedState,
    ItemClass, ItemIdGenerator, Layer, LayerStructure, NetClassId, Nets, Package, PackagePin,
    Packages, Padstack, PadstackId, Padstacks, Unit, ViaInfo, ViaRule, equals_ignore_case,
    java_to_lower, java_to_upper,
};
use fr_geometry::{
    Area, Circle, FloatPoint, IntBox, IntOctagon, IntPoint, IntVector, Point, PolygonShape,
    PolylineShapeRef, Shape, TileShape, Vector,
};

use crate::coordinate_transform::CoordinateTransform;
use crate::error::{BoardMetadata, BoardReadResult, DsnError};
use crate::format::double::java_format_fixed;
use crate::format::java_round_to_int;
use crate::kicad::dto::{KiCadBoardJson, NetClassJson, PadJson, Point2D, UnitJson};
use crate::parser::network::is_kicad_default_net_class_name;

/// Port of `KiCadJsonReader.readBoard` (KiCadJsonReader.java:61-755) — **complete**.
///
/// Task 8 landed the signature and sections 1-8 (`:63-497`); **Task 9 extended the same function
/// body** with sections 9-11 (`:498-755`) and the two private helpers only they call, plus the
/// metadata/warnings tail. The board this returns now carries everything the jar's does: layers,
/// clearance matrix, outline, bounding box, communication, net classes, nets, via infos, via rules
/// and via padstacks from sections 1-8, and the library packages, padstacks, components, pins,
/// conduction areas, traces and vias from sections 9-11.
///
/// It answers `fr_dsn`'s own [`BoardReadResult`], so `fr-core`'s load path is format-agnostic —
/// `fr_core::load::kicad_read_board` calls straight through to it, which is what makes
/// `-de <board>.json -do out.ses` a byte-identical round trip against the jar
/// (`tests/reference/cli-kicad-ecc83-json/`).
///
/// # Evidence
///
/// `crates/fr-dsn/tests/data/p8t8-kicad-read-a.txt` (996 `[s8]` rows over 24 inputs) and
/// `…-read-b.txt` (2 775 `[s9]` rows over 67) are the pinned-jar transcripts, replayed row by row
/// by `crates/fr-dsn/tests/kicad_reader.rs`.
///
/// # Signature
///
/// Java takes `(Reader, BoardObservers, IdGenerator)`.
///
/// * `reader` becomes `json: &str` for the reason [`crate::dsn_reader::read_board`] takes its
///   input decoded: the caller owns the decoding, and Java's `null`-reader guard (`:64-66`) is
///   then unrepresentable.
/// * `observers` is dropped — `global-constraints.md` forbids board observers, and Java's own
///   `observers == null` default (`:69-71`) is a `BoardObserverAdaptor`, i.e. no observers.
/// * `id_generator` is Java's nullable `IdGenerator`; `None` is Java's `new ItemIdGenerator()`
///   (`:72-74`). **Taken by value, and [`ItemIdGenerator`] is `Copy`** — the same shape
///   [`crate::dsn_reader::read_board`] uses, so the two readers present one signature to
///   `fr-core`.
///
/// # `catch (Throwable)`
///
/// Java wraps the whole body in `try { … } catch (Throwable e) { return ParseError("json_payload",
/// "Exception occurred: " + e.getMessage()); }` (`:76`, `:746-750`). The port has no blanket
/// catch: it returns that `ParseError` at **every** point Java can actually throw from, each
/// named at its own site. Sections 9-11's share is **24 early returns** covering **19 distinct
/// Java throw sites** — three of the nineteen are reached from more than one place (`Nets.get`'s
/// `currentNet.name` from `:639`, `:649`, `:668` and `:685`; `DrillItem.tileShapeCount` from
/// `:642` and `:716`; `:706`'s `shapes[li]` store from both ends of its index range) — plus one
/// port-only `Result` that Java has no counterpart for (`insert_via_checked`'s, the `totalized:`
/// note at the via site). Only for a payload the *parser* rejects is the
/// `detail` text not Java's (quirks #277 and #279). The second recovery boundary, `:603`'s
/// `catch (Exception)`, is a package-dedup fallback rather than a method-level one; see the
/// section-9 header comment in the body.
// renamed: KiCadJsonReader.readBoard -> read_board, and its `Reader` parameter -> `json: &str`.
#[allow(clippy::too_many_lines)] // Java's own 693-line method, kept in one piece.
#[must_use]
pub fn read_board(json: &str, id_generator: Option<ItemIdGenerator>) -> BoardReadResult {
    // :68 `long startTime` and :83-86 the parse-duration `FRLogger.debug` — not ported (this
    // crate has no logger, and a clock would make the transcript irreproducible).
    // :69-71 `observers` — not ported, see the signature note.
    // :72-74.
    let id_generator = id_generator.unwrap_or_default();

    // ---------------------------------------------------------------- 1. Deserialize JSON :77
    // `:78` `GsonProvider.GSON.fromJson(reader, KiCadBoardJson.class)`, `:79-81` the null guard.
    // Gson answers `null` for a payload that is empty or is the literal `null`; serde_json
    // rejects both, so the emptiness test is explicit here.
    let board_json: KiCadBoardJson = if json.chars().all(java_is_whitespace) {
        return parse_error("json_root", "JSON payload is empty or invalid");
    } else {
        match serde_json::from_str::<Option<KiCadBoardJson>>(json) {
            Ok(Some(board_json)) => board_json,
            // `:79-81`.
            Ok(None) => return parse_error("json_root", "JSON payload is empty or invalid"),
            // `:746-749` over a `JsonSyntaxException`. Quirk #277: the `detail` text is
            // serde_json's, not Gson's.
            Err(error) => {
                return parse_error("json_payload", &format!("Exception occurred: {error}"));
            }
        }
    };

    // ------------------------------------------------------- 2. Set up units and scaling :88
    // `:89-94`. Every arm that is neither `MIL` nor `UM` — including Gson's `null` for an absent
    // key, an explicit `null` and an unrecognised constant name — lands on `MM`.
    let user_unit = match board_json.unit {
        // `:90-91`.
        Some(UnitJson::MIL) => Unit::Mil,
        // `:92-93` — the `UnitJson.UM` arm the task brief names.
        Some(UnitJson::UM) => Unit::Um,
        // `:89` — the documented fall-through: `Some(MM)` and `None` both reach it.
        Some(UnitJson::MM) | None => Unit::Mm,
    };

    // `:96-100`. "We maintain similar resolution scaling factors as DSN mapping (e.g. 1000 for
    // mm)". `(int)` on a `double` is Java's narrowing conversion — NaN to 0, out-of-range
    // saturating — which Rust's `as` reproduces exactly.
    let mut resolution = java_max(1.0, board_json.resolution) as i32;
    if board_json.resolution == 1.0 && user_unit == Unit::Mm {
        resolution = 10000; // 0.1 micrometer resolution is default for mm if unspecified
    }

    // `:102`.
    let scale_factor = f64::from(resolution);

    // ------------------------------------------------------------------ 3. Layer Structure :104
    // `:105` dereferences `boardJson.layers` with no null check.
    let Some(json_layers) = board_json.layers.as_ref() else {
        return npe("java.util.List.isEmpty()", "boardJson.layers");
    };
    let layer_count = if json_layers.is_empty() {
        2
    } else {
        json_layers.len()
    };
    let mut board_layers: Vec<Layer> = Vec::with_capacity(layer_count);
    // `Layer.name` is a nullable Java `String` and `fr_board::Layer::name` is a `String`, so the
    // line below totalizes a JSON `"name": null` to `""` (quirk #282). **Section 9 turns on
    // exactly that lost bit**: `:545` dereferences `boardLayers[li].name` unconditionally, so a
    // board with a null layer name and a pad that names any layer is a `ParseError` in Java and
    // would be a silent `""` comparison here. This vector carries the bit forward; nothing else
    // reads it.
    let mut board_layer_names: Vec<Option<String>> = Vec::with_capacity(layer_count);
    if json_layers.is_empty() {
        // `:108-109`.
        board_layers.push(Layer::new("F.Cu", true));
        board_layers.push(Layer::new("B.Cu", true));
        board_layer_names.push(Some("F.Cu".to_string()));
        board_layer_names.push(Some("B.Cu".to_string()));
    } else {
        for layer_json in json_layers {
            // `:113` — `!"plane".equalsIgnoreCase(type)`, so a `null` type is a *signal* layer.
            let is_signal = !layer_json
                .r#type
                .as_deref()
                .is_some_and(|kind| equals_ignore_case("plane", kind));
            // totalized: `Layer.name` is a Java `String` and takes `layerJson.name` verbatim,
            // `null` included (`:114`); `fr_board::Layer::new` needs a `String`, so a JSON
            // `"name": null` becomes `""` here. Java only crashes on it later, in **section 9**
            // (`:545`, `boardLayers[li].name.equalsIgnoreCase(layerName)`), and only for a board
            // that also has pads; a KiCad export always writes the key. Quirk #282.
            board_layers.push(Layer::new(
                layer_json.name.clone().unwrap_or_default(),
                is_signal,
            ));
            board_layer_names.push(layer_json.name.clone());
        }
    }
    let layer_structure = LayerStructure::new(board_layers);
    // not ported: `:118-120`'s `specctraLayerStructure` — a dead local. Java constructs an
    // `io.specctra.parser.LayerStructure` ("dummy or map layer names if needed", its own comment
    // says) and never reads it again; `grep -n specctraLayerStructure KiCadJsonReader.java` finds
    // only the declaration.

    // ------------------------------------------------------------------ 4. Clearance Matrix :122
    // `:123-124` dereferences `boardJson.netClasses` with no null check, inside
    // `nonDefaultNetClasses`' own enhanced-for.
    let Some(json_net_classes) = board_json.netClasses.as_ref() else {
        return npe("java.util.List.iterator()", "netClasses");
    };
    let additional_net_classes = non_default_net_classes(json_net_classes);

    // `:126-132`. The two reserved rows are `"null"` (the four-character string, not a null
    // reference) and `"default"`.
    let clearance_class_count = 2.max(additional_net_classes.len() + 2);
    let mut clearance_class_names: Vec<String> = Vec::with_capacity(clearance_class_count);
    clearance_class_names.push("null".to_string());
    clearance_class_names.push("default".to_string());
    for net_class in &additional_net_classes {
        // totalized: as `Layer::new` above — **quirk #282**. Java stores the `null` name in the
        // matrix row and NPEs in `ClearanceMatrix.getNo` the next time a custom clearance rule is
        // looked up, because `row[i].name.equalsIgnoreCase(...)` dereferences the *row's* name.
        clearance_class_names.push(net_class.name.clone().unwrap_or_default());
    }

    // `:134-135`.
    let mut clearance_matrix = ClearanceMatrix::new(
        clearance_class_count,
        &layer_structure,
        &clearance_class_names,
    );
    // `:136-138` — fallback 0.2mm.
    let default_clearance = java_round_to_int(Unit::scale(0.2, Unit::Mm, user_unit) * scale_factor);
    clearance_matrix.set_default_value(default_clearance);

    // `:140-145`.
    let kicad_default_net_class = find_kicad_default_net_class(json_net_classes);
    if let Some(kicad_default) = kicad_default_net_class
        && kicad_default.clearance > 0.0
    {
        let default_cl_val = java_round_to_int(kicad_default.clearance * scale_factor);
        // `:144` — the three-argument `setValue`, i.e. every layer.
        clearance_matrix.set_value_on_all_layers(1, 1, default_cl_val);
    }

    // Populate clearance matrix from non-default NetClasses and Custom Clearance Rules
    // (`:147-154`).
    //
    // **Quirk #83's J-then-I `ClearanceMatrix.setValue`/`getValue` indexing must not be
    // "corrected" here** (plan-3 hand-off §Plan 8). `:153` writes only `setValue(1, clNo, …)` and
    // `:161` only `setValue(idxA, idxB, …)`, so the matrix a KiCad board arrives with is
    // genuinely asymmetric: `getValue(1, 2, …)` and `getValue(2, 1, …)` differ. Writing both
    // orders would change the clearance of every KiCad-sourced board.
    for (i, net_class) in additional_net_classes.iter().enumerate() {
        let cl_no = i + 2;
        let cl_val = java_round_to_int(net_class.clearance * scale_factor);
        clearance_matrix.set_value_on_all_layers(cl_no, cl_no, cl_val);
        clearance_matrix.set_value_on_all_layers(1, cl_no, cl_val); // spacing between default and class
    }

    // `:156` dereferences `boardJson.clearanceRules` with no null check.
    let Some(json_clearance_rules) = board_json.clearanceRules.as_ref() else {
        return npe("java.util.List.iterator()", "boardJson.clearanceRules");
    };
    for rule in json_clearance_rules {
        // `:157-158`. `ClearanceMatrix.getNo` answers `-1` for an unknown name and — because
        // `String.equalsIgnoreCase(null)` is `false`, not a throw — for a `null` one too.
        let idx_a = rule
            .classA
            .as_deref()
            .and_then(|name| clearance_matrix.get_no(name));
        let idx_b = rule
            .classB
            .as_deref()
            .and_then(|name| clearance_matrix.get_no(name));
        // `:159` — `idxA >= 0 && idxB >= 0`.
        if let (Some(idx_a), Some(idx_b)) = (idx_a, idx_b) {
            let clearance_val = java_round_to_int(rule.clearance * scale_factor);
            clearance_matrix.set_value_on_all_layers(idx_a, idx_b, clearance_val);
        }
    }

    // ------------------------------------- 5. Board Outline / Boundary Shape Creation :165
    let mut outline_shapes: Vec<PolylineShapeRef> = Vec::new();
    let bounding_box: IntBox;
    // `:168-169`. The `||` short-circuits, so a `null` outline is *not* a crash but a `null`
    // `outline.corners` is.
    let outline_missing = match board_json.outline.as_ref() {
        None => true,
        Some(outline) => match outline.corners.as_ref() {
            None => return npe("java.util.List.size()", "boardJson.outline.corners"),
            Some(corners) => corners.len() < 3,
        },
    };
    if outline_missing {
        // `:171-172`.
        let mut outline = PointOutline::new();
        // not ported: `:173-175`'s `FRLogger.warn`. Its text ("… will be generated for
        // routing") differs from the `warnings` entry `:740-742` adds ("… was generated"); only
        // the latter leaves this function, and it is emitted at the tail below.
        // `:176-179`.
        let mut min_x = f64::MAX;
        let mut max_x = -f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_y = -f64::MAX;

        // Check components and pads (`:181-201`).
        if let Some(components) = board_json.components.as_ref() {
            for component in components {
                if let Some(position) = component.position.as_ref() {
                    min_x = java_min(min_x, position.x);
                    max_x = java_max(max_x, position.x);
                    min_y = java_min(min_y, position.y);
                    max_y = java_max(max_y, position.y);
                }
                if let Some(pads) = component.pads.as_ref() {
                    for pad in pads {
                        if let Some(position) = pad.position.as_ref() {
                            min_x = java_min(min_x, position.x);
                            max_x = java_max(max_x, position.x);
                            min_y = java_min(min_y, position.y);
                            max_y = java_max(max_y, position.y);
                        }
                    }
                }
            }
        }

        // Check vias (`:203-213`).
        if let Some(vias) = board_json.vias.as_ref() {
            for via in vias {
                if let Some(position) = via.position.as_ref() {
                    min_x = java_min(min_x, position.x);
                    max_x = java_max(max_x, position.x);
                    min_y = java_min(min_y, position.y);
                    max_y = java_max(max_y, position.y);
                }
            }
        }

        // Check traces (`:215-227`).
        if let Some(traces) = board_json.traces.as_ref() {
            for trace in traces {
                if let Some(points) = trace.points.as_ref() {
                    for point in points {
                        min_x = java_min(min_x, point.x);
                        max_x = java_max(max_x, point.x);
                        min_y = java_min(min_y, point.y);
                        max_y = java_max(max_y, point.y);
                    }
                }
            }
        }

        // Check conduction areas (`:229-241`).
        if let Some(zones) = board_json.conductionAreas.as_ref() {
            for zone in zones {
                if let Some(polygon) = zone.polygon.as_ref() {
                    for point in polygon {
                        min_x = java_min(min_x, point.x);
                        max_x = java_max(max_x, point.x);
                        min_y = java_min(min_y, point.y);
                        max_y = java_max(max_y, point.y);
                    }
                }
            }
        }

        // `:243`.
        let padding = Unit::scale(5.0, Unit::Mm, user_unit);

        if min_x == f64::MAX {
            // No items found on the board at all, fallback to a default 1000x1000 region in
            // internal units (`:245-252`).
            min_x = 0.0;
            max_x = 1000.0 / scale_factor;
            min_y = -1000.0 / scale_factor;
            max_y = 0.0;
        }

        // Apply padding (`:254-258`).
        min_x -= padding;
        max_x += padding;
        min_y -= padding;
        max_y += padding;

        // `:260-272` — the Y negation the task brief names, through `java_round` (Convention 5),
        // never `f64::round`.
        let points = [
            Point::Int(IntPoint::new(
                java_round_to_int(min_x * scale_factor),
                java_round_to_int(-min_y * scale_factor),
            )),
            Point::Int(IntPoint::new(
                java_round_to_int(min_x * scale_factor),
                java_round_to_int(-max_y * scale_factor),
            )),
            Point::Int(IntPoint::new(
                java_round_to_int(max_x * scale_factor),
                java_round_to_int(-max_y * scale_factor),
            )),
            Point::Int(IntPoint::new(
                java_round_to_int(max_x * scale_factor),
                java_round_to_int(-min_y * scale_factor),
            )),
        ];

        // `:274-278`.
        for point in &points {
            outline.add_point(point.to_float());
        }
        outline_shapes.push(PolylineShapeRef::Polygon(PolygonShape::from_points(
            &points,
        )));
        bounding_box = outline.bounding_box().offset(1000.0);
    } else {
        // `:280-282`.
        let mut outline = PointOutline::new();
        let mut corners: Vec<Point2D> = board_json
            .outline
            .as_ref()
            .expect("outline_missing is true when it is None")
            .corners
            .as_ref()
            .expect("outline_missing returned early when it is None")
            .clone();
        if corners.len() > 2 {
            // Sort corners by polar angle around centroid to ensure simple polygon construction
            // (`:283-297`).
            let mut sum_x = 0.0;
            let mut sum_y = 0.0;
            for point in &corners {
                sum_x += point.x;
                sum_y += point.y;
            }
            let count = corners.len() as f64;
            let cx = sum_x / count;
            let cy = sum_y / count;
            // Java's `List.sort` is a **stable** TimSort and so is Rust's `sort_by`;
            // `Double.compare` is [`java_double_compare`], not `f64::partial_cmp`.
            //
            // **The one part of this line that is not exact.** `Math.atan2` is not required to be
            // correctly rounded: `java.lang.Math`'s contract allows 2 ulp and lets the JIT
            // substitute an intrinsic, and Rust's `f64::atan2` is the platform libm with no
            // accuracy guarantee at all. Two corners at almost exactly the same polar angle around
            // the centroid could therefore compare differently on the two sides and swap places —
            // which changes the *polygon*, not just the corner list, because `PolygonShape`'s
            // constructor then drops different collinear corners. Measured clean on all 24 inputs
            // of `crates/fr-dsn/tests/data/p8t8-kicad-read-a.txt`, including `interf-u`'s 9-corner
            // outline and the deliberately shuffled `outline-unsorted` stem: every corner of every
            // outline matches the JVM. A residual risk, not an observed one, and a property of two
            // libm implementations rather than of Java — which is why it is recorded here and not
            // in `docs/java-quirks.md`. The input that would show it is two corners whose `atan2`
            // differs only in the last ulp.
            corners.sort_by(|p1, p2| {
                java_double_compare((p1.y - cy).atan2(p1.x - cx), (p2.y - cy).atan2(p2.x - cx))
            });
        }
        // `:298-305` — the same Y negation and `Math.round`.
        let mut points: Vec<Point> = Vec::with_capacity(corners.len());
        for corner in &corners {
            let point = Point::Int(IntPoint::new(
                java_round_to_int(corner.x * scale_factor),
                java_round_to_int(-corner.y * scale_factor),
            ));
            outline.add_point(point.to_float());
            points.push(point);
        }
        // `:306-307`.
        outline_shapes.push(PolylineShapeRef::Polygon(PolygonShape::from_points(
            &points,
        )));
        bounding_box = outline.bounding_box().offset(1000.0);
    }

    // `:310` — Default clearance class. `OutlineJson.clearance` ("outline/edge clearance class
    // mapping", KiCadBoardJson.java:90) is never consulted.
    let outline_clearance_no = 1;

    // ------------------------------------------------------- 6. Communication object setup :312
    // `:313` — the KiCad reader builds its own transform; `LoadedBoard` (Plan 8 Task 3) is what
    // carries it to the writer, so it rides out on `BoardReadResult::Success`.
    // `scale_factor` is `f64::from(resolution)` and `resolution` is `java_max(1.0, …) as i32`, so
    // it is finite and at least `1.0` and the constructor's #89 guard cannot fire. Answered, not
    // unwrapped, because this function already has a failure channel.
    let coordinate_transform = match CoordinateTransform::new(scale_factor, 0.0, 0.0) {
        Ok(coordinate_transform) => coordinate_transform,
        Err(error) => return parse_error("resolution", &error.to_string()),
    };
    // `:314-319` — the two host fallbacks are the literal strings `"KiCad"` and `"v10.0"`. They
    // are **not** `PARITY_VERSION` (plan-8 ruling 5): they reach
    // `Communication.SpecctraParserInfo` and therefore the SES the port later writes.
    let host_cad = board_json
        .hostCad
        .as_deref()
        .filter(|host| !java_is_blank(host))
        .unwrap_or("KiCad")
        .to_string();
    let host_version = board_json
        .hostVersion
        .as_deref()
        .filter(|version| !java_is_blank(version))
        .unwrap_or("v10.0")
        .to_string();
    // `:320-324`. Java's `new SpecctraParserInfo("\"", hostCad, hostVersion, null, null, false)`
    // is exactly what `Communication::new` leaves behind: `fr-board` flattens the nested record
    // onto the struct and seeds `string_quote = "\""`, no constants, no write resolution and
    // `dsn_file_generated_by_host = false`.
    let communication = Communication::new(
        user_unit,
        resolution,
        id_generator,
        Some(host_cad),
        Some(host_version),
    );

    // --------------------------------------------------------------- 7. Construct RoutingBoard
    // `:327`.
    let board_rules = BoardRules::new(layer_structure.clone(), clearance_matrix);
    // `:337-338`. Java replaces `board.library.padstacks`/`packages` *after* construction; the
    // port hands them in, which is equivalent because `BasicBoard`'s constructor only inserts the
    // outline and never touches the library. `BoardLibrary::via_padstacks` is deliberately left
    // unset — Java's is `null` until `addViaPadstack` (`:394`) creates it, and quirks #42-43 hang
    // off exactly that state.
    let library = BoardLibrary::new(Padstacks::new(layer_structure.clone()), Packages::new());
    // `:328-335`.
    let mut board = Board::new(
        outline_shapes,
        outline_clearance_no,
        bounding_box,
        board_rules,
        library,
        Components::new(),
        communication,
    );

    // ------------------------- 8. Populate Net Classes & Netlist in Rules :340
    // (now that board is fully linked)
    // `:341-342`.
    board.rules.create_default_net_class();
    let default_net_class = board.rules.get_default_net_class();
    // `:343` — a `HashMap<String, Integer>`, whose **iteration** order `resolveNetClassIndex`
    // depends on (`:972-976`). [`JavaStringMap`] reproduces it.
    let mut net_class_index_map = JavaStringMap::new();
    // `:344-347`.
    if let Some(kicad_default) = kicad_default_net_class {
        apply_kicad_net_class_parameters(
            &mut board.rules,
            default_net_class,
            kicad_default,
            layer_count,
            scale_factor,
            1,
        );
    }

    // `:349-356`.
    for (i, net_class) in additional_net_classes.iter().enumerate() {
        let cl_no = i + 2;
        let name = net_class.name.clone().unwrap_or_default();
        let board_net_class = board
            .rules
            .net_classes
            .append(name.clone(), &layer_structure, false);
        apply_kicad_net_class_parameters(
            &mut board.rules,
            board_net_class,
            net_class,
            layer_count,
            scale_factor,
            cl_no,
        );
        net_class_index_map.put(name, cl_no);
    }

    // Create via rules and register via padstacks so that the router is allowed to use vias
    // (`:358-367`).
    let mut default_via_diameter = 0.8;
    let mut default_via_drill = 0.4;
    if user_unit == Unit::Mil {
        default_via_diameter = 30.0;
        default_via_drill = 15.0;
    } else if user_unit == Unit::Um {
        default_via_diameter = 800.0;
        default_via_drill = 400.0;
    }

    // `:369-378`.
    let mut def_via_dia = default_via_diameter;
    let mut def_via_drill = default_via_drill;
    if let Some(kicad_default) = kicad_default_net_class {
        if kicad_default.viaDiameter > 0.0 {
            def_via_dia = kicad_default.viaDiameter;
        }
        if kicad_default.viaDrill > 0.0 {
            def_via_drill = kicad_default.viaDrill;
        }
    }
    // `:380-394`.
    let def_radius = def_via_dia * scale_factor / 2.0;
    let def_via_shape = via_shape(def_radius);
    let def_via_shape_arr = vec![Some(def_via_shape); layer_count];
    let default_via_padstack =
        board
            .library
            .padstacks
            .add("defaultVia", def_via_shape_arr, true, false);
    board.library.add_via_padstack(default_via_padstack);

    // `:396-401`.
    let default_via_cl_class = board
        .rules
        .net_classes
        .get(default_net_class)
        .default_item_clearance_classes
        .get(ItemClass::Via);
    let default_via_info = ViaInfo::new(
        "defaultVia",
        default_via_padstack,
        default_via_cl_class,
        true,
    );
    board.rules.via_infos.add(default_via_info);

    // `:403-406`. Plan 7 Task 0 (`bde59ef`) made `ViaRule` **own** its `ViaInfo`s, so the rule
    // gets the copy `ViaInfos::add` stamped with an identity serial rather than a fresh value —
    // that is what makes `ViaInfo::is_same_object` (Java's `==`) answer `true` for the rule's via
    // and the registered one, as it does in Java where both names bind the same object. The
    // `// Java bug:` aliasing note from plan-8 ruling H does not apply: this reader never
    // re-declares a `ViaInfo`.
    let mut default_via_rule = ViaRule::new("default");
    default_via_rule.append_via(registered_via_info(&board.rules, "defaultVia"));
    board.rules.via_rules.push(default_via_rule.clone());
    board
        .rules
        .net_classes
        .get_mut(default_net_class)
        .set_via_rule(Some(default_via_rule));

    // `:408-443`.
    for (i, net_class) in additional_net_classes.iter().enumerate() {
        let cl_no = i + 2;
        // `:411` — see the `netClasses.get(clNo - 1)` note on the net loop below.
        let board_net_class = NetClassId(cl_no - 1);

        // `:413-414`.
        let via_dia = if net_class.viaDiameter > 0.0 {
            net_class.viaDiameter
        } else {
            def_via_dia
        };
        let via_drill = if net_class.viaDrill > 0.0 {
            net_class.viaDrill
        } else {
            def_via_drill
        };
        let _ = via_drill; // `:414` is dead in section 8: only the diameter shapes the padstack.

        // `:416-427`.
        let radius = via_dia * scale_factor / 2.0;
        let via_shape_arr = vec![Some(via_shape(radius)); layer_count];

        // `:429-431`.
        let via_name = format!("via_{}", net_class.name.clone().unwrap_or_default());
        let via_padstack =
            board
                .library
                .padstacks
                .add(via_name.clone(), via_shape_arr, true, false);
        board.library.add_via_padstack(via_padstack);

        // `:433-437`.
        let via_cl_class = board
            .rules
            .net_classes
            .get(board_net_class)
            .default_item_clearance_classes
            .get(ItemClass::Via);
        let via_info = ViaInfo::new(via_name.clone(), via_padstack, via_cl_class, true);
        board.rules.via_infos.add(via_info);

        // `:439-442`.
        let mut via_rule = ViaRule::new(net_class.name.clone().unwrap_or_default());
        via_rule.append_via(registered_via_info(&board.rules, &via_name));
        board.rules.via_rules.push(via_rule.clone());
        board
            .rules
            .net_classes
            .get_mut(board_net_class)
            .set_via_rule(Some(via_rule));
    }

    // Add actual nets (`:445-452`).
    // not ported: `:446`'s `int maxNetNo = boardJson.nets.size();` — a dead local, never read
    // again (quirk #281). The dereference it performs is *not* dead, though: `boardJson.nets` is
    // unguarded here exactly as `layers` is at `:105`.
    let Some(json_nets) = board_json.nets.as_ref() else {
        return npe("java.util.List.size()", "boardJson.nets");
    };
    // As `board_layer_names` above: `fr_board::Net::name` is a `String` where Java's is nullable,
    // and `Nets.get(String, int)` (Nets.java:43-45) dereferences the **stored** name of every net
    // it walks past. A JSON `"name": null` therefore kills the first lookup that reaches it — at
    // `:491` below if anything references a net at all, otherwise in section 9 at `:639`. Both
    // measured; quirk #282, whose "crashes in section 9" claim Task 9 was handed to verify.
    let mut net_name_is_null: Vec<bool> = Vec::with_capacity(json_nets.len());
    for net_json in json_nets {
        net_name_is_null.push(net_json.name.is_none());
        // `:448`. Java's `Nets.add` assigns `rules.getDefaultNetClass()` inside the `Net`
        // constructor (Net.java:50) through the board back-pointer `BasicBoard`'s constructor
        // set; this port passes it, and `:450-451` immediately overwrites it anyway.
        //
        // Java computes `clNo` *after* the `add` (`:448-449`); the two are swapped here because
        // `Nets::add` hands back a `&mut Net` that borrows the rules `resolve_net_class_index`
        // would have to read. The swap is safe: the helper is a pure read of
        // `netClassIndexMap`, which nothing in this loop writes.
        let cl_no = resolve_net_class_index(&net_class_index_map, net_json.className.as_deref());
        let net = board.rules.nets.add(
            net_json.name.clone().unwrap_or_default(),
            1,
            net_json.containsPlane,
            NetClassId(0),
        );
        // `:450-451` — `boardRules.netClasses.get(clNo - 1)`, with Java's own comment. The
        // off-by-one is deliberate: `clNo` is a *clearance* class index, whose 0 and 1 rows are
        // `"null"` and `"default"`, and the net-class list has no `"null"` entry.
        net.set_class(NetClassId(cl_no - 1)); // NetClass array indices are 0-based
    }

    // Automatically register any referenced nets that were not explicitly defined in the nets
    // list (`:454-496`).
    //
    // `:456`'s `java.util.HashSet<String>` is iterated at `:490`, and the order it hands the
    // names back **is** the order `Nets.add` numbers them in — so the net numbers of every
    // auto-registered net are a function of `String.hashCode` and of nothing else. Quirk #280:
    // [`java_hash_iteration_order`] reproduces `java.util.HashMap`'s bucket layout so the
    // numbering matches.
    let mut referenced_nets = JavaStringSet::new();
    if let Some(components) = board_json.components.as_ref() {
        for component in components {
            if let Some(pads) = component.pads.as_ref() {
                for pad in pads {
                    if let Some(net_name) = pad.netName.as_deref()
                        && !net_name.is_empty()
                    {
                        referenced_nets.add(net_name);
                    }
                }
            }
        }
    }
    if let Some(zones) = board_json.conductionAreas.as_ref() {
        for zone in zones {
            if let Some(net_name) = zone.netName.as_deref()
                && !net_name.is_empty()
            {
                referenced_nets.add(net_name);
            }
        }
    }
    if let Some(traces) = board_json.traces.as_ref() {
        for trace in traces {
            if let Some(net_name) = trace.netName.as_deref()
                && !net_name.is_empty()
            {
                referenced_nets.add(net_name);
            }
        }
    }
    if let Some(vias) = board_json.vias.as_ref() {
        for via in vias {
            if let Some(net_name) = via.netName.as_deref()
                && !net_name.is_empty()
            {
                referenced_nets.add(net_name);
            }
        }
    }

    // `:490-496`. The `nets.get(netName, 1)` lookup is case-**insensitive**
    // (`String.equalsIgnoreCase`, Nets.java:44), so a pad whose net name differs only in case
    // from a declared net registers nothing.
    for net_name in referenced_nets.iteration_order() {
        // `:491`. [`java_nets_get`] rather than `Nets::get_by_name_and_subnet` because Java's
        // loop dereferences `currentNet.name` **before** comparing, so a declared net whose name
        // was `null` throws here rather than simply failing to match (quirk #282).
        let existing = match java_nets_get(&board.rules.nets, &net_name_is_null, Some(net_name), 1)
        {
            Ok(existing) => existing,
            Err(error) => return npe(error.invoked, error.receiver),
        };
        if existing.is_none() {
            let net = board
                .rules
                .nets
                .add(net_name.to_string(), 1, false, NetClassId(0));
            // Fallback to default class (default net class is at index 0)
            net.set_class(NetClassId(0));
            net_name_is_null.push(false);
        }
    }

    // ================= 9. Load Components & Library templates :498
    //
    // `:499-500` alias `board.library.padstacks` and `board.library.packages` into two locals.
    // The port reaches both through `board.library` at each use: Rust cannot hold two `&mut`
    // borrows into one struct, and the aliases carry no behaviour of their own.
    //
    // # The two recovery boundaries Java has on this path
    //
    // Both are **reproduced from the jar, not invented**, and both live in section 9.
    //
    // 1. `:603`'s `catch (Exception e)` wraps **only the package-dedup lookup**, not the
    //    component. It logs `"KiCadJsonReader package deduplication error, falling back"` and adds
    //    a *duplicate* package under the base name; the component is still created and every later
    //    component still loads. *(The task brief called it "skips one component and continues".
    //    The jar says otherwise — stem `pad-null-name-dedup` in
    //    `crates/fr-dsn/tests/data/p8t8-kicad-read-b.txt` loads all three components and ends with
    //    three packages all named `NONAME`. Java wins over the brief.)* The one throw it can catch
    //    is `arePackagePinsIdentical:908`'s `pin1.name.equals(...)` over a `null` pin name — see
    //    [`are_package_pins_identical`], which answers that throw as an `Err` rather than
    //    performing it. Quirk #285.
    // 2. `:746`'s `catch (Throwable e)` is the method boundary and answers a `ParseError`. The
    //    port has no blanket catch: **every** point Java throws from inside sections 9-11 is an
    //    explicit early return carrying Java's own message: 24 returns over 19 distinct Java
    //    throw sites, and the transcript pins every one of them. `Throwable` also catches a `StackOverflowError` — the one place in this whole port
    //    where Java recovers from a JVM `Error` (quirk #279) — but nothing on this path recurses,
    //    so no site below needs one.
    //
    // **No `catch_unwind`.** One panic *was* found that Java's `Exception` arm would have caught:
    // a padstack with a null shape on every layer makes `DrillItem.tileShapeCount` negative and
    // Java throws `NegativeArraySizeException`. It is answered by an explicit test at the two
    // insert sites ([`java_drill_item_tile_shape_count`]), which is the reported finding quirk
    // #286 records — not by catching a Rust panic.

    // Java's `Package.Pin.name` is a nullable `String` and `fr_board::PackagePin::name` is a
    // `String`, so a pad with no `name` key is totalized to `""` (quirk #282's family). Recovery
    // boundary 1 above turns on exactly that lost bit, because `arePackagePinsIdentical`
    // dereferences the **existing** package's pin name. This side table keeps it, indexed by
    // `Package.no - 1`; section 9 is the only thing that ever adds a package to this board, so it
    // stays in step with `board.library.packages` by construction.
    let mut package_pin_names: Vec<Vec<Option<String>>> = Vec::new();

    // `:503` dereferences `boardJson.components` with no null check.
    let Some(json_components) = board_json.components.as_ref() else {
        return npe("java.util.List.iterator()", "boardJson.components");
    };
    for component in json_components {
        // `:506` dereferences `comp.pads` with no null check.
        let Some(pads) = component.pads.as_ref() else {
            return npe("java.util.List.iterator()", "comp.pads");
        };
        // `:505`.
        let mut package_pins: Vec<PackagePin> = Vec::new();
        let mut package_pin_name_arr: Vec<Option<String>> = Vec::new();
        for pad in pads {
            // `:508` — one slot per board layer, all null until `:555-557` fills a range.
            let mut shapes: Vec<Option<Shape>> = vec![None; layer_count];
            // `:509-510` — `pad.size` is unguarded. Its Java field initializer is
            // `new Point2D()`, so only an explicit `"size": null` reaches this.
            let Some(pad_size) = pad.size.as_ref() else {
                return npe_field("x", "pad.size");
            };
            let dx = pad_size.x * scale_factor / 2.0;
            let dy = pad_size.y * scale_factor / 2.0;
            // `:512-534` — the three shape arms. Each test is `"<literal>".equalsIgnoreCase(
            // pad.shape)`, i.e. the *literal* is the receiver, so a `null` shape is `false` rather
            // than a throw and lands on the `else`.
            let pad_shape = if pad
                .shape
                .as_deref()
                .is_some_and(|shape| equals_ignore_case("circle", shape))
            {
                // `:512-514`. Note the radius comes from the **unscaled halves' minimum**, so a
                // non-square circular pad is drawn to the smaller dimension.
                let radius = java_min(pad_size.x, pad_size.y) * scale_factor / 2.0;
                Shape::Circle(Circle::new(IntPoint::ZERO, java_round_to_int(radius)))
            } else if pad
                .shape
                .as_deref()
                .is_some_and(|shape| equals_ignore_case("oval", shape))
            {
                // `:515-525` — the `"oval"` arm the task brief names: an `IntOctagon` built from
                // the four sides plus a 45-degree corner cut of `(2 - sqrt(2)) * min(dx, dy)`,
                // then `toSimplex()`. Every one of the eight arguments is transcribed in Java's
                // order (IntOctagon.java:72-80: leftX, bottomY, rightX, topY, upperLeftDiagonalX,
                // lowerRightDiagonalX, lowerLeftDiagonalX, upperRightDiagonalX).
                let lx = java_round_to_int(-dx);
                let rx = java_round_to_int(dx);
                let ly = java_round_to_int(-dy);
                let uy = java_round_to_int(dy);
                let r = java_round_to_int(java_min(dx, dy));
                let cut = java_round_to_int((2.0 - 2.0_f64.sqrt()) * f64::from(r));
                let octagon = IntOctagon::new(
                    lx,
                    ly,
                    rx,
                    uy,
                    lx - uy + cut,
                    rx - ly - cut,
                    lx + ly + cut,
                    rx + uy - cut,
                );
                Shape::Tile(TileShape::Simplex(octagon.to_simplex()))
            } else {
                // `:526-534` — the fall-through the task brief names: `"rect"`, `"rectangle"`,
                // any other spelling, and a `null` shape all become the plain box.
                Shape::Tile(TileShape::Simplex(
                    IntBox::from_coords(
                        java_round_to_int(-dx),
                        java_round_to_int(-dy),
                        java_round_to_int(dx),
                        java_round_to_int(dy),
                    )
                    .to_simplex(),
                ))
            };

            // Standardize pad's layer mappings (`:536-553`).
            let mut start_layer = 0usize;
            let mut end_layer = layer_count - 1;
            if let Some(pad_layers) = pad.layers.as_ref()
                && !pad_layers.is_empty()
            {
                // `:541-542`. Note the seeds: `lowestIdx` starts at the **last** layer and
                // `highestIdx` at the **first**, so a list that matches nothing leaves
                // `startLayer > endLayer` and the fill loop below simply does not run — the
                // padstack then has a null shape on every layer, which quirk #286 is about.
                let mut lowest_idx = layer_count - 1;
                let mut highest_idx = 0usize;
                for layer_name in pad_layers {
                    for (li, board_layer_name) in board_layer_names.iter().enumerate() {
                        // `:545` — **quirk #282's crash site**, and the line Task 8 handed this
                        // task to verify. `boardLayers[li].name.equalsIgnoreCase(layerName)`
                        // dereferences the *board layer's* name, which Gson leaves `null` for a
                        // JSON `"name": null`. Measured: stem `layer-null-name-with-pads` is a
                        // `ParseError` here, and `layer-null-name-no-pad-layers` — the same board
                        // with an empty pad `layers` list, which never enters this branch — loads.
                        let Some(board_layer_name) = board_layer_name.as_deref() else {
                            return npe("String.equalsIgnoreCase(String)", "boardLayers[li].name");
                        };
                        // A `null` *element* of `pad.layers` is `equalsIgnoreCase(null)`, i.e.
                        // false, not a throw — quirk #283, which is why the DTO's element type is
                        // `Option<String>`.
                        if layer_name
                            .as_deref()
                            .is_some_and(|name| equals_ignore_case(board_layer_name, name))
                        {
                            lowest_idx = lowest_idx.min(li);
                            highest_idx = highest_idx.max(li);
                        }
                    }
                }
                // `:551-552`.
                start_layer = lowest_idx;
                end_layer = highest_idx;
            }

            // `:555-557`. The guard is Java's loop condition: `startLayer > endLayer` — every
            // layer name unmatched — leaves the array all-null, which is quirk #286's input
            // condition.
            if start_layer <= end_layer {
                for shape in &mut shapes[start_layer..=end_layer] {
                    *shape = Some(pad_shape.clone());
                }
            }

            // `:559`.
            let is_drillable = pad.drill > 0.0;
            // `:560`.
            let padstack_name =
                match get_descriptive_padstack_name(pad, &board_layer_names, layer_count) {
                    Ok(name) => name,
                    // `:746` over whatever `:857-890` threw.
                    Err(message) => {
                        return parse_error(
                            "json_payload",
                            &format!("Exception occurred: {message}"),
                        );
                    }
                };
            // `:561-564`. `Padstacks.get(String)` is case-**insensitive**
            // (core/library/Padstacks.java:25-32),
            // and the name encodes neither the layer span nor the drill flag — quirk #284, which
            // is why a second pad can silently inherit the first one's shapes.
            let existing_padstack = board
                .library
                .padstacks
                .get_by_name(&padstack_name)
                .map(|padstack| PadstackId(padstack.no));
            let padstack = match existing_padstack {
                Some(id) => id,
                None => board
                    .library
                    .padstacks
                    .add(padstack_name, shapes, is_drillable, false),
            };
            // `:565-568` — `pad.offset` is unguarded, and its Y is negated as every other
            // coordinate in this reader is.
            let Some(offset) = pad.offset.as_ref() else {
                return npe_field("x", "pad.offset");
            };
            let relative_loc = Vector::Int(IntVector::new(
                java_round_to_int(offset.x * scale_factor),
                java_round_to_int(-offset.y * scale_factor),
            ));
            // `:569`. `Package.Pin.name` takes `pad.name` verbatim, `null` included; see
            // `package_pin_names` above for the bit this line drops.
            package_pins.push(PackagePin::new(
                pad.name.clone().unwrap_or_default(),
                padstack,
                relative_loc,
                0.0,
            ));
            package_pin_name_arr.push(pad.name.clone());
        }

        // `:572` — anything that is not `B.Cu` (case-insensitively) is the front, `null` layer
        // included.
        let is_front = !component
            .layer
            .as_deref()
            .is_some_and(|layer| equals_ignore_case("B.Cu", layer));
        // `:573-576`.
        let base_package_name = match component.footprint.as_deref() {
            Some(footprint) if !footprint.is_empty() => footprint.to_string(),
            _ => "Package".to_string(),
        };

        // `:580-619` — the package-dedup ladder. `suffix == 0` tries the base name; every later
        // round tries `"<base>::<suffix>"`, which `Packages.get` strips back to the base name
        // (Packages.java:40) unless a package with that exact name already exists.
        let mut suffix = 0usize;
        let component_package = loop {
            // `:581`.
            let test_name = if suffix == 0 {
                base_package_name.clone()
            } else {
                format!("{base_package_name}::{suffix}")
            };
            // `:583-584`.
            let existing = board
                .library
                .packages
                .get_by_name(&test_name, is_front)
                .map(|existing| existing.no);
            let names_match = existing.is_some_and(|no| {
                equals_ignore_case(&board.library.packages.get(no).name, &test_name)
            });
            if !names_match {
                // `:585-596`. Java passes `null` for the outline, the outline widths and the
                // closed flags, and three empty `Keepout[]`s.
                let added = board.library.packages.add(
                    test_name,
                    package_pins.clone(),
                    None,
                    None,
                    None,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                    is_front,
                );
                package_pin_names.push(package_pin_name_arr.clone());
                break added;
            }
            let existing_no = existing.expect("names_match implies a package was found");
            // `:598` inside `:582`'s `try`.
            match are_package_pins_identical(
                board.library.packages.get(existing_no),
                &package_pin_names[existing_no - 1],
                &package_pins,
                &package_pin_name_arr,
            ) {
                // `:599-601`.
                Ok(true) => break existing_no,
                Ok(false) => {}
                // `:603-617` — recovery boundary 1. not ported: `:604`'s `FRLogger.error`.
                // The fallback name is `comp.footprint` *raw* — **not** `basePackageName`, so an
                // empty-string footprint produces a package literally named `""` here where the
                // ladder above would have called it `"Package"`.
                Err(_) => {
                    let fallback_name = component
                        .footprint
                        .clone()
                        .unwrap_or_else(|| "Package".to_string());
                    let added = board.library.packages.add(
                        fallback_name,
                        package_pins.clone(),
                        None,
                        None,
                        None,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        is_front,
                    );
                    package_pin_names.push(package_pin_name_arr.clone());
                    break added;
                }
            }
            // `:618`.
            suffix += 1;
        };
        debug_assert_eq!(
            package_pin_names.len(),
            board.library.packages.count(),
            "the nullable-pin-name side table must stay indexed by Package.no - 1"
        );

        // `:620-623` — `comp.position` is unguarded.
        let Some(position) = component.position.as_ref() else {
            return npe_field("x", "comp.position");
        };
        let position = IntPoint::new(
            java_round_to_int(position.x * scale_factor),
            java_round_to_int(-position.y * scale_factor),
        );

        // `:625-634`. Note `-comp.rotation`: the JSON's rotation is negated, and `Component`'s
        // constructor then normalises it into `[0, 360)` (Component.java:65-70) — so `0.0` stays
        // `-0.0`, because `-0.0 < 0` is false in Java and in Rust alike.
        //
        // Java bug (quirk #287): `Components.add:51` hands the new component to
        // `UndoableObjects.insert`, whose `ConcurrentSkipListMap.put` orders keys through
        // `Component.compareTo` -> `this.name.compareToIgnoreCase(...)`. A `null` `reference`
        // therefore dies inside the *container*, before anything reads the component — measured on
        // both `comp-null-reference-single` (one component is enough) and
        // `comp-null-reference-second`. The port has no undo list at all (global constraints), so
        // the throw is reproduced here rather than arising.
        let Some(reference) = component.reference.as_deref() else {
            return parse_error(
                "json_payload",
                "Exception occurred: Cannot invoke \"String.compareToIgnoreCase(String)\" \
                 because \"this.name\" is null",
            );
        };
        let board_comp_id = board
            .components
            .add(
                reference.to_string(),
                Some(Point::Int(position)),
                -component.rotation,
                is_front,
                component_package,
                component_package,
                true,
                component.value.clone(),
            )
            .id;

        // Insert actual pin items mapped to nets (`:637-644`).
        for (pad_index, pad) in pads.iter().enumerate() {
            // `:639-641`.
            let target_net = match java_nets_get(
                &board.rules.nets,
                &net_name_is_null,
                pad.netName.as_deref(),
                1,
            ) {
                Ok(target) => target,
                Err(error) => return npe(error.invoked, error.receiver),
            };
            let net_number = target_net.unwrap_or(0);
            let net_numbers = if net_number > 0 {
                vec![net_number]
            } else {
                Vec::new()
            };
            // Quirk #286: `:642`'s `insertPin` reaches `DrillItem.tileShapeCount` through the
            // search tree, and a package pin whose padstack has no shape on any layer makes that
            // count negative. The pin's padstack is the one on the **component's** package, which
            // is not always the array built above (the ladder may have reused an existing
            // package), so it is read back the way `Pin.getPadstack` does.
            let pin_padstack = board
                .library
                .packages
                .get(component_package)
                .get_pin(i32::try_from(pad_index).unwrap_or(i32::MAX))
                .map(|pin| pin.padstack_no);
            if let Some(pin_padstack) = pin_padstack
                && let Some(padstack) = board.library.padstacks.get(pin_padstack)
            {
                let span = java_drill_item_tile_shape_count(padstack);
                if span < 0 {
                    return parse_error("json_payload", &format!("Exception occurred: {span}"));
                }
            }
            // `:642-643`.
            board.insert_pin(
                board_comp_id,
                i32::try_from(pad_index).unwrap_or(i32::MAX),
                net_numbers,
                outline_clearance_no,
                FixedState::SystemFixed,
            );
        }
    }

    // ================= 10. Load conduction areas (copper pours) :647
    // `:648` dereferences `boardJson.conductionAreas` with no null check.
    let Some(json_zones) = board_json.conductionAreas.as_ref() else {
        return npe("java.util.List.iterator()", "boardJson.conductionAreas");
    };
    for zone in json_zones {
        // `:649-651`.
        let target_net = match java_nets_get(
            &board.rules.nets,
            &net_name_is_null,
            zone.netName.as_deref(),
            1,
        ) {
            Ok(target) => target,
            Err(error) => return npe(error.invoked, error.receiver),
        };
        let net_number = target_net.unwrap_or(0);
        let net_numbers = if net_number > 0 {
            vec![net_number]
        } else {
            Vec::new()
        };

        // Build Area path polygon (`:653-660`) — `zone.polygon` is unguarded.
        let Some(polygon) = zone.polygon.as_ref() else {
            return npe("java.util.List.size()", "zone.polygon");
        };
        let mut zone_points: Vec<Point> = Vec::with_capacity(polygon.len());
        for corner in polygon {
            zone_points.push(Point::Int(IntPoint::new(
                java_round_to_int(corner.x * scale_factor),
                java_round_to_int(-corner.y * scale_factor),
            )));
        }
        // `:661`. `new PolygonShape(Point[])` reads `corners[0]` (PolygonShape.java's
        // constructor), so an **empty** polygon is an `ArrayIndexOutOfBoundsException` rather
        // than an empty shape. Measured, stem `zone-empty-polygon`.
        if zone_points.is_empty() {
            return parse_error(
                "json_payload",
                "Exception occurred: Index 0 out of bounds for length 0",
            );
        }
        // `:662-663`. totalized: `zone.layerIndex` is a Java `int` that `ObstacleArea` stores
        // verbatim — a negative one is kept, as stem `zone-negative-layer` measures (`layer=-3`)
        // — while `fr_board`'s layer is a `usize`.
        //
        // **The cast below is wrapping, and that is the deliberate choice**, not an oversight:
        // `-3 as usize` is `18_446_744_073_709_551_613`, i.e. the *same 64 bits* Java's `int`
        // holds sign-extended, so the port stores Java's value and only its **rendering**
        // differs. Measured: the stem's Rust row prints that number where the jar prints `-3`,
        // which is the one `XDIFF_B` entry it carries. `try_from(...).unwrap_or(0)` and a clamp
        // were both rejected — each would silently move the zone to a *different* layer, which
        // Java never does, and would turn a legible divergence into an invented one. Nothing on
        // the KiCad path produces a negative `layerIndex` and no corpus fixture carries one; the
        // stem exists so the divergence is measured rather than assumed.
        #[allow(clippy::cast_sign_loss)] // deliberate: preserves Java's bits, see above.
        board.insert_conduction_area(
            Area::Shape(Shape::Polygon(PolygonShape::from_points(&zone_points))),
            zone.layerIndex as usize,
            net_numbers,
            1,
            zone.isObstacle,
            FixedState::UserFixed,
        );
    }

    // ================= 11. Load traces and vias (existing wiring) :666
    // `:667` dereferences `boardJson.traces` with no null check.
    let Some(json_traces) = board_json.traces.as_ref() else {
        return npe("java.util.List.iterator()", "boardJson.traces");
    };
    for trace in json_traces {
        // `:668-670`.
        let target_net = match java_nets_get(
            &board.rules.nets,
            &net_name_is_null,
            trace.netName.as_deref(),
            1,
        ) {
            Ok(target) => target,
            Err(error) => return npe(error.invoked, error.receiver),
        };
        let net_number = target_net.unwrap_or(0);
        let net_numbers = if net_number > 0 {
            vec![net_number]
        } else {
            Vec::new()
        };
        // `:671`.
        let trace_half_width = java_round_to_int(trace.width * scale_factor / 2.0);

        // `:673-679` — `tr.points` is unguarded, and the Y is negated as everywhere else.
        let Some(points) = trace.points.as_ref() else {
            return npe("java.util.List.size()", "tr.points");
        };
        let mut trace_points: Vec<Point> = Vec::with_capacity(points.len());
        for point in points {
            trace_points.push(Point::Int(IntPoint::new(
                java_round_to_int(point.x * scale_factor),
                java_round_to_int(-point.y * scale_factor),
            )));
        }
        // `:680-681`.
        //
        // **Convention 7, decided at this line: neither `new_polyline` nor
        // `new_polyline_in_place`.** `:680` calls the `BasicBoard.insertTrace(Point[], …)`
        // overload (BasicBoard.java:248-262), whose polyline is `new Polyline(Point[])`
        // (Polyline.java:54-56) — the *Polygon* constructor. It is handed no `Line[]`, so there is
        // no caller array for the normaliser to write back through and the identity-token
        // contract [`fr_geometry::Polyline::from_lines_in_place`] exists for cannot arise.
        // [`fr_board::Board::insert_trace_at_points`] is the exact port of the overload Java
        // calls, and its `Polyline::from_points` is the exact port of the constructor.
        // `crates/fr-dsn/src/parser/wiring.rs:378` is the other branch of the same choice, made
        // there because `Wiring.java:562` really does hand `new Polyline(Line[])` an array.
        //
        // `Trace`'s constructor clamps the layer twice (Trace.java:45-47): `Math.max(layer, 0)` —
        // reproduced here, because the port's parameter is a `usize` — and then
        // `Math.min(layer, getLayerCount() - 1)`, which `PolylineTrace::new` already carries.
        // Measured on both sides of the range: `trace-negative-layer` lands on layer 0 and
        // `trace-layer-out-of-range` on the last layer.
        board.insert_trace_at_points(
            &trace_points,
            trace.layerIndex.max(0) as usize,
            trace_half_width,
            net_numbers,
            1,
            FixedState::UserFixed,
        );
    }

    // `:684` dereferences `boardJson.vias` with no null check.
    let Some(json_vias) = board_json.vias.as_ref() else {
        return npe("java.util.List.iterator()", "boardJson.vias");
    };
    for via in json_vias {
        // `:685-687`.
        let target_net = match java_nets_get(
            &board.rules.nets,
            &net_name_is_null,
            via.netName.as_deref(),
            1,
        ) {
            Ok(target) => target,
            Err(error) => return npe(error.invoked, error.receiver),
        };
        let net_number = target_net.unwrap_or(0);
        let net_numbers = if net_number > 0 {
            vec![net_number]
        } else {
            Vec::new()
        };

        // `:689-692` — `vj.position` is unguarded.
        let Some(position) = via.position.as_ref() else {
            return npe_field("x", "vj.position");
        };
        let center = IntPoint::new(
            java_round_to_int(position.x * scale_factor),
            java_round_to_int(-position.y * scale_factor),
        );

        // Dynamically create via padstack (`:694-707`). The shape is the same
        // `new IntBox(round(-r), …).toSimplex()` section 8 builds, so it goes through the same
        // [`via_shape`] — the drill is not in it (quirk #281).
        let mut shapes: Vec<Option<Shape>> = vec![None; layer_count];
        let radius = via.diameter * scale_factor / 2.0;
        let shape = via_shape(radius);
        // `:705-707`. Java writes `shapes[li]` with no bounds test, so an index outside
        // `[0, layerCount)` is an `ArrayIndexOutOfBoundsException` — reported by the method's
        // `catch (Throwable)` as a parse error about the JSON file. The loop ascends, so the
        // **first** offending index is the one named; measured on `via-negative-layer`
        // (`Index -1 out of bounds for length 2`) and `via-layer-out-of-range` (`Index 2 …`).
        let mut layer = via.startLayerIndex;
        while layer <= via.endLayerIndex {
            let Ok(index) = usize::try_from(layer) else {
                return parse_error(
                    "json_payload",
                    &format!(
                        "Exception occurred: Index {layer} out of bounds for length {layer_count}"
                    ),
                );
            };
            if index >= layer_count {
                return parse_error(
                    "json_payload",
                    &format!(
                        "Exception occurred: Index {index} out of bounds for length {layer_count}"
                    ),
                );
            }
            shapes[index] = Some(shape.clone());
            layer += 1;
        }
        // `:708-711` — the second of the two generated names quirk #288 is about.
        let via_padstack_name = format!(
            "Via[{}-{}]_{}:{}_um",
            via.startLayerIndex,
            via.endLayerIndex,
            java_format_fixed(via.diameter * 1000.0, 0),
            java_format_fixed(via.drill * 1000.0, 0)
        );
        // `:712-715`.
        let existing_padstack = board
            .library
            .padstacks
            .get_by_name(&via_padstack_name)
            .map(|padstack| PadstackId(padstack.no));
        let via_padstack = match existing_padstack {
            Some(id) => id,
            None => board
                .library
                .padstacks
                .add(via_padstack_name, shapes, true, false),
        };
        // Quirk #286, as at the pin site above: `startLayerIndex > endLayerIndex` leaves every
        // shape null, `Padstack.fromLayer()` answers `layerCount` and `toLayer()` answers `-1`,
        // and `DrillItem.tileShapeCount` is then `-layerCount`. `new TileShape[-2]` throws
        // `NegativeArraySizeException`, whose `getMessage()` is the bare number. Measured, stem
        // `via-start-gt-end`.
        if let Some(padstack) = board.library.padstacks.get(via_padstack) {
            let span = java_drill_item_tile_shape_count(padstack);
            if span < 0 {
                return parse_error("json_payload", &format!("Exception occurred: {span}"));
            }
        }
        // `:716`. **The checked seam**, not the unchecked wrapper: `BasicBoard.insertVia` walks
        // `fromLayer..toLayer` calling `splitTraces` -> `PolylineTrace.split`, the machinery quirk
        // #76 does not terminate in, and Java has no catch around it. Task 3 gave the DSN reader
        // the same seam at `crates/fr-dsn/src/parser/wiring.rs:624` — the call; the comment
        // that argues the choice starts twenty-eight lines above it — backed by
        // `DsnReadOptions::normalize_time_limit`; this reader has no options struct of its own, so
        // it passes the same `|| false` the unchecked wrapper does and says so here rather than
        // silently taking the wrapper. A KiCad JSON is a fresh board with no pre-existing traces
        // for `splitTraces` to walk on the first via, and every later via can only meet traces
        // this same reader inserted.
        let stop = || false;
        if let Err(error) = board.insert_via_checked(
            via_padstack,
            Point::Int(center),
            net_numbers,
            1,
            FixedState::UserFixed,
            true,
            &stop,
        ) {
            // totalized: `readBoard:716`'s `board.insertVia` cannot fail in Java, and the port's
            // answers a `Result` because `split_traces` can surface a `Polyline` normalisation
            // failure (quirk #109). Java would reach `:746`'s `catch (Throwable)` for the same
            // condition, so the failure is reported the same way.
            return parse_error("json_payload", &format!("Exception occurred: {error}"));
        }
    }

    // `:719-725`'s duration `FRLogger.debug` — not ported, as `:83-86`.

    // Build metadata for the board (`:727-736`).
    //
    // Java bug: `:730-731` passes the **literals** `"KiCad"` and `"v10.0"`, not the `hostCad` /
    // `hostVersion` locals section 6 resolved 400 lines earlier — so a board that names its host
    // reports the wrong one in its metadata while its `Communication` reports the right one.
    // Quirk #278; measured, stem `resolution-zero` (`hostCad=Altium` in `comm`, `hostCad=KiCad`
    // in `metadata`).
    let metadata = BoardMetadata {
        host_cad: Some("KiCad".to_string()),
        host_version: Some("v10.0".to_string()),
        layer_count,
        unit: user_unit,
        resolution,
        snap_angle: fr_board::AngleRestriction::FortyFiveDegree,
        router_settings: None,
    };

    // `:738-743`.
    let mut warnings: Vec<String> = Vec::new();
    if outline_missing {
        warnings.push(
            "Board Outline/Boundary is missing or empty in the JSON file. A supposed board edge \
             around components with 5mm padding was generated."
                .to_string(),
        );
    }
    // `:744`. `coordinate_transform` is the field the **port** adds to `BoardReadResult`
    // (plan-3 controller ruling A); Java's record has no such member and its writers re-derive
    // the transform from the board.
    BoardReadResult::Success {
        board: Some(Box::new(board)),
        metadata: Some(metadata),
        warnings,
        coordinate_transform: Some(coordinate_transform),
    }
}

// ==================================================================== importSession (:757-855)

/// Port of `KiCadJsonReader.importSession` (KiCadJsonReader.java:757-855): the traces, vias and
/// conduction areas of a KiCad **session** JSON, imported onto an existing board.
///
/// # Where it is reached from — **two** call sites, both live
///
/// * `Freerouting.initializeDrc:301-307` — the `.json` arm of `-drc`'s optional session slot,
///   ported at `crates/freerouting/src/commands/drc.rs`'s `load_session_file`.
/// * `RoutingJobScheduler.java:194-207` — the `.json` arm of `-di`, ported at
///   `crates/freerouting/src/commands/route.rs`'s `import_session_file`. **Re-read against the
///   clone for this citation**: `:194-197` is the `endsWith(".json")` test, `:198-200` the
///   "Loading …" log, `:201-202` the `FileReader`, `:203-204` the call, `:205-206` the success
///   log and `:207` the try-with-resources close; **`:208` opens the SES `else`**, so a span
///   ending at `:211` names four lines of the wrong branch.
///
/// Both sites test `filename.toLowerCase().endsWith(".json")` and hand the *other* branch to
/// `SesReader`.
///
/// # Quirk #290 (label U): Java opens the file with the **platform default charset**
///
/// `new java.io.FileReader(sessionFile)` (`Freerouting.java:304`, and the same at
/// `RoutingJobScheduler.java:203`) is the one-argument constructor, i.e.
/// `Charset.defaultCharset()` — while every other JSON path in the tree names UTF-8 explicitly
/// (`RoutingJob.setInputFromFile`, `KiCadJsonWriter`'s caller at
/// `RoutingJobSchedulerActionThread.java:278`, `GsonProvider`'s readers). The port takes a
/// decoded `&str` and its callers decode UTF-8. See `docs/java-quirks.md` #290 for the
/// measurement: on **JDK 18+ the two agree**, because JEP 400 made `Charset.defaultCharset()`
/// UTF-8 regardless of the locale, and the divergence is reachable only under an older JVM or an
/// explicit `-Dfile.encoding`.
///
/// # Signature
///
/// Java takes `(Reader, RoutingBoard)` and `throws Exception`; the port takes the decoded
/// document and answers [`DsnError::KicadSession`], whose payload is the Java throwable's own
/// `toString()` — which is the first line log4j prints for `FRLogger.error(msg, e)` at both call
/// sites. Java's `void` return carries the same information: everything it inserted before the
/// throw stays on the board, and the port's early returns leave it there too.
///
/// # It is **not** `readBoard`'s sections 10-11 a second time
///
/// The two look alike and differ in four places that are all behaviour:
///
/// | | `readBoard:647-717` | `importSession:776-854` |
/// |---|---|---|
/// | the three lists | dereferenced unguarded — a `"traces": null` is an NPE | each guarded by `!= null` (`:777`, `:797`, `:817`) |
/// | the scale factor | `structure`'s, from section 6 | recomputed here from `unit`/`resolution` (`:763-774`), including the `1.0`-means-`10000` special case |
/// | the via shape array | `shapes[li] = viaShape` **unbounded** (`:706`) — an out-of-range layer throws | `if (li >= 0 && li < layerCount)` (`:840`) — an out-of-range layer is silently skipped |
/// | the trace layer | `insertTrace` clamps it (Trace.java:45-47) | the same clamp, reached through the same call |
///
/// The port therefore transcribes `:757-855` on its own rather than sharing a helper with the
/// reader: a shared helper would have to carry the four differences as flags, and the third one
/// is the difference between a refusal and a silent skip.
///
/// # Errors
///
/// [`DsnError::KicadSession`] for every point Java throws: an empty or unparseable payload
/// (`:759-761`), a `null` `zone.polygon` / `tr.points` / `vj.position`, an empty zone polygon
/// (`new PolygonShape(new Point[0])` reads `corners[0]`), and quirk #286's negative
/// `DrillItem.tileShapeCount`. Plus the port-only `insert_via_checked` failure, which Java's
/// `insertVia` cannot produce and which the caller's `catch (Exception)` would have caught.
pub fn import_session(json: &str, board: &mut Board) -> Result<(), DsnError> {
    // `:758` — `GsonProvider.GSON.fromJson(reader, KiCadBoardJson.class)`.
    //
    // Gson's `fromJson(Reader, Class)` answers **`null`** for a stream that holds no JSON value at
    // all — `JsonReader.peek()` sees `END_DOCUMENT` and `Gson.fromJson` returns `null` rather than
    // throwing. `serde_json` calls that an EOF error, so an empty (or whitespace-only) document is
    // routed to the `null` arm here instead of to the syntax-error arm; measured, stem
    // `json-empty-string`, which the jar answers with `IllegalArgumentException` and not with a
    // `JsonSyntaxException`.
    //
    // `str::trim` uses Rust's **Unicode** `White_Space` set where Gson's `JsonReader` skips only
    // the four ASCII characters ` `, `\t`, `\r`, `\n` (plus its comment syntax). So a document
    // consisting of nothing but, say, `U+00A0` takes this arm here and Gson's
    // `MalformedJsonException` there. **Checked: no probe stem reaches it** — the four
    // empty-ish stems are `""`, `null`, `{}` and `{"unit":`, none of which contains a non-ASCII
    // space — so the divergence is unmeasured, and narrowing the test to the four ASCII
    // characters would trade one unmeasured prose divergence (quirk #277's) for another. Left as
    // Rust's trim, said here so the next reader does not have to re-derive the difference.
    if json.trim().is_empty() {
        return Err(DsnError::KicadSession(
            "java.lang.IllegalArgumentException: JSON session file payload is empty or invalid"
                .to_string(),
        ));
    }
    let board_json: Option<KiCadBoardJson> = serde_json::from_str(json).map_err(|error| {
        // Gson's own refusal. `Strictness.LENIENT` accepts more than `serde_json` does, so the
        // *message* differs; quirk #277 already records that for `readBoard` and the same
        // divergence-in-prose applies here. Both refuse, and both refuse before inserting
        // anything.
        DsnError::KicadSession(format!("com.google.gson.JsonSyntaxException: {error}"))
    })?;
    // `:759-761`. Gson answers `null` for the document `null` and for an empty stream; serde's
    // `Option<T>` reproduces the first and its `Err` above the second.
    let Some(board_json) = board_json else {
        return Err(DsnError::KicadSession(
            "java.lang.IllegalArgumentException: JSON session file payload is empty or invalid"
                .to_string(),
        ));
    };

    // `:763-768`.
    let user_unit = match board_json.unit {
        Some(UnitJson::MIL) => Unit::Mil,
        Some(UnitJson::UM) => Unit::Um,
        _ => Unit::Mm,
    };

    // `:770-774`. `(int) Math.max(1.0, …)` is a **narrowing** cast: it truncates toward zero and
    // saturates at `Integer.MIN_VALUE`/`MAX_VALUE`, which is what [`java_double_to_int`]
    // reproduces. The `== 1.0 && MM` special case then replaces the *default* resolution with
    // `10000`, so a session document that omits `resolution` altogether (leaving
    // `KiCadBoardJson.resolution`'s `= 1.0` initialiser) is read in tenths of a micrometre —
    // the units `KiCadJsonWriter` writes.
    // [`java_max`], not `f64::max`: Java's `Math.max` propagates NaN and Rust's `max` drops it.
    // Unreachable — `serde_json` rejects the bare `NaN` token `Strictness.LENIENT` accepts, which
    // is quirk #277's territory — but this module already carries the helper for exactly this
    // difference and `read_board:129` uses it at the sibling site.
    let mut resolution = java_double_to_int(java_max(1.0, board_json.resolution));
    if board_json.resolution == 1.0 && user_unit == Unit::Mm {
        resolution = 10_000;
    }
    let scale_factor = f64::from(resolution);

    // Every net on the board came from a reader that cannot store a `null` name (`readBoard`
    // crashes on one long before this, and the DSN reader has no nullable name at all), so
    // `java_nets_get`'s null-name side table is empty here — see its own docs.
    let net_name_is_null: Vec<bool> = Vec::new();
    let layer_count = board.get_layer_count();

    // ── 1. Conduction Areas (`:776-794`) ────────────────────────────────────────────────────
    if let Some(zones) = board_json.conductionAreas.as_ref() {
        for zone in zones {
            // `:779-781`.
            let net_number = java_nets_get(
                &board.rules.nets,
                &net_name_is_null,
                zone.netName.as_deref(),
                1,
            )
            .map_err(session_npe)?
            .unwrap_or(0);
            let net_numbers = if net_number > 0 {
                vec![net_number]
            } else {
                Vec::new()
            };

            // `:783-789` — `zone.polygon` is dereferenced unguarded.
            let Some(polygon) = zone.polygon.as_ref() else {
                return Err(session_npe(JavaNpe {
                    invoked: "java.util.List.size()",
                    receiver: "zone.polygon",
                }));
            };
            let zone_points: Vec<Point> = polygon
                .iter()
                .map(|corner| {
                    Point::Int(IntPoint::new(
                        java_round_to_int(corner.x * scale_factor),
                        java_round_to_int(-corner.y * scale_factor),
                    ))
                })
                .collect();
            // `:790` — `new PolygonShape(Point[])` reads `corners[0]`, so an empty polygon is an
            // `ArrayIndexOutOfBoundsException`, as at `readBoard:661`.
            if zone_points.is_empty() {
                return Err(DsnError::KicadSession(
                    "java.lang.ArrayIndexOutOfBoundsException: Index 0 out of bounds for length 0"
                        .to_string(),
                ));
            }
            // `:791-792`. The `as usize` is the same deliberate wrap `readBoard:662` documents:
            // it preserves Java's bits for a negative `layerIndex` rather than inventing a
            // different layer.
            #[allow(clippy::cast_sign_loss)] // deliberate: preserves Java's bits, see readBoard.
            board.insert_conduction_area(
                Area::Shape(Shape::Polygon(PolygonShape::from_points(&zone_points))),
                zone.layerIndex as usize,
                net_numbers,
                1,
                zone.isObstacle,
                FixedState::UserFixed,
            );
        }
    }

    // ── 2. Traces (`:796-814`) ──────────────────────────────────────────────────────────────
    if let Some(traces) = board_json.traces.as_ref() {
        for trace in traces {
            // `:799-801`.
            let net_number = java_nets_get(
                &board.rules.nets,
                &net_name_is_null,
                trace.netName.as_deref(),
                1,
            )
            .map_err(session_npe)?
            .unwrap_or(0);
            let net_numbers = if net_number > 0 {
                vec![net_number]
            } else {
                Vec::new()
            };
            // `:802`.
            let trace_half_width = java_round_to_int(trace.width * scale_factor / 2.0);

            // `:804-810` — `tr.points` is dereferenced unguarded.
            let Some(points) = trace.points.as_ref() else {
                return Err(session_npe(JavaNpe {
                    invoked: "java.util.List.size()",
                    receiver: "tr.points",
                }));
            };
            let trace_points: Vec<Point> = points
                .iter()
                .map(|point| {
                    Point::Int(IntPoint::new(
                        java_round_to_int(point.x * scale_factor),
                        java_round_to_int(-point.y * scale_factor),
                    ))
                })
                .collect();
            // `:811-812` — the same `BasicBoard.insertTrace(Point[], …)` overload `readBoard:680`
            // takes, so Convention 7's answer is the same one and for the same reason (see the
            // reader's section 11). `Math.max(layer, 0)` is Trace.java:45's first clamp,
            // reproduced at the call site because the port's parameter is a `usize`.
            board.insert_trace_at_points(
                &trace_points,
                trace.layerIndex.max(0) as usize,
                trace_half_width,
                net_numbers,
                1,
                FixedState::UserFixed,
            );
        }
    }

    // ── 3. Vias (`:816-854`) ────────────────────────────────────────────────────────────────
    //
    // `:818` — `int layerCount = board.getLayerCount()`, which Java reads **inside** the
    // `boardJson.vias != null` guard and this port hoists above section 1 (see its binding). The
    // move is behaviour-neutral: nothing between the two points can change the layer count —
    // sections 1-3 insert items and padstacks and never touch `LayerStructure` — and the read has
    // no side effect. It is hoisted because sections 1 and 2 would otherwise borrow `board`
    // mutably across it.
    if let Some(vias) = board_json.vias.as_ref() {
        for via in vias {
            // `:820-822`.
            let net_number = java_nets_get(
                &board.rules.nets,
                &net_name_is_null,
                via.netName.as_deref(),
                1,
            )
            .map_err(session_npe)?
            .unwrap_or(0);
            let net_numbers = if net_number > 0 {
                vec![net_number]
            } else {
                Vec::new()
            };

            // `:824-827` — `vj.position` is dereferenced unguarded.
            let Some(position) = via.position.as_ref() else {
                return Err(session_npe_field("x", "vj.position"));
            };
            let center = IntPoint::new(
                java_round_to_int(position.x * scale_factor),
                java_round_to_int(-position.y * scale_factor),
            );

            // `:829-837` — the same `new IntBox(round(-r), …).toSimplex()` shape section 8 and
            // `readBoard:694-703` build, so it goes through the same [`via_shape`]; the drill is
            // not in it (quirk #281).
            let mut shapes: Vec<Option<Shape>> = vec![None; layer_count];
            let shape = via_shape(via.diameter * scale_factor / 2.0);
            // `:839-843`. **Unlike `readBoard:705-707`, this loop bounds-checks**: an index
            // outside `[0, layerCount)` is skipped rather than thrown on. A via with
            // `startLayerIndex > endLayerIndex` therefore leaves every shape `null` and reaches
            // quirk #286 below; one with an out-of-range span silently loses those layers.
            //
            // **`wrapping_add(1)`, deliberately: this loop does not terminate for
            // `endLayerIndex == i32::MAX`, and neither does Java's.** `:839` is
            // `for (int li = vj.startLayerIndex; li <= vj.endLayerIndex; li++)`; at
            // `li == Integer.MAX_VALUE` the `li++` overflows to `Integer.MIN_VALUE`, which is
            // still `<= end`, so the jar spins forever writing nothing (every index outside
            // `[0, layerCount)` is skipped by `:840`'s guard). A plain `layer += 1` here would
            // **panic** in a debug build and spin in a release one — the two profiles disagreeing
            // with each other, which is worse than either. The wrap makes both profiles Java.
            //
            // Reproducing a Java non-termination is this port's established answer, not a new
            // choice: quirk #76 is a `PolylineTrace.normalize` ladder that hangs on the JVM and
            // that `Board::split_trace` hangs on identically. A `// totalized:` bound would be a
            // divergence, and would need a controller ruling of the kind plan ruling 7 gave quirk
            // #244. `readBoard:705-707`'s sibling loop cannot reach this: it returns a
            // `ParseError` at the first out-of-range index, so it never walks past `layerCount`.
            //
            // Unreachable from any writer — `KiCadJsonWriter` emits `endLayerIndex` from
            // `board.getLayerCount() - 1` downwards — so it takes a hand-edited
            // `"endLayerIndex": 2147483647`.
            let mut layer = via.startLayerIndex;
            while layer <= via.endLayerIndex {
                if layer >= 0 && (layer as i64) < layer_count as i64 {
                    shapes[layer as usize] = Some(shape.clone());
                }
                layer = layer.wrapping_add(1);
            }
            // `:844-847` — the same generated name `readBoard:708-711` builds, and the same two
            // `%.0f`s through [`java_format_fixed`] (quirk #288: Java's HALF_UP over the shortest
            // round-trip digits, where Rust's `{:.0}` is half-to-even).
            let via_padstack_name = format!(
                "Via[{}-{}]_{}:{}_um",
                via.startLayerIndex,
                via.endLayerIndex,
                java_format_fixed(via.diameter * 1000.0, 0),
                java_format_fixed(via.drill * 1000.0, 0)
            );
            // `:848-851`.
            let existing = board
                .library
                .padstacks
                .get_by_name(&via_padstack_name)
                .map(|padstack| PadstackId(padstack.no));
            let via_padstack = match existing {
                Some(id) => id,
                None => board
                    .library
                    .padstacks
                    .add(via_padstack_name, shapes, true, false),
            };
            // Quirk #286, as at `readBoard:716`: an all-`null` padstack makes
            // `DrillItem.tileShapeCount` negative and `new TileShape[-n]` throws
            // `NegativeArraySizeException`, whose `getMessage()` is the bare number. `fr_board`'s
            // own `tile_shape_count` panics instead, so the span is tested before inserting.
            if let Some(padstack) = board.library.padstacks.get(via_padstack) {
                let span = java_drill_item_tile_shape_count(padstack);
                if span < 0 {
                    return Err(DsnError::KicadSession(format!(
                        "java.lang.NegativeArraySizeException: {span}"
                    )));
                }
            }
            // `:852`. The **checked** seam, for the reason `readBoard:716` gives — and here the
            // argument is stronger, not weaker: `importSession` runs onto a board that already
            // carries wiring, so `splitTraces` really does walk pre-existing traces on the very
            // first via.
            let stop = || false;
            board
                .insert_via_checked(
                    via_padstack,
                    Point::Int(center),
                    net_numbers,
                    1,
                    FixedState::UserFixed,
                    true,
                    &stop,
                )
                .map_err(|error| {
                    // totalized: `:852`'s `board.insertVia` cannot fail in Java; the port's
                    // answers a `Result` because `split_traces` can surface a `Polyline`
                    // normalisation failure (quirk #109). Java would reach the caller's
                    // `catch (Exception)` for the same condition, which is where this goes.
                    DsnError::KicadSession(format!("java.lang.RuntimeException: {error}"))
                })?;
        }
    }
    Ok(())
}

/// [`JavaNpe`] as the [`DsnError`] `importSession`'s callers catch.
///
/// `readBoard` turns the same value into a `ParseError` because its own `catch (Throwable)` at
/// `:746` does; `importSession` has no catch of its own, so the throwable reaches the caller and
/// what it prints is `Throwable.toString()`.
fn session_npe(error: JavaNpe) -> DsnError {
    DsnError::KicadSession(format!(
        "java.lang.NullPointerException: Cannot invoke \"{}\" because \"{}\" is null",
        error.invoked, error.receiver
    ))
}

/// [`session_npe`] for a **field read** rather than a method invocation: the JVM's helpful
/// message is `Cannot read field "x"`, not `Cannot invoke …`.
///
/// `readBoard`'s pair is [`npe`]/[`npe_field`]; this is the same distinction on the
/// `importSession` side. Measured, stem `via-null-position`.
fn session_npe_field(field: &str, receiver: &str) -> DsnError {
    DsnError::KicadSession(format!(
        "java.lang.NullPointerException: Cannot read field \"{field}\" because \"{receiver}\" is null"
    ))
}

/// Java's `(int)` narrowing cast on a `double` (JLS 5.1.3): truncate toward zero, saturate at
/// `Integer.MIN_VALUE`/`MAX_VALUE`, and answer `0` for `NaN`.
///
/// `as i32` in Rust already saturates and already maps `NaN` to `0`, so this is a named wrapper
/// rather than a reimplementation — named because `importSession:770` is the only place in this
/// module that performs the cast and the reader of that line should not have to know that Rust's
/// float-to-int cast happens to agree with Java's.
#[allow(clippy::cast_possible_truncation)] // the saturation *is* the Java semantics.
fn java_double_to_int(value: f64) -> i32 {
    value as i32
}

// ===================================================================== the private helpers
//
// Task 8 writes the body of every helper sections 1-8 call, and therefore owns them (Plan 7 scan
// ruling 7 — the earliest task that writes the body owns it):
//
//   `findKiCadDefaultNetClass`   :926-934   section 4 (:141) and section 8 (:344, :371)
//   `nonDefaultNetClasses`       :936-945   section 4 (:124)
//   `applyKiCadNetClassParameters` :947-962 section 8 (:345, :354)
//   `resolveNetClassIndex`       :964-978   section 8 (:449)
//   `PointOutline.addPoint`      :985-987   section 5 (:275, :304)
//   `PointOutline.boundingBox`   :990-1009  section 5 (:278, :307)
//
// Task 9 owns the remaining two, `getDescriptivePadstackName` (:857-890) and
// `arePackagePinsIdentical` (:892-924), which only section 9 calls.

/// Port of `KiCadJsonReader.findKiCadDefaultNetClass` (KiCadJsonReader.java:926-934).
///
/// A `null` name is `false` (`KiCadNetClassNames.isKiCadDefaultNetClassName`, :25-30), so a
/// nameless class is never the default one.
fn find_kicad_default_net_class(net_classes: &[NetClassJson]) -> Option<&NetClassJson> {
    net_classes.iter().find(|net_class| {
        net_class
            .name
            .as_deref()
            .is_some_and(is_kicad_default_net_class_name)
    })
}

/// Port of `KiCadJsonReader.nonDefaultNetClasses` (KiCadJsonReader.java:936-945): every class
/// whose name is not one of KiCad's two spellings of "default", in declaration order.
fn non_default_net_classes(net_classes: &[NetClassJson]) -> Vec<&NetClassJson> {
    net_classes
        .iter()
        .filter(|net_class| {
            !net_class
                .name
                .as_deref()
                .is_some_and(is_kicad_default_net_class_name)
        })
        .collect()
}

/// Port of `KiCadJsonReader.applyKiCadNetClassParameters` (KiCadJsonReader.java:947-962).
///
/// Java takes the `NetClass` object; the port takes the rules plus the class's id, because
/// `fr_board::NetClass` lives inside `BoardRules::net_classes`.
fn apply_kicad_net_class_parameters(
    rules: &mut BoardRules,
    target: NetClassId,
    source: &NetClassJson,
    layer_count: usize,
    scale_factor: f64,
    clearance_class_index: usize,
) {
    // `:953-958`. Note the per-layer loop rather than `setTraceHalfWidth(int)`: identical here,
    // because it covers every layer.
    if source.traceWidth > 0.0 {
        let trace_half_width = java_round_to_int(source.traceWidth * scale_factor / 2.0);
        for layer in 0..layer_count {
            rules
                .net_classes
                .get_mut(target)
                .set_trace_half_width(layer, trace_half_width);
        }
    }
    // `:959-961`.
    if source.clearance > 0.0 {
        rules
            .net_classes
            .get_mut(target)
            .set_trace_clearance_class(clearance_class_index);
    }
}

/// Port of `KiCadJsonReader.resolveNetClassIndex` (KiCadJsonReader.java:964-978): the clearance
/// class index a `NetJson.className` names, defaulting to 1.
///
/// The `equalsIgnoreCase` fallback at `:972-976` walks `HashMap.entrySet()`, so when two classes
/// differ only in case the answer depends on `java.util.HashMap`'s bucket order — see
/// [`JavaStringMap`].
///
/// # Its relationship to section 8's `clNo - 1`
///
/// What this returns is a **clearance** class index, not a net-class index. Section 8's one call
/// site (`:449-451`) immediately subtracts one — `boardRules.netClasses.get(clNo - 1)`, with
/// Java's own `// NetClass array indices are 0-based` comment — because the clearance-class list
/// carries two reserved rows (`"null"` at 0 and `"default"` at 1, `:126-133`) where the net-class
/// list carries only `"default"` at 0. So this function's `1` default and its `:970` map values
/// are both one greater than the `NetClassId` the caller wants, and the two lines only agree
/// because `netClassIndexMap` was filled at `:345`'s `i + 2` from the same offset. Change either
/// and the other must change with it; the caller's comment says the same thing from its side.
fn resolve_net_class_index(map: &JavaStringMap, class_name: Option<&str>) -> usize {
    // `:965-967`. A `null` class name is not a default-class name, so it falls through.
    if class_name.is_some_and(is_kicad_default_net_class_name) {
        return 1;
    }
    // `:968-971` — the exact-match `HashMap.get`.
    if let Some(class_index) = class_name.and_then(|name| map.get(name)) {
        return class_index;
    }
    // `:972-976`.
    if let Some(name) = class_name {
        for (key, value) in map.entry_set() {
            if equals_ignore_case(key, name) {
                return value;
            }
        }
    }
    // `:977`.
    1
}

/// Port of `KiCadJsonReader.PointOutline` (KiCadJsonReader.java:980-1010): "helper class to trace
/// bounding outer box".
struct PointOutline {
    /// `PointOutline.points` (:982).
    points: Vec<FloatPoint>,
}

impl PointOutline {
    fn new() -> PointOutline {
        PointOutline { points: Vec::new() }
    }

    /// Port of `PointOutline.addPoint` (KiCadJsonReader.java:985-987).
    fn add_point(&mut self, point: FloatPoint) {
        self.points.push(point);
    }

    /// Port of `PointOutline.boundingBox` (KiCadJsonReader.java:990-1009): "the integer bounding
    /// box of the outline points".
    fn bounding_box(&self) -> IntBox {
        // `:991-993`. Unreachable from `readBoard` — both section-5 branches add at least three
        // points before calling this — but ported because it is two lines and is the class's
        // whole contract.
        if self.points.is_empty() {
            return IntBox::EMPTY;
        }
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut max_x = -f64::MAX;
        let mut max_y = -f64::MAX;
        for point in &self.points {
            min_x = java_min(min_x, point.x);
            min_y = java_min(min_y, point.y);
            max_x = java_max(max_x, point.x);
            max_y = java_max(max_y, point.y);
        }
        IntBox::from_coords(
            java_round_to_int(min_x),
            java_round_to_int(min_y),
            java_round_to_int(max_x),
            java_round_to_int(max_y),
        )
    }
}

/// Port of `KiCadJsonReader.getDescriptivePadstackName` (KiCadJsonReader.java:857-890).
///
/// **A name-generating function whose output reaches the SES**: the padstack names this composes
/// are what `Padstack.name` carries into `SesWriter`/`DsnWriter`, so the string building is
/// transcribed exactly rather than paraphrased.
///
/// Three things about it are worth knowing before editing:
///
/// * `"Round".equals(shapeStr)` at `:882` is **case-sensitive** and compares against the value
///   this function itself computed, so it selects the one-number `Pad_<x>_um` form for exactly the
///   shapes `:861-862` mapped to the literal `"Round"` — `"circle"` and `"round"`, in any case —
///   and for a `null` shape, which never enters the `if` at all. A shape spelled `"rounded"`
///   falls through to `:868`, becomes `"Rounded"`, and takes the two-number form.
/// * `:885-888` formats `"%s[%s]Pad_%.0fxf_%.0f_um"` and then `.replace("xf_", "x")`. The
///   round-trip through the `xf_` marker is Java's; it is reproduced verbatim because
///   `String.replace(CharSequence, CharSequence)` replaces **every** occurrence, so a shape name
///   containing `xf_` would be rewritten too.
/// * both `%.0f`s go through [`java_format_fixed`], not Rust's `{:.0}`: `java.util.Formatter`
///   rounds the shortest round-trip digits **HALF_UP** where Rust rounds half-to-even, so a pad
///   `0.0025 mm` wide is `3` to Java and `2` to Rust. Measured, stem `pad-name-half-up`.
///
/// Quirk #288: `String.format` here is the **one-argument** form, which resolves
/// `Locale.getDefault(Locale.Category.FORMAT)`. On an `ar_EG` JVM this returns
/// `Round[A]Pad_١٠٠٠_um`. The port always writes ASCII digits, which is plan-5 ruling 6's stance
/// for the same hazard in the DRC report (quirk #145).
///
/// `Err` is `Throwable.getMessage()` — the text `:746`'s `catch` would have wrapped — rather than
/// a whole [`BoardReadResult`], so the `Result` stays small; the one call site turns it into the
/// same `ParseError` every other throw site here returns.
///
/// It carries the `NullPointerException` at `:875`/`:877` over a `null` board-layer name. That is
/// **not reachable** from `readBoard`: a `pad.layers` of size 1 is non-empty, so `:539-553` has
/// already walked every board layer's name and thrown there — measured, stem
/// `layer-null-name-with-pads`, whose `ParseError` names `boardLayers[li].name`. The arm is
/// transcribed anyway because the function is 35 lines of contract.
fn get_descriptive_padstack_name(
    pad: &PadJson,
    board_layer_names: &[Option<String>],
    layer_count: usize,
) -> Result<String, String> {
    // `:859-870`.
    let mut shape_str = "Round";
    let mut fall_through: String;
    if let Some(shape) = pad.shape.as_deref() {
        if equals_ignore_case("circle", shape) || equals_ignore_case("round", shape) {
            shape_str = "Round";
        } else if equals_ignore_case("rect", shape) || equals_ignore_case("rectangle", shape) {
            shape_str = "Rect";
        } else if equals_ignore_case("oval", shape) {
            shape_str = "Oval";
        } else {
            // `:868` — `shape.substring(0, 1).toUpperCase() + shape.substring(1).toLowerCase()`.
            //
            // An **empty** shape name makes `substring(0, 1)` a
            // `StringIndexOutOfBoundsException`, which `:746`'s `catch (Throwable)` reports as a
            // parse error about the file. Measured, stem `pad-shape-empty`.
            let mut chars = shape.chars();
            let Some(first) = chars.next() else {
                return Err("Range [0, 1) out of bounds for length 0".to_string());
            };
            // totalized: Java's `substring` counts **UTF-16 code units** and `String.toUpperCase`
            // / `toLowerCase` are the locale-sensitive, possibly-expanding full mappings; this
            // takes the first `char` and uses `Character.toUpperCase`/`toLowerCase`
            // ([`java_to_upper`] / [`java_to_lower`], the simple per-character mappings the rest
            // of this port already uses). The two differ only for a shape name whose first
            // character is outside the BMP (Java would split a surrogate pair) or whose case
            // mapping expands or is locale-dependent — the Turkish dotted `i` being the classic
            // one, and the same quirk #288 hazard as the digits.
            fall_through = String::new();
            fall_through.push(java_to_upper(first));
            for c in chars {
                fall_through.push(java_to_lower(c));
            }
            shape_str = &fall_through;
        }
    }

    // `:872-880`.
    let mut layer_type = "A";
    if let Some(pad_layers) = pad.layers.as_ref()
        && pad_layers.len() == 1
    {
        let layer_name = pad_layers[0].as_deref();
        // `:875`.
        let Some(first_layer_name) = board_layer_names[0].as_deref() else {
            return Err(npe_message(
                "String.equalsIgnoreCase(String)",
                "boardLayers[0].name",
            ));
        };
        if layer_name.is_some_and(|name| equals_ignore_case(first_layer_name, name)) {
            layer_type = "T";
        } else {
            // `:877`.
            let Some(last_layer_name) = board_layer_names[layer_count - 1].as_deref() else {
                return Err(npe_message(
                    "String.equalsIgnoreCase(String)",
                    "boardLayers[layerCount - 1].name",
                ));
            };
            if layer_name.is_some_and(|name| equals_ignore_case(last_layer_name, name)) {
                layer_type = "B";
            }
        }
    }

    // `:882-889`. `pad.size` was dereferenced at `:509`, in the same loop iteration, so it is
    // non-null by the time this runs.
    let size = pad
        .size
        .as_ref()
        .expect("KiCadJsonReader.java:509 dereferenced pad.size before calling this");
    if shape_str == "Round" {
        // `:883`.
        Ok(format!(
            "{shape_str}[{layer_type}]Pad_{}_um",
            java_format_fixed(size.x * 1000.0, 0)
        ))
    } else {
        // `:885-888`.
        Ok(format!(
            "{shape_str}[{layer_type}]Pad_{}xf_{}_um",
            java_format_fixed(size.x * 1000.0, 0),
            java_format_fixed(size.y * 1000.0, 0)
        )
        .replace("xf_", "x"))
    }
}

/// Port of `KiCadJsonReader.arePackagePinsIdentical` (KiCadJsonReader.java:892-924): **the
/// function that decides package reuse**, so getting it wrong either duplicates or merges library
/// packages — visible in the item count and in the SES. Transcribed pin by pin.
///
/// # The `Err` arm is the throw `:603` catches
///
/// `:908` is `pin1.name.equals(pin2.name)`, and `pin1` is a pin of the **existing** package. Java
/// stores `pad.name` there verbatim, `null` included, so a pad with no `name` key makes this
/// `NullPointerException` — the one exception the `catch (Exception e)` at `:603` ever sees, and
/// therefore the whole reason quirk #285's duplicate packages exist. `fr_board::PackagePin::name`
/// is a `String`, so the nullability travels beside the pins in `pkg1_names` / `p2_names`.
///
/// Java's `pin2.name` may be `null` too, and `String.equals(null)` is simply `false`; that is why
/// the comparison below is over `Option<&str>` rather than over the totalized `""`.
///
/// not reachable: `:893-895`'s `pkg1 == null || p2 == null` guard — the one call site tested
/// `existingPkg != null` at `:584` and built `p2` two lines earlier, and neither of the port's
/// parameters can be null anyway.
///
/// not reachable: `:902-907`'s `pin1 == null || pin2 == null` arm — `Package.getPin` answers
/// `null` only out of range, and `:896`'s length test has already made that impossible.
fn are_package_pins_identical(
    pkg1: &Package,
    pkg1_names: &[Option<String>],
    p2: &[PackagePin],
    p2_names: &[Option<String>],
) -> Result<bool, JavaNpe> {
    // `:896-898`.
    if pkg1.pin_count() != p2.len() {
        return Ok(false);
    }
    for i in 0..p2.len() {
        // `:900-901`.
        let pin1 = pkg1
            .get_pin(i32::try_from(i).unwrap_or(i32::MAX))
            .expect("i < pin_count(), checked above");
        let pin2 = &p2[i];
        // `:908-910`.
        let Some(name1) = pkg1_names[i].as_deref() else {
            return Err(JavaNpe {
                invoked: "String.equals(Object)",
                receiver: "pin1.name",
            });
        };
        if p2_names[i].as_deref() != Some(name1) {
            return Ok(false);
        }
        // `:911-913`.
        if pin1.padstack_no != pin2.padstack_no {
            return Ok(false);
        }
        // `:914-918`. `Math.abs` on a NaN difference answers NaN, and `NaN > 0.001` is false in
        // both languages, so a NaN coordinate reports "identical" on both sides.
        let loc1 = pin1.relative_location.to_float();
        let loc2 = pin2.relative_location.to_float();
        if (loc1.x - loc2.x).abs() > 0.001 || (loc1.y - loc2.y).abs() > 0.001 {
            return Ok(false);
        }
        // `:919-921`.
        if (pin1.rotation_in_degree - pin2.rotation_in_degree).abs() > 0.001 {
            return Ok(false);
        }
    }
    // `:923`.
    Ok(true)
}

/// A `java.lang.NullPointerException` as a value: the two facts the JVM's helpful message is
/// composed from. See [`npe`], which turns one into the `ParseError` `:746` would have answered.
#[derive(Debug, Clone, Copy)]
struct JavaNpe {
    /// The method the throwing expression was about to invoke.
    invoked: &'static str,
    /// The source expression that was `null`.
    receiver: &'static str,
}

/// Port of `Nets.get(String, int)` (Nets.java:42-52), **including the `NullPointerException`
/// quirk #282 predicted**.
///
/// Java's loop is `currentNet != null && currentNet.name.equalsIgnoreCase(name)`, so it
/// dereferences the *stored* name of every net it walks past — and stops at the first match. A net
/// whose JSON `name` was `null` therefore kills the first lookup that reaches it, and only if it
/// is reached: a match earlier in the list returns before ever touching it.
///
/// `fr_board::Net::name` is a `String` where Java's is nullable, so `name_is_null` carries the bit
/// section 8's totalization dropped, one entry per net in net-number order.
///
/// A `null` **argument** is not a throw: `String.equalsIgnoreCase(null)` is `false`. That is why
/// `name` is an `Option<&str>` and a `None` simply never matches — which is what makes a pad with
/// no `netName` land on net number 0 rather than on a net the port stored as `""`.
///
/// `Ok(None)` is Java's `return null`; the answer is the net **number**, because every caller
/// immediately reads `targetNet.netNumber`.
fn java_nets_get(
    nets: &Nets,
    name_is_null: &[bool],
    name: Option<&str>,
    subnet_number: i32,
) -> Result<Option<i32>, JavaNpe> {
    for (index, current_net) in nets.iter().enumerate() {
        if name_is_null.get(index).copied().unwrap_or(false) {
            return Err(JavaNpe {
                invoked: "String.equalsIgnoreCase(String)",
                receiver: "currentNet.name",
            });
        }
        if name.is_some_and(|name| equals_ignore_case(&current_net.name, name))
            && current_net.subnet_number == subnet_number
        {
            return Ok(Some(current_net.net_number));
        }
    }
    Ok(None)
}

/// Port of `DrillItem.tileShapeCount` (DrillItem.java:202-208): `toLayer - fromLayer + 1`, read
/// straight off the padstack.
///
/// **It can be negative**, and that is quirk #286. `Padstack.fromLayer` answers `shapes.length`
/// and `Padstack.toLayer` answers `-1` when every layer's shape is `null` (Padstack.java:137-152),
/// so an all-null padstack makes this `-layerCount`. Java then allocates `new TileShape[count]`
/// from inside `insertPin`/`insertVia`'s search-tree update and throws
/// `NegativeArraySizeException`, whose `getMessage()` is the bare number — which `readBoard`'s
/// `catch (Throwable)` reports as `Exception occurred: -2` on a two-layer board.
///
/// Two inputs reach it, both measured: a via with `startLayerIndex > endLayerIndex`
/// (`via-start-gt-end`) and a pad whose `layers` list matches no board layer and whose padstack
/// name is not already taken (`pad-layers-unmatched-only`).
///
/// This is the **reported finding** the task's "no `catch_unwind` unless you find a panic Java's
/// `Exception` arm would have caught" rule asks for: `fr_board`'s own `tile_shape_count` panics
/// here (`crates/fr-board/src/items/drill.rs`'s `layer_index`, which cannot express a negative
/// layer in a `usize`), so the reader tests the span itself before inserting rather than letting
/// a panic stand in for Java's throw.
fn java_drill_item_tile_shape_count(padstack: &Padstack) -> i32 {
    padstack.to_layer() - padstack.from_layer() + 1
}

// ============================================================ the Java primitives this file needs

/// `BoardReadResult.ParseError` (`:65`, `:80`, `:748-749`).
fn parse_error(location: &str, detail: &str) -> BoardReadResult {
    BoardReadResult::ParseError {
        location: location.to_string(),
        detail: detail.to_string(),
    }
}

/// The `catch (Throwable)` arm (`:746-750`) reached by one of the **five** unguarded list
/// dereferences sections 1-8 make: `boardJson.layers` (`:105`), `boardJson.netClasses` (`:124`,
/// dereferenced inside `nonDefaultNetClasses` at `:940`), `boardJson.clearanceRules` (`:156`),
/// `boardJson.outline.corners` (`:169`) and `boardJson.nets` (`:446`). Sections 9-11 add **five**
/// more of the same `iterator()` shape — `boardJson.components` (`:503`), `comp.pads` (`:506`),
/// `boardJson.conductionAreas` (`:648`), `boardJson.traces` (`:667`) and `boardJson.vias`
/// (`:684`) — so ten in the whole method, plus the two `List.size()` invokes sections 10 and 11
/// make on `zone.polygon` (`:654`) and `tr.points` (`:673`), which throw the same way with a
/// different verb.
///
/// Quirk #277: Java's `detail` carries the JVM's **helpful** `NullPointerException` message
/// (`Cannot invoke "java.util.List.isEmpty()" because "boardJson.layers" is null`) — a string the
/// JIT composes from the bytecode, which no port can reconstruct. The port writes the same shape
/// from the same two facts, so the row is greppable both ways, and
/// `crates/fr-dsn/tests/data/p8t8-kicad-read-a.txt` records Java's exact text beside it.
fn npe(invoked: &str, receiver: &str) -> BoardReadResult {
    parse_error(
        "json_payload",
        &format!("Exception occurred: {}", npe_message(invoked, receiver)),
    )
}

/// [`npe`]'s message half — `NullPointerException.getMessage()` — for the one helper that answers
/// Java's throw text rather than a whole [`BoardReadResult`].
fn npe_message(invoked: &str, receiver: &str) -> String {
    format!("Cannot invoke \"{invoked}\" because \"{receiver}\" is null")
}

/// [`npe`]'s sibling for a **field read** rather than a method call: the JVM's other helpful
/// message shape, `Cannot read field "x" because "pad.size" is null`.
///
/// Sections 9-11 reach it four times, all on a `Point2D` whose Java field initializer is
/// `new Point2D()` and which therefore only goes `null` for an explicit JSON `null`: `pad.size`
/// (`:509`), `pad.offset` (`:567`), `comp.position` (`:622`) and `vj.position` (`:691`).
fn npe_field(field: &str, receiver: &str) -> BoardReadResult {
    parse_error(
        "json_payload",
        &format!(
            "Exception occurred: Cannot read field \"{field}\" because \"{receiver}\" is null"
        ),
    )
}

/// `new IntBox(round(-r), round(-r), round(r), round(r)).toSimplex()` (`:382-388`, `:418-424`).
///
/// The two roundings are computed **separately**, as Java writes them, because `Math.round` is
/// half-**up** and therefore not symmetric about zero: `round(0.5)` is `1` while `round(-0.5)` is
/// `0`, so `-round(r)` is not `round(-r)` at a half-integer radius. A radius lands on one exactly
/// when `viaDiameter * scaleFactor` is an odd integer — `0.0001 mm` at the default `10000`
/// resolution, say — which no corpus fixture does, but the fixture corpus is not the input space.
/// `the_via_shape_rounds_each_corner_separately` pins it.
fn via_shape(radius: f64) -> Shape {
    let lower = java_round_to_int(-radius);
    let upper = java_round_to_int(radius);
    Shape::Tile(fr_geometry::TileShape::Simplex(
        IntBox::from_coords(lower, lower, upper, upper).to_simplex(),
    ))
}

/// The copy of `name` that [`fr_board::ViaInfos::add`] just stamped with an identity serial.
///
/// Java's `viaRule.appendVia(viaInfo)` and `boardRules.viaInfos.add(viaInfo)` bind the **same**
/// object; Plan 7 Task 0 made the Rust `ViaRule` own its infos, so the rule must be given the
/// registered copy rather than the pre-registration value — otherwise `ViaInfo::is_same_object`
/// (Java's `==`, read by `ViaRule::contains`) would answer `false` where Java answers `true`.
fn registered_via_info(rules: &BoardRules, name: &str) -> ViaInfo {
    rules
        .via_infos
        .get_by_name(name)
        .cloned()
        .expect("ViaInfos::add was called with this name immediately above")
}

/// Java `Math.min(double, double)`, transcribed whole — **both** the ways it differs from Rust's
/// `f64::min`:
///
/// * **NaN propagates.** Rust's `f64::min` returns the non-NaN operand; Java's returns NaN.
/// * **Signed zero is ordered.** `Math.min(-0.0, 0.0)` is `-0.0` and `Math.max(-0.0, 0.0)` is
///   `0.0`; `a <= b` alone cannot tell the two zeros apart, so Java tests the sign bit.
///
/// Neither difference is observable through this port today, and both halves are transcribed
/// anyway rather than caveated, because the caveat is longer than the code. The NaN half is
/// unreachable by construction: a NaN coordinate needs Gson's LENIENT bare `NaN` token, which
/// `serde_json` rejects before section 2 (quirk #277). The signed-zero half is *reachable* — JSON
/// has a `-0.0` literal — but not observable, and these are the consumers that make it so: section
/// 5's `min_x`/`max_x`/`min_y`/`max_y` accumulators and [`PointOutline::bounding_box`]'s four, all
/// eight of which end in [`java_round_to_int`], for which `-0.0` and `0.0` are both `0`.
fn java_min(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && b.is_sign_negative() {
        return b;
    }
    if a <= b { a } else { b }
}

/// Java `Math.max(double, double)`; see [`java_min`]. Note the asymmetry against it: `min` tests
/// **`b`**'s sign bit and `max` tests **`a`**'s.
fn java_max(a: f64, b: f64) -> f64 {
    if a.is_nan() {
        return a;
    }
    if a == 0.0 && b == 0.0 && a.is_sign_negative() {
        return b;
    }
    if a >= b { a } else { b }
}

/// Java `Double.compare(double, double)`, the comparator `:293-296` sorts the outline corners
/// with: a **total** order, where `-0.0 < 0.0` and NaN sorts above every number and equals
/// itself. Rust's `f64::partial_cmp` answers `None` for NaN and cannot be used here.
fn java_double_compare(a: f64, b: f64) -> Ordering {
    if a < b {
        Ordering::Less
    } else if a > b {
        Ordering::Greater
    } else {
        // `Double.compare`'s tail: `Long.compare(doubleToLongBits(a), doubleToLongBits(b))`, with
        // every NaN collapsed to the canonical bit pattern first.
        let bits = |x: f64| {
            if x.is_nan() {
                0x7ff8_0000_0000_0000_u64 as i64
            } else {
                x.to_bits() as i64
            }
        };
        bits(a).cmp(&bits(b))
    }
}

/// Java `String.isBlank()` (`:315`, `:317`): empty, or every code point is
/// `Character.isWhitespace`.
///
/// That predicate is **not** Rust's `char::is_whitespace`: Java excludes the three non-breaking
/// spaces (U+00A0, U+2007, U+202F) and U+0085 NEL, and includes the four ASCII information
/// separators U+001C..U+001F, which Rust's does not.
fn java_is_blank(text: &str) -> bool {
    text.chars().all(java_is_whitespace)
}

/// `Character.isWhitespace(char)` — see [`java_is_blank`].
fn java_is_whitespace(c: char) -> bool {
    matches!(c, '\u{1c}'..='\u{1f}')
        || (c.is_whitespace() && !matches!(c, '\u{a0}' | '\u{85}' | '\u{2007}' | '\u{202f}'))
}

/// `String.hashCode()` (`h = 31 * h + c` over the **UTF-16 code units**, wrapping as an `int`).
fn java_string_hash(text: &str) -> i32 {
    let mut hash: i32 = 0;
    for unit in text.encode_utf16() {
        hash = hash.wrapping_mul(31).wrapping_add(i32::from(unit));
    }
    hash
}

/// The order `java.util.HashMap` (and therefore `java.util.HashSet`) hands `String` keys back,
/// given them in insertion order and with no duplicates.
///
/// **Quirk #280.** `readBoard:490` iterates a `HashSet<String>` and numbers the nets it creates in
/// that order, so this is a parity surface, not an implementation detail. The reconstruction is
/// exact for the shape `readBoard` builds, and rests on three facts about `HashMap`:
///
/// * the table starts at capacity 16 / threshold 12 and doubles whenever `++size > threshold`,
///   so the final capacity is a function of the element count alone;
/// * `hash(key) = h ^ (h >>> 16)` and `index = (capacity - 1) & hash`;
/// * `resize()` splits each bucket into a lo/hi list **preserving relative order**, and `putVal`
///   appends at the tail, so within a bucket the final order is insertion order — the intermediate
///   capacities never have to be replayed.
///
/// # The one case it does not reproduce
///
/// A bucket that reaches 8 entries **while the table is at capacity >= 64** is converted to a
/// red-black tree (`TREEIFY_THRESHOLD`/`MIN_TREEIFY_CAPACITY`), and its iteration order becomes
/// the tree's. Below capacity 64 Java resizes instead, which this function already models. The
/// `debug_assert!` below names the condition, conservatively: `HashMap.putVal` treeifies only when
/// a bin that already holds 8 takes a 9th.
///
/// Peak bucket over the `referencedNets` set of every corpus fixture, measured with this same
/// model — which `the_hash_iteration_order_matches_the_jvm` validates against the JVM:
///
/// | fixture | referenced nets | final capacity | peak bucket |
/// |---|---|---|---|
/// | `Issue733-kicad_interf_u_input_design.json` | 173 | 256 | **4** |
/// | `Issue733-kicad_complex_hierarchy_input_design.json` | 52 | 128 | 3 |
/// | `Issue733-kicad_complex_hierarchy_output_session.json` | 48 | 64 | 3 |
/// | `Issue649-kicad_ecc83-pp_input_board_v{1,2}.json` | 13 | 32 | 2 |
/// | `Issue368-CorneyIslandWireless_input_design.json` | 5 | 16 | 1 |
///
/// So the worst case the corpus reaches is **4 of the 9** an actual treeify needs — a two-step
/// margin, not a hair's breadth, but not the "peak of 2" an earlier revision of this comment
/// claimed either. The other consumer, `netClassIndexMap.entrySet()` (`:972`), holds one entry per
/// non-default net class and cannot plausibly reach capacity 64 at all.
///
/// # Sections 9-11 add no consumer (Plan 8 Task 9's check of Task 8's concern 3)
///
/// Task 8 left open whether a hash-ordered iteration on a large board could reach
/// `java.util.HashMap`'s treeification threshold, which this does not model.
/// `awk 'NR>=498 && NR<=755' KiCadJsonReader.java | grep -n 'Hash\|entrySet\|keySet\|Map<\|Set<'`
/// finds **nothing**: sections 9-11 reach `Padstacks` and `Packages`, both `java.util.Vector`
/// (Padstacks.java:16, Packages.java:14), `Components`, whose `UndoableObjects` is a
/// `ConcurrentSkipListMap` ordered by `Component.compareTo` rather than by any hash, and
/// `Nets.get`, a linear scan. So the consumer set is still exactly Task 8's two — `:456`'s
/// `referencedNets` and `:972`'s `netClassIndexMap` — and the bucket table below is unchanged.
/// The `debug_assert!` runs under every test build, including the 67-input part-B replay and the
/// seven whole-fixture round trips, and has never fired.
fn java_hash_iteration_order(keys: &[String]) -> Vec<usize> {
    let mut capacity = 16_usize;
    let mut threshold = 12_usize;
    for size in 1..=keys.len() {
        if size > threshold {
            capacity *= 2;
            threshold *= 2;
        }
    }
    let mut buckets: Vec<Vec<usize>> = vec![Vec::new(); capacity];
    for (index, key) in keys.iter().enumerate() {
        let hash = java_string_hash(key) as u32;
        let spread = hash ^ (hash >> 16);
        buckets[(capacity - 1) & (spread as usize)].push(index);
    }
    // `debug_assert!` and not `assert!`: this is a *diagnostic* for a case the corpus does not
    // reach, on a function every KiCad read runs — so it is **silent in release**, which is what
    // the CLI ships, and a board that did treeify would diverge quietly. Left as a debug assert
    // because a wrong net *numbering* is caught downstream by any DSN/SES parity run; recorded
    // here so Task 9 and Task 14's `io/` sweep can revisit the choice rather than rediscover it.
    debug_assert!(
        capacity < 64 || buckets.iter().all(|bucket| bucket.len() < 8),
        "java_hash_iteration_order: a bucket reached TREEIFY_THRESHOLD at capacity {capacity}, \
         where java.util.HashMap switches to a red-black tree this function does not model"
    );
    buckets.into_iter().flatten().collect()
}

/// `java.util.HashSet<String>` — `readBoard:456`'s `referencedNets`, reduced to the two
/// operations that method performs: `add` and the enhanced-for at `:490`.
struct JavaStringSet {
    /// The distinct keys, in insertion order.
    keys: Vec<String>,
}

impl JavaStringSet {
    fn new() -> JavaStringSet {
        JavaStringSet { keys: Vec::new() }
    }

    /// `HashSet.add` — a no-op when the key is already present (**exact** equality, not
    /// `equalsIgnoreCase`).
    fn add(&mut self, key: &str) {
        if !self.keys.iter().any(|existing| existing == key) {
            self.keys.push(key.to_string());
        }
    }

    /// The order the enhanced-for at `:490` sees — see [`java_hash_iteration_order`].
    fn iteration_order(&self) -> Vec<&str> {
        java_hash_iteration_order(&self.keys)
            .into_iter()
            .map(|index| self.keys[index].as_str())
            .collect()
    }
}

/// `java.util.HashMap<String, Integer>` — `readBoard:343`'s `netClassIndexMap`, reduced to the
/// three operations it needs: `put` (`:355`), `get` (`:968`) and `entrySet` (`:972`).
struct JavaStringMap {
    /// The distinct keys with their current values, in **first-insertion** order — `put` on an
    /// existing key replaces the value and leaves the entry where it is, which is what
    /// `HashMap.putVal` does.
    entries: Vec<(String, usize)>,
}

impl JavaStringMap {
    fn new() -> JavaStringMap {
        JavaStringMap {
            entries: Vec::new(),
        }
    }

    fn put(&mut self, key: String, value: usize) {
        if let Some(entry) = self.entries.iter_mut().find(|(name, _)| *name == key) {
            entry.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    fn get(&self, key: &str) -> Option<usize> {
        self.entries
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| *value)
    }

    /// The order `entrySet()`'s iterator hands the entries back — see
    /// [`java_hash_iteration_order`].
    fn entry_set(&self) -> Vec<(&str, usize)> {
        let keys: Vec<String> = self.entries.iter().map(|(name, _)| name.clone()).collect();
        java_hash_iteration_order(&keys)
            .into_iter()
            .map(|index| {
                let (name, value) = &self.entries[index];
                (name.as_str(), *value)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn java_is_blank_matches_character_is_whitespace() {
        assert!(java_is_blank(""));
        assert!(java_is_blank("  \t\r\n\u{b}\u{c}"));
        assert!(java_is_blank("\u{1c}\u{1d}\u{1e}\u{1f}"));
        // The three non-breaking spaces and NEL are whitespace to Rust and not to Java.
        assert!(!java_is_blank("\u{a0}"));
        assert!(!java_is_blank("\u{85}"));
        assert!(!java_is_blank("\u{2007}"));
        assert!(!java_is_blank("\u{202f}"));
        assert!(!java_is_blank("KiCad"));
    }

    #[test]
    fn java_string_hash_matches_the_jvm() {
        // Measured on JDK 25: `java -e 'for (String s : …) System.out.println(s.hashCode());'`.
        assert_eq!(java_string_hash(""), 0);
        assert_eq!(java_string_hash("GND"), 70717);
        assert_eq!(java_string_hash("default"), 1_544_803_905);
        assert_eq!(java_string_hash("kicad_default"), -800_824_662);
        // A non-BMP code point folds as **two** UTF-16 units, not one `char`.
        assert_eq!(java_string_hash("\u{1F600}"), 1_772_899);
    }

    #[test]
    fn java_double_compare_is_a_total_order() {
        assert_eq!(java_double_compare(1.0, 2.0), Ordering::Less);
        assert_eq!(java_double_compare(2.0, 1.0), Ordering::Greater);
        assert_eq!(java_double_compare(1.0, 1.0), Ordering::Equal);
        assert_eq!(java_double_compare(-0.0, 0.0), Ordering::Less);
        assert_eq!(
            java_double_compare(f64::NAN, f64::INFINITY),
            Ordering::Greater
        );
        assert_eq!(java_double_compare(f64::NAN, f64::NAN), Ordering::Equal);
    }

    #[test]
    fn java_min_and_max_propagate_nan_where_rusts_do_not() {
        assert!(java_min(f64::NAN, 1.0).is_nan());
        assert!(java_max(f64::NAN, 1.0).is_nan());
        assert!(java_min(1.0, f64::NAN).is_nan());
        assert!(java_max(1.0, f64::NAN).is_nan());
        assert_eq!(f64::min(f64::NAN, 1.0), 1.0); // the behaviour these two exist to avoid
        assert_eq!(f64::max(1.0, f64::NAN), 1.0);
    }

    /// `Math.min(-0.0, 0.0)` is `-0.0` and `Math.max(-0.0, 0.0)` is `0.0`, in either argument
    /// order. Unobservable through the reader — every consumer rounds to an `i32` — and pinned so
    /// the primitives are the primitives they claim to be.
    #[test]
    fn java_min_and_max_order_signed_zero() {
        assert!(java_min(-0.0, 0.0).is_sign_negative());
        assert!(java_min(0.0, -0.0).is_sign_negative());
        assert!(java_max(-0.0, 0.0).is_sign_positive());
        assert!(java_max(0.0, -0.0).is_sign_positive());
        // The ordinary cases are untouched.
        assert_eq!(java_min(1.0, 2.0), 1.0);
        assert_eq!(java_max(1.0, 2.0), 2.0);
        assert_eq!(java_min(2.0, 1.0), 1.0);
        assert_eq!(java_max(2.0, 1.0), 2.0);
    }

    /// The `referenced-nets-only` stem of `crates/fr-dsn/tests/data/p8t8-kicad-read-a.txt`: 17
    /// Greek letters inserted in alphabetical order, handed back by the JVM in this one.
    #[test]
    fn the_hash_iteration_order_matches_the_jvm() {
        let inserted = [
            "alpha", "beta", "gamma", "delta", "epsilon", "zeta", "eta", "theta", "iota", "kappa",
            "lambda", "mu", "nu", "xi", "omicron", "pi", "rho",
        ];
        let mut set = JavaStringSet::new();
        for name in inserted {
            set.add(name);
        }
        assert_eq!(
            set.iteration_order(),
            [
                "zeta", "iota", "nu", "delta", "mu", "theta", "epsilon", "xi", "lambda", "eta",
                "omicron", "alpha", "rho", "pi", "kappa", "beta", "gamma"
            ]
        );
    }

    /// `Math.round` is half-**up**, so `round(-0.5) == 0` and `round(0.5) == 1`: a via whose
    /// radius is exactly a half-integer gets an **asymmetric** box, and `-round(r)` would give
    /// the wrong lower corner.
    #[test]
    fn the_via_shape_rounds_each_corner_separately() {
        let Shape::Tile(fr_geometry::TileShape::Simplex(simplex)) = via_shape(0.5) else {
            panic!("IntBox::to_simplex answers a Simplex");
        };
        let expected = IntBox::from_coords(0, 0, 1, 1).to_simplex();
        assert_eq!(simplex, expected, "round(-0.5) is 0, not -1");
        // The symmetric case, for contrast.
        let Shape::Tile(fr_geometry::TileShape::Simplex(simplex)) = via_shape(4000.0) else {
            panic!("IntBox::to_simplex answers a Simplex");
        };
        assert_eq!(
            simplex,
            IntBox::from_coords(-4000, -4000, 4000, 4000).to_simplex()
        );
    }

    #[test]
    fn the_hash_set_dedups_on_exact_equality() {
        let mut set = JavaStringSet::new();
        set.add("GND");
        set.add("gnd");
        set.add("GND");
        assert_eq!(set.keys, ["GND", "gnd"]);
    }
}
