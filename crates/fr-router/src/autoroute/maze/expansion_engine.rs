//! Port of `autoroute.maze.MazeExpansionEngine` (MazeExpansionEngine.java:23-415) — "expands
//! drill pages, existing vias, and candidate via layers for a maze search".
//!
//! # Shape
//!
//! Java's class is `final`, package-private, holds one field (`private final MazeSearchEngine
//! search`) and is constructed once by `MazeSearchEngine`'s constructor (`:82`). The port is a
//! unit struct with associated functions taking `search: &mut MazeSearchEngine<'_>`, because the
//! search engine already borrows the `AutorouteEngine` mutably and a second struct holding a
//! `&mut MazeSearchEngine` would make every call site a re-borrow dance for no gain. The Java
//! field is therefore the leading parameter, exactly as `board.actions.ForcedViaInserter`'s
//! `RoutingBoard` is.
//!
//! # Visibility
//!
//! Every method here is package-private in Java (and `checkLayerWithAnyMatchingVia` is `private`)
//! and `pub` in the port, for the reason `search.rs` and `expand.rs` already give: Java's own
//! ground truth is a probe that declares the same package (and uses `setAccessible(true)` for the
//! private one), Rust integration tests have no such reach, and the branches that matter — the
//! thin-room refusal, the `DrillPage`-door cost branch, the obstacle-via layer span, the via mask
//! loop — are not separable through `occupyNextElement` alone.
//!
//! # `Via.getAutorouteDrillInfo` lives here
//!
//! [`via_autoroute_drill_info`] is `Via.getAutorouteDrillInfo` (Via.java:203-217). It cannot live
//! in `fr-board`: it builds an `autoroute.drill.ExpansionDrill` and fills its room array from
//! `ItemAutorouteInfo.getExpansionRoom`, neither of which `fr-board` can name (plan-6 ruling 15 —
//! the same split `autoroute/item_info.rs` already makes). The `autorouteDrillInfo` **field** is
//! `fr_board::AutorouteInfo::autoroute_drill_info`; see `fr_board::DrillId` for why that
//! placement reproduces Java's two clear sites exactly.

use fr_board::rules::PadstackLookup;
use fr_board::{Board, Item, ItemId, StopCheck};
use fr_geometry::{FloatLine, Point, TileShape};

use crate::arena::{DrillId, PageId};
use crate::autoroute::drill::ExpansionDrill;
use crate::autoroute::expansion::{ExpandableRef, RoomRef};
use crate::autoroute::item_info;
use crate::autoroute::maze::{AutorouteEngine, MazeAdjustment, MazeListElement, MazeSearchEngine};
use crate::board_ext::{CheckDrillResult, ForcedViaInserter};

/// Port of `maze.MazeExpansionEngine` (MazeExpansionEngine.java:23-415).
pub struct MazeExpansionEngine;

impl MazeExpansionEngine {
    // =============================================================================================
    // expandToDrill (:31-112)
    // =============================================================================================

    /// Port of `expandToDrill(ExpansionDrill, MazeListElement, int)`
    /// (MazeExpansionEngine.java:31-112).
    ///
    /// Every `search.fanoutDiagnostics.trace` payload (`:40-49`, `:102-111`) is dropped — the
    /// class is on the plan's not-ported roster — but the `return` the first one guards
    /// (`:37-51`) is not.
    ///
    /// # The two cost branches (`:76-85`)
    ///
    /// Reaching a drill **from the drill page's own element** keeps the element's backtrack door
    /// and charges nothing extra; reaching it from any other door makes that door the backtrack
    /// door and adds `ctrl.minNormalViaCost`. The page branch also re-aims the comparison corner
    /// at the start pin's nearest trace exit corner (`:56-65`), which is why the two produce
    /// different weighted distances as well as different constants.
    pub fn expand_to_drill(
        search: &mut MazeSearchEngine<'_>,
        board: &mut Board,
        drill: DrillId,
        from_element: &MazeListElement,
        add_costs: i32,
    ) {
        // :33.
        let Some(next_room) = from_element.next_room else {
            // Java would NPE on `fromElement.nextRoom.getLayer()`; no caller passes a drill
            // element (whose `nextRoom` is null) here.
            return;
        };
        let Some(layer) = search.engine.rooms.room_layer(board, next_room) else {
            return;
        };
        // :34.
        let trace_half_width = search.ctrl.compensated_trace_half_width[layer];
        // :35.
        let Some(room_shape) = search.engine.rooms.room_shape(next_room).cloned() else {
            return;
        };
        let room_shape_is_thin = room_shape.min_width() < 2.0 * f64::from(trace_half_width);

        let Some(drill_shape) = search
            .engine
            .rooms
            .drills
            .get(drill.0)
            .map(|d| d.get_shape().clone())
        else {
            return;
        };
        // :37-51. A thin room is only crossed towards a drill the backtrack door already touches.
        if room_shape_is_thin {
            let backtrack_intersects = from_element.backtrack_door.is_some_and(|door| {
                Self::expandable_shape(search.engine, door)
                    .is_some_and(|shape| drill_shape.intersects(&shape))
            });
            if !backtrack_intersects {
                return;
            }
        }

        // :53-54.
        let via_radius = search.ctrl.via_radii[layer];
        let shrinked_drill_shape = drill_shape.shrink(via_radius);
        // :55.
        let mut compare_corner = from_element
            .shape_entry
            .a
            .middle_point(&from_element.shape_entry.b);
        // :56-65. "the from door is the drill page itself and the backtrack door is a pin": aim
        // at the pin's own trace exit corner rather than at the shape entry's middle.
        if let (ExpandableRef::Page(_), Some(ExpandableRef::TargetDoor(backtrack))) =
            (from_element.door, from_element.backtrack_door)
        {
            let backtrack_item = search
                .engine
                .rooms
                .target_door(backtrack)
                .map(|door| door.item);
            if let Some(backtrack_item) = backtrack_item {
                let drill_location = search
                    .engine
                    .rooms
                    .drills
                    .get(drill.0)
                    .map(|d| d.location.clone());
                if let (Some(drill_location), Some(Item::Pin(pin))) =
                    (drill_location, board.items.get(&backtrack_item))
                {
                    let ctx = board.ctx();
                    if let Some(nearest_exit_corner) = pin.nearest_trace_exit_corner(
                        &drill_location.to_float(),
                        trace_half_width,
                        layer,
                        &ctx,
                    ) {
                        compare_corner = nearest_exit_corner;
                    }
                }
            }
        }
        // :66-67. Java NPEs on an empty shrunk shape; the drill shapes `getDrills` builds are
        // never empty, and a `None` here would be that throw.
        let Some(nearest_point) = shrinked_drill_shape.nearest_point_approx(&compare_corner) else {
            return;
        };
        let shape_entry = FloatLine::new(nearest_point, nearest_point);
        // :68.
        let Some(drill_first_layer) = search
            .engine
            .rooms
            .drills
            .get(drill.0)
            .map(|d| d.first_layer)
        else {
            return;
        };
        let section_index = i32::try_from(layer).unwrap_or(i32::MAX)
            - i32::try_from(drill_first_layer).unwrap_or(i32::MAX);
        // :69-75.
        let mut expansion_value = from_element.expansion_value
            + f64::from(add_costs)
            + nearest_point.weighted_distance(
                &compare_corner,
                search.ctrl.trace_costs[layer].horizontal,
                search.ctrl.trace_costs[layer].vertical,
            );
        // :76-85.
        let (new_backtrack_door, new_section_no_of_backtrack_door) =
            if matches!(from_element.door, ExpandableRef::Page(_)) {
                (
                    from_element.backtrack_door,
                    from_element.section_no_of_backtrack_door,
                )
            } else {
                expansion_value += search.ctrl.min_normal_via_cost;
                (Some(from_element.door), from_element.section_no_of_door)
            };
        // :86-87.
        let sorting_value = expansion_value
            + search
                .destination_distance
                .calculate_from_point(&nearest_point, layer);
        // :88-101.
        let new_element = MazeListElement {
            door: ExpandableRef::Drill(drill),
            section_no_of_door: section_index,
            backtrack_door: new_backtrack_door,
            section_no_of_backtrack_door: new_section_no_of_backtrack_door,
            expansion_value,
            sorting_value,
            next_room: None,
            shape_entry,
            room_ripped: from_element.room_ripped,
            adjustment: MazeAdjustment::None,
            already_checked: false,
            ripup_cost: 0,
        };
        search.push(new_element, board);
    }

    // =============================================================================================
    // expandToDrillPage (:115-143)
    // =============================================================================================

    /// Port of `expandToDrillPage(DrillPage, MazeListElement)` (MazeExpansionEngine.java:115-143):
    /// "inserts a drill page between a room and its candidate drills."
    ///
    /// The new element keeps the from element's door as its backtrack door and its room as the
    /// next room, so the page is a *pass-through*: `occupyNextElement` pops it, hands it to
    /// `expandToDrillsOfPage` and returns without expanding the room again.
    ///
    /// Note the asymmetry with [`Self::expand_to_drill`]: the `minNormalViaCost` goes into
    /// `expansionValue` (`:121`) but the weighted distance goes only into `sortingValue`
    /// (`:122-128`), so a page never charges the walk to it against the path cost.
    ///
    /// **Both drill expansions are hot on a real board, and Task 17 measured them.** The
    /// controller's Task 12 note listed `expandToDrillPage` among the paths no unit fixture
    /// reaches. Over Task 17's acceptance corpus (`tests/reference/router-fixtures.txt`) this
    /// method is entered **2 342 / 9 244 / 607 095 / 702** times on `router-rpi-splitter` /
    /// `router-j2-reference` / `router-dac2020-bm01` / `router-ecc83-input`, and
    /// [`Self::expand_to_drill`] **4 671 / 5 442 / 198 542 / 0** times on the same four — every
    /// one of them on a connection that matches the HEAD jar byte for byte. (`router-ecc83-input`
    /// builds 702 pages and expands to no drill at all: its via padstack never fits.) Only the
    /// **fanout** half of the drill machinery is unreached, and that needs `ctrl.isFanout`, which
    /// only `BatchFanout` sets — Plan 7's. Coverage notes rather than obligation markers.
    pub fn expand_to_drill_page(
        search: &mut MazeSearchEngine<'_>,
        board: &mut Board,
        drill_page: PageId,
        from_element: &MazeListElement,
    ) {
        // :117.
        let Some(next_room) = from_element.next_room else {
            return;
        };
        let Some(layer) = search.engine.rooms.room_layer(board, next_room) else {
            return;
        };
        // :118-119.
        let from_element_shape_entry_middle = from_element
            .shape_entry
            .a
            .middle_point(&from_element.shape_entry.b);
        // :120.
        let nearest_point = search
            .engine
            .drill_pages()
            .page(drill_page)
            .shape
            .nearest_point(&from_element_shape_entry_middle);
        // :121.
        let expansion_value = from_element.expansion_value + search.ctrl.min_normal_via_cost;
        // :122-128.
        let sorting_value = expansion_value
            + nearest_point.weighted_distance(
                &from_element_shape_entry_middle,
                search.ctrl.trace_costs[layer].horizontal,
                search.ctrl.trace_costs[layer].vertical,
            )
            + search
                .destination_distance
                .calculate_from_point(&nearest_point, layer);
        // :129-141. `sectionNoOfDoor` is the **layer**, which is how `expandToDrillsOfPage`
        // recovers it at `:147`.
        let new_element = MazeListElement {
            door: ExpandableRef::Page(drill_page),
            section_no_of_door: i32::try_from(layer).unwrap_or(i32::MAX),
            backtrack_door: Some(from_element.door),
            section_no_of_backtrack_door: from_element.section_no_of_door,
            expansion_value,
            sorting_value,
            next_room: from_element.next_room,
            shape_entry: from_element.shape_entry,
            room_ripped: from_element.room_ripped,
            adjustment: MazeAdjustment::None,
            already_checked: false,
            ripup_cost: 0,
        };
        search.push(new_element, board);
    }

    // =============================================================================================
    // expandToDrillsOfPage (:145-235)
    // =============================================================================================

    /// Port of `expandToDrillsOfPage(MazeListElement)` (MazeExpansionEngine.java:145-235).
    ///
    /// Three `continue`s reject a candidate drill (`:169-179` a section index outside the drill's
    /// layer span, `:180-223` a drill whose room on that layer is not the element's room,
    /// `:224-232` an already-occupied section); every one of them is a dropped
    /// `fanoutDiagnostics.trace` plus the `continue`, and the `FRLogger.trace` block of `:189-221`
    /// goes with them.
    ///
    /// `stop` is not a Java parameter — Java's method takes none — but
    /// `DrillPage.getDrills(autorouteEngine, attachSmd)` (`:150`) reaches
    /// `PolylineArea.splitToConvex(autorouteEngine.stoppableThread)`, which is plan-6 ruling 6's
    /// sixth cancellation site. The port threads the same flag through the borrow bridge.
    pub fn expand_to_drills_of_page(
        search: &mut MazeSearchEngine<'_>,
        board: &mut Board,
        from_element: &MazeListElement,
        stop: StopCheck<'_>,
    ) {
        // :147-148.
        let from_room_layer = from_element.section_no_of_door;
        let ExpandableRef::Page(drill_page) = from_element.door else {
            // Java's cast; `occupyNextElement:348` has already tested `instanceof DrillPage`.
            return;
        };
        // :149-150.
        let attach_smd_allowed = search.ctrl.attach_smd_allowed;
        let drill_list =
            search
                .engine
                .drill_page_drills(board, drill_page, attach_smd_allowed, stop);

        // :167.
        for current_drill in drill_list {
            let Some(drill) = search.engine.rooms.drills.get(current_drill.0) else {
                continue;
            };
            // :168.
            let section_index =
                from_room_layer - i32::try_from(drill.first_layer).unwrap_or(i32::MAX);
            // :169-179.
            let Ok(section_index_usize) = usize::try_from(section_index) else {
                continue;
            };
            if section_index_usize >= drill.rooms.len() {
                continue;
            }
            // :180-223.
            if drill.rooms[section_index_usize] != from_element.next_room {
                continue;
            }
            // :224-232.
            if drill
                .get_maze_search_element(section_index_usize)
                .is_occupied
            {
                continue;
            }
            // :233.
            Self::expand_to_drill(search, board, current_drill, from_element, 0);
        }
    }

    // =============================================================================================
    // expandToOtherLayers (:237-375)
    // =============================================================================================

    /// Port of `expandToOtherLayers(MazeListElement)` (MazeExpansionEngine.java:237-375): the
    /// layer span a via may span from the popped drill's layer, and one queue element per
    /// reachable layer.
    ///
    /// # The two ways the span is found
    ///
    /// * **an obstacle room** (`:246-261`): the drill sits on an existing `Via`, and the span is
    ///   that via's own padstack range — but only if ripup is allowed, the item is a via, its
    ///   padstack is in `ctrl.viaRule` and its clearance class matches. `roomRipped` is then
    ///   `true` for every element produced.
    /// * **free space** (`:262-314`): walk down from `fromLayer` to
    ///   `max(drill.firstLayer, ctrl.viaLowerBound)` and up to
    ///   `min(drill.lastLayer, ctrl.viaUpperBound)`, stopping at the first `NOT_DRILLABLE` layer,
    ///   and **abandon the whole drill** unless the span still covers the drill's full range
    ///   (`:289-291`, `:312-314`).
    ///
    /// The `viaLowerBound`/`viaUpperBound` locals are Java `int`s that legitimately go negative
    /// (`:240` starts `viaUpperBound` at `-1`, and `:303` can write `currentLayer - 1` with
    /// `currentLayer == 0`), so they are `i32` here rather than `usize`.
    pub fn expand_to_other_layers(
        search: &mut MazeSearchEngine<'_>,
        board: &mut Board,
        list_element: &MazeListElement,
    ) {
        // :239-241. Java's initialisers are `0` and `-1`; neither is ever read, because both
        // branches of `:246-315` assign both before the span loop at `:317` — the down-loop can
        // only leave through one of its two `break`s, and so can the up-loop. They are written
        // out anyway, so a later edit that adds a path past them is a compile error rather than a
        // silent `0`/`-1`.
        #[allow(unused_assignments)]
        let mut via_lower_bound = 0i32;
        #[allow(unused_assignments)]
        let mut via_upper_bound = -1i32;
        let ExpandableRef::Drill(current_drill) = list_element.door else {
            // Java's cast; `occupyNextElement:370` has already tested `instanceof ExpansionDrill`.
            return;
        };
        // The drill's four reads are taken once. Java re-reads `currentDrill.roomArr[…]` inside
        // both loops, but nothing they reach can change it: `checkLayerWithAnyMatchingVia` only
        // takes the **board** mutably, and the room arena is the engine's.
        let Some((drill_first_layer, drill_last_layer, drill_rooms, drill_location)) =
            search.engine.rooms.drills.get(current_drill.0).map(|d| {
                (
                    i32::try_from(d.first_layer).unwrap_or(i32::MAX),
                    i32::try_from(d.last_layer).unwrap_or(i32::MAX),
                    d.rooms.clone(),
                    d.location.clone(),
                )
            })
        else {
            return;
        };
        // :242.
        let from_layer = drill_first_layer + list_element.section_no_of_door;
        // :243-244.
        let mut smd_attached_on_component_side = false;
        let mut smd_attached_on_solder_side = false;
        let room_ripped;
        // :246. Java indexes `roomArr` with the raw `sectionNoOfDoor` and its `instanceof` is
        // `false` for a **null** slot, which is the `else` branch below — so a hole behaves the
        // same either way. An index *outside* the array is Java's
        // `ArrayIndexOutOfBoundsException` and this `None`; `occupyNextElement` has already
        // resolved `door.getMazeSearchElement(sectionNoOfDoor)` for this drill, so it cannot
        // arise.
        let from_room = usize::try_from(list_element.section_no_of_door)
            .ok()
            .and_then(|index| drill_rooms.get(index).copied().flatten());
        if let Some(RoomRef::Obstacle(room)) = from_room {
            // :247-249.
            if !search.ctrl.ripup_allowed {
                return;
            }
            // :250-253.
            let Some(obstacle_item) = search
                .engine
                .rooms
                .obstacle_room(room)
                .map(crate::autoroute::expansion::ObstacleExpansionRoom::get_item)
            else {
                return;
            };
            let Some(Item::Via(via)) = board.items.get(&obstacle_item) else {
                return;
            };
            // :254.
            let obstacle_padstack = via.get_padstack_id();
            let obstacle_clearance_class = via.hdr.clearance_class();
            // :255-258.
            let via_rule_contains = search
                .ctrl
                .via_rule
                .as_ref()
                .is_some_and(|rule| rule.contains_padstack(obstacle_padstack));
            if !via_rule_contains || obstacle_clearance_class != search.ctrl.via_clearance_class {
                return;
            }
            // :259-261.
            via_lower_bound = board
                .library
                .padstacks
                .padstack_from_layer(obstacle_padstack);
            via_upper_bound = board.library.padstacks.padstack_to_layer(obstacle_padstack);
            room_ripped = true;
        } else {
            // :263-264.
            let net_numbers = [search.ctrl.net_number];
            room_ripped = false;
            // :265-266.
            let via_lower_limit = drill_first_layer
                .max(i32::try_from(search.ctrl.via_lower_bound).unwrap_or(i32::MAX));
            let via_upper_limit = drill_last_layer
                .min(i32::try_from(search.ctrl.via_upper_bound).unwrap_or(i32::MAX));
            // :267-288. Downwards from `fromLayer`.
            let mut current_layer = from_layer;
            loop {
                let Some(current_room_shape) =
                    Self::drill_room_shape(search, &drill_rooms, current_layer - drill_first_layer)
                else {
                    // Java NPEs on a null room slot; every slot of a drill that reached the queue
                    // is filled by `calculateExpansionRooms`.
                    return;
                };
                let Ok(layer) = usize::try_from(current_layer) else {
                    return;
                };
                let drill_result = Self::check_layer_with_any_matching_via(
                    search,
                    board,
                    &current_room_shape,
                    &drill_location,
                    layer,
                    &net_numbers,
                );
                if drill_result == CheckDrillResult::NotDrillable {
                    // :273-275.
                    via_lower_bound = current_layer + 1;
                    break;
                } else if drill_result == CheckDrillResult::DrillableWithAttachSmd {
                    // :276-282.
                    if current_layer == 0 {
                        smd_attached_on_component_side = true;
                    } else if current_layer
                        == i32::try_from(search.ctrl.layer_count).unwrap_or(i32::MAX) - 1
                    {
                        // obligation: `MazeExpansionEngine.expandToOtherLayers`'
                        // `smdAttachedOnSolderSide` writes (`:279-281` here and `:306-308` in the
                        // up loop) still have **no ground truth** — **re-marked in Task 17**.
                        // Both need `checkLayerWithAnyMatchingVia` to answer
                        // `DRILLABLE_WITH_ATTACH_SMD` on the **last** layer, i.e. a `Pin` obstacle
                        // there at a location where the drill's rooms can both be built.
                        // `P6T13Probe` mode `attachsmd` pins the component-side twin and both
                        // halves of `maskOk` off the board's layer-0 SMD pad. Measured over
                        // Task 17's acceptance corpus (369 connections, five boards) with a
                        // counter on this write: **zero entries**. All five corpus boards are
                        // two-layer, and none of them has a bottom-side SMD pad the drill's rooms
                        // can be built around; the fixture that would discharge this needs one.
                        smd_attached_on_solder_side = true;
                    }
                }
                // :283-286.
                if current_layer <= via_lower_limit {
                    via_lower_bound = via_lower_limit;
                    break;
                }
                current_layer -= 1;
            }
            // :289-291.
            if via_lower_bound > drill_first_layer {
                return;
            }
            // :292-311. Upwards from `fromLayer + 1`.
            current_layer = from_layer + 1;
            loop {
                // :294-297.
                if current_layer > via_upper_limit {
                    via_upper_bound = via_upper_limit;
                    break;
                }
                let Some(current_room_shape) =
                    Self::drill_room_shape(search, &drill_rooms, current_layer - drill_first_layer)
                else {
                    return;
                };
                let Ok(layer) = usize::try_from(current_layer) else {
                    return;
                };
                let drill_result = Self::check_layer_with_any_matching_via(
                    search,
                    board,
                    &current_room_shape,
                    &drill_location,
                    layer,
                    &net_numbers,
                );
                if drill_result == CheckDrillResult::NotDrillable {
                    // :302-304.
                    via_upper_bound = current_layer - 1;
                    break;
                } else if drill_result == CheckDrillResult::DrillableWithAttachSmd
                    && current_layer
                        == i32::try_from(search.ctrl.layer_count).unwrap_or(i32::MAX) - 1
                {
                    // :305-309.
                    smd_attached_on_solder_side = true;
                }
                current_layer += 1;
            }
            // :312-314.
            if via_upper_bound < drill_last_layer {
                return;
            }
        }

        // :317.
        let mut to_layer = via_lower_bound;
        while to_layer <= via_upper_bound {
            let this_layer = to_layer;
            to_layer += 1;
            // :318-320.
            if this_layer == from_layer {
                continue;
            }
            // :321-329.
            let (current_first_layer, current_last_layer) = if this_layer < from_layer {
                (this_layer, from_layer)
            } else {
                (from_layer, this_layer)
            };
            // :330-345.
            let mut mask_found = false;
            for current_via_info in &search.ctrl.via_infos {
                if current_first_layer >= current_via_info.from_layer
                    && current_last_layer <= current_via_info.to_layer
                    && current_via_info.from_layer >= via_lower_bound
                    && current_via_info.to_layer <= via_upper_bound
                {
                    // :336-339.
                    let mask_ok = !(current_via_info.from_layer == 0
                        && smd_attached_on_component_side
                        || current_via_info.to_layer
                            == i32::try_from(search.ctrl.layer_count).unwrap_or(i32::MAX) - 1
                            && smd_attached_on_solder_side)
                        || current_via_info.attach_smd_allowed;
                    if mask_ok {
                        mask_found = true;
                        break;
                    }
                }
            }
            // :346-348.
            if !mask_found {
                continue;
            }
            // :349-353.
            let current_room_index = this_layer - drill_first_layer;
            let Some(current_drill_layer_info) = search
                .engine
                .maze_search_element(ExpandableRef::Drill(current_drill), current_room_index)
            else {
                // Java throws on an out-of-range section; the span above is inside the drill's
                // own layer range at every reachable input.
                return;
            };
            if current_drill_layer_info.is_occupied {
                continue;
            }
            // :354-355.
            let (Ok(from_layer_index), Ok(to_layer_index)) =
                (usize::try_from(from_layer), usize::try_from(this_layer))
            else {
                return;
            };
            let expansion_value = list_element.expansion_value
                + f64::from(search.ctrl.add_via_costs[from_layer_index][to_layer_index]);
            // :356-358.
            let shape_entry_middle = list_element
                .shape_entry
                .a
                .middle_point(&list_element.shape_entry.b);
            let sorting_value = expansion_value
                + search
                    .destination_distance
                    .calculate_from_point(&shape_entry_middle, to_layer_index);
            // :359-373.
            let Ok(room_index) = usize::try_from(current_room_index) else {
                return;
            };
            let new_element = MazeListElement {
                door: ExpandableRef::Drill(current_drill),
                section_no_of_door: current_room_index,
                backtrack_door: Some(ExpandableRef::Drill(current_drill)),
                section_no_of_backtrack_door: list_element.section_no_of_door,
                expansion_value,
                sorting_value,
                next_room: drill_rooms.get(room_index).copied().flatten(),
                shape_entry: list_element.shape_entry,
                room_ripped,
                adjustment: MazeAdjustment::None,
                already_checked: false,
                ripup_cost: 0,
            };
            search.push(new_element, board);
        }
    }

    // =============================================================================================
    // checkLayerWithAnyMatchingVia (:377-414)
    // =============================================================================================

    /// Port of the private `checkLayerWithAnyMatchingVia(ExpansionDrill, int, TileShape, int[])`
    /// (MazeExpansionEngine.java:377-414): the best `CheckDrillResult` over every via of
    /// `ctrl.viaRule` whose padstack covers `layer`.
    ///
    /// `DRILLABLE` short-circuits (`:404-406`); a `DRILLABLE_WITH_ATTACH_SMD` is remembered and
    /// answered only if no via manages a plain `DRILLABLE` (`:407-413`).
    ///
    /// `pub` where Java's is `private`, and it takes the drill's `location` rather than the drill
    /// itself: `:396` is the method's only read of the argument, and passing the point keeps the
    /// call sites free of a second borrow of the drill arena while `board` is `&mut`.
    // renamed: `checkLayerWithAnyMatchingVia(drill, …)` -> `(…, location, …)`.
    pub fn check_layer_with_any_matching_via(
        search: &mut MazeSearchEngine<'_>,
        board: &mut Board,
        room_shape: &TileShape,
        location: &Point,
        layer: usize,
        net_numbers: &[i32],
    ) -> CheckDrillResult {
        // :380.
        let mut drillable_with_attach_smd = false;
        let Some(via_rule) = search.ctrl.via_rule.as_ref() else {
            // `:381` dereferences `ctrl.viaRule`; `AutorouteControl.rebuildViaInfo` has already
            // NPE'd on a null one long before this (see its `# Panics`).
            return CheckDrillResult::NotDrillable;
        };
        // :381. The rule's via list is read once — `checkLayer` takes the board mutably and
        // nothing it reaches can change the rules. Java iterates the rule's own `ViaInfo`
        // objects; the port clones the rule's owned copies out for the same reason.
        let vias: Vec<fr_board::ViaInfo> = via_rule.iter().cloned().collect();
        for via_info in &vias {
            // :382-383.
            let via_padstack = via_info.get_padstack();
            let clearance_class_index = via_info.get_clearance_class_index();
            let attach_smd_allowed = via_info.attach_smd_allowed();
            // :384-386.
            let from_layer = board.library.padstacks.padstack_from_layer(via_padstack);
            let to_layer = board.library.padstacks.padstack_to_layer(via_padstack);
            let layer_no = i32::try_from(layer).unwrap_or(i32::MAX);
            if layer_no < from_layer || layer_no > to_layer {
                continue;
            }
            // :387-388. "viaShape == null ? 0 : 0.5 * viaShape.maxWidth()".
            let via_radius = board
                .library
                .padstacks
                .padstack_shape_max_width(via_padstack, layer_no)
                .map_or(0.0, |width| 0.5 * width);
            // :389.
            let required_radius =
                fr_geometry::java_max(via_radius, f64::from(search.ctrl.trace_half_width[layer]));
            // :390-403.
            let result = ForcedViaInserter::check_layer(
                board,
                required_radius,
                clearance_class_index,
                attach_smd_allowed,
                room_shape,
                location,
                layer,
                net_numbers,
                search.ctrl.max_shove_trace_recursion_depth,
                0,
                search.ctrl.trace_half_width[layer],
                search.ctrl.trace_clearance_class_index,
            );
            // :404-409.
            if result == CheckDrillResult::Drillable {
                return result;
            }
            if result == CheckDrillResult::DrillableWithAttachSmd {
                drillable_with_attach_smd = true;
            }
        }
        // :411-413.
        if drillable_with_attach_smd {
            CheckDrillResult::DrillableWithAttachSmd
        } else {
            CheckDrillResult::NotDrillable
        }
    }

    // =============================================================================================
    // The two `ExpandableObject` reads this file needs
    // =============================================================================================

    /// `ExpandableObject.getShape()` (ExpandableObject.java:16) over the four implementors —
    /// `MazeExpansionEngine.java:39`'s virtual call on `fromElement.backtrackDoor`.
    fn expandable_shape(engine: &AutorouteEngine, object: ExpandableRef) -> Option<TileShape> {
        engine.expandable_shape(object)
    }

    /// `currentDrill.roomArr[index].getShape()` (`:269-270`, `:298-299`).
    fn drill_room_shape(
        search: &MazeSearchEngine<'_>,
        drill_rooms: &[Option<RoomRef>],
        index: i32,
    ) -> Option<TileShape> {
        let index = usize::try_from(index).ok()?;
        let room = (*drill_rooms.get(index)?)?;
        search.engine.rooms.room_shape(room).cloned()
    }
}

// =================================================================================================
// Via.getAutorouteDrillInfo (Via.java:203-217)
// =================================================================================================

/// Port of `Via.getAutorouteDrillInfo(ShapeSearchTree)` (Via.java:203-217): the memoised
/// [`ExpansionDrill`] that binds this via's obstacle expansion room on every layer it spans.
///
/// It lives here rather than in `fr-board` — see the module docs. `None` is a `via` that is not a
/// `Via` on the board, which Java's caller (`MazeSearchEngine.java:617-619`) has already tested
/// with an `instanceof`.
///
/// # The memo is load-bearing
///
/// The drill carries a `MazeSearchElement` per layer, and `occupyNextElement` writes `isOccupied`
/// through it. A second call that built a *fresh* drill would hand the search an unoccupied one
/// and let it expand the same via for ever, so the cache is not an optimisation. It is stored in
/// `AutorouteInfo::autoroute_drill_info`, which the two Java sites that null
/// `Via.autorouteDrillInfo` also drop — see [`fr_board::DrillId`].
pub fn via_autoroute_drill_info(
    engine: &mut AutorouteEngine,
    board: &mut Board,
    via: ItemId,
) -> Option<DrillId> {
    // :205. The `getAutorouteInfo()` at `:206` creates the scratch, so a `pur` read here would
    // answer `None` for an item Java has just allocated one for; reading the created info is what
    // Java does.
    if let Some(existing) = board
        .get_item_mut(via)?
        .get_autoroute_info()
        .autoroute_drill_info
    {
        return Some(existing);
    }
    let ctx = board.ctx();
    let Some(Item::Via(via_item)) = board.items.get(&via) else {
        return None;
    };
    // :207.
    let centre = via_item.get_center();
    let first_layer = via_item.first_layer(&ctx);
    let last_layer = via_item.last_layer(&ctx);
    let current_drill_shape = TileShape::Box(TileShape::get_instance_from_point(&centre));
    // :208-210.
    let mut new_drill = ExpansionDrill::new(current_drill_shape, centre, first_layer, last_layer);
    // :211-214.
    let tree = engine.tree;
    let via_layer_count = last_layer - first_layer + 1;
    for i in 0..via_layer_count {
        let room = item_info::get_expansion_room(board, via, i, tree, |b, item, index, tree| {
            engine.rooms.new_obstacle_room(b, item, index, tree)
        });
        new_drill.rooms[i] = room.map(RoomRef::Obstacle);
    }
    let id = DrillId(engine.rooms.drills.insert(new_drill));
    board
        .get_item_mut(via)?
        .get_autoroute_info()
        .autoroute_drill_info = Some(id);
    // :216.
    Some(id)
}

// =================================================================================================
// The deferral roster for `autoroute/maze/MazeExpansionEngine.java`
// =================================================================================================

// not ported: `MazeExpansionEngine.MazeExpansionEngine` — the constructor, which stores the one
// `MazeSearchEngine` field. The port's methods take that field as their leading parameter (module
// docs), so there is nothing to construct.
//
// not ported: every `search.fanoutDiagnostics.trace` / `FRLogger.trace` payload in this file
// (`:40-49`, `:102-111`, `:151-165`, `:170-177`, `:181-221`, `:225-230`) — `MazeFanoutDiagnostics`
// is on the plan's not-ported roster and plan-6's global constraints drop the logger. Every
// `return` and `continue` those payloads guard **is** ported.
