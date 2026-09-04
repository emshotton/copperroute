#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Arena<T> {
        items: Vec<Option<T>>,
        live: usize,
}

impl<T> Arena<T> {
        pub fn new() -> Self {
        Arena {
            items: Vec::new(),
            live: 0,
        }
    }

        pub fn with_capacity(capacity: usize) -> Self {
        Arena {
            items: Vec::with_capacity(capacity),
            live: 0,
        }
    }

                            pub fn insert(&mut self, value: T) -> u32 {
        let index = u32::try_from(self.items.len()).expect("arena index overflowed u32");
        self.items.push(Some(value));
        self.live += 1;
        index
    }

        pub fn get(&self, index: u32) -> Option<&T> {
        self.items.get(index as usize)?.as_ref()
    }

        pub fn get_mut(&mut self, index: u32) -> Option<&mut T> {
        self.items.get_mut(index as usize)?.as_mut()
    }

        pub fn remove(&mut self, index: u32) -> Option<T> {
        let slot = self.items.get_mut(index as usize)?;
        let taken = slot.take();
        if taken.is_some() {
            self.live -= 1;
        }
        taken
    }

        pub fn len(&self) -> usize {
        self.live
    }

        pub fn is_empty(&self) -> bool {
        self.live == 0
    }

            pub fn slot_count(&self) -> usize {
        self.items.len()
    }

        pub fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(i, slot)| slot.as_ref().map(|v| (i as u32, v)))
    }

        pub fn iter_mut(&mut self) -> impl Iterator<Item = (u32, &mut T)> {
        self.items
            .iter_mut()
            .enumerate()
            .filter_map(|(i, slot)| slot.as_mut().map(|v| (i as u32, v)))
    }

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


#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IncompleteRoomId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DoorId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetDoorId(pub u32);

pub use fr_board::DrillId;

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
