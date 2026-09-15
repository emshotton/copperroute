use rustc_hash::FxHashMap;

use crate::ids::ItemId;

/// A map from [`ItemId`] to a value, whose iteration order is ascending by id.
#[derive(Debug, Clone)]
pub struct ItemStore<V> {
    map: FxHashMap<ItemId, V>,
    order: Vec<ItemId>,
}

impl<V> ItemStore<V> {
    pub fn new() -> ItemStore<V> {
        ItemStore {
            map: FxHashMap::default(),
            order: Vec::new(),
        }
    }

    pub fn insert(&mut self, id: ItemId, value: V) -> Option<V> {
        let previous = self.map.insert(id, value);
        if previous.is_none() {
            match self.order.last() {
                Some(last) if *last >= id => {
                    let at = self.order.partition_point(|held| *held < id);
                    self.order.insert(at, id);
                }
                _ => self.order.push(id),
            }
        }
        previous
    }

    pub fn remove(&mut self, id: &ItemId) -> Option<V> {
        let removed = self.map.remove(id)?;
        let at = self
            .order
            .binary_search(id)
            .expect("every mapped id is in the order");
        self.order.remove(at);
        Some(removed)
    }

    pub fn get(&self, id: &ItemId) -> Option<&V> {
        self.map.get(id)
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    pub fn get_mut(&mut self, id: &ItemId) -> Option<&mut V> {
        self.map.get_mut(id)
    }

    pub fn contains_key(&self, id: &ItemId) -> bool {
        self.map.contains_key(id)
    }

    /// Yields every value, in no particular order.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.map.values_mut()
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = (ItemId, &V)> + ExactSizeIterator + '_ {
        self.order.iter().map(|id| {
            (
                *id,
                self.map.get(id).expect("every ordered id is in the map"),
            )
        })
    }

    /// Every value in ascending id order. Collected rather than lazy, because the values live in
    /// a hash map that cannot hand out ordered mutable borrows one at a time.
    pub fn ordered_values_mut(&mut self) -> Vec<&mut V> {
        let mut held: Vec<(ItemId, &mut V)> = self
            .map
            .iter_mut()
            .map(|(id, value)| (*id, value))
            .collect();
        held.sort_unstable_by_key(|(id, _)| *id);
        held.into_iter().map(|(_, value)| value).collect()
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = ItemId> + ExactSizeIterator + '_ {
        self.order.iter().copied()
    }

    pub fn values(&self) -> impl DoubleEndedIterator<Item = &V> + ExactSizeIterator + '_ {
        self.order
            .iter()
            .map(|id| self.map.get(id).expect("every ordered id is in the map"))
    }
}

impl<V: PartialEq> PartialEq for ItemStore<V> {
    fn eq(&self, other: &ItemStore<V>) -> bool {
        self.map == other.map
    }
}

impl<V: Eq> Eq for ItemStore<V> {}

impl<V> FromIterator<(ItemId, V)> for ItemStore<V> {
    fn from_iter<I: IntoIterator<Item = (ItemId, V)>>(entries: I) -> ItemStore<V> {
        let mut store = ItemStore::new();
        for (id, value) in entries {
            store.insert(id, value);
        }
        store
    }
}

impl<V> Default for ItemStore<V> {
    fn default() -> ItemStore<V> {
        ItemStore::new()
    }
}

impl<V> std::ops::Index<&ItemId> for ItemStore<V> {
    type Output = V;

    fn index(&self, id: &ItemId) -> &V {
        self.get(id)
            .unwrap_or_else(|| panic!("ItemStore: no item {id:?}"))
    }
}
