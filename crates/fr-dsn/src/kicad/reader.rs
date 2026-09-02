//! Port of `io/kicad/KiCadJsonReader.java`'s `readBoard` — the KiCad board-JSON reader.
//!
//! **Plan 8 Task 8 lands the signature and sections 1-8 (`KiCadJsonReader.java:63-497`); Task 9
//! extends the same function body with sections 9-11 (`:498-755`)**, which build the library
//! templates, the components and their pins, the traces, the conduction areas and the vias. The
//! obligation marker sits exactly where section 9 begins.
//!
//! not ported: the private `KiCadJsonReader()` constructor (KiCadJsonReader.java:55) — a
//! utility-class no-op; this is a Rust module.
// not ported: KiCadJsonReader.importSession (KiCadJsonReader.java:757-855) — Plan 8 Task 10's,
// which lands it beside `KiCadJsonWriter.write`. The roster line naming it is
// `crates/fr-drc/src/lib.rs:115`, where `scripts/audit-map/fr-drc.map` maps the class; this line
// exists so a reader of *this* file knows the other half of the class has a home.

use std::cmp::Ordering;

use fr_board::{
    Board, BoardLibrary, BoardRules, ClearanceMatrix, Communication, Components, ItemClass,
    ItemIdGenerator, Layer, LayerStructure, NetClassId, Packages, Padstacks, Unit, ViaInfo,
    ViaRule, equals_ignore_case,
};
use fr_geometry::{FloatPoint, IntBox, IntPoint, Point, PolygonShape, PolylineShapeRef, Shape};

use crate::coordinate_transform::CoordinateTransform;
use crate::error::{BoardMetadata, BoardReadResult};
use crate::format::java_round_to_int;
use crate::kicad::dto::{KiCadBoardJson, NetClassJson, Point2D, UnitJson};
use crate::parser::network::is_kicad_default_net_class_name;

/// Port of `KiCadJsonReader.readBoard` (KiCadJsonReader.java:61-755).
///
/// **Task 8 lands the signature and sections 1-8 (`:63-497`); Task 9 lands sections 9-11
/// (`:498-755`) and the two remaining private helpers — a fn-body extension, NOT a second
/// declaration.** Until Task 9 lands, the board this returns carries everything sections 1-8
/// build (layers, clearance matrix, outline, bounding box, communication, net classes, nets, via
/// infos, via rules and the via padstacks) and **no items beyond the board outline**: no
/// components, no pins, no traces, no vias, no conduction areas. Nothing on the end-to-end load
/// path reaches it — `fr_core::load::kicad_read_board` is still the inert stub Task 3 left, and
/// its `// obligation:` marker already names Task 9 — so the only callers are this crate's tests.
///
/// It answers `fr_dsn`'s own [`BoardReadResult`], so `fr-core`'s load path is format-agnostic.
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
/// catch: it returns that `ParseError` at each of the five points Java can actually throw from
/// inside sections 1-8 — the Gson parse and the four unguarded `null` lists — and its `detail`
/// text is **not** Java's (quirks #277 and #279).
// renamed: KiCadJsonReader.readBoard -> read_board, and its `Reader` parameter -> `json: &str`.
#[allow(clippy::too_many_lines)] // Java's own 435-line section-1-to-8 block, kept in one piece.
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
    if json_layers.is_empty() {
        // `:108-109`.
        board_layers.push(Layer::new("F.Cu", true));
        board_layers.push(Layer::new("B.Cu", true));
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
    let coordinate_transform = CoordinateTransform::new(scale_factor, 0.0, 0.0);
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
    for net_json in json_nets {
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
        if board
            .rules
            .nets
            .get_by_name_and_subnet(net_name, 1)
            .is_none()
        {
            let net = board
                .rules
                .nets
                .add(net_name.to_string(), 1, false, NetClassId(0));
            // Fallback to default class (default net class is at index 0)
            net.set_class(NetClassId(0));
        }
    }

    // obligation: Task 9 (`io/kicad/KiCadJsonReader.readBoard`, KiCadJsonReader.java:498-755)
    //   extends **this function body** with sections 9-11 — `// 9. Load Components & Library
    //   templates` (:498), `// 10. Traces` and `// 11. Vias/Conduction areas` — plus the two
    //   private helpers only they need, `getDescriptivePadstackName` (:857-890) and
    //   `arePackagePinsIdentical` (:892-924). Until then the board below carries no components,
    //   pins, traces, vias or conduction areas, and **no end-to-end path reaches this function**:
    //   `fr_core::load::kicad_read_board` is Task 3's inert stub and its own `// obligation:`
    //   marker already names Task 9, so `-de <board>.json` still fails there rather than loading
    //   a partial board. `crates/fr-dsn/tests/kicad_reader.rs` asserts only the section-1-to-8
    //   surface, and `crates/fr-dsn/tests/data/p8t8-kicad-read-a.txt`'s `[s9]` rows are the
    //   ground truth Task 9 finishes against.
    //   **Task 9 also inherits quirk #282**, whose "Java crashes at `:545`" claim is about a line
    //   section 9 is about to write: `boardLayers[li].name.equalsIgnoreCase(layerName)` is the
    //   first unconditional dereference of a layer name, and this port stores the empty string
    //   where Java stores `null`. Re-read that row before porting `:539-553`.

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

// ============================================================ the Java primitives this file needs

/// `BoardReadResult.ParseError` (`:65`, `:80`, `:748-749`).
fn parse_error(location: &str, detail: &str) -> BoardReadResult {
    BoardReadResult::ParseError {
        location: location.to_string(),
        detail: detail.to_string(),
    }
}

/// The `catch (Throwable)` arm (`:746-750`) reached by one of the four **unguarded** list
/// dereferences: `boardJson.layers` (`:105`), `boardJson.netClasses` (`:124`, inside
/// `nonDefaultNetClasses`), `boardJson.outline.corners` (`:169`), `boardJson.clearanceRules`
/// (`:156`) and `boardJson.nets` (`:446`).
///
/// Quirk #277: Java's `detail` carries the JVM's **helpful** `NullPointerException` message
/// (`Cannot invoke "java.util.List.isEmpty()" because "boardJson.layers" is null`) — a string the
/// JIT composes from the bytecode, which no port can reconstruct. The port writes the same shape
/// from the same two facts, so the row is greppable both ways, and
/// `crates/fr-dsn/tests/data/p8t8-kicad-read-a.txt` records Java's exact text beside it.
fn npe(invoked: &str, receiver: &str) -> BoardReadResult {
    parse_error(
        "json_payload",
        &format!("Exception occurred: Cannot invoke \"{invoked}\" because \"{receiver}\" is null"),
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
