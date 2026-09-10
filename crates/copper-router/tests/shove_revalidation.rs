use copper_board::Board;
use copper_drc::{DesignRulesChecker, DrcViolationKind};
use copper_geometry::{IntPoint, Point, Polyline};
use copper_router::RoutingBoardExt;

fn shorts(board: &mut Board, a: i32, b: i32) -> usize {
    let violations = DesignRulesChecker::new(board).get_all_violations();
    violations
        .iter()
        .filter(|v| {
            if v.kind != DrcViolationKind::ShortingItems {
                return false;
            }
            let Some(second) = v.second_item else {
                return false;
            };
            let first = board.get_item(v.first_item).unwrap();
            let second = board.get_item(second).unwrap();
            (first.contains_net(a) && second.contains_net(b))
                || (first.contains_net(b) && second.contains_net(a))
        })
        .count()
}

#[test]
fn captured_forced_polyline_must_not_short_another_net() {
    if std::env::var_os("COPPERROUTE_RECHECK_SHOVED_PATH").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "captured_forced_polyline_must_not_short_another_net",
                "--nocapture",
            ])
            .env("COPPERROUTE_RECHECK_SHOVED_PATH", "1")
            .status()
            .unwrap();
        assert!(status.success(), "the enabled guard regression must pass");
        return;
    }
    verify_blocked_path();
}

#[test]
fn clear_multisegment_prefix_remains_routable() {
    if std::env::var_os("COPPERROUTE_RECHECK_SHOVED_PATH").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "clear_multisegment_prefix_remains_routable",
                "--nocapture",
            ])
            .env("COPPERROUTE_RECHECK_SHOVED_PATH", "1")
            .status()
            .unwrap();
        assert!(status.success(), "the enabled guard regression must pass");
        return;
    }
    let input = &include_bytes!("fixtures/shove-later-segment.dsn")[..];
    let copper_dsn::BoardReadResult::Success {
        board: Some(mut board),
        warnings,
        ..
    } = copper_dsn::read_board(input, None, None, &Default::default())
    else {
        panic!("capture must import");
    };
    assert!(warnings.is_empty());
    let a = board.rules.nets.get_by_name("AGND")[0].net_number;
    let b = board.rules.nets.get_by_name("Net-(C30-Pad1)")[0].net_number;
    assert_eq!(
        shorts(&mut board, a, b),
        0,
        "capture begins without this short"
    );
    let corners: Vec<_> = [(779636, -702181), (780874, -703419), (788000, -703419)]
        .into_iter()
        .map(|(x, y)| Point::Int(IntPoint::new(x, y)))
        .collect();
    let path = Polyline::from_points(&corners);
    let result = board
        .insert_forced_trace_polyline(
            None,
            &path,
            1270,
            0,
            &[b],
            1,
            20,
            5,
            5,
            i32::MAX,
            500,
            true,
            None,
            &|| false,
        )
        .unwrap();
    assert_eq!(
        result,
        corners.last().cloned(),
        "the clear prefix remains available"
    );
    assert_eq!(
        shorts(&mut board, a, b),
        0,
        "a successful insertion must not short AGND and the signal"
    );
}

fn verify_blocked_path() {
    let input = &include_bytes!("fixtures/shove-later-segment.dsn")[..];
    let copper_dsn::BoardReadResult::Success {
        board: Some(mut board),
        warnings,
        ..
    } = copper_dsn::read_board(input, None, None, &Default::default())
    else {
        panic!("capture must import");
    };
    assert!(warnings.is_empty());
    let a = board.rules.nets.get_by_name("AGND")[0].net_number;
    let b = board.rules.nets.get_by_name("Net-(C30-Pad1)")[0].net_number;
    assert_eq!(
        shorts(&mut board, a, b),
        0,
        "capture begins without this short"
    );
    let corners: Vec<_> = [
        (779636, -702181),
        (780874, -703419),
        (797814, -703419),
        (800100, -703419),
        (801573, -701946),
        (804926, -701946),
    ]
    .into_iter()
    .map(|(x, y)| Point::Int(IntPoint::new(x, y)))
    .collect();
    let path = Polyline::from_points(&corners);
    let result = board
        .insert_forced_trace_polyline(
            None,
            &path,
            1270,
            0,
            &[b],
            1,
            20,
            5,
            5,
            i32::MAX,
            500,
            true,
            None,
            &|| false,
        )
        .unwrap();
    assert_eq!(
        result,
        Some(corners[0].clone()),
        "the newly blocked path is rejected"
    );
    assert_eq!(
        shorts(&mut board, a, b),
        0,
        "a successful insertion must not short AGND and the signal"
    );
}

#[test]
fn corridor_guidance_also_revalidates_paths() {
    if std::env::var_os("COPPERROUTE_CORRIDOR_GUIDANCE").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "corridor_guidance_also_revalidates_paths",
                "--nocapture",
            ])
            .env_remove("COPPERROUTE_RECHECK_SHOVED_PATH")
            .env("COPPERROUTE_CORRIDOR_GUIDANCE", "1")
            .status()
            .unwrap();
        assert!(
            status.success(),
            "guidance must enable its path safety check"
        );
        return;
    }
    verify_blocked_path();
}

#[test]
fn default_corridor_guidance_revalidates_paths() {
    if std::env::var_os("COPPERROUTE_TEST_DEFAULT_GUARD").is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "default_corridor_guidance_revalidates_paths",
                "--nocapture",
            ])
            .env("COPPERROUTE_TEST_DEFAULT_GUARD", "1")
            .env_remove("COPPERROUTE_CORRIDOR_GUIDANCE")
            .env_remove("COPPERROUTE_RECHECK_SHOVED_PATH")
            .status()
            .unwrap();
        assert!(status.success());
        return;
    }
    verify_blocked_path();
}
