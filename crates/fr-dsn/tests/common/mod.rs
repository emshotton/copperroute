//! The Rust half of `crates/fr-dsn/tests/data/NProbe.java` — the JVM probe that prints a board
//! read by the pinned 2.3.0 jar, line by line, so a Rust read can be diffed against it.
//!
//! `dump` reproduces `NProbe.main`'s output exactly: same lines, same order, same spelling.
//! Regenerate a golden with
//!
//! ```text
//! export PATH=/opt/homebrew/opt/openjdk@25/bin:$PATH
//! javac -cp tools/freerouting-2.3.0.jar -d /tmp/nprobe crates/fr-dsn/tests/data/NProbe.java
//! java -Djava.awt.headless=true -cp tools/freerouting-2.3.0.jar:/tmp/nprobe \
//!     NProbe ../freerouting/fixtures/<name>.dsn
//! ```
//!
//! `tests/network_scope.rs` keeps its own copy of this function (Plan 3 Task 9); the two are
//! byte-identical apart from the `PolylineTrace`/`Via` arms Task 10 added, which no Task 9
//! fixture reaches (all of theirs have an empty `(wiring)` scope).

#![allow(dead_code)]

use fr_board::{Board, Item};

/// A fixture from the Java repo's corpus.
///
/// Resolves through `parity::java_dir()`, so `FREEROUTING_JAVA_DIR` is honoured here too —
/// unset, it falls back to the same `../freerouting` sibling checkout this used to hard-code.
pub fn fixture(name: &str) -> String {
    let path = parity::java_dir().join("fixtures").join(name);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("fixture {name} ({}): {e}", path.display()))
}

/// A file under `crates/fr-dsn/tests/data/`.
pub fn test_data(name: &str) -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/");
    std::fs::read_to_string(format!("{path}{name}"))
        .unwrap_or_else(|e| panic!("test data {name}: {e}"))
}

/// A golden file's payload: every line that is not part of the `#` header.
pub fn golden(name: &str) -> Vec<String> {
    test_data(name)
        .lines()
        .filter(|l| !l.starts_with('#'))
        .map(ToString::to_string)
        .collect()
}

/// `NProbe.main`, in Rust.
#[allow(clippy::too_many_lines)]
pub fn dump(board: &Board, warnings: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    out.push(format!("layers {}", board.get_layer_count()));
    let ctx = board.ctx();
    for (id, item) in board.items.iter() {
        let kind = match item {
            Item::BoardOutline(_) => "BoardOutline",
            Item::ObstacleArea(_) => "ObstacleArea",
            Item::ViaObstacleArea(_) => "ViaObstacleArea",
            Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
            Item::ComponentOutline(_) => "ComponentOutline",
            Item::ConductionArea(_) => "ConductionArea",
            Item::Pin(_) => "Pin",
            Item::Via(_) => "Via",
            Item::Trace(_) => "PolylineTrace",
        };
        let detail = match item {
            Item::ObstacleArea(a) => format!(
                " layer={} name={}",
                a.area.get_layer(),
                a.area.name().unwrap_or("null")
            ),
            Item::ViaObstacleArea(a) => format!(
                " layer={} name={}",
                a.area.get_layer(),
                a.area.name().unwrap_or("null")
            ),
            Item::ComponentObstacleArea(a) => format!(
                " layer={} name={}",
                a.area.get_layer(),
                a.area.name().unwrap_or("null")
            ),
            Item::ComponentOutline(o) => {
                format!(
                    " layer={} courtyard={}",
                    o.get_layer(&ctx),
                    o.is_courtyard()
                )
            }
            Item::Pin(p) => format!(" pin={}", p.name(&ctx).unwrap_or("null")),
            // Java's `ConductionArea extends ObstacleArea`, so `NProbe`'s `instanceof
            // ObstacleArea` arm catches it and prints the name too.
            Item::ConductionArea(a) => format!(
                " layer={} name={}",
                a.area.get_layer(),
                a.area.name().unwrap_or("null")
            ),
            // Added by Plan 3 Task 10: the two kinds the `wiring` scope inserts.
            Item::Trace(t) => format!(
                " layer={} hw={} corners={} first={} last={} fixed={}",
                t.get_layer(),
                t.get_half_width(),
                t.corner_count(),
                point(t.first_corner()),
                point(t.last_corner()),
                fixed_state(item),
            ),
            Item::Via(v) => format!(
                " padstack={} at={} layers={}..{} attach={} fixed={}",
                board
                    .library
                    .get_padstack(v.get_padstack_id())
                    .map_or("null", |p| p.name.as_str()),
                point(Some(v.get_center())),
                v.first_layer(&ctx),
                v.last_layer(&ctx),
                v.attach_allowed,
                fixed_state(item),
            ),
            Item::BoardOutline(_) => String::new(),
        };
        let header = item.header();
        let mut nets = String::new();
        for i in 0..header.net_count() {
            nets.push_str(&format!("{},", header.get_net_number(i)));
        }
        out.push(format!(
            "item {} {kind}{detail} cmp={} cl={} nets=[{nets}]",
            id.0,
            header.get_component_id(),
            header.clearance_class(),
        ));
    }
    out.push(format!("itemcount {}", board.items.len()));

    for i in 1..=board.rules.nets.max_net_number() {
        let net = board.rules.nets.get(i).expect("net in range");
        out.push(format!(
            "net {i} {} subnet={} class={} plane={}",
            net.name,
            net.subnet_number,
            board.rules.net_classes.get(net.get_net_class()).get_name(),
            net.contains_plane(),
        ));
    }

    let via_padstacks = board.library.get_via_padstacks();
    let names: Vec<&str> = via_padstacks
        .iter()
        .map(|id| {
            board
                .library
                .get_padstack(*id)
                .map_or("null", |p| p.name.as_str())
        })
        .collect();
    out.push(format!(
        "viapadstacks[{}] {}",
        via_padstacks.len(),
        names.join(" ")
    ));

    for (i, via) in board.rules.via_infos.iter().enumerate() {
        out.push(format!(
            "viainfo {i} {} padstack={} cl={} attach={}",
            via.get_name(),
            board
                .library
                .get_padstack(via.get_padstack())
                .map_or("null", |p| p.name.as_str()),
            via.get_clearance_class_index(),
            via.attach_smd_allowed(),
        ));
    }

    for (i, rule) in board.rules.via_rules.iter().enumerate() {
        let vias: Vec<&str> = rule.iter().map(fr_board::ViaInfo::get_name).collect();
        out.push(format!("viarule {i} {} [{}]", rule.name, vias.join(" ")));
    }

    for i in 0..board.rules.net_classes.count() {
        let net_class = board.rules.net_classes.get(fr_board::NetClassId(i));
        let via_rule = net_class
            .get_via_rule()
            .map_or("null", |rule| rule.name.as_str());
        let half_widths: Vec<String> = (0..net_class.layer_count())
            .map(|layer| net_class.get_trace_half_width(layer).to_string())
            .collect();
        out.push(format!(
            "netclass {i} {} traceCl={} viaRule={via_rule} hw=[{}] pullTight={} shoveFixed={} \
             minLen={:?} maxLen={:?}",
            net_class.get_name(),
            net_class.get_trace_clearance_class(),
            half_widths.join(","),
            net_class.get_pull_tight(),
            net_class.is_shove_fixed(),
            net_class.get_minimum_trace_length(),
            net_class.get_maximum_trace_length(),
        ));
    }

    let cm = &board.rules.clearance_matrix;
    let mut class_names = String::new();
    for i in 0..cm.get_class_count() {
        class_names.push_str(&format!("{i}:{} ", cm.get_name(i).unwrap_or("null")));
    }
    out.push(format!(
        "clclasses[{}] {}",
        cm.get_class_count(),
        class_names.trim_end()
    ));
    for i in 0..cm.get_class_count() {
        let mut row = String::new();
        for j in 0..cm.get_class_count() {
            row.push_str(&format!("{} ", cm.get_value(i, j, 0, false)));
        }
        out.push(format!("clrow {i} {}", row.trim_end()));
    }

    for i in 1..=board.components.count() {
        let component = board.components.get(i as i32);
        out.push(format!(
            "component {i} {} pkg={} placed={} front={} lp={}",
            component.name,
            board.library.packages.get(component.get_package()).name,
            component.is_placed(),
            component.placed_on_front(),
            component.get_logical_part().map_or("null", |no| board
                .library
                .logical_parts
                .get(no)
                .name
                .as_str()),
        ));
    }

    for i in 0..board.library.logical_parts.count() {
        out.push(format!(
            "logicalpart {i} {}",
            board.library.logical_parts.get(i).name
        ));
    }

    for warning in warnings {
        out.push(format!("warning {warning}"));
    }
    out
}

/// `IntPoint.toString` (IntPoint.java:401-403): `"(" + x + "," + y + ")"`. A `null` point prints
/// as Java's `String.valueOf(null)`.
fn point(p: Option<fr_geometry::Point>) -> String {
    match p {
        Some(fr_geometry::Point::Int(p)) => format!("({},{})", p.x, p.y),
        Some(other) => format!("{other:?}"),
        None => "null".to_string(),
    }
}

/// `FixedState`'s enum constant name, as Java's implicit `Enum.toString` prints it
/// (FixedState.java:5-8).
fn fixed_state(item: &Item) -> &'static str {
    match item.header().get_fixed_state() {
        fr_board::FixedState::Unfixed => "UNFIXED",
        fr_board::FixedState::ShoveFixed => "SHOVE_FIXED",
        fr_board::FixedState::UserFixed => "USER_FIXED",
        fr_board::FixedState::SystemFixed => "SYSTEM_FIXED",
    }
}

/// Diffs a whole dump against a golden, line by line.
pub fn assert_matches_golden(board: &Board, warnings: &[String], golden_name: &str) {
    let actual = dump(board, warnings);
    let expected = golden(golden_name);
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(a, e, "{golden_name}: line {} differs", i + 1);
    }
    assert_eq!(
        actual.len(),
        expected.len(),
        "{golden_name}: line count differs"
    );
}

// ---------------------------------------------------------------------------------------------
// `SesRoundTripTest.java`'s two SES-text helpers, shared by `tests/parity_ses.rs` (Task 12) and
// `tests/ses_round_trip.rs` (Task 13) — Java has one copy of each and so does this port.
// ---------------------------------------------------------------------------------------------

/// `SesRoundTripTest.assertBalancedScopes` (SesRoundTripTest.java:280-284).
///
/// **Java wins over the brief**, which describes this helper as counting brackets "outside quoted
/// strings": Java's counts every `(` and `)` character in the file, quoted ones included. Ported
/// as Java has it — a stricter helper would be a different test.
pub fn assert_balanced_scopes(content: &str) {
    let opens = content.chars().filter(|&c| c == '(').count();
    let closes = content.chars().filter(|&c| c == ')').count();
    assert_eq!(opens, closes, "SES scopes must be balanced");
}

/// `SesRoundTripTest.assertUniqueLibraryPadstacks` (SesRoundTripTest.java:286-302), with the
/// `\(padstack\s+([^\s()]+)` regex hand-rolled (`fr-dsn` has no `regex` dependency).
pub fn assert_unique_library_padstacks(content: &str) {
    let library_start = content
        .find("(library_out")
        .expect("SES must contain library_out scope");
    let network_start = library_start
        + content[library_start..]
            .find("(network_out")
            .expect("SES must contain network_out scope after library_out");
    let library_section = &content[library_start..network_start];

    let mut padstack_names: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut rest = library_section;
    let mut found_any = false;
    while let Some(at) = rest.find("(padstack") {
        let after = &rest[at + "(padstack".len()..];
        let name_start = after.len() - after.trim_start().len();
        // `\s+` — at least one whitespace character, else the regex would not match here.
        if name_start == 0 {
            rest = after;
            continue;
        }
        let name = &after[name_start..];
        let name_end = name
            .find([' ', '\t', '\n', '\r', '(', ')'])
            .unwrap_or(name.len());
        let name = &name[..name_end];
        assert!(
            padstack_names.insert(name),
            "library_out must not contain duplicate padstack entries: {name}"
        );
        found_any = true;
        rest = after;
    }
    assert!(
        found_any,
        "library_out must declare at least one via padstack"
    );
}

// ---------------------------------------------------------------------------------------------
// Plan 8 Task 13: the four directed fixtures for Plan 3's zero-coverage paths.
//
// Each fixture is `tests/data/p8t13-<path>.dsn` and its JVM ground truth is
// `tests/data/p8t13-directed-<path>.txt` — the byte-exact stdout of
// `scripts/differential/java/probes/P8T13Probe.java` against the pinned HEAD jar under JDK 25
// (that probe's header comment carries the exact `javac`/`java` invocation, and its `[jar-cli]`
// rows carry the task brief's `java -jar <HEAD jar> -de <fixture> -do <out.ses>` acceptance run).
// The helpers below replay those transcripts; the four tests that use them live in the suites
// that own each path, and `tests/plan_3_zero_coverage.rs` is the list assertion over all four.
// ---------------------------------------------------------------------------------------------

/// Reads one directed fixture exactly as `P8T13Probe.read` does:
/// `DsnReader.readBoard(in, null, new ItemIdGenerator(), "<file>.dsn")` — the design name carries
/// the `.dsn` suffix, as `File.getName()` hands it over.
pub fn read_directed(path_name: &str) -> (Board, fr_dsn::CoordinateTransform) {
    let name = format!("p8t13-{path_name}.dsn");
    let text = test_data(&name);
    let options = fr_dsn::parser::scope_parameter::DsnReadOptions::default();
    match fr_dsn::read_board(text.as_bytes(), None, Some(&name), &options) {
        fr_dsn::BoardReadResult::Success {
            board,
            coordinate_transform,
            warnings,
            ..
        } => {
            assert!(
                warnings.is_empty(),
                "{name}: the probe recorded `[read] Success warnings=0`, the port warned {warnings:?}"
            );
            (
                *board.unwrap_or_else(|| panic!("{name} produced no board")),
                coordinate_transform
                    .unwrap_or_else(|| panic!("{name} produced no coordinate transform")),
            )
        }
        other => panic!("{name}: expected Success, got {other:?}"),
    }
}

/// `SesWriter.write(board, out, "<file>")` — the probe's `emitSes`, whose design name is
/// `File.getName().replace(".dsn", "")`.
pub fn write_directed_ses(
    board: &Board,
    ct: &fr_dsn::CoordinateTransform,
    path_name: &str,
) -> String {
    let mut out: Vec<u8> = Vec::new();
    fr_dsn::ses_writer::write(board, ct, &mut out, &format!("p8t13-{path_name}"))
        .expect("writing into a Vec cannot fail");
    String::from_utf8(out).expect("SES output must be valid UTF-8")
}

/// Every row of `p8t13-directed-<path>.txt` that starts with `prefix`, verbatim.
pub fn directed_rows(path_name: &str, prefix: &str) -> Vec<String> {
    let transcript = test_data(&format!("p8t13-directed-{path_name}.txt"));
    let rows: Vec<String> = transcript
        .lines()
        .filter(|l| l.starts_with(prefix))
        .map(ToString::to_string)
        .collect();
    assert!(
        !rows.is_empty(),
        "p8t13-directed-{path_name}.txt carries no `{prefix}` row — the transcript was \
         regenerated with a different row set"
    );
    rows
}

/// The SES text a `[ses <label>]|` block of `p8t13-directed-<path>.txt` carries, plus the
/// `[ses <label>] bytes=` length the probe recorded beside it.
pub fn directed_ses(path_name: &str, label: &str) -> (String, usize) {
    let head = format!("[ses {label}] bytes=");
    let bytes: usize = directed_rows(path_name, &head)
        .first()
        .expect("directed_rows never returns empty")
        .trim_start_matches(&head)
        .parse()
        .expect("the probe writes a decimal byte count");
    let body = format!("[ses {label}]|");
    let text = directed_rows(path_name, &body)
        .iter()
        .map(|l| l.trim_start_matches(&body).to_string())
        .collect::<Vec<_>>()
        .join("\n");
    (text, bytes)
}

/// `P8T13Probe.items`, in Rust: one `[item] …` row per item, id ascending, then `[itemcount] …`.
pub fn directed_items(board: &Board) -> Vec<String> {
    let mut ids: Vec<_> = board.items.keys().copied().collect();
    ids.sort_unstable();
    let mut out = Vec::new();
    for id in ids {
        let item = board.get_item(id).expect("id came from the item list");
        let kind = match item {
            Item::BoardOutline(_) => "BoardOutline",
            Item::ObstacleArea(_) => "ObstacleArea",
            Item::ViaObstacleArea(_) => "ViaObstacleArea",
            Item::ComponentObstacleArea(_) => "ComponentObstacleArea",
            Item::ComponentOutline(_) => "ComponentOutline",
            Item::ConductionArea(_) => "ConductionArea",
            Item::Pin(_) => "Pin",
            Item::Via(_) => "Via",
            Item::Trace(_) => "PolylineTrace",
        };
        let header = item.header();
        let nets: Vec<String> = (0..header.net_count())
            .map(|i| header.get_net_number(i).to_string())
            .collect();
        out.push(format!(
            "[item] {} {kind} comp={} cl={} nets=[{}] fixed={}",
            id.0,
            header.get_component_id(),
            header.clearance_class(),
            nets.join(","),
            fixed_state(item),
        ));
    }
    out.push(format!("[itemcount] {}", board.items.len()));
    out
}

/// `P8T13Probe.viaNetNumbers`' `[net] …` rows, in Rust.
pub fn directed_nets(board: &Board) -> Vec<String> {
    (1..=board.rules.nets.max_net_number())
        .map(|i| {
            let net = board.rules.nets.get(i).expect("net in range");
            format!(
                "[net] {i} {} subnet={} class={} plane={}",
                net.name,
                net.subnet_number,
                board.rules.net_classes.get(net.get_net_class()).get_name(),
                net.contains_plane(),
            )
        })
        .collect()
}

/// Diffs two row lists line by line, naming the transcript.
pub fn assert_rows_match(actual: &[String], expected: &[String], what: &str) {
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(a, e, "{what}: row {} differs", i + 1);
    }
    assert_eq!(actual.len(), expected.len(), "{what}: row count differs");
}
