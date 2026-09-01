//! Port of `autoroute.pipeline.BatchFanout` (BatchFanout.java:20-780) — the SMD escape pre-pass
//! that runs before the first autoroute pass.
//!
//! **Task 11 of Plan 7 lands the ordering**: the [`BatchFanout`] type and its constructor
//! (`:35-78`), the [`FanoutComponent`] / [`FanoutPin`] pair the constructor builds (`:631-693`,
//! `:695-778`) and the three result records (`:591-606`, `:609-622`, `:625-629`). Scan ruling 7:
//! the struct is declared by the earliest task that writes methods on it, and **Task 12 adds the
//! `impl` blocks** — `fanoutBoard` (`:81-163`), `fanoutPass` (`:166-...`) and `publishProgress` —
//! and declares nothing new.
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

use fr_board::items::Item;
use fr_board::structure::Unit;
use fr_board::{Board, ItemId};
use fr_geometry::FloatPoint;
use fr_settings::RouterSettings;

use crate::JavaTreeSet;
use crate::score::{BoardStatistics, BoardStatisticsFanout};

use super::ProgressThrottler;

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
    /// flag; see [`BatchFanout::deadline_ms`].
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
    /// `deadlineMs` (`:32`), set by `fanoutBoard` from
    /// `settings.fanout.timeoutString` (`:94-99`) and read at `:111-116` and `:396`.
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
    pub deadline_ms: Option<i64>,
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
            deadline_ms: None,                                // :32, set by Task 12's `fanoutBoard`
            is_timed_out: false,                              // :33
        }
    }
}
