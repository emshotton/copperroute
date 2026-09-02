//! Port of `autoroute.pipeline.BatchFanout` (BatchFanout.java:20-780) — the SMD escape pre-pass
//! that runs before the first autoroute pass.
//!
//! **Task 11 of Plan 7 landed the ordering**: the [`BatchFanout`] type and its constructor
//! (`:35-78`), the [`FanoutComponent`] / [`FanoutPin`] pair the constructor builds (`:631-693`,
//! `:695-778`) and the three result records (`:591-606`, `:609-622`, `:625-629`). Scan ruling 7:
//! the struct is declared by the earliest task that writes methods on it.
//!
//! **Task 12 added the `impl` blocks** — [`BatchFanout::fanout_board`] (`:81-163`),
//! `fanout_pass` (`:166-506`) and the two progress publishers (`:508-576`) — and, with them, the
//! three arms of the loop no corpus board reaches: [`FanoutLoopState`] (the oscillation detector
//! and the hash stop, `:125-156`), [`fanout_ripup_costs`] (`:173`, `:179-183`) and
//! [`fanout_pin_can_use_vias`] (`:238-259`). Those three are lifted out for the reason Task 11
//! lifted `sorted_unconnected_targets` out: they are pure functions of data a test can build, and
//! each has a case the eight `p7t5` stems cannot produce.
//!
//! The per-pin router those loops call is not here either: it is
//! [`RoutingBoardExt::fanout`](crate::board_ext::RoutingBoardExt::fanout), the port of
//! `RoutingBoard.fanout` (RoutingBoard.java:978-1110), which Task 11 also lands.
//!
//! # The struct does not hold the board
//!
//! Java's `BatchFanout` keeps `private final RoutingBoard routingBoard` (`:23`) and reads it from
//! every method. This port cannot: `fanoutPass` needs `&mut Board` and the same call needs
//! `&self.settings`, which a struct holding both would not allow. So the board is a **parameter**
//! of the constructor and of every Task 12 method, exactly as `AutorouteEngine`'s board is
//! elsewhere in this crate (see `board_ext::RoutingBoardExt`'s "The engine is a value, not a
//! field").
//!
//! not ported: `BatchFanout.thread` (`:22`) — plan-6 ruling 6 makes cancellation a per-call
//! [`StopCheck`](fr_board::datastructures::StopCheck) rather than object state.
//!
//! # The two containers, and ruling 5's recorded decision
//!
//! | container | Java | comparator | decision |
//! |---|---|---|---|
//! | `sortedComponents` (`:25`, filled `:53-61`) | `TreeSet<Component>` | `Component.compareTo` (`:682-693`): pin count **descending**, then `boardComponent.id` **ascending** — both keys `final`, both total | **`BTreeSet<FanoutComponent>`** |
//! | `Component.smdPins` (`:635`, filled `:673-677`) | `TreeSet<Pin>` | `Pin.compareTo` (`:742-777`): a key chosen from `settings.fanout.pinSortingOrder` **at run time**, then `boardPin.pinIndex` | **[`JavaTreeSet<FanoutPin>`](crate::JavaTreeSet)** |
//!
//! The second row is scan ruling 8's, and this task **confirms the container while correcting the
//! reason** — see [`FanoutPin::compare_to`].

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use fr_board::items::Item;
use fr_board::structure::Unit;
use fr_board::{Board, ItemId};
use fr_geometry::FloatPoint;
use fr_settings::RouterSettings;

use crate::JavaTreeSet;
use crate::autoroute::AutorouteAttemptState;
use crate::board_ext::RoutingBoardExt;
use crate::error::RouterError;
use crate::score::{BoardStatistics, BoardStatisticsFanout};

use super::{ProgressSink, ProgressThrottler, RouterBudget, RouterStop, RoutingEvent};

// =================================================================================================
// `EscapeStatistics` / `FanoutPassStatus` / `FanoutRunSummary` — BatchFanout.java:591-629
// =================================================================================================

/// Port of `BatchFanout.EscapeStatistics` (BatchFanout.java:591-606), a `record`: "statistics
/// about how many SMD pins were successfully escaped after a fanout pass. A pin is considered
/// escaped when it has at least one Trace (wire) or Via directly connected to it (with no
/// clearance violations on the trace/via), or a Via that itself has a Trace connected to it (also
/// without clearance violations)."
///
/// **Three components, not the plan sketch's three.** The plan's interface block gives this
/// record `total_smd_pins`, `escaped_count` and `pins_to_escape`; Java's third component is
/// `escapedPercentage`, a `double` that `fromBoardStatistics` computes (`:595-598`). `pinsToEscape`
/// is a field of `BoardStatistics.BoardStatisticsFanout` (BoardStatistics.java:642-643), which is
/// a different type and is already ported as
/// [`BoardStatisticsFanout::pins_to_escape`](crate::score::BoardStatisticsFanout::pins_to_escape).
/// Java wins.
///
/// not ported: `EscapeStatistics.toString` (`:602-605`) — `String.format("%d/%d (%.1f%%)", …)`,
/// whose only readers are `FRLogger` payloads (`BatchFanout.java:439`, `:520`) and the GUI. Java's
/// `%.1f` is `HALF_UP` while Rust's `{:.1}` is round-half-to-even, so porting the formatter would
/// mean porting a rounding mode for a string no decision reads.
// renamed: BatchFanout.EscapeStatistics -> fr_router::pipeline::EscapeStatistics — a Java
// `record`, as above.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EscapeStatistics {
    /// `totalSmdPins` (`:591`).
    pub total_smd_pins: i32,
    /// `escapedCount` (`:591`).
    pub escaped_count: i32,
    /// `escapedPercentage` (`:591`), `escapedCount * 100.0 / totalSmdPins` or `0.0` (`:595-598`).
    pub escaped_percentage: f64,
}

impl EscapeStatistics {
    /// Port of `EscapeStatistics.fromBoardStatistics(BoardStatistics)` (BatchFanout.java:594-600).
    ///
    /// The guard is `> 0` on the pin count, so an empty board answers `0.0` rather than `NaN`.
    #[must_use]
    pub fn from_board_statistics(stats: &BoardStatistics) -> EscapeStatistics {
        EscapeStatistics::from_fanout_statistics(&stats.fanout)
    }

    /// [`Self::from_board_statistics`] against the nested record it actually reads
    /// (`stats.fanout`, BoardStatistics.java:637-647). Not a Java method: Java writes
    /// `stats.fanout.totalSmdPins` inline three times.
    #[must_use]
    pub fn from_fanout_statistics(fanout: &BoardStatisticsFanout) -> EscapeStatistics {
        // :595-598.
        let percentage = if fanout.total_smd_pins > 0 {
            f64::from(fanout.escaped_count) * 100.0 / f64::from(fanout.total_smd_pins)
        } else {
            0.0
        };
        // :599.
        EscapeStatistics {
            total_smd_pins: fanout.total_smd_pins,
            escaped_count: fanout.escaped_count,
            escaped_percentage: percentage,
        }
    }
}

/// Port of `BatchFanout.FanoutPassStatus` (BatchFanout.java:609-622), a `record`: "status
/// snapshot for a single fanout pass".
///
/// Filled by `publishProgress` (`:540-576`, Task 12) and read by
/// `FanoutProgressListener`-shaped consumers only — plan-7 ruling 11 routes those through
/// [`ProgressSink`](super::ProgressSink), and **no port decision reads any field**.
// renamed: BatchFanout.FanoutPassStatus -> fr_router::pipeline::FanoutPassStatus — a Java
// `record`, which `audit-port.sh`'s line-based extraction reads as a public method of the
// enclosing class.
#[derive(Debug, Clone, PartialEq)]
pub struct FanoutPassStatus {
    /// `passNo` (`:610`) — 0-based, as `fanoutBoard`'s loop variable is.
    pub pass_no: i32,
    /// `ripupCosts` (`:611`).
    pub ripup_costs: i32,
    /// `totalPins` (`:612`).
    pub total_pins: i32,
    /// `pinsToGo` (`:613`).
    pub pins_to_go: i32,
    /// `routedCount` (`:614`).
    pub routed_count: i32,
    /// `notRoutedCount` (`:615`).
    pub not_routed_count: i32,
    /// `insertErrorCount` (`:616`).
    pub insert_error_count: i32,
    /// `extraViasThisPass` (`:617`).
    pub extra_vias_this_pass: i32,
    /// `extraViasTotal` (`:618`).
    pub extra_vias_total: i32,
    /// `passDurationMillis` (`:619`) — wall clock, and therefore never a decision input.
    pub pass_duration_millis: i64,
    /// `boardStatistics` (`:620`).
    pub board_statistics: Box<BoardStatistics>,
    /// `passCompleted` (`:621`).
    pub pass_completed: bool,
    /// `escapeStatistics` (`:622`).
    pub escape_statistics: EscapeStatistics,
}

/// Port of `BatchFanout.FanoutRunSummary` (BatchFanout.java:625-629), a `record`: "summary of a
/// complete fanout run" — what `fanoutBoard` answers (`:161-162`) and what Task 12 hands back to
/// `AutorouteBatchLoop`.
///
/// **Four components, not the plan sketch's three.** The sketch has `timed_out`, `passes_run` and
/// `escape`; Java also carries `totalDurationMillis` (`:627`).
// renamed: BatchFanout.FanoutRunSummary -> fr_router::pipeline::FanoutRunSummary — a Java
// `record`, as above.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FanoutRunSummary {
    /// `completedPassCount` (`:626`).
    pub completed_pass_count: i32,
    /// `totalDurationMillis` (`:627`) — `Math.max(0, now - fanoutStart)` (`:160`). Wall clock:
    /// the one field of this type a parity run must not compare.
    pub total_duration_millis: i64,
    /// `escapeStatistics` (`:628`).
    pub escape_statistics: EscapeStatistics,
    /// `isTimedOut` (`:629`), from the **per-stage** deadline of `:111-116` — never the job stop
    /// flag; see [`BatchFanout::deadline`].
    pub is_timed_out: bool,
}

// not ported: `BatchFanout.FanoutProgressListener` (BatchFanout.java:578-583) — plan-7 ruling 11
// and controller ruling AK replace `NamedAlgorithm`'s listener interfaces and
// `autoroute/events/**` with the single [`ProgressSink`](super::ProgressSink) seam, which Task 12
// hands the [`FanoutPassStatus`] above. The interface is one method over one record and has no
// other implementor in `src/main`.

// =================================================================================================
// `Component.Pin` — BatchFanout.java:695-778
// =================================================================================================

/// Port of `BatchFanout.Component.Pin` (BatchFanout.java:695-778) — one SMD pin of one component,
/// with the four keys [`Self::compare_to`] can sort on.
///
/// # `pinSortingOrder` is not a field here, and Java's is not one either
///
/// Java's `Pin` is a **non-static inner class** of `Component` (`:695`), so `compareTo` reads
/// `pinSortingOrder` off the enclosing `Component` (`:640`, `:744`). The port keeps the string on
/// [`FanoutComponent::pin_sorting_order`] the same way and passes it to
/// [`JavaTreeSet::add_by`](crate::JavaTreeSet::add_by) as the comparator's captured state, which
/// is why a `FanoutPin` alone has no `Ord`.
#[derive(Debug, Clone, PartialEq)]
pub struct FanoutPin {
    /// `boardPin` (`:697`) — the board item, by id.
    pub pin: ItemId,
    /// `boardPin.pinIndex` (`board/model/items/Pin.java:40`), read by `compareTo`'s tie-break
    /// (`:774`). Cached because the comparator must not need the board.
    pub pin_index: i32,
    /// `distanceToComponentCenter` (`:698`), `pinLocation.distance(gravityCenterOfSmdPins)`
    /// (`:708`).
    pub distance_to_component_center: f64,
    /// `distanceToClosestOnNet` (`:699`), `:710-723`.
    ///
    /// **`f64::MAX` when the pin's net has no other pin** — Java seeds `minDistance` with
    /// `Double.MAX_VALUE` (`:711`) and only lowers it inside the loop, and the loop does not run
    /// at all for `netNumber <= 0` (`:713`). Quirk **#219**.
    pub distance_to_closest_on_net: f64,
    /// `surroundingsDensity` (`:700`), `:725-738`: how many *other* SMD pins with nets lie within
    /// 20 mm of this one, in board units.
    ///
    /// The plan's interface sketch omits this field and calls `Pin.compareTo` "a `double`
    /// selected by `settings.fanout.pinSortingOrder`". It is an `int`, it is one of the four
    /// sorting keys (`:765-771`), and Java wins.
    pub surroundings_density: i32,
}

impl FanoutPin {
    /// Port of the `Pin(Pin, Collection<Pin>, RoutingBoard)` constructor (BatchFanout.java:702-739).
    ///
    /// `board_smd_pin_list` is the ctor's filtered SMD-pin list (`:46-51`) — the *whole* board's,
    /// not this component's, because `surroundingsDensity` counts across components (`:730`).
    /// `gravity_center` is the enclosing `Component`'s, which Java reads off the outer instance
    /// (`:708`).
    #[must_use]
    pub fn new(
        board: &Board,
        board_pin: ItemId,
        board_smd_pin_list: &[ItemId],
        gravity_center: &FloatPoint,
    ) -> FanoutPin {
        let ctx = board.ctx();
        let pin_item = board.get_item(board_pin).expect("a board SMD pin");
        let Item::Pin(pin) = pin_item else {
            panic!("BatchFanout.Component.Pin: getSmdPins() answers Pins only");
        };
        // :707-708.
        let pin_location = pin.get_center(&ctx).to_float();
        let distance_to_component_center = pin_location.distance(gravity_center);

        // :710-723 — `distanceToClosestOnNet`.
        let mut min_distance = f64::MAX; // :711
        // :712 — `netCount() > 0 ? getNetNumber(0) : 0`.
        let net_number = if pin_item.net_count() > 0 {
            pin_item.get_net_number(0)
        } else {
            0
        };
        if net_number > 0 {
            // :714 — **every** pin on the board, not only the SMD ones.
            for other in board.get_pins() {
                let other_item = board.get_item(other).expect("a board pin");
                // :715. Java's `otherPin != boardPin` is reference identity, which for an item
                // graph keyed by id is the id comparison.
                if other != board_pin && other_item.contains_net(net_number) {
                    let Item::Pin(other_pin) = other_item else {
                        continue;
                    };
                    // :716-719.
                    let dist = pin_location.distance(&other_pin.get_center(&ctx).to_float());
                    if dist < min_distance {
                        min_distance = dist;
                    }
                }
            }
        }
        let distance_to_closest_on_net = min_distance; // :723

        // :725-738 — `surroundingsDensity`.
        let resolution = board.communication.get_resolution(Unit::Um); // :726-727
        let max_dist = 20_000.0 * resolution; // :728, "20.0 mm in coordinate units"
        let mut density = 0_i32;
        for other in board_smd_pin_list {
            if *other == board_pin {
                continue; // :731
            }
            let Some(Item::Pin(other_pin)) = board.get_item(*other) else {
                continue;
            };
            // :732-735.
            let dist = pin_location.distance(&other_pin.get_center(&ctx).to_float());
            if dist <= max_dist {
                density += 1;
            }
        }

        FanoutPin {
            pin: board_pin,
            pin_index: pin.get_pin_index(),
            distance_to_component_center,
            distance_to_closest_on_net,
            surroundings_density: density,
        }
    }

    /// Port of `Pin.compareTo(Pin)` (BatchFanout.java:741-777), with the enclosing `Component`'s
    /// `pinSortingOrder` (`:640`) passed in because Rust has no inner classes.
    ///
    /// The four recognised strings are `"inner_first"`, `"outer_first"`,
    /// `"distanceToClosestOnNet"` and `"surroundingsDensity"` — matched with
    /// `String.equals` (case sensitive) against a **constant receiver**, so a `null`
    /// `pinSortingOrder` takes no branch rather than throwing. Then `:773-775` applies the
    /// `pinIndex` tie-break **unconditionally**, on every branch including the one no string
    /// matched.
    ///
    /// # The `<`/`>` chain is transcribed, not replaced by `total_cmp`
    ///
    /// Java computes a `double` *difference* and tests its sign (`:745-750` and the three arms
    /// below it), which is not the same function as comparing the two operands: it answers
    /// `Equal` for `Double.MAX_VALUE` against itself (the [`distance_to_closest_on_net`](Self::distance_to_closest_on_net) case of
    /// quirk #219, where two pins alone on their nets tie and fall through to `pinIndex`), and
    /// it answers `Equal` for a `NaN` operand where `total_cmp` would order it. No distance here
    /// can be `NaN`, so the port's answer equals Java's everywhere — but the shape is Java's.
    ///
    /// # Java-wins: scan ruling 8's "falls through with `result = 0`" is half right
    ///
    /// The plan (and the task brief) say an unrecognised `pinSortingOrder` "leaves `result = 0`",
    /// and conclude that the comparator "can return `0` for two distinct pins" and is therefore
    /// not a total order. **`:773-775` is outside the `if`/`else if` chain**, so an unrecognised
    /// string leaves `result = 0` only until the tie-break runs, and the order becomes *pure*
    /// `pinIndex` — which the plan also says, one sentence later, and which is a total order over
    /// one component's pins because `pinIndex` is "the index of the pin in its component"
    /// (`board/model/items/Pin.java:49`) and no component holds two pins at one index.
    ///
    /// So this comparator returns `0` for two **distinct** pins only on a board that has
    /// duplicated a `(component, pinIndex)` pair, which no reader produces. The container stays a
    /// [`JavaTreeSet`] regardless, and deliberately: the claim "no board duplicates a pin index"
    /// is a property of the *input*, not of the comparator, and `JavaTreeSet` is `java.util.TreeSet`
    /// whether or not the input has that property, while `BTreeSet` is defined only if it does.
    /// Quirk **#220** records the correction.
    #[must_use]
    pub fn compare_to(&self, other: &FanoutPin, pin_sorting_order: Option<&str>) -> i32 {
        let mut result = 0_i32; // :743
        let order = pin_sorting_order.unwrap_or("");
        if order == "inner_first" {
            // :744-750.
            let delta_dist = self.distance_to_component_center - other.distance_to_component_center;
            if delta_dist > 0.0 {
                result = 1;
            } else if delta_dist < 0.0 {
                result = -1;
            }
        } else if order == "outer_first" {
            // :751-757 — the same difference, the opposite sign.
            let delta_dist = self.distance_to_component_center - other.distance_to_component_center;
            if delta_dist > 0.0 {
                result = -1;
            } else if delta_dist < 0.0 {
                result = 1;
            }
        } else if order == "distanceToClosestOnNet" {
            // :758-764.
            let delta = self.distance_to_closest_on_net - other.distance_to_closest_on_net;
            if delta > 0.0 {
                result = 1;
            } else if delta < 0.0 {
                result = -1;
            }
        } else if order == "surroundingsDensity" {
            // :765-771 — "densest first", so the subtraction is the other way round. `int`
            // arithmetic, not `double`: the plan's sketch has only `double` keys.
            let delta = other.surroundings_density - self.surroundings_density;
            if delta > 0 {
                result = 1;
            } else if delta < 0 {
                result = -1;
            }
        }
        // :773-775 — unconditional, on every branch above and on the one that matched nothing.
        if result == 0 {
            result = self.pin_index - other.pin_index;
        }
        result // :776
    }
}

// =================================================================================================
// `Component` — BatchFanout.java:631-693
// =================================================================================================

/// Port of `BatchFanout.Component` (BatchFanout.java:631-693) — one board component with SMD
/// pins, its gravity centre and its sorted pin set.
///
/// # `BTreeSet`-safe, confirmed rather than assumed (ruling 5)
///
/// [`Self::compare_to`] (`:682-693`) sorts on `smdPinCount` **descending** then
/// `boardComponent.id` **ascending**. Both are `final` — `smdPinCount` is written once in the
/// constructor (`:665`) and `boardComponent` is the `final` field at `:633` — so neither key can
/// move under a set that has already placed the element, and the pair is a **total order**
/// because component ids are unique. `BTreeSet<FanoutComponent>` is therefore Java's `TreeSet`
/// here, and the `Ord` below is the whole of the element's identity: Java's `Component` declares
/// no `equals`, so its `TreeSet` membership is `compareTo` and nothing else, which is exactly
/// what an `Ord`-consistent `Eq` gives.
#[derive(Debug, Clone)]
pub struct FanoutComponent {
    /// `boardComponent.id` (`:633`, read at `:652` and `:690`). The port names the component by
    /// its id because `fr_board::Components` is keyed by one and has no handle type.
    pub component: i32,
    /// `boardComponent.name`, cached from the same object. Read by `fanoutPass:234`'s
    /// `boardComponent.name + "-" + boardPin.name()`, which is
    /// `AutorouteControl.fanoutStartPinName`'s twin.
    pub component_name: String,
    /// `smdPins` (`:635`), the `TreeSet<Pin>` of `:673-677` — **[`JavaTreeSet`]**, see the module
    /// docs' container table and [`FanoutPin::compare_to`].
    pub smd_pins: JavaTreeSet<FanoutPin>,
    /// `gravityCenterOfSmdPins` (`:638`): "the center of gravity of all SMD pins of this
    /// component" (`:650-670`).
    pub gravity_center_of_smd_pins: FloatPoint,
    /// `smdPinCount` (`:634`), `currentPinList.size()` (`:665`) — the count **before** the
    /// `TreeSet` is filled, so a component whose pins collapse under `compareTo` still reports
    /// the unsorted count. Kept as an `i32` because `compareTo` subtracts it (`:683`).
    pub smd_pin_count: i32,
    /// `pinSortingOrder` (`:640`), the string `Pin.compareTo` reads off the enclosing instance.
    /// `None` is Java's `null`, which matches none of the four `equals` tests.
    pub pin_sorting_order: Option<String>,
}

impl FanoutComponent {
    /// Port of the `Component(Component, Collection<Pin>, String, RoutingBoard)` constructor
    /// (BatchFanout.java:642-678).
    #[must_use]
    pub fn new(
        board: &Board,
        board_component: i32,
        board_smd_pin_list: &[ItemId],
        pin_sorting_order: Option<&str>,
    ) -> FanoutComponent {
        let component_id = board.components.get(board_component).id; // :652
        let component_name = board.components.get(board_component).name.clone();

        // :650-657 — this component's share of the filtered SMD pin list, in list order.
        let current_pin_list: Vec<ItemId> = board_smd_pin_list
            .iter()
            .copied()
            .filter(|pin| {
                board
                    .get_item(*pin)
                    .is_some_and(|item| item.component_id() == component_id)
            })
            .collect();

        // :658-670 — the gravity centre. Java divides only when the count is positive, so a
        // component with no SMD pins keeps `(0, 0)`; `:58` then drops it from `sortedComponents`.
        let ctx = board.ctx();
        let mut x = 0.0_f64;
        let mut y = 0.0_f64;
        for pin in &current_pin_list {
            let Some(Item::Pin(current_pin)) = board.get_item(*pin) else {
                continue;
            };
            let current_point = current_pin.get_center(&ctx).to_float();
            x += current_point.x;
            y += current_point.y;
        }
        let smd_pin_count = i32::try_from(current_pin_list.len()).expect("a board pin count");
        if smd_pin_count > 0 {
            x /= f64::from(smd_pin_count);
            y /= f64::from(smd_pin_count);
        }
        let gravity_center_of_smd_pins = FloatPoint::new(x, y);

        // :672-677 — the sorted pin set, built in `currentPinList` order.
        let mut smd_pins = JavaTreeSet::new();
        for pin in &current_pin_list {
            let fanout_pin =
                FanoutPin::new(board, *pin, board_smd_pin_list, &gravity_center_of_smd_pins);
            smd_pins.add_by(fanout_pin, |a, b| {
                a.compare_to(b, pin_sorting_order).cmp(&0)
            });
        }

        FanoutComponent {
            component: component_id,
            component_name,
            smd_pins,
            gravity_center_of_smd_pins,
            smd_pin_count,
            pin_sorting_order: pin_sorting_order.map(str::to_owned),
        }
    }

    /// Port of `Component.compareTo(Component)` (BatchFanout.java:680-693): "sort the components,
    /// so that components with more pins come first."
    ///
    /// `this.smdPinCount - other.smdPinCount` is `int` arithmetic on two non-negative pin counts,
    /// so it cannot overflow on any board; the same holds for the `boardComponent.id` difference
    /// at `:690`, where ids are positive and 1-based.
    #[must_use]
    pub fn compare_to(&self, other: &FanoutComponent) -> std::cmp::Ordering {
        // :683.
        let compare_value = self.smd_pin_count - other.smd_pin_count;
        let result = if compare_value > 0 {
            -1 // :685-686
        } else if compare_value < 0 {
            1 // :687-688
        } else {
            self.component - other.component // :690
        };
        result.cmp(&0) // :692
    }
}

impl PartialEq for FanoutComponent {
    fn eq(&self, other: &FanoutComponent) -> bool {
        self.compare_to(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for FanoutComponent {}

impl PartialOrd for FanoutComponent {
    fn partial_cmp(&self, other: &FanoutComponent) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// `Component implements Comparable<Component>` (`:631`), and the `TreeSet` at `:52` is ordered
/// by it and by nothing else.
impl Ord for FanoutComponent {
    fn cmp(&self, other: &FanoutComponent) -> std::cmp::Ordering {
        self.compare_to(other)
    }
}

// =================================================================================================
// `BatchFanout` — BatchFanout.java:20-78 (Task 12 adds the methods)
// =================================================================================================

/// Port of `autoroute.pipeline.BatchFanout` (BatchFanout.java:20-780): "handles the sequencing of
/// the fanout inside the batch autorouter."
///
/// Task 11 lands the type and its constructor; **Task 12 adds `fanout_board` and `fanout_pass`**
/// (scan ruling 7). See the module docs for what the constructor does and does not hold.
#[derive(Debug)]
pub struct BatchFanout<'a> {
    /// `sortedComponents` (`:25`), filled 1-based over `board.components` at `:53-61` — every
    /// component with at least one net-carrying SMD pin, in `Component.compareTo` order.
    pub sorted_components: BTreeSet<FanoutComponent>,
    /// `settings` (`:24`).
    pub settings: &'a RouterSettings,
    /// `totalSmdPinCount` (`:26`), `:62-76`: the sum of `smdPinCount` over
    /// [`Self::sorted_components`] — so a component that was dropped at `:58` contributes
    /// nothing, and a pin the `TreeSet` dropped still counts.
    pub total_smd_pin_count: i32,
    /// `alreadyConnectedPinCount` (`:27`), `:66-74`: the pins whose unconnected set is empty.
    pub already_connected_pin_count: i32,
    /// `progressThrottler` (`:28`), `new ProgressThrottler(1000)`.
    pub progress_throttler: ProgressThrottler,
    /// `lastNotRoutedCount` (`:29`), written by Task 12's `fanoutPass`.
    pub last_not_routed_count: i32,
    /// `extraViasTotal` (`:30`), written by Task 12's `fanoutPass`.
    pub extra_vias_total: i32,
    /// `totalItemsFanouted` (`:31`) — `public` in Java too.
    pub total_items_fanouted: i32,
    /// `deadlineMs` (`:32`), set by [`BatchFanout::fanout_board`] from
    /// `settings.fanout.timeoutString` (`:94-99`) and read at `:111-116` and `:396`.
    ///
    /// # A monotonic [`Instant`], not Java's epoch milliseconds
    ///
    /// Java writes `fanoutStart + timeoutSeconds * 1000`, an absolute wall-clock instant in
    /// milliseconds since the epoch, and compares it with `System.currentTimeMillis()`. The port
    /// keeps the *meaning* — an absolute instant the stage must not run past — on the clock the
    /// rest of this crate measures with ([`fr_board::TimeLimit`] is `Instant`-based for the same
    /// reason, and records the same deviation). Task 11 declared this field as an `Option<i64>`
    /// of Java's epoch milliseconds; Task 12, which owns the reads, made it the instant, because
    /// an epoch number the port never produces would have had to be converted at both read
    /// sites.
    ///
    /// **Not a [`TimeLimit`](fr_board::TimeLimit)**, deliberately: `TimeLimit::is_exceeded` is
    /// Java's `TimeLimit.limitExceeded`, a **strict** `>` (`datastructures/TimeLimit.java:18-21`),
    /// while `:112` and `:396` are `System.currentTimeMillis() >= deadlineMs` — non-strict. The
    /// difference is one millisecond and no parity run is near it, but the port compares the way
    /// the site compares.
    ///
    /// # This is **not** the stop flag (Task 4's documented split)
    ///
    /// `:113` writes `this.isTimedOut = true` and `break`s; it never calls `requestStop` or
    /// `requestStopAutoRouter` — a grep of `BatchFanout.java` finds neither. So a fanout timeout
    /// ends the fanout stage and leaves the router and the optimizer running. The port must
    /// therefore **not** call
    /// [`RouterStop::poll_deadline`](super::RouterStop::poll_deadline) here, which requests
    /// `ALL`; see `pipeline::stop`'s module docs, whose marker this field discharges for the
    /// declaration half (Task 12 owns the reads).
    pub deadline: Option<Instant>,
    /// `isTimedOut` (`:33`) — the per-stage flag of the field above.
    pub is_timed_out: bool,
}

impl<'a> BatchFanout<'a> {
    /// Port of the private `BatchFanout(RoutingBoard, RouterSettings, StoppableThread)`
    /// constructor (BatchFanout.java:35-78).
    ///
    /// The `StoppableThread` argument has no counterpart (see the module docs' roster line for
    /// `BatchFanout.thread`), and the board is a parameter rather than a field.
    #[must_use]
    pub fn new(board: &Board, settings: &'a RouterSettings) -> BatchFanout<'a> {
        // :39-42.
        let sorting_order: &str = settings
            .fanout
            .as_ref()
            .and_then(|f| f.pin_sorting_order.as_deref())
            .unwrap_or("outer_first");

        // :43-51. "Filter out SMD pins that belong to no net — they don't need fanout and would
        // inflate total pin counts and escape statistics."
        let board_smd_pin_list = board.get_smd_pins();
        let board_smd_pin_list_with_nets: Vec<ItemId> = board_smd_pin_list
            .into_iter()
            .filter(|pin| {
                board
                    .get_item(*pin)
                    .is_some_and(|item| item.net_count() > 0)
            })
            .collect();

        // :52-61. **1-based**, `1..=components.count()`, which is `Components.get`'s own indexing
        // (Components.java:84-94).
        let mut sorted_components: BTreeSet<FanoutComponent> = BTreeSet::new();
        for i in 1..=i32::try_from(board.components.count()).expect("a board component count") {
            let current_component =
                FanoutComponent::new(board, i, &board_smd_pin_list_with_nets, Some(sorting_order));
            if current_component.smd_pin_count > 0 {
                sorted_components.insert(current_component);
            }
        }

        // :62-75.
        let mut pin_count = 0_i32;
        let mut already_connected = 0_i32;
        for component in &sorted_components {
            pin_count += component.smd_pin_count;
            for pin in &component.smd_pins {
                // ":67-68 — a pin is already connected if all items in its connected set are on
                // the pin's layer and its unconnected set is empty — same logic as
                // RoutingBoard.fanout()." Only the second half is actually tested (`:71`).
                let Some(item) = board.get_item(pin.pin) else {
                    continue;
                };
                let net_number = item.get_net_number(0); // :70
                if board.unconnected_set(pin.pin, net_number).is_empty() {
                    already_connected += 1; // :72
                }
            }
        }

        BatchFanout {
            sorted_components,
            settings,
            total_smd_pin_count: pin_count,                   // :76
            already_connected_pin_count: already_connected,   // :77
            progress_throttler: ProgressThrottler::new(1000), // :28
            last_not_routed_count: 0,                         // :29
            extra_vias_total: 0,                              // :30
            total_items_fanouted: 0,                          // :31
            deadline: None,                                   // :32, set by `fanout_board`
            is_timed_out: false,                              // :33
        }
    }
}

// =================================================================================================
// The three arms of `fanoutBoard`/`fanoutPass` a corpus run cannot reach — BatchFanout.java:125-156,
// :173-183, :238-259
// =================================================================================================

/// Why [`BatchFanout::fanout_board`]'s pass loop stopped, as [`FanoutLoopState::after_pass`]
/// answers it.
///
/// Not a Java type: Java `break`s out of `:110-157` from four places and records nothing about
/// which one fired (three of the four log a `FRLogger.info` line the port drops). The enum exists
/// so that the loop reads as four named decisions and so that `p7t5 board` can print which one
/// ended the run — the transcript line the brief asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanoutStop {
    /// `:125-127` — the pass routed nothing.
    NothingRouted,
    /// `:128-148` — the oscillation detector saw the same `(routedCount, viaCount)` pair once
    /// too often. Quirk **#222**.
    Stagnated,
    /// `:149-151` — the per-stage deadline fired inside the pass ([`BatchFanout::deadline`]).
    TimedOut,
    /// `:152-156` — `board.getHash()` has not moved since the previous pass. Controller ruling
    /// AH's first decision site.
    UnchangedHash,
}

/// The three pieces of pass-to-pass state `fanoutBoard`'s loop carries (`:106-108`, `:109`), and
/// the decision block that reads them (`:125-156`).
///
/// # Why this is lifted out of the loop
///
/// Task 11's precedent (`sorted_unconnected_targets`, `combined_fallback_via_rule`): a pure
/// function of data a test can build, with cases **no corpus board reaches**. The oscillation
/// detector needs four passes that route the same number of pins and leave the same number of
/// vias, and the hash arm needs a pass that routes something without moving the board's hash;
/// neither happens on the eight `p7t5` stems. A test that had to route a board to observe either
/// would be a slow test of the router rather than a test of the decision.
///
/// Everything else about the loop — the deadline gate, the `maxItems` gate and the call itself —
/// stays inline in [`BatchFanout::fanout_board`], which reads as Java does with one name
/// substituted for one block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FanoutLoopState {
    /// `previousBoardState` (`:107`), seeded [`i64::MIN`] — Java's `Long.MIN_VALUE`.
    pub previous_board_state: i64,
    /// `identicalPasses` (`:108`).
    pub identical_passes: i32,
    /// `lastBoardHash` (`:109`), `board.getHash()` **before the first pass**. Java's is a hex
    /// MD5 string and the port's is [`Board::structural_hash`]'s `u64`; controller ruling AH
    /// compares the *decision*, never the value.
    pub last_board_hash: u64,
}

impl FanoutLoopState {
    /// `stagnationPassLimit` (`:105`), a `final int` local.
    ///
    /// # Java bug (quirk #222): the constant is a repeat count, so it takes **four** identical
    /// passes to fire
    ///
    /// `identicalPasses` is incremented only when the new state **equals** the previous one, and
    /// the first pass to produce a state can only set `previousBoardState`. So pass *n* seeds,
    /// passes *n+1*, *n+2* and *n+3* increment, and the break fires on the fourth pass with the
    /// same `(routedCount, viaCount)` — not the third, as `:143`'s own message ("no progress for
    /// 3 consecutive passes") says.
    // Java bug: `BatchFanout.fanoutBoard` (autoroute/pipeline/BatchFanout.java:105, :134-146) —
    // `stagnationPassLimit` counts **repeats**, not passes, so the break fires on the fourth
    // identical pass while `:141-146` logs "no progress for 3 consecutive passes". Quirk **#222**.
    pub const STAGNATION_PASS_LIMIT: i32 = 3;

    /// `:106-109`, given the board's hash before the first pass.
    #[must_use]
    pub fn new(initial_board_hash: u64) -> FanoutLoopState {
        FanoutLoopState {
            previous_board_state: i64::MIN,
            identical_passes: 0,
            last_board_hash: initial_board_hash,
        }
    }

    /// `long boardState = ((long) routedCount << 32) ^ board.getVias().size()` (`:133`).
    ///
    /// # Java bug (quirk #222): the pair is a summary, not a board
    ///
    /// The defect is not the arithmetic — `board.getVias().size()` is a non-negative `int`, so
    /// the shift-and-XOR packs the two numbers unambiguously on any board that exists. It is what
    /// the two numbers *are*: two passes that route the same **number** of pins and leave the
    /// same **number** of vias are treated as identical progress even when they escaped
    /// *different* pins in *different* places, which is exactly what a ripping cycle does. That
    /// is the situation the comment at `:128-132` describes and the reason the check exists; it
    /// is also why it can end a run that is still moving. Three lines later Java takes
    /// `board.getHash()`, which is the thing that actually answers "did this pass change
    /// anything".
    ///
    /// # The `as i32` is Java's `int`, not a port choice
    ///
    /// `via_count` is a `usize` here and an `int` there, and Java widens the `int` to `long` with
    /// sign extension inside the XOR. The port therefore narrows to `i32` **first** and widens
    /// after, so that a hypothetical count at or above `2^31` lands where Java's would rather
    /// than in a bit pattern Java cannot produce. No board reaches it — `Vec::len` would have to
    /// exceed two billion vias — and
    /// `the_board_state_packs_the_routed_count_above_the_via_count` pins the narrowing rather
    /// than a reachable defect.
    // Java bug: `BatchFanout.fanoutBoard` (autoroute/pipeline/BatchFanout.java:133) — `boardState`
    // packs a pass into `((long) routedCount << 32) ^ board.getVias().size()`, a two-number
    // summary that cannot tell two passes apart when they escaped the same *count* of pins in
    // different places. Quirk **#222**.
    #[must_use]
    pub fn board_state(routed_count: i32, via_count: usize) -> i64 {
        // `(long) routedCount << 32` — Java's `<<` on a `long` discards the shifted-out bits, and
        // so does Rust's on an `i64`.
        let packed = i64::from(routed_count).wrapping_shl(32);
        // `board.getVias().size()` is an `int`; the XOR widens it with sign extension.
        let via_count = i64::from(via_count as i32);
        packed ^ via_count
    }

    /// The whole of `:125-156` — the four post-pass stops, in Java's order, answering the one
    /// that fired.
    ///
    /// `board_hash` is a closure because Java evaluates `board.getHash()` at `:153` **only** if
    /// the three earlier arms did not break, and [`Board::structural_hash`] walks the item graph.
    ///
    /// `is_timed_out` is [`BatchFanout::is_timed_out`] as the pass left it (`:149`) — the
    /// per-stage flag of `:396`, never the job stop flag (Task 4's split).
    pub fn after_pass<F: FnOnce() -> u64>(
        &mut self,
        routed_count: i32,
        via_count: usize,
        is_timed_out: bool,
        board_hash: F,
    ) -> Option<FanoutStop> {
        // :125-127.
        if routed_count == 0 {
            return Some(FanoutStop::NothingRouted);
        }
        // :133.
        let board_state = FanoutLoopState::board_state(routed_count, via_count);
        // :134-147.
        if board_state == self.previous_board_state {
            self.identical_passes += 1;
            if self.identical_passes >= FanoutLoopState::STAGNATION_PASS_LIMIT {
                return Some(FanoutStop::Stagnated);
            }
        } else {
            self.identical_passes = 0;
            self.previous_board_state = board_state;
        }
        // :149-151.
        if is_timed_out {
            return Some(FanoutStop::TimedOut);
        }
        // :152-156. Ruling AH: an equality **decision** between two hashes, never a value.
        let current_board_hash = board_hash();
        if current_board_hash == self.last_board_hash {
            return Some(FanoutStop::UnchangedHash);
        }
        self.last_board_hash = current_board_hash;
        None
    }
}

/// `fanoutPass:173` and `:179-183` — the ripup costs one pass hands
/// [`RoutingBoardExt::fanout`](crate::board_ext::RoutingBoardExt::fanout).
///
/// `settings.getStartRipupCosts() * (passNo + 1)`, or **`-1`** when `fanout.ripupAllowed` is
/// explicitly `false`: "negative ripup costs signal 'no ripup' to RoutingBoard.fanout()" (`:182`).
/// An absent `fanout` block and an absent `ripupAllowed` both mean *allowed* (`:179-181`), which
/// is the opposite default from `RouterSettings.isFanoutEnabled`.
///
/// Lifted out of [`BatchFanout::fanout_pass`] so a test can walk the pass number without routing
/// a board; `fanout_pass` is its only production caller.
#[must_use]
pub fn fanout_ripup_costs(settings: &RouterSettings, pass_no: i32) -> i32 {
    // :173. Java's `int` multiply; `getStartRipupCosts` is clamped to `>= 1` by its setter and
    // `passNo` is a loop index below `maxPasses`, so no corpus board can overflow it.
    let ripup_costs = settings
        .get_start_ripup_costs()
        .wrapping_mul(pass_no.wrapping_add(1));
    // :179-181.
    let ripup_allowed = settings
        .fanout
        .as_ref()
        .and_then(|fanout| fanout.ripup_allowed)
        .unwrap_or(true);
    // :182-183.
    if ripup_allowed { ripup_costs } else { -1 }
}

/// `fanoutPass:238-259` — the `canUseVias` gate, answering **whether the pin is fanned out at
/// all**: `false` is `:255-258`'s `--pinsToGo; continue`.
///
/// ```java
/// Net net = this.routingBoard.rules.nets.get(netNumber);
/// if (net != null) {
///   NetClass netClass = net.getNetClass();
///   ViaRule viaRule = netClass != null ? netClass.getViaRule() : null;
///   boolean hasBoardVias =
///       !this.routingBoard.rules.viaRules.isEmpty()
///           && this.routingBoard.rules.viaRules.firstElement().viaCount() > 0;
///   boolean fallbackAllowed = … fanout.fallbackToBoardVias … && hasBoardVias;
///   boolean canUseVias = (viaRule != null && viaRule.viaCount() > 0) || fallbackAllowed;
///   if (!canUseVias) { --pinsToGo; continue; }
/// }
/// ```
///
// Java bug: `BatchFanout.fanoutPass` (autoroute/pipeline/BatchFanout.java:239-259) — the whole
// gate hangs off `net != null`, so a pin whose net number names **no net** skips the via check
// entirely and is fanned out with whatever `AutorouteControl` derives, while a pin whose net
// exists but whose class has no vias is skipped. The safe case is the one that is checked and the
// unchecked case is the odd one; quirk **#223**.
///
/// The port answers `true` for the absent net exactly as Java falls through the `if`, and
/// `crates/fr-router/tests/fanout.rs`'s `a_null_net_pin_skips_the_via_gate` is the pin. Lifted
/// out for the same reason as [`fanout_ripup_costs`]: no corpus board carries an SMD pin whose
/// net number is not in `rules.nets`, so the arm is unreachable from `p7t5`.
#[must_use]
pub fn fanout_pin_can_use_vias(board: &Board, settings: &RouterSettings, net_number: i32) -> bool {
    // :239. `Nets.get` answers `null` for a number outside `1..=count`.
    let Some(net) = board.rules.nets.get(net_number) else {
        return true;
    };
    // :240-241. `NetClass.getViaRule` is `Option` because Java's field is nullable.
    let via_count = board
        .rules
        .net_classes
        .get(net.get_net_class())
        .get_via_rule()
        .map_or(0, fr_board::ViaRule::via_count);
    // :242-245 — `firstElement()`, not "any rule".
    let has_board_vias =
        !board.rules.via_rules.is_empty() && board.rules.via_rules[0].via_count() > 0;
    // :246-250.
    let fallback_allowed = settings
        .fanout
        .as_ref()
        .and_then(|fanout| fanout.fallback_to_board_vias)
        .unwrap_or(false)
        && has_board_vias;
    // :251.
    via_count > 0 || fallback_allowed
}

/// Port of `TextManager.parseTimespanString` (util/TextManager.java:83-95) together with
/// `convertFromTimespanToDurationFormat` (`:103-119`), for the one caller Plan 7 has:
/// `fanoutBoard:94-99`.
///
/// `fr-settings` deferred the method to Plan 8 (`crates/fr-settings/src/lib.rs`) because the
/// *settings* path never parses a timeout string — the only reader there is
/// `RoutingJobSchedulerActionThread.threadAction:44`. `BatchFanout` is a second reader, in
/// Plan 7, so the function landed here rather than moving that roster line. **Plan 8 Task 0
/// consumed the `fr-settings` line by re-exporting this function** as
/// `fr_core::parse_timespan_seconds` rather than porting the method a second time (plan-8
/// ruling 1: `fr-core` re-exports, it does not move); `fr_core::job_timeout_deadline` is
/// `threadAction:43-52`'s ladder on top of it, and `crates/fr-core/tests/data/p8t0-timespans.txt`
/// pins both against the HEAD jar on thirty inputs.
///
/// Java splits on `':'` and builds an ISO-8601 duration:
/// `HH:mm:ss` → `PT<h>H<m>M<s>S`, `mm:ss` → `PT<m>M<s>S`, `ss` → `PT<s>S`; anything else leaves
/// the bare `"PT"`, which `Duration.parse` rejects. A `DateTimeParseException` answers `null`,
/// and so does a blank string.
///
// Java bug: `FanoutSettings.timeout`'s own javadoc (settings/FanoutSettings.java:98) gives
// `"5m"` and `"300s"` as the example values, and `TextManager.parseTimespanString`
// (util/TextManager.java:83-95) parses **neither** — both become `PT5mS` / `PT300sS`, which
// `Duration.parse` rejects, so the method answers `null` and the fanout stage silently runs with
// no timeout at all. Only the colon forms (`"300"`, `"5:00"`, `"0:05:00"`) work. Quirk **#224**;
// measured against a JDK 25 `Duration.parse`, not read off the regex.
///
/// The port reproduces the accepted grammar rather than `Duration.parse`'s whole surface. An
/// hour or minute component is Java's `[-+]?[0-9]+`; the **seconds** component additionally
/// allows a `[.,]`-separated fraction of at most nine digits, which is reachable because
/// `convertFromTimespanToDurationFormat` copies the caller's text through unchanged (`"1.5"`
/// becomes `PT1.5S`). `Duration.getSeconds()` is the whole-second field of a **normalised**
/// `(seconds, 0..=999_999_999 nanos)` pair, so it **floors** rather than truncating toward zero:
/// `"-1.5"` answers `-2` and `"1:-0.5"` answers `59`, both measured on a JDK 25. The `P…D` day
/// form is genuinely unreachable — the converter never emits a `D` — so the port does not carry
/// it, and anything else answers `None`.
///
/// `pub` for the same reason [`BatchFanout::fanout_pass`] is: `crates/fr-router/tests/fanout.rs`
/// pins quirk #224's grammar against a JDK 25 `Duration.parse`, and Java's own method is
/// `public static` anyway.
pub fn parse_timespan_seconds(timespan_string: &str) -> Option<i64> {
    // :84-86 — `null || isBlank()`.
    if timespan_string.trim().is_empty() {
        return None;
    }
    // :104 — `String.split(":")`.
    let parts = split_java(timespan_string, ':');
    // :105-119, then `Duration.parse` at `:89`. `Duration`'s grammar allows a `[.,]`-separated
    // fraction of at most nine digits on the **seconds** component only; the hour and minute
    // components are `[-+]?[0-9]+`.
    let (seconds, fraction) = match parts.as_slice() {
        // `PT<h>H<m>M<s>S`.
        [hours, minutes, seconds] => {
            let (whole, fraction) = duration_seconds_component(seconds)?;
            (
                duration_integer_component(hours)?
                    .checked_mul(3600)?
                    .checked_add(duration_integer_component(minutes)?.checked_mul(60)?)?
                    .checked_add(whole)?,
                fraction,
            )
        }
        // `PT<m>M<s>S`.
        [minutes, seconds] => {
            let (whole, fraction) = duration_seconds_component(seconds)?;
            (
                duration_integer_component(minutes)?
                    .checked_mul(60)?
                    .checked_add(whole)?,
                fraction,
            )
        }
        // `PT<s>S`.
        [seconds] => duration_seconds_component(seconds)?,
        // `"PT"` (`:104`'s split answered 0 or more than 3 parts), which `Duration.parse`
        // rejects at `:90-92`.
        _ => return None,
    };
    // `Duration.getSeconds()` is the whole-second field of a `(seconds, 0..=999_999_999 nanos)`
    // pair, and `Duration.ofSeconds` normalises a **negative** nanosecond adjustment by carrying
    // one second down — so `"-1.5"` answers `-2`, not `-1`, and `"1:-0.5"` answers `59`.
    // Measured on a JDK 25, not read off `Duration`'s source.
    if fraction == Fraction::Negative {
        seconds.checked_sub(1)
    } else {
        Some(seconds)
    }
}

/// Whether a parsed seconds component carried a non-zero fraction, and of which sign — the only
/// thing `Duration.getSeconds()` needs from it (see [`parse_timespan_seconds`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fraction {
    /// No `[.,]`, or one whose digits are all zero.
    Zero,
    /// A non-zero fraction on a non-negative component.
    Positive,
    /// A non-zero fraction on a component written with a `-` sign.
    Negative,
}

/// One `[-+]?[0-9]+` component of `Duration`'s grammar — the `H` and `M` slots.
fn duration_integer_component(text: &str) -> Option<i64> {
    let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<i64>().ok()
}

/// The `S` slot, `([-+]?[0-9]+)(?:[.,]([0-9]{0,9}))?`: the whole seconds and the fraction's sign.
fn duration_seconds_component(text: &str) -> Option<(i64, Fraction)> {
    let Some(separator) = text.find(['.', ',']) else {
        return duration_integer_component(text).map(|whole| (whole, Fraction::Zero));
    };
    let (whole_text, fraction_text) = text.split_at(separator);
    let fraction_digits = &fraction_text[1..];
    // `[0-9]{0,9}` — an empty fraction is legal (`"1."` parses), ten digits are not.
    if fraction_digits.len() > 9 || !fraction_digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let whole = duration_integer_component(whole_text)?;
    let fraction = if fraction_digits.bytes().all(|byte| byte == b'0') {
        Fraction::Zero
    } else if whole_text.starts_with('-') {
        Fraction::Negative
    } else {
        Fraction::Positive
    };
    Some((whole, fraction))
}

/// `String.split(String)` with a one-character regex-free separator.
///
/// Java drops **trailing** empty strings and keeps leading and interior ones, and it drops them
/// all the way to an empty array — `"::".split(":")` has length **0**, which is one of the two
/// arities `convertFromTimespanToDurationFormat` answers a bare `"PT"` for. The one exception is
/// an input the separator does not occur in at all, which `String.split` short-circuits to a
/// one-element array, so `""` answers `[""]` rather than `[]`.
fn split_java(text: &str, separator: char) -> Vec<&str> {
    if !text.contains(separator) {
        return vec![text];
    }
    let mut parts: Vec<&str> = text.split(separator).collect();
    while parts.last().is_some_and(|last| last.is_empty()) {
        parts.pop();
    }
    parts
}

// =================================================================================================
// `fanoutBoard` / `fanoutPass` / `publishProgress` — BatchFanout.java:81-163, :166-506, :508-576
// =================================================================================================

impl<'a> BatchFanout<'a> {
    /// Port of `BatchFanout.fanoutBoard(RoutingBoard, RouterSettings, Stoppable,
    /// FanoutProgressListener)` (BatchFanout.java:81-163) — "performs fanout routing for SMD
    /// components on board".
    ///
    // renamed: `BatchFanout.fanoutBoard` — Java's **two** overloads (`:81-84` and `:87-163`)
    // collapse into this one. The three-argument one is `fanoutBoard(board, settings, thread,
    // null)` and nothing else, and the port has no `null` listener: an absent listener is
    // [`NoopProgressSink`](super::NoopProgressSink), which is what `AutorouteBatchLoop`'s own
    // call site would pass (`AutorouteBatchLoop.java:123-172` passes a real one).
    ///
    /// # The pass loop, and its four stops
    ///
    /// `:110-157` runs at most `settings.fanout.maxPasses` (default 20) passes and leaves through
    /// one of **seven** doors: the loop head, the per-stage deadline (`:111-116`), the `maxItems`
    /// gate (`:117-122`) and the four [`FanoutStop`]s of [`FanoutLoopState::after_pass`].
    /// `p7t5 board`'s `STOP <reason>` line names which one ended a run.
    ///
    /// # The deadline is the **stage's**, and does not touch the stop flag
    ///
    /// `:111-116` writes [`BatchFanout::is_timed_out`] and `break`s. It does not call
    /// `requestStop` or `requestStopAutoRouter`, so a fanout timeout leaves the router and the
    /// optimizer running — see [`BatchFanout::deadline`] and `pipeline::stop`'s module docs,
    /// whose `obligation:` line this method discharges for `BatchFanout`. [`RouterStop`] is read
    /// here only through `fanoutPass:416`'s `isStopAutoRouterRequested`.
    ///
    /// # Controller caveat: the hash stop is defined against **warm** hashes
    ///
    /// `:152-156` compares two `BasicBoard.getHash()` values, and the port compares two
    /// [`Board::structural_hash`] values (ruling AH — a *decision*, never a value). Task 3
    /// measured that Java's **raw** hash decisions diverge one-way against its own history: 3 of
    /// 350 raw `p7t10` steps are `FANOUTSTOP false -> true`, i.e. the jar keeps fanning out where
    /// the port stops, because a lazily filled `DrillItem` cache moved a hash the board did not
    /// (**quirk #200**). Decision parity is therefore defined against `p7t10`'s `warm` mode, and
    /// `p7t5 board` prints the decision on both sides so the caveat stays measured rather than
    /// assumed.
    ///
    /// # Not ported
    ///
    /// The two `FRLogger.info` payloads of `:114` and `:137-146`, and the
    /// `AutorouteRuntimeMetrics` CPU/heap block `AutorouteBatchLoop` wraps this call in
    /// (`AutorouteBatchLoop.java:93-96`, `:175-216`).
    pub fn fanout_board(
        board: &mut Board,
        settings: &'a RouterSettings,
        stop: &RouterStop,
        budget: RouterBudget,
        progress: &mut dyn ProgressSink,
    ) -> Result<FanoutRunSummary, RouterError> {
        // :89.
        let mut fanout_instance = BatchFanout::new(board, settings);
        // `:28`'s `new ProgressThrottler(1000)` as ruling AI's knob. The default budget carries
        // Java's 1000, so this is the same throttler unless a driver disabled it.
        fanout_instance.progress_throttler = budget.progress_throttler();
        // :90.
        let fanout_start = Instant::now();
        // :91-100.
        if let Some(timeout_string) = settings
            .fanout
            .as_ref()
            .and_then(|fanout| fanout.timeout_string.as_deref())
            && let Some(timeout_seconds) = parse_timespan_seconds(timeout_string)
        {
            // :98 — `fanoutStart + timeoutSeconds * 1000`, on the port's monotonic clock.
            fanout_instance.deadline =
                instant_offset_ms(fanout_start, timeout_seconds.saturating_mul(1000));
        }
        // :101-104.
        let max_passes = settings
            .fanout
            .as_ref()
            .and_then(|fanout| fanout.max_passes)
            .unwrap_or(20);
        // :106-109. `stagnationPassLimit` (`:105`) lives on [`FanoutLoopState`].
        let mut completed_passes = 0_i32;
        let mut loop_state = FanoutLoopState::new(board.structural_hash());

        // :110.
        let mut i = 0_i32;
        while i < max_passes {
            // :111-116 — the per-stage deadline. **Not** `RouterStop::poll_deadline`.
            if fanout_instance.is_deadline_reached() {
                fanout_instance.is_timed_out = true; // :113
                break;
            }
            // :117-122.
            if fanout_instance.max_items_reached() {
                break;
            }
            // :123.
            let routed_count = fanout_instance.fanout_pass(board, i, stop, budget, progress)?;
            // :124.
            completed_passes += 1;
            // :125-156.
            let via_count = board.get_vias().len();
            let is_timed_out = fanout_instance.is_timed_out;
            if loop_state
                .after_pass(routed_count, via_count, is_timed_out, || {
                    board.structural_hash()
                })
                .is_some()
            {
                break;
            }
            i += 1;
        }

        // :158-159.
        let stats = BoardStatistics::with_options(board, None, false);
        let final_escape = EscapeStatistics::from_board_statistics(&stats);
        // :160. Wall clock, and the one component of the summary a parity run must not compare.
        let total_duration_millis =
            i64::try_from(fanout_start.elapsed().as_millis()).unwrap_or(i64::MAX);
        // :161-162.
        Ok(FanoutRunSummary {
            completed_pass_count: completed_passes,
            total_duration_millis,
            escape_statistics: final_escape,
            is_timed_out: fanout_instance.is_timed_out,
        })
    }

    /// `:111-112` and `:396` — `deadlineMs != null && System.currentTimeMillis() >= deadlineMs`,
    /// **non-strict**, on the port's monotonic clock. See [`BatchFanout::deadline`].
    #[must_use]
    pub fn is_deadline_reached(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    /// `:117-122` and `:222-230` — the same four-term `maxItems` gate, written twice in Java.
    ///
    /// `maxItems <= 0` is "no limit"; `DefaultSettings` sets [`i32::MAX`]
    /// (`crates/fr-settings/src/sources/default_settings.rs`), so no corpus run reaches it.
    #[must_use]
    pub fn max_items_reached(&self) -> bool {
        self.settings
            .fanout
            .as_ref()
            .and_then(|fanout| fanout.max_items)
            .is_some_and(|max_items| max_items > 0 && self.total_items_fanouted >= max_items)
    }

    /// Port of `BatchFanout.fanoutPass(int, FanoutProgressListener)` (BatchFanout.java:166-506) —
    /// "routes a fanout pass and returns the number of new fanouted SMD-pins in this pass".
    ///
    /// # The walk order is snapshotted, and that is not a deviation
    ///
    /// Java iterates the live `sortedComponents` and, inside it, the live `Component.smdPins`
    /// (`:220-221`). Nothing in the body writes either container — the only fields the loop
    /// touches are the counters and `totalItemsFanouted` — so the port collects the two levels
    /// into a `Vec` first, which is what lets `&mut self` and `&mut Board` coexist. The set is
    /// built once per *run*, not per pass, in both languages (`:89` constructs the instance and
    /// `fanoutBoard`'s loop reuses it), so a pin whose escape changes the board is **not**
    /// re-sorted between passes in Java either.
    ///
    /// # Not ported
    ///
    /// The nine `FRLogger.trace` payloads (`:185`, `:261`, `:290`, `:306`, `:323`, `:344`,
    /// `:359`, `:367`, `:445`), the three `FRLogger.info`/`debug` ones (`:226`, `:251`, `:397`)
    /// and the `progressListener == null` `FRLogger.info` at `:470-489`, with the three locals
    /// that exist only to fill them: `fullPinName` (`:233-234`, `boardComponent.name + "-" +
    /// boardPin.name()`), `targetCount` (`:236`, which calls `getUnconnectedSet` a second time
    /// purely to size it) and `alreadyConnectedCount` (`:219`, `:305`). Dropping `targetCount`
    /// drops a `getUnconnectedSet` call the port would otherwise make; it has no effect on the
    /// board and none on [`Board::structural_hash`], though it *is* one of the lazy-cache fills
    /// that can move Java's raw `getHash()` — see the warm-mode caveat on
    /// [`BatchFanout::fanout_board`].
    ///
    /// # `pub`, where Java's is `private`
    ///
    /// The plan's interface block writes this method `fn`, i.e. private, and inside the crate it
    /// has exactly one caller. It is `pub` because `scripts/differential/rust/src/bin/p7t5.rs`
    /// mode `pass` calls **the real method** for its `[real]` half, and a differential driver is
    /// a separate crate. `P7T5.java` does the same thing on the other side with
    /// `Method.setAccessible(true)`; the port has no reflection, so the widening *is* the
    /// reflection. Nothing else in the workspace calls it.
    pub fn fanout_pass(
        &mut self,
        board: &mut Board,
        pass_no: i32,
        stop: &RouterStop,
        budget: RouterBudget,
        progress: &mut dyn ProgressSink,
    ) -> Result<i32, RouterError> {
        // :167.
        let pass_start = Instant::now();
        // :168-171.
        let mut pins_to_go = self.total_smd_pin_count;
        let mut routed_count = 0_i32;
        let mut not_routed_count = 0_i32;
        let mut insert_error_count = 0_i32;
        // :172.
        let vias_before_pass = board.get_vias().len();
        // :173 and :179-183.
        let ripup_costs = self
            .settings
            .get_start_ripup_costs()
            .wrapping_mul(pass_no.wrapping_add(1));
        let effective_ripup_costs = fanout_ripup_costs(self.settings, pass_no);
        // :175-178 — `settings.fanout.maxMillisecondsPerPin ?? 10000L`, with ruling AI's budget
        // as the fallback so a parity run can disable the clock on both sides. Folding it into
        // the budget is what lets `:231-232` stay one call.
        let pass_budget = RouterBudget {
            fanout_ms_per_pin: self
                .settings
                .fanout
                .as_ref()
                .and_then(|fanout| fanout.max_milliseconds_per_pin)
                .map_or(budget.fanout_ms_per_pin, |base| {
                    i32::try_from(base).unwrap_or(if base < 0 { i32::MIN } else { i32::MAX })
                }),
            ..budget
        };

        // :203.
        self.progress_throttler.reset();
        // :204.
        let progress_stats = BoardStatistics::with_options(board, None, false);
        // :205-217 — the pass-start tick, with the placeholder escape statistics of `:213`.
        let pass_start_escape = EscapeStatistics {
            total_smd_pins: self.total_smd_pin_count,
            escaped_count: 0,
            escaped_percentage: 0.0,
        };
        self.publish_progress(
            progress,
            pass_no,
            ripup_costs,
            pins_to_go,
            routed_count,
            not_routed_count,
            insert_error_count,
            0,
            pass_start_escape,
            false,
            pass_start,
            &progress_stats,
        );

        // :218-221. See "the walk order is snapshotted" above.
        let walk: Vec<Vec<ItemId>> = self
            .sorted_components
            .iter()
            .map(|component| component.smd_pins.iter().map(|pin| pin.pin).collect())
            .collect();
        let mut max_limit_reached = false; // :218
        for component_pins in &walk {
            for current_pin in component_pins {
                // :222-230.
                if self.max_items_reached() {
                    max_limit_reached = true;
                    break;
                }
                // :231-232.
                let time_limit = pass_budget.fanout_limit_for_pass(pass_no);
                // :235.
                let net_number = board
                    .get_item(*current_pin)
                    .map_or(0, |item| item.get_net_number(0));
                // :238-259.
                if !fanout_pin_can_use_vias(board, self.settings, net_number) {
                    pins_to_go -= 1; // :256
                    continue; // :257
                }

                // :279.
                board.start_marking_changed_area();
                // :281-283. `retainAutorouteDatabase` is permanently `false` (ruling AJ), so the
                // engine is per call here as it is in `AutoroutePassRunner`. The `Stoppable` Java
                // hands down is `this.thread`, whose `isStopRequested()` is the **`ALL`** state.
                let mut engine = None;
                let current_result = board.fanout(
                    &mut engine,
                    *current_pin,
                    self.settings,
                    effective_ripup_costs,
                    &|| stop.is_stop_requested(),
                    Some(time_limit),
                    budget,
                );

                // :286-381 — the five-way switch. `ALREADY_CONNECTED` and the two silent arms do
                // **not** count towards `totalItemsFanouted`, so a pass over an already-escaped
                // board cannot exhaust `maxItems`.
                match current_result.state {
                    // :287-303.
                    AutorouteAttemptState::Routed => {
                        routed_count += 1;
                        self.total_items_fanouted += 1;
                    }
                    // :304-320 — `alreadyConnectedCount++`, an `FRLogger` payload only.
                    AutorouteAttemptState::AlreadyConnected => {}
                    // :321-340.
                    AutorouteAttemptState::Failed => {
                        not_routed_count += 1;
                        self.total_items_fanouted += 1;
                    }
                    // :341-357.
                    AutorouteAttemptState::InsertError => {
                        insert_error_count += 1;
                        self.total_items_fanouted += 1;
                    }
                    // :358-367 `NO_UNCONNECTED_NETS` and `:368-380` the default arm: both are
                    // `FRLogger.trace` and nothing else.
                    _ => {}
                }

                // :382.
                pins_to_go -= 1;
                // :383.
                let extra_vias_this_pass = extra_vias(board, vias_before_pass);
                // :384-395.
                self.maybe_publish_progress(
                    progress,
                    board,
                    pass_no,
                    ripup_costs,
                    pins_to_go,
                    routed_count,
                    not_routed_count,
                    insert_error_count,
                    extra_vias_this_pass,
                    false,
                    pass_start,
                    &progress_stats,
                );
                // :396-415 — the per-stage deadline again, mid-pass.
                if self.is_deadline_reached() {
                    self.is_timed_out = true; // :398
                    let pass_stats = BoardStatistics::with_options(board, None, false); // :399
                    let escape_stats = EscapeStatistics::from_board_statistics(&pass_stats); // :400
                    self.publish_progress(
                        progress,
                        pass_no,
                        ripup_costs,
                        pins_to_go,
                        routed_count,
                        not_routed_count,
                        insert_error_count,
                        extra_vias_this_pass,
                        escape_stats,
                        true,
                        pass_start,
                        &pass_stats,
                    );
                    return Ok(routed_count); // :414
                }
                // :416-433 — the job stop flag, `!= NONE`.
                if stop.is_stop_auto_router_requested() {
                    let pass_stats = BoardStatistics::with_options(board, None, false); // :417
                    let escape_stats = EscapeStatistics::from_board_statistics(&pass_stats);
                    self.publish_progress(
                        progress,
                        pass_no,
                        ripup_costs,
                        pins_to_go,
                        routed_count,
                        not_routed_count,
                        insert_error_count,
                        extra_vias_this_pass,
                        escape_stats,
                        true,
                        pass_start,
                        &pass_stats,
                    );
                    return Ok(routed_count); // :432
                }
            }
            // :435-437.
            if max_limit_reached {
                break;
            }
        }

        // :439.
        let extra_vias_this_pass = extra_vias(board, vias_before_pass);
        // :440.
        self.extra_vias_total += extra_vias_this_pass;
        // :441-442.
        let pass_stats = BoardStatistics::with_options(board, None, false);
        let escape_stats = EscapeStatistics::from_board_statistics(&pass_stats);
        // :444-489 are the pass-end log payloads.
        // :490.
        self.last_not_routed_count = not_routed_count;
        // :491-503.
        self.publish_progress(
            progress,
            pass_no,
            ripup_costs,
            pins_to_go,
            routed_count,
            not_routed_count,
            insert_error_count,
            extra_vias_this_pass,
            escape_stats,
            true,
            pass_start,
            &pass_stats,
        );
        // :505.
        Ok(routed_count)
    }

    /// Port of `BatchFanout.maybePublishProgress` (BatchFanout.java:508-542) — the throttled
    /// per-pin tick.
    ///
    /// The gate is `passCompleted || progressThrottler.shouldUpdate()` (`:520`), so a completed
    /// pass always publishes and a mid-pass tick publishes at most once per
    /// [`RouterBudget::progress_throttle_ms`]. The interim escape statistics are the same
    /// `(totalSmdPins, 0, 0.0)` placeholder `fanoutPass` opens with — "to avoid the cost of a
    /// full escape scan on every tick" (`:521-522`).
    ///
    /// **`progressStats` is mutated in place** at `:525-527`: Java overwrites the pass-start
    /// snapshot's via and trace counts with the board's current ones and leaves every other field
    /// stale. The port copies rather than mutating the caller's snapshot, because the copy is what
    /// the event carries and Java's next tick overwrites the same two fields again — the two are
    /// distinguishable only by a listener that keeps the object, which
    /// [`RoutingEvent`](super::RoutingEvent) cannot.
    #[allow(clippy::too_many_arguments)] // Java's eleven-parameter private method, kept.
    fn maybe_publish_progress(
        &mut self,
        progress: &mut dyn ProgressSink,
        board: &Board,
        pass_no: i32,
        ripup_costs: i32,
        pins_to_go: i32,
        routed_count: i32,
        not_routed_count: i32,
        insert_error_count: i32,
        extra_vias_this_pass: i32,
        pass_completed: bool,
        pass_start: Instant,
        progress_stats: &BoardStatistics,
    ) {
        // :520.
        if !(pass_completed || self.progress_throttler.should_update()) {
            return;
        }
        // :523.
        let interim_escape = EscapeStatistics {
            total_smd_pins: self.total_smd_pin_count,
            escaped_count: 0,
            escaped_percentage: 0.0,
        };
        // :524-527.
        let mut stats = progress_stats.clone();
        stats.vias.total_count = Some(i32::try_from(board.get_vias().len()).unwrap_or(i32::MAX));
        stats.traces.total_count =
            Some(i32::try_from(board.get_traces().len()).unwrap_or(i32::MAX));
        // :528-540.
        self.publish_progress(
            progress,
            pass_no,
            ripup_costs,
            pins_to_go,
            routed_count,
            not_routed_count,
            insert_error_count,
            extra_vias_this_pass,
            interim_escape,
            pass_completed,
            pass_start,
            &stats,
        );
    }

    /// Port of `BatchFanout.publishProgress` (BatchFanout.java:544-576) — fills a
    /// [`FanoutPassStatus`] and hands it to the listener.
    ///
    /// `:555-556`'s `if (progressListener == null) return;` has no counterpart: the port's sink
    /// is always present and [`NoopProgressSink`](super::NoopProgressSink) is Java's empty
    /// listener list. Everything below it is unconditional in Java too.
    ///
    /// Ruling AK reduces the record to [`RoutingEvent::FanoutProgress`](super::RoutingEvent),
    /// three of Java's thirteen constructor arguments — the numbers a headless caller can act on.
    /// The whole [`FanoutPassStatus`] is still built, so that the mapping is visible at the one
    /// place it happens and so that a later task that needs a fourth number widens the variant
    /// rather than opening a second channel (ruling 11).
    #[allow(clippy::too_many_arguments)] // Java's twelve-parameter private method, kept.
    fn publish_progress(
        &mut self,
        progress: &mut dyn ProgressSink,
        pass_no: i32,
        ripup_costs: i32,
        pins_to_go: i32,
        routed_count: i32,
        not_routed_count: i32,
        insert_error_count: i32,
        extra_vias_this_pass: i32,
        escape_statistics: EscapeStatistics,
        pass_completed: bool,
        pass_start: Instant,
        board_statistics: &BoardStatistics,
    ) {
        // :558.
        let duration = i64::try_from(pass_start.elapsed().as_millis()).unwrap_or(i64::MAX);
        // :559-575.
        let status = FanoutPassStatus {
            pass_no: pass_no + 1, // :561 — the record's `passNo` is 1-based
            ripup_costs,
            total_pins: self.total_smd_pin_count,
            pins_to_go,
            routed_count,
            not_routed_count,
            insert_error_count,
            extra_vias_this_pass,
            // :568 — `this.extraViasTotal + extraViasThisPass`, i.e. the field is *not* updated
            // here; `fanoutPass:440` does that once per pass.
            extra_vias_total: self.extra_vias_total + extra_vias_this_pass,
            pass_duration_millis: duration,
            board_statistics: Box::new(board_statistics.clone()),
            pass_completed,
            escape_statistics,
        };
        progress.on_event(&RoutingEvent::FanoutProgress {
            pass: status.pass_no,
            routed: status.routed_count,
            pins_to_go: status.pins_to_go,
        });
    }
}

/// `Math.max(0, this.routingBoard.getVias().size() - viasBeforePass)` (BatchFanout.java:383,
/// `:439`) — a `max(0, …)` because a pass that rips more vias than it places would otherwise
/// report a negative "extra".
fn extra_vias(board: &Board, vias_before_pass: usize) -> i32 {
    let vias_now = board.get_vias().len();
    i32::try_from(vias_now.saturating_sub(vias_before_pass)).unwrap_or(i32::MAX)
}

/// `fanoutStart + timeoutSeconds * 1000` (BatchFanout.java:98) on a monotonic clock, with Java's
/// negative timeouts kept: a negative `timeoutString` puts the deadline *before* the start, and
/// the stage times out before its first pass.
///
/// The two ends saturate in **opposite** directions, and deliberately, because [`Instant`] is
/// bounded at both while Java's epoch `long` is not:
///
/// * a positive offset too large to represent answers `None`, i.e. *no* deadline. NOTE (Task 12
///   review SF3): this is a deliberate divergence at the unreachable extreme — Java's raw `long`
///   `fanoutStart + timeoutSeconds * 1000` WRAPS near `i64::MAX`, which can land the deadline in
///   the past (an IMMEDIATE pass-1 timeout), the OPPOSITE observable of the port's "no deadline".
///   Unreachable by any human-typed `timeoutString` (~292-million-year threshold), so recorded
///   here rather than as a register row; below that threshold the two behaviours agree;
/// * a negative offset too large to represent answers `Some(start)`, i.e. a deadline **already
///   past**, because `Instant::now() >= fanout_start` holds by the first check. Answering `None`
///   there would turn "time out before pass 1" into "never time out", which is the opposite of
///   what Java does with the same string.
pub(crate) fn instant_offset_ms(start: Instant, offset_ms: i64) -> Option<Instant> {
    let magnitude = Duration::from_millis(offset_ms.unsigned_abs());
    if offset_ms >= 0 {
        start.checked_add(magnitude)
    } else {
        Some(start.checked_sub(magnitude).unwrap_or(start))
    }
}
