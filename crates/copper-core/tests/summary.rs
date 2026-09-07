use copper_core::{RoutingJob, SessionId, load_board_if_needed, summarise};

fn load(relative: &str) -> copper_board::Board {
    let path = parity::reference_dir().join(relative);
    let mut job = RoutingJob::new(SessionId::NIL);
    job.set_input(&path)
        .unwrap_or_else(|e| panic!("cannot set {} as input: {e}", path.display()));
    load_board_if_needed(&mut job)
        .unwrap_or_else(|e| panic!("cannot load {}: {e}", path.display()))
        .board
}

#[test]
fn the_summary_names_every_layer_net_and_component() {
    if !parity::require_reference_dir() {
        return;
    }
    let mut board = load("fixtures/Issue143-rpi_splitter.dsn");
    let summary = summarise(&mut board, None);

    assert_eq!(
        summary
            .layers
            .iter()
            .map(|l| (l.index, l.name.as_str(), l.signal))
            .collect::<Vec<_>>(),
        vec![(0, "1#Top", true), (1, "16#Bottom", true)],
    );

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

    assert!(!summary.components.is_empty());
    for (i, component) in summary.components.iter().enumerate() {
        assert_eq!(component.id, i as i32 + 1);
    }
}

#[test]
fn the_summary_counts_agree_with_the_statistics() {
    if !parity::require_reference_dir() {
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

#[test]
fn the_metadata_comes_from_the_boards_own_communication() {
    if !parity::require_reference_dir() {
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

#[test]
fn the_top_level_key_order_is_declaration_order() {
    if !parity::require_reference_dir() {
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
