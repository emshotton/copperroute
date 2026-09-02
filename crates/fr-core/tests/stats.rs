//! Plan 8 Task 2's acceptance: the text-scraping `BoardStatistics(byte[], FileFormat)`
//! (core/scoring/BoardStatistics.java:436-552), `countOccurrences` (`:578-586`) and the Gson JSON
//! surface `toString` (`:588-591`), against the HEAD jar.
//!
//! # The transcript
//!
//! `scripts/differential/java/probes/P8T2Probe.java` drives the jar over **every corpus `.dsn`**
//! (`tests/reference/fixtures.txt`'s seven stems, both the Java checkout's source file and this
//! repository's committed `roundtrip.dsn`), **every committed `.ses`** (the seven
//! `unrouted.ses`), **every `batch.ses`** (`tests/reference/router-fixtures.txt`'s eight batch
//! stems), thirty-eight synthetic edge cases and three hand-built `BoardStatistics`, printing
//! the fifty DTO fields and the byte-exact `toString()` for each. Its output is committed
//! verbatim as `tests/data/p8t2-byte-statistics.txt`, and `scripts/differential/run.sh p8t2probe`
//! diffs it live against `scripts/differential/rust/src/bin/p8t2probe.rs` — **MATCH on all 244
//! lines**, against
//! `../freerouting/build/libs/freerouting-current-executable.jar` (JDK 25,
//! `-Djava.awt.headless=true -Duser.language=en -Duser.country=US
//! -XX:+UnlockExperimentalVMOptions -XX:hashCode=2`).
//!
//! [`TRANSCRIPT`] is that file transcribed into Rust line for line, exactly as Task 0's timespan
//! table and Task 1's job model are: a test that reads its expectations out of a file can agree
//! with itself while disagreeing with the jar.
//! [`the_committed_transcript_still_says_what_this_table_says`] then re-reads the file and
//! requires the two to agree, and [`the_port_reproduces_every_transcript_row`] re-derives every
//! row from the port and compares it against the literal.
//!
//! # The two `XDIFF` rows
//!
//! | row | Java | the port |
//! |---|---|---|
//! | `(parser (hostCad))` — `substring(hcIdx + 9, hcEnd)` with `hcIdx + 9 > hcEnd` (`:493`) | `StringIndexOutOfBoundsException` out of the constructor, quirk #250 | the start is clamped to the end, so the value is the empty string the slice would have held and `host` is Java's `null` |
//! | `(parser (hostCad  ))` — an *empty* `hostCad` (`:509-510`) | `host` is `""`, and Gson prints `"host": ""` | `fr_router::score::BoardStatistics.host` is a `String`, so the port spells Java's `null` as `""` too and omits the key, quirk #251 |
//!
//! Both rows print `java=…` and `rust=…` on **both** sides, so the driver's diff is still empty
//! while the transcript records the divergence.
//!
//! # Rows the sibling Java checkout gates
//!
//! Fourteen `BS` rows read a file out of `$FREEROUTING_JAVA_DIR` (the seven `source.dsn`s and the
//! two `AS SES`/`AS DSN` re-reads). They are skipped, loudly, on a checkout without the sibling
//! clone — `parity::require_java_dir`'s convention. Every other row travels with this repository.

use fr_core::{
    BoardStatistics, BoardStatisticsExt, FileFormat, count_occurrences, to_gson_json,
    to_gson_string,
};
use fr_dsn::{java_double_to_string, java_float_to_string};
use fr_router::score::{BoardStatisticsFanout, Rectangle2DFloat};

/// The FLD column order: `BoardStatistics.java:37-79`, then each DTO's own declaration order.
/// Checked against the transcript's own `HDR` row, so a reordering cannot pass silently.
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

/// `tests/data/p8t2-byte-statistics.txt`, transcribed line for line. Regenerate with
/// `scripts/differential/run.sh p8t2probe`, which prints the same 244 lines from the jar and
/// from the port and diffs them.
#[rustfmt::skip]
const TRANSCRIPT: &[&str] = &[
    "HDR\thost\tunit\tboard.bounding_box.x\tboard.bounding_box.y\tboard.bounding_box.width\tboard.bounding_box.height\tboard.size.x\tboard.size.y\tboard.size.width\tboard.size.height\tlayers.total_count\tlayers.signal_count\titems.total_count\titems.trace_count\titems.via_count\titems.conduction_area_count\titems.drill_item_count\titems.pin_count\titems.component_count\titems.other_count\tcomponents.total_count\tpads.total_count\tnets.total_count\tnets.class_count\tconnections.maximum_count\tconnections.incomplete_count\ttraces.total_count\ttraces.total_segment_count\ttraces.total_length\ttraces.total_length_mm\ttraces.total_weighted_length\ttraces.average_length\ttraces.total_vertical_length\ttraces.total_horizontal_length\ttraces.total_angled_length\tbends.total_count\tbends.90_degree_count\tbends.45_degree_count\tbends.other_angle_count\tvias.total_count\tvias.through_hole_count\tvias.blind_count\tvias.buried_count\tclearance_violations.total_count\tclearance_violations.min_violation_um\tclearance_violations.max_violation_um\tclearance_violations.avg_violation_um\tfanout.total_smd_pins\tfanout.pins_to_escape\tfanout.escaped_count",
    "COUNT\t0\t\t(net\t0",
    "COUNT\t1\t(net\t(net\t1",
    "COUNT\t2\t(network\t(net\t1",
    "COUNT\t3\t(net_class\t(net\t1",
    "COUNT\t4\t(net (net (net\t(net\t3",
    "COUNT\t5\t(layer_rule (layer TOP)\t(layer\t2",
    "COUNT\t6\t(via_rule (via V1)\t(via\t2",
    "COUNT\t7\t(class_class (class C)\t(class\t2",
    "COUNT\t8\taaaa\taa\t2",
    "COUNT\t9\t(wire(wire(wire\t(wire\t3",
    "COUNT\t10\t(component\t(componentx\t0",
    "COUNT\t11\té(net\t(net\t1",
    "BS\t0\ttutorial_board/source.dsn\tDSN\tjava:examples/tutorial_board/tutorial_board.dsn",
    "FLD\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t439\t1\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t1\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t0\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 439,\\n    \"class_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 1\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t1\ttutorial_board/roundtrip.dsn\tDSN\tref:tutorial_board/roundtrip.dsn",
    "FLD\t1\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t877\t1\t<null>\t<null>\t438\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t6\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t1\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 877,\\n    \"class_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 438\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 6\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t2\ttutorial_board/unrouted.ses\tSES\tref:tutorial_board/unrouted.ses",
    "FLD\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t1\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t2\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t3\tIssue026-J2_reference/source.dsn\tDSN\tjava:fixtures/Issue026-J2_reference.dsn",
    "FLD\t3\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t25\t1\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t1\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t3\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 2\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 25,\\n    \"class_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 1\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t4\tIssue026-J2_reference/roundtrip.dsn\tDSN\tref:Issue026-J2_reference/roundtrip.dsn",
    "FLD\t4\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t25\t1\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t6\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t4\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 2\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 25,\\n    \"class_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 6\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t5\tIssue026-J2_reference/unrouted.ses\tSES\tref:Issue026-J2_reference/unrouted.ses",
    "FLD\t5\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t1\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t5\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 2\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t6\tIssue103-Board-Unrouted/source.dsn\tDSN\tjava:fixtures/Issue103-Board-Unrouted.dsn",
    "FLD\t6\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t5\t<null>\t283\t1\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t1\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t6\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 5\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 283,\\n    \"class_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 1\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t7\tIssue103-Board-Unrouted/roundtrip.dsn\tDSN\tref:Issue103-Board-Unrouted/roundtrip.dsn",
    "FLD\t7\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t5\t<null>\t283\t1\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t6\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t7\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 5\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 283,\\n    \"class_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 6\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t8\tIssue103-Board-Unrouted/unrouted.ses\tSES\tref:Issue103-Board-Unrouted/unrouted.ses",
    "FLD\t8\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t5\t<null>\t1\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t8\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 5\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t9\tIssue143-rpi_splitter/source.dsn\tDSN\tjava:fixtures/Issue143-rpi_splitter.dsn",
    "FLD\t9\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t3\t<null>\t6\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t3\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t9\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 3\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 6,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 3\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t10\tIssue143-rpi_splitter/roundtrip.dsn\tDSN\tref:Issue143-rpi_splitter/roundtrip.dsn",
    "FLD\t10\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t6\t1\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t5\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t10\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 2\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 6,\\n    \"class_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 5\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t11\tIssue143-rpi_splitter/unrouted.ses\tSES\tref:Issue143-rpi_splitter/unrouted.ses",
    "FLD\t11\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t1\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t11\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 2\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t12\tIssue413-test/source.dsn\tDSN\tjava:fixtures/Issue413-test.dsn",
    "FLD\t12\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t1\t<null>\t21\t2\t<null>\t<null>\t11\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t12\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t12\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 1\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 21,\\n    \"class_count\": 2\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 11\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 12\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t13\tIssue413-test/roundtrip.dsn\tDSN\tref:Issue413-test/roundtrip.dsn",
    "FLD\t13\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t1\t<null>\t21\t1\t<null>\t<null>\t11\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t11\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t13\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 1\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 21,\\n    \"class_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 11\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 11\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t14\tIssue413-test/unrouted.ses\tSES\tref:Issue413-test/unrouted.ses",
    "FLD\t14\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t1\t<null>\t5\t<null>\t<null>\t<null>\t11\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t4\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t14\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 1\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 5\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 11\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 4\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t15\tIssue110-RelayModule/source.dsn\tDSN\tjava:fixtures/Issue110-RelayModule.dsn",
    "FLD\t15\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t33\t<null>\t87\t2\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t23\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t15\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 33\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 87,\\n    \"class_count\": 2\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 23\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t16\tIssue110-RelayModule/roundtrip.dsn\tDSN\tref:Issue110-RelayModule/roundtrip.dsn",
    "FLD\t16\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t19\t<null>\t87\t2\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t31\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t16\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 19\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 87,\\n    \"class_count\": 2\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 31\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t17\tIssue110-RelayModule/unrouted.ses\tSES\tref:Issue110-RelayModule/unrouted.ses",
    "FLD\t17\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t19\t<null>\t17\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t22\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t17\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 19\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 17\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 22\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t18\tIssue753-CPU-85_r104/source.dsn\tDSN\tjava:fixtures/Issue753-CPU-85_r104.dsn",
    "FLD\t18\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t4\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t31\t<null>\t293\t1\t<null>\t<null>\t65\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t20\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t18\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 4\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 31\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 293,\\n    \"class_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 65\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 20\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t19\tIssue753-CPU-85_r104/roundtrip.dsn\tDSN\tref:Issue753-CPU-85_r104/roundtrip.dsn",
    "FLD\t19\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t4\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t22\t<null>\t293\t1\t<null>\t<null>\t65\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t25\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t19\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 4\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 22\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 293,\\n    \"class_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 65\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 25\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t20\tIssue753-CPU-85_r104/unrouted.ses\tSES\tref:Issue753-CPU-85_r104/unrouted.ses",
    "FLD\t20\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t22\t<null>\t1\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t20\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 22\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t21\trouter-rpi-splitter/batch.ses\tSES\tref:router-rpi-splitter/batch.ses",
    "FLD\t21\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t5\t<null>\t<null>\t<null>\t16\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t9\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t21\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 2\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 5\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 16\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 9\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t22\trouter-dac2020-bm01/batch.ses\tSES\tref:router-dac2020-bm01/batch.ses",
    "FLD\t22\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t20\t<null>\t77\t<null>\t<null>\t<null>\t367\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t81\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t22\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 20\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 77\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 367\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 81\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t23\trouter-j2-reference/batch.ses\tSES\tref:router-j2-reference/batch.ses",
    "FLD\t23\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t13\t<null>\t<null>\t<null>\t99\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t16\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t23\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 2\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 13\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 99\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 16\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t24\trouter-tutorial-board/batch.ses\tSES\tref:router-tutorial-board/batch.ses",
    "FLD\t24\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t1\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t24\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t25\trouter-ecc83-input/batch.ses\tSES\tref:router-ecc83-input/batch.ses",
    "FLD\t25\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t6\t<null>\t9\t<null>\t<null>\t<null>\t18\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t25\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 6\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 9\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 18\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t26\trouter-fanout-bm11/batch.ses\tSES\tref:router-fanout-bm11/batch.ses",
    "FLD\t26\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t4\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t21\t<null>\t36\t<null>\t<null>\t<null>\t335\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t53\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t26\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 4\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 21\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 36\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 335\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 53\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t27\trouter-strict-drc-cnh/batch.ses\tSES\tref:router-strict-drc-cnh/batch.ses",
    "FLD\t27\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t17\t<null>\t58\t<null>\t<null>\t<null>\t206\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t4\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t27\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 17\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 58\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 206\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 4\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t28\trouter-empty-board/batch.ses\tSES\tref:router-empty-board/batch.ses",
    "FLD\t28\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t1\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t28\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t29\tIssue143-rpi_splitter/source.dsn AS SES\tSES\tjava:fixtures/Issue143-rpi_splitter.dsn",
    "FLD\t29\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t1\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t3\t<null>\t6\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t3\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t29\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 1\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 3\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 6\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 3\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t30\tIssue143-rpi_splitter/unrouted.ses AS DSN\tDSN\tref:Issue143-rpi_splitter/unrouted.ses",
    "FLD\t30\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t1\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t30\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 2\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 1,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t31\tnull data\tDSN\tnull",
    "FLD\t31\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t31\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t32\tnull format\t<null>\ttxt:(pcb x",
    "FLD\t32\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t32\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t33\tno branch UNKNOWN\tUNKNOWN\ttxt:(pcb x",
    "FLD\t33\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t33\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t34\tno branch FRB\tFRB\ttxt:(pcb x",
    "FLD\t34\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t34\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t35\tno branch RULES\tRULES\ttxt:(pcb x",
    "FLD\t35\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t35\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t36\tno branch SCR\tSCR\ttxt:(pcb x",
    "FLD\t36\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t36\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t37\tno branch DRC_JSON\tDRC_JSON\ttxt:(pcb x",
    "FLD\t37\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t37\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t38\tno branch KICAD_SESSION_JSON\tKICAD_SESSION_JSON\ttxt:(pcb x",
    "FLD\t38\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t38\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t39\tempty SES\tSES\ttxt:",
    "FLD\t39\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t39\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t40\tempty DSN\tDSN\ttxt:",
    "FLD\t40\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t40\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t41\tempty KiCad JSON\tKICAD_DESIGN_JSON\ttxt:",
    "FLD\t41\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t41\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t42\tbinary junk SES\tSES\thex:00ff01fe0210286e6574",
    "FLD\t42\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t1\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t42\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t43\tbinary junk DSN\tDSN\thex:00ff01fe0210286c61796572",
    "FLD\t43\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t1\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t43\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 1\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t44\tquirk F DSN keywords\tDSN\ttxt:(layer_rule)(layer TOP)(network X)(net_class Y)(net N)(via_rule R)(via V)(class_class Z)(class C)(component U1)(wire)",
    "FLD\t44\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t1\t<null>\t3\t2\t<null>\t<null>\t1\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t44\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 1\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 3,\\n    \"class_count\": 2\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 1\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 2\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t45\tquirk F SES keywords\tSES\ttxt:(component U1)(net N)(network X)(net_class Y)(wire)(via V)(via_rule R)",
    "FLD\t45\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t1\t<null>\t3\t<null>\t<null>\t<null>\t1\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t45\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 1\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 3\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 1\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 2\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t46\tSES no path\tSES\ttxt:(session x)",
    "FLD\t46\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t46\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t47\tSES one path\tSES\ttxt:(wire (path F.Cu 250 1 2 3 4))",
    "FLD\t47\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t1\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t<null>\t<null>\t<null>\t1\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t47\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 1\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 1\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t48\tSES two layers\tSES\ttxt:(path F.Cu 250 1 2)(path B.Cu 250 3 4)",
    "FLD\t48\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t48\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t49\tSES repeated layer\tSES\ttxt:(path F.Cu 250 1 2)(path F.Cu 250 3 4)",
    "FLD\t49\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t1\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t49\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 1\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t50\tSES leading path\tSES\ttxt:(path F.Cu 250 1 2)",
    "FLD\t50\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t1\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t50\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 1\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t51\tSES one-word chunk\tSES\ttxt:a(path F.Cu",
    "FLD\t51\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t51\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t52\tSES empty chunk\tSES\ttxt:a(path (path B.Cu 1 2",
    "FLD\t52\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t1\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t52\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 1\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t53\tSES trailing path\tSES\ttxt:a(path ",
    "FLD\t53\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t53\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t54\treal DSN parser scope\tDSN\ttxt:(pcb x\\n  (parser\\n    (string_quote \")\\n    (host_cad \"KiCad's Pcbnew\")\\n    (host_version \"8.0.4\")\\n  )\\n)",
    "FLD\t54\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t54\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t55\tcamelCase, both\tDSN\ttxt:(parser (hostCad \"KiCad\" (hostVersion \"8.0\" ))",
    "FLD\t55\t\"KiCad\" (hostVersion \"8.0,8.0\"\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t55\t{\\n  \"host\": \"KiCad\\\\\" (hostVersion \\\\\"8.0,8.0\",\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t56\tcamelCase, cad only\tDSN\ttxt:(parser (hostCad \"KiCad\" ))",
    "FLD\t56\t\"KiCad\"\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t56\t{\\n  \"host\": \"KiCad\",\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t57\tcamelCase, version only\tDSN\ttxt:(parser (hostVersion \"8.0\" ))",
    "FLD\t57\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t57\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t58\tcamelCase, two spaces\tDSN\ttxt:(parser (hostCad  \"KiCad\" ))",
    "FLD\t58\t\"KiCad\"\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t58\t{\\n  \"host\": \"KiCad\",\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t59\tcamelCase, no space\tDSN\ttxt:(parser (hostCad\"KiCad\" ))",
    "FLD\t59\t\"KiCad\"\"\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t59\t{\\n  \"host\": \"KiCad\\\\\"\",\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t60\tcamelCase, unquoted\tDSN\ttxt:(parser (hostCad KiCad (hostVersion 8.0 ))",
    "FLD\t60\t\"KiCad (hostVersion 8.0,8.0\"\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t60\t{\\n  \"host\": \"KiCad (hostVersion 8.0,8.0\",\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t61\tno parser scope\tDSN\ttxt:(pcb x (structure))",
    "FLD\t61\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t61\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t62\tparser, no close\tDSN\ttxt:(parser (hostCad \"K\" (hostVersion \"8\" ",
    "FLD\t62\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t62\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t63\tparser, no close, >1000\tDSN\ttxt:(parser (hostCad \"K\" xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx (hostVersion \"8\" ",
    "FLD\t63\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t63\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t64\thost is not unescaped\tDSN\ttxt:(parser (hostCad \"K\\\\u0041D\" (hostVersion \"8\" ))",
    "FLD\t64\t\"K\\\\u0041D\" (hostVersion \"8,8\"\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t64\t{\\n  \"host\": \"K\\\\\\\\u0041D\\\\\" (hostVersion \\\\\"8,8\",\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t65\tempty hostCad\tDSN\ttxt:(parser (hostCad  ))",
    "FLD\t65\t\"\"\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t0\t0\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "XDIFF\t65\tjava=host=\"\"\trust=host=<null>",
    "JSON\t65\tXDIFF\tjava={\\n  \"host\": \"\",\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}\trust={\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 0\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 0\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 0,\\n    \"class_count\": 0\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 0\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t66\tinverted substring\tDSN\ttxt:(parser (hostCad))",
    "XDIFF\t66\tjava=StringIndexOutOfBoundsException\trust=host=<omitted>",
    "BS\t67\tkicad full\tKICAD_DESIGN_JSON\ttxt:{\"layers\":[1,2,3,4],\"components\":[{},{}],\"netClasses\":[{}],\"nets\":[1,2,3],\"traces\":[],\"vias\":[{},{},{}],\"designName\":\"board\"}",
    "FLD\t67\t\"KiCad JSON,board\"\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t4\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t3\t1\t<null>\t<null>\t0\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t3\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t67\t{\\n  \"host\": \"KiCad JSON,board\",\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 4\\n  },\\n  \"items\": {},\\n  \"components\": {\\n    \"total_count\": 2\\n  },\\n  \"pads\": {},\\n  \"nets\": {\\n    \"total_count\": 3,\\n    \"class_count\": 1\\n  },\\n  \"connections\": {},\\n  \"traces\": {\\n    \"total_count\": 0\\n  },\\n  \"bends\": {},\\n  \"vias\": {\\n    \"total_count\": 3\\n  },\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t68\tkicad empty object\tKICAD_DESIGN_JSON\ttxt:{}",
    "FLD\t68\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t68\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t69\tkicad array\tKICAD_DESIGN_JSON\ttxt:[1,2]",
    "FLD\t69\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t69\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t70\tkicad null literal\tKICAD_DESIGN_JSON\ttxt:null",
    "FLD\t70\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t70\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t71\tkicad malformed\tKICAD_DESIGN_JSON\ttxt:{\"layers\":",
    "FLD\t71\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t71\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t72\tkicad wrong type mid-way\tKICAD_DESIGN_JSON\ttxt:{\"layers\":[1,2],\"components\":7,\"nets\":[1]}",
    "FLD\t72\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t2\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t72\t{\\n  \"board\": {},\\n  \"layers\": {\\n    \"total_count\": 2\\n  },\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t73\tkicad numeric designName\tKICAD_DESIGN_JSON\ttxt:{\"designName\":42}",
    "FLD\t73\t\"KiCad JSON,42\"\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t73\t{\\n  \"host\": \"KiCad JSON,42\",\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t74\tempty\t<none>\tsynth:empty",
    "FLD\t74\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t0\t0\t0",
    "JSON\t74\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 0,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
    "BS\t75\tpopulated\t<none>\tsynth:populated",
    "FLD\t75\t\"KiCad's \"Pcbnew\",8.0.4 é\"\t\"um\"\t1.5\t-2.25\t-1000000.5\t0.1\t0.0\t0.0\t1.0E8\t3.0E-4\t4\t2\t11\t12\t13\t14\t15\t16\t17\t18\t19\t20\t21\t22\t23\t24\t25\t26\t0.1\t1.0E7\t9.999999E-4\t-0.0\t1234567.9\t3.4028235E38\t1.4E-45\t27\t28\t29\t30\t31\t32\t33\t34\t35\t0.1\t1.0E7\t-9.999999999999998E-4\t36\t37\t38",
    "JSON\t75\t{\\n  \"host\": \"KiCad's \\\\\"Pcbnew\\\\\",8.0.4 é\",\\n  \"unit\": \"um\",\\n  \"board\": {\\n    \"bounding_box\": {\\n      \"x\": 1.5,\\n      \"y\": -2.25,\\n      \"width\": -1000000.5,\\n      \"height\": 0.1\\n    },\\n    \"size\": {\\n      \"x\": 0.0,\\n      \"y\": 0.0,\\n      \"width\": 1.0E8,\\n      \"height\": 3.0E-4\\n    }\\n  },\\n  \"layers\": {\\n    \"total_count\": 4,\\n    \"signal_count\": 2\\n  },\\n  \"items\": {\\n    \"total_count\": 11,\\n    \"trace_count\": 12,\\n    \"via_count\": 13,\\n    \"conduction_area_count\": 14,\\n    \"drill_item_count\": 15,\\n    \"pin_count\": 16,\\n    \"component_count\": 17,\\n    \"other_count\": 18\\n  },\\n  \"components\": {\\n    \"total_count\": 19\\n  },\\n  \"pads\": {\\n    \"total_count\": 20\\n  },\\n  \"nets\": {\\n    \"total_count\": 21,\\n    \"class_count\": 22\\n  },\\n  \"connections\": {\\n    \"maximum_count\": 23,\\n    \"incomplete_count\": 24\\n  },\\n  \"traces\": {\\n    \"total_count\": 25,\\n    \"total_segment_count\": 26,\\n    \"total_length\": 0.1,\\n    \"total_length_mm\": 1.0E7,\\n    \"total_weighted_length\": 9.999999E-4,\\n    \"average_length\": -0.0,\\n    \"total_vertical_length\": 1234567.9,\\n    \"total_horizontal_length\": 3.4028235E38,\\n    \"total_angled_length\": 1.4E-45\\n  },\\n  \"bends\": {\\n    \"total_count\": 27,\\n    \"90_degree_count\": 28,\\n    \"45_degree_count\": 29,\\n    \"other_angle_count\": 30\\n  },\\n  \"vias\": {\\n    \"total_count\": 31,\\n    \"through_hole_count\": 32,\\n    \"blind_count\": 33,\\n    \"buried_count\": 34\\n  },\\n  \"clearance_violations\": {\\n    \"total_count\": 35,\\n    \"min_violation_um\": 0.1,\\n    \"max_violation_um\": 1.0E7,\\n    \"avg_violation_um\": -9.999999999999998E-4\\n  },\\n  \"fanout\": {\\n    \"total_smd_pins\": 36,\\n    \"pins_to_escape\": 37,\\n    \"escaped_count\": 38\\n  }\\n}",
    "BS\t76\tfanout only\t<none>\tsynth:fanout-only",
    "FLD\t76\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t<null>\t7\t0\t0",
    "JSON\t76\t{\\n  \"board\": {},\\n  \"layers\": {},\\n  \"items\": {},\\n  \"components\": {},\\n  \"pads\": {},\\n  \"nets\": {},\\n  \"connections\": {},\\n  \"traces\": {},\\n  \"bends\": {},\\n  \"vias\": {},\\n  \"clearance_violations\": {},\\n  \"fanout\": {\\n    \"total_smd_pins\": 7,\\n    \"pins_to_escape\": 0,\\n    \"escaped_count\": 0\\n  }\\n}",
];

// =================================================================================================
// The two whole-transcript tests
// =================================================================================================

#[test]
fn the_committed_transcript_still_says_what_this_table_says() {
    let file = include_str!("data/p8t2-byte-statistics.txt");
    let file_lines: Vec<&str> = file.lines().collect();
    assert_eq!(
        file_lines.len(),
        TRANSCRIPT.len(),
        "tests/data/p8t2-byte-statistics.txt has {} lines, the table has {}",
        file_lines.len(),
        TRANSCRIPT.len()
    );
    for (i, (a, b)) in file_lines.iter().zip(TRANSCRIPT.iter()).enumerate() {
        assert_eq!(a, b, "line {} of the committed transcript", i + 1);
    }
}

#[test]
fn the_port_reproduces_every_transcript_row() {
    let have_java = parity::require_java_dir();
    let mut mismatches = Vec::new();
    let mut skipped = 0usize;
    let mut checked = 0usize;

    // The `BS` row that the following `FLD` / `JSON` / `XDIFF` rows belong to.
    let mut current: Option<(usize, BoardStatistics)> = None;

    for (line_number, line) in TRANSCRIPT.iter().enumerate() {
        let columns: Vec<&str> = line.split('\t').collect();
        let mut check = |expected: &str, actual: String| {
            checked += 1;
            if expected != actual {
                mismatches.push(format!(
                    "line {}\n  java: {expected}\n  rust: {actual}",
                    line_number + 1
                ));
            }
        };
        match columns[0] {
            "HDR" => check(line, format!("HDR\t{}", FIELD_PATHS.join("\t"))),
            "COUNT" => {
                let haystack = unescape(columns[2]);
                let needle = unescape(columns[3]);
                check(
                    line,
                    format!(
                        "COUNT\t{}\t{}\t{}\t{}",
                        columns[1],
                        columns[2],
                        columns[3],
                        count_occurrences(&haystack, &needle)
                    ),
                );
            }
            "BS" => {
                let index: usize = columns[1].parse().expect("a row index");
                let source = columns[4];
                if source.starts_with("java:") && !have_java {
                    skipped += 1;
                    current = None;
                    continue;
                }
                current = Some((index, statistics_of(columns[3], source)));
            }
            "FLD" => {
                let Some((index, stats)) = &current else {
                    continue;
                };
                let mut values = fields(stats);
                // The `empty hostCad` row's `host` column is the jar's literal (quirk #251): the
                // port answers `<null>` and the `XDIFF` line below carries both.
                if columns[2] == "\"\"" && values[0] == "<null>" {
                    values[0] = "\"\"".to_string();
                }
                check(line, format!("FLD\t{index}\t{}", values.join("\t")));
            }
            "XDIFF" => {
                // Neither divergence can be re-derived from the port — that is what makes it a
                // divergence — so the row is asserted to be one of the two this task recorded.
                checked += 1;
                let recorded = [
                    "java=StringIndexOutOfBoundsException\trust=host=<omitted>",
                    "java=host=\"\"\trust=host=<null>",
                ];
                let tail = columns[2..].join("\t");
                if !recorded.contains(&tail.as_str()) {
                    mismatches.push(format!(
                        "line {}\n  an XDIFF row this task did not record: {tail}",
                        line_number + 1
                    ));
                }
            }
            "JSON" => {
                let Some((index, stats)) = &current else {
                    continue;
                };
                let json = escape(&to_gson_string(stats));
                if columns[2] == "XDIFF" {
                    // Quirk #251: the `java=` half is the jar's, the `rust=` half is the port's.
                    check(
                        line,
                        format!("JSON\t{index}\tXDIFF\t{}\trust={json}", columns[3]),
                    );
                } else {
                    check(line, format!("JSON\t{index}\t{json}"));
                }
            }
            other => panic!("line {}: unknown row type {other}", line_number + 1),
        }
    }

    assert!(
        mismatches.is_empty(),
        "{} of {checked} transcript rows disagree with the port:\n\n{}",
        mismatches.len(),
        mismatches.join("\n\n")
    );
    // A rebuilder that silently matched nothing would pass every assertion above. Every line but
    // the `BS` header of a row is checked, less the two lines each skipped `java:` row carries.
    let bs_rows = TRANSCRIPT.iter().filter(|l| l.starts_with("BS\t")).count();
    assert_eq!(
        checked,
        TRANSCRIPT.len() - bs_rows - 2 * skipped,
        "{checked} rows rebuilt out of {} lines, {bs_rows} of them `BS` headers and {skipped} rows skipped",
        TRANSCRIPT.len()
    );
    if have_java {
        assert_eq!(
            skipped, 0,
            "no row should be skipped with the clone present"
        );
    }
}

// =================================================================================================
// The named behaviours the plan asks for by name
// =================================================================================================

/// Quirk #248. `searchLimit` is the **first `)` after `(parser`** (`:482`), which in a real
/// Specctra DSN closes `(string_quote ")`, so `(host_cad …)` is outside `parserScope`; and the
/// keywords Java looks for are the camelCase `(hostCad` / `(hostVersion`, which the Specctra
/// grammar never writes. Both together mean the scrape has never once succeeded on a real file.
#[test]
fn the_dsn_host_scrape_finds_nothing_on_a_real_dsn() {
    let dsn = b"(pcb x\n  (parser\n    (string_quote \")\n    (host_cad \"KiCad's Pcbnew\")\n\
                \n    (host_version \"8.0.4\")\n  )\n)";
    let stats = BoardStatistics::from_bytes(dsn, FileFormat::Dsn);
    assert_eq!(
        stats.host, "",
        "the port spells Java's null host as the empty string"
    );
    assert!(!to_gson_string(&stats).contains("\"host\""));

    // Every committed corpus DSN agrees, which is the transcript's twenty-one `roundtrip.dsn` /
    // `source.dsn` rows in one assertion.
    for stem in [
        "tutorial_board",
        "Issue026-J2_reference",
        "Issue103-Board-Unrouted",
        "Issue143-rpi_splitter",
        "Issue413-test",
        "Issue110-RelayModule",
        "Issue753-CPU-85_r104",
    ] {
        let path = parity::reference(stem, "roundtrip.dsn");
        let data = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let stats = BoardStatistics::from_bytes(&data, FileFormat::Dsn);
        assert_eq!(stats.host, "", "{stem}");
    }

    // The camelCase spelling *does* match — which is how the quirk is provably about the
    // keywords and the scope, not about the port's transcription.
    let camel = b"(parser (hostCad \"KiCad\" (hostVersion \"8.0\" ))";
    let stats = BoardStatistics::from_bytes(camel, FileFormat::Dsn);
    assert_eq!(stats.host, "KiCad\" (hostVersion \"8.0,8.0");
}

/// Quirk #247. `countOccurrences` is a substring count, so every keyword also matches its longer
/// relatives — and the SES layer scrape drops the chunk before the first `"(path "` and any chunk
/// with no space in it.
#[test]
fn layer_rule_is_counted_as_a_layer() {
    let dsn = b"(layer_rule)(layer TOP)(network X)(net_class Y)(net N)(via_rule R)(via V)\
                (class_class Z)(class C)(component U1)(wire)";
    let stats = BoardStatistics::from_bytes(dsn, FileFormat::Dsn);
    assert_eq!(
        stats.layers.total_count,
        Some(2),
        "`(layer` counts `(layer_rule`"
    );
    assert_eq!(
        stats.nets.total_count,
        Some(3),
        "`(net` counts `(network` and `(net_class`"
    );
    assert_eq!(stats.vias.total_count, Some(2), "`(via` counts `(via_rule`");
    assert_eq!(
        stats.nets.class_count,
        Some(2),
        "`(class` counts `(class_class`"
    );

    // Non-overlapping: the cursor advances by the whole needle (`:583`).
    assert_eq!(count_occurrences("aaaa", "aa"), 2);
    // totalized: an empty needle spins for ever in Java (quirk #252).
    assert_eq!(count_occurrences("aaaa", ""), 0);

    // The SES layer scrape's own off-by-one: the chunk **before** the first `"(path "` is never
    // read, and `"a(path F.Cu"` — one word, no trailing space — is dropped by `words.length >= 2`.
    let one = BoardStatistics::from_bytes(b"(path F.Cu 250 1 2)", FileFormat::Ses);
    assert_eq!(one.layers.total_count, Some(1));
    let dropped = BoardStatistics::from_bytes(b"a(path F.Cu", FileFormat::Ses);
    assert_eq!(dropped.layers.total_count, Some(0));
    let repeated =
        BoardStatistics::from_bytes(b"(path F.Cu 250 1 2)(path F.Cu 250 3 4)", FileFormat::Ses);
    assert_eq!(repeated.layers.total_count, Some(1), "distinct names only");
}

/// `BoardStatisticsBends.java:12` and `:15`: two `@SerializedName`s begin with a digit, which is
/// legal JSON and is not derivable from the Java field names.
#[test]
fn bends_keys_start_with_digits() {
    let mut stats = BoardStatistics::default();
    stats.bends.total_count = Some(1);
    stats.bends.ninety_degree_count = Some(2);
    stats.bends.forty_five_degree_count = Some(3);
    stats.bends.other_angle_count = Some(4);
    let json = to_gson_string(&stats);
    assert!(json.contains("\"90_degree_count\": 2"), "{json}");
    assert!(json.contains("\"45_degree_count\": 3"), "{json}");
    assert!(!json.contains("ninety"), "{json}");
    assert!(!json.contains("forty"), "{json}");
}

/// `BoardStatisticsItems.java:27-28`: `componentOutlineCount` serialises as `component_count`,
/// which collides with `components.total_count` two objects later. Java's name, not a slip.
#[test]
fn component_outline_count_serialises_as_component_count() {
    let mut stats = BoardStatistics::default();
    stats.items.component_outline_count = Some(7);
    stats.components.total_count = Some(9);
    let json = to_gson_string(&stats);
    assert!(
        json.contains("  \"items\": {\n    \"component_count\": 7\n  },"),
        "{json}"
    );
    assert!(
        json.contains("  \"components\": {\n    \"total_count\": 9\n  },"),
        "{json}"
    );
    assert!(!json.contains("component_outline"), "{json}");
}

/// Gson's reflective adapter takes the key order from the field declarations
/// (`BoardStatistics.java:37-79`) and omits every null; `GsonProvider` never calls
/// `serializeNulls()`.
#[test]
fn the_json_key_order_is_declaration_order_and_nulls_are_omitted() {
    let empty = to_gson_string(&BoardStatistics::default());
    let expected_empty = "{\n  \"board\": {},\n  \"layers\": {},\n  \"items\": {},\n  \
         \"components\": {},\n  \"pads\": {},\n  \"nets\": {},\n  \"connections\": {},\n  \
         \"traces\": {},\n  \"bends\": {},\n  \"vias\": {},\n  \"clearance_violations\": {},\n  \
         \"fanout\": {\n    \"total_smd_pins\": 0,\n    \"pins_to_escape\": 0,\n    \
         \"escaped_count\": 0\n  }\n}";
    assert_eq!(empty, expected_empty);

    // The two `String` fields lead, in that order, when they are set.
    let stats = BoardStatistics {
        host: "KiCad,8.0".to_string(),
        unit: "um".to_string(),
        ..Default::default()
    };
    let json = to_gson_string(&stats);
    assert!(
        json.starts_with("{\n  \"host\": \"KiCad,8.0\",\n  \"unit\": \"um\",\n  \"board\": {},"),
        "{json}"
    );

    // Every key, in order, from the fully populated object the probe's `synth:populated` row
    // pins byte for byte.
    let populated_json = to_gson_string(&populated());
    let keys: Vec<&str> = populated_json
        .lines()
        .filter(|l| l.starts_with("  \""))
        .map(|l| l.trim_start().split('"').nth(1).expect("a key"))
        .collect();
    assert_eq!(
        keys,
        [
            "host",
            "unit",
            "board",
            "layers",
            "items",
            "components",
            "pads",
            "nets",
            "connections",
            "traces",
            "bends",
            "vias",
            "clearance_violations",
            "fanout",
        ]
    );

    // No trailing newline: `Gson.toJson` returns the document and nothing after it.
    assert!(!empty.ends_with('\n'));
}

/// Scan ruling R16: twelve DTOs, not ten. `clearance_violations` is `fr-drc`'s type and its last
/// three fields are `Double` (so `Double.toString`, not `Float.toString`), and `fanout`'s three
/// fields are primitive `int`, so Gson emits `0` rather than omitting them.
#[test]
fn the_fanout_and_clearance_violation_objects_serialise() {
    // The all-default object still carries all three `fanout` keys, and `clearance_violations`
    // is the empty object beside it.
    let json = to_gson_string(&BoardStatistics::default());
    assert!(json.contains("  \"clearance_violations\": {},\n"), "{json}");
    assert!(
        json.contains(
            "  \"fanout\": {\n    \"total_smd_pins\": 0,\n    \"pins_to_escape\": 0,\n    \
             \"escaped_count\": 0\n  }\n}"
        ),
        "{json}"
    );

    let mut stats = BoardStatistics {
        fanout: BoardStatisticsFanout {
            total_smd_pins: 7,
            pins_to_escape: 0,
            escaped_count: 0,
        },
        ..Default::default()
    };
    stats.clearance_violations.total_count = Some(35);
    stats.clearance_violations.min_violation_um = Some(0.1);
    stats.clearance_violations.max_violation_um = Some(1.0E7);
    stats.clearance_violations.avg_violation_um = Some(-9.999999999999999E-4);
    let json = to_gson_string(&stats);
    assert!(
        json.contains(
            "  \"clearance_violations\": {\n    \"total_count\": 35,\n    \
             \"min_violation_um\": 0.1,\n    \"max_violation_um\": 1.0E7,\n    \
             \"avg_violation_um\": -9.999999999999998E-4\n  },"
        ),
        "{json}"
    );
    assert!(
        json.contains("\"total_smd_pins\": 7,\n    \"pins_to_escape\": 0"),
        "{json}"
    );

    // The value form keeps the same twelve DTO keys, even though it alphabetises them.
    let value = to_gson_json(&stats);
    let object = value.as_object().expect("an object");
    assert_eq!(object.len(), 12, "twelve DTOs, no `host` and no `unit`");
    assert!(object.contains_key("clearance_violations"));
    assert!(object.contains_key("fanout"));
}

/// The port's `to_gson_string` panics where `Gson.toJson` throws `IllegalArgumentException`:
/// `GsonProvider` never calls `serializeSpecialFloatingPointValues()`.
#[test]
#[should_panic(expected = "IllegalArgumentException")]
fn a_non_finite_float_is_refused_exactly_where_gson_throws() {
    let mut stats = BoardStatistics::default();
    stats.traces.average_length = Some(f32::NAN);
    let _ = to_gson_string(&stats);
}

// =================================================================================================
// The row rebuilder — the Java probe's printer, transcribed
// =================================================================================================

/// Rebuilds one `BS` row's statistics from its `format` and `src` columns.
fn statistics_of(format: &str, source: &str) -> BoardStatistics {
    match source {
        "null" => BoardStatistics::default(),
        "synth:empty" => BoardStatistics::default(),
        "synth:populated" => populated(),
        "synth:fanout-only" => fanout_only(),
        _ => {
            let data = bytes_of(source);
            match FileFormat::from_java_name(format) {
                // `:437-439`'s null-`FileFormat` arm, which is not expressible in the port's
                // signature and produces exactly the all-default object.
                None => BoardStatistics::default(),
                Some(format) => BoardStatistics::from_bytes(&data, format),
            }
        }
    }
}

fn bytes_of(source: &str) -> Vec<u8> {
    if let Some(text) = source.strip_prefix("txt:") {
        return unescape(text).into_bytes();
    }
    if let Some(hex) = source.strip_prefix("hex:") {
        return (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("two hex digits"))
            .collect();
    }
    let path = if let Some(relative) = source.strip_prefix("java:") {
        parity::java_dir().join(relative)
    } else if let Some(relative) = source.strip_prefix("ref:") {
        parity::workspace_root()
            .join("tests/reference")
            .join(relative)
    } else {
        panic!("unknown source {source}");
    };
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

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
    assert_eq!(v.len(), FIELD_PATHS.len());
    v
}

fn rect(v: &mut Vec<String>, r: Option<&Rectangle2DFloat>) {
    match r {
        None => v.extend(std::iter::repeat_n("<null>".to_string(), 4)),
        Some(r) => {
            v.push(java_float_to_string(r.x));
            v.push(java_float_to_string(r.y));
            v.push(java_float_to_string(r.width));
            v.push(java_float_to_string(r.height));
        }
    }
}

fn int(n: Option<i32>) -> String {
    n.map_or_else(|| "<null>".to_string(), |n| n.to_string())
}

fn float(n: Option<f32>) -> String {
    n.map_or_else(|| "<null>".to_string(), java_float_to_string)
}

fn double(n: Option<f64>) -> String {
    n.map_or_else(|| "<null>".to_string(), java_double_to_string)
}

fn string_field(s: &str) -> String {
    if s.is_empty() {
        "<null>".to_string()
    } else {
        format!("\"{}\"", escape(s))
    }
}

fn escape(s: &str) -> String {
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

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
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

// =================================================================================================
// The two synthetic statistics — `P8T2Probe.populated()` and `fanoutOnly()`
// =================================================================================================

/// Every field non-null and distinct, so the `synth:populated` row pins all fifty in declaration
/// order and every float through `Float.toString` / `Double.toString`.
///
/// Deliberately a second copy of `scripts/differential/rust/src/bin/p8t2probe.rs`'s builder: the
/// differential harness is its own excluded workspace and cannot be imported from here. The two
/// cannot drift, because both are compared against the same committed transcript.
fn populated() -> BoardStatistics {
    let mut s = BoardStatistics {
        host: "KiCad's \"Pcbnew\",8.0.4 é".to_string(),
        unit: "um".to_string(),
        ..Default::default()
    };
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

/// The one DTO whose fields are primitive `int`: Gson emits `0`, it does not omit them.
fn fanout_only() -> BoardStatistics {
    let mut s = BoardStatistics::default();
    s.fanout.total_smd_pins = 7;
    s
}
