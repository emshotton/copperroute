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

/// An `autoroute.expansion.ObstacleExpansionRoom`'s identity — the room an item's tree shape
/// carries while a connection is being routed (`ItemAutorouteInfo.expansionRoomArr`,
/// ItemAutorouteInfo.java:20).
///
/// Reserved here, like [`RoomId`], because [`crate::items::AutorouteInfo`] stores it and
/// `fr-board` cannot name `fr-router`'s types (plan-6 ruling 15). It indexes `AutorouteEngine`'s
/// obstacle-room arena, which is a plain `Vec<Option<T>>` with no generation counter (plan-6
/// ruling 16), so a stale id reads a hole exactly as Java's stale reference reads a dead object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObstacleRoomId(pub u32);

/// An `autoroute.path.Connection`'s identity
/// (`ItemAutorouteInfo.precalculatedConnection`, ItemAutorouteInfo.java:17).
///
/// Reserved here for the same reason as [`ObstacleRoomId`]: `fr-router` owns `Connection`, and
/// the id indexes the engine's connection arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectionId(pub u32);

/// A search-tree instance id (`SearchTreeManager`'s per-tree identity, Task 3+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TreeId(pub u32);

/// The two kinds of object the shared `ShapeTree` stores: board items and (from Plan 6)
/// autoroute expansion rooms — Java's `board.searchtree.SearchTreeObject` interface is
/// implemented by both `board.Item` and `autoroute.ExpansionRoom` (plan-rulings.md #2).
///
/// # Ordering
///
/// This is the comparator every search-tree result set is sorted by: `ShapeTree.Leaf.compareTo`
/// (ShapeTree.java:216-223) compares `object.compareTo(other.object)` first and the shape index
/// second, and Java's `TreeSet<Leaf>` (MinAreaTree.java:26) is what `overlaps` returns. So this
/// `Ord` must reproduce the two `compareTo` implementations exactly:
///
/// * `Item.compareTo(Object)` (Item.java:93-103): `item.id - id`, i.e. **`other.id - this.id`** —
///   the subtraction is the wrong way round, so items sort by *descending* id. Against a
///   non-`Item` it returns `1`, i.e. every item is greater than every room.
/// * `CompleteFreeSpaceExpansionRoom.compareTo(Object)`
///   (autoroute/expansion/CompleteFreeSpaceExpansionRoom.java:45-53): `other.id - this.id`
///   against another room — descending id again — and `-1` against anything else, i.e. every
///   room is less than every item. The two agree, so the order is total.
///
/// The result: `Room` before `Item`, and **descending** id within each. `Ord` is therefore
/// hand-written; the derived one would give ascending ids and put items first.
//
// Java bug: the descending order is Item.java:98's reversed subtraction, reproduced here
// because it decides the order the router visits overlapping items in. See
// `crate::items::Item::compare_to` and docs/java-quirks.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TreeObject {
    Item(ItemId),
    Room(RoomId),
}

impl Ord for TreeObject {
    fn cmp(&self, other: &TreeObject) -> std::cmp::Ordering {
        match (self, other) {
            // Item.java:98 / CompleteFreeSpaceExpansionRoom.java:48: `other.id - this.id`.
            (TreeObject::Item(a), TreeObject::Item(b)) => b.cmp(a),
            (TreeObject::Room(a), TreeObject::Room(b)) => b.cmp(a),
            // Item.java:100 returns 1 and CompleteFreeSpaceExpansionRoom.java:50 returns -1.
            (TreeObject::Item(_), TreeObject::Room(_)) => std::cmp::Ordering::Greater,
            (TreeObject::Room(_), TreeObject::Item(_)) => std::cmp::Ordering::Less,
        }
    }
}

impl PartialOrd for TreeObject {
    fn partial_cmp(&self, other: &TreeObject) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Index of a [`crate::rules::NetClass`] in [`crate::rules::NetClasses`].
///
/// Java's `Net.netClass` is a `NetClass` object reference (Net.java:41); the port stores an
/// index instead so no rules object holds a reference to another (Plan 2 design rule).
/// `NetClasses` is a `Vector` in Java (NetClasses.java:10) and `NetClasses.get(int)`
/// (NetClasses.java:18-21) already indexes it, so the index *is* Java's identity for a net
/// class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetClassId(pub usize);

/// Index of a [`crate::rules::ViaInfo`] in [`crate::rules::ViaInfos`].
///
/// Java's `ViaRule.list` is a `List<ViaInfo>` of object references (ViaRule.java:21); the port
/// stores indices into `BoardRules.viaInfos` instead.
///
/// **Removal hazard:** `ViaInfos.remove` (ViaInfos.java:62-64) deletes from the middle of a
/// `List`, which shifts every later index. Java is immune because its `ViaRule`s hold object
/// references; the port is not — see [`crate::rules::ViaInfos::remove`] and the
/// `docs/java-quirks.md` obligation-register row "`ViaInfoId` renumbering across
/// `ViaInfos.remove`". **Discharged in Plan 3 Task 14** by
/// `BoardRules::replace_via_info_renumbering_rules`, which is the only removal path the DSN
/// layer uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ViaInfoId(pub usize);

/// Index of a [`crate::rules::ViaRule`] in `BoardRules::via_rules`.
///
/// Java's `NetClass.viaRule` is a `ViaRule` object reference (NetClass.java:28) and
/// `BoardRules.viaRules` is a `Vector<ViaRule>` (BoardRules.java:26); the port stores the
/// vector index.
///
/// **Removal hazard**, the same shape as [`ViaInfoId`]'s: `Network.addViaRule`
/// (Network.java:414-416) removes a same-named rule from the middle of the vector before
/// appending its replacement, which shifts every later index while Java's object references
/// survive untouched. Go through
/// [`BoardRules::replace_via_rule_renumbering_net_classes`](crate::rules::BoardRules::replace_via_rule_renumbering_net_classes),
/// which rewrites every `NetClass::via_rule` — the only place a `ViaRuleId` is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ViaRuleId(pub usize);

/// Index of a `Padstack` in the board library's `Padstacks` (Task 4 owns both).
///
/// Java's `ViaInfo.padstack` is a `Padstack` object reference (ViaInfo.java:17); the port stores
/// the index, matching Java's own `Padstack.no`/`Padstacks.get(int)` addressing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PadstackId(pub usize);

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
    fn tree_object_ordering_reproduces_the_two_java_compare_tos() {
        // Item.compareTo (Item.java:93-103) is `other.id - this.id`, so items sort by
        // *descending* id, and `compareTo(non-Item)` returns 1 — every item is greater than
        // every room. CompleteFreeSpaceExpansionRoom.compareTo
        // (autoroute/expansion/CompleteFreeSpaceExpansionRoom.java:45-53) agrees: descending id
        // between rooms, and -1 against an item.
        assert!(TreeObject::Item(ItemId(7)) < TreeObject::Item(ItemId(5)));
        assert!(TreeObject::Room(RoomId(1)) < TreeObject::Item(ItemId(7)));
        assert!(TreeObject::Room(RoomId(2)) < TreeObject::Room(RoomId(1)));
    }

    #[test]
    fn tree_object_sets_iterate_rooms_first_then_items_by_descending_id() {
        // This is the order `ShapeTree::overlaps` hands results back in, via `Leaf.compareTo`
        // (ShapeTree.java:216-223).
        let sorted: Vec<TreeObject> = std::collections::BTreeSet::from([
            TreeObject::Item(ItemId(1)),
            TreeObject::Item(ItemId(3)),
            TreeObject::Room(RoomId(1)),
            TreeObject::Room(RoomId(2)),
        ])
        .into_iter()
        .collect();
        assert_eq!(
            sorted,
            vec![
                TreeObject::Room(RoomId(2)),
                TreeObject::Room(RoomId(1)),
                TreeObject::Item(ItemId(3)),
                TreeObject::Item(ItemId(1)),
            ]
        );
    }

    #[test]
    fn item_id_display() {
        assert_eq!(ItemId(42).to_string(), "42");
    }
}
