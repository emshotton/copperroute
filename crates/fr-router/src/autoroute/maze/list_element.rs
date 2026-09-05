use std::cmp::Ordering;

use fr_geometry::FloatLine;

use crate::autoroute::expansion::{ExpandableRef, RoomRef};
use crate::autoroute::maze::MazeAdjustment;

#[derive(Debug, Clone, PartialEq)]
pub struct MazeListElement {
    pub door: ExpandableRef,
    pub section_no_of_door: i32,
    pub backtrack_door: Option<ExpandableRef>,
    pub section_no_of_backtrack_door: i32,
    pub expansion_value: f64,
    pub sorting_value: f64,
    pub next_room: Option<RoomRef>,
    pub shape_entry: FloatLine,
    pub room_ripped: bool,
    pub adjustment: MazeAdjustment,
    pub already_checked: bool,
    pub ripup_cost: i32,
}

impl MazeListElement {
    pub fn compare_to<F>(&self, other: &MazeListElement, door_id: F) -> Ordering
    where
        F: Fn(ExpandableRef) -> i32,
    {
        if self.sorting_value < other.sorting_value {
            return Ordering::Less;
        }
        if self.sorting_value > other.sorting_value {
            return Ordering::Greater;
        }
        if self.expansion_value < other.expansion_value {
            return Ordering::Less;
        }
        if self.expansion_value > other.expansion_value {
            return Ordering::Greater;
        }
        let id1 = door_id(self.door);
        let id2 = door_id(other.door);
        if id1 < id2 {
            return Ordering::Less;
        }
        if id1 > id2 {
            return Ordering::Greater;
        }
        if self.section_no_of_door < other.section_no_of_door {
            return Ordering::Less;
        }
        if self.section_no_of_door > other.section_no_of_door {
            return Ordering::Greater;
        }
        self.door
            .cmp(&other.door)
            .then_with(|| self.backtrack_door.cmp(&other.backtrack_door))
            .then_with(|| {
                self.section_no_of_backtrack_door
                    .cmp(&other.section_no_of_backtrack_door)
            })
            .then_with(|| self.next_room.cmp(&other.next_room))
            .then_with(|| line_key(&self.shape_entry).cmp(&line_key(&other.shape_entry)))
            .then_with(|| self.room_ripped.cmp(&other.room_ripped))
            .then_with(|| self.adjustment.cmp(&other.adjustment))
            .then_with(|| self.already_checked.cmp(&other.already_checked))
            .then_with(|| self.ripup_cost.cmp(&other.ripup_cost))
    }
}

fn line_key(line: &FloatLine) -> [OrderedF64; 4] {
    [
        OrderedF64(line.a.x),
        OrderedF64(line.a.y),
        OrderedF64(line.b.x),
        OrderedF64(line.b.y),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct OrderedF64(f64);

impl Eq for OrderedF64 {}

impl PartialOrd for OrderedF64 {
    fn partial_cmp(&self, other: &OrderedF64) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedF64 {
    fn cmp(&self, other: &OrderedF64) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}
