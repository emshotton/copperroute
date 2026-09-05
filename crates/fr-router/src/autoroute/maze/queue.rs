use fr_board::Board;
use fr_board::structure::Unit;

use crate::autoroute::expansion::ExpandableRef;
use crate::autoroute::maze::{AutorouteControl, AutorouteEngine, MazeListElement};
use crate::java_tree_set::JavaTreeSet;

#[derive(Debug, Clone, Default)]
pub struct MazeQueue {
    set: JavaTreeSet<MazeListElement>,
}

impl MazeQueue {
    pub fn new() -> MazeQueue {
        MazeQueue {
            set: JavaTreeSet::new(),
        }
    }

    pub fn push(
        &mut self,
        element: MazeListElement,
        ctrl: &AutorouteControl,
        engine: &AutorouteEngine,
        board: &Board,
    ) -> bool {
        if !element.sorting_value.is_finite() {
            return false;
        }
        if ctrl.is_fanout
            && let Some(pin_center) = ctrl.fanout_start_pin_center.as_ref()
        {
            let pin_center_float = pin_center.to_float();

            let on_start_layer = element.next_room.is_some_and(|room| {
                engine.rooms.room_layer(board, room).is_some_and(|layer| {
                    i32::try_from(layer).is_ok_and(|layer| layer == ctrl.fanout_start_pin_layer)
                })
            });
            if on_start_layer {
                let max_len = ctrl.fanout_max_escape_length;
                let resolution = board.communication.get_resolution(Unit::Um);
                let entry_point = element.shape_entry.a.middle_point(&element.shape_entry.b);
                let dist = entry_point.distance(&pin_center_float);
                if dist > max_len * resolution {
                    return false;
                }
            }
            if let ExpandableRef::Drill(drill) = element.door {
                let min_len = ctrl.fanout_min_escape_length;
                let resolution = board.communication.get_resolution(Unit::Um);
                let drill_dist = engine
                    .rooms
                    .drills
                    .get(drill.0)
                    .expect("MazeSearchEngine.add: the element's drill (Java holds a reference)")
                    .location
                    .to_float()
                    .distance(&pin_center_float);
                if drill_dist < min_len * resolution {
                    return false;
                }
            }
        }
        let door_id = |door: ExpandableRef| engine.expandable_id_no(door);
        if p7t14b_maze_ledger() {
            let (section, sorting, expansion, id, adjustment) = (
                element.section_no_of_door,
                element.sorting_value,
                element.expansion_value,
                door_id(element.door),
                element.adjustment,
            );
            let added = self.set.add_by(element, |a, b| a.compare_to(b, door_id));
            eprintln!(
                "ADD ok={added} sec={section} sort={sorting:.6} exp={expansion:.6} door={id} \
                 adj={adjustment:?} size={}",
                self.set.len()
            );
            return added;
        }
        self.set.add_by(element, |a, b| a.compare_to(b, door_id))
    }

    pub fn pop_first(&mut self) -> Option<MazeListElement> {
        self.set.poll_first()
    }

    pub fn is_empty(&self) -> bool {
        self.set.is_empty()
    }

    pub fn len(&self) -> usize {
        self.set.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &MazeListElement> {
        self.set.iter()
    }
}

pub(crate) fn p7t14b_maze_ledger() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P7T14B_MAZE").is_some());
    *ON
}
