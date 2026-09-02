//! Plan 8 Task 12: [`fr_core::summarise`], the one document `freerouting info` (spec §12) and the
//! `board_info` MCP tool (spec §13) both answer.
//!
//! Java has no counterpart to compare against — there is no `info` mode and no `board_info` tool
//! — so these are the port's own pins. The one thing that *is* jar-derived is `statistics`, and
//! the test that matters most below is the one asserting that the summary's three vectors and
//! that document count the same board.

use fr_core::{RoutingJob, SessionId, load_board_if_needed, summarise};

/// The load the two callers perform: `BoardLoader.loadBoardIfNeeded` whole, exactly as
/// `commands::drc` does — see that runner's step 6 for why the parse half is not enough.
fn load(relative: &str) -> fr_board::Board {
    let path = parity::java_dir().join(relative);
    let mut job = RoutingJob::new(SessionId::NIL);
    job.set_input(&path)
        .unwrap_or_else(|e| panic!("cannot set {} as input: {e}", path.display()));
    load_board_if_needed(&mut job)
        .unwrap_or_else(|e| panic!("cannot load {}: {e}", path.display()))
        .board
}

#[test]
fn the_summary_names_every_layer_net_and_component() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load("fixtures/Issue143-rpi_splitter.dsn");
    let summary = summarise(&mut board, None);

    // The layer stack, in stack order, with the index every `layers[i]` setting is keyed by.
    assert_eq!(
        summary
            .layers
            .iter()
            .map(|l| (l.index, l.name.as_str(), l.signal))
            .collect::<Vec<_>>(),
        vec![(0, "1#Top", true), (1, "16#Bottom", true)],
    );

    // Net numbers are 1-based and consecutive (`rules/Nets.java`'s invariant), and every net
    // names a real class.
    assert!(!summary.nets.is_empty());
    for (i, net) in summary.nets.iter().enumerate() {
        assert_eq!(net.number, i as i32 + 1, "net numbers are consecutive");
        assert!(!net.class.is_empty(), "every net resolves a class name");
    }
    assert_eq!(
        summary
            .nets
            .iter()
            .map(|n| n.name.as_str())
            .collect::<Vec<_>>(),
        vec!["D+", "D-", "N$5", "VBUS", "VCC"],
        "the splitter board's five nets, in net-number order"
    );

    // Component ids are `Components.add`'s `count + 1`, so they are 1-based and consecutive too.
    assert!(!summary.components.is_empty());
    for (i, component) in summary.components.iter().enumerate() {
        assert_eq!(component.id, i as i32 + 1);
    }
}

/// **The anti-drift pin.** Every count in the answer comes from `BoardStatistics` and the three
/// vectors carry only names; this asserts the two halves are looking at the same board, so a
/// future edit that starts counting things itself fails here.
#[test]
fn the_summary_counts_agree_with_the_statistics() {
    if !parity::require_java_dir() {
        return;
    }
    for stem in [
        "fixtures/Issue143-rpi_splitter.dsn",
        "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn",
    ] {
        let mut board = load(stem);
        let summary = summarise(&mut board, None);
        let stats = &summary.statistics;
        assert_eq!(
            stats["layers"]["total_count"].as_u64(),
            Some(summary.layers.len() as u64),
            "{stem}: layers"
        );
        assert_eq!(
            stats["nets"]["total_count"].as_u64(),
            Some(summary.nets.len() as u64),
            "{stem}: nets"
        );
        assert_eq!(
            stats["components"]["total_count"].as_u64(),
            Some(summary.components.len() as u64),
            "{stem}: components"
        );
        assert_eq!(
            stats["layers"]["signal_count"].as_u64(),
            Some(summary.layers.iter().filter(|l| l.signal).count() as u64),
            "{stem}: signal layers"
        );
    }
}

/// The metadata is read off `board.communication` — where the DSN reader actually put it — and a
/// `None` [`fr_dsn::BoardMetadata`] is the ordinary case, not a degraded one (see `summarise`'s
/// own doc, and `LoadedBoard::metadata`).
#[test]
fn the_metadata_comes_from_the_boards_own_communication() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load("examples/tutorial_board/tutorial_board.dsn");
    let summary = summarise(&mut board, None);
    assert_eq!(summary.metadata.host_cad.as_deref(), Some("KiCad's Pcbnew"));
    assert_eq!(summary.metadata.host_version.as_deref(), Some("8.0.0"));
    assert_eq!(summary.metadata.unit, "um");
    assert_eq!(summary.metadata.snap_angle, "fortyfive_degree");
    assert!(summary.metadata.resolution > 0);
}

/// Convention 8: the top-level keys come out in declaration order, which is what makes the
/// `info` document diffable. (Inside `statistics` they are alphabetised — `to_gson_json`'s
/// documented loss, recorded in this module's docs.)
#[test]
fn the_top_level_key_order_is_declaration_order() {
    if !parity::require_java_dir() {
        return;
    }
    let mut board = load("fixtures/Issue143-rpi_splitter.dsn");
    let text = summarise(&mut board, None).to_json_pretty();
    let keys: Vec<&str> = ["layers", "nets", "components", "statistics", "metadata"]
        .into_iter()
        .filter(|key| text.contains(&format!("\"{key}\"")))
        .collect();
    assert_eq!(
        keys,
        vec!["layers", "nets", "components", "statistics", "metadata"]
    );
    let positions: Vec<usize> = keys
        .iter()
        .map(|key| text.find(&format!("\n  \"{key}\"")).unwrap_or(usize::MAX))
        .collect();
    assert!(
        positions.windows(2).all(|w| w[0] < w[1]),
        "top-level keys are out of declaration order: {positions:?}"
    );
}
