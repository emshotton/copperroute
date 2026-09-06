use std::fmt::Write as _;

use fr_board::{Board, DefaultItemClearanceClasses, Item, ItemClass, NetClassId, Unit};
use fr_dsn::error::{BoardMetadata, BoardReadResult};
use fr_dsn::format::format_double;
use fr_dsn::kicad::{UnitJson, read_board};
use fr_geometry::PolylineShapeRef;

const TRANSCRIPT: &str = include_str!("data/p8t8-kicad-read-a.txt");

fn fixture(relative: &str) -> String {
    let root = std::env::var("FREEROUTING_JAVA_DIR").unwrap_or_else(|_| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../../freerouting").to_string()
    });
    std::fs::read_to_string(format!("{root}/{relative}"))
        .unwrap_or_else(|e| panic!("fixture {relative}: {e}"))
}

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
        BoardReadResult::Partial { diagnostic, .. } => {
            rows.push(format!(
                "result=Partial diagnostic={}",
                escape(Some(diagnostic))
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

    let layers = &board.layer_structure().layers;
    rows.push(format!("layers count={}", layers.len()));
    for (i, layer) in layers.iter().enumerate() {
        rows.push(format!(
            "layer {i} name={} signal={}",
            escape(Some(&layer.name)),
            layer.is_signal
        ));
    }

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
                                format_double(point.x),
                                format_double(point.y)
                            ));
                        }
                    }
                    other => panic!("the KiCad reader only ever builds PolygonShapes: {other:?}"),
                }
            }
        }
        Some(other) => panic!("get_outline returned a non-outline item: {other:?}"),
    }

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
        format_double(transform.scale_factor()),
        format_double(transform.base_x()),
        format_double(transform.base_y())
    ));

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
                    item_class_name(*item_class),
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
            format_double(net_class.get_minimum_trace_length()),
            format_double(net_class.get_maximum_trace_length()),
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
        angle_restriction_name(board.rules.trace_angle_restriction),
        board.rules.get_ignore_conduction()
    ));

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
        angle_restriction_name(metadata.snap_angle)
    ));
    rows.push(format!("warnings count={}", warnings.len()));
    for (i, warning) in warnings.iter().enumerate() {
        rows.push(format!("warning {i} {}", escape(Some(warning))));
    }
    rows
}

fn item_class_name(item_class: ItemClass) -> &'static str {
    match item_class {
        ItemClass::None => "NONE",
        ItemClass::Trace => "TRACE",
        ItemClass::Via => "VIA",
        ItemClass::Pin => "PIN",
        ItemClass::Smd => "SMD",
        ItemClass::Area => "AREA",
    }
}

fn angle_restriction_name(angle: fr_board::AngleRestriction) -> &'static str {
    match angle {
        fr_board::AngleRestriction::None => "NONE",
        fr_board::AngleRestriction::FortyFiveDegree => "FORTYFIVE_DEGREE",
        fr_board::AngleRestriction::NinetyDegree => "NINETY_DEGREE",
    }
}

const PORT_TRANSCRIPT: &str = include_str!("data/p9t7-kicad-read-a.txt");

const PORT_TRANSCRIPT_B: &str = include_str!("data/p9t7-kicad-read-b.txt");

fn golden_rows(text: &str, prefix: &str) -> Vec<(String, Vec<String>)> {
    let mut cases: Vec<(String, Vec<String>)> = Vec::new();
    let row_prefix = format!("{prefix} ");
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("[case] stem=") {
            let stem = rest.split(' ').next().expect("a [case] line names a stem");
            cases.push((stem.to_string(), Vec::new()));
        } else if let Some(row) = line.strip_prefix(row_prefix.as_str()) {
            cases
                .last_mut()
                .expect("a row follows a [case] line")
                .1
                .push(row.to_string());
        }
    }
    cases
}

const KNOWN_DIVERGENCES: &[(&str, &str, &str)] = &[
    (
        "json-truncated",
        "#277",
        "the ParseError detail on a syntactically invalid payload is the JSON parser's own \
         message — Gson's `java.io.EOFException: End of input at line 1 column 2 path $.` against \
         serde_json's `EOF while parsing an object at line 1 column 1`. Same location, same \
         rejection, different prose. Not a fix: no port can reconstruct Gson's text.",
    ),
    (
        "trace-point-null-element",
        "#283",
        "`\"points\": [{...}, null]` is a `List<Point2D>` holding a null in Gson, and `:675`'s \
         `pt.x` then throws. `serde_json` refuses the null against `Vec<Point2D>` first, so the \
         port answers the same `location` with the deserializer's prose. Both reject the file.",
    ),
    (
        "outline-corner-null-element",
        "#283",
        "as `trace-point-null-element`, through section 5's `outline.corners`.",
    ),
    (
        "netclass-null-element",
        "#283",
        "`\"netClasses\": [null]` is a one-element list holding a null, and \
         `isKiCadDefaultNetClassName(netClass.name)` then throws. Same rejection, different prose.",
    ),
    (
        "zone-negative-layer",
        "#282-print",
        "totalized: `ObstacleArea.layer` is a Java `int` that `:663` fills from `zone.layerIndex` \
         verbatim, so Java keeps `-3`; `fr_board`'s layer is a `usize` holding the same 64 bits \
         and printing them unsigned. Nothing a KiCad export writes reaches it.",
    ),
    (
        "layer-null-name-with-pads",
        "#282",
        "a `null` layer name. Java dies at `:545` — 430 lines from the JSON that caused it, and \
         only because this board also has pads naming layers. The port refuses at the DTO \
         boundary, naming `layers[i].name`.",
    ),
    (
        "layer-null-name-no-pad-layers",
        "#282",
        "the **same** board with an empty pad `layers` list, which never reaches `:545` — so Java \
         loads it, with a layer whose name is `null`, and every later lookup against that layer \
         silently fails. The port refuses it too: the document is malformed either way.",
    ),
    (
        "net-null-name-with-pads",
        "#282",
        "a `null` net name. Java dies inside `Nets.get`'s walk, at the first lookup that reaches \
         the net. The port refuses at the boundary, naming `nets[i].name`.",
    ),
    (
        "net-null-name-pad-without-net",
        "#282",
        "the same board whose pad names no net, so Java's walk never reaches the null and the \
         board loads with a nameless net. Refused at the boundary.",
    ),
    (
        "comp-null-reference-single",
        "#287",
        "a `null` component `reference`. Java dies inside `ConcurrentSkipListMap.put`, through \
         `Component.compareTo`, before anything reads the component. The port refuses at the \
         boundary, naming `components[i].reference`.",
    ),
    (
        "comp-null-reference-second",
        "#287",
        "as `comp-null-reference-single`, with the null on the second component.",
    ),
    (
        "pad-null-name-dedup",
        "#282+#285",
        "three components whose pads have no `name`. Java's `:603` catch turns \
         `arePackagePinsIdentical`'s throw into a **duplicate package per component** — three \
         packages all called `NONAME`, silently. The port refuses the document, naming \
         `components[i].pads[j].name`; the deduplication case it was hiding is now testable, in \
         `kicad_packages.rs`.",
    ),
    (
        "pad-layers-null-element",
        "#283",
        "`\"layers\": [null, \"B.Cu\"]` — the one null element Java *tolerates*, because \
         `equalsIgnoreCase(null)` is `false`: the pad silently spans `B.Cu` only. The port refuses \
         it, naming `components[i].pads[j].layers[k]`.",
    ),
    (
        "pad-shape-arms",
        "#284",
        "every pad shape arm, so every generated padstack name: the `Round` form carries `size.y` \
         now, where `:883` dropped it and two round pads of different heights therefore shared a \
         padstack.",
    ),
    (
        "pad-layer-selection",
        "#284",
        "the `T`/`B`/`A` layer-type letter with pads on different spans — the case where Java's \
         name-keyed lookup hands the second pad the first pad's shapes.",
    ),
    (
        "pad-name-half-up",
        "#284",
        "the HALF_UP `%.0f` rounding is unchanged; what moved is the `Round` form's second \
         number, which the name now carries.",
    ),
    (
        "pad-layers-unmatched-only",
        "#284+#286",
        "a pad whose `layers` match no board layer, and nothing before it in the library. Java \
         built the all-`null` padstack, inserted the pin, and threw `NegativeArraySizeException` \
         from inside the search-tree update — reported as the bare number, `Exception occurred: \
         -2`. The port refuses before the padstack exists, naming the pad.",
    ),
    (
        "via-start-gt-end",
        "#286",
        "a via whose `startLayerIndex` is past its `endLayerIndex`, which is the same negative \
         count reached through `insertVia` — and on the `importSession` path Java keeps a via that \
         is in the item list and in no search tree.",
    ),
    (
        "referenced-nets-only",
        "#280",
        "seventeen nets, none declared, all auto-registered: the stem that exists to measure \
         `java.util.HashSet`'s iteration order. They are numbered 1..17 in first-reference order \
         now.",
    ),
    (
        "ecc83-v1",
        "#280",
        "the fixture the fix list names: thirteen pad nets, none declared. This renumbering is \
         what moves `tests/reference/cli-kicad-ecc83-json/`.",
    ),
    (
        "ecc83-v2",
        "#280+#284",
        "the same board's v2 export — its nets renumber, and its 24 round pads take the two-number \
         name.",
    ),
    (
        "interf-u",
        "#284",
        "173 referenced nets, all declared, so #280 does not touch it; its 158 round pads take the \
         two-number name.",
    ),
    (
        "complex-hierarchy",
        "#284",
        "52 declared nets, so #280 does not touch it either; its 54 round pads take the two-number \
         name. This is the second CLI stem, and its SES does not carry a pad padstack name, which \
         is why the golden does not move.",
    ),
    (
        "corney-island",
        "#284",
        "ten round pads, five distinct names.",
    ),
    ("traces", "#280", "three trace nets, none declared."),
    (
        "mixed",
        "#280",
        "a board mixing declared and referenced nets: the declared ones keep their numbers and \
         the referenced ones follow in first-reference order.",
    ),
];

#[test]
fn the_whole_section_1_to_8_surface_matches_the_port_golden() {
    assert_golden(
        &transcript_cases(),
        &golden_rows(PORT_TRANSCRIPT, "[s8]"),
        emit,
    );
}

#[test]
fn the_port_golden_differs_from_the_jar_only_where_a_fix_says_so() {
    assert_divergences(
        &golden_rows(TRANSCRIPT, "[s8]"),
        &golden_rows(PORT_TRANSCRIPT, "[s8]"),
        "part A",
    );
}

fn assert_golden(
    cases: &[Case],
    golden: &[(String, Vec<String>)],
    emit_rows: fn(&BoardReadResult) -> Vec<String>,
) {
    assert_eq!(
        cases.len(),
        golden.len(),
        "the port golden must cover the same inputs as the jar transcript"
    );
    let mut diffs: Vec<String> = Vec::new();
    let mut compared = 0usize;
    for (case, (stem, expected)) in cases.iter().zip(golden) {
        assert_eq!(
            &case.stem, stem,
            "the two files list the stems in one order"
        );
        let actual = emit_rows(&read_board(&case.json, None));
        for (i, expected) in expected.iter().enumerate() {
            compared += 1;
            match actual.get(i) {
                Some(row) if row == expected => {}
                Some(row) => diffs.push(format!(
                    "{stem}[{i}]\n  golden: {expected}\n  rust:   {row}"
                )),
                None => diffs.push(format!("{stem}[{i}] missing\n  golden: {expected}")),
            }
        }
        for row in actual.iter().skip(expected.len()) {
            diffs.push(format!("{stem} extra\n  rust: {row}"));
        }
    }
    assert!(
        diffs.is_empty(),
        "{} of {compared} rows differ from the port golden — regenerate it only if the change \
         was intended, and record the fix that caused it:\n{}",
        diffs.len(),
        diffs.join("\n")
    );
}

fn diverging_stems(jar: &[(String, Vec<String>)], port: &[(String, Vec<String>)]) -> Vec<String> {
    assert_eq!(jar.len(), port.len(), "the two files list the same inputs");
    jar.iter()
        .zip(port)
        .filter(|((stem, jar_rows), (port_stem, port_rows))| {
            assert_eq!(stem, port_stem, "the stems are in one order");
            jar_rows != port_rows
        })
        .map(|((stem, _), _)| stem.clone())
        .collect()
}

fn assert_divergences(jar: &[(String, Vec<String>)], port: &[(String, Vec<String>)], part: &str) {
    let mut unexplained: Vec<String> = Vec::new();
    for ((stem, jar_rows), (_, port_rows)) in jar.iter().zip(port) {
        if jar_rows == port_rows || KNOWN_DIVERGENCES.iter().any(|(name, _, _)| name == stem) {
            continue;
        }
        let first = jar_rows
            .iter()
            .zip(port_rows)
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| jar_rows.len().min(port_rows.len()));
        unexplained.push(format!(
            "{stem}[{first}]\n  jar:  {}\n  port: {}",
            jar_rows.get(first).map_or("<none>", String::as_str),
            port_rows.get(first).map_or("<none>", String::as_str),
        ));
    }
    assert!(
        unexplained.is_empty(),
        "{part}: {} stem(s) differ from the jar with no KNOWN_DIVERGENCES entry — add one naming \
         the register row that authorizes it, or fix the port:\n{}",
        unexplained.len(),
        unexplained.join("\n")
    );
}

#[test]
fn every_known_divergence_still_differs() {
    let mut diverged = diverging_stems(
        &golden_rows(TRANSCRIPT, "[s8]"),
        &golden_rows(PORT_TRANSCRIPT, "[s8]"),
    );
    diverged.extend(diverging_stems(
        &golden_rows(TRANSCRIPT_B, "[s9]"),
        &golden_rows(PORT_TRANSCRIPT_B, "[s9]"),
    ));
    for (stem, row, reason) in KNOWN_DIVERGENCES {
        assert!(
            diverged.iter().any(|name| name == stem),
            "`{stem}` now MATCHES the jar in both parts — delete its KNOWN_DIVERGENCES entry \
             ({row}: {reason})"
        );
    }
}

#[test]
#[ignore = "a generator, not a check — see PORT_TRANSCRIPT for the command"]
fn emit_the_port_transcripts() {
    for (path, cases, prefix, emit_rows) in [
        (
            "p9t7-kicad-read-a.txt",
            transcript_cases(),
            "[s8]",
            emit as fn(&BoardReadResult) -> Vec<String>,
        ),
        (
            "p9t7-kicad-read-b.txt",
            transcript_b_cases(),
            "[s9]",
            emit_b as fn(&BoardReadResult) -> Vec<String>,
        ),
    ] {
        println!("===== {path} =====");
        println!(
            "# the PORT's rows over the inputs of data/p8t8-kicad-read-{}.txt, which stays as",
            if prefix == "[s8]" { "a" } else { "b" }
        );
        println!("# the jar's record. Plan 9 Task 7 moved this family to the port lane: see");
        println!("# kicad_reader.rs's PORT_TRANSCRIPT for why, and KNOWN_DIVERGENCES for where");
        println!("# the two differ and which register row authorizes each difference.");
        for case in cases {
            println!();
            println!("[case] stem={}", case.stem);
            for row in emit_rows(&read_board(&case.json, None)) {
                println!("{prefix} {row}");
            }
        }
    }
}

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

fn emit_shape(shape: Option<&fr_geometry::Shape>) -> String {
    use fr_geometry::{Shape, ShapeOps, TileShape};
    let Some(shape) = shape else {
        return "<null>".to_string();
    };
    if let Shape::Circle(circle) = shape {
        return format!(
            "Circle({},{},r={})",
            format_double(circle.center.to_float().x),
            format_double(circle.center.to_float().y),
            circle.radius
        );
    }
    let name = match shape {
        Shape::Tile(TileShape::Box(_)) => "IntBox",
        Shape::Tile(TileShape::Octagon(_)) => "IntOctagon",
        Shape::Tile(TileShape::Simplex(_)) => "Simplex",
        Shape::Polygon(_) => "PolygonShape",
        Shape::Circle(_) => unreachable!("handled above"),
    };
    format!("{name}{}", emit_corners(&shape.corner_approx_arr()))
}

fn emit_corners(corners: &[fr_geometry::FloatPoint]) -> String {
    let mut out = String::from("(");
    for (i, corner) in corners.iter().enumerate() {
        if i > 0 {
            out.push(';');
        }
        let _ = write!(
            out,
            "{},{}",
            format_double(corner.x),
            format_double(corner.y)
        );
    }
    out.push(')');
    out
}

fn fixed_state_name(state: fr_board::FixedState) -> &'static str {
    match state {
        fr_board::FixedState::Unfixed => "NOT_FIXED",
        fr_board::FixedState::ShoveFixed => "SHOVE_FIXED",
        fr_board::FixedState::UserFixed => "USER_FIXED",
        fr_board::FixedState::SystemFixed => "SYSTEM_FIXED",
    }
}

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
        BoardReadResult::Partial { diagnostic, .. } => {
            rows.push(format!(
                "result=Partial diagnostic={}",
                escape(Some(diagnostic))
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
                format_double(pin.relative_location.to_float().x),
                format_double(pin.relative_location.to_float().y),
                format_double(pin.rotation_in_degree)
            ));
        }
    }

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
                    format_double(location.to_float().x),
                    format_double(location.to_float().y)
                )
            },
        );
        rows.push(format!(
            "component {i} name={} id={} location={location} rotation={} onFront={} package={} \
             positionFixed={} partNumber={}",
            escape(Some(&component.name)),
            component.id,
            format_double(component.get_rotation_in_degree()),
            component.placed_on_front(),
            component.get_package(),
            component.position_fixed,
            escape(component.get_part_number())
        ));
    }

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
            fixed_state_name(header.get_fixed_state())
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
                        format!("{},{}", format_double(corner.x), format_double(corner.y))
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
                format_double(via.get_center().to_float().x),
                format_double(via.get_center().to_float().y),
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

#[test]
fn the_whole_section_9_to_11_item_graph_matches_the_port_golden() {
    assert_golden(
        &transcript_b_cases(),
        &golden_rows(PORT_TRANSCRIPT_B, "[s9]"),
        emit_b,
    );
}

#[test]
fn the_part_b_port_golden_differs_from_the_jar_only_where_a_fix_says_so() {
    assert_divergences(
        &golden_rows(TRANSCRIPT_B, "[s9]"),
        &golden_rows(PORT_TRANSCRIPT_B, "[s9]"),
        "part B",
    );
}

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

    assert_eq!(matrix.get_value(1, 2, 0, false), 78);
    assert_eq!(matrix.get_value(2, 1, 0, false), 8);
    assert_ne!(
        matrix.get_value(1, 2, 0, false),
        matrix.get_value(2, 1, 0, false),
        "quirk #83's asymmetry must survive a KiCad read"
    );
    for layer in 0..3 {
        assert_eq!(matrix.get_value(2, 2, layer, false), 40);
        assert_eq!(matrix.get_value(1, 1, layer, false), 10);
    }
}

#[test]
fn y_is_negated_and_rounded_away_from_zero() {
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
    assert_eq!(corners, [(11.0, -21.0), (11.0, -1.0), (1.0, -1.0)]);
    assert_eq!(f64::round(-20.5), -21.0, "the rounding the port uses");
    assert!(corners.iter().all(|(_, y)| *y <= 0.0), "{corners:?}");
}

#[test]
fn the_host_fallbacks_are_kicad_and_v10() {
    let outline =
        r#""outline":{"corners":[{"x":0.0,"y":0.0},{"x":1.0,"y":0.0},{"x":1.0,"y":1.0}]}"#;
    for json in [
        format!("{{{outline}}}"),
        format!(r#"{{"hostCad":null,"hostVersion":null,{outline}}}"#),
        format!(r#"{{"hostCad":"","hostVersion":"",{outline}}}"#),
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

    let json = format!(r#"{{"hostCad":"Altium","hostVersion":"v24",{outline}}}"#);
    let (board, metadata, _) = board_of(&json);
    assert_eq!(board.communication.host_cad.as_deref(), Some("Altium"));
    assert_eq!(board.communication.host_version.as_deref(), Some("v24"));
    assert_eq!(metadata.host_cad.as_deref(), Some("KiCad"));
    assert_eq!(metadata.host_version.as_deref(), Some("v10.0"));

    let json = format!("{{\"hostCad\":\"\u{a0}\",{outline}}}");
    let (board, _, _) = board_of(&json);
    assert_eq!(board.communication.host_cad.as_deref(), Some("KiCad"));
}

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
        assert_eq!(board.communication.resolution, 10000, "for {unit:?}");
    }

    for (unit, expected) in [("MIL", Unit::Mil), ("UM", Unit::Um)] {
        let json = format!(r#"{{"unit":"{unit}","resolution":1.0,{outline}}}"#);
        let (board, _, _) = board_of(&json);
        assert_eq!(board.communication.unit, expected);
        assert_eq!(
            board.communication.resolution, 1,
            "no MM default for {unit}"
        );
    }

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

#[test]
fn every_null_list_is_a_parse_error_somewhere_in_read_board() {
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
    assert!(matches!(
        read_board(r#"{"outline": null}"#, None),
        BoardReadResult::Success { .. }
    ));
    assert!(matches!(
        read_board(r#"{"outline": {"corners": null}}"#, None),
        BoardReadResult::ParseError { .. }
    ));
}

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

#[test]
fn the_auto_registered_nets_take_first_reference_order_not_java_hash_set_order() {
    let (board, _, _) = board_of(&fixture(
        "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.json",
    ));
    let names: Vec<&str> = (1..=board.rules.nets.max_net_number())
        .map(|no| board.rules.nets.get(no).expect("in range").name.as_str())
        .collect();
    const JAR: [&str; 13] = [
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
    ];
    assert_eq!(
        names,
        [
            "Net-(P3-P1)",
            "GND",
            "Net-(P2-P1)",
            "Net-(U1A-K)",
            "unconnected-(P5-Pad1)",
            "unconnected-(P6-Pad1)",
            "unconnected-(P7-Pad1)",
            "Net-(U1A-G)",
            "Net-(U1B-K)",
            "Net-(P1-PM)",
            "Net-(P4-P1)",
            "Net-(P4-PM)",
            "unconnected-(P8-Pad1)",
        ],
        "the order the pads first mention each name"
    );
    assert_ne!(names, JAR, "and it is not the jar's bucket order");
    let mut sorted_port = names.clone();
    sorted_port.sort_unstable();
    let mut sorted_jar = JAR.to_vec();
    sorted_jar.sort_unstable();
    assert_eq!(sorted_port, sorted_jar);
}

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
    assert_eq!(rule.get_via(0), registered);
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
    assert_eq!(
        DefaultItemClearanceClasses::new().get(ItemClass::Via),
        registered.get_clearance_class_index()
    );
}

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
    assert_eq!(class_of("GND"), "default");
    assert_eq!(class_of("HV1"), "HV");
    assert_eq!(class_of("NC"), "default");
    assert_eq!(class_of("NN"), "default");
}

#[test]
fn the_two_reserved_clearance_rows_are_null_and_default() {
    let (board, _, _) = board_of("{}");
    let matrix = &board.rules.clearance_matrix;
    assert_eq!(matrix.get_class_count(), 2);
    assert_eq!(matrix.get_name(0), Some("null"));
    assert_eq!(matrix.get_name(1), Some("default"));
    assert_eq!(matrix.get_value(0, 0, 0, false), 0);
    assert_eq!(matrix.get_value(0, 1, 0, false), 0);
    assert_eq!(matrix.get_value(1, 1, 0, false), 2000);
}

#[test]
fn the_reader_builds_its_own_coordinate_transform() {
    for (json, expected) in [
        ("{}", 10000.0),
        (r#"{"unit":"MIL","resolution":1.0}"#, 1.0),
        (r#"{"unit":"UM","resolution":10.0}"#, 10.0),
        (r#"{"resolution":2.75}"#, 2.0),
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
    let bbox = board.bounding_box;
    assert_eq!(
        (bbox.ll.x, bbox.ll.y, bbox.ur.x, bbox.ur.y),
        (-51000, -51000, 52000, 52000)
    );
    let json = r#"{"outline":{"corners":[{"x":0.0,"y":0.0},{"x":10.0,"y":10.0}]}}"#;
    let (_, _, warnings) = board_of(json);
    assert_eq!(warnings.len(), 1);
    let json =
        r#"{"outline":{"corners":[{"x":0.0,"y":0.0},{"x":10.0,"y":0.0},{"x":5.0,"y":9.0}]}}"#;
    let (_, _, warnings) = board_of(json);
    assert!(warnings.is_empty());
}

fn kicad_board(body: &str) -> String {
    format!(
        "{{\"unit\":\"MM\",\"resolution\":1000.0,\
           \"layers\":[{{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"}},\
           {{\"index\":1,\"name\":\"B.Cu\",\"type\":\"signal\"}}],\
           \"outline\":{{\"corners\":[{{\"x\":0.0,\"y\":0.0}},{{\"x\":50.0,\"y\":0.0}},\
           {{\"x\":50.0,\"y\":40.0}},{{\"x\":0.0,\"y\":40.0}}]}},{body}}}"
    )
}

fn kicad_component(reference: &str, footprint: &str, pad: &str) -> String {
    format!(
        "{{\"reference\":\"{reference}\",\"value\":\"v\",\"footprint\":\"{footprint}\",\
          \"position\":{{\"x\":10.0,\"y\":10.0}},\"rotation\":0.0,\"layer\":\"F.Cu\",\
          \"pads\":[{pad}]}}"
    )
}

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

#[test]
fn a_package_dedup_failure_is_refused_rather_than_falling_back() {
    let nameless_pad = "{\"netName\":\"N\",\"shape\":\"rect\",\
                        \"size\":{\"x\":1.0,\"y\":1.0},\"drill\":0.0}";
    let json = kicad_board(&format!(
        "\"components\":[{},{},{}]",
        kicad_component("U1", "NONAME", nameless_pad),
        kicad_component("U2", "NONAME", nameless_pad),
        kicad_component("U3", "NONAME", nameless_pad)
    ));
    match read_board(&json, None) {
        BoardReadResult::ParseError { location, detail } => {
            assert_eq!(location, "components");
            assert!(
                detail.contains("components[0].pads[0].name") && detail.contains("is null"),
                "the diagnostic names the pad that would have thrown: {detail}"
            );
        }
        other => panic!("expected the DTO refusal, got {other:?}"),
    }

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
    assert_eq!(board.components.count(), 3);
}

#[test]
fn a_malformed_document_answers_parse_error() {
    let cases: &[(&str, &str)] = &[
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
            "Index 0 out of bounds for length 0",
        ),
        (
            "\"vias\":[{\"id\":1,\"netName\":\"N\",\"position\":{\"x\":3.0,\"y\":4.0},\
              \"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":5}]",
            "Index 2 out of bounds for length 2",
        ),
        (
            "\"components\":[{\"reference\":\"U1\",\"value\":\"v\",\"footprint\":\"P\",\
              \"position\":{\"x\":1.0,\"y\":1.0},\"rotation\":0.0,\"layer\":\"F.Cu\",\
              \"pads\":[{\"name\":\"1\",\"netName\":\"N\",\"shape\":\"\",\
              \"size\":{\"x\":1.0,\"y\":1.0},\"drill\":0.0}]}]",
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

#[test]
fn a_null_layer_or_net_name_is_refused_at_the_boundary_not_in_section_9() {
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
    for pad_layers in ["[\"B.Cu\"]", "[]"] {
        match read_board(&board_with_null_layer(pad_layers), None) {
            BoardReadResult::ParseError { location, detail } => {
                assert_eq!(location, "layers", "for pad layers {pad_layers}");
                assert!(
                    detail.contains("KiCad board JSON file")
                        && detail.contains("layers[0].name")
                        && detail.contains("is null"),
                    "for pad layers {pad_layers}: {detail}"
                );
                assert!(
                    !detail.contains("boardLayers[li].name"),
                    "the crash 430 lines away is gone: {detail}"
                );
            }
            other => panic!("expected the DTO refusal for {pad_layers}, got {other:?}"),
        }
    }

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
            BoardReadResult::ParseError { location, detail } => {
                assert_eq!(location, "nets", "for pad net {pad_net:?}");
                assert!(
                    detail.contains("nets[0].name") && detail.contains("is null"),
                    "for pad net {pad_net:?}: {detail}"
                );
                assert!(
                    !detail.contains("currentNet.name"),
                    "the crash inside Nets.get's walk is gone: {detail}"
                );
            }
            other => panic!("expected the DTO refusal, got {other:?}"),
        }
    }
}

#[test]
fn the_generated_padstack_names_carry_both_dimensions() {
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
            "Round[A]Pad_1000x2000_um",
            "Round[A]Pad_3000x2000_um",
            "Oval[A]Pad_1000x2000_um",
            "Rect[A]Pad_1000x2000_um",
            "Rect[A]Pad_4000x5000_um",
            "Round[A]Pad_6000x2000_um",
            "Rect[T]Pad_7000x7000_um",
            "Rect[B]Pad_8000x8000_um",
            "Rect[A]Pad_9000x9000_um",
            "Round[A]Pad_0x0_um",
            "Rect[A]Pad_2x4_um",
        ]
    );

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

#[test]
fn the_padstack_identity_is_the_layer_span_and_the_drill_not_the_name() {
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
    let mid = package.get_pin(1).expect("mid").padstack_no;
    let drilled = package.get_pin(2).expect("drilled").padstack_no;
    assert_ne!(
        mid, span,
        "an In1.Cu-only pad must not inherit the all-layer padstack"
    );
    assert_ne!(
        drilled, span,
        "and a drilled pad must not inherit an undrilled one's"
    );
    assert_ne!(mid, drilled);

    let span = board.library.padstacks.get(span).expect("the span pad's");
    assert_eq!((span.from_layer(), span.to_layer()), (0, 2));
    assert!(!span.attach_allowed);
    let mid = board.library.padstacks.get(mid).expect("the mid pad's");
    assert_eq!(
        (mid.from_layer(), mid.to_layer()),
        (1, 1),
        "the In1.Cu pad has copper on In1.Cu and nowhere else"
    );
    let drilled = board
        .library
        .padstacks
        .get(drilled)
        .expect("the drilled pad's");
    assert_eq!((drilled.from_layer(), drilled.to_layer()), (0, 2));
    assert!(drilled.attach_allowed);

    assert_eq!(span.name, "Rect[A]Pad_1000x1000_um");
    assert_eq!(mid.name, "Rect[A]Pad_1000x1000_um#2");
    assert_eq!(drilled.name, "Rect[A]Pad_1000x1000_um#3");
}
