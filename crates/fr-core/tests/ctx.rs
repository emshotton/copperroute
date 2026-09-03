//! Plan 9 Task 1: what a plain run's wall-clock budget is, and what it deliberately is not.
//!
//! [`fr_core::Ctx`] is the thing a routing run is handed, and its `budget` field is ruling AI's
//! knob. Until Plan 9 Task 1 it carried Java's four literals verbatim, `optChangedArea`'s 1000 ms
//! included. That one literal is quirk **#234**: it abandons the pull-tight part-way through on
//! wall clock, so the routed board depends on how fast the machine is — measured on
//! `Issue508-DAC2020_bm01.dsn`, the jar scores 841.0115 at `-mp 3` and 835.88336 at `-mp 20`
//! (three runs each), and the same `-mp 20` run with the jar's DEBUG log on, which slows it down,
//! is back to 841.0115.
//!
//! The jar cannot switch it off. All four declarations are `static final int … = 1000` with a
//! constant initialiser, so `javac` inlines them — `javap -c -p` on the shipping jar shows
//! `sipush 1000` before every `optChangedArea` call and no `getstatic` — which makes the fields
//! dead and reflection useless. **The port can**, and from Task 1 on it does: `0` is Java's own
//! "off" value (`board/optimize/TraceTightener.java:73-77` builds a `TimeLimit` only
//! `if (timeLimit > 0)`), so this is a configuration Java already understands rather than a
//! port-only branch.

use fr_core::RouterBudget;

/// The directed test the task brief names.
///
/// A completed pull-tight cannot lengthen a trace, so this is a strictly-more-work,
/// strictly-better-output change; what it buys is that two runs of the same board on the same
/// binary answer the same bytes whatever else the machine is doing.
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

/// The fix is **one** field. The other three keep Java's literals, and a reader has to be able to
/// see that this is a targeted change and not a general retreat from parity.
///
/// * `fanout_ms_per_pin` is a per-pin budget with a real purpose on a huge board — the corpus's
///   widest outline is rejected as a reference *because* of it — and turning it off would change
///   what a user gets rather than only what a measurement sees.
/// * the two throttles are progress only (ruling 11): they decide whether an event fires, never
///   what the board becomes.
#[test]
fn the_other_three_budget_fields_still_carry_javas_literals() {
    let default = RouterBudget::default();
    let java = RouterBudget::java_literals();

    assert_eq!(default.fanout_ms_per_pin, java.fanout_ms_per_pin);
    assert_eq!(
        default.board_update_throttle_ms,
        java.board_update_throttle_ms
    );
    assert_eq!(default.progress_throttle_ms, java.progress_throttle_ms);

    assert_eq!(
        java.opt_changed_area_ms, 1000,
        "the Java fact survives the fix: java_literals() is still what the jar does, and it is \
         what `--router.opt_changed_area_ms=1000` reproduces"
    );
    assert_ne!(
        default, java,
        "#234 is a real change and this pair is the record of exactly how much of one"
    );
}

/// `Ctx::new` is what a plain `-de/-do` run builds, and it must carry the fixed budget rather
/// than a second copy of Java's literals that the fix forgot about.
#[test]
fn a_plain_context_carries_the_fixed_budget() {
    let settings = fr_settings::RouterSettings::new();
    let progress = fr_core::SyncProgressSink::noop();

    let ctx = fr_core::Ctx::new(&settings, &progress);
    assert_eq!(ctx.budget, RouterBudget::default());
    assert_eq!(ctx.budget.opt_changed_area_ms, 0);

    // Ruling AI's parity configuration is unchanged by #234 and is still stronger than the
    // default: it turns the fanout clock off too, which the default deliberately leaves alone.
    let disabled = fr_core::Ctx::with_disabled_budget(&settings, &progress);
    assert_eq!(disabled.budget, RouterBudget::disabled());
    assert_ne!(
        disabled.budget,
        RouterBudget::default(),
        "`disabled` is not merely `default` — `fanout_ms_per_pin` still separates them, which is \
         why scripts/quality-ab.sh's FR_ROUTER_BUDGET=disabled lane is not redundant after #234"
    );
}
