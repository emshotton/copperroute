use fr_board::{Board, ConnectionId, ItemId, ObstacleRoomId, TreeId};

pub fn is_start_info(board: &mut Board, item: ItemId) -> bool {
    match board.get_item_mut(item) {
        Some(i) => i.get_autoroute_info().start_info,
        None => false,
    }
}

pub fn set_start_info(board: &mut Board, item: ItemId, value: bool) {
    if let Some(i) = board.get_item_mut(item) {
        i.get_autoroute_info().start_info = value;
    }
}

pub fn get_precalculated_connection(board: &mut Board, item: ItemId) -> Option<ConnectionId> {
    board
        .get_item_mut(item)?
        .get_autoroute_info()
        .precalculated_connection
}

pub fn set_precalculated_connection(
    board: &mut Board,
    item: ItemId,
    connection: Option<ConnectionId>,
) {
    if let Some(i) = board.get_item_mut(item) {
        i.get_autoroute_info().precalculated_connection = connection;
    }
}

pub fn get_expansion_room<F>(
    board: &mut Board,
    item: ItemId,
    index: usize,
    autoroute_tree: TreeId,
    create_room: F,
) -> Option<ObstacleRoomId>
where
    F: FnOnce(&mut Board, ItemId, usize, TreeId) -> ObstacleRoomId,
{
    let current_shape_count = board.item_tree_shape_count(item, autoroute_tree);

    let existing = {
        let info = board.get_item_mut(item)?.get_autoroute_info();
        let previous_len = info.expansion_rooms.len();
        let slot = prepare_room_slot(info, current_shape_count, index);
        record_g2(
            item,
            index,
            previous_len,
            current_shape_count,
            slot.is_none(),
        );
        slot?
    };
    if existing.is_some() {
        return existing;
    }

    let room = create_room(board, item, index, autoroute_tree);

    let info = board.get_item_mut(item)?.get_autoroute_info();
    if index < info.expansion_rooms.len() {
        info.expansion_rooms[index] = Some(room);
    }
    Some(room)
}

pub fn reset_doors<F>(board: &mut Board, item: ItemId, mut reset_room_doors: F)
where
    F: FnMut(&mut Board, ObstacleRoomId),
{
    let rooms: Vec<ObstacleRoomId> = match board
        .get_item(item)
        .and_then(|i| i.get_autoroute_info_pur())
    {
        Some(info) => info.expansion_rooms.iter().flatten().copied().collect(),
        None => return,
    };
    for room in rooms {
        reset_room_doors(board, room);
    }
}

fn record_g2(
    item: ItemId,
    index: usize,
    previous_len: usize,
    current_shape_count: usize,
    out_of_range: bool,
) {
    use crate::autoroute::instrument::{Guard, record_guard, record_visit};
    record_visit(Guard::G2RoomArrayResized);
    record_visit(Guard::G2RoomIndexOutOfRange);
    if previous_len > 0 && previous_len != current_shape_count {
        record_guard(
            Guard::G2RoomArrayResized,
            u64::from(item.0),
            previous_len,
            current_shape_count,
            true,
        );
    }
    if out_of_range {
        record_guard(
            Guard::G2RoomIndexOutOfRange,
            u64::from(item.0),
            index,
            current_shape_count,
            current_shape_count > 0,
        );
    }
}

fn prepare_room_slot(
    info: &mut fr_board::AutorouteInfo,
    current_shape_count: usize,
    index: usize,
) -> Option<Option<ObstacleRoomId>> {
    if info.expansion_rooms.len() != current_shape_count {
        info.expansion_rooms.resize(current_shape_count, None);
    }
    if index >= info.expansion_rooms.len() {
        return None;
    }
    Some(info.expansion_rooms[index])
}

#[cfg(test)]
mod tests {
    use super::*;
    use fr_board::AutorouteInfo;

    #[test]
    fn a_null_array_is_allocated_to_the_current_shape_count() {
        let mut info = AutorouteInfo::default();
        assert_eq!(prepare_room_slot(&mut info, 3, 0), Some(None));
        assert_eq!(info.expansion_rooms, vec![None, None, None]);
    }

    #[test]
    fn a_grown_shape_count_preserves_the_existing_rooms() {
        let mut info = AutorouteInfo {
            expansion_rooms: vec![Some(ObstacleRoomId(1)), Some(ObstacleRoomId(2))],
            ..AutorouteInfo::default()
        };
        assert_eq!(
            prepare_room_slot(&mut info, 4, 3),
            Some(None),
            "the new tail slot is empty"
        );
        assert_eq!(
            info.expansion_rooms,
            vec![Some(ObstacleRoomId(1)), Some(ObstacleRoomId(2)), None, None]
        );
    }

    #[test]
    fn a_shrunk_shape_count_truncates() {
        let mut info = AutorouteInfo {
            expansion_rooms: vec![
                Some(ObstacleRoomId(1)),
                Some(ObstacleRoomId(2)),
                Some(ObstacleRoomId(3)),
            ],
            ..AutorouteInfo::default()
        };
        assert_eq!(
            prepare_room_slot(&mut info, 1, 0),
            Some(Some(ObstacleRoomId(1)))
        );
        assert_eq!(info.expansion_rooms, vec![Some(ObstacleRoomId(1))]);
    }

    #[test]
    fn an_out_of_range_index_is_javas_silent_null() {
        let mut info = AutorouteInfo::default();
        assert_eq!(prepare_room_slot(&mut info, 2, 2), None);
        assert_eq!(prepare_room_slot(&mut info, 2, 99), None);
        assert_eq!(
            info.expansion_rooms,
            vec![None, None],
            "the resize still happened before the bounds test"
        );
    }

    #[test]
    fn a_shape_count_of_zero_leaves_an_empty_array_and_every_index_out_of_range() {
        let mut info = AutorouteInfo {
            expansion_rooms: vec![Some(ObstacleRoomId(1))],
            ..AutorouteInfo::default()
        };
        assert_eq!(prepare_room_slot(&mut info, 0, 0), None);
        assert!(info.expansion_rooms.is_empty());
    }

    #[test]
    fn an_already_created_room_is_handed_back_unchanged() {
        let mut info = AutorouteInfo {
            expansion_rooms: vec![Some(ObstacleRoomId(9))],
            ..AutorouteInfo::default()
        };
        assert_eq!(
            prepare_room_slot(&mut info, 1, 0),
            Some(Some(ObstacleRoomId(9)))
        );
    }
}
