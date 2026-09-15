use copper_board::board::item_store::ItemStore;
use copper_board::ids::ItemId;

#[test]
fn iteration_is_ascending_by_id_whatever_the_insertion_order() {
    let mut store: ItemStore<u32> = ItemStore::new();
    store.insert(ItemId(30), 300);
    store.insert(ItemId(10), 100);
    store.insert(ItemId(20), 200);

    let ids: Vec<ItemId> = store.keys().collect();
    assert_eq!(ids, vec![ItemId(10), ItemId(20), ItemId(30)]);

    let descending: Vec<u32> = store.values().rev().copied().collect();
    assert_eq!(descending, vec![300, 200, 100]);
}

#[test]
fn removing_drops_the_id_from_the_iteration_order() {
    let mut store: ItemStore<u32> = ItemStore::new();
    store.insert(ItemId(1), 10);
    store.insert(ItemId(2), 20);
    store.insert(ItemId(3), 30);

    assert_eq!(store.remove(&ItemId(2)), Some(20));

    assert_eq!(store.keys().collect::<Vec<_>>(), vec![ItemId(1), ItemId(3)]);
    assert_eq!(store.len(), 2);
    assert_eq!(store.get(&ItemId(2)), None);
    assert_eq!(store.remove(&ItemId(2)), None);
}

#[test]
fn reinserting_an_existing_id_replaces_without_duplicating_the_order_entry() {
    let mut store: ItemStore<u32> = ItemStore::new();
    store.insert(ItemId(5), 50);
    assert_eq!(store.insert(ItemId(5), 55), Some(50));

    assert_eq!(store.keys().collect::<Vec<_>>(), vec![ItemId(5)]);
    assert_eq!(store.get(&ItemId(5)), Some(&55));
    assert_eq!(store.len(), 1);
}

#[test]
fn iter_yields_id_and_value_pairs_in_ascending_id_order() {
    let mut store: ItemStore<u32> = ItemStore::new();
    store.insert(ItemId(7), 70);
    store.insert(ItemId(3), 30);

    let pairs: Vec<(ItemId, u32)> = store.iter().map(|(id, v)| (id, *v)).collect();
    assert_eq!(pairs, vec![(ItemId(3), 30), (ItemId(7), 70)]);

    let reversed: Vec<ItemId> = store.iter().rev().map(|(id, _)| id).collect();
    assert_eq!(reversed, vec![ItemId(7), ItemId(3)]);
}

#[test]
fn get_mut_and_values_mut_reach_the_stored_values() {
    let mut store: ItemStore<u32> = ItemStore::new();
    store.insert(ItemId(1), 1);
    store.insert(ItemId(2), 2);

    *store.get_mut(&ItemId(1)).expect("present") = 100;
    for value in store.values_mut() {
        *value += 1;
    }

    assert_eq!(store.get(&ItemId(1)), Some(&101));
    assert_eq!(store.get(&ItemId(2)), Some(&3));
    assert!(store.contains_key(&ItemId(2)));
    assert!(!store.contains_key(&ItemId(9)));
}

#[test]
fn equality_ignores_the_order_values_were_inserted_in() {
    let mut ascending: ItemStore<u32> = ItemStore::new();
    ascending.insert(ItemId(1), 10);
    ascending.insert(ItemId(2), 20);

    let mut descending: ItemStore<u32> = ItemStore::new();
    descending.insert(ItemId(2), 20);
    descending.insert(ItemId(1), 10);

    assert_eq!(ascending, descending);
    assert_eq!(
        ascending.keys().collect::<Vec<_>>(),
        descending.keys().collect::<Vec<_>>()
    );
}

/// The store stands in for a `BTreeMap<ItemId, _>`, so a mixed sequence of edits must leave it
/// reporting exactly what the map would.
#[test]
fn a_mixed_edit_sequence_matches_a_btree_map() {
    use std::collections::BTreeMap;

    let mut oracle: BTreeMap<ItemId, u32> = BTreeMap::new();
    let mut store: ItemStore<u32> = [(ItemId(1), 1), (ItemId(2), 2)].into_iter().collect();
    oracle.insert(ItemId(1), 1);
    oracle.insert(ItemId(2), 2);

    let mut state: u64 = 0x2545F491_4F6CDD1D;
    for step in 0..2000_u32 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let id = ItemId((state >> 33) as u32 % 64);
        if step % 3 == 0 {
            assert_eq!(store.remove(&id), oracle.remove(&id), "remove {id:?}");
        } else {
            assert_eq!(store.insert(id, step), oracle.insert(id, step), "insert {id:?}");
        }
        assert_eq!(store.len(), oracle.len());
        assert_eq!(store.get(&id), oracle.get(&id));
        assert_eq!(
            store.keys().collect::<Vec<_>>(),
            oracle.keys().copied().collect::<Vec<_>>()
        );
    }

    assert_eq!(
        store.iter().map(|(id, v)| (id, *v)).collect::<Vec<_>>(),
        oracle.iter().map(|(id, v)| (*id, *v)).collect::<Vec<_>>()
    );
    assert_eq!(
        store.values().rev().copied().collect::<Vec<_>>(),
        oracle.values().rev().copied().collect::<Vec<_>>()
    );
}

#[test]
fn ordered_values_mut_hands_out_every_value_in_ascending_id_order() {
    let mut store: ItemStore<u32> = ItemStore::new();
    store.insert(ItemId(30), 300);
    store.insert(ItemId(10), 100);
    store.insert(ItemId(20), 200);

    let refs = store.ordered_values_mut();
    let seen: Vec<u32> = refs.iter().map(|value| **value).collect();
    assert_eq!(seen, vec![100, 200, 300]);

    for value in refs {
        *value += 1;
    }
    assert_eq!(store.get(&ItemId(10)), Some(&101));
    assert_eq!(store.get(&ItemId(30)), Some(&301));
}

#[test]
fn indexing_by_id_yields_the_stored_value() {
    let mut store: ItemStore<u32> = ItemStore::new();
    store.insert(ItemId(4), 40);
    assert_eq!(store[&ItemId(4)], 40);
}

#[test]
fn a_default_store_is_empty_even_when_the_value_type_has_no_default() {
    struct NoDefault(#[allow(dead_code)] u8);
    let store: ItemStore<NoDefault> = ItemStore::default();
    assert!(store.is_empty());
}
