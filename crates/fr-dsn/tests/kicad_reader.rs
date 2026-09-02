//! `fr_dsn::kicad` — the KiCad board-JSON DTO tree and `readBoard`'s sections 1-8.
//!
//! Java authority: `io/kicad/KiCadBoardJson.java` and `io/kicad/KiCadJsonReader.java:61-497`.
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
//! Rust board and requires **zero** differing lines. The probe's `[s9]` rows — padstacks beyond
//! section 8's, packages, components and the item count — are Task 9's surface and are not
//! compared; the file carries them so Task 9 has its target.
//!
//! The named tests after it pin the four behaviours the task brief calls out by name, as literals.

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

/// The four **unguarded** list dereferences (`:105`, `:124`, `:156`, `:169`, `:446`) reach Java's
/// `catch (Throwable)` and come back as `ParseError("json_payload", …)`; the four guarded ones
/// (`components`, `traces`, `vias`, `conductionAreas`) do not. Quirk #277 covers the `detail`
/// text, which is the JVM's helpful-NPE string on one side and the port's reconstruction on the
/// other; the *shape* of the answer is what parity needs and what this pins.
#[test]
fn a_null_list_is_a_parse_error_exactly_where_java_leaves_it_unguarded() {
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
    for key in ["components", "traces", "vias", "conductionAreas"] {
        let json = format!("{{\"{key}\": null}}");
        assert!(
            matches!(read_board(&json, None), BoardReadResult::Success { .. }),
            "{key} is guarded and must load"
        );
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
