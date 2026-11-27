use copper_kicad::{RouteRequest, route};

const BOARD: &str = include_str!("data/two-nets.kicad_pcb");

fn request() -> RouteRequest {
    RouteRequest {
        boardText: BOARD.to_string(),
        maxPasses: 5,
        timeoutSeconds: 60,
        ..RouteRequest::default()
    }
}

#[test]
fn routing_keeps_existing_copper_and_adds_only_new_tracks() {
    let mut calls = 0;
    let result = route(&request(), &mut |_| {
        calls += 1;
        false
    })
    .expect("the board routes");
    assert_eq!(result.incomplete, 0, "{result:?}");
    assert!(result.changes.removeIds.is_empty());
    assert!(!result.changes.addTracks.is_empty());
    let duplicates_locked = result.changes.addTracks.iter().any(|t| {
        t.layer == "F.Cu"
            && t.net == "A"
            && t.start.y == 105.0
            && t.end.y == 105.0
            && t.start.x.min(t.end.x) >= 105.0
            && t.start.x.max(t.end.x) <= 110.0
    });
    assert!(!duplicates_locked, "the locked track is not written again");
    assert!(calls > 0, "progress is reported");
}

#[test]
fn rip_up_removes_unlocked_tracks_by_their_kicad_id() {
    let result = route(
        &RouteRequest {
            ripUp: true,
            ..request()
        },
        &mut |_| false,
    )
    .expect("the board routes");
    assert_eq!(result.changes.removeIds, vec!["loose-b".to_string()]);
    assert_eq!(result.incomplete, 0);
}

#[test]
fn only_the_selected_nets_are_routed() {
    let result = route(
        &RouteRequest {
            onlyNets: Some(vec!["B".to_string()]),
            ..request()
        },
        &mut |_| false,
    )
    .expect("the board routes");
    assert!(result.changes.addTracks.iter().all(|t| t.net == "B"));
}

#[test]
fn a_cancel_from_progress_stops_the_run() {
    let result = route(&request(), &mut |_| true).expect("the run returns");
    assert!(result.cancelled);
}
