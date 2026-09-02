//! `fr_dsn::kicad` — the KiCad board-JSON DTO tree and the whole of `readBoard`.
//!
//! Java authority: `io/kicad/KiCadBoardJson.java` and `io/kicad/KiCadJsonReader.java:61-755`
//! plus the six private helpers at `:857-1009`.
//!
//! # The ground truth
//!
//! `data/p8t8-kicad-read-a.txt` is the byte-exact stdout of
//! `scripts/differential/java/probes/P8T8Probe.java` against the pinned HEAD jar under JDK 25
//! (the probe's header comment carries the exact `javac`/`java` invocation). It measures
//! **24 inputs**: the seven real KiCad board-JSON files the Java checkout ships under `fixtures/`
//! — the fixture hunt the task brief asked for found them, so no writer-generated stand-in was
//! needed — plus seventeen synthetic payloads that reach the arms no fixture does (MIL and UM
//! units, an unknown unit, a lower-case unit, a missing layer list, a `plane` layer, custom
//! clearance rules, an empty board, a fractional and a zero resolution, a two-corner and an
//! unsorted outline, half-integer coordinates, duplicate net-class names, an all-referenced-nets
//! board, two `null` lists and the three malformed payloads).
//!
//! [`the_whole_section_1_to_8_surface_matches_the_jvm`] re-emits the same `[s8]` rows from the
//! Rust board and requires **zero** differing lines.
//!
//! `data/p8t8-kicad-read-b.txt` is the same probe's **part B** (`P8T8Probe b`), added by Task 9:
//! **67 inputs** — the same seven fixtures plus sixty synthetic payloads — and the whole item
//! graph rather than a count. Padstacks with their generated names and per-layer shapes, packages
//! with every pin, components in `Components` order, and every item in `board.itemList` order with
//! its net numbers, clearance class, fixed state and geometry.
//! [`the_whole_section_9_to_11_item_graph_matches_the_jvm`] compares it row by row.
//!
//! The named tests after the two replays pin, as literals, the four behaviours each task brief
//! calls out by name. Task 9's `:603` one is **not** named as its brief named it — see
//! [`a_package_dedup_failure_falls_back_to_a_duplicate_package`].

use std::fmt::Write as _;

use fr_board::{Board, DefaultItemClearanceClasses, Item, ItemClass, NetClassId, Unit};
use fr_dsn::error::{BoardMetadata, BoardReadResult};
use fr_dsn::format::java_double_to_string;
use fr_dsn::kicad::{UnitJson, read_board};
use fr_geometry::PolylineShapeRef;

// ============================================================== the transcript replay

const TRANSCRIPT: &str = include_str!("data/p8t8-kicad-read-a.txt");

/// The Java checkout's `fixtures/` directory, honouring `FREEROUTING_JAVA_DIR` the way the rest
/// of the suite does.
fn fixture(relative: &str) -> String {
    let root = std::env::var("FREEROUTING_JAVA_DIR").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../../freerouting").to_string()
    });
    std::fs::read_to_string(format!("{root}/{relative}"))
        .unwrap_or_else(|e| panic!("fixture {relative}: {e}"))
}

/// The inverse of `P8T8Probe.escape`.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\\') => out.push('\\'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('n') => out.push('\n'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// `escape` itself, for the rows this file re-emits.
fn escape(text: Option<&str>) -> String {
    let Some(text) = text else {
        return "<null>".to_string();
    };
    let mut out = String::with_capacity(text.len() + 8);
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out
}

/// One `[case]` of the transcript: its stem, the JSON that produced it, and its `[s8]` rows.
struct Case {
    stem: String,
    json: String,
    expected: Vec<String>,
}

fn transcript_cases() -> Vec<Case> {
    let mut cases: Vec<Case> = Vec::new();
    for line in TRANSCRIPT.lines() {
        if let Some(rest) = line.strip_prefix("[case] stem=") {
            let (stem, rest) = rest.split_once(' ').expect("a [case] line has two fields");
            let json = if let Some(path) = rest.strip_prefix("file=") {
                let path = path.split(' ').next().expect("non-empty");
                fixture(path)
            } else {
                unescape(rest.strip_prefix("json=").expect("file= or json="))
            };
            cases.push(Case {
                stem: stem.to_string(),
                json,
                expected: Vec::new(),
            });
        } else if let Some(row) = line.strip_prefix("[s8] ") {
            cases
                .last_mut()
                .expect("an [s8] row follows a [case] line")
                .expected
                .push(row.to_string());
        }
    }
    assert_eq!(cases.len(), 24, "the transcript measures 24 inputs");
    cases
}

/// Re-emits the probe's `[s8]` rows (without the prefix) from a Rust `BoardReadResult`.
///
/// Three columns need a note, because the port's types are not shaped exactly like Java's:
///
/// * `constants=<null>` — Java prints `<null>` for a `null` `SpecctraParserInfo.constants`, and
///   `fr_board::Communication` collapses Java's `null` and its empty collection into one empty
///   `Vec` on purpose (the two are indistinguishable to `Parser.writeScope`). The KiCad reader
///   always leaves it empty, so the mapping is unambiguous here.
/// * `writeResolution=null` — Java's `String.valueOf(null)`. `Communication::new` never sets one.
/// * `routerSettings=<null>` — `BoardMetadata.routerSettings` is `null` on this path (`:736`).
fn emit(result: &BoardReadResult) -> Vec<String> {
    let mut rows: Vec<String> = Vec::new();
    let (board, metadata, warnings) = match result {
        BoardReadResult::ParseError { location, detail } => {
            rows.push(format!(
                "result=ParseError location={} detail={}",
                escape(Some(location)),
                escape(Some(detail))
            ));
            return rows;
        }
        BoardReadResult::IoError(error) => {
            rows.push(format!(
                "result=IoError cause={}",
                escape(Some(&error.to_string()))
            ));
            return rows;
        }
        BoardReadResult::OutlineMissing {
            board,
            metadata,
            warnings,
            ..
        } => {
            rows.push("result=OutlineMissing".to_string());
            (board, metadata, warnings)
        }
        BoardReadResult::Success {
            board,
            metadata,
            warnings,
            ..
        } => {
            rows.push("result=Success".to_string());
            (board, metadata, warnings)
        }
    };
    let board = board
        .as_ref()
        .expect("a KiCad read that succeeds has a board");
    let transform = match result {
        BoardReadResult::Success {
            coordinate_transform,
            ..
        }
        | BoardReadResult::OutlineMissing {
            coordinate_transform,
            ..
        } => coordinate_transform.as_ref().expect("section 6 built one"),
        _ => unreachable!("handled above"),
    };

    // --- section 3 ------------------------------------------------------------------------
    let layers = &board.layer_structure().layers;
    rows.push(format!("layers count={}", layers.len()));
    for (i, layer) in layers.iter().enumerate() {
        rows.push(format!(
            "layer {i} name={} signal={}",
            escape(Some(&layer.name)),
            layer.is_signal
        ));
    }

    // --- section 4 ------------------------------------------------------------------------
    let matrix = &board.rules.clearance_matrix;
    rows.push(format!(
        "clearance classes={} layers={}",
        matrix.get_class_count(),
        matrix.get_layer_count()
    ));
    for i in 0..matrix.get_class_count() {
        rows.push(format!("clname {i} {}", escape(matrix.get_name(i))));
    }
    for layer in 0..matrix.get_layer_count() {
        for i in 0..matrix.get_class_count() {
            let mut row = format!("cl {layer} {i}");
            for j in 0..matrix.get_class_count() {
                let _ = write!(row, " {}", matrix.get_value(i, j, layer, false));
            }
            rows.push(row);
        }
    }

    // --- section 5 ------------------------------------------------------------------------
    let bbox = board.bounding_box;
    rows.push(format!(
        "bbox {} {} {} {}",
        bbox.ll.x, bbox.ll.y, bbox.ur.x, bbox.ur.y
    ));
    match board.get_outline().and_then(|id| board.get_item(id)) {
        None => rows.push("outline <null>".to_string()),
        Some(Item::BoardOutline(outline)) => {
            rows.push(format!(
                "outline shapes={} clearanceClass={}",
                outline.shape_count(),
                outline.hdr.clearance_class()
            ));
            for s in 0..outline.shape_count() {
                match outline.get_shape(s).expect("in range") {
                    PolylineShapeRef::Polygon(poly) => {
                        rows.push(format!(
                            "outlineshape {s} PolygonShape corners={}",
                            poly.corners().len()
                        ));
                        for (c, corner) in poly.corners().iter().enumerate() {
                            let point = corner.to_float();
                            rows.push(format!(
                                "corner {s} {c} {} {}",
                                java_double_to_string(point.x),
                                java_double_to_string(point.y)
                            ));
                        }
                    }
                    other => panic!("the KiCad reader only ever builds PolygonShapes: {other:?}"),
                }
            }
        }
        Some(other) => panic!("get_outline returned a non-outline item: {other:?}"),
    }

    // --- section 6 ------------------------------------------------------------------------
    let comm = &board.communication;
    assert!(
        comm.constants.is_empty() && comm.write_resolution.is_none(),
        "the KiCad reader passes `new SpecctraParserInfo(\"\\\"\", …, null, null, false)`"
    );
    rows.push(format!(
        "comm unit={} resolution={} stringQuote={} hostCad={} hostVersion={} constants=<null> \
         writeResolution=null dsnGeneratedByHost={}",
        comm.unit,
        comm.resolution,
        escape(Some(&comm.string_quote)),
        escape(comm.host_cad.as_deref()),
        escape(comm.host_version.as_deref()),
        comm.dsn_file_generated_by_host
    ));
    rows.push(format!(
        "transform scale={} baseX={} baseY={}",
        java_double_to_string(transform.scale_factor()),
        java_double_to_string(transform.base_x()),
        java_double_to_string(transform.base_y())
    ));

    // --- sections 7 and 8 -------------------------------------------------------------------
    rows.push(format!(
        "netclasses count={}",
        board.rules.net_classes.count()
    ));
    for i in 0..board.rules.net_classes.count() {
        let net_class = board.rules.net_classes.get(NetClassId(i));
        let widths: Vec<String> = (0..layers.len())
            .map(|layer| net_class.get_trace_half_width(layer).to_string())
            .collect();
        let active: Vec<String> = (0..layers.len())
            .map(|layer| net_class.is_active_routing_layer(layer).to_string())
            .collect();
        let dicc: Vec<String> = ItemClass::VALUES
            .iter()
            .map(|item_class| {
                format!(
                    "{}={}",
                    java_item_class_name(*item_class),
                    net_class.default_item_clearance_classes.get(*item_class)
                )
            })
            .collect();
        rows.push(format!(
            "netclass {i} name={} traceClearanceClass={} halfWidths=[{}] activeLayers=[{}] \
             viaRule={} shoveFixed={} pullTight={} ignoreCyclesWithAreas={} minTraceLength={} \
             maxTraceLength={} dicc=[{}]",
            escape(Some(net_class.get_name())),
            net_class.get_trace_clearance_class(),
            widths.join(","),
            active.join(","),
            net_class
                .get_via_rule()
                .map_or_else(|| "<null>".to_string(), |rule| escape(Some(&rule.name))),
            net_class.is_shove_fixed(),
            net_class.get_pull_tight(),
            net_class.get_ignore_cycles_with_areas(),
            java_double_to_string(net_class.get_minimum_trace_length()),
            java_double_to_string(net_class.get_maximum_trace_length()),
            dicc.join(",")
        ));
    }

    let max_net_no = board.rules.nets.max_net_number();
    rows.push(format!("nets count={max_net_no}"));
    for no in 1..=max_net_no {
        let net = board.rules.nets.get(no).expect("1..=max is in range");
        rows.push(format!(
            "net {no} name={} subnet={} containsPlane={} class={}",
            escape(Some(&net.name)),
            net.subnet_number,
            net.contains_plane(),
            escape(Some(
                board.rules.net_classes.get(net.get_net_class()).get_name()
            ))
        ));
    }

    rows.push(format!("viainfos count={}", board.rules.via_infos.count()));
    for (i, via_info) in board.rules.via_infos.iter().enumerate() {
        rows.push(format!(
            "viainfo {i} name={} padstack={} clearanceClass={} attachSmd={}",
            escape(Some(via_info.get_name())),
            escape(
                board
                    .library
                    .padstacks
                    .get(via_info.get_padstack())
                    .map(|padstack| padstack.name.as_str())
            ),
            via_info.get_clearance_class_index(),
            via_info.attach_smd_allowed()
        ));
    }

    rows.push(format!("viarules count={}", board.rules.via_rules.len()));
    for (i, rule) in board.rules.via_rules.iter().enumerate() {
        let vias: Vec<&str> = rule.iter().map(fr_board::ViaInfo::get_name).collect();
        rows.push(format!(
            "viarule {i} name={} vias=[{}]",
            escape(Some(&rule.name)),
            vias.join(",")
        ));
    }

    let via_padstacks = board.library.get_via_padstacks();
    rows.push(format!("viapadstacks count={}", via_padstacks.len()));
    for (i, id) in via_padstacks.iter().enumerate() {
        let padstack = board.library.padstacks.get(*id).expect("resolves");
        rows.push(format!(
            "viapadstack {i} name={} fromLayer={} toLayer={} attachAllowed={} placedAbsolute={}",
            escape(Some(&padstack.name)),
            padstack.from_layer(),
            padstack.to_layer(),
            padstack.attach_allowed,
            padstack.placed_absolute
        ));
    }

    rows.push(format!(
        "boardrules holeClearance={} minTraceHalfWidth={} maxTraceHalfWidth={} \
         traceAngleRestriction={} ignoreConduction={}",
        board.rules.get_hole_clearance(),
        board.rules.get_min_trace_half_width(),
        board.rules.get_max_trace_half_width(),
        java_angle_restriction_name(board.rules.trace_angle_restriction),
        board.rules.get_ignore_conduction()
    ));

    // --- the tail ---------------------------------------------------------------------------
    let metadata: &BoardMetadata = metadata.as_ref().expect("the tail always builds one");
    assert!(metadata.router_settings.is_none(), "`:736` passes null");
    rows.push(format!(
        "metadata hostCad={} hostVersion={} layerCount={} unit={} resolution={} snapAngle={} \
         routerSettings=<null>",
        escape(metadata.host_cad.as_deref()),
        escape(metadata.host_version.as_deref()),
        metadata.layer_count,
        metadata.unit,
        metadata.resolution,
        java_angle_restriction_name(metadata.snap_angle)
    ));
    rows.push(format!("warnings count={}", warnings.len()));
    for (i, warning) in warnings.iter().enumerate() {
        rows.push(format!("warning {i} {}", escape(Some(warning))));
    }
    rows
}

/// `DefaultItemClearanceClasses.ItemClass`'s Java constant names (net_class.rs's `ItemClass` is
/// the same enum in the same order).
fn java_item_class_name(item_class: ItemClass) -> &'static str {
    match item_class {
        ItemClass::None => "NONE",
        ItemClass::Trace => "TRACE",
        ItemClass::Via => "VIA",
        ItemClass::Pin => "PIN",
        ItemClass::Smd => "SMD",
        ItemClass::Area => "AREA",
    }
}

/// `AngleRestriction`'s Java constant names.
fn java_angle_restriction_name(angle: fr_board::AngleRestriction) -> &'static str {
    match angle {
        fr_board::AngleRestriction::None => "NONE",
        fr_board::AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
        fr_board::AngleRestriction::NinetyDegree => "NINETY_DEGREE",
    }
}

/// The one row of the transcript that the port is **not** expected to reproduce, with its root
/// cause — an XDIFF, in the sense the workspace uses the word: a divergence that is understood,
/// bounded and recorded, not a failure that is tolerated.
///
/// **Quirk #277.** Java parses with Gson and the port with `serde_json`. On a *well-formed*
/// payload the two agree on everything (995 of the 996 rows below prove it), but a **malformed**
/// one produces a `ParseError` whose `detail` is the parser's own message, and no port can
/// reconstruct Gson's. Both readers reject the same inputs at the same `location`; only the
/// human-readable text differs, and nothing in the tree parses it — `fr_core::load`'s
/// `parse_board_result` turns a `ParseError` into an `Error` by formatting both fields into a
/// message for the user (`crates/fr-core/src/load.rs`), and the CLI prints that.
///
/// The other three malformed stems — `json-null`, `json-empty`, `layers-null`,
/// `netclasses-null` — do **not** appear here: their `detail` is a string `readBoard` itself
/// composes, so the port matches them byte for byte.
const XDIFF: &[(&str, usize, &str)] = &[(
    "json-truncated",
    0,
    "quirk #277: the ParseError detail on a syntactically invalid payload is the JSON parser's \
     own message — `java.io.EOFException: End of input at line 1 column 2 path $.` from Gson, \
     `EOF while parsing an object at line 1 column 1` from serde_json. Same location, same \
     rejection, different prose.",
)];

/// **The acceptance test**: zero *unexplained* differing `[s8]` rows on all 24 inputs, the one
/// [`XDIFF`] row aside.
#[test]
fn the_whole_section_1_to_8_surface_matches_the_jvm() {
    let mut diffs: Vec<String> = Vec::new();
    let mut compared = 0usize;
    let mut xdiffs_seen = 0usize;
    for case in transcript_cases() {
        let result = read_board(&case.json, None);
        let actual = emit(&result);
        for (i, expected) in case.expected.iter().enumerate() {
            compared += 1;
            let excused = XDIFF
                .iter()
                .find(|(stem, row, _)| *stem == case.stem && *row == i);
            match (actual.get(i), excused) {
                (Some(row), None) if row == expected => {}
                (Some(row), Some((_, _, reason))) => {
                    assert_ne!(
                        row, expected,
                        "{}[{i}] now MATCHES the JVM — delete its XDIFF entry ({reason})",
                        case.stem
                    );
                    xdiffs_seen += 1;
                }
                (Some(row), None) => diffs.push(format!(
                    "{}[{i}]\n  java: {expected}\n  rust: {row}",
                    case.stem
                )),
                (None, _) => diffs.push(format!("{}[{i}] missing\n  java: {expected}", case.stem)),
            }
        }
        if actual.len() > case.expected.len() {
            for row in &actual[case.expected.len()..] {
                diffs.push(format!("{} extra\n  rust: {row}", case.stem));
            }
        }
    }
    assert!(
        compared > 900,
        "the transcript should carry well over 900 [s8] rows, got {compared}"
    );
    assert_eq!(
        xdiffs_seen,
        XDIFF.len(),
        "every XDIFF row must have been reached"
    );
    assert!(
        diffs.is_empty(),
        "{} of {compared} rows differ from the JVM with no XDIFF entry:\n{}",
        diffs.len(),
        diffs.join("\n")
    );
}

// ============================================ the part-B transcript replay (sections 9-11)

/// `data/p8t8-kicad-read-b.txt` is the byte-exact stdout of the **same** probe run with the
/// argument `b`: `P8T8Probe b`. Part A's 24 inputs come first, unchanged, and then the
/// **forty-three** section-9-to-11 inputs `CORPUS_B` adds — the pad-shape arms, the package-dedup
/// ladder, both `%.0f` HALF_UP name generators, every unguarded list dereference, the
/// out-of-range and inverted layer indices, and the four `null`-name crashes quirk #282 and its
/// neighbours predict.
///
/// The rows are `[s9]` and cover the whole item graph: every padstack with its per-layer shape,
/// every package with every pin, every component, and every item in `board.getItems()` order —
/// Java's **descending item id** (quirk #63), so a port that numbers items differently fails
/// immediately.
const TRANSCRIPT_B: &str = include_str!("data/p8t8-kicad-read-b.txt");

fn transcript_b_cases() -> Vec<Case> {
    let mut cases: Vec<Case> = Vec::new();
    for line in TRANSCRIPT_B.lines() {
        if let Some(rest) = line.strip_prefix("[case] stem=") {
            let (stem, rest) = rest.split_once(' ').expect("a [case] line has two fields");
            let json = if let Some(path) = rest.strip_prefix("file=") {
                let path = path.split(' ').next().expect("non-empty");
                fixture(path)
            } else {
                unescape(rest.strip_prefix("json=").expect("file= or json="))
            };
            cases.push(Case {
                stem: stem.to_string(),
                json,
                expected: Vec::new(),
            });
        } else if let Some(row) = line.strip_prefix("[s9] ") {
            cases
                .last_mut()
                .expect("an [s9] row follows a [case] line")
                .expected
                .push(row.to_string());
        }
    }
    assert_eq!(cases.len(), 67, "the part-B transcript measures 67 inputs");
    cases
}

/// `P8T8Probe.shapeOf`: the discriminator plus enough geometry to tell two shapes apart.
fn emit_shape(shape: Option<&fr_geometry::Shape>) -> String {
    use fr_geometry::{Shape, ShapeOps, TileShape};
    let Some(shape) = shape else {
        return "<null>".to_string();
    };
    if let Shape::Circle(circle) = shape {
        return format!(
            "Circle({},{},r={})",
            java_double_to_string(circle.center.to_float().x),
            java_double_to_string(circle.center.to_float().y),
            circle.radius
        );
    }
    // Java's `getClass().getSimpleName()` over the concrete `ConvexShape`.
    let name = match shape {
        Shape::Tile(TileShape::Box(_)) => "IntBox",
        Shape::Tile(TileShape::Octagon(_)) => "IntOctagon",
        Shape::Tile(TileShape::Simplex(_)) => "Simplex",
        Shape::Polygon(_) => "PolygonShape",
        Shape::Circle(_) => unreachable!("handled above"),
    };
    format!("{name}{}", emit_corners(&shape.corner_approx_arr()))
}

/// `P8T8Probe.areaOf`: an area's corner list, approximated.
fn emit_corners(corners: &[fr_geometry::FloatPoint]) -> String {
    let mut out = String::from("(");
    for (i, corner) in corners.iter().enumerate() {
        if i > 0 {
            out.push(';');
        }
        let _ = write!(
            out,
            "{},{}",
            java_double_to_string(corner.x),
            java_double_to_string(corner.y)
        );
    }
    out.push(')');
    out
}

/// `FixedState`'s Java constant names.
fn java_fixed_state_name(state: fr_board::FixedState) -> &'static str {
    match state {
        fr_board::FixedState::Unfixed => "NOT_FIXED",
        fr_board::FixedState::ShoveFixed => "SHOVE_FIXED",
        fr_board::FixedState::UserFixed => "USER_FIXED",
        fr_board::FixedState::SystemFixed => "SYSTEM_FIXED",
    }
}

/// Re-emits the probe's `[s9]` rows (without the prefix) from a Rust `BoardReadResult`.
fn emit_b(result: &BoardReadResult) -> Vec<String> {
    use fr_geometry::{Area, ShapeOps};

    let mut rows: Vec<String> = Vec::new();
    let board = match result {
        BoardReadResult::ParseError { location, detail } => {
            rows.push(format!(
                "result=ParseError location={} detail={}",
                escape(Some(location)),
                escape(Some(detail))
            ));
            return rows;
        }
        BoardReadResult::IoError(error) => {
            rows.push(format!(
                "result=IoError cause={}",
                escape(Some(&error.to_string()))
            ));
            return rows;
        }
        BoardReadResult::OutlineMissing { board, .. } => {
            rows.push("result=OutlineMissing".to_string());
            board
        }
        BoardReadResult::Success { board, .. } => {
            rows.push("result=Success".to_string());
            board
        }
    };
    let board = board
        .as_ref()
        .expect("a KiCad read that succeeds has a board");
    let layer_count = board.layer_structure().layers.len();

    // --- the library: padstacks ---------------------------------------------------------------
    rows.push(format!(
        "padstacks count={}",
        board.library.padstacks.count()
    ));
    for i in 1..=board.library.padstacks.count() {
        let padstack = board
            .library
            .padstacks
            .get(fr_board::PadstackId(i))
            .expect("1..=count");
        let shapes: Vec<String> = (0..layer_count)
            .map(|layer| {
                emit_shape(padstack.get_shape(i32::try_from(layer).expect("layer fits an i32")))
            })
            .collect();
        rows.push(format!(
            "padstack {i} name={} fromLayer={} toLayer={} attachAllowed={} placedAbsolute={} \
             shapes=[{}]",
            escape(Some(&padstack.name)),
            padstack.from_layer(),
            padstack.to_layer(),
            padstack.attach_allowed,
            padstack.placed_absolute,
            shapes.join(";")
        ));
    }

    // --- the library: packages and their pins -------------------------------------------------
    rows.push(format!("packages count={}", board.library.packages.count()));
    for i in 1..=board.library.packages.count() {
        let package = board.library.packages.get(i);
        rows.push(format!(
            "package {i} name={} isFront={} pins={}",
            escape(Some(&package.name)),
            package.is_front,
            package.pin_count()
        ));
        for j in 0..package.pin_count() {
            let pin = package
                .get_pin(i32::try_from(j).expect("pin index fits an i32"))
                .expect("j < pin_count()");
            rows.push(format!(
                "packagepin {i} {j} name={} padstack={} rel={},{} rot={}",
                escape(Some(&pin.name)),
                pin.padstack_no.0,
                java_double_to_string(pin.relative_location.to_float().x),
                java_double_to_string(pin.relative_location.to_float().y),
                java_double_to_string(pin.rotation_in_degree)
            ));
        }
    }

    // --- the components -----------------------------------------------------------------------
    rows.push(format!("components count={}", board.components.count()));
    for i in 1..=board.components.count() {
        let component = board
            .components
            .get(i32::try_from(i).expect("component id fits an i32"));
        let location = component.get_location().map_or_else(
            || "<null>".to_string(),
            |location| {
                format!(
                    "{},{}",
                    java_double_to_string(location.to_float().x),
                    java_double_to_string(location.to_float().y)
                )
            },
        );
        rows.push(format!(
            "component {i} name={} id={} location={location} rotation={} onFront={} package={} \
             positionFixed={} partNumber={}",
            escape(Some(&component.name)),
            component.id,
            java_double_to_string(component.get_rotation_in_degree()),
            component.placed_on_front(),
            component.get_package(),
            component.position_fixed,
            escape(component.get_part_number())
        ));
    }

    // --- every item, in `getItems()` order (descending id, quirk #63) --------------------------
    let items: Vec<&Item> = board.get_items().collect();
    rows.push(format!("items count={}", items.len()));
    for item in items {
        let header = item.header();
        let nets: Vec<String> = (0..header.net_count())
            .map(|n| header.get_net_number(n).to_string())
            .collect();
        let kind = match item {
            Item::Trace(_) => "PolylineTrace",
            Item::Via(_) => "Via",
            Item::Pin(_) => "Pin",
            Item::ObstacleArea(_) => "ObstacleArea",
            Item::ConductionArea(_) => "ConductionArea",
            Item::ViaObstacleArea(_) => "ViaObstacleArea",
            Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
            Item::ComponentOutline(_) => "ComponentOutline",
            Item::BoardOutline(_) => "BoardOutline",
        };
        let head = format!(
            "item {} {kind} nets=[{}] cl={} comp={} fixed={}",
            header.id().0,
            nets.join(","),
            header.clearance_class(),
            header.get_component_id(),
            java_fixed_state_name(header.get_fixed_state())
        );
        rows.push(match item {
            Item::Pin(pin) => format!("{head} pinIndex={}", pin.get_pin_index()),
            Item::Trace(trace) => {
                let corners: Vec<String> = (0..trace.polyline().corner_count())
                    .map(|c| {
                        let corner = trace
                            .polyline()
                            .corner_approx(c)
                            .expect("c < corner_count()");
                        format!(
                            "{},{}",
                            java_double_to_string(corner.x),
                            java_double_to_string(corner.y)
                        )
                    })
                    .collect();
                format!(
                    "{head} layer={} halfWidth={} corners=[{}]",
                    trace.get_layer(),
                    trace.get_half_width(),
                    corners.join(";")
                )
            }
            Item::Via(via) => format!(
                "{head} padstack={} center={},{} attachAllowed={}",
                via.get_padstack_id().0,
                java_double_to_string(via.get_center().to_float().x),
                java_double_to_string(via.get_center().to_float().y),
                via.attach_allowed
            ),
            Item::ConductionArea(zone) => {
                let area = match zone.get_relative_area() {
                    Area::Shape(shape) => emit_corners(&shape.corner_approx_arr()),
                    Area::Polyline(area) => emit_corners(&area.corner_approx_arr()),
                };
                format!(
                    "{head} layer={} isObstacle={} area={area}",
                    zone.get_layer(),
                    zone.get_is_obstacle()
                )
            }
            Item::BoardOutline(outline) => format!("{head} shapes={}", outline.shape_count()),
            _ => head,
        });
    }
    rows
}

/// The part-B rows the port is **not** expected to reproduce, each with its root cause.
///
/// There are three families and all three are totalizations, not disagreements about what the
/// reader does:
///
/// 1. **quirk #277** — the `ParseError.detail` of a payload the *parser* rejects is the parser's
///    own prose (Gson vs `serde_json`). Row 0 of `json-truncated`, exactly as in part A.
/// 2. **quirk #283** — a `null` **element** inside a JSON array. Gson stores a `null` reference;
///    `serde_json` needs the element type to be nullable. `PadJson.layers` was made
///    `Option<Vec<Option<String>>>` because Java *loads* that input (stem
///    `pad-layers-null-element`, which matches). The three lists where Java stores the `null` and
///    then **throws** on it are left as they are: both sides reject the file, and only the prose
///    differs.
/// 3. **quirk #282's family** — a name Java keeps as `null` and `fr_board` keeps as a `String`.
///    Two consequences reach a row: a package pin whose `name` the probe prints as `<null>`, and a
///    conduction area whose `layerIndex` Java stores negative where `fr_board`'s layer is a
///    `usize`.
const XDIFF_B: &[(&str, usize, &str)] = &[
    (
        "json-truncated",
        0,
        "quirk #277: the ParseError detail on a syntactically invalid payload is the JSON \
         parser's own message — Gson's `java.io.EOFException: End of input at line 1 column 2 \
         path $.` against serde_json's `EOF while parsing an object at line 1 column 1`.",
    ),
    (
        "trace-point-null-element",
        0,
        "quirk #283: `\"points\": [{...}, null]` is a `List<Point2D>` holding a null in Gson, and \
         `:675`'s `pt.x` then throws `Cannot read field \"x\" because \"pt\" is null`. \
         `serde_json` refuses the null against `Vec<Point2D>` first, so the port answers the same \
         `location` with the deserializer's prose. Both reject the file.",
    ),
    (
        "outline-corner-null-element",
        0,
        "quirk #283, as `trace-point-null-element` but through section 5's `outline.corners`.",
    ),
    (
        "netclass-null-element",
        0,
        "quirk #283: `\"netClasses\": [null]` is a one-element list holding a null, and \
         `isKiCadDefaultNetClassName(netClass.name)` then throws `Cannot read field \"name\" \
         because \"netClass\" is null`. Same rejection, different prose.",
    ),
    (
        "pad-null-name-dedup",
        6,
        "quirk #282's family: `Package.Pin.name` is a nullable Java `String` and \
         `fr_board::PackagePin::name` is a `String`, so a pad with no `name` key is `<null>` to \
         the probe and `` here. The behaviour it drives — `arePackagePinsIdentical:908`'s throw, \
         and therefore the three duplicate packages this stem ends with — **is** reproduced; only \
         the printed name differs.",
    ),
    ("pad-null-name-dedup", 8, "quirk #282's family, as row 6."),
    ("pad-null-name-dedup", 10, "quirk #282's family, as row 6."),
    (
        "zone-negative-layer",
        6,
        "totalized: `ObstacleArea.layer` is a Java `int` that `:663` fills from `zone.layerIndex` \
         verbatim, so Java keeps `-3`; `fr_board`'s layer is a `usize`. Nothing a KiCad export \
         writes and nothing in the corpus reaches it — the stem exists so the divergence is \
         measured rather than assumed.",
    ),
];

/// **The part-B acceptance test**: zero *unexplained* differing `[s9]` rows on all 67 inputs.
#[test]
fn the_whole_section_9_to_11_item_graph_matches_the_jvm() {
    let mut diffs: Vec<String> = Vec::new();
    let mut compared = 0usize;
    let mut xdiffs_seen = 0usize;
    for case in transcript_b_cases() {
        let result = read_board(&case.json, None);
        let actual = emit_b(&result);
        for (i, expected) in case.expected.iter().enumerate() {
            compared += 1;
            let excused = XDIFF_B
                .iter()
                .find(|(stem, row, _)| *stem == case.stem && *row == i);
            match (actual.get(i), excused) {
                (Some(row), None) if row == expected => {}
                (Some(row), Some((_, _, reason))) => {
                    assert_ne!(
                        row, expected,
                        "{}[{i}] now MATCHES the JVM — delete its XDIFF entry ({reason})",
                        case.stem
                    );
                    xdiffs_seen += 1;
                }
                (Some(row), None) => diffs.push(format!(
                    "{}[{i}]\n  java: {expected}\n  rust: {row}",
                    case.stem
                )),
                (None, _) => diffs.push(format!("{}[{i}] missing\n  java: {expected}", case.stem)),
            }
        }
        if actual.len() > case.expected.len() {
            for row in &actual[case.expected.len()..] {
                diffs.push(format!("{} extra\n  rust: {row}", case.stem));
            }
        }
    }
    assert!(
        compared > 2700,
        "the part-B transcript should carry well over 2700 [s9] rows, got {compared}"
    );
    assert_eq!(
        xdiffs_seen,
        XDIFF_B.len(),
        "every XDIFF row must have been reached"
    );
    assert!(
        diffs.is_empty(),
        "{} of {compared} rows differ from the JVM with no XDIFF entry:\n{}",
        diffs.len(),
        diffs.join("\n")
    );
}

// ============================================================== the four named behaviours

/// Reads a board and unwraps it, for the literal tests below.
fn board_of(json: &str) -> (Box<Board>, BoardMetadata, Vec<String>) {
    match read_board(json, None) {
        BoardReadResult::Success {
            board,
            metadata,
            warnings,
            ..
        } => (
            board.expect("a board"),
            metadata.expect("metadata"),
            warnings,
        ),
        other => panic!("expected Success, got {other:?}"),
    }
}

/// **Quirk #83.** `ClearanceMatrix.setValue`/`getValue` index `row[j].column[i]`, and
/// `readBoard:153` writes only `setValue(1, clNo, …)` while `:161` writes only
/// `setValue(idxA, idxB, …)` — so a KiCad-sourced matrix is genuinely asymmetric and the port
/// must not "fix" it by writing both orders (plan-3 hand-off §Plan 8).
///
/// Literals from `data/p8t8-kicad-read-a.txt`, stem `unit-mil`:
///
/// ```text
/// [s8] clname 0 null / 1 default / 2 HV
/// [s8] cl 0 1 0 10 78      <- getValue(1, ·) : 0, 10, 78
/// [s8] cl 0 2 0 8 40       <- getValue(2, ·) : 0,  8, 40
/// ```
#[test]
fn the_clearance_matrix_stays_asymmetric() {
    let json = r#"{"unit":"MIL","resolution":1.0,
        "layers":[{"index":0,"name":"F.Cu","type":"signal"},
                  {"index":1,"name":"In1.Cu","type":"plane"},
                  {"index":2,"name":"B.Cu","type":"SIGNAL"}],
        "netClasses":[{"name":"Default","clearance":10.0,"traceWidth":15.0},
                      {"name":"HV","clearance":40.0,"traceWidth":25.0}],
        "clearanceRules":[{"classA":"default","classB":"HV","clearance":77.0}],
        "outline":{"corners":[{"x":0.0,"y":0.0},{"x":100.0,"y":0.0},
                              {"x":100.0,"y":80.0},{"x":0.0,"y":80.0}]}}"#;
    let (board, _, _) = board_of(json);
    let matrix = &board.rules.clearance_matrix;
    assert_eq!(matrix.get_name(0), Some("null"));
    assert_eq!(matrix.get_name(1), Some("default"));
    assert_eq!(matrix.get_name(2), Some("HV"));

    // The custom rule wrote `setValue(1, 2, 77)`, which `ClearanceMatrix` rounds up to the even
    // 78 and stores at `row[2].column[1]`.
    assert_eq!(matrix.get_value(1, 2, 0, false), 78);
    // Nothing ever wrote `row[1].column[2]`, so it still holds `setDefaultValue`'s
    // `round(scale(0.2, MM, MIL) * 1) == 8`.
    assert_eq!(matrix.get_value(2, 1, 0, false), 8);
    assert_ne!(
        matrix.get_value(1, 2, 0, false),
        matrix.get_value(2, 1, 0, false),
        "quirk #83's asymmetry must survive a KiCad read"
    );
    // The diagonal the net-class loop wrote, on every layer.
    for layer in 0..3 {
        assert_eq!(matrix.get_value(2, 2, layer, false), 40);
        assert_eq!(matrix.get_value(1, 1, layer, false), 10);
    }
}

/// `readBoard:299-303` builds each outline corner as
/// `new IntPoint(round(pt.x * scale), round(-pt.y * scale))` — the **Y is negated**, and the
/// rounding is `Math.round` (half **up**, away from -inf), not `f64::round` (half away from
/// zero) and not `Math.rint`.
///
/// `Math.round` is half **up** (towards +inf), so `round(0.5) == 1` while `round(-0.5) == 0` —
/// an asymmetry neither `f64::round` (half away from zero: `-1`) nor `Math.rint` (half to even:
/// `-0.0`) has. The literals are the transcript's `y-half-tie` stem, whose three corners are all
/// half-integers and whose Y coordinates are all positive, so the negation and the tie direction
/// are both observable.
///
/// ```text
/// [s8] corner 0 0 11.0 -20.0     <- ( 10.5,  20.5) -> (round(10.5), round(-20.5)) = (11, -20)
/// [s8] corner 0 1 11.0 0.0       <- ( 10.5,   0.5) -> (round(10.5), round( -0.5)) = (11,   0)
/// [s8] corner 0 2 1.0 0.0        <- (  0.5,   0.5) -> (round( 0.5), round( -0.5)) = ( 1,   0)
/// ```
#[test]
fn y_is_negated_and_java_rounded() {
    // scale = 1 (MIL keeps the file's resolution), so the coordinates are the corner values.
    let json = r#"{"unit":"MIL","resolution":1.0,
        "outline":{"corners":[{"x":0.5,"y":0.5},{"x":10.5,"y":0.5},{"x":10.5,"y":20.5}]}}"#;
    let (board, _, _) = board_of(json);
    let outline_id = board.get_outline().expect("the constructor inserts one");
    let Some(Item::BoardOutline(outline)) = board.get_item(outline_id) else {
        panic!("the outline item");
    };
    let PolylineShapeRef::Polygon(polygon) = outline.get_shape(0).expect("one shape") else {
        panic!("the KiCad reader builds a PolygonShape");
    };
    let corners: Vec<(f64, f64)> = polygon
        .corners()
        .iter()
        .map(|corner| {
            let point = corner.to_float();
            (point.x, point.y)
        })
        .collect();
    assert_eq!(corners, [(11.0, -20.0), (11.0, 0.0), (1.0, 0.0)]);
    // `round(-20.5) == -20`, not `-21`: half rounds towards +inf. `f64::round` would give `-21`.
    assert_eq!(
        f64::round(-20.5),
        -21.0,
        "the rounding the port must NOT use"
    );
    // Every Y is <= 0 because every input Y was >= 0.
    assert!(corners.iter().all(|(_, y)| *y <= 0.0), "{corners:?}");
}

/// `readBoard:313-321`: a missing, `null` or blank `hostCad`/`hostVersion` falls back to the
/// literal strings `"KiCad"` and `"v10.0"`, which reach `Communication.SpecctraParserInfo` and
/// therefore the SES the port later writes. They are **not** `PARITY_VERSION` (plan-8 ruling 5).
///
/// The tail's `BoardMetadata` (`:730-731`) is a different matter — see quirk #278, pinned below.
#[test]
fn the_host_fallbacks_are_kicad_and_v10() {
    let outline =
        r#""outline":{"corners":[{"x":0.0,"y":0.0},{"x":1.0,"y":0.0},{"x":1.0,"y":1.0}]}"#;
    for json in [
        format!("{{{outline}}}"),
        format!(r#"{{"hostCad":null,"hostVersion":null,{outline}}}"#),
        format!(r#"{{"hostCad":"","hostVersion":"",{outline}}}"#),
        // `String.isBlank()`: every code point is `Character.isWhitespace`.
        format!("{{\"hostCad\":\"  \\t\",\"hostVersion\":\" \\n \",{outline}}}"),
    ] {
        let (board, _, _) = board_of(&json);
        assert_eq!(
            board.communication.host_cad.as_deref(),
            Some("KiCad"),
            "{json}"
        );
        assert_eq!(
            board.communication.host_version.as_deref(),
            Some("v10.0"),
            "{json}"
        );
    }

    // A non-blank host survives into `Communication` …
    let json = format!(r#"{{"hostCad":"Altium","hostVersion":"v24",{outline}}}"#);
    let (board, metadata, _) = board_of(&json);
    assert_eq!(board.communication.host_cad.as_deref(), Some("Altium"));
    assert_eq!(board.communication.host_version.as_deref(), Some("v24"));
    // … and **not** into the metadata: `:730-731` passes the literals, dropping the locals
    // section 6 resolved. Quirk #278; transcript stem `resolution-zero` measures the same pair.
    assert_eq!(metadata.host_cad.as_deref(), Some("KiCad"));
    assert_eq!(metadata.host_version.as_deref(), Some("v10.0"));

    // `\u{a0}` is whitespace to Rust and **not** to `Character.isWhitespace`, so it is a
    // non-blank host name.
    let json = format!("{{\"hostCad\":\"\u{a0}\",{outline}}}");
    let (board, _, _) = board_of(&json);
    assert_eq!(board.communication.host_cad.as_deref(), Some("\u{a0}"));
}

/// `readBoard:89-94` tests `boardJson.unit` against `MIL` and then `UM`; **every** other value
/// falls through to the `Unit userUnit = Unit.MM` the variable was declared with — an absent key
/// (Gson leaves the `= UnitJson.MM` initializer), an explicit `null`, and an unrecognised
/// constant name, because Gson's `EnumTypeAdapter` answers `null` rather than throwing.
///
/// The match is **case-sensitive**: `"mil"` is not `MIL`. Transcript stems `unit-unknown` and
/// `unit-lowercase` measure both, and both come back `unit=mm resolution=10000` — the
/// `resolution == 1.0 && userUnit == MM` arm at `:98-100`, which an actual `MIL` board does not
/// reach.
#[test]
fn an_unknown_unit_falls_through_to_the_documented_arm() {
    let outline =
        r#""outline":{"corners":[{"x":0.0,"y":0.0},{"x":1.0,"y":0.0},{"x":1.0,"y":1.0}]}"#;
    for unit in [
        r#""unit":"FOO","#,
        r#""unit":"mil","#,
        r#""unit":"mm","#,
        r#""unit":null,"#,
        "",
    ] {
        let json = format!(r#"{{{unit}"resolution":1.0,{outline}}}"#);
        let (board, metadata, _) = board_of(&json);
        assert_eq!(board.communication.unit, Unit::Mm, "for {unit:?}");
        assert_eq!(metadata.unit, Unit::Mm, "for {unit:?}");
        // The `:98-100` default only a MM board reaches.
        assert_eq!(board.communication.resolution, 10000, "for {unit:?}");
    }

    // The two arms that are *not* the fall-through.
    for (unit, expected) in [("MIL", Unit::Mil), ("UM", Unit::Um)] {
        let json = format!(r#"{{"unit":"{unit}","resolution":1.0,{outline}}}"#);
        let (board, _, _) = board_of(&json);
        assert_eq!(board.communication.unit, expected);
        assert_eq!(
            board.communication.resolution, 1,
            "no MM default for {unit}"
        );
    }

    // And the DTO layer the fall-through rests on.
    assert_eq!(
        serde_json::from_str::<fr_dsn::kicad::KiCadBoardJson>(r#"{"unit":"FOO"}"#)
            .expect("Gson does not throw on an unknown constant")
            .unit,
        None
    );
    assert_eq!(
        serde_json::from_str::<fr_dsn::kicad::KiCadBoardJson>("{}")
            .expect("parses")
            .unit,
        Some(UnitJson::MM)
    );
}

// ============================================================== the remaining brief behaviours

/// **Every** `null` list in `readBoard` reaches Java's `catch (Throwable)` and comes back as
/// `ParseError("json_payload", …)`. Quirk #277 covers the `detail` text, which is the JVM's
/// helpful-NPE string on one side and the port's reconstruction on the other; the *shape* of the
/// answer is what parity needs and what this pins.
///
/// **This test read `components`, `traces`, `vias` and `conductionAreas` as "guarded and must
/// load" until Task 9.** They *are* guarded — but only inside section 8's `referencedNets` sweep
/// (`:457`, `:468`, `:475`, `:482`), which is where sections 1-8 stopped. Sections 9-11 then walk
/// all four again **unguarded**, at `:503`, `:648`, `:667` and `:684`, so a `null` any of them is
/// a `ParseError` on the finished reader. Measured on the four `*-null` stems of
/// `data/p8t8-kicad-read-b.txt`; the "must load" assertion was true of a half-ported `readBoard`
/// and of nothing else.
#[test]
fn every_null_list_is_a_parse_error_somewhere_in_read_board() {
    // Sections 1-8's five unguarded dereferences: `:105`, `:124`, `:156`, `:169` and `:446`.
    for key in ["layers", "netClasses", "clearanceRules", "nets"] {
        let json = format!("{{\"{key}\": null}}");
        match read_board(&json, None) {
            BoardReadResult::ParseError { location, detail } => {
                assert_eq!(location, "json_payload", "for {key}");
                assert!(
                    detail.starts_with("Exception occurred: Cannot invoke"),
                    "{detail}"
                );
            }
            other => panic!("expected a ParseError for {key}, got {other:?}"),
        }
    }
    // Sections 9-11's four, guarded in section 8 and unguarded here.
    for (key, receiver) in [
        ("components", "boardJson.components"),
        ("conductionAreas", "boardJson.conductionAreas"),
        ("traces", "boardJson.traces"),
        ("vias", "boardJson.vias"),
    ] {
        let json = format!("{{\"{key}\": null}}");
        match read_board(&json, None) {
            BoardReadResult::ParseError { location, detail } => {
                assert_eq!(location, "json_payload", "for {key}");
                assert_eq!(
                    detail,
                    format!(
                        "Exception occurred: Cannot invoke \"java.util.List.iterator()\" \
                         because \"{receiver}\" is null"
                    ),
                    "for {key}"
                );
            }
            other => panic!("expected a ParseError for {key}, got {other:?}"),
        }
    }
    // `outline.corners` is unguarded behind a guarded `outline`.
    assert!(matches!(
        read_board(r#"{"outline": null}"#, None),
        BoardReadResult::Success { .. }
    ));
    assert!(matches!(
        read_board(r#"{"outline": {"corners": null}}"#, None),
        BoardReadResult::ParseError { .. }
    ));
}

/// `:79-81` — an empty payload and the literal `null` both answer
/// `ParseError("json_root", "JSON payload is empty or invalid")`, because Gson's `fromJson`
/// returns `null` for both rather than throwing.
#[test]
fn an_empty_or_null_payload_is_a_json_root_parse_error() {
    for json in ["", "   ", "\n\t", "null"] {
        match read_board(json, None) {
            BoardReadResult::ParseError { location, detail } => {
                assert_eq!(location, "json_root", "for {json:?}");
                assert_eq!(detail, "JSON payload is empty or invalid");
            }
            other => panic!("expected a json_root ParseError for {json:?}, got {other:?}"),
        }
    }
}

/// **Quirk #280.** `:456`'s `HashSet<String> referencedNets` is iterated at `:490`, and the order
/// it hands the names back is the order `Nets.add` numbers them in — so the net numbers of every
/// auto-registered net are a function of `String.hashCode` and of nothing else. The literals are
/// the transcript's `ecc83-v1` stem, whose JSON declares **no** nets at all: all thirteen come
/// from pad `netName`s.
#[test]
fn the_auto_registered_nets_take_java_hash_set_order() {
    let (board, _, _) = board_of(&fixture(
        "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.json",
    ));
    let names: Vec<&str> = (1..=board.rules.nets.max_net_number())
        .map(|no| board.rules.nets.get(no).expect("in range").name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "unconnected-(P7-Pad1)",
            "unconnected-(P6-Pad1)",
            "unconnected-(P8-Pad1)",
            "Net-(P2-P1)",
            "Net-(U1B-K)",
            "unconnected-(P5-Pad1)",
            "Net-(U1A-K)",
            "Net-(P4-PM)",
            "Net-(P3-P1)",
            "Net-(U1A-G)",
            "Net-(P4-P1)",
            "GND",
            "Net-(P1-PM)",
        ],
        "this is java.util.HashSet's bucket order, not the pads' declaration order"
    );
}

/// `:341-406` — the default net class, its via info, its via rule and the `defaultVia` padstack,
/// on a board with no net classes at all. `ViaRule` owns its `ViaInfo`s since Plan 7 Task 0
/// (`bde59ef`), and the copy it owns must be the one `ViaInfos::add` stamped, so that
/// `ViaInfo::is_same_object` — Java's `==`, read by `ViaRule::contains` — answers `true`.
#[test]
fn the_default_via_rule_owns_the_registered_via_info() {
    let (board, _, _) = board_of("{}");
    assert_eq!(board.rules.via_infos.count(), 1);
    assert_eq!(board.rules.via_rules.len(), 1);
    let registered = board
        .rules
        .via_infos
        .get_by_name("defaultVia")
        .expect("added");
    let rule = &board.rules.via_rules[0];
    assert_eq!(rule.name, "default");
    assert_eq!(rule.via_count(), 1);
    assert!(
        rule.get_via(0).is_same_object(registered),
        "Java's `viaRule.appendVia(viaInfo)` shares the object `viaInfos.add(viaInfo)` registered"
    );
    // The default net class points at the same rule, and the padstack is a via padstack.
    let default_class = board.rules.net_classes.get(NetClassId(0));
    assert_eq!(
        default_class.get_via_rule().map(|r| r.name.as_str()),
        Some("default")
    );
    assert_eq!(board.library.get_via_padstacks().len(), 1);
    assert_eq!(
        board
            .library
            .padstacks
            .get(registered.get_padstack())
            .expect("resolves")
            .name,
        "defaultVia"
    );
    // `DefaultItemClearanceClasses.ItemClass.VIA` is 1 on a freshly built class (`:396-398`).
    assert_eq!(
        DefaultItemClearanceClasses::new().get(ItemClass::Via),
        registered.get_clearance_class_index()
    );
}

/// `:450-451`'s `boardRules.netClasses.get(clNo - 1)`, with Java's own comment "NetClass array
/// indices are 0-based": `clNo` is a **clearance** class index whose rows 0 and 1 are `"null"`
/// and `"default"`, while the net-class list has no `"null"` entry — so the subtraction is
/// deliberate, not a slip. `resolveNetClassIndex` returns 1 for KiCad's own default spellings and
/// for anything it cannot resolve, both of which land on net class 0.
#[test]
fn the_net_class_index_is_deliberately_off_by_one() {
    let json = r#"{"unit":"MIL","resolution":1.0,
        "netClasses":[{"name":"Default","clearance":10.0},{"name":"HV","clearance":40.0}],
        "nets":[{"name":"GND","className":"Default"},
                {"name":"HV1","className":"hv"},
                {"name":"NC","className":"nope"},
                {"name":"NN","className":null}],
        "outline":{"corners":[{"x":0.0,"y":0.0},{"x":1.0,"y":0.0},{"x":1.0,"y":1.0}]}}"#;
    let (board, _, _) = board_of(json);
    let class_of = |name: &str| {
        let net = board
            .rules
            .nets
            .get_by_name_and_subnet(name, 1)
            .expect("declared");
        board
            .rules
            .net_classes
            .get(net.get_net_class())
            .get_name()
            .to_string()
    };
    // `"Default"` is KiCad's default spelling -> clNo 1 -> net class 0.
    assert_eq!(class_of("GND"), "default");
    // `"hv"` misses the exact `HashMap.get` and is found by the `equalsIgnoreCase` scan -> 2 -> 1.
    assert_eq!(class_of("HV1"), "HV");
    // Unresolvable, and `null`, both fall through to `return 1` -> net class 0.
    assert_eq!(class_of("NC"), "default");
    assert_eq!(class_of("NN"), "default");
}

/// `:126` — `clearanceClassCount = Math.max(2, additionalNetClasses.size() + 2)`, with
/// `clearanceClassNames[0] = "null"` (the four-character **string**) and `[1] = "default"`.
#[test]
fn the_two_reserved_clearance_rows_are_null_and_default() {
    let (board, _, _) = board_of("{}");
    let matrix = &board.rules.clearance_matrix;
    assert_eq!(matrix.get_class_count(), 2);
    assert_eq!(matrix.get_name(0), Some("null"));
    assert_eq!(matrix.get_name(1), Some("default"));
    // Class 0 keeps its zeros: `setDefaultValue`'s loops both start at 1.
    assert_eq!(matrix.get_value(0, 0, 0, false), 0);
    assert_eq!(matrix.get_value(0, 1, 0, false), 0);
    // `round(scale(0.2, MM, MM) * 10000) == 2000`.
    assert_eq!(matrix.get_value(1, 1, 0, false), 2000);
}

/// `:313` — the reader builds its **own** `CoordinateTransform(scaleFactor, 0, 0)`, and the port
/// rides it out on `BoardReadResult::Success` so `LoadedBoard` (Plan 8 Task 3) can carry it to
/// the writer. Java's record has no such member and its writers re-derive the transform.
#[test]
fn the_reader_builds_its_own_coordinate_transform() {
    for (json, expected) in [
        ("{}", 10000.0),
        (r#"{"unit":"MIL","resolution":1.0}"#, 1.0),
        (r#"{"unit":"UM","resolution":10.0}"#, 10.0),
        (r#"{"resolution":2.75}"#, 2.0), // `(int) Math.max(1.0, 2.75)` truncates
    ] {
        let BoardReadResult::Success {
            coordinate_transform,
            ..
        } = read_board(json, None)
        else {
            panic!("expected Success for {json}");
        };
        let transform = coordinate_transform.expect("section 6 built one");
        assert_eq!(transform.scale_factor(), expected, "for {json}");
        assert_eq!(transform.base_x(), 0.0);
        assert_eq!(transform.base_y(), 0.0);
    }
}

/// `:170-278` — the missing-outline branch: a board with no items at all falls back to the
/// `1000 x 1000` internal-unit region at `:245-252`, pads it by `scale(5mm, MM, unit)`, negates Y
/// and offsets the bounding box by `1000` (`:278`). The warning `:740-742` adds says "**was**
/// generated", not the `FRLogger.warn` at `:173-175`'s "**will be** generated for routing".
#[test]
fn a_missing_outline_generates_a_padded_box_and_one_warning() {
    let (board, _, warnings) = board_of("{}");
    assert_eq!(
        warnings,
        [
            "Board Outline/Boundary is missing or empty in the JSON file. A supposed board edge \
          around components with 5mm padding was generated."
        ]
    );
    // Transcript stem `empty-object`.
    let bbox = board.bounding_box;
    assert_eq!(
        (bbox.ll.x, bbox.ll.y, bbox.ur.x, bbox.ur.y),
        (-51000, -51000, 52000, 52000)
    );
    // Fewer than three corners is "missing" too, and then the *items* set the extent.
    let json = r#"{"outline":{"corners":[{"x":0.0,"y":0.0},{"x":10.0,"y":10.0}]}}"#;
    let (_, _, warnings) = board_of(json);
    assert_eq!(warnings.len(), 1);
    // Three corners is enough, and produces no warning.
    let json =
        r#"{"outline":{"corners":[{"x":0.0,"y":0.0},{"x":10.0,"y":0.0},{"x":5.0,"y":9.0}]}}"#;
    let (_, _, warnings) = board_of(json);
    assert!(warnings.is_empty());
}

// ================================================= Task 9's named behaviours (sections 9-11)

/// A two-layer 50x40 mm board at resolution 1000, plus whatever `body` adds — the same helper
/// `P8T8Probe.board` builds `CORPUS_B`'s synthetic stems with, so a literal here and a transcript
/// row there describe the same input.
fn kicad_board(body: &str) -> String {
    format!(
        "{{\"unit\":\"MM\",\"resolution\":1000.0,\
           \"layers\":[{{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"}},\
           {{\"index\":1,\"name\":\"B.Cu\",\"type\":\"signal\"}}],\
           \"outline\":{{\"corners\":[{{\"x\":0.0,\"y\":0.0}},{{\"x\":50.0,\"y\":0.0}},\
           {{\"x\":50.0,\"y\":40.0}},{{\"x\":0.0,\"y\":40.0}}]}},{body}}}"
    )
}

/// One component with one pad of the given shape and size, on `footprint`.
fn kicad_component(reference: &str, footprint: &str, pad: &str) -> String {
    format!(
        "{{\"reference\":\"{reference}\",\"value\":\"v\",\"footprint\":\"{footprint}\",\
          \"position\":{{\"x\":10.0,\"y\":10.0}},\"rotation\":0.0,\"layer\":\"F.Cu\",\
          \"pads\":[{pad}]}}"
    )
}

/// **`:583-620`, the package-dedup ladder.** Two components with the same `footprint` and
/// pin-identical pads share one library package; a third whose pads differ gets `"<base>::1"`.
///
/// This is `arePackagePinsIdentical` (`:892-924`) doing its job: get it wrong and the library
/// either duplicates or merges packages, which is visible in the package count and in the SES.
#[test]
fn identical_packages_are_reused() {
    let pad = |sx: f64, sy: f64| {
        format!(
            "{{\"name\":\"1\",\"netName\":\"GND\",\"shape\":\"rect\",\
              \"size\":{{\"x\":{sx},\"y\":{sy}}},\"offset\":{{\"x\":0.0,\"y\":0.0}},\
              \"drill\":0.0,\"layers\":[]}}"
        )
    };
    let two_identical = kicad_board(&format!(
        "\"components\":[{},{}]",
        kicad_component("U1", "SO8", &pad(1.0, 2.0)),
        kicad_component("U2", "SO8", &pad(1.0, 2.0))
    ));
    let (board, _, _) = board_of(&two_identical);
    assert_eq!(board.components.count(), 2);
    assert_eq!(
        board.library.packages.count(),
        1,
        "pin-identical packages under one footprint are reused (`:599-601`)"
    );
    assert_eq!(board.library.packages.get(1).name, "SO8");

    // A third component whose pad is a different size is **not** pin-identical, so the ladder
    // moves to `SO8::1` — `Packages.get("SO8::1", …)` strips the suffix back to `SO8`
    // (Packages.java:40), whose name then fails `:589`'s `equalsIgnoreCase`, so `:590` adds it.
    let three = kicad_board(&format!(
        "\"components\":[{},{},{}]",
        kicad_component("U1", "SO8", &pad(1.0, 2.0)),
        kicad_component("U2", "SO8", &pad(3.0, 4.0)),
        kicad_component("U3", "SO8", &pad(5.0, 6.0))
    ));
    let (board, _, _) = board_of(&three);
    assert_eq!(board.library.packages.count(), 3);
    let names: Vec<&str> = (1..=3)
        .map(|no| board.library.packages.get(no).name.as_str())
        .collect();
    assert_eq!(names, ["SO8", "SO8::1", "SO8::2"]);
}

/// **`:603`'s `catch (Exception e)` — recovery boundary 1, and the task brief's one factual
/// error.**
///
/// The brief said this arm "skips one component and continues". It does not: it wraps *only* the
/// package-dedup lookup, adds a **duplicate** package under the base name and lets the component
/// through. Java wins; measured on the `pad-null-name-dedup` stem, which this test replays as a
/// literal.
///
/// The throw it catches is `arePackagePinsIdentical:908`'s `pin1.name.equals(pin2.name)` over a
/// `null` pin name — a pad with no `"name"` key. So three components sharing one footprint end
/// with **three** packages all called `NONAME`, of which `Packages.get` can only ever reach the
/// first (quirk #285).
#[test]
fn a_package_dedup_failure_falls_back_to_a_duplicate_package() {
    let nameless_pad = "{\"netName\":\"N\",\"shape\":\"rect\",\
                        \"size\":{\"x\":1.0,\"y\":1.0},\"drill\":0.0}";
    let json = kicad_board(&format!(
        "\"components\":[{},{},{}]",
        kicad_component("U1", "NONAME", nameless_pad),
        kicad_component("U2", "NONAME", nameless_pad),
        kicad_component("U3", "NONAME", nameless_pad)
    ));
    let (board, _, _) = board_of(&json);
    assert_eq!(
        board.components.count(),
        3,
        "the catch does NOT skip the component — every one of the three loads"
    );
    assert_eq!(
        board.library.packages.count(),
        3,
        "each retry adds a duplicate package under the base name (`:605-617`)"
    );
    for no in 1..=3 {
        assert_eq!(board.library.packages.get(no).name, "NONAME");
    }
    // `Packages.get` answers the first of the three, so the other two are unreachable by name.
    assert_eq!(
        board
            .library
            .packages
            .get_by_name("NONAME", true)
            .expect("one of the three")
            .no,
        1
    );
    // The same board with **named** pads takes the ordinary path and ends with one package.
    let named_pad = "{\"name\":\"1\",\"netName\":\"N\",\"shape\":\"rect\",\
                     \"size\":{\"x\":1.0,\"y\":1.0},\"drill\":0.0}";
    let json = kicad_board(&format!(
        "\"components\":[{},{},{}]",
        kicad_component("U1", "NONAME", named_pad),
        kicad_component("U2", "NONAME", named_pad),
        kicad_component("U3", "NONAME", named_pad)
    ));
    let (board, _, _) = board_of(&json);
    assert_eq!(board.library.packages.count(), 1);
}

/// **`:746`'s `catch (Throwable e)` — recovery boundary 2.** Every point sections 9-11 can throw
/// from comes back as `ParseError("json_payload", "Exception occurred: …")` with Java's own
/// message, and the reader never panics.
///
/// The eleven inputs are the literals of `p8t8-kicad-read-b.txt`'s corresponding stems; the
/// transcript replay compares the same rows, and this test is the one that reads as a list of
/// what the boundary covers.
#[test]
fn a_malformed_document_answers_parse_error() {
    let cases: &[(&str, &str)] = &[
        // The four unguarded list dereferences sections 9-11 add.
        (
            r#"{"components": null}"#,
            "Cannot invoke \"java.util.List.iterator()\" because \"boardJson.components\" is null",
        ),
        (
            r#"{"conductionAreas": null}"#,
            "Cannot invoke \"java.util.List.iterator()\" because \"boardJson.conductionAreas\" is null",
        ),
        (
            r#"{"traces": null}"#,
            "Cannot invoke \"java.util.List.iterator()\" because \"boardJson.traces\" is null",
        ),
        (
            r#"{"vias": null}"#,
            "Cannot invoke \"java.util.List.iterator()\" because \"boardJson.vias\" is null",
        ),
    ];
    for (json, detail) in cases {
        match read_board(json, None) {
            BoardReadResult::ParseError {
                location,
                detail: d,
            } => {
                assert_eq!(location, "json_payload", "for {json}");
                assert_eq!(d, format!("Exception occurred: {detail}"), "for {json}");
            }
            other => panic!("expected a ParseError for {json}, got {other:?}"),
        }
    }

    // The four field reads on a `null` `Point2D`, the two string throws, and the three
    // array-index throws — every one of them measured against the jar.
    let bodies: &[(&str, &str)] = &[
        (
            "\"components\":[{\"reference\":\"U1\",\"value\":\"v\",\"footprint\":\"P\",\
              \"position\":{\"x\":1.0,\"y\":1.0},\"rotation\":0.0,\"layer\":\"F.Cu\",\
              \"pads\":[{\"name\":\"1\",\"netName\":\"N\",\"shape\":\"rect\",\"size\":null,\
              \"drill\":0.0}]}]",
            "Cannot read field \"x\" because \"pad.size\" is null",
        ),
        (
            "\"components\":[{\"reference\":\"U1\",\"value\":\"v\",\"footprint\":\"P\",\
              \"position\":null,\"rotation\":0.0,\"layer\":\"F.Cu\",\
              \"pads\":[{\"name\":\"1\",\"netName\":\"N\",\"shape\":\"rect\",\
              \"size\":{\"x\":1.0,\"y\":1.0},\"drill\":0.0}]}]",
            "Cannot read field \"x\" because \"comp.position\" is null",
        ),
        (
            "\"vias\":[{\"id\":1,\"netName\":\"N\",\"position\":null,\"diameter\":0.8,\
              \"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":1}]",
            "Cannot read field \"x\" because \"vj.position\" is null",
        ),
        (
            "\"conductionAreas\":[{\"id\":1,\"netName\":\"N\",\"layerIndex\":0,\
              \"isObstacle\":false,\"polygon\":null}]",
            "Cannot invoke \"java.util.List.size()\" because \"zone.polygon\" is null",
        ),
        (
            "\"conductionAreas\":[{\"id\":1,\"netName\":\"N\",\"layerIndex\":0,\
              \"isObstacle\":false,\"polygon\":[]}]",
            // `new PolygonShape(new Point[0])` reads `corners[0]`.
            "Index 0 out of bounds for length 0",
        ),
        (
            "\"vias\":[{\"id\":1,\"netName\":\"N\",\"position\":{\"x\":3.0,\"y\":4.0},\
              \"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":5}]",
            // `shapes[li] = viaShape` with `li == layerCount`.
            "Index 2 out of bounds for length 2",
        ),
        (
            "\"vias\":[{\"id\":1,\"netName\":\"N\",\"position\":{\"x\":3.0,\"y\":4.0},\
              \"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":1,\"endLayerIndex\":0}]",
            // Quirk #286: every shape null, so `DrillItem.tileShapeCount` is `-layerCount`.
            "-2",
        ),
        (
            "\"components\":[{\"reference\":null,\"value\":\"v\",\"footprint\":\"P\",\
              \"position\":{\"x\":1.0,\"y\":1.0},\"rotation\":0.0,\"layer\":\"F.Cu\",\
              \"pads\":[{\"name\":\"1\",\"netName\":\"N\",\"shape\":\"rect\",\
              \"size\":{\"x\":1.0,\"y\":1.0},\"drill\":0.0}]}]",
            // Quirk #287: `UndoableObjects.insert`'s skip list orders through `Component.compareTo`.
            "Cannot invoke \"String.compareToIgnoreCase(String)\" because \"this.name\" is null",
        ),
        (
            "\"components\":[{\"reference\":\"U1\",\"value\":\"v\",\"footprint\":\"P\",\
              \"position\":{\"x\":1.0,\"y\":1.0},\"rotation\":0.0,\"layer\":\"F.Cu\",\
              \"pads\":[{\"name\":\"1\",\"netName\":\"N\",\"shape\":\"\",\
              \"size\":{\"x\":1.0,\"y\":1.0},\"drill\":0.0}]}]",
            // `:868`'s `pad.shape.substring(0, 1)` on an empty shape name.
            "Range [0, 1) out of bounds for length 0",
        ),
    ];
    for (body, detail) in bodies {
        let json = kicad_board(body);
        match read_board(&json, None) {
            BoardReadResult::ParseError {
                location,
                detail: d,
            } => {
                assert_eq!(location, "json_payload", "for {body}");
                assert_eq!(d, format!("Exception occurred: {detail}"), "for {body}");
            }
            other => panic!("expected a ParseError for {body}, got {other:?}"),
        }
    }
}

/// **Quirk #282, verified.** Task 8 recorded that a `null` layer or net-class `name` is stored and
/// "only crashes in section 9, at `:545`", and handed Task 9 the obligation of checking that claim
/// while writing the line. It holds, and it is narrower and wider than the row said:
///
/// * **narrower** — `:545` fires only when a pad's `layers` list is *non-empty*. The same board
///   with `"layers": []` loads, because `:539`'s guard skips the whole comparison loop;
/// * **wider** — a `null` **net** name crashes earlier still, in `Nets.get`'s own
///   `currentNet.name.equalsIgnoreCase(name)` (Nets.java:44), which section 8's auto-registration
///   loop already reaches at `:491`. A board with no referenced nets at all defers that to
///   section 9's `:639`, and both answer the same message.
#[test]
fn a_null_layer_name_crashes_in_section_9_exactly_where_quirk_282_says() {
    let board_with_null_layer = |pad_layers: &str| {
        format!(
            "{{\"unit\":\"MM\",\"resolution\":1000.0,\
               \"layers\":[{{\"index\":0,\"name\":null,\"type\":\"signal\"}},\
               {{\"index\":1,\"name\":\"B.Cu\",\"type\":\"signal\"}}],\
               \"outline\":{{\"corners\":[{{\"x\":0.0,\"y\":0.0}},{{\"x\":50.0,\"y\":0.0}},\
               {{\"x\":50.0,\"y\":40.0}},{{\"x\":0.0,\"y\":40.0}}]}},\
               \"components\":[{{\"reference\":\"U1\",\"value\":\"v\",\"footprint\":\"P\",\
               \"position\":{{\"x\":10.0,\"y\":10.0}},\"rotation\":0.0,\"layer\":\"F.Cu\",\
               \"pads\":[{{\"name\":\"1\",\"netName\":\"N\",\"shape\":\"rect\",\
               \"size\":{{\"x\":1.0,\"y\":1.0}},\"drill\":0.0,\"layers\":{pad_layers}}}]}}]}}"
        )
    };
    match read_board(&board_with_null_layer("[\"B.Cu\"]"), None) {
        BoardReadResult::ParseError { location, detail } => {
            assert_eq!(location, "json_payload");
            assert_eq!(
                detail,
                "Exception occurred: Cannot invoke \"String.equalsIgnoreCase(String)\" \
                 because \"boardLayers[li].name\" is null"
            );
        }
        other => panic!("expected the `:545` crash, got {other:?}"),
    }
    // The narrow half: an empty pad `layers` list never enters `:540-550`.
    let (board, _, _) = board_of(&board_with_null_layer("[]"));
    assert_eq!(board.components.count(), 1);
    assert_eq!(
        board.layer_structure().layers[0].name,
        "",
        "totalized `null`"
    );

    // The wide half: a `null` **net** name dies in `Nets.get`, at `:491` when anything references
    // a net and at `:639` when nothing does. Same message either way.
    for pad_net in ["N", ""] {
        let json = format!(
            "{{\"unit\":\"MM\",\"resolution\":1000.0,\
               \"nets\":[{{\"id\":1,\"name\":null,\"className\":null,\"containsPlane\":false}}],\
               \"outline\":{{\"corners\":[{{\"x\":0.0,\"y\":0.0}},{{\"x\":50.0,\"y\":0.0}},\
               {{\"x\":50.0,\"y\":40.0}},{{\"x\":0.0,\"y\":40.0}}]}},\
               \"components\":[{{\"reference\":\"U1\",\"value\":\"v\",\"footprint\":\"P\",\
               \"position\":{{\"x\":10.0,\"y\":10.0}},\"rotation\":0.0,\"layer\":\"F.Cu\",\
               \"pads\":[{{\"name\":\"1\",\"netName\":\"{pad_net}\",\"shape\":\"rect\",\
               \"size\":{{\"x\":1.0,\"y\":1.0}},\"drill\":0.0}}]}}]}}"
        );
        match read_board(&json, None) {
            BoardReadResult::ParseError { detail, .. } => assert_eq!(
                detail,
                "Exception occurred: Cannot invoke \"String.equalsIgnoreCase(String)\" \
                 because \"currentNet.name\" is null"
            ),
            other => panic!("expected the Nets.get crash, got {other:?}"),
        }
    }
}

/// **`getDescriptivePadstackName` (`:857-890`) — a name-generating function whose output reaches
/// the SES.** Every literal below is the jar's, from `p8t8-kicad-read-b.txt`.
///
/// Three things it pins that nothing else does:
///
/// * the shape word: `Round` for `circle`/`round` *and* for a `null` shape, `Rect` for
///   `rect`/`rectangle`, `Oval` for `oval`, and `Ucfirst`-then-lowercase for anything else;
/// * the `[T]`/`[B]`/`[A]` layer discriminator, which is `T`/`B` only for a **single**-element
///   `layers` list naming the first or last board layer;
/// * the `%.0f`s, which are `java.util.Formatter`'s **HALF_UP** over the shortest round-trip
///   digits, not Rust's half-to-even: `0.0005 mm` is `1`, `0.0025` is `3`, `0.0035` is `4`.
#[test]
fn the_generated_padstack_names_match_the_jar() {
    let pad = |name: &str, shape: &str, sx: f64, sy: f64, layers: &str| {
        format!(
            "{{\"name\":\"{name}\",\"netName\":\"N\",\"shape\":{shape},\
              \"size\":{{\"x\":{sx},\"y\":{sy}}},\"offset\":{{\"x\":0.0,\"y\":0.0}},\
              \"drill\":0.0,\"layers\":{layers}}}"
        )
    };
    let pads = [
        pad("1", "\"circle\"", 1.0, 2.0, "[]"),
        pad("2", "\"ROUND\"", 3.0, 2.0, "[]"),
        pad("3", "\"oval\"", 1.0, 2.0, "[]"),
        pad("4", "\"rect\"", 1.0, 2.0, "[]"),
        pad("5", "\"rectangle\"", 4.0, 5.0, "[]"),
        pad("6", "\"trapezoid\"", 1.0, 2.0, "[]"),
        pad("7", "null", 6.0, 2.0, "[]"),
        pad("8", "\"rect\"", 7.0, 7.0, "[\"f.cu\"]"),
        pad("9", "\"rect\"", 8.0, 8.0, "[\"B.Cu\"]"),
        pad("10", "\"rect\"", 9.0, 9.0, "[\"B.Cu\",\"F.Cu\"]"),
        // HALF_UP, all three of them.
        pad("11", "\"circle\"", 0.0005, 0.0005, "[]"),
        pad("12", "\"rect\"", 0.0025, 0.0035, "[]"),
    ];
    let json = kicad_board(&format!(
        "\"components\":[{{\"reference\":\"U1\",\"value\":\"v\",\"footprint\":\"P\",\
          \"position\":{{\"x\":10.0,\"y\":10.0}},\"rotation\":0.0,\"layer\":\"F.Cu\",\
          \"pads\":[{}]}}]",
        pads.join(",")
    ));
    let (board, _, _) = board_of(&json);
    // `defaultVia` is section 8's; every later padstack is section 9's, in pad order, deduplicated
    // by name.
    let names: Vec<&str> = (1..=board.library.padstacks.count())
        .map(|no| {
            board
                .library
                .padstacks
                .get(fr_board::PadstackId(no))
                .expect("1..=count")
                .name
                .as_str()
        })
        .collect();
    assert_eq!(
        names,
        [
            "defaultVia",
            "Round[A]Pad_1000_um",
            "Round[A]Pad_3000_um",
            "Oval[A]Pad_1000x2000_um",
            "Rect[A]Pad_1000x2000_um",
            "Rect[A]Pad_4000x5000_um",
            "Trapezoid[A]Pad_1000x2000_um",
            "Round[A]Pad_6000_um",
            "Rect[T]Pad_7000x7000_um",
            "Rect[B]Pad_8000x8000_um",
            "Rect[A]Pad_9000x9000_um",
            "Round[A]Pad_1_um",
            "Rect[A]Pad_3x4_um",
        ]
    );

    // The via names `:707-711` generates take the same `%.0f`.
    let json = kicad_board(
        "\"vias\":[{\"id\":1,\"netName\":\"N\",\"position\":{\"x\":3.0,\"y\":4.0},\
          \"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":1},\
          {\"id\":2,\"netName\":\"N\",\"position\":{\"x\":9.0,\"y\":2.0},\
          \"diameter\":0.0035,\"drill\":0.0015,\"startLayerIndex\":0,\"endLayerIndex\":0}]",
    );
    let (board, _, _) = board_of(&json);
    assert_eq!(
        board
            .library
            .padstacks
            .get(fr_board::PadstackId(2))
            .expect("the first via padstack")
            .name,
        "Via[0-1]_800:400_um"
    );
    assert_eq!(
        board
            .library
            .padstacks
            .get(fr_board::PadstackId(3))
            .expect("the second via padstack")
            .name,
        "Via[0-0]_4:2_um"
    );
}

/// **Quirk #284**: the generated name encodes the pad's *shape word*, a single-layer T/B/A
/// discriminator and its two dimensions — and neither the **layer span** nor the **drill flag**.
/// So `Padstacks.get(name)` hands a later pad the earlier one's padstack, shapes and all.
///
/// The `mid` pad below lives on `In1.Cu` alone and ends up on the padstack the `span` pad built
/// across all three layers; the `drilled` pad ends up on the undrilled one's.
#[test]
fn the_padstack_name_encodes_neither_the_layer_span_nor_the_drill() {
    let pad = |name: &str, drill: f64, layers: &str| {
        format!(
            "{{\"name\":\"{name}\",\"netName\":\"N\",\"shape\":\"rect\",\
              \"size\":{{\"x\":1.0,\"y\":1.0}},\"offset\":{{\"x\":0.0,\"y\":0.0}},\
              \"drill\":{drill},\"layers\":{layers}}}"
        )
    };
    let json = format!(
        "{{\"unit\":\"MM\",\"resolution\":1000.0,\
           \"layers\":[{{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"}},\
           {{\"index\":1,\"name\":\"In1.Cu\",\"type\":\"signal\"}},\
           {{\"index\":2,\"name\":\"B.Cu\",\"type\":\"signal\"}}],\
           \"outline\":{{\"corners\":[{{\"x\":0.0,\"y\":0.0}},{{\"x\":50.0,\"y\":0.0}},\
           {{\"x\":50.0,\"y\":40.0}},{{\"x\":0.0,\"y\":40.0}}]}},\
           \"components\":[{{\"reference\":\"U1\",\"value\":\"v\",\"footprint\":\"P\",\
           \"position\":{{\"x\":10.0,\"y\":10.0}},\"rotation\":0.0,\"layer\":\"F.Cu\",\
           \"pads\":[{},{},{}]}}]}}",
        pad("span", 0.0, "[\"B.Cu\",\"F.Cu\"]"),
        pad("mid", 0.0, "[\"In1.Cu\"]"),
        pad("drilled", 0.5, "[\"B.Cu\",\"F.Cu\"]")
    );
    let (board, _, _) = board_of(&json);
    let package = board.library.packages.get(1);
    let span = package.get_pin(0).expect("span").padstack_no;
    assert_eq!(
        package.get_pin(1).expect("mid").padstack_no,
        span,
        "an In1.Cu-only pad silently inherits the all-layer padstack (quirk #284)"
    );
    assert_eq!(
        package.get_pin(2).expect("drilled").padstack_no,
        span,
        "and so does a drilled pad of the same size"
    );
    let padstack = board.library.padstacks.get(span).expect("the shared one");
    assert_eq!(padstack.name, "Rect[A]Pad_1000x1000_um");
    assert_eq!((padstack.from_layer(), padstack.to_layer()), (0, 2));
    assert!(
        !padstack.attach_allowed,
        "the first pad was undrilled, and the name carries no drill flag to distinguish them"
    );
}
