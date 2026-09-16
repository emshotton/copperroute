use copper_geometry::IntBox;
use std::collections::{BTreeMap, VecDeque};

#[derive(Clone, Debug, PartialEq)]
struct Barrier {
    area: IntBox,
    layer: usize,
}

fn conflict_search<P: Clone>(
    initial: Vec<P>,
    mut replan: impl FnMut(usize, &[Barrier], &[P]) -> Option<P>,
    mut conflict: impl FnMut(&[P]) -> Option<(usize, usize, Barrier)>,
    limit: usize,
) -> Option<Vec<P>> {
    let mut queue = VecDeque::from([(initial, BTreeMap::<usize, Vec<Barrier>>::new())]);
    for _ in 0..limit {
        let (plans, constraints) = queue.pop_front()?;
        let Some((a, b, barrier)) = conflict(&plans) else {
            return Some(plans);
        };
        for net in [a, b] {
            let mut child_constraints = constraints.clone();
            let exclusions = child_constraints.entry(net).or_default();
            if exclusions.contains(&barrier) {
                continue;
            }
            exclusions.push(barrier.clone());
            if let Some(plan) = replan(net, exclusions, &plans) {
                let mut child = plans.clone();
                child[net] = plan;
                queue.push_back((child, child_constraints));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    fn barrier() -> Barrier {
        Barrier {
            area: rect(0, 0, 10, 10),
            layer: 0,
        }
    }

    #[test]
    fn conflict_search_explores_both_owners_of_a_contested_passage() {
        let solution = conflict_search(
            vec![0, 0],
            |net, constraints, _| {
                assert_eq!(constraints, &[barrier()]);
                (net == 1).then_some(1)
            },
            |paths| (paths[0] == paths[1]).then_some((0, 1, barrier())),
            8,
        );
        assert_eq!(solution, Some(vec![0, 1]));
    }

    #[test]
    fn exhausted_search_never_returns_an_unresolved_conflict() {
        assert_eq!(
            conflict_search(vec![0, 0], |_, _, _| None, |_| Some((0, 1, barrier())), 8),
            None
        );
    }
}

use super::connection_budget::ConnectionBudget;
use super::{BatchAutorouter, RouterBudget, RouterStop};
use crate::score::BoardStatistics;
use copper_board::prelude::*;
use copper_drc::{AirLine, DesignRulesChecker};
use copper_geometry::{Area, FloatPoint, Shape, TileShape, Vector};
use copper_settings::RouterSettings;
use std::collections::BTreeSet;
use web_time::Instant;

#[derive(Clone)]
struct NetPlan {
    net: i32,
    copper: Vec<Item>,
}

struct Planner<'a> {
    base: &'a Board,
    settings: &'a RouterSettings,
    stop: &'a RouterStop,
    budget: RouterBudget,
    pass: i32,
    started: Instant,
}
impl Planner<'_> {
    fn stopped(&self) -> bool {
        self.stop.is_stopped_or_expired() || self.started.elapsed().as_secs_f64() >= 30.0
    }
    fn route(&self, net: i32, exclusions: &[Barrier]) -> Option<NetPlan> {
        if self.stopped() {
            return None;
        }
        let mut board = self.base.clone();
        for barrier in exclusions {
            let id = board.new_item_id();
            board.insert_item(Item::ObstacleArea(ObstacleArea::new(
                ItemHeader::new(id, vec![], 0, 0, FixedState::SystemFixed),
                ObstacleAreaData::new(
                    Area::Shape(Shape::Tile(TileShape::Box(barrier.area))),
                    barrier.layer,
                    Vector::ZERO,
                    0.0,
                    false,
                    None,
                ),
            )));
        }
        let mut settings = self.settings.clone();
        settings.net_filter = Some(BTreeSet::from([net]));
        let router = BatchAutorouter::for_routing_job(&board, &settings, self.budget);
        for attempt in 0..2 {
            let items = router.autoroute_items(&board);
            if items.len() > 24 {
                return None;
            }
            if items.is_empty() {
                break;
            }
            for (item, route_net) in items {
                if self.stopped() {
                    return None;
                }
                let mut engine = None;
                let search_budget = ConnectionBudget::start(&settings);
                router.autoroute_item(
                    &mut board,
                    &mut engine,
                    item,
                    route_net,
                    &mut BTreeSet::new(),
                    &mut BTreeMap::new(),
                    self.pass + attempt,
                    &|| self.stopped(),
                    Some(&search_budget),
                );
            }
        }
        if self.stopped()
            || DesignRulesChecker::incomplete_count_for_nets(&board, &BTreeSet::from([net])) != 0
        {
            return None;
        }
        let copper = board
            .items_in_board_order()
            .into_iter()
            .filter_map(|id| {
                let item = board.get_item(id)?;
                (item.is_routable() && item.net_nos() == [net]).then(|| item.clone())
            })
            .collect();
        Some(NetPlan { net, copper })
    }
    fn assemble(&self, plans: &[NetPlan]) -> Board {
        let mut board = self.base.clone();
        for plan in plans {
            for item in &plan.copper {
                let id = board.new_item_id();
                board.insert_item(item.copy(id).expect("trace and via copies always succeed"));
            }
        }
        board
    }
    fn conflict(&self, plans: &[NetPlan]) -> Option<(usize, usize, Barrier)> {
        let mut board = self.assemble(plans);
        let indices: BTreeMap<_, _> = plans.iter().enumerate().map(|(i, p)| (p.net, i)).collect();
        for violation in board.aggregate_violations_sorted_by_severity() {
            let a = board.get_item(violation.first_item)?;
            let b = board.get_item(violation.second_item)?;
            let Some(&ia) = a.net_nos().first().and_then(|n| indices.get(n)) else {
                continue;
            };
            let Some(&ib) = b.net_nos().first().and_then(|n| indices.get(n)) else {
                continue;
            };
            if ia == ib {
                continue;
            }
            let bounds = violation.shape.bounding_box();
            let x = (i64::from(bounds.ll.x) + i64::from(bounds.ur.x)) / 2;
            let y = (i64::from(bounds.ll.y) + i64::from(bounds.ur.y)) / 2;
            let radius = (crate::autoroute::maze::control::board_units_per_mm(&board) * 0.05)
                .ceil()
                .max(1.0) as i32;
            return Some((
                ia,
                ib,
                Barrier {
                    area: rect(
                        x as i32 - radius,
                        y as i32 - radius,
                        x as i32 + radius,
                        y as i32 + radius,
                    ),
                    layer: violation.layer,
                },
            ));
        }
        None
    }
}

fn quality(board: &mut Board) -> (i32, i32) {
    let s = BoardStatistics::new(board);
    (
        s.connections.incomplete_count.unwrap(),
        s.clearance_violations.total_count.unwrap(),
    )
}

fn center(area: IntBox) -> FloatPoint {
    FloatPoint::new(
        (f64::from(area.ll.x) + f64::from(area.ur.x)) * 0.5,
        (f64::from(area.ll.y) + f64::from(area.ur.y)) * 0.5,
    )
}

fn distance_to_segment(p: FloatPoint, a: FloatPoint, b: FloatPoint) -> f64 {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / (dx * dx + dy * dy).max(1.0)).clamp(0.0, 1.0);
    (p.x - a.x - t * dx).hypot(p.y - a.y - t * dy)
}

pub(crate) fn run(
    board: &mut Board,
    settings: &RouterSettings,
    budget: RouterBudget,
    stop: &RouterStop,
    pass: i32,
) {
    if std::env::var("COPPERROUTE_AFTER_OPTIMIZATION_REPAIR")
        .ok()
        .as_deref()
        != Some("1")
        || pass <= 0
        || stop.is_stopped_or_expired()
    {
        return;
    }
    let mut airlines = DesignRulesChecker::new(board).get_all_airlines();
    airlines.sort_by(|a, b| {
        a.from_corner
            .distance(&a.to_corner)
            .total_cmp(&b.from_corner.distance(&b.to_corner))
    });
    let mut tried = BTreeSet::new();
    let mut groups = 0;
    for airline in airlines {
        let net = airline.net_number;
        if tried.contains(&net)
            || settings
                .net_filter
                .as_ref()
                .is_some_and(|f| !f.contains(&net))
            || board
                .rules
                .nets
                .get(net)
                .is_some_and(|n| n.contains_plane())
        {
            continue;
        }
        let pins = board.get_pins();
        let root_pins = pins
            .iter()
            .filter(|&&id| board.get_item(id).is_some_and(|i| i.contains_net(net)))
            .count();
        if !(2..=16).contains(&root_pins) {
            continue;
        }
        if groups >= 3 || stop.is_stopped_or_expired() {
            break;
        }
        let group = select_group(board, &airline, settings);
        if group.len() < 2 {
            continue;
        }
        groups += 1;
        tried.extend(group.iter().copied());
        let before = quality(board);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            solve_group(board, settings, budget, stop, pass, &group)
        }));
        if let Ok(Some(mut candidate)) = result {
            let after = quality(&mut candidate);
            if after.0 < before.0 && after.1 <= before.1 {
                *board = candidate;
            }
        }
    }
}

fn select_group(board: &Board, airline: &AirLine, settings: &RouterSettings) -> Vec<i32> {
    let mut distances = BTreeMap::<i32, f64>::new();
    let ctx = board.ctx();
    let length = airline.from_corner.distance(&airline.to_corner);
    for id in board.items_in_board_order() {
        let item = board.get_item(id).unwrap();
        if !item.is_routable() || item.net_count() != 1 {
            continue;
        }
        let net = item.get_net_number(0);
        if net == airline.net_number
            || board
                .rules
                .nets
                .get(net)
                .is_some_and(|n| n.contains_plane())
            || settings
                .net_filter
                .as_ref()
                .is_some_and(|f| !f.contains(&net))
        {
            continue;
        }
        let distance = distance_to_segment(
            center(item.bounding_box(&ctx)),
            airline.from_corner,
            airline.to_corner,
        );
        if distance > length * 0.25 {
            continue;
        }
        distances
            .entry(net)
            .and_modify(|d| *d = d.min(distance))
            .or_insert(distance);
    }
    let mut distances: Vec<_> = distances.into_iter().collect();
    distances.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    let mut group = vec![airline.net_number];
    for (net, _) in distances {
        let pins = board
            .get_pins()
            .into_iter()
            .filter(|&id| board.get_item(id).is_some_and(|i| i.contains_net(net)))
            .count();
        if (2..=16).contains(&pins) {
            group.push(net);
        }
        if group.len() == 3 {
            break;
        }
    }
    group
}

fn solve_group(
    board: &Board,
    settings: &RouterSettings,
    budget: RouterBudget,
    stop: &RouterStop,
    pass: i32,
    group: &[i32],
) -> Option<Board> {
    let mut base = board.clone();
    let ids: Vec<_> = base
        .items_in_board_order()
        .into_iter()
        .filter(|&id| {
            let item = base.get_item(id).unwrap();
            item.is_routable() && item.net_count() == 1 && group.contains(&item.get_net_number(0))
        })
        .collect();
    if !base.remove_items(ids) {
        return None;
    }
    let fixed: Vec<_> = base
        .items_in_board_order()
        .into_iter()
        .filter_map(|id| {
            let item = base.get_item(id)?;
            item.is_routable().then_some((id, item.get_fixed_state()))
        })
        .collect();
    for &(id, _) in &fixed {
        base.get_item_mut(id)?
            .set_fixed_state(FixedState::UserFixed);
    }
    let planner = Planner {
        base: &base,
        settings,
        stop,
        budget,
        pass,
        started: Instant::now(),
    };
    let mut initial = Vec::new();
    for &net in group {
        let Some(plan) = planner.route(net, &[]) else {
            return None;
        };
        initial.push(plan);
    }
    let plans = conflict_search(
        initial,
        |i, barriers, _| planner.route(group[i], barriers),
        |p| planner.conflict(p),
        8,
    );
    let mut result = planner.assemble(&plans?);
    for (id, state) in fixed {
        result.get_item_mut(id)?.set_fixed_state(state);
    }
    if DesignRulesChecker::incomplete_count_for_nets(&result, &group.iter().copied().collect()) != 0
    {
        return None;
    }
    Some(result)
}

fn rect(x0: i32, y0: i32, x1: i32, y1: i32) -> IntBox {
    IntBox {
        ll: copper_geometry::IntPoint::new(x0, y0),
        ur: copper_geometry::IntPoint::new(x1, y1),
    }
}

#[cfg(test)]
mod geometry_tests {
    use super::*;
    use copper_geometry::{Point, Polyline};
    use copper_settings::sources::DefaultSettings;
    use copper_settings::{HostEnvironment, SettingsSource};
    fn board() -> Board {
        let layers = LayerStructure::new(vec![Layer::new("front", true)]);
        let rules = BoardRules::new(
            layers.clone(),
            ClearanceMatrix::get_default_instance(&layers, 20),
        );
        Board::new(
            vec![],
            0,
            rect(-1000, -1000, 1000, 1000),
            rules,
            BoardLibrary::new(Padstacks::new(layers), Packages::new()),
            Components::new(),
            Communication::default(),
        )
    }
    fn trace(board: &mut Board, net: i32, from: Point, to: Point) -> Item {
        let id = board.new_item_id();
        Item::Trace(PolylineTrace::new(
            ItemHeader::new(id, vec![net], 0, 0, FixedState::Unfixed),
            Polyline::from_points(&[from, to]),
            0,
            10,
            Some(1),
        ))
    }
    #[test]
    fn independent_routes_get_unique_ids_and_real_crossings_are_detected() {
        let mut board = board();
        let a = trace(&mut board, 1, Point::new(-500, 0), Point::new(500, 0));
        let b = trace(&mut board, 2, Point::new(0, -500), Point::new(0, 500));
        let plans = vec![
            NetPlan {
                net: 1,
                copper: vec![a],
            },
            NetPlan {
                net: 2,
                copper: vec![b],
            },
        ];
        let settings = DefaultSettings::new(&HostEnvironment::detect())
            .get_settings()
            .unwrap()
            .clone();
        let stop = RouterStop::new();
        let planner = Planner {
            base: &board,
            settings: &settings,
            stop: &stop,
            budget: RouterBudget::disabled(),
            pass: 3,
            started: Instant::now(),
        };
        let assembled = planner.assemble(&plans);
        assert_eq!(
            assembled.items_in_board_order().len(),
            board.items_in_board_order().len() + 2
        );
        assert!(
            board
                .items_in_board_order()
                .into_iter()
                .all(|id| !matches!(board.get_item(id), Some(Item::Trace(_))))
        );
        let (a, b, barrier) = planner
            .conflict(&plans)
            .expect("crossing copper must conflict");
        assert_ne!(a, b);
        assert_eq!(barrier.layer, 0);
        assert!(barrier.area.ll.x <= 0 && barrier.area.ur.x >= 0);
    }
    #[test]
    fn detailed_planning_routes_around_a_cut_without_exporting_it() {
        let mut board = board();
        let class = board.rules.get_default_net_class();
        board.rules.nets.add("signal", 1, false, class);
        board
            .rules
            .net_classes
            .get_mut(class)
            .set_via_rule(Some(ViaRule::new("empty")));
        board.rules.set_default_trace_half_width(0, 10);
        for (a, b) in [
            (Point::new(-600, 0), Point::new(-500, 0)),
            (Point::new(500, 0), Point::new(600, 0)),
        ] {
            let mut item = trace(&mut board, 1, a, b);
            item.set_fixed_state(FixedState::UserFixed);
            board.insert_item(item);
        }
        let mut settings = DefaultSettings::new(&HostEnvironment::detect())
            .get_settings()
            .unwrap()
            .clone();
        settings.set_layer_count(1);
        settings.apply_board_specific_optimizations(&board);
        let stop = RouterStop::new();
        let planner = Planner {
            base: &board,
            settings: &settings,
            stop: &stop,
            budget: RouterBudget::disabled(),
            pass: 3,
            started: Instant::now(),
        };
        let plan = planner
            .route(
                1,
                &[Barrier {
                    area: rect(-20, -1000, 20, 100),
                    layer: 0,
                }],
            )
            .expect("a route exists above the artificial cut");
        let result = planner.assemble(&[plan]);
        assert_eq!(
            DesignRulesChecker::incomplete_count_for_nets(&result, &BTreeSet::from([1])),
            0
        );
        assert!(
            result
                .items_in_board_order()
                .into_iter()
                .all(|id| !matches!(result.get_item(id), Some(Item::ObstacleArea(_))))
        );
        let fixed_before: Vec<_> = board
            .items_in_board_order()
            .into_iter()
            .map(|id| (id, board.get_item(id).unwrap().get_fixed_state()))
            .collect();
        for (id, state) in fixed_before {
            assert_eq!(result.get_item(id).unwrap().get_fixed_state(), state);
        }
    }
}
