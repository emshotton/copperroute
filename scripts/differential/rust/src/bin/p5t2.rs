//! Rust twin of `scripts/differential/java/P5T2.java` (Plan 5 Task 10), algorithm level.
//!
//! The three raw lists behind the report, so a matching report cannot mask a compensating pair of
//! errors (plan-5 ruling 14). `P5T2.java`'s class comment is the specification of every line this
//! prints — the format, and which columns are a parity surface and which are a hash-independent
//! projection of one that is not.
//!
//! Usage: `p5t2 <dsn> [rules|-] [ses|-] <mode 0-4>`, the mode last.

use std::io::{BufWriter, Write};

use fr_board::board::Board;
use fr_board::ids::ItemId;
use fr_board::items::{Item, ItemKind};
use fr_drc::unconnected::UnconnectedKind;
use fr_drc::DesignRulesChecker;
use fr_dsn::format::double::java_double_to_string;

#[path = "../drc_common.rs"]
mod drc_common;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: p5t2 <dsn> [rules|-] [ses|-] <mode 0-4>");
        std::process::exit(2);
    }
    // The mode is the last argument, so `<dsn> <mode>`, `<dsn> <rules> <mode>` and
    // `<dsn> <rules> <ses> <mode>` are all accepted — `P5T2.main`'s rule.
    let mode: u8 = args[args.len() - 1].parse().expect("mode must be 0-4");
    let slots = &args[..args.len() - 1];
    let inputs = drc_common::Inputs::from_args(slots);

    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    drc_common::print_rust_provenance();
    drc_common::print_header(&mut out, &inputs, &format!(" mode={mode}"));

    if mode == 4 {
        // The transcription check (`P5T2.transcriptionSelfCheck`). This side states the expected
        // outcome and the Java side computes it, exactly as `p4t1`'s twin states
        // `JSON_SOURCE_EMPTY`: the harness's diff is the assertion. No board is read, so the mode
        // costs one process start.
        writeln!(out, "TRANSCRIPTION equal 0").expect("write");
        return;
    }

    let (mut board, _transform) = drc_common::load_board(&inputs);
    match mode {
        0 => clearance_violations(&mut out, &mut board),
        1 => unconnected_items(&mut out, &mut board),
        // Modes 2 and 3 are the same computation on this side: the port has only one seed order,
        // plan-5 ruling 3's ascending item id. They differ on the Java side — mode 2 is the jar's
        // own hash-seeded `getAllAirlines()`, mode 3 is the same algorithm reseeded the port's way
        // — which is what makes mode 3 the ratsnest's parity surface and mode 2 the measurement of
        // how far the two seed orders drift apart. See `P5T2.java`. Only the printing differs:
        // mode 3 additionally emits the `ALD` block, because with the seed pinned on both sides the
        // airlines' direction and acceptance order are facts about the algorithm rather than hash
        // noise, so mode 3 gates on them. Mode 2 keeps only the canonical `AL` block.
        2 | 3 => ratsnest(&mut out, &mut board, mode == 3),
        _ => {
            eprintln!("mode must be 0, 1, 2, 3 or 4");
            std::process::exit(2);
        }
    }
}

/// Mode 0.
fn clearance_violations<W: Write>(out: &mut W, board: &mut Board) {
    let violations = DesignRulesChecker::new(board).get_all_clearance_violations();
    for violation in &violations {
        writeln!(
            out,
            "V {} {} {} {} {}",
            violation.first_item.0,
            violation.second_item.0,
            violation.layer,
            java_double_to_string(violation.expected_clearance),
            java_double_to_string(violation.actual_clearance),
        )
        .expect("write");
    }
    writeln!(out, "VCOUNT {}", violations.len()).expect("write");
}

/// Mode 1.
fn unconnected_items<W: Write>(out: &mut W, board: &mut Board) {
    let entries = DesignRulesChecker::new(board).get_all_unconnected_items();
    let (mut nets, mut track_dangling, mut via_dangling) = (0usize, 0usize, 0usize);
    let mut lines = Vec::with_capacity(entries.len());
    for entry in &entries {
        match entry.kind {
            UnconnectedKind::UnconnectedItems => nets += 1,
            UnconnectedKind::TrackDangling => track_dangling += 1,
            UnconnectedKind::ViaDangling => via_dangling += 1,
        }
        let mut ids: Vec<i64> = entry.all_items.iter().map(|id| id.0 as i64).collect();
        ids.sort_unstable();
        let joined: Vec<String> = ids.iter().map(i64::to_string).collect();
        lines.push(format!(
            "U {} {} {} {}",
            kind_string(entry.kind),
            kind_class(board, entry.first_item),
            entry.second_item.map_or("-", |id| kind_class(board, id)),
            joined.join(","),
        ));
    }
    for line in lines {
        writeln!(out, "{line}").expect("write");
    }
    writeln!(out, "UCOUNT {}", entries.len()).expect("write");
    writeln!(out, "UTYPE unconnectedItems {nets}").expect("write");
    writeln!(out, "UTYPE track_dangling {track_dangling}").expect("write");
    writeln!(out, "UTYPE via_dangling {via_dangling}").expect("write");
}

/// Modes 2 and 3. `directional` appends the `ALD` block (mode 3 only) — see `P5T2.java`'s
/// `directionalAirlineLines`.
fn ratsnest<W: Write>(out: &mut W, board: &mut Board, directional: bool) {
    let max_net_no = board.rules.nets.max_net_number();
    let mut drc = DesignRulesChecker::new(board);
    drc.calculate_all_incompletes();
    writeln!(out, "MAXCONN {}", drc.max_connections()).expect("write");
    writeln!(out, "INCOMPLETE {}", drc.get_incomplete_count()).expect("write");

    for net_number in 1..=max_net_no {
        let net_incompletes = drc
            .get_net_incompletes(net_number)
            .expect("net numbers 1..=maxNetNumber are all in range");
        writeln!(
            out,
            "NET {net_number} {} {} {}",
            net_incompletes.count(),
            net_incompletes.get_connected_group_count(),
            java_double_to_string(net_incompletes.get_length_violation()),
        )
        .expect("write");
    }

    let airlines = drc.get_all_airlines();
    writeln!(out, "ALCOUNT {}", airlines.len()).expect("write");
    // The unordered pair, sorted by `(net, low, high)` — `P5T2.ratsnest`'s convention and the
    // committed `*.airlines-union.txt` files'.
    let mut triples: Vec<(i32, u64, u64)> = airlines
        .iter()
        .map(|airline| {
            let (from, to) = (airline.from_item.0 as u64, airline.to_item.0 as u64);
            (airline.net_number, from.min(to), from.max(to))
        })
        .collect();
    triples.sort_unstable();
    for (net, low, high) in triples {
        writeln!(out, "AL {net} {low} {high}").expect("write");
    }
    if directional {
        // `get_all_airlines` walks the per-net lists in net order and each net's `incompletes` in
        // Kruskal's acceptance order (`checker.rs:556-562`), which is the order this block
        // preserves — unsorted, and with the edge's own `from`/`to`.
        for airline in &airlines {
            writeln!(
                out,
                "ALD {} {} {}",
                airline.net_number, airline.from_item.0, airline.to_item.0,
            )
            .expect("write");
        }
    }
}

/// `UnconnectedItems.type`'s three literals (UnconnectedItems.java:31, `:36`,
/// DesignRulesChecker.java:161, `:172`).
fn kind_string(kind: UnconnectedKind) -> &'static str {
    match kind {
        UnconnectedKind::UnconnectedItems => "unconnectedItems",
        UnconnectedKind::TrackDangling => "track_dangling",
        UnconnectedKind::ViaDangling => "via_dangling",
    }
}

/// `P5T2.kindClass`: `findRepresentativeItem`'s three-way preference
/// (DesignRulesChecker.java:186-200), which is the part of the representative that survives the
/// `HashSet` order.
fn kind_class(board: &Board, id: ItemId) -> &'static str {
    match board.get_item(id).map(Item::kind) {
        Some(ItemKind::Pin) => "Pin",
        Some(ItemKind::Trace) => "Trace",
        _ => "other",
    }
}
