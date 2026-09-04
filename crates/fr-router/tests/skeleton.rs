use std::any::TypeId;

use fr_router::{Arena, AutorouteAttemptResult, AutorouteAttemptState, ExpansionCostFactor};

// ---------------------------------------------------------------------------------------------
// ---------------------------------------------------------------------------------------------

#[test]
fn arena_hands_out_dense_indices_and_reads_them_back() {
    let mut arena: Arena<&str> = Arena::new();
    assert!(arena.is_empty());
    assert_eq!(arena.len(), 0);

    let a = arena.insert("a");
    let b = arena.insert("b");
    let c = arena.insert("c");
    assert_eq!((a, b, c), (0, 1, 2));
    assert_eq!(arena.len(), 3);
    assert_eq!(arena.get(b), Some(&"b"));
    assert_eq!(arena.get(99), None);

    *arena.get_mut(b).unwrap() = "B";
    assert_eq!(arena.get(b), Some(&"B"));
}

#[test]
fn arena_remove_leaves_a_hole_and_the_index_is_never_reused() {
    let mut arena: Arena<u32> = Arena::new();
    let a = arena.insert(10);
    let b = arena.insert(20);
    assert_eq!(arena.remove(b), Some(20));
    assert_eq!(arena.remove(b), None, "removing twice is a no-op");
    assert_eq!(arena.get(b), None);
    assert_eq!(arena.len(), 1);

    let c = arena.insert(30);
    assert_eq!(c, 2, "the hole at index 1 is not refilled");
    assert_eq!(arena.get(b), None);
    assert_eq!(arena.get(a), Some(&10));
    assert_eq!(arena.get(c), Some(&30));
    assert_eq!(arena.len(), 2);
}

#[test]
fn arena_iter_skips_holes_and_yields_ascending_indices() {
    let mut arena: Arena<u32> = Arena::new();
    for v in [0_u32, 1, 2, 3] {
        arena.insert(v * 10);
    }
    arena.remove(1);
    arena.remove(3);
    let seen: Vec<(u32, u32)> = arena.iter().map(|(i, v)| (i, *v)).collect();
    assert_eq!(seen, vec![(0, 0), (2, 20)]);
}

#[test]
fn arena_default_is_empty() {
    let arena: Arena<String> = Arena::default();
    assert_eq!(arena.len(), 0);
    assert!(arena.is_empty());
}


#[test]
fn attempt_state_is_javas_declaration_order() {
    let all = AutorouteAttemptState::ALL;
    assert_eq!(
        all,
        [
            AutorouteAttemptState::Unknown,
            AutorouteAttemptState::Skipped,
            AutorouteAttemptState::NoUnconnectedNets,
            AutorouteAttemptState::ConnectedToPlane,
            AutorouteAttemptState::AlreadyConnected,
            AutorouteAttemptState::NoConnections,
            AutorouteAttemptState::Routed,
            AutorouteAttemptState::Failed,
            AutorouteAttemptState::InsertError,
        ]
    );
    let names: Vec<String> = all.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        names,
        vec![
            "UNKNOWN",
            "SKIPPED",
            "NO_UNCONNECTED_NETS",
            "CONNECTED_TO_PLANE",
            "ALREADY_CONNECTED",
            "NO_CONNECTIONS",
            "ROUTED",
            "FAILED",
            "INSERT_ERROR",
        ]
    );
}

#[test]
fn attempt_result_mirrors_javas_two_constructors() {
    let bare = AutorouteAttemptResult::new(AutorouteAttemptState::Routed);
    assert_eq!(bare.state, AutorouteAttemptState::Routed);
    assert_eq!(bare.details, None);
    assert_eq!(bare.details(), "");

    let detailed = AutorouteAttemptResult::with_details(
        AutorouteAttemptState::Failed,
        "no route found".to_string(),
    );
    assert_eq!(detailed.state, AutorouteAttemptState::Failed);
    assert_eq!(detailed.details.as_deref(), Some("no route found"));
    assert_eq!(detailed.details(), "no route found");
}

#[test]
fn attempt_result_equality_is_state_plus_details() {
    let a = AutorouteAttemptResult::new(AutorouteAttemptState::Routed);
    let b = AutorouteAttemptResult::new(AutorouteAttemptState::Routed);
    let c = AutorouteAttemptResult::new(AutorouteAttemptState::Failed);
    let d = AutorouteAttemptResult::with_details(AutorouteAttemptState::Routed, "x".to_string());
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_ne!(a, d);
}

#[test]
fn attempt_result_to_string_is_javas_to_string() {
    assert_eq!(
        AutorouteAttemptResult::new(AutorouteAttemptState::Routed).to_string(),
        "ROUTED: "
    );
    assert_eq!(
        AutorouteAttemptResult::with_details(
            AutorouteAttemptState::Failed,
            "1 unrouted".to_string()
        )
        .to_string(),
        "FAILED: 1 unrouted"
    );
}


#[test]
fn expansion_cost_factor_is_the_one_from_fr_settings() {
    assert_eq!(
        TypeId::of::<ExpansionCostFactor>(),
        TypeId::of::<fr_settings::ExpansionCostFactor>()
    );
    let from_settings = fr_settings::ExpansionCostFactor {
        horizontal: 1.5,
        vertical: 2.5,
    };
    let through_router: ExpansionCostFactor = from_settings;
    assert_eq!(through_router.horizontal, 1.5);
    assert_eq!(through_router.vertical, 2.5);
}
