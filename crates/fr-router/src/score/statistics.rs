//! Port of `core.scoring.BoardStatistics` (BoardStatistics.java:35-647) — **the score-relevant
//! subset only** (controller ruling AG).

use fr_board::items::Item;
use fr_board::rules::BoardRules;
use fr_board::structure::{FixedState, Unit};
use fr_board::{Board, ItemId};
use fr_drc::{BoardStatisticsClearanceViolations, DesignRulesChecker};
use fr_geometry::java_min;

use super::dtos::{
    BoardStatisticsBends, BoardStatisticsBoard, BoardStatisticsComponents,
    BoardStatisticsConnections, BoardStatisticsItems, BoardStatisticsLayers, BoardStatisticsNets,
    BoardStatisticsPads, BoardStatisticsTraces, BoardStatisticsVias, Rectangle2DFloat,
};

/// Port of `BoardStatistics.BoardStatisticsFanout` (BoardStatistics.java:637-647).
///
/// Java's three fields are primitive `int`, not boxed, so they start at `0` rather than `null` —
/// which is why this one struct in the family holds plain `i32`s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BoardStatisticsFanout {
    /// Java `totalSmdPins` (`:639-640`): SMD pins carrying at least one net.
    pub total_smd_pins: i32,
    /// Java `pinsToEscape` (`:642-643`): `totalSmdPins - alreadyConnected`.
    pub pins_to_escape: i32,
    /// Java `escapedCount` (`:645-646`): the pins [`BoardStatistics::is_pin_escaped`] accepts.
    pub escaped_count: i32,
}

/// Port of `core.scoring.BoardStatistics` (BoardStatistics.java:35-647) — **the score-relevant
/// subset only** (controller ruling AG). Plan 8's `fr-core` re-exports this type and adds the
/// Gson-compatible JSON surface and the `byte[]`/`FileFormat` constructor.
///
/// Every field is public and mutable, as Java's are: `BatchFanout`, `AutorouteBatchLoop` and
/// `BatchOptimizer` all read them directly rather than through accessors.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BoardStatistics {
    /// Java `host` (`:37-38`).
    pub host: String,
    /// Java `unit` (`:40-41`) — `board.communication.unit.toString()`, or the preferred unit's
    /// name once the conversion block at `:377-405` has run.
    pub unit: String,
    /// Java `board` (`:43-44`).
    pub board: BoardStatisticsBoard,
    /// Java `layers` (`:46-47`).
    pub layers: BoardStatisticsLayers,
    /// Java `items` (`:49-50`).
    pub items: BoardStatisticsItems,
    /// Java `components` (`:52-53`).
    pub components: BoardStatisticsComponents,
    /// Java `pads` (`:55-56`).
    pub pads: BoardStatisticsPads,
    /// Java `nets` (`:58-59`).
    pub nets: BoardStatisticsNets,
    /// Java `connections` (`:61-62`).
    pub connections: BoardStatisticsConnections,
    /// Java `traces` (`:64-65`).
    pub traces: BoardStatisticsTraces,
    /// Java `bends` (`:67-68`).
    pub bends: BoardStatisticsBends,
    /// Java `vias` (`:70-71`).
    pub vias: BoardStatisticsVias,
    /// Java `clearanceViolations` (`:73-75`). Re-used from `fr-drc`, never redeclared: Plan 5
    /// delivered both the DTO and the block of this constructor that fills it.
    pub clearance_violations: BoardStatisticsClearanceViolations,
    /// Java `fanout` (`:77-78`).
    pub fanout: BoardStatisticsFanout,
}

impl BoardStatistics {
    /// Port of `BoardStatistics(BasicBoard)` (BoardStatistics.java:83-86): the preferred unit is
    /// `null` (so millimetres, `:373-375`), clearance violations and connections both included.
    pub fn new(board: &mut Board) -> BoardStatistics {
        BoardStatistics::with_options(board, None, true)
    }

    /// Port of `BoardStatistics(BasicBoard, Unit, boolean)` (BoardStatistics.java:99-102) — the
    /// constructor `BatchFanout.java:158-162` calls with `(board, null, false)`.
    ///
    // not ported: `BoardStatistics(BasicBoard, Unit)` (BoardStatistics.java:92-94) — a second
    // two-argument delegator to the same three-argument body, with no caller in `src/main` or
    // `src/test`; [`Self::with_options`] with `include_clearance_violations = true` is it.
    pub fn with_options(
        board: &mut Board,
        unit: Option<Unit>,
        include_clearance_violations: bool,
    ) -> BoardStatistics {
        BoardStatistics::compute(board, unit, include_clearance_violations, true)
    }

    /// Port of the computing constructor
    /// `BoardStatistics(BasicBoard, Unit, boolean, boolean)` (BoardStatistics.java:110-427).
    ///
    /// `&mut Board` rather than Java's `BasicBoard`: the block at `:265-271` builds a
    /// [`DesignRulesChecker`], which plan-5 ruling 8 takes a `&mut Board`, and the clearance
    /// block at `:338-341` builds a second one.
    ///
    /// **Two `DesignRulesChecker`s, never one.** `:265-268` and `:338-341` each construct their
    /// own with `null` DRC settings and each runs its own full calculation; the two have
    /// different internal state after `calculateAllIncompletes`, and the construction count is
    /// observable through Plan 5's memo behaviour. Sharing one would be a different program.
    pub fn compute(
        board: &mut Board,
        unit: Option<Unit>,
        include_clearance_violations: bool,
        include_connections: bool,
    ) -> BoardStatistics {
        let mut stats = BoardStatistics::default();

        // BoardStatistics.java:111.
        let bb = board.get_bounding_box();

        // BoardStatistics.java:113-121.
        stats.host = host_of(board);

        // BoardStatistics.java:123.
        stats.unit = board.communication.unit.to_string();

        // BoardStatistics.java:125-137. Java re-reads `board.getBoundingBox()` for five of the
        // eight coordinates and uses the hoisted `bb` for the sixth; the method is a plain field
        // read (`BasicBoard.java:1093-1095`), so the two are the same box.
        //
        // Java bug: BoardStatistics.<init> (BoardStatistics.java:126-131) hands `Rectangle2D.Float`'s `w`/`h` parameters the board's **lower-left corner**, so `boundingBox.width`/`height` are negative on every corpus board. Quirk #196; `board.size` (`:132-137`) is the rectangle that really carries the extent, and [`Rectangle2DFloat`] has the whole note.
        stats.board.bounding_box = Some(Rectangle2DFloat {
            x: bb.ur.x as f32,
            y: bb.ur.y as f32,
            width: bb.ll.x as f32,
            height: bb.ll.y as f32,
        });
        stats.board.size = Some(Rectangle2DFloat {
            x: 0.0,
            y: 0.0,
            width: java_abs_f32(bb.ll.x as f32 - bb.ur.x as f32),
            height: java_abs_f32(bb.ll.y as f32 - bb.ur.y as f32),
        });

        // BoardStatistics.java:139-141.
        stats.layers.total_count = Some(board.get_layer_count() as i32);
        stats.layers.signal_count = Some(board.layer_structure().signal_layer_count() as i32);

        // BoardStatistics.java:143-174: one walk of `board.itemList` — descending item id,
        // quirk #63 — classifying each item by the `instanceof` chain at `:161-173`. The chain's
        // order is load-bearing: `Via` and `Pin` are tested before their own superclass
        // `DrillItem`, and `ConductionArea` before the `ObstacleArea` that would otherwise catch
        // it in the `else`.
        // `:144-151` zeroes all eight counters first; the accumulators below are that.
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
            // `DrillItem` has exactly two concrete subclasses, `Pin` and `Via`, and the chain
            // matches both above — so `:169-170` is unreachable and this counter is always 0.
            // Kept, and kept in the JSON, because it is a serialised field a reader looks for.
            drill_item_count: Some(0),
            pin_count: Some(pins),
            component_outline_count: Some(outlines),
            other_count: Some(other),
        };

        // BoardStatistics.java:176-177.
        stats.components = BoardStatisticsComponents {
            total_count: Some(board.components.count() as i32),
        };

        // BoardStatistics.java:179-180.
        stats.pads = BoardStatisticsPads {
            total_count: Some(board.get_pins().len() as i32),
        };

        // BoardStatistics.java:182-184.
        stats.nets = BoardStatisticsNets {
            total_count: Some(board.rules.nets.max_net_number()),
            class_count: Some(board.rules.net_classes.count() as i32),
        };

        // BoardStatistics.java:186-189.
        let trace_ids = board.get_traces();
        stats.traces.total_count = Some(trace_ids.len() as i32);
        // `mapToDouble(Trace::getLength).sum()` — a `DoubleStream.sum()`, i.e. **Kahan/Neumaier
        // compensated** summation over the collection's own order (descending item id), not a
        // naive `+=` fold. See [`java_double_stream_sum`].
        let total_length =
            java_double_stream_sum(trace_ids.iter().map(|id| match board.get_item(*id) {
                Some(Item::Trace(trace)) => trace.get_length(),
                _ => 0.0,
            })) as f32;
        stats.traces.total_length = Some(total_length);

        // BoardStatistics.java:190-202 — a HEAD-only correction that carries its own eleven-line
        // comment. Transcribed including the `resolution > 0` guard: a DSN whose `(resolution …)`
        // scope is missing or zero divides by 1, not by 0.
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

        // BoardStatistics.java:203.
        stats.traces.total_length_mm =
            Some((f64::from(total_length) * board_unit_to_mm_factor) as f32);
        // BoardStatistics.java:204-208: `float / int`, so a float division.
        stats.traces.average_length = Some(if trace_ids.is_empty() {
            0.0
        } else {
            total_length / trace_ids.len() as f32
        });

        // BoardStatistics.java:209-241.
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
            // BoardStatistics.java:216-220.
            if corner_count > 1 {
                total_segment_count += (corner_count - 1) as i32;
            }
            // Java bug: BoardStatistics.<init> (BoardStatistics.java:222-238) walks `polyline.lines` — each line's two **defining** points, not the trace's corners — and over **all** the lines, the two bounding ones included, so the horizontal / vertical / angled breakdown measures something that is not the trace and the three do not sum to `totalLength`. Quirk #195; measured on `Issue143-rpi_splitter` at k=8: 121 606.75 against a `totalLength` of 130 610.65.
            for line in polyline.lines() {
                let a = line.a.to_float();
                let b = line.b.to_float();
                // `Math.pow(x, 2.0)` is exactly `x * x`: fdlibm's `__ieee754_pow` special-cases
                // `y == 2` (`if (hy == 0x40000000) return x * x;`) and HotSpot's intrinsic does
                // the same, so the multiplication below is not an approximation of Java's.
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

        // BoardStatistics.java:243-263: a **second** walk of `board.itemList`, in the same
        // descending-id order, accumulating `float`s — so the order is observable in the last
        // bits and is Java's, not `getTraces()`'.
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
                // `:257-260`: "to produce less violations with pin exit directions."
                weighted /= 2.0;
            }
            total_weighted_length += weighted as f32;
        }
        stats.traces.total_weighted_length = Some(total_weighted_length);

        // BoardStatistics.java:265-271. The first of the two checkers.
        if include_connections {
            let mut drc = DesignRulesChecker::new(board);
            drc.calculate_all_incompletes();
            stats.connections = BoardStatisticsConnections {
                maximum_count: Some(drc.max_connections()),
                incomplete_count: Some(drc.get_incomplete_count() as i32),
            };
        }

        // BoardStatistics.java:273-320.
        // `:274-277` zeroes the four counters first.
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
                // `:303-307`. `Math.toDegrees(x)` is `x * 180.0 / PI` (Math.java), and
                // `Math.atan2` is fdlibm's — `f64::atan2` is the same algorithm.
                let mut angle = (to_degrees(dy2.atan2(dx2) - dy1.atan2(dx1))).abs();
                // `:306-308`: the two normalisations Java writes, both kept — the second is a
                // no-op after the first, but `Math.min` and the ternary are not the same
                // function on a NaN input. `fr_geometry::java_min`, not a local copy and not
                // `f64::min` (plan-6 convention 4): Java propagates NaN and Rust absorbs it.
                angle = java_min(angle, 360.0 - angle);
                angle = if angle > 180.0 { 360.0 - angle } else { angle };
                // `:310-316`.
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

        // BoardStatistics.java:322-336.
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

        // BoardStatistics.java:338-368. The second checker, and Plan 5's `from_violations` for
        // the arithmetic inside it.
        stats.clearance_violations = if include_clearance_violations {
            let violations = {
                let mut clearance_drc = DesignRulesChecker::new(board);
                clearance_drc.get_all_clearance_violations()
            };
            BoardStatisticsClearanceViolations::from_violations(
                &violations,
                board_unit_to_um_factor,
            )
        } else {
            // `:362-367`: the arm `from_violations` cannot reach, because a caller who does not
            // want the block does not call it.
            BoardStatisticsClearanceViolations {
                total_count: Some(0),
                min_violation_um: Some(0.0),
                max_violation_um: Some(0.0),
                avg_violation_um: Some(0.0),
            }
        };

        // BoardStatistics.java:370-405: convert every length to the preferred unit.
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
            // `traces.totalLengthMm` is deliberately **not** converted (`:394-405` lists six
            // fields and this is not one of them): it is a millimetre value by construction and
            // `calculateScore` reads it as one.
        }

        // BoardStatistics.java:407-426: the fanout block.
        let smd_pins = board.get_smd_pins();
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
            // Java bug: BoardStatistics.<init> (BoardStatistics.java:415) reads net index **0** only, so a multi-net SMD pin's later nets cannot move `pinsToEscape`. Quirk #194.
            let net_number = board
                .get_item(pin)
                .expect("the pin was just read")
                .get_net_number(0);
            if board.unconnected_set(pin, net_number).is_empty() {
                already_connected += 1;
            }
            if BoardStatistics::is_pin_escaped(board, pin) {
                escaped += 1;
            }
        }
        stats.fanout = BoardStatisticsFanout {
            total_smd_pins: total_pins,
            pins_to_escape: total_pins - already_connected,
            escaped_count: escaped,
        };

        stats
    }

    /// Port of `isPinEscaped(Pin)` (BoardStatistics.java:554-576): whether the pin has a trace, a
    /// conduction area, or a via-plus-trace/area escape that carries no clearance violation.
    ///
    /// `&mut Board`, not the brief's `&Board`: the predicate calls `Item.clearanceViolations`
    /// twice (`:559`, `:564`), and this port's `Board::clearance_violations` is `&mut self`
    /// because Java's lowers `this.smallestClearance` and advances the search tree's entry
    /// counter (`crates/fr-board/src/board/clearance.rs:37`). A `&Board` signature would have to
    /// drop that state change, which is exactly what plan-2's note says not to do.
    ///
    /// The three arms are Java's, in Java's order, and the second one's asymmetry is Java's too:
    /// a **trace** contact escapes on its own, a **via** contact escapes only if it in turn
    /// touches a trace or a conduction area, and that inner walk does not check the inner item's
    /// own violations.
    pub fn is_pin_escaped(board: &mut Board, pin: ItemId) -> bool {
        // `:556`. `getNormalContacts()` is `Item`'s no-argument overload, and Java's is a
        // `TreeSet<Item>` — **descending** item id (quirk #44) — where the port's is an ascending
        // `BTreeSet<ItemId>`, so the walk is reversed. The *answer* is order-independent (the
        // loop is an existence test), but the **side effects** are not: `clearance_violations`
        // lowers `smallestClearance` and advances the search tree's entry counter on every
        // candidate it reaches, and the early return decides how many it reaches.
        let contacts: Vec<ItemId> = board.normal_contacts(pin).into_iter().rev().collect();
        for contact in contacts {
            let kind = match board.get_item(contact) {
                Some(Item::Trace(_)) => ContactKind::Trace,
                Some(Item::Via(_)) => ContactKind::Via,
                Some(Item::ConductionArea(_)) => ContactKind::ConductionArea,
                _ => ContactKind::Other,
            };
            match kind {
                // `:558-562`.
                ContactKind::Trace => {
                    if board.clearance_violations(contact).is_empty() {
                        return true;
                    }
                }
                // `:562-571`.
                ContactKind::Via => {
                    if board.clearance_violations(contact).is_empty() {
                        // Descending again (`:565`), for the same reason.
                        let via_contacts: Vec<ItemId> =
                            board.normal_contacts(contact).into_iter().rev().collect();
                        for via_contact in via_contacts {
                            if matches!(
                                board.get_item(via_contact),
                                Some(Item::Trace(_)) | Some(Item::ConductionArea(_))
                            ) {
                                return true;
                            }
                        }
                    }
                }
                // `:571-573`.
                ContactKind::ConductionArea => return true,
                ContactKind::Other => {}
            }
        }
        // `:575`.
        false
    }
}

/// The `instanceof` chain of `isPinEscaped` (`:558`, `:562`, `:571`), resolved once so the
/// `&Board` borrow it needs does not overlap the `&mut Board` borrow the body needs.
enum ContactKind {
    Trace,
    Via,
    ConductionArea,
    Other,
}

/// `BoardStatistics.java:113-121`: the host string, and the one line of it that cannot run.
///
// not reachable: the `"Freerouting," + Constants.FREEROUTING_VERSION` fallback (BoardStatistics.java:118-120) — `hostCad + "," + hostVersion` is a Java string concatenation, so it is never `null` and always contains at least the comma; `host.isEmpty()` is therefore false on every input, including two nulls (which concatenate to the literal `"null,null"`).
fn host_of(board: &Board) -> String {
    // `:113-117`. Java's `+` renders a null reference as the four characters `null`; the port's
    // `Communication` flattened `SpecctraParserInfo` onto itself (plan-2), so the two `Option`s
    // are the same two nullable fields.
    let host = format!(
        "{},{}",
        board.communication.host_cad.as_deref().unwrap_or("null"),
        board
            .communication
            .host_version
            .as_deref()
            .unwrap_or("null")
    );
    // `:121`.
    unescape_unicode(&host)
}

/// Port of `TextManager.unescapeUnicode` (util/TextManager.java:177-186), transcribed inline.
///
// not ported: the rest of `util/TextManager.java` — a GUI resource-bundle façade (spec §2: no GUI). Only this one static string helper is on the scoring path.
//
// totalized: `TextManager.unescapeUnicode` (util/TextManager.java:181-182) — Java feeds the decoded character to `Matcher.appendReplacement`, which treats `$` and `\` in the *replacement* as metacharacters, so an input containing the escape `$` or `\` throws `IllegalArgumentException` ("Illegal group reference" / "character to be escaped is missing") instead of decoding. This port decodes them. Unreachable from a DSN `(host_cad …)` in the corpus, and a crash is not a value worth reproducing.
fn unescape_unicode(text: &str) -> String {
    let bytes: Vec<char> = text.chars().collect();
    let mut result = String::with_capacity(text.len());
    let mut i = 0;
    while i < bytes.len() {
        // `\\u(\p{XDigit}{4})` — a literal backslash, a `u`, then exactly four hex digits.
        if bytes[i] == '\\'
            && i + 5 < bytes.len()
            && bytes[i + 1] == 'u'
            && bytes[i + 2..i + 6].iter().all(|c| c.is_ascii_hexdigit())
        {
            let hex: String = bytes[i + 2..i + 6].iter().collect();
            let code = u32::from_str_radix(&hex, 16).expect("four hex digits");
            // Java's `(char)` cast: a UTF-16 code unit. A lone surrogate cannot become a Rust
            // `char`, so it is replaced the way `String::from_utf16_lossy` would — the corpus
            // reaches neither branch.
            result.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
            i += 6;
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }
    result
}

/// `java.util.stream.DoubleStream.sum()` — **not** a naive fold.
///
/// `DoublePipeline.sum()` collects into a three-slot array through
/// `Collectors.sumWithCompensation` (Kahan/Neumaier) while keeping a plain running total in the
/// third slot, then hands the array to `Collectors.computeFinalSum`.
/// `BoardStatistics.java:188-189` reaches it through
/// `board.getTraces().stream().mapToDouble(Trace::getLength).sum()`.
///
/// **The compensation slot holds the *negated* low-order bits, so the final sum SUBTRACTS it.**
/// JDK 25 `Collectors.computeFinalSum` (java.base, `src.zip`), comment and all:
///
/// ```java
/// static double computeFinalSum(double[] summands) {
///     // Final sum with better error bounds subtract second summand as it is negated
///     double tmp = summands[0] - summands[1];
///     double simpleSum = summands[summands.length - 1];
///     if (Double.isNaN(tmp) && Double.isInfinite(simpleSum))
///         return simpleSum;
///     else
///         return tmp;
/// }
/// ```
///
/// Adding it instead moves *away* from the true sum by twice the compensation — measured at
/// double width on 865 of 200 000 random summations, and invisible at `f32` width on all of them,
/// which is why the whole `p7t7` corpus passes either way. `the_kahan_sum_subtracts_the_negated_
/// compensation_term` pins the sign with three vectors whose compensation lands exactly on a
/// rounding tie.
///
/// Public because the sum is a general JDK helper: `BoardStatistics`' only use narrows it to
/// `f32` one line later (`:189`), which hides the difference, but a later caller that does not
/// narrow would see it.
///
// renamed: the JDK's `DoubleStream.sum` / `DoublePipeline.sum` / `Collectors.sumWithCompensation` / `Collectors.computeFinalSum` -> this function; it is runtime-library code rather than freerouting code, so it has no `audit-port.sh` row.
pub fn java_double_stream_sum(values: impl Iterator<Item = f64>) -> f64 {
    let mut sum = 0.0_f64;
    let mut compensation = 0.0_f64;
    let mut simple_sum = 0.0_f64;
    for value in values {
        // `Collectors.sumWithCompensation`: `compensation` is `intermediateSum[1]`, the negated
        // low-order bits.
        let tmp = value - compensation;
        let velvel = sum + tmp;
        compensation = (velvel - sum) - tmp;
        sum = velvel;
        simple_sum += value;
    }
    // `Collectors.computeFinalSum`: `summands[0] - summands[1]`, because `summands[1]` is negated.
    let tmp = sum - compensation;
    if tmp.is_nan() && simple_sum.is_infinite() {
        simple_sum
    } else {
        tmp
    }
}

/// `Math.abs(float)` — which, unlike `f32::abs`, is defined as clearing the sign bit and so
/// answers `+0.0` for `-0.0` and `NaN` for `NaN`. Rust's `f32::abs` does the same; the wrapper
/// exists to name the Java method at the call site.
fn java_abs_f32(value: f32) -> f32 {
    value.abs()
}

/// `Math.toDegrees(double)` (java.lang.Math): `angrad * 180.0 / PI`, in that association.
fn to_degrees(radians: f64) -> f64 {
    radians * 180.0 / std::f64::consts::PI
}

/// `(float) Unit.scale(<float>, from, to)` — the widening to `double`, the scale, and the
/// narrowing back, as the six lines at `BoardStatistics.java:395-404` write it.
fn scale_f32(value: f32, from_unit: Unit, to_unit: Unit) -> f32 {
    Unit::scale(f64::from(value), from_unit, to_unit) as f32
}
