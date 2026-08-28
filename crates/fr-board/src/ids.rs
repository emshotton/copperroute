//! Item/room/tree object identifiers and the item-id generator.
//!
//! Java: `board/actions/ItemIdGenerator.java`, `datastructures/IdGenerator.java`,
//! `board/searchtree/SearchTreeObject.java` (`getId` — the identity every `TreeObject`
//! variant exposes via its wrapped id).

use std::fmt;

/// A board item's identity: the item id assigned by [`ItemIdGenerator`].
///
/// Java has no dedicated id type — items just carry an `int id`
/// (`SearchTreeObject.getId`, `board/actions/ItemIdGenerator.java`). This newtype exists so
/// `Board` can key `items: BTreeMap<ItemId, Item>` by it (plan-rulings.md #1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId(pub u32);

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// An autoroute expansion room's identity (`autoroute.CompleteFreeSpaceExpansionRoom`).
///
/// Reserved here (plan-rulings.md #2): the shared search tree stores both items and rooms as
/// [`TreeObject`], but `RoomId` is not populated until Plan 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoomId(pub u32);

/// A search-tree instance id (`SearchTreeManager`'s per-tree identity, Task 3+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TreeId(pub u32);

/// The two kinds of object the shared `ShapeTree` stores: board items and (from Plan 6)
/// autoroute expansion rooms — Java's `board.searchtree.SearchTreeObject` interface is
/// implemented by both `board.Item` and `autoroute.ExpansionRoom` (plan-rulings.md #2).
///
/// `Ord`: items order by id, rooms order by id, and every `Item` is less than every `Room`.
/// Deriving `Ord` gives exactly this: Rust orders enum variants first by declaration order
/// (`Item` before `Room`), then by the wrapped field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TreeObject {
    Item(ItemId),
    Room(RoomId),
}

/// `Integer.MAX_VALUE / 2` (ItemIdGenerator.java:23).
const MAX_ID: u32 = i32::MAX as u32 / 2;

/// Port of `ItemIdGenerator` (`board/actions/ItemIdGenerator.java`): creates unique item ids
/// with overflow protection built in.
///
/// The counter starts at 1 and increments monotonically; once it reaches `MAX_ID`
/// (`Integer.MAX_VALUE / 2`, ItemIdGenerator.java:23) the next call wraps back to 1
/// (ItemIdGenerator.java:37-55) instead of overflowing into negative `int` territory.
///
/// not ported: the `wrapAroundCount` diagnostic field and its `FRLogger.warn` call
/// (ItemIdGenerator.java:30,42-50) — `fr-board` must not depend on `tracing`
/// (global-constraints.md); wrapping is silent here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemIdGenerator {
    /// Java `lastGeneratedId` (ItemIdGenerator.java:24), renamed per the task brief.
    last: u32,
}

impl ItemIdGenerator {
    /// A fresh generator; the first `new_id()` call returns `ItemId(1)`, matching Java's
    /// implicit `lastGeneratedId = 0` default.
    pub fn new() -> Self {
        Self::default()
    }

    /// Port of `ItemIdGenerator.newId` (ItemIdGenerator.java:37-55).
    pub fn new_id(&mut self) -> ItemId {
        if self.last >= MAX_ID {
            self.last = 0;
        }
        self.last += 1;
        ItemId(self.last)
    }

    /// Port of `ItemIdGenerator.maxGeneratedId` / `IdGenerator.maxGeneratedId`
    /// (ItemIdGenerator.java:57-61).
    pub fn max_generated_id(&self) -> ItemId {
        ItemId(self.last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generator_starts_at_one() {
        let mut id_gen = ItemIdGenerator::new();
        assert_eq!(id_gen.new_id(), ItemId(1));
        assert_eq!(id_gen.new_id(), ItemId(2));
    }

    #[test]
    fn generator_wraps_like_java() {
        // ItemIdGenerator.java:38: `if (lastGeneratedId >= MAX_ID)` where
        // MAX_ID = Integer.MAX_VALUE / 2 = 1_073_741_823 (ItemIdGenerator.java:23).
        let mut id_gen = ItemIdGenerator {
            last: i32::MAX as u32 / 2,
        };
        assert_eq!(id_gen.new_id(), ItemId(1));
        assert_eq!(id_gen.new_id(), ItemId(2));
    }

    #[test]
    fn generator_does_not_wrap_one_below_max() {
        let mut id_gen = ItemIdGenerator {
            last: i32::MAX as u32 / 2 - 1,
        };
        assert_eq!(id_gen.new_id(), ItemId(i32::MAX as u32 / 2));
    }

    #[test]
    fn max_generated_id_tracks_last() {
        let mut id_gen = ItemIdGenerator::new();
        id_gen.new_id();
        id_gen.new_id();
        assert_eq!(id_gen.max_generated_id(), ItemId(2));
    }

    #[test]
    fn tree_object_ordering() {
        // plan-rulings.md #2: Item < Room; within a variant, order by id.
        assert!(TreeObject::Item(ItemId(5)) < TreeObject::Item(ItemId(7)));
        assert!(TreeObject::Item(ItemId(7)) < TreeObject::Room(RoomId(1)));
        assert!(TreeObject::Room(RoomId(1)) < TreeObject::Room(RoomId(2)));
    }

    #[test]
    fn item_id_display() {
        assert_eq!(ItemId(42).to_string(), "42");
    }
}
