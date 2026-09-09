use std::collections::{BTreeMap, BTreeSet};

use copper_board::items::Item;
use copper_board::rules::BoardRules;
use copper_board::structure::{FixedState, Unit};
use copper_board::{Board, ItemId};
use copper_drc::{BoardStatisticsClearanceViolations, DesignRulesChecker, DrcViolation};

use super::dtos::{
    BoardStatisticsBends, BoardStatisticsBoard, BoardStatisticsComponents,
    BoardStatisticsConnections, BoardStatisticsItems, BoardStatisticsLayers, BoardStatisticsNets,
    BoardStatisticsPads, BoardStatisticsTraces, BoardStatisticsVias, Rectangle2DFloat,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BoardStatisticsFanout {
    pub total_smd_pins: i32,
    pub pins_to_escape: i32,
    pub escaped_count: i32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatistics {
    pub host: String,
    pub unit: String,
    pub board: BoardStatisticsBoard,
    pub layers: BoardStatisticsLayers,
    pub items: BoardStatisticsItems,
    pub components: BoardStatisticsComponents,
    pub pads: BoardStatisticsPads,
    pub nets: BoardStatisticsNets,
    pub connections: BoardStatisticsConnections,
    pub traces: BoardStatisticsTraces,
    pub bends: BoardStatisticsBends,
    pub vias: BoardStatisticsVias,
    pub clearance_violations: BoardStatisticsClearanceViolations,
    pub fanout: BoardStatisticsFanout,
}

impl BoardStatistics {
    pub fn new(board: &mut Board) -> BoardStatistics {
        BoardStatistics::with_options(board, None, true)
    }

    pub fn with_options(
        board: &mut Board,
        unit: Option<Unit>,
        include_clearance_violations: bool,
    ) -> BoardStatistics {
        BoardStatistics::compute(board, unit, include_clearance_violations, true)
    }

    pub fn compute(
        board: &mut Board,
        unit: Option<Unit>,
        include_clearance_violations: bool,
        include_connections: bool,
    ) -> BoardStatistics {
        BoardStatistics::compute_with(
            board,
            unit,
            include_clearance_violations,
            include_connections,
            true,
        )
    }

    /// The counts the router and optimizer decide on: items, traces, vias, bends and open
    /// connections. The fanout census walks every SMD pin's connected set and nothing on the
    /// routing path reads it, so it is left at its default.
    pub fn for_routing_decisions(board: &mut Board) -> BoardStatistics {
        BoardStatistics::compute_with(board, None, false, true, false)
    }

    /// [`BoardStatistics::for_routing_decisions`] with `connections` supplied by the caller
    /// instead of a whole-board incomplete pass.
    pub fn for_routing_decisions_carrying(
        board: &mut Board,
        connections: BoardStatisticsConnections,
    ) -> BoardStatistics {
        let mut stats = BoardStatistics::compute_with(board, None, false, false, false);
        stats.connections = connections;
        stats
    }

    fn compute_with(
        board: &mut Board,
        unit: Option<Unit>,
        include_clearance_violations: bool,
        include_connections: bool,
        include_fanout: bool,
    ) -> BoardStatistics {
        let mut stats = BoardStatistics::default();

        let bb = board.get_bounding_box();

        stats.host = host_of(board);

        stats.unit = board.communication.unit.to_string();

        stats.board.bounding_box = Some(Rectangle2DFloat {
            x: bb.ur.x as f32,
            y: bb.ur.y as f32,
            width: bb.ll.x as f32,
            height: bb.ll.y as f32,
        });
        stats.board.size = Some(Rectangle2DFloat {
            x: 0.0,
            y: 0.0,
            width: (bb.ll.x as f32 - bb.ur.x as f32).abs(),
            height: (bb.ll.y as f32 - bb.ur.y as f32).abs(),
        });

        stats.layers.total_count = Some(board.get_layer_count() as i32);
        stats.layers.signal_count = Some(board.layer_structure().signal_layer_count() as i32);

        let mut total = 0_i32;
        let (mut traces, mut vias, mut conduction, mut pins, mut outlines, mut other) =
            (0_i32, 0_i32, 0_i32, 0_i32, 0_i32, 0_i32);
        for id in board.items_in_board_order() {
            let Some(item) = board.get_item(id) else {
                continue;
            };
            total += 1;
            match item {
                Item::Trace(_) => traces += 1,
                Item::Via(_) => vias += 1,
                Item::ConductionArea(_) => conduction += 1,
                Item::Pin(_) => pins += 1,
                Item::ComponentOutline(_) => outlines += 1,
                _ => other += 1,
            }
        }
        stats.items = BoardStatisticsItems {
            total_count: Some(total),
            trace_count: Some(traces),
            via_count: Some(vias),
            conduction_area_count: Some(conduction),
            drill_item_count: Some(0),
            pin_count: Some(pins),
            component_outline_count: Some(outlines),
            other_count: Some(other),
        };

        stats.components = BoardStatisticsComponents {
            total_count: Some(board.components.count() as i32),
        };

        stats.pads = BoardStatisticsPads {
            total_count: Some(board.get_pins().len() as i32),
        };

        stats.nets = BoardStatisticsNets {
            total_count: Some(board.rules.nets.max_net_number()),
            class_count: Some(board.rules.net_classes.count() as i32),
        };

        let trace_ids = board.get_traces();
        stats.traces.total_count = Some(trace_ids.len() as i32);
        let total_length = (trace_ids.iter().map(|id| match board.get_item(*id) {
            Some(Item::Trace(trace)) => trace.get_length(),
            _ => 0.0,
        }))
        .sum::<f64>() as f32;
        stats.traces.total_length = Some(total_length);

        let resolution = board.communication.resolution;
        let divisor = if resolution > 0 {
            f64::from(resolution)
        } else {
            1.0
        };
        let board_unit_to_mm_factor =
            Unit::scale(1.0, board.communication.unit, Unit::Mm) / divisor;
        let board_unit_to_um_factor =
            Unit::scale(1.0, board.communication.unit, Unit::Um) / divisor;

        stats.traces.total_length_mm =
            Some((f64::from(total_length) * board_unit_to_mm_factor) as f32);
        stats.traces.average_length = Some(if trace_ids.is_empty() {
            0.0
        } else {
            total_length / trace_ids.len() as f32
        });

        let mut total_segment_count = 0_i32;
        let mut total_horizontal_length = 0.0_f32;
        let mut total_vertical_length = 0.0_f32;
        let mut total_angled_length = 0.0_f32;
        for id in &trace_ids {
            let Some(Item::Trace(trace)) = board.get_item(*id) else {
                continue;
            };
            let polyline = trace.polyline();
            let corner_count = polyline.corner_count();
            if corner_count > 1 {
                total_segment_count += (corner_count - 1) as i32;
            }
            for line in polyline.lines() {
                let a = line.a.to_float();
                let b = line.b.to_float();
                let dx = a.x - b.x;
                let dy = a.y - b.y;
                let length = (dx * dx + dy * dy).sqrt() as f32;
                if a.x == b.x {
                    total_vertical_length += length;
                } else if a.y == b.y {
                    total_horizontal_length += length;
                } else {
                    total_angled_length += length;
                }
            }
        }
        stats.traces.total_segment_count = Some(total_segment_count);
        stats.traces.total_horizontal_length = Some(total_horizontal_length);
        stats.traces.total_vertical_length = Some(total_vertical_length);
        stats.traces.total_angled_length = Some(total_angled_length);

        let default_clearance_class = BoardRules::default_clearance_class();
        let mut total_weighted_length = 0.0_f32;
        for id in board.items_in_board_order() {
            let Some(Item::Trace(trace)) = board.get_item(id) else {
                continue;
            };
            let fixed_state = trace.hdr.get_fixed_state();
            if fixed_state != FixedState::Unfixed && fixed_state != FixedState::ShoveFixed {
                continue;
            }
            let clearance = board.clearance_value(
                trace.hdr.clearance_class(),
                default_clearance_class,
                trace.get_layer(),
            );
            let mut weighted = trace.get_length() * f64::from(trace.get_half_width() + clearance);
            if fixed_state == FixedState::ShoveFixed {
                weighted /= 2.0;
            }
            total_weighted_length += weighted as f32;
        }
        stats.traces.total_weighted_length = Some(total_weighted_length);

        if include_connections {
            let mut drc = DesignRulesChecker::new(board);
            drc.calculate_all_incompletes();
            stats.connections = BoardStatisticsConnections {
                maximum_count: Some(drc.max_connections()),
                incomplete_count: Some(drc.get_incomplete_count() as i32),
            };
        }

        let (mut bend_total, mut ninety, mut forty_five, mut other_angle) =
            (0_i32, 0_i32, 0_i32, 0_i32);
        for id in &trace_ids {
            let Some(Item::Trace(trace)) = board.get_item(*id) else {
                continue;
            };
            let polyline = trace.polyline();
            let corner_count = polyline.corner_count();
            if corner_count < 3 {
                continue;
            }
            bend_total += (corner_count - 2) as i32;
            for i in 1..corner_count - 1 {
                let prev = polyline
                    .corner(i - 1)
                    .expect("corner index below cornerCount")
                    .to_float();
                let current = polyline
                    .corner(i)
                    .expect("corner index below cornerCount")
                    .to_float();
                let next = polyline
                    .corner(i + 1)
                    .expect("corner index below cornerCount")
                    .to_float();
                let dx1 = current.x - prev.x;
                let dy1 = current.y - prev.y;
                let dx2 = next.x - current.x;
                let dy2 = next.y - current.y;
                let mut angle = (to_degrees(dy2.atan2(dx2) - dy1.atan2(dx1))).abs();
                angle = (angle).min(360.0 - angle);
                angle = if angle > 180.0 { 360.0 - angle } else { angle };
                if (angle - 90.0).abs() < 1.0 {
                    ninety += 1;
                } else if (angle - 45.0).abs() < 1.0 || (angle - 135.0).abs() < 1.0 {
                    forty_five += 1;
                } else {
                    other_angle += 1;
                }
            }
        }
        stats.bends = BoardStatisticsBends {
            total_count: Some(bend_total),
            ninety_degree_count: Some(ninety),
            forty_five_degree_count: Some(forty_five),
            other_angle_count: Some(other_angle),
        };

        let via_ids = board.get_vias();
        let layer_count = stats.layers.total_count.unwrap_or(0);
        let (mut through, mut blind, mut buried) = (0_i32, 0_i32, 0_i32);
        {
            let ctx = board.ctx();
            for id in &via_ids {
                let Some(item) = board.get_item(*id) else {
                    continue;
                };
                let first = item.first_layer(&ctx) as i32;
                let last = item.last_layer(&ctx) as i32;
                if first == 0 && last == layer_count - 1 {
                    through += 1;
                } else if first == 0 || last == layer_count - 1 {
                    blind += 1;
                } else {
                    buried += 1;
                }
            }
        }
        stats.vias = BoardStatisticsVias {
            total_count: Some(via_ids.len() as i32),
            through_hole_count: Some(through),
            blind_count: Some(blind),
            buried_count: Some(buried),
        };

        stats.clearance_violations = if include_clearance_violations {
            let violations = {
                let mut clearance_drc = DesignRulesChecker::new(board);
                clearance_drc.get_all_violations()
            };
            let routing_involved: Vec<DrcViolation> = violations
                .into_iter()
                .filter(|violation| violation.involves_routing(board))
                .collect();
            BoardStatisticsClearanceViolations::from_violations(
                &routing_involved,
                board_unit_to_um_factor,
            )
        } else {
            BoardStatisticsClearanceViolations {
                total_count: Some(0),
                min_violation_um: Some(0.0),
                max_violation_um: Some(0.0),
                avg_violation_um: Some(0.0),
            }
        };

        let unit = unit.unwrap_or(Unit::Mm);
        if unit != board.communication.unit {
            let from_unit = board.communication.unit;
            let to_unit = unit;
            stats.unit = unit.to_string();

            let bounding_box = stats.board.bounding_box.unwrap_or_default();
            stats.board.bounding_box = Some(Rectangle2DFloat {
                x: scale_f32(bounding_box.x, from_unit, to_unit),
                y: scale_f32(bounding_box.y, from_unit, to_unit),
                width: scale_f32(bounding_box.width, from_unit, to_unit),
                height: scale_f32(bounding_box.height, from_unit, to_unit),
            });
            let size = stats.board.size.unwrap_or_default();
            stats.board.size = Some(Rectangle2DFloat {
                x: 0.0,
                y: 0.0,
                width: scale_f32(size.width, from_unit, to_unit),
                height: scale_f32(size.height, from_unit, to_unit),
            });

            stats.traces.total_length = stats
                .traces
                .total_length
                .map(|v| scale_f32(v, from_unit, to_unit));
            stats.traces.total_weighted_length = stats
                .traces
                .total_weighted_length
                .map(|v| scale_f32(v, from_unit, to_unit));
            stats.traces.average_length = stats
                .traces
                .average_length
                .map(|v| scale_f32(v, from_unit, to_unit));
            stats.traces.total_horizontal_length = stats
                .traces
                .total_horizontal_length
                .map(|v| scale_f32(v, from_unit, to_unit));
            stats.traces.total_vertical_length = stats
                .traces
                .total_vertical_length
                .map(|v| scale_f32(v, from_unit, to_unit));
            stats.traces.total_angled_length = stats
                .traces
                .total_angled_length
                .map(|v| scale_f32(v, from_unit, to_unit));
        }

        if include_fanout {
            stats.fanout = BoardStatistics::fanout_census(board);
        }

        stats
    }

    fn fanout_census(board: &mut Board) -> BoardStatisticsFanout {
        let smd_pins = board.get_smd_pins();
        let connected_pins = {
            let mut connectivity = CensusConnectivity::new(board);
            smd_pins
                .iter()
                .copied()
                .filter(|&pin| {
                    let item = board
                        .get_item(pin)
                        .expect("get_smd_pins returns existing pins");
                    !item.net_nos().is_empty()
                        && item
                            .net_nos()
                            .iter()
                            .all(|&net| connectivity.connected_to_all(pin, net))
                })
                .collect::<BTreeSet<_>>()
        };
        let mut total_pins = 0_i32;
        let mut escaped = 0_i32;
        let mut already_connected = 0_i32;
        for pin in smd_pins {
            let net_count = match board.get_item(pin) {
                Some(item) => item.net_count(),
                None => continue,
            };
            if net_count == 0 {
                continue;
            }
            total_pins += 1;
            if connected_pins.contains(&pin) {
                already_connected += 1;
            }
            if BoardStatistics::is_pin_escaped(board, pin) {
                escaped += 1;
            }
        }
        BoardStatisticsFanout {
            total_smd_pins: total_pins,
            pins_to_escape: total_pins - already_connected,
            escaped_count: escaped,
        }
    }

    pub fn is_pin_escaped(board: &mut Board, pin: ItemId) -> bool {
        let net_count = match board.get_item(pin) {
            Some(item) => item.net_count(),
            None => return false,
        };
        (0..net_count).all(|net_index| {
            let net_number = board
                .get_item(pin)
                .expect("the pin was just read")
                .get_net_number(net_index);
            BoardStatistics::is_pin_escaped_on_net(board, pin, net_number)
        })
    }

    fn is_pin_escaped_on_net(board: &mut Board, pin: ItemId, net_number: i32) -> bool {
        let contacts: Vec<ItemId> = board.normal_contacts(pin).into_iter().rev().collect();
        for contact in contacts {
            let (kind, shares_net) = match board.get_item(contact) {
                Some(item @ Item::Trace(_)) => (ContactKind::Trace, item.contains_net(net_number)),
                Some(item @ Item::Via(_)) => (ContactKind::Via, item.contains_net(net_number)),
                Some(item @ Item::ConductionArea(_)) => {
                    (ContactKind::ConductionArea, item.contains_net(net_number))
                }
                _ => (ContactKind::Other, false),
            };
            if !shares_net {
                continue;
            }
            match kind {
                ContactKind::Trace => {
                    if board.clearance_violations(contact).is_empty() {
                        return true;
                    }
                }
                ContactKind::Via => {
                    if board.clearance_violations(contact).is_empty() {
                        let via_contacts: Vec<ItemId> =
                            board.normal_contacts(contact).into_iter().rev().collect();
                        for via_contact in via_contacts {
                            let via_contact_shares_net = matches!(
                                board.get_item(via_contact),
                                Some(item @ (Item::Trace(_) | Item::ConductionArea(_)))
                                    if item.contains_net(net_number)
                            );
                            if via_contact_shares_net {
                                return true;
                            }
                        }
                    }
                }
                ContactKind::ConductionArea => return true,
                ContactKind::Other => {}
            }
        }
        false
    }
}

enum ContactKind {
    Trace,
    Via,
    ConductionArea,
    Other,
}

fn host_of(board: &Board) -> String {
    let host = format!(
        "{},{}",
        board.communication.host_cad.as_deref().unwrap_or("null"),
        board
            .communication
            .host_version
            .as_deref()
            .unwrap_or("null")
    );
    unescape_unicode(&host)
}

pub fn unescape_unicode(text: &str) -> String {
    let bytes: Vec<char> = text.chars().collect();
    let mut result = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '\\'
            && i + 5 < bytes.len()
            && bytes[i + 1] == 'u'
            && bytes[i + 2..i + 6].iter().all(|c| c.is_ascii_hexdigit())
        {
            let hex: String = bytes[i + 2..i + 6].iter().collect();
            let code = u32::from_str_radix(&hex, 16).expect("four hex digits");
            result.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
            i += 6;
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }
    result
}

fn to_degrees(radians: f64) -> f64 {
    radians * 180.0 / std::f64::consts::PI
}

fn scale_f32(value: f32, from_unit: Unit, to_unit: Unit) -> f32 {
    Unit::scale(f64::from(value), from_unit, to_unit) as f32
}

/// Connectivity queries for one fanout census, during which the board is unchanged.
struct CensusConnectivity<'a> {
    board: &'a Board,
    contacts: BTreeMap<ItemId, BTreeSet<ItemId>>,
    #[cfg(test)]
    contact_queries: usize,
}

impl<'a> CensusConnectivity<'a> {
    fn new(board: &'a Board) -> Self {
        Self {
            board,
            contacts: BTreeMap::new(),
            #[cfg(test)]
            contact_queries: 0,
        }
    }

    fn contacts(&mut self, id: ItemId) -> BTreeSet<ItemId> {
        self.contacts
            .entry(id)
            .or_insert_with(|| {
                #[cfg(test)]
                {
                    self.contact_queries += 1;
                }
                self.board.normal_contacts(id)
            })
            .clone()
    }

    fn connected_to_all(&mut self, id: ItemId, net_number: i32) -> bool {
        let Some(item) = self.board.get_item(id) else {
            return true;
        };
        if net_number > 0 && !item.contains_net(net_number) {
            return true;
        }
        let mut remaining = BTreeSet::new();
        if net_number > 0 {
            remaining.extend(self.board.get_connectable_items(net_number));
        } else {
            for net in item.net_nos() {
                remaining.extend(self.board.get_connectable_items(*net));
            }
        }
        let mut visited = BTreeSet::from([id]);
        let mut pending = vec![id];
        while let Some(current) = pending.pop() {
            remaining.remove(&current);
            for contact in self.contacts(current) {
                let Some(item) = self.board.get_item(contact) else {
                    continue;
                };
                if net_number > 0 && !item.contains_net(net_number) {
                    continue;
                }
                if visited.insert(contact) {
                    pending.push(contact);
                }
            }
        }
        remaining.is_empty()
    }
}

#[cfg(test)]
mod census_tests {
    use super::*;

    #[test]
    fn census_reuses_geometry_queries_without_changing_multinet_reachability() {
        let source = include_bytes!("../../tests/data/p9t13-multi-net-smd-pin.dsn");
        let result = copper_dsn::read_board(
            &source[..],
            None,
            Some("census"),
            &copper_dsn::DsnReadOptions::default(),
        );
        let board = match result {
            copper_dsn::BoardReadResult::Success { board, .. } => board.unwrap(),
            other => panic!("fixture failed: {other:?}"),
        };
        let mut census = CensusConnectivity::new(&board);
        let mut queries = Vec::new();
        for item in board.items.values() {
            for net in item.net_nos().iter().copied().chain([0, -1, 999]) {
                queries.push((item.id(), net));
            }
        }
        for &(id, net) in &queries {
            assert_eq!(
                census.connected_to_all(id, net),
                board.unconnected_set(id, net).is_empty(),
                "connectivity differs for {id:?}, net {net}"
            );
        }
        let first_queries = census.contact_queries;
        assert!(first_queries > 0, "exercise actual geometric queries");
        for &(id, net) in queries.iter().rev() {
            assert_eq!(
                census.connected_to_all(id, net),
                board.unconnected_set(id, net).is_empty()
            );
        }
        assert_eq!(
            census.contact_queries, first_queries,
            "a second census query over the same immutable board must reuse contact geometry"
        );
    }
}
