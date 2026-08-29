//! Port of `autoroute.ItemAutorouteInfo` (ItemAutorouteInfo.java:10-105) — the per-run autoroute
//! scratch every board item carries while a connection is being routed.
//!
//! The **data** lives in `fr-board` ([`fr_board::AutorouteInfo`], reached through
//! `Item::get_autoroute_info`), because `Board::deep_copy` and `Item::clear_derived_data` have to
//! be able to drop it wholesale and plan-6 ruling 10 makes that drop router-observable. The
//! **accessors** live here, as free functions over a `&mut Board`, because
//! `getExpansionRoom(index, tree)` needs `item.treeShapeCount(autorouteTree)` and the engine's
//! obstacle-room arena — neither of which `fr-board` can name (plan-6 ruling 15).
//!
//! Java's `private final Item item` back-pointer (:12) is the `ItemId` argument each function
//! takes. Java's methods are instance methods on an info object the caller fetches first, and
//! **there are two ways to fetch it** — which one a call site uses is observable, because
//! `getAutorouteInfo` allocates an `ItemAutorouteInfo` on an item that has none:
//!
//! * `getAutorouteInfo()` (Item.java:1038-1044), which **creates on demand**, is what all twelve
//!   in-scope call sites but one use: `path/Connection.java:43` (`getPrecalculatedConnection`)
//!   and `:127` (`setPrecalculatedConnection`); `maze/MazeSearchEngine.java:978`
//!   (`setStartInfo(false)`) and `:1012` (`setStartInfo(true)`);
//!   `expansion/TargetItemExpansionDoor.java:46` (`isStartInfo`);
//!   `expansion/SortedRoomNeighbours.java:237,274`,
//!   `expansion/Sorted45DegreeRoomNeighbours.java:136,157` and
//!   `expansion/SortedOrthogonalRoomNeighbours.java:172,194` (all `getExpansionRoom`); plus
//!   `maze/AutorouteEngine.java:329` (`emitDiagnostics`, not ported — and note its `!= null`
//!   guard at :330 is dead, since `getAutorouteInfo` never returns null).
//! * `getAutorouteInfoPur()` (Item.java:1046-1049), which is **nullable and creates nothing**, is
//!   used at exactly one site: `maze/AutorouteEngine.java:662`, inside `resetAllDoors`
//!   (:654-668). It null-checks and then calls `resetDoors()` **and**
//!   `setPrecalculatedConnection(null)` through the same reference, so neither of those two
//!   reaches a fresh info there.
//!
//! The functions below follow their call sites: everything except [`reset_doors`] goes through
//! `Item::get_autoroute_info()` and creates, because that is what its callers do;
//! [`reset_doors`] goes through `get_autoroute_info_pur` and does not.
//!
//! obligation: `AutorouteEngine.resetAllDoors` (Task 9) must **not** call
//! [`set_precalculated_connection`] — it creates, and Java's :662-666 does not, so calling it in a
//! loop over `board.getItems()` would allocate scratch on every item Java skips. Use
//! `Item::get_autoroute_info_pur_mut` (added to `fr-board` in Task 1 for this call site: Java has
//! no such twin only because `getAutorouteInfoPur` already hands back a mutable reference) and
//! write `precalculated_connection = None` through it, under the same `is_some` guard.
//!
//! not ported: `ItemAutorouteInfo.emitDiagnostics` (ItemAutorouteInfo.java:94-104) — it drives
//! `AutorouteDiagnostic.Sink`, a GUI overlay (`global-constraints.md`: no GUI, no observers), and
//! no routing decision reads it.

use fr_board::{Board, ConnectionId, ItemId, ObstacleRoomId, TreeId};

/// Port of `ItemAutorouteInfo.isStartInfo` (ItemAutorouteInfo.java:31-33): whether the item
/// belongs to the start or destination set of the maze search.
///
/// `false` for an item that is not on the board — Java would have NPE'd on the `Item` reference
/// it holds, and no caller can produce an id for an absent item.
pub fn is_start_info(board: &mut Board, item: ItemId) -> bool {
    match board.get_item_mut(item) {
        Some(i) => i.get_autoroute_info().start_info,
        None => false,
    }
}

/// Port of `ItemAutorouteInfo.setStartInfo` (ItemAutorouteInfo.java:39-41).
pub fn set_start_info(board: &mut Board, item: ItemId, value: bool) {
    if let Some(i) = board.get_item_mut(item) {
        i.get_autoroute_info().start_info = value;
    }
}

/// Port of `ItemAutorouteInfo.getPrecalculatedConnection` (ItemAutorouteInfo.java:44-46):
/// `None` where Java returns `null`, i.e. "not yet precalculated".
pub fn get_precalculated_connection(board: &mut Board, item: ItemId) -> Option<ConnectionId> {
    board
        .get_item_mut(item)?
        .get_autoroute_info()
        .precalculated_connection
}

/// Port of `ItemAutorouteInfo.setPrecalculatedConnection` (ItemAutorouteInfo.java:49-51).
/// `None` is Java's `setPrecalculatedConnection(null)`.
pub fn set_precalculated_connection(
    board: &mut Board,
    item: ItemId,
    connection: Option<ConnectionId>,
) {
    if let Some(i) = board.get_item_mut(item) {
        i.get_autoroute_info().precalculated_connection = connection;
    }
}

/// Port of `ItemAutorouteInfo.getExpansionRoom(int, ShapeSearchTree)`
/// (ItemAutorouteInfo.java:54-81): "gets the `ExpansionRoom` of index `index`. Creates it, if it
/// is not yet existing."
///
/// `create_room` is the `new ObstacleExpansionRoom(this.item, index, autorouteTree)` of
/// ItemAutorouteInfo.java:78, which Task 2 supplies: it is `fr-router`'s own room arena that the
/// new room goes into, and this function is the only writer of the item's room slots.
///
/// Two behaviours are load-bearing and both come from HEAD:
///
/// * the **resize-preserving branch** (:59-66, hazard N): when the item's tree-shape count has
///   changed under the router — a trace modified mid-connection — the array is reallocated to the
///   new count and the overlapping prefix `System.arraycopy`d across, rather than the whole thing
///   being thrown away. [`Vec::resize`] with `None` is exactly that: it truncates, or extends
///   with the null slots Java's fresh array starts with.
/// * the **silent out-of-range `null`** (:68-76): an index past the (possibly just resized) array
///   returns `None` instead of panicking. Java logs an `FRLogger.warn` there, which is dropped
///   (`global-constraints.md`); the silent `continue` at the call sites is ported verbatim.
///
/// `index < 0` (:68) is unrepresentable in a `usize` and is therefore folded into the upper-bound
/// test.
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
    // ItemAutorouteInfo.java:55 — `this.item.treeShapeCount(autorouteTree)`, which lazily fills
    // the item's tree shapes for that tree.
    let current_shape_count = board.item_tree_shape_count(item, autoroute_tree);

    let existing = {
        let info = board.get_item_mut(item)?.get_autoroute_info();
        prepare_room_slot(info, current_shape_count, index)?
    };
    if existing.is_some() {
        // ItemAutorouteInfo.java:77-80: only a null slot is filled.
        return existing;
    }

    let room = create_room(board, item, index, autoroute_tree);

    // Re-fetch: `create_room` borrows the board. `ObstacleExpansionRoom`'s constructor reads
    // `item.getTreeShape(index, tree)` for an index this function has already bounds-checked, so
    // it cannot trip `getTreeShape`'s `clearDerivedData()` retry (Item.java:218-221) and cannot
    // drop the scratch out from under us. The length is re-tested anyway, so that a Task 2
    // closure which *did* disturb the item leaves the room unreferenced rather than panicking.
    let info = board.get_item_mut(item)?.get_autoroute_info();
    if index < info.expansion_rooms.len() {
        info.expansion_rooms[index] = Some(room);
    }
    Some(room)
}

/// Port of `ItemAutorouteInfo.resetDoors` (ItemAutorouteInfo.java:84-92): "resets the expansion
/// rooms for autorouting the next connection."
///
/// `reset_room_doors` is the `currentRoom.resetDoors()` of :88, which Task 2 supplies. Java's
/// `expansionRoomArr != null` guard (:85) is an item with no scratch at all, and its
/// `currentRoom != null` guard (:87) is a hole in the array; both are skipped here.
///
/// Unlike the accessors above this one does **not** create the scratch on demand: Java reaches it
/// through an info object the caller already holds, and creating one here would only produce an
/// empty array to iterate.
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

/// The array-maintenance half of [`get_expansion_room`] (ItemAutorouteInfo.java:57-76), split out
/// so the resize branch can be tested without a board.
///
/// Returns `None` for Java's out-of-range `null` (:68-76), and `Some(slot)` otherwise — where
/// `slot` is the room already in the array, or `None` if it still has to be created.
///
/// Java distinguishes a `null` array from a zero-length one (:57 vs :59); `Vec` cannot, and does
/// not need to: for a shape count of 0 both branches end with an empty array, and for a shape
/// count of *n* both end with *n* null slots.
fn prepare_room_slot(
    info: &mut fr_board::AutorouteInfo,
    current_shape_count: usize,
    index: usize,
) -> Option<Option<ObstacleRoomId>> {
    if info.expansion_rooms.len() != current_shape_count {
        // ItemAutorouteInfo.java:59-66 — resize, preserving `min(old, new)` entries.
        info.expansion_rooms.resize(current_shape_count, None);
    }
    if index >= info.expansion_rooms.len() {
        // ItemAutorouteInfo.java:68-76, `FRLogger.warn` dropped.
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
        // ItemAutorouteInfo.java:59-66, the HEAD-only resize branch: `System.arraycopy` of the
        // overlapping prefix into a longer null array.
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
