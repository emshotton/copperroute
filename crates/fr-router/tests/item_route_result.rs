use std::cmp::Ordering;

use fr_board::ItemId;
use fr_router::pipeline::ItemRouteResult;

#[test]
fn the_unimproved_constructor_reports_one_new_incomplete() {
    let r = ItemRouteResult::unimproved(ItemId(7));
    assert_eq!(r.item_id(), ItemId(7));
    assert!(!r.improved());
    assert_eq!(r.improvement_percentage(), 0.0);
    assert_eq!(r.via_count(), 0);
    assert_eq!(r.trace_length(), 0.0);
    assert_eq!(r.incomplete_count_before(), 0);
    assert_eq!(r.incomplete_count(), 1);
    assert_eq!(r.via_count_reduced(), 0);
    assert_eq!(r.length_reduced(), 0.0);
}

#[test]
fn update_improved_overrides_the_constructor() {
    let mut r = ItemRouteResult::unimproved(ItemId(1));
    assert!(!r.improved());
    r.update_improved(true);
    assert!(r.improved());
    r.update_improved(false);
    assert!(!r.improved());
}

#[test]
fn improved_follows_the_penalty_alone() {
    let cheaper = ItemRouteResult::new(ItemId(1), 3, 3, 10.0, 9.5, 2, 2, 160.0, 159.5);
    assert!(cheaper.improved());

    let dearer = ItemRouteResult::new(ItemId(1), 3, 3, 9.5, 10.0, 2, 2, 159.5, 160.0);
    assert!(!dearer.improved());

    let tied = ItemRouteResult::new(ItemId(1), 3, 3, 10.0, 10.0, 2, 2, 160.0, 160.0);
    assert!(!tied.improved());

    let fewer_vias_but_dearer =
        ItemRouteResult::new(ItemId(1), 4, 2, 1.0, 200.0, 2, 2, 201.0, 300.0);
    assert!(!fewer_vias_but_dearer.improved());
}

#[test]
fn the_reductions_are_before_minus_after() {
    let r = ItemRouteResult::new(ItemId(1), 4, 2, 100.0, 50.0, 1, 1, 300.0, 150.0);
    assert_eq!(r.via_count_reduced(), 2);
    assert_eq!(r.length_reduced(), 50.0);
    assert_eq!(r.via_count(), 2);
    assert_eq!(r.trace_length(), 50.0);
    assert_eq!(r.incomplete_count(), 1);
    assert_eq!(r.incomplete_count_before(), 1);
}

#[test]
fn the_improvement_percentage_keeps_javas_integer_via_term() {
    let r = ItemRouteResult::new(ItemId(1), 4, 2, 100.0, 50.0, 1, 1, 300.0, 150.0);
    assert!(
        (r.improvement_percentage() - 0.75).abs() < 1e-6,
        "2 / 4 truncates to 0, so (1 - (0 + 0.5) / 2) is 0.75"
    );

    let no_vias = ItemRouteResult::new(ItemId(1), 0, 3, 100.0, 50.0, 1, 1, 100.0, 200.0);
    assert_eq!(no_vias.improvement_percentage(), 0.0);
    let no_length = ItemRouteResult::new(ItemId(1), 4, 2, 0.0, 50.0, 1, 1, 200.0, 150.0);
    assert_eq!(no_length.improvement_percentage(), 0.0);
}

#[test]
fn compare_to_orders_by_incompletes_then_vias_then_length() {
    let a = ItemRouteResult::new(ItemId(1), 0, 2, 0.0, 10.0, 0, 0, 0.0, 110.0);
    let b = ItemRouteResult::new(ItemId(2), 0, 2, 0.0, 20.0, 0, 0, 0.0, 120.0);
    let c = ItemRouteResult::new(ItemId(3), 0, 1, 0.0, 30.0, 0, 0, 0.0, 80.0);
    let d = ItemRouteResult::new(ItemId(4), 0, 9, 0.0, 0.0, 0, 1, 0.0, 5_000_450.0);

    assert_eq!(a.compare_to(&b), Ordering::Less);
    assert_eq!(b.compare_to(&a), Ordering::Greater);
    assert_eq!(c.compare_to(&a), Ordering::Less);
    assert_eq!(a.compare_to(&d), Ordering::Less);
    assert_eq!(a.compare_to(&a), Ordering::Equal);
    assert!(c.improved_over(&a));
    assert!(!a.improved_over(&c));
}
