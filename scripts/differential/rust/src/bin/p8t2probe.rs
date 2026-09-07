//! Plan 8 Task 2's Rust twin of `scripts/differential/java/probes/P8T2Probe.java`.
//!
//! Prints the same five tables in the same order with the same escaping, so
//! `scripts/differential/run.sh p8t2probe` is a byte diff of the text-scraping
//! `BoardStatistics(byte[], FileFormat)` constructor, `countOccurrences` and the whole Gson JSON
//! surface against the HEAD jar. See the Java probe's class javadoc for what each table covers
//! and for the two `XDIFF` rows.
//!
//! The driver is called `p8t2probe`, not `p8t2`: the plan reserves `p8t2` for Task 4's manifest
//! driver, which is a different driver against the same jar.

use copper_core::{BoardStatistics, BoardStatisticsExt, FileFormat, count_occurrences, to_gson_string};
use copper_dsn::{format_double, format_float};
use copper_router::score::{BoardStatisticsFanout, Rectangle2DFloat};
use std::path::{Path, PathBuf};

/// The FLD column order: `BoardStatistics.java:37-79`, then each DTO's own declaration order.
const FIELD_PATHS: &[&str] = &[
    "host",
    "unit",
    "board.bounding_box.x",
    "board.bounding_box.y",
    "board.bounding_box.width",
    "board.bounding_box.height",
    "board.size.x",
    "board.size.y",
    "board.size.width",
    "board.size.height",
    "layers.total_count",
    "layers.signal_count",
    "items.total_count",
    "items.trace_count",
    "items.via_count",
    "items.conduction_area_count",
    "items.drill_item_count",
    "items.pin_count",
    "items.component_count",
    "items.other_count",
    "components.total_count",
    "pads.total_count",
    "nets.total_count",
    "nets.class_count",
    "connections.maximum_count",
    "connections.incomplete_count",
    "traces.total_count",
    "traces.total_segment_count",
    "traces.total_length",
    "traces.total_length_mm",
    "traces.total_weighted_length",
    "traces.average_length",
    "traces.total_vertical_length",
    "traces.total_horizontal_length",
    "traces.total_angled_length",
    "bends.total_count",
    "bends.90_degree_count",
    "bends.45_degree_count",
    "bends.other_angle_count",
    "vias.total_count",
    "vias.through_hole_count",
    "vias.blind_count",
    "vias.buried_count",
    "clearance_violations.total_count",
    "clearance_violations.min_violation_um",
    "clearance_violations.max_violation_um",
    "clearance_violations.avg_violation_um",
    "fanout.total_smd_pins",
    "fanout.pins_to_escape",
    "fanout.escaped_count",
];

/// The seven DSN/SES reference stems of `tests/reference/fixtures.txt`, in file order.
const STEMS: &[(&str, &str)] = &[
    ("tutorial_board", "examples/tutorial_board/tutorial_board.dsn"),
    ("Issue026-J2_reference", "fixtures/Issue026-J2_reference.dsn"),
    ("Issue103-Board-Unrouted", "fixtures/Issue103-Board-Unrouted.dsn"),
    ("Issue143-rpi_splitter", "fixtures/Issue143-rpi_splitter.dsn"),
    ("Issue413-test", "fixtures/Issue413-test.dsn"),
    ("Issue110-RelayModule", "fixtures/Issue110-RelayModule.dsn"),
    ("Issue753-CPU-85_r104", "fixtures/Issue753-CPU-85_r104.dsn"),
];

/// The eight batch stems of `tests/reference/router-fixtures.txt`, in file order.
const BATCH_STEMS: &[&str] = &[
    "router-rpi-splitter",
    "router-dac2020-bm01",
    "router-j2-reference",
    "router-tutorial-board",
    "router-ecc83-input",
    "router-fanout-bm11",
    "router-strict-drc-cnh",
    "router-empty-board",
];

/// `Gson.toJson` on the `empty hostCad` row, transcribed from the HEAD jar. Java's `host` is the
/// empty string there and the port's is Java's `null`, so the two disagree by exactly this one
/// key; the row prints both answers (quirk #251).
const EMPTY_HOST_JAVA_JSON: &str = concat!(
    "{\n",
    "  \"host\": \"\",\n",
    "  \"board\": {},\n",
    "  \"layers\": {\n    \"total_count\": 0\n  },\n",
    "  \"items\": {},\n",
    "  \"components\": {\n    \"total_count\": 0\n  },\n",
    "  \"pads\": {},\n",
    "  \"nets\": {\n    \"total_count\": 0,\n    \"class_count\": 0\n  },\n",
    "  \"connections\": {},\n",
    "  \"traces\": {\n    \"total_count\": 0\n  },\n",
    "  \"bends\": {},\n",
    "  \"vias\": {\n    \"total_count\": 0\n  },\n",
    "  \"clearance_violations\": {},\n",
    "  \"fanout\": {\n",
    "    \"total_smd_pins\": 0,\n",
    "    \"pins_to_escape\": 0,\n",
    "    \"escaped_count\": 0\n",
    "  }\n",
    "}",
);

/// How a row diverges from the jar, if it does.
enum Divergence {
    /// The port and the jar agree on every column.
    None,
    /// `substring(hcIdx + 9, hcEnd)` throws out of the Java constructor (quirk #250). The
    /// `java=` half is the literal the jar prints.
    JavaThrows,
    /// Java's `host` is `""` where the port spells Java's `null` the same way (quirk #251).
    EmptyHost,
}

struct Row {
    label: String,
    /// `None` is Java's null `FileFormat`, which returns at `:437-439`.
    format: Option<FileFormat>,
    /// `None` is Java's null `byte[]`, same guard.
    data: Option<Vec<u8>>,
    src: String,
    divergence: Divergence,
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let java_dir = PathBuf::from(&args[0]);
    let repo_root = PathBuf::from(&args[1]);

    let mut out = String::new();
    out.push_str(&format!("HDR\t{}\n", FIELD_PATHS.join("\t")));

    // ---- countOccurrences (:578-586) ---------------------------------------------------------
    let count_cases: &[(&str, &str)] = &[
        ("", "(net"),
        ("(net", "(net"),
        ("(network", "(net"),
        ("(net_class", "(net"),
        ("(net (net (net", "(net"),
        ("(layer_rule (layer TOP)", "(layer"),
        ("(via_rule (via V1)", "(via"),
        ("(class_class (class C)", "(class"),
        ("aaaa", "aa"),
        ("(wire(wire(wire", "(wire"),
        ("(component", "(componentx"),
        ("é(net", "(net"),
    ];
    for (i, (haystack, needle)) in count_cases.iter().enumerate() {
        out.push_str(&format!(
            "COUNT\t{i}\t{}\t{}\t{}\n",
            esc(haystack),
            esc(needle),
            count_occurrences(haystack, needle)
        ));
    }

    let mut rows: Vec<Row> = Vec::new();

    // ---- the scraper over the committed corpus -----------------------------------------------
    for (stem, relative) in STEMS {
        rows.push(file_row(
            &format!("{stem}/source.dsn"),
            FileFormat::Dsn,
            &format!("java:{relative}"),
            &java_dir.join(relative),
        ));
        rows.push(file_row(
            &format!("{stem}/roundtrip.dsn"),
            FileFormat::Dsn,
            &format!("ref:{stem}/roundtrip.dsn"),
            &repo_root.join(format!("tests/reference/{stem}/roundtrip.dsn")),
        ));
        rows.push(file_row(
            &format!("{stem}/unrouted.ses"),
            FileFormat::Ses,
            &format!("ref:{stem}/unrouted.ses"),
            &repo_root.join(format!("tests/reference/{stem}/unrouted.ses")),
        ));
    }
    for stem in BATCH_STEMS {
        rows.push(file_row(
            &format!("{stem}/batch.ses"),
            FileFormat::Ses,
            // The migrated, pre-lane-switch jar-written copies (ruling BT, Plan 9 Task 2); see
            // `crates/copper-core/tests/data/p8t2-batch-ses/README.md` and the Java twin's note.
            &format!("data:p8t2-batch-ses/{stem}.ses"),
            &repo_root.join(format!("crates/copper-core/tests/data/p8t2-batch-ses/{stem}.ses")),
        ));
    }
    rows.push(file_row(
        "Issue143-rpi_splitter/source.dsn AS SES",
        FileFormat::Ses,
        "java:fixtures/Issue143-rpi_splitter.dsn",
        &java_dir.join("fixtures/Issue143-rpi_splitter.dsn"),
    ));
    rows.push(file_row(
        "Issue143-rpi_splitter/unrouted.ses AS DSN",
        FileFormat::Dsn,
        "ref:Issue143-rpi_splitter/unrouted.ses",
        &repo_root.join("tests/reference/Issue143-rpi_splitter/unrouted.ses"),
    ));
    // The ONE shape in this repository where the host scrape succeeds on a real, jar-written
    // file: `Parser.writeScope(..., reduced = true)` skips `(stringQuote ")`, so the first `)`
    // after `(parser` closes `(hostCad …)` instead of truncating in front of it, and HEAD's own
    // keyword IS the camelCase one the scrape looks for.
    rows.push(file_row(
        "router-dac2020-bm01/batch.ses AS DSN",
        FileFormat::Dsn,
        "data:p8t2-batch-ses/router-dac2020-bm01.ses",
        &repo_root.join("crates/copper-core/tests/data/p8t2-batch-ses/router-dac2020-bm01.ses"),
    ));

    // ---- the guards and the formats with no branch --------------------------------------------
    rows.push(Row {
        label: "null data".to_string(),
        format: Some(FileFormat::Dsn),
        data: None,
        src: "null".to_string(),
        divergence: Divergence::None,
    });
    rows.push(bytes_row("null format", "s:(pcb x", None));
    for format in [
        FileFormat::Unknown,
        FileFormat::Frb,
        FileFormat::Rules,
        FileFormat::Scr,
        FileFormat::DrcJson,
        FileFormat::KicadSessionJson,
    ] {
        rows.push(bytes_row(
            &format!("no branch {}", format.name()),
            "s:(pcb x",
            Some(format),
        ));
    }
    rows.push(bytes_row("empty SES", "s:", Some(FileFormat::Ses)));
    rows.push(bytes_row("empty DSN", "s:", Some(FileFormat::Dsn)));
    rows.push(bytes_row(
        "empty KiCad JSON",
        "s:",
        Some(FileFormat::KicadDesignJson),
    ));
    rows.push(bytes_row(
        "binary junk SES",
        "h:00ff01fe0210(net",
        Some(FileFormat::Ses),
    ));
    rows.push(bytes_row(
        "binary junk DSN",
        "h:00ff01fe0210(layer",
        Some(FileFormat::Dsn),
    ));

    // ---- quirk #247, the substring counting ----------------------------------------------------
    rows.push(bytes_row(
        "quirk F DSN keywords",
        "s:(layer_rule)(layer TOP)(network X)(net_class Y)(net N)(via_rule R)(via V)\
         (class_class Z)(class C)(component U1)(wire)",
        Some(FileFormat::Dsn),
    ));
    rows.push(bytes_row(
        "quirk F SES keywords",
        "s:(component U1)(net N)(network X)(net_class Y)(wire)(via V)(via_rule R)",
        Some(FileFormat::Ses),
    ));

    // ---- the SES layer scrape ------------------------------------------------------------------
    rows.push(bytes_row("SES no path", "s:(session x)", Some(FileFormat::Ses)));
    rows.push(bytes_row(
        "SES one path",
        "s:(wire (path F.Cu 250 1 2 3 4))",
        Some(FileFormat::Ses),
    ));
    rows.push(bytes_row(
        "SES two layers",
        "s:(path F.Cu 250 1 2)(path B.Cu 250 3 4)",
        Some(FileFormat::Ses),
    ));
    rows.push(bytes_row(
        "SES repeated layer",
        "s:(path F.Cu 250 1 2)(path F.Cu 250 3 4)",
        Some(FileFormat::Ses),
    ));
    rows.push(bytes_row(
        "SES leading path",
        "s:(path F.Cu 250 1 2)",
        Some(FileFormat::Ses),
    ));
    rows.push(bytes_row("SES one-word chunk", "s:a(path F.Cu", Some(FileFormat::Ses)));
    rows.push(bytes_row(
        "SES empty chunk",
        "s:a(path (path B.Cu 1 2",
        Some(FileFormat::Ses),
    ));
    rows.push(bytes_row("SES trailing path", "s:a(path ", Some(FileFormat::Ses)));

    // ---- quirk #248, the DSN host scrape --------------------------------------------------------
    rows.push(bytes_row(
        "real DSN parser scope",
        concat!(
            "s:(pcb x\n  (parser\n    (string_quote \")\n",
            "    (host_cad \"KiCad's Pcbnew\")\n",
            "    (host_version \"8.0.4\")\n  )\n)"
        ),
        Some(FileFormat::Dsn),
    ));
    rows.push(bytes_row(
        "camelCase, both",
        "s:(parser (hostCad \"KiCad\" (hostVersion \"8.0\" ))",
        Some(FileFormat::Dsn),
    ));
    rows.push(bytes_row(
        "camelCase, cad only",
        "s:(parser (hostCad \"KiCad\" ))",
        Some(FileFormat::Dsn),
    ));
    rows.push(bytes_row(
        "camelCase, version only",
        "s:(parser (hostVersion \"8.0\" ))",
        Some(FileFormat::Dsn),
    ));
    rows.push(bytes_row(
        "camelCase, two spaces",
        "s:(parser (hostCad  \"KiCad\" ))",
        Some(FileFormat::Dsn),
    ));
    rows.push(bytes_row(
        "camelCase, no space",
        "s:(parser (hostCad\"KiCad\" ))",
        Some(FileFormat::Dsn),
    ));
    rows.push(bytes_row(
        "trim keeps NBSP",
        "s:(parser (hostCad K\u{a0} ))",
        Some(FileFormat::Dsn),
    ));
    rows.push(bytes_row(
        "trim drops the control char",
        "s:(parser (hostCad K\u{1} ))",
        Some(FileFormat::Dsn),
    ));
    rows.push(bytes_row(
        "camelCase, unquoted",
        "s:(parser (hostCad KiCad (hostVersion 8.0 ))",
        Some(FileFormat::Dsn),
    ));
    rows.push(bytes_row(
        "no parser scope",
        "s:(pcb x (structure))",
        Some(FileFormat::Dsn),
    ));
    rows.push(bytes_row(
        "parser, no close",
        "s:(parser (hostCad \"K\" (hostVersion \"8\" ",
        Some(FileFormat::Dsn),
    ));
    rows.push(bytes_row(
        "parser, no close, >1000",
        &format!(
            "s:(parser (hostCad \"K\" {} (hostVersion \"8\" ",
            "x".repeat(1200)
        ),
        Some(FileFormat::Dsn),
    ));
    rows.push(bytes_row(
        "host is not unescaped",
        "s:(parser (hostCad \"K\\u0041D\" (hostVersion \"8\" ))",
        Some(FileFormat::Dsn),
    ));
    let mut empty_host = bytes_row(
        "empty hostCad",
        "s:(parser (hostCad  ))",
        Some(FileFormat::Dsn),
    );
    empty_host.divergence = Divergence::EmptyHost;
    rows.push(empty_host);
    let mut inverted = bytes_row(
        "inverted substring",
        "s:(parser (hostCad))",
        Some(FileFormat::Dsn),
    );
    inverted.divergence = Divergence::JavaThrows;
    rows.push(inverted);

    // ---- the KiCad design JSON branch -----------------------------------------------------------
    rows.push(bytes_row(
        "kicad full",
        "s:{\"layers\":[1,2,3,4],\"components\":[{},{}],\"netClasses\":[{}],\"nets\":[1,2,3],\
         \"traces\":[],\"vias\":[{},{},{}],\"designName\":\"board\"}",
        Some(FileFormat::KicadDesignJson),
    ));
    rows.push(bytes_row(
        "kicad empty object",
        "s:{}",
        Some(FileFormat::KicadDesignJson),
    ));
    rows.push(bytes_row("kicad array", "s:[1,2]", Some(FileFormat::KicadDesignJson)));
    rows.push(bytes_row(
        "kicad null literal",
        "s:null",
        Some(FileFormat::KicadDesignJson),
    ));
    rows.push(bytes_row(
        "kicad malformed",
        "s:{\"layers\":",
        Some(FileFormat::KicadDesignJson),
    ));
    rows.push(bytes_row(
        "kicad wrong type mid-way",
        "s:{\"layers\":[1,2],\"components\":7,\"nets\":[1]}",
        Some(FileFormat::KicadDesignJson),
    ));
    rows.push(bytes_row(
        "kicad numeric designName",
        "s:{\"designName\":42}",
        Some(FileFormat::KicadDesignJson),
    ));
    for (label, spec) in [
        ("kicad designName 1e5", "s:{\"designName\":1e5}"),
        ("kicad designName 1.50", "s:{\"designName\":1.50}"),
        (
            "kicad designName big integer",
            "s:{\"designName\":123456789012345678901234567890}",
        ),
        ("kicad designName one-element array", "s:{\"designName\":[\"foo\"]}"),
        (
            "kicad designName nested one-element array",
            "s:{\"designName\":[[\"deep\"]]}",
        ),
        ("kicad designName one-element numeric array", "s:{\"designName\":[1e5]}"),
        ("kicad designName two-element array", "s:{\"designName\":[\"a\",\"b\"]}"),
        ("kicad designName empty array", "s:{\"designName\":[]}"),
        ("kicad designName boolean", "s:{\"designName\":true}"),
        ("kicad designName null", "s:{\"designName\":null}"),
        ("kicad designName object", "s:{\"designName\":{\"a\":1}}"),
        (
            "kicad designName duplicated",
            "s:{\"designName\":\"first\",\"designName\":\"last\"}",
        ),
    ] {
        rows.push(bytes_row(label, spec, Some(FileFormat::KicadDesignJson)));
    }

    let mut index = 0;
    for row in &rows {
        out.push_str(&format!(
            "BS\t{index}\t{}\t{}\t{}\n",
            row.label,
            row.format.map_or("<null>", FileFormat::name),
            row.src
        ));
        let stats = match (&row.data, row.format) {
            (Some(data), Some(format)) => BoardStatistics::from_bytes(data, format),
            // `:437-439` — both null arms leave the all-null object, which is also what a
            // `FileFormat` with no branch produces.
            _ => BoardStatistics::default(),
        };
        match row.divergence {
            Divergence::JavaThrows => {
                // The `rust=` half is a hard-coded literal, so assert that the port still says
                // it: without this, `slice_totalized`'s clamp is only exercised, never checked.
                assert!(
                    stats.host.is_empty(),
                    "the clamp should leave `host` at Java's null, not {:?}",
                    stats.host
                );
                out.push_str(&format!(
                    "XDIFF\t{index}\tjava=StringIndexOutOfBoundsException\trust=host=<omitted>\n"
                ));
            }
            Divergence::EmptyHost => {
                // The `host` column is the jar's literal, transcribed: Java holds `""` where the
                // port holds its spelling of `null`, and the `XDIFF` line below carries both.
                let mut columns = fields(&stats);
                columns[0] = "\"\"".to_string();
                out.push_str(&format!("FLD\t{index}\t{}\n", columns.join("\t")));
                out.push_str(&format!(
                    "XDIFF\t{index}\tjava=host=\"\"\trust=host=<null>\n"
                ));
                out.push_str(&format!(
                    "JSON\t{index}\tXDIFF\tjava={}\trust={}\n",
                    esc(EMPTY_HOST_JAVA_JSON),
                    esc(&to_gson_string(&stats))
                ));
            }
            Divergence::None => {
                out.push_str(&format!("FLD\t{index}\t{}\n", fields(&stats).join("\t")));
                out.push_str(&format!("JSON\t{index}\t{}\n", esc(&to_gson_string(&stats))));
            }
        }
        index += 1;
    }

    // ---- the Gson surface on the fields the scraper never writes ----------------------------------
    for (label, stats) in [
        ("empty", BoardStatistics::default()),
        ("populated", populated()),
        ("fanout only", fanout_only()),
    ] {
        out.push_str(&format!(
            "BS\t{index}\t{label}\t<none>\tsynth:{}\n",
            label.replace(' ', "-")
        ));
        out.push_str(&format!("FLD\t{index}\t{}\n", fields(&stats).join("\t")));
        out.push_str(&format!("JSON\t{index}\t{}\n", esc(&to_gson_string(&stats))));
        index += 1;
    }

    print!("{out}");
}

// ==================================================================================================
// the row builders
// ==================================================================================================

fn file_row(label: &str, format: FileFormat, src: &str, path: &Path) -> Row {
    let data = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    Row {
        label: label.to_string(),
        format: Some(format),
        data: Some(data),
        src: src.to_string(),
        divergence: Divergence::None,
    }
}

fn bytes_row(label: &str, spec: &str, format: Option<FileFormat>) -> Row {
    let data = decode(spec);
    Row {
        label: label.to_string(),
        format,
        src: src_of(&data),
        data: Some(data),
        divergence: Divergence::None,
    }
}

/// `txt:<escaped>` when the bytes survive a UTF-8 round trip and hold no control character other
/// than tab/CR/LF, `hex:<hex>` otherwise — the Java probe's `src(byte[])`.
fn src_of(data: &[u8]) -> String {
    match std::str::from_utf8(data) {
        Ok(text) if !text.chars().any(|c| c < '\u{20}' && c != '\t' && c != '\r' && c != '\n') => {
            format!("txt:{}", esc(text))
        }
        _ => format!("hex:{}", hex(data)),
    }
}

/// `s:<text>` or `h:<hex followed by literal text>` — the Java probe's `decode`.
fn decode(spec: &str) -> Vec<u8> {
    if let Some(text) = spec.strip_prefix("s:") {
        return text.as_bytes().to_vec();
    }
    let rest = spec.strip_prefix("h:").expect("an `s:` or `h:` spec");
    let mut n = rest.chars().take_while(char::is_ascii_hexdigit).count();
    n -= n % 2;
    let mut data: Vec<u8> = (0..n / 2)
        .map(|i| u8::from_str_radix(&rest[2 * i..2 * i + 2], 16).expect("two hex digits"))
        .collect();
    data.extend_from_slice(rest[n..].as_bytes());
    data
}

// ==================================================================================================
// the field printer
// ==================================================================================================

fn fields(stats: &BoardStatistics) -> Vec<String> {
    let mut v = Vec::with_capacity(FIELD_PATHS.len());
    v.push(string_field(&stats.host));
    v.push(string_field(&stats.unit));
    rect(&mut v, stats.board.bounding_box.as_ref());
    rect(&mut v, stats.board.size.as_ref());
    v.push(int(stats.layers.total_count));
    v.push(int(stats.layers.signal_count));
    v.push(int(stats.items.total_count));
    v.push(int(stats.items.trace_count));
    v.push(int(stats.items.via_count));
    v.push(int(stats.items.conduction_area_count));
    v.push(int(stats.items.drill_item_count));
    v.push(int(stats.items.pin_count));
    v.push(int(stats.items.component_outline_count));
    v.push(int(stats.items.other_count));
    v.push(int(stats.components.total_count));
    v.push(int(stats.pads.total_count));
    v.push(int(stats.nets.total_count));
    v.push(int(stats.nets.class_count));
    v.push(int(stats.connections.maximum_count));
    v.push(int(stats.connections.incomplete_count));
    v.push(int(stats.traces.total_count));
    v.push(int(stats.traces.total_segment_count));
    v.push(float(stats.traces.total_length));
    v.push(float(stats.traces.total_length_mm));
    v.push(float(stats.traces.total_weighted_length));
    v.push(float(stats.traces.average_length));
    v.push(float(stats.traces.total_vertical_length));
    v.push(float(stats.traces.total_horizontal_length));
    v.push(float(stats.traces.total_angled_length));
    v.push(int(stats.bends.total_count));
    v.push(int(stats.bends.ninety_degree_count));
    v.push(int(stats.bends.forty_five_degree_count));
    v.push(int(stats.bends.other_angle_count));
    v.push(int(stats.vias.total_count));
    v.push(int(stats.vias.through_hole_count));
    v.push(int(stats.vias.blind_count));
    v.push(int(stats.vias.buried_count));
    v.push(int(stats.clearance_violations.total_count));
    v.push(double(stats.clearance_violations.min_violation_um));
    v.push(double(stats.clearance_violations.max_violation_um));
    v.push(double(stats.clearance_violations.avg_violation_um));
    v.push(stats.fanout.total_smd_pins.to_string());
    v.push(stats.fanout.pins_to_escape.to_string());
    v.push(stats.fanout.escaped_count.to_string());
    assert_eq!(v.len(), FIELD_PATHS.len(), "field count");
    v
}

fn rect(v: &mut Vec<String>, r: Option<&Rectangle2DFloat>) {
    match r {
        None => v.extend(std::iter::repeat_n("<null>".to_string(), 4)),
        Some(r) => {
            v.push(format_float(r.x));
            v.push(format_float(r.y));
            v.push(format_float(r.width));
            v.push(format_float(r.height));
        }
    }
}

fn int(n: Option<i32>) -> String {
    n.map_or_else(|| "<null>".to_string(), |n| n.to_string())
}

fn float(n: Option<f32>) -> String {
    n.map_or_else(|| "<null>".to_string(), format_float)
}

fn double(n: Option<f64>) -> String {
    n.map_or_else(|| "<null>".to_string(), format_double)
}

/// The port spells Java's `null` string as the empty string (quirk #251), so an empty field
/// prints `<null>` here — which is what the jar prints for the same input.
fn string_field(s: &str) -> String {
    if s.is_empty() {
        "<null>".to_string()
    } else {
        format!("\"{}\"", esc(s))
    }
}

// ==================================================================================================
// the two synthetic statistics — the Java probe's `populated()` and `fanoutOnly()`
// ==================================================================================================

fn populated() -> BoardStatistics {
    let mut s = BoardStatistics::default();
    s.host = "KiCad's \"Pcbnew\",8.0.4 é".to_string();
    s.unit = "um".to_string();
    s.board.bounding_box = Some(Rectangle2DFloat {
        x: 1.5,
        y: -2.25,
        width: -1000000.5,
        height: 0.1,
    });
    s.board.size = Some(Rectangle2DFloat {
        x: 0.0,
        y: 0.0,
        width: 1.0E8,
        height: 3.0E-4,
    });
    s.layers.total_count = Some(4);
    s.layers.signal_count = Some(2);
    s.items.total_count = Some(11);
    s.items.trace_count = Some(12);
    s.items.via_count = Some(13);
    s.items.conduction_area_count = Some(14);
    s.items.drill_item_count = Some(15);
    s.items.pin_count = Some(16);
    s.items.component_outline_count = Some(17);
    s.items.other_count = Some(18);
    s.components.total_count = Some(19);
    s.pads.total_count = Some(20);
    s.nets.total_count = Some(21);
    s.nets.class_count = Some(22);
    s.connections.maximum_count = Some(23);
    s.connections.incomplete_count = Some(24);
    s.traces.total_count = Some(25);
    s.traces.total_segment_count = Some(26);
    s.traces.total_length = Some(0.1);
    s.traces.total_length_mm = Some(1.0E7);
    s.traces.total_weighted_length = Some(9.999999E-4);
    s.traces.average_length = Some(-0.0);
    s.traces.total_vertical_length = Some(1234567.9);
    s.traces.total_horizontal_length = Some(3.4028235E38);
    s.traces.total_angled_length = Some(1.4E-45);
    s.bends.total_count = Some(27);
    s.bends.ninety_degree_count = Some(28);
    s.bends.forty_five_degree_count = Some(29);
    s.bends.other_angle_count = Some(30);
    s.vias.total_count = Some(31);
    s.vias.through_hole_count = Some(32);
    s.vias.blind_count = Some(33);
    s.vias.buried_count = Some(34);
    s.clearance_violations.total_count = Some(35);
    s.clearance_violations.min_violation_um = Some(0.1);
    s.clearance_violations.max_violation_um = Some(1.0E7);
    s.clearance_violations.avg_violation_um = Some(-9.999999999999999E-4);
    s.fanout = BoardStatisticsFanout {
        total_smd_pins: 36,
        pins_to_escape: 37,
        escaped_count: 38,
    };
    s
}

fn fanout_only() -> BoardStatistics {
    let mut s = BoardStatistics::default();
    s.fanout.total_smd_pins = 7;
    s
}

// ==================================================================================================
// plumbing
// ==================================================================================================

fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out
}
