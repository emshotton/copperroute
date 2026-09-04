use std::collections::{BTreeMap, BTreeSet};

use fr_board::structure::AngleRestriction;
use fr_board::{Board, Item, ItemId, TreeId};
use fr_geometry::{FloatPoint, IntPoint, TileShape};

use crate::autoroute::expansion::{ExpandableRef, RoomRef};
use crate::autoroute::maze::{AutorouteControl, AutorouteEngine, MazeResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultItem {
    pub corners: Vec<IntPoint>,
    pub layer: usize,
}

impl ResultItem {
    pub fn new(corners: Vec<IntPoint>, layer: usize) -> ResultItem {
        ResultItem { corners, layer }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BacktrackElement {
    pub door: ExpandableRef,
    pub section_no_of_door: i32,
    pub next_room: Option<RoomRef>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocatorKind {
    FortyFiveDegree,
    AnyAngle,
}

impl LocatorKind {
    pub fn of(angle_restriction: AngleRestriction) -> LocatorKind {
        if angle_restriction == AngleRestriction::NinetyDegree
            || angle_restriction == AngleRestriction::FortyFiveDegree
        {
            LocatorKind::FortyFiveDegree
        } else {
            LocatorKind::AnyAngle
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundConnectionLocator {
    pub connection_items: Vec<ResultItem>,

    pub start_item: Option<ItemId>,

    pub start_layer: usize,

    pub target_item: Option<ItemId>,

    pub target_layer: usize,

    pub backtrack_array: Vec<BacktrackElement>,
}

impl FoundConnectionLocator {
    pub fn get_instance(
        maze_search_result: Option<&MazeResult>,
        ctrl: &AutorouteControl,
        engine: &mut AutorouteEngine,
        board: &mut Board,
        angle_restriction: AngleRestriction,
        ripped_item_list: &mut BTreeSet<ItemId>,
        ripup_costs: Option<&mut BTreeMap<ItemId, i32>>,
    ) -> Option<FoundConnectionLocator> {
        let maze_search_result = maze_search_result?;
        Some(FoundConnectionLocator::new(
            maze_search_result,
            ctrl,
            engine,
            board,
            angle_restriction,
            ripped_item_list,
            ripup_costs,
        ))
    }

    #[allow(clippy::too_many_lines)]
    fn new(
        maze_search_result: &MazeResult,
        ctrl: &AutorouteControl,
        engine: &mut AutorouteEngine,
        board: &mut Board,
        angle_restriction: AngleRestriction,
        ripped_item_list: &mut BTreeSet<ItemId>,
        ripup_costs: Option<&mut BTreeMap<ItemId, i32>>,
    ) -> FoundConnectionLocator {
        let backtrack_array = backtrack(maze_search_result, engine, ripped_item_list, ripup_costs);

        let start_info = *backtrack_array
            .last()
            .expect("backtrack always yields at least the destination element");

        let ExpandableRef::TargetDoor(start_door) = start_info.door else {
            // FRLogger.warn("FoundConnectionLocator: ItemExpansionDoor expected for
            return FoundConnectionLocator {
                connection_items: Vec::new(),
                start_item: None,
                start_layer: 0,
                target_item: None,
                target_layer: 0,
                backtrack_array,
            };
        };

        let start_door_data = engine
            .rooms
            .target_door(start_door)
            .expect("the start door is live: the backtrack walk just read it");
        let start_item = start_door_data.item;
        let start_tree_entry_no = start_door_data.tree_entry_no;
        let start_room = start_door_data
            .room
            .expect("TargetItemExpansionDoor.room: Java dereferences it at :114 with no guard");
        let start_layer = engine
            .rooms
            .room_layer(board, start_room)
            .expect("the start door's room is live");

        let mut at_fanout_end = false;
        let target_item;
        let target_layer;
        let current_from_point;
        match maze_search_result.destination_door {
            ExpandableRef::TargetDoor(destination_door) => {
                let door = engine
                    .rooms
                    .target_door(destination_door)
                    .expect("the destination door is live");
                let door_item = door.item;
                let door_tree_entry_no = door.tree_entry_no;
                let door_room = door
                    .room
                    .expect("TargetItemExpansionDoor.room: dereferenced at :121 with no guard");
                target_item = Some(door_item);
                target_layer = engine
                    .rooms
                    .room_layer(board, door_room)
                    .expect("the destination door's room is live");
                current_from_point = calculate_starting_point(
                    engine,
                    board,
                    door_item,
                    door_tree_entry_no,
                    door_room,
                    engine.tree,
                );
            }
            ExpandableRef::Drill(drill) => {
                let drill = engine
                    .rooms
                    .drills
                    .get(drill.0)
                    .expect("the destination drill is live");
                target_item = None;
                current_from_point = drill.location.to_float();
                target_layer = drill.first_layer
                    + usize::try_from(maze_search_result.section_no_of_door)
                        .expect("a section number is non-negative");
                at_fanout_end = true;
            }
            ExpandableRef::Door(_) | ExpandableRef::Page(_) => {
                // FRLogger.warn("FoundConnectionLocator: unexpected type of destinationDoor")
                return FoundConnectionLocator {
                    connection_items: Vec::new(),
                    start_item: Some(start_item),
                    start_layer,
                    target_item: None,
                    target_layer: 0,
                    backtrack_array,
                };
            }
        }

        let mut walk = LocatorWalk {
            engine,
            ctrl,
            angle_restriction,
            kind: LocatorKind::of(angle_restriction),
            current_from_point,
            current_from_id: 0,
            previous_from_point: current_from_point,
            current_trace_layer: target_layer,
            current_from_door_index: 0,
            current_to_door_index: 0,
            current_target_door_index: 0,
            current_target_shape: TileShape::Simplex(fr_geometry::Simplex::EMPTY),
            next_corner_id: 1,
        };

        let mut connection_items: Vec<ResultItem> = Vec::new();
        let element_count = i32::try_from(backtrack_array.len())
            .expect("the backtrack array is far shorter than i32::MAX");

        let mut connection_done = false;
        while !connection_done {
            let mut layer_changed = false;
            if at_fanout_end {
                layer_changed = true;
            } else {
                walk.current_target_door_index = walk.current_from_door_index + 1;
                while walk.current_target_door_index < element_count && !layer_changed {
                    let index = usize::try_from(walk.current_target_door_index)
                        .expect("the loop starts at >= 1");
                    if matches!(backtrack_array[index].door, ExpandableRef::Drill(_)) {
                        layer_changed = true;
                    } else {
                        walk.current_target_door_index += 1;
                    }
                }
            }
            if layer_changed {
                let index = usize::try_from(walk.current_target_door_index)
                    .expect("a drill index is non-negative");
                let ExpandableRef::Drill(drill) = backtrack_array[index].door else {
                    panic!(
                        "FoundConnectionLocator: backtrackArray[{index}].door is not an \
                         ExpansionDrill — Java throws a ClassCastException at \
                         FoundConnectionLocator.java:157-158"
                    )
                };
                let location = walk
                    .engine
                    .rooms
                    .drills
                    .get(drill.0)
                    .expect("the backtrack drill is live")
                    .location
                    .clone();
                walk.current_target_shape =
                    TileShape::Box(TileShape::get_instance_from_point(&location));
            } else {
                connection_done = true;
                walk.current_target_door_index = element_count - 1;
                let target_shape = trace_connection_shape(
                    board,
                    start_item,
                    walk.engine.tree,
                    start_tree_entry_no,
                )
                .expect("Connectable.getTraceConnectionShape: the start item is connectable");
                let start_room_shape = walk
                    .engine
                    .rooms
                    .room_shape(start_room)
                    .expect("the start door's room has a shape")
                    .clone();
                walk.current_target_shape = target_shape.intersection(&start_room_shape);
                if walk.current_target_shape.dimension() >= 2 {
                    let start_room_layer = walk
                        .engine
                        .rooms
                        .room_layer(board, start_room)
                        .expect("the start door's room is live");
                    let trace_half_width =
                        f64::from(walk.ctrl.compensated_trace_half_width[start_room_layer]);
                    let shrinked_shape = walk.current_target_shape.offset(-trace_half_width);
                    if !shrinked_shape.is_empty() {
                        walk.current_target_shape = shrinked_shape;
                    }
                }
            }
            walk.current_to_door_index = walk.current_from_door_index + 1;
            let next_trace =
                walk.calculate_next_trace(board, &backtrack_array, layer_changed, at_fanout_end);
            at_fanout_end = false;
            connection_items.push(next_trace);
        }

        FoundConnectionLocator {
            connection_items,
            start_item: Some(start_item),
            start_layer,
            target_item,
            target_layer,
            backtrack_array,
        }
    }
}

fn calculate_starting_point(
    engine: &AutorouteEngine,
    board: &mut Board,
    item: ItemId,
    tree_entry_no: usize,
    room: RoomRef,
    tree: TreeId,
) -> FloatPoint {
    let connection_shape = trace_connection_shape(board, item, tree, tree_entry_no)
        .expect("Connectable.getTraceConnectionShape: a target door's item is connectable");
    let room_shape = engine
        .rooms
        .room_shape(room)
        .expect("the target door's room has a shape");
    let connection_shape = connection_shape.intersection(room_shape);
    connection_shape.centre_of_gravity().round().to_float()
}

fn trace_connection_shape(
    board: &Board,
    item: ItemId,
    tree: TreeId,
    index: usize,
) -> Option<TileShape> {
    let ctx = board.ctx();
    board
        .items
        .get(&item)
        .and_then(Item::as_connectable)
        .and_then(|connectable| {
            connectable
                .as_dyn()
                .get_trace_connection_shape(tree, index, &ctx)
        })
}

fn backtrack(
    maze_search_result: &MazeResult,
    engine: &AutorouteEngine,
    ripped_item_list: &mut BTreeSet<ItemId>,
    mut ripup_costs: Option<&mut BTreeMap<ItemId, i32>>,
) -> Vec<BacktrackElement> {
    let mut result: Vec<BacktrackElement> = Vec::new();
    let mut current_next_room: Option<RoomRef> = None;
    let mut current_backtrack_door = maze_search_result.destination_door;
    let mut current_element = engine
        .maze_search_element(
            current_backtrack_door,
            maze_search_result.section_no_of_door,
        )
        .expect("the destination door's section is live")
        .clone();

    match current_backtrack_door {
        ExpandableRef::TargetDoor(door) => {
            current_next_room = engine
                .rooms
                .target_door(door)
                .expect("the destination door is live")
                .room;
        }
        ExpandableRef::Drill(drill) => {
            let drill = engine
                .rooms
                .drills
                .get(drill.0)
                .expect("the destination drill is live");
            let index = drill.first_layer
                + usize::try_from(maze_search_result.section_no_of_door)
                    .expect("a section number is non-negative");
            current_next_room = drill.rooms[index];
            if current_element.room_ripped {
                let rooms = drill.rooms.clone();
                for room in rooms.into_iter().flatten() {
                    if let RoomRef::Obstacle(obstacle) = room {
                        let item = engine
                            .rooms
                            .obstacle_room(obstacle)
                            .expect("the drill's obstacle room is live")
                            .get_item();
                        ripped_item_list.insert(item);
                        if let Some(costs) = ripup_costs.as_deref_mut() {
                            costs.insert(item, current_element.ripup_cost);
                        }
                    }
                }
            }
        }
        ExpandableRef::Door(_) | ExpandableRef::Page(_) => {}
    }

    let mut current_backtrack_element = BacktrackElement {
        door: current_backtrack_door,
        section_no_of_door: maze_search_result.section_no_of_door,
        next_room: current_next_room,
    };
    loop {
        result.push(current_backtrack_element);
        let Some(next_door) = current_element.backtrack_door else {
            break;
        };
        current_backtrack_door = next_door;
        let mut current_section_no = current_element.section_no_of_backtrack_door;
        let section_count = i32::try_from(
            engine
                .maze_search_element_count(current_backtrack_door)
                .expect("a backtrack door always has its section array allocated"),
        )
        .expect("a door has far fewer sections than i32::MAX");
        if current_section_no >= section_count {
            // FRLogger.warn("FoundConnectionLocator: currentSectionNo to big")
            current_section_no = section_count - 1;
        }
        current_next_room = match current_backtrack_door {
            ExpandableRef::Drill(drill) => {
                let index =
                    usize::try_from(current_section_no).expect("a section number is non-negative");
                engine
                    .rooms
                    .drills
                    .get(drill.0)
                    .expect("the backtrack drill is live")
                    .rooms[index]
            }
            _ => match current_next_room {
                Some(room) => engine.expandable_other_room(current_backtrack_door, room),
                None => None,
            },
        };
        current_element = engine
            .maze_search_element(current_backtrack_door, current_section_no)
            .expect("the backtrack door's section is live")
            .clone();
        current_backtrack_element = BacktrackElement {
            door: current_backtrack_door,
            section_no_of_door: current_section_no,
            next_room: current_next_room,
        };
        if current_element.room_ripped
            && let Some(RoomRef::Obstacle(obstacle)) = current_next_room
        {
            let item = engine
                .rooms
                .obstacle_room(obstacle)
                .expect("the backtrack obstacle room is live")
                .get_item();
            ripped_item_list.insert(item);
            if let Some(costs) = ripup_costs.as_deref_mut() {
                costs.insert(item, current_element.ripup_cost);
            }
        }
    }
    result
}

fn ninety_degree_corner(
    from_point: FloatPoint,
    to_point: FloatPoint,
    horizontal_first: bool,
) -> FloatPoint {
    let (x, y) = if horizontal_first {
        (to_point.x, from_point.y)
    } else {
        (from_point.x, to_point.y)
    };
    FloatPoint::new(x, y)
}

fn fortyfive_degree_corner(
    from_point: FloatPoint,
    to_point: FloatPoint,
    horizontal_first: bool,
) -> FloatPoint {
    let abs_dx = (to_point.x - from_point.x).abs();
    let abs_dy = (to_point.y - from_point.y).abs();
    let x;
    let y;

    if abs_dx <= abs_dy {
        if horizontal_first {
            x = to_point.x;
            y = if to_point.y >= from_point.y {
                from_point.y + abs_dx
            } else {
                from_point.y - abs_dx
            };
        } else {
            x = from_point.x;
            y = if to_point.y > from_point.y {
                to_point.y - abs_dx
            } else {
                to_point.y + abs_dx
            };
        }
    } else if horizontal_first {
        y = from_point.y;
        x = if to_point.x > from_point.x {
            to_point.x - abs_dy
        } else {
            to_point.x + abs_dy
        };
    } else {
        y = to_point.y;
        x = if to_point.x > from_point.x {
            from_point.x + abs_dy
        } else {
            from_point.x - abs_dy
        };
    }
    FloatPoint::new(x, y)
}

pub fn calculate_additional_corner(
    from_point: FloatPoint,
    to_point: FloatPoint,
    horizontal_first: bool,
    angle_restriction: AngleRestriction,
) -> FloatPoint {
    match angle_restriction {
        AngleRestriction::NinetyDegree => {
            ninety_degree_corner(from_point, to_point, horizontal_first)
        }
        AngleRestriction::FortyFiveDegree => {
            fortyfive_degree_corner(from_point, to_point, horizontal_first)
        }
        AngleRestriction::None => to_point,
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LocatedCorner {
    pub(crate) point: FloatPoint,
    pub(crate) id: u64,
}

pub(crate) struct LocatorWalk<'a> {
    pub(crate) engine: &'a mut AutorouteEngine,
    pub(crate) ctrl: &'a AutorouteControl,
    pub(crate) angle_restriction: AngleRestriction,
    pub(crate) kind: LocatorKind,
    pub(crate) current_from_point: FloatPoint,
    pub(crate) current_from_id: u64,
    pub(crate) previous_from_point: FloatPoint,
    pub(crate) current_trace_layer: usize,
    pub(crate) current_from_door_index: i32,
    pub(crate) current_to_door_index: i32,
    pub(crate) current_target_door_index: i32,
    pub(crate) current_target_shape: TileShape,
    next_corner_id: u64,
}

impl LocatorWalk<'_> {
    pub(crate) fn new_corner_id(&mut self) -> u64 {
        self.next_corner_id += 1;
        self.next_corner_id
    }

    pub(crate) fn fresh(&mut self, point: FloatPoint) -> LocatedCorner {
        LocatedCorner {
            point,
            id: self.new_corner_id(),
        }
    }

    pub(crate) fn same_as_current_from_point(&self) -> LocatedCorner {
        LocatedCorner {
            point: self.current_from_point,
            id: self.current_from_id,
        }
    }

    pub(crate) fn set_current_from_point(&mut self, corner: LocatedCorner) {
        self.current_from_point = corner.point;
        self.current_from_id = corner.id;
    }

    fn calculate_next_trace(
        &mut self,
        board: &mut Board,
        backtrack_array: &[BacktrackElement],
        layer_changed: bool,
        at_fanout_end: bool,
    ) -> ResultItem {
        let mut corner_list: Vec<FloatPoint> = vec![self.current_from_point];
        if !at_fanout_end
            && let Some(adjusted_start_corner) = self.adjust_start_corner(backtrack_array)
        {
            let add_corner = calculate_additional_corner(
                self.current_from_point,
                adjusted_start_corner,
                true,
                self.angle_restriction,
            );
            corner_list.push(add_corner);
            corner_list.push(adjusted_start_corner);
            self.previous_from_point = self.current_from_point;
            let adjusted = self.fresh(adjusted_start_corner);
            self.set_current_from_point(adjusted);
        }
        let mut prev_corner_id = self.current_from_id;
        loop {
            let next_corners = self.calculate_next_trace_corners(backtrack_array);
            if next_corners.is_empty() {
                break;
            }
            for corner in next_corners {
                if corner.id != prev_corner_id {
                    corner_list.push(corner.point);
                    self.previous_from_point = self.current_from_point;
                    self.set_current_from_point(corner);
                    prev_corner_id = corner.id;
                }
            }
        }

        let mut next_layer = self.current_trace_layer;
        if layer_changed {
            self.current_from_door_index = self.current_target_door_index + 1;
            let index = usize::try_from(self.current_from_door_index)
                .expect("the door index is non-negative");
            if let Some(next_room) = backtrack_array[index].next_room {
                next_layer = self
                    .engine
                    .rooms
                    .room_layer(board, next_room)
                    .expect("the backtrack room is live");
            }
        }

        let mut rounded_corner_list: Vec<IntPoint> = Vec::new();
        let mut prev_point: Option<IntPoint> = None;
        for corner in corner_list {
            let current_point = corner.round();
            if Some(current_point) != prev_point {
                rounded_corner_list.push(current_point);
                prev_point = Some(current_point);
            }
        }

        let result = ResultItem::new(rounded_corner_list, self.current_trace_layer);
        self.current_trace_layer = next_layer;
        result
    }

    fn adjust_start_corner(&self, backtrack_array: &[BacktrackElement]) -> Option<FloatPoint> {
        if self.current_from_door_index < 0 {
            return None;
        }
        let index =
            usize::try_from(self.current_from_door_index).expect("checked non-negative above");
        let next_room = backtrack_array[index].next_room?;
        let trace_half_width =
            f64::from(self.ctrl.compensated_trace_half_width[self.current_trace_layer]);
        let room_shape = self
            .engine
            .rooms
            .room_shape(next_room)
            .expect("the backtrack room has a shape");
        let shrinked_room_shape = room_shape.offset(-trace_half_width);
        if shrinked_room_shape.is_empty()
            || shrinked_room_shape.contains_float(&self.current_from_point)
        {
            return None;
        }
        Some(
            shrinked_room_shape
                .nearest_point_approx(&self.current_from_point)
                .expect("a non-empty shape has a nearest point")
                .round()
                .to_float(),
        )
    }

    fn calculate_next_trace_corners(
        &mut self,
        backtrack_array: &[BacktrackElement],
    ) -> Vec<LocatedCorner> {
        match self.kind {
            LocatorKind::FortyFiveDegree => {
                super::locator_45::calculate_next_trace_corners(self, backtrack_array)
            }
            LocatorKind::AnyAngle => {
                super::locator_any_angle::calculate_next_trace_corners(self, backtrack_array)
            }
        }
    }
}
