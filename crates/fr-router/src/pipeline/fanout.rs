use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use fr_board::items::Item;
use fr_board::structure::Unit;
use fr_board::{Board, BoardError, ItemId};
use fr_geometry::FloatPoint;
use fr_settings::RouterSettings;

use crate::JavaTreeSet;
use crate::autoroute::AutorouteAttemptState;
use crate::board_ext::RoutingBoardExt;
use crate::error::RouterError;
use crate::score::{BoardStatistics, BoardStatisticsFanout};

use super::{ProgressSink, ProgressThrottler, RouterBudget, RouterStop, RoutingEvent};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EscapeStatistics {
    pub total_smd_pins: i32,
    pub escaped_count: i32,
    pub escaped_percentage: f64,
}

impl EscapeStatistics {
    #[must_use]
    pub fn from_board_statistics(stats: &BoardStatistics) -> EscapeStatistics {
        EscapeStatistics::from_fanout_statistics(&stats.fanout)
    }

    #[must_use]
    pub fn from_fanout_statistics(fanout: &BoardStatisticsFanout) -> EscapeStatistics {
        let percentage = if fanout.total_smd_pins > 0 {
            f64::from(fanout.escaped_count) * 100.0 / f64::from(fanout.total_smd_pins)
        } else {
            0.0
        };
        EscapeStatistics {
            total_smd_pins: fanout.total_smd_pins,
            escaped_count: fanout.escaped_count,
            escaped_percentage: percentage,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FanoutPassStatus {
    pub pass_no: i32,
    pub ripup_costs: i32,
    pub total_pins: i32,
    pub pins_to_go: i32,
    pub routed_count: i32,
    pub not_routed_count: i32,
    pub insert_error_count: i32,
    pub extra_vias_this_pass: i32,
    pub extra_vias_total: i32,
    pub pass_duration_millis: i64,
    pub board_statistics: Box<BoardStatistics>,
    pub pass_completed: bool,
    pub escape_statistics: EscapeStatistics,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FanoutRunSummary {
    pub completed_pass_count: i32,
    pub total_duration_millis: i64,
    pub escape_statistics: EscapeStatistics,
    pub is_timed_out: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FanoutPin {
    pub pin: ItemId,
    pub pin_index: i32,
    pub distance_to_component_center: f64,
    pub distance_to_closest_on_net: Option<f64>,
    pub surroundings_density: i32,
}

impl FanoutPin {
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
        let pin_location = pin.get_center(&ctx).to_float();
        let distance_to_component_center = pin_location.distance(gravity_center);

        let mut min_distance = None;
        let net_number = if pin_item.net_count() > 0 {
            pin_item.get_net_number(0)
        } else {
            0
        };
        if net_number > 0 {
            for other in board.get_pins() {
                let other_item = board.get_item(other).expect("a board pin");
                if other != board_pin && other_item.contains_net(net_number) {
                    let Item::Pin(other_pin) = other_item else {
                        continue;
                    };
                    let dist = pin_location.distance(&other_pin.get_center(&ctx).to_float());
                    if min_distance.is_none_or(|current| dist < current) {
                        min_distance = Some(dist);
                    }
                }
            }
        }
        let distance_to_closest_on_net = min_distance;

        let resolution = board.communication.get_resolution(Unit::Um);
        let max_dist = 20_000.0 * resolution;
        let mut density = 0_i32;
        for other in board_smd_pin_list {
            if *other == board_pin {
                continue;
            }
            let Some(Item::Pin(other_pin)) = board.get_item(*other) else {
                continue;
            };
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

    #[must_use]
    pub fn compare_to(&self, other: &FanoutPin, pin_sorting_order: Option<&str>) -> i32 {
        let mut result = 0_i32;
        let order = pin_sorting_order.unwrap_or("");
        if order == "inner_first" {
            let delta_dist = self.distance_to_component_center - other.distance_to_component_center;
            if delta_dist > 0.0 {
                result = 1;
            } else if delta_dist < 0.0 {
                result = -1;
            }
        } else if order == "outer_first" {
            let delta_dist = self.distance_to_component_center - other.distance_to_component_center;
            if delta_dist > 0.0 {
                result = -1;
            } else if delta_dist < 0.0 {
                result = 1;
            }
        } else if order == "distanceToClosestOnNet" {
            result = match (
                self.distance_to_closest_on_net,
                other.distance_to_closest_on_net,
            ) {
                (Some(left), Some(right)) => match left.total_cmp(&right) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                },
                (Some(_), None) => -1,
                (None, Some(_)) => 1,
                (None, None) => 0,
            };
        } else if order == "surroundingsDensity" {
            let delta = other.surroundings_density - self.surroundings_density;
            if delta > 0 {
                result = 1;
            } else if delta < 0 {
                result = -1;
            }
        }
        if result == 0 {
            result = self.pin_index - other.pin_index;
        }
        result
    }
}

#[derive(Debug, Clone)]
pub struct FanoutComponent {
    pub component: i32,
    pub component_name: String,
    pub smd_pins: JavaTreeSet<FanoutPin>,
    pub gravity_center_of_smd_pins: FloatPoint,
    pub smd_pin_count: i32,
    pub pin_sorting_order: Option<String>,
}

impl FanoutComponent {
    #[must_use]
    pub fn new(
        board: &Board,
        board_component: i32,
        board_smd_pin_list: &[ItemId],
        pin_sorting_order: Option<&str>,
    ) -> FanoutComponent {
        let component_id = board.components.get(board_component).id;
        let component_name = board.components.get(board_component).name.clone();

        let current_pin_list: Vec<ItemId> = board_smd_pin_list
            .iter()
            .copied()
            .filter(|pin| {
                board
                    .get_item(*pin)
                    .is_some_and(|item| item.component_id() == component_id)
            })
            .collect();

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

    #[must_use]
    pub fn compare_to(&self, other: &FanoutComponent) -> std::cmp::Ordering {
        let compare_value = self.smd_pin_count - other.smd_pin_count;
        let result = if compare_value > 0 {
            -1
        } else if compare_value < 0 {
            1
        } else {
            self.component - other.component
        };
        result.cmp(&0)
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

impl Ord for FanoutComponent {
    fn cmp(&self, other: &FanoutComponent) -> std::cmp::Ordering {
        self.compare_to(other)
    }
}

#[derive(Debug)]
pub struct BatchFanout<'a> {
    pub sorted_components: BTreeSet<FanoutComponent>,
    pub settings: &'a RouterSettings,
    pub total_smd_pin_count: i32,
    pub already_connected_pin_count: i32,
    pub progress_throttler: ProgressThrottler,
    pub last_not_routed_count: i32,
    pub extra_vias_total: i32,
    pub total_items_fanouted: i32,
    pub deadline: Option<Instant>,
    pub is_timed_out: bool,
}

impl<'a> BatchFanout<'a> {
    #[must_use]
    pub fn new(board: &Board, settings: &'a RouterSettings) -> BatchFanout<'a> {
        let sorting_order: &str = settings
            .fanout
            .as_ref()
            .and_then(|f| f.pin_sorting_order.as_deref())
            .unwrap_or("outer_first");

        let board_smd_pin_list = board.get_smd_pins();
        let board_smd_pin_list_with_nets: Vec<ItemId> = board_smd_pin_list
            .into_iter()
            .filter(|pin| {
                board
                    .get_item(*pin)
                    .is_some_and(|item| item.net_count() > 0)
            })
            .collect();

        let mut sorted_components: BTreeSet<FanoutComponent> = BTreeSet::new();
        for i in 1..=i32::try_from(board.components.count()).expect("a board component count") {
            let current_component =
                FanoutComponent::new(board, i, &board_smd_pin_list_with_nets, Some(sorting_order));
            if current_component.smd_pin_count > 0 {
                sorted_components.insert(current_component);
            }
        }

        let mut pin_count = 0_i32;
        let mut already_connected = 0_i32;
        for component in &sorted_components {
            pin_count += component.smd_pin_count;
            for pin in &component.smd_pins {
                let Some(item) = board.get_item(pin.pin) else {
                    continue;
                };
                let net_number = item.get_net_number(0);
                if board.unconnected_set(pin.pin, net_number).is_empty() {
                    already_connected += 1;
                }
            }
        }

        BatchFanout {
            sorted_components,
            settings,
            total_smd_pin_count: pin_count,
            already_connected_pin_count: already_connected,
            progress_throttler: ProgressThrottler::new(1000),
            last_not_routed_count: 0,
            extra_vias_total: 0,
            total_items_fanouted: 0,
            deadline: None,
            is_timed_out: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanoutStop {
    NothingRouted,
    UnchangedHash,
    TimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FanoutLoopState {
    pub last_board_hash: u64,
}

impl FanoutLoopState {
    #[must_use]
    pub fn new(initial_board_hash: u64) -> FanoutLoopState {
        FanoutLoopState {
            last_board_hash: initial_board_hash,
        }
    }

    pub fn after_pass<F: FnOnce() -> u64>(
        &mut self,
        routed_count: i32,
        is_timed_out: bool,
        board_hash: F,
    ) -> Option<FanoutStop> {
        if routed_count == 0 {
            return Some(FanoutStop::NothingRouted);
        }
        if is_timed_out {
            return Some(FanoutStop::TimedOut);
        }
        let current_board_hash = board_hash();
        if current_board_hash == self.last_board_hash {
            return Some(FanoutStop::UnchangedHash);
        }
        self.last_board_hash = current_board_hash;
        None
    }
}

#[must_use]
pub fn fanout_ripup_costs(settings: &RouterSettings, pass_no: i32) -> i32 {
    let ripup_costs = settings
        .get_start_ripup_costs()
        .wrapping_mul(pass_no.wrapping_add(1));
    let ripup_allowed = settings
        .fanout
        .as_ref()
        .and_then(|fanout| fanout.ripup_allowed)
        .unwrap_or(true);
    if ripup_allowed { ripup_costs } else { -1 }
}

#[must_use]
pub fn fanout_pin_can_use_vias(board: &Board, settings: &RouterSettings, net_number: i32) -> bool {
    let Some(net) = board.rules.nets.get(net_number) else {
        return false;
    };
    let via_count = board
        .rules
        .net_classes
        .get(net.get_net_class())
        .get_via_rule()
        .map_or(0, fr_board::ViaRule::via_count);
    let has_board_vias =
        !board.rules.via_rules.is_empty() && board.rules.via_rules[0].via_count() > 0;
    let fallback_allowed = settings
        .fanout
        .as_ref()
        .and_then(|fanout| fanout.fallback_to_board_vias)
        .unwrap_or(false)
        && has_board_vias;
    via_count > 0 || fallback_allowed
}

pub fn parse_timespan_seconds_java(timespan_string: &str) -> Option<i64> {
    if timespan_string.trim().is_empty() {
        return None;
    }
    let parts = split_java(timespan_string, ':');
    let (seconds, fraction) = match parts.as_slice() {
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
        [minutes, seconds] => {
            let (whole, fraction) = duration_seconds_component(seconds)?;
            (
                duration_integer_component(minutes)?
                    .checked_mul(60)?
                    .checked_add(whole)?,
                fraction,
            )
        }
        [seconds] => duration_seconds_component(seconds)?,
        _ => return None,
    };
    if fraction == Fraction::Negative {
        seconds.checked_sub(1)
    } else {
        Some(seconds)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "`{input}` is not a timespan. Accepted: a plain number of seconds (`300`), the colon forms \
     `mm:ss` and `hh:mm:ss` (`5:00`, `0:05:00`), or unit suffixes (`5m`, `300s`, `1h30m`). Leave \
     the setting empty for no timeout — an unreadable one is refused rather than silently ignored \
     (quirk #224)."
)]
pub struct TimespanError {
    pub input: String,
}

pub fn parse_timespan_seconds(timespan_string: &str) -> Result<Option<i64>, TimespanError> {
    if timespan_string.trim().is_empty() {
        return Ok(None);
    }
    let refuse = || TimespanError {
        input: timespan_string.to_string(),
    };
    if timespan_string.contains(':') {
        return parse_timespan_seconds_java(timespan_string)
            .map(Some)
            .ok_or_else(refuse);
    }
    if let Some(seconds) = parse_unit_suffixes(timespan_string) {
        return Ok(Some(seconds));
    }
    parse_timespan_seconds_java(timespan_string)
        .map(Some)
        .ok_or_else(refuse)
}

fn parse_unit_suffixes(text: &str) -> Option<i64> {
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    if digits.is_empty() {
        return None;
    }
    const UNITS: [(char, i64); 3] = [('h', 3600), ('m', 60), ('s', 1)];
    let mut total: i64 = 0;
    let mut rest = digits;
    let mut next_unit = 0;
    let mut matched_any = false;
    while !rest.is_empty() {
        let count_len = rest.chars().take_while(char::is_ascii_digit).count();
        if count_len == 0 || count_len == rest.len() {
            return None;
        }
        let count: i64 = rest[..count_len].parse().ok()?;
        let unit = rest[count_len..].chars().next()?;
        let index = UNITS[next_unit..]
            .iter()
            .position(|(letter, _)| *letter == unit)?
            + next_unit;
        total = total.checked_add(count.checked_mul(UNITS[index].1)?)?;
        next_unit = index + 1;
        matched_any = true;
        rest = &rest[count_len + unit.len_utf8()..];
    }
    if !matched_any {
        return None;
    }
    if negative {
        total.checked_neg()
    } else {
        Some(total)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fraction {
    Zero,
    Positive,
    Negative,
}

fn duration_integer_component(text: &str) -> Option<i64> {
    let digits = text.strip_prefix(['-', '+']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<i64>().ok()
}

fn duration_seconds_component(text: &str) -> Option<(i64, Fraction)> {
    let Some(separator) = text.find(['.', ',']) else {
        return duration_integer_component(text).map(|whole| (whole, Fraction::Zero));
    };
    let (whole_text, fraction_text) = text.split_at(separator);
    let fraction_digits = &fraction_text[1..];
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

impl<'a> BatchFanout<'a> {
    pub fn fanout_board(
        board: &mut Board,
        settings: &'a RouterSettings,
        stop: &RouterStop,
        budget: RouterBudget,
        progress: &mut dyn ProgressSink,
    ) -> Result<FanoutRunSummary, RouterError> {
        let mut fanout_instance = BatchFanout::new(board, settings);
        fanout_instance.progress_throttler = budget.progress_throttler();
        let fanout_start = Instant::now();
        if let Some(timeout_string) = settings
            .fanout
            .as_ref()
            .and_then(|fanout| fanout.timeout_string.as_deref())
        {
            if let Some(timeout_seconds) =
                parse_timespan_seconds(timeout_string).map_err(RouterError::Timespan)?
            {
                fanout_instance.deadline =
                    instant_offset_ms(fanout_start, timeout_seconds.saturating_mul(1000));
            }
        }
        let max_passes = settings
            .fanout
            .as_ref()
            .and_then(|fanout| fanout.max_passes)
            .unwrap_or(20);
        let mut completed_passes = 0_i32;
        let mut loop_state = FanoutLoopState::new(board.structural_hash());

        let mut i = 0_i32;
        while i < max_passes {
            stop.poll_cancel();
            stop.poll_deadline();
            if stop.is_stop_auto_router_requested() {
                fanout_instance.is_timed_out = stop.is_timed_out();
                break;
            }
            if fanout_instance.is_deadline_reached() {
                fanout_instance.is_timed_out = true;
                break;
            }
            if fanout_instance.max_items_reached() {
                break;
            }
            let routed_count = fanout_instance.fanout_pass(board, i, stop, budget, progress)?;
            completed_passes += 1;
            let is_timed_out = fanout_instance.is_timed_out;
            if loop_state
                .after_pass(routed_count, is_timed_out, || board.structural_hash())
                .is_some()
            {
                break;
            }
            i += 1;
        }

        let stats = BoardStatistics::with_options(board, None, false);
        let final_escape = EscapeStatistics::from_board_statistics(&stats);
        let total_duration_millis =
            i64::try_from(fanout_start.elapsed().as_millis()).unwrap_or(i64::MAX);
        Ok(FanoutRunSummary {
            completed_pass_count: completed_passes,
            total_duration_millis,
            escape_statistics: final_escape,
            is_timed_out: fanout_instance.is_timed_out,
        })
    }

    #[must_use]
    pub fn is_deadline_reached(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }

    #[must_use]
    pub fn max_items_reached(&self) -> bool {
        self.settings
            .fanout
            .as_ref()
            .and_then(|fanout| fanout.max_items)
            .is_some_and(|max_items| max_items > 0 && self.total_items_fanouted >= max_items)
    }

    pub fn fanout_pass(
        &mut self,
        board: &mut Board,
        pass_no: i32,
        stop: &RouterStop,
        budget: RouterBudget,
        progress: &mut dyn ProgressSink,
    ) -> Result<i32, RouterError> {
        let pass_start = Instant::now();
        let mut pins_to_go = self.total_smd_pin_count;
        let mut routed_count = 0_i32;
        let mut not_routed_count = 0_i32;
        let mut insert_error_count = 0_i32;
        let vias_before_pass = board.get_vias().len();
        let ripup_costs = self
            .settings
            .get_start_ripup_costs()
            .wrapping_mul(pass_no.wrapping_add(1));
        let effective_ripup_costs = fanout_ripup_costs(self.settings, pass_no);
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

        self.progress_throttler.reset();
        let progress_stats = BoardStatistics::with_options(board, None, false);
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

        let walk: Vec<Vec<ItemId>> = self
            .sorted_components
            .iter()
            .map(|component| component.smd_pins.iter().map(|pin| pin.pin).collect())
            .collect();
        let mut max_limit_reached = false;
        'pins: for component_pins in &walk {
            for current_pin in component_pins {
                if self.max_items_reached() {
                    max_limit_reached = true;
                    break;
                }
                let time_limit = pass_budget.fanout_limit_for_pass(pass_no);
                let net_number = board
                    .get_item(*current_pin)
                    .map_or(0, |item| item.get_net_number(0));
                if !fanout_pin_can_use_vias(board, self.settings, net_number) {
                    pins_to_go -= 1;
                    continue;
                }

                board.start_marking_changed_area();
                let mut engine = None;
                let current_result = match board.fanout(
                    &mut engine,
                    *current_pin,
                    self.settings,
                    effective_ripup_costs,
                    &|| stop.is_stopped_or_expired(),
                    Some(time_limit),
                    budget,
                ) {
                    Ok(result) => result,
                    Err(BoardError::Stopped) => {
                        self.is_timed_out = stop.is_timed_out();
                        board.changed_area = None;
                        break 'pins;
                    }
                    Err(error) => return Err(error.into()),
                };

                match current_result.state {
                    AutorouteAttemptState::Routed => {
                        routed_count += 1;
                        self.total_items_fanouted += 1;
                    }
                    AutorouteAttemptState::AlreadyConnected => {}
                    AutorouteAttemptState::Failed => {
                        not_routed_count += 1;
                        self.total_items_fanouted += 1;
                    }
                    AutorouteAttemptState::InsertError => {
                        insert_error_count += 1;
                        self.total_items_fanouted += 1;
                    }
                    _ => {}
                }

                pins_to_go -= 1;
                let extra_vias_this_pass = extra_vias(board, vias_before_pass);
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
                if self.is_deadline_reached() {
                    self.is_timed_out = true;
                    let pass_stats = BoardStatistics::with_options(board, None, false);
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
                    return Ok(routed_count);
                }
                if stop.is_stop_auto_router_requested() {
                    self.is_timed_out = stop.is_timed_out();
                    board.changed_area = None;
                    let pass_stats = BoardStatistics::with_options(board, None, false);
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
                    return Ok(routed_count);
                }
            }
            if max_limit_reached {
                break;
            }
        }

        let extra_vias_this_pass = extra_vias(board, vias_before_pass);
        self.extra_vias_total += extra_vias_this_pass;
        let pass_stats = BoardStatistics::with_options(board, None, false);
        let escape_stats = EscapeStatistics::from_board_statistics(&pass_stats);
        self.last_not_routed_count = not_routed_count;
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
        Ok(routed_count)
    }

    #[allow(clippy::too_many_arguments)]
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
        if !(pass_completed || self.progress_throttler.should_update()) {
            return;
        }
        let interim_escape = EscapeStatistics {
            total_smd_pins: self.total_smd_pin_count,
            escaped_count: 0,
            escaped_percentage: 0.0,
        };
        let mut stats = progress_stats.clone();
        stats.vias.total_count = Some(i32::try_from(board.get_vias().len()).unwrap_or(i32::MAX));
        stats.traces.total_count =
            Some(i32::try_from(board.get_traces().len()).unwrap_or(i32::MAX));
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

    #[allow(clippy::too_many_arguments)]
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
        let duration = i64::try_from(pass_start.elapsed().as_millis()).unwrap_or(i64::MAX);
        let status = FanoutPassStatus {
            pass_no: pass_no + 1,
            ripup_costs,
            total_pins: self.total_smd_pin_count,
            pins_to_go,
            routed_count,
            not_routed_count,
            insert_error_count,
            extra_vias_this_pass,
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

fn extra_vias(board: &Board, vias_before_pass: usize) -> i32 {
    let vias_now = board.get_vias().len();
    i32::try_from(vias_now.saturating_sub(vias_before_pass)).unwrap_or(i32::MAX)
}

pub(crate) fn instant_offset_ms(start: Instant, offset_ms: i64) -> Option<Instant> {
    let magnitude = Duration::from_millis(offset_ms.unsigned_abs());
    if offset_ms >= 0 {
        start.checked_add(magnitude)
    } else {
        Some(start.checked_sub(magnitude).unwrap_or(start))
    }
}
