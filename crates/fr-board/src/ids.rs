use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemId(pub u32);

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RoomId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObstacleRoomId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectionId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DrillId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TreeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TreeObject {
    Item(ItemId),
    Room(RoomId),
}

impl Ord for TreeObject {
    fn cmp(&self, other: &TreeObject) -> std::cmp::Ordering {
        match (self, other) {
            (TreeObject::Item(a), TreeObject::Item(b)) => b.cmp(a),
            (TreeObject::Room(a), TreeObject::Room(b)) => b.cmp(a),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetClassId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ViaInfoId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ViaRuleId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PadstackId(pub usize);

const MAX_ID: u32 = i32::MAX as u32 / 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemIdGenerator {
        last: u32,
}

impl ItemIdGenerator {
            pub fn new() -> Self {
        Self::default()
    }

        pub fn new_id(&mut self) -> ItemId {
        if self.last >= MAX_ID {
            self.last = 0;
        }
        self.last += 1;
        ItemId(self.last)
    }

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
        assert!(TreeObject::Item(ItemId(7)) < TreeObject::Item(ItemId(5)));
        assert!(TreeObject::Room(RoomId(1)) < TreeObject::Item(ItemId(7)));
        assert!(TreeObject::Room(RoomId(2)) < TreeObject::Room(RoomId(1)));
    }

    #[test]
    fn tree_object_sets_iterate_rooms_first_then_items_by_descending_id() {
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
