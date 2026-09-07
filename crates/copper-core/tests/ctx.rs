//! (three runs each), and the same `-mp 20` run with the jar's DEBUG log on, which slows it down,
use copper_core::RouterBudget;

#[test]
fn the_default_router_budget_disables_the_opt_changed_area_clock() {
    let default = RouterBudget::default();
    assert_eq!(
        default.opt_changed_area_ms, 0,
        "#234: the port's default must be Java's own `off` value, not its inlined 1000 ms — \
         TraceTightener.java:73-77 builds a TimeLimit only `if (timeLimit > 0)`"
    );
    assert!(
        default.opt_changed_area_limit().is_none(),
        "0 takes TraceTightener's `else this.timeLimit = null` branch, so the pull-tight runs to \
         completion and no wall clock can abandon it"
    );
}

#[test]
fn the_other_three_budget_fields_still_carry_javas_literals() {
    let default = RouterBudget::default();
    let java = RouterBudget::from_fixed_budget();

    assert_eq!(default.fanout_ms_per_pin, java.fanout_ms_per_pin);
    assert_eq!(
        default.board_update_throttle_ms,
        java.board_update_throttle_ms
    );
    assert_eq!(default.progress_throttle_ms, java.progress_throttle_ms);

    assert_eq!(
        java.opt_changed_area_ms, 1000,
        "the Java fact survives the fix: from_fixed_budget() is still what the jar does, and it is \
         what `--router.opt_changed_area_ms=1000` reproduces"
    );
    assert_ne!(
        default, java,
        "#234 is a real change and this pair is the record of exactly how much of one"
    );
}

#[test]
fn a_plain_context_carries_the_fixed_budget() {
    let settings = copper_settings::RouterSettings::new();
    let progress = copper_core::SyncProgressSink::noop();

    let ctx = copper_core::Ctx::new(&settings, &progress);
    assert_eq!(ctx.budget, RouterBudget::default());
    assert_eq!(ctx.budget.opt_changed_area_ms, 0);

    let disabled = copper_core::Ctx::with_disabled_budget(&settings, &progress);
    assert_eq!(disabled.budget, RouterBudget::disabled());
    assert_ne!(
        disabled.budget,
        RouterBudget::default(),
        "`disabled` is not merely `default` — `fanout_ms_per_pin` still separates them, which is \
         why scripts/quality-ab.sh's quality lane also pins router.fanout.max_milliseconds_per_pin"
    );
}
