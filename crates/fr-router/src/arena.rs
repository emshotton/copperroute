//! [`Arena`], the flat store every autoroute object lives in (plan-6 ruling 16).
//!
//! Java's autoroute object graph is cyclic — a room holds its doors, a door holds both of its
//! rooms (`autoroute/expansion/ExpansionDoor.java`), a drill holds one room per layer
//! (`autoroute/drill/ExpansionDrill.java:55-92`) — and `Rc<RefCell<…>>` would make the engine's
//! borrow of a `Send + Sync` `Board` (plan-2 ruling 11) unrepresentable. So every room, door,
//! drill and page lives in one of these instead, addressed by a `u32` index.
//!
//! **No generation counter, and none is wanted.** `slotmap` was considered and rejected (ruling
//! 16): its generational keys would turn a stale index — Java's stale object reference, which is
//! a live read there — into a `None`, and that is a divergence, not a safety improvement. This
//! type reproduces Java's semantics as closely as a flat vector can: a removed slot becomes a
//! permanent hole, and reading a stale index yields `None` rather than someone else's object.

/// A flat, index-addressed store of `T` with permanent holes.
///
/// # Index reuse
///
/// **Indices are never reused.** [`insert`](Arena::insert) always appends, so the index it hands
/// back is `items.len()` before the push and stays valid — or permanently `None` after a
/// [`remove`](Arena::remove) — for the arena's whole life. Java never reuses an object identity
/// either, and the maze search holds indices across mutation (`ItemAutorouteInfo`'s room array,
/// `MazeListElement`'s door ids), so a free list would silently alias a torn-down room onto a
/// live one.
///
/// The cost is that a long routing run's arena grows to the total number of objects ever
/// created, not the live count. That is what Java's heap does too, minus the garbage collector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arena<T> {
    /// Slot `i` holds the value at index `i`, or `None` for a hole.
    items: Vec<Option<T>>,
    /// The number of `Some` slots, so [`len`](Arena::len) is O(1).
    live: usize,
}

impl<T> Arena<T> {
    /// An empty arena.
    pub fn new() -> Self {
        Arena {
            items: Vec::new(),
            live: 0,
        }
    }

    /// An empty arena with room for `capacity` entries before reallocating.
    pub fn with_capacity(capacity: usize) -> Self {
        Arena {
            items: Vec::with_capacity(capacity),
            live: 0,
        }
    }

    /// Appends `value` and returns its index. The index is never reused, even after
    /// [`remove`](Arena::remove).
    ///
    /// # Panics
    /// If the arena already holds `u32::MAX` slots. Java would keep allocating; a board that
    /// creates four billion expansion rooms has failed long before this.
    pub fn insert(&mut self, value: T) -> u32 {
        let index = u32::try_from(self.items.len()).expect("arena index overflowed u32");
        self.items.push(Some(value));
        self.live += 1;
        index
    }

    /// The value at `index`, or `None` for an out-of-range index or a hole.
    pub fn get(&self, index: u32) -> Option<&T> {
        self.items.get(index as usize)?.as_ref()
    }

    /// The value at `index`, mutably.
    pub fn get_mut(&mut self, index: u32) -> Option<&mut T> {
        self.items.get_mut(index as usize)?.as_mut()
    }

    /// Takes the value at `index` out, leaving a permanent hole. Removing twice yields `None`.
    pub fn remove(&mut self, index: u32) -> Option<T> {
        let slot = self.items.get_mut(index as usize)?;
        let taken = slot.take();
        if taken.is_some() {
            self.live -= 1;
        }
        taken
    }

    /// The number of live entries — holes are not counted.
    pub fn len(&self) -> usize {
        self.live
    }

    /// Whether there are no live entries. An arena of nothing but holes is empty.
    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    /// The number of slots ever allocated, live or hole. This is the exclusive upper bound on
    /// every index the arena has ever handed out.
    pub fn slot_count(&self) -> usize {
        self.items.len()
    }

    /// Every live `(index, &value)` pair, in ascending index order, skipping holes.
    pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| slot.as_ref().map(|v| (i as u32, v)))
    }

    /// Every live `(index, &mut value)` pair, in ascending index order, skipping holes.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (u32, &mut T)> {
        self.items
            .iter_mut()
            .enumerate()
            .filter_map(|(i, slot)| slot.as_mut().map(|v| (i as u32, v)))
    }

    /// Drops every entry **and every index**, so the next [`insert`](Arena::insert) returns 0.
    /// Only sound between connections, when no id from the old arena survives.
    pub fn clear(&mut self) {
        self.items.clear();
        self.live = 0;
    }
}

impl<T> Default for Arena<T> {
    fn default() -> Self {
        Arena::new()
    }
}

// =================================================================================================
// The arena index newtypes (plan-6 ruling 16).
//
// Two of the family already live in `fr-board` — `RoomId` (reserved in Plan 2, because the shared
// search tree stores it as a `TreeObject`) and `ObstacleRoomId`/`ConnectionId` (added in Task 1,
// because `AutorouteInfo` stores them). The rest are `fr-router`'s alone and live here.
//
// Every one of them is a **plain arena index with no generation counter**, exactly as ruling 16
// requires: a stale id reads a hole, which is as close as a flat vector gets to Java's stale
// object reference. None of them is a Java `getId()` — those are hashes, computed on demand by
// the room and door types themselves.
// =================================================================================================

/// An `autoroute.expansion.IncompleteFreeSpaceExpansionRoom`'s arena index.
///
/// Incomplete rooms never enter a search tree (they have no completed shape yet), so unlike
/// [`fr_board::RoomId`] this one is not a `TreeObject` and `fr-board` never sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IncompleteRoomId(pub u32);

/// An `autoroute.expansion.ExpansionDoor`'s arena index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DoorId(pub u32);

/// An `autoroute.expansion.TargetItemExpansionDoor`'s arena index.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetDoorId(pub u32);

/// An `autoroute.drill.ExpansionDrill`'s arena index.
///
/// Declared here in Task 2 because `MazeSearchElement.backtrackDoor` is an `ExpandableObject`
/// and `ExpansionDrill` is one of its four implementors, so the enum needs the variant before
/// the drill itself exists.
// (Task 7 adds `ExpansionDrill` itself; this is only its index type, and the class's deferral
// marker lives in `autoroute/drill/`, where `audit-port.sh` looks for it.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DrillId(pub u32);

/// An `autoroute.drill.DrillPage`'s arena index — the fourth `ExpandableObject` implementor.
// (Task 7 adds `DrillPage` itself; this is only its index type.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PageId(pub u32);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_restarts_the_indices() {
        let mut arena: Arena<u8> = Arena::new();
        arena.insert(1);
        arena.insert(2);
        arena.clear();
        assert!(arena.is_empty());
        assert_eq!(arena.slot_count(), 0);
        assert_eq!(arena.insert(3), 0);
    }

    #[test]
    fn slot_count_keeps_counting_holes() {
        let mut arena: Arena<u8> = Arena::new();
        arena.insert(1);
        arena.insert(2);
        arena.remove(0);
        assert_eq!(arena.len(), 1);
        assert_eq!(arena.slot_count(), 2);
    }

    #[test]
    fn iter_mut_visits_the_live_entries_only() {
        let mut arena: Arena<u8> = Arena::new();
        arena.insert(1);
        arena.insert(2);
        arena.insert(3);
        arena.remove(1);
        for (_, v) in arena.iter_mut() {
            *v += 10;
        }
        let seen: Vec<(u32, u8)> = arena.iter().map(|(i, v)| (i, *v)).collect();
        assert_eq!(seen, vec![(0, 11), (2, 13)]);
    }

    #[test]
    fn with_capacity_is_still_empty() {
        let arena: Arena<u8> = Arena::with_capacity(16);
        assert!(arena.is_empty());
        assert_eq!(arena.slot_count(), 0);
    }
}
