use std::collections::{BTreeMap, BTreeSet};

use fr_board::items::Item;
use fr_board::{Board, ItemId};
use fr_geometry::{FloatLine, FloatPoint};


pub fn calculate_airline(
    board: &Board,
    from_items: &BTreeSet<ItemId>,
    to_items: &BTreeSet<ItemId>,
) -> Option<FloatLine> {
    let mut from_corner: Option<FloatPoint> = None;
    let mut to_corner: Option<FloatPoint> = None;
    let mut min_distance = f64::MAX;

    for from_id in from_items {
        let Some(from_center) = board.drill_center(*from_id) else {
            continue;
        };
        let current_from_corner = from_center.to_float();

        for to_id in to_items {
            let Some(to_center) = board.drill_center(*to_id) else {
                continue;
            };
            let current_to_corner = to_center.to_float();
            let current_distance = current_from_corner.distance_square(&current_to_corner);
            if current_distance < min_distance {
                min_distance = current_distance;
                from_corner = Some(current_from_corner);
                to_corner = Some(current_to_corner);
            }
        }
    }

    Some(FloatLine::new(from_corner?, to_corner?))
}


#[derive(Debug, Default)]
pub(crate) struct ItemDistanceCache {
        connectable: BTreeMap<i32, BTreeSet<ItemId>>,
        keys: BTreeMap<ItemId, f64>,
        reference: BTreeMap<ItemId, Option<FloatPoint>>,
}

#[must_use]
pub fn calculate_item_distance(board: &Board, item: ItemId) -> f64 {
    calculate_item_distance_cached(board, item, &mut ItemDistanceCache::default())
}

pub(crate) fn calculate_item_distance_cached(
    board: &Board,
    item: ItemId,
    cache: &mut ItemDistanceCache,
) -> f64 {
    if let Some(known) = cache.keys.get(&item) {
        return *known;
    }
    let distance = item_distance(board, item, cache);
    cache.keys.insert(item, distance);
    distance
}

fn item_distance(board: &Board, item: ItemId, cache: &mut ItemDistanceCache) -> f64 {
    let Some(current) = board.get_item(item) else {
        return f64::MAX;
    };
    if current.net_count() == 0 {
        return f64::MAX;
    }
    let net_number = current.get_net_number(0);
    let connected_set = board.connected_set(item, net_number, false);
    let unconnected_set = unconnected_set_of(board, item, net_number, &connected_set, cache);

    if unconnected_set.is_empty() {
        return 0.0;
    }

    if connected_set.is_empty() {
        let singleton: BTreeSet<ItemId> = [item].into_iter().collect();
        return calculate_min_distance(board, &singleton, &unconnected_set, cache);
    }
    calculate_min_distance(board, &connected_set, &unconnected_set, cache)
}

fn unconnected_set_of(
    board: &Board,
    item: ItemId,
    net_number: i32,
    connected: &BTreeSet<ItemId>,
    cache: &mut ItemDistanceCache,
) -> BTreeSet<ItemId> {
    let mut result = BTreeSet::new();
    let Some(current) = board.get_item(item) else {
        return result;
    };
    if net_number > 0 && !current.contains_net(net_number) {
        return result;
    }
    if net_number > 0 {
        result.extend(connectable_items(board, net_number, cache).iter().copied());
    } else {
        let nets: Vec<i32> = current.net_nos().to_vec();
        for current_net_number in nets {
            result.extend(
                connectable_items(board, current_net_number, cache)
                    .iter()
                    .copied(),
            );
        }
    }
    for id in connected {
        result.remove(id);
    }
    result
}

fn connectable_items<'a>(
    board: &Board,
    net_number: i32,
    cache: &'a mut ItemDistanceCache,
) -> &'a BTreeSet<ItemId> {
    cache.connectable.entry(net_number).or_insert_with(|| {
        board
            .get_connectable_items(net_number)
            .into_iter()
            .collect()
    })
}

fn calculate_min_distance(
    board: &Board,
    from_items: &BTreeSet<ItemId>,
    to_items: &BTreeSet<ItemId>,
    cache: &mut ItemDistanceCache,
) -> f64 {
    let from_points: Vec<FloatPoint> = from_items
        .iter()
        .filter_map(|id| reference_point(board, *id, cache))
        .collect();
    let to_points: Vec<FloatPoint> = to_items
        .iter()
        .filter_map(|id| reference_point(board, *id, cache))
        .collect();

    let mut min_distance = f64::MAX;
    for from_point in &from_points {
        for to_point in &to_points {
            let distance = from_point.distance(to_point);
            if distance < min_distance {
                min_distance = distance;
            }
        }
    }

    min_distance
}

fn reference_point(
    board: &Board,
    item: ItemId,
    cache: &mut ItemDistanceCache,
) -> Option<FloatPoint> {
    if let Some(known) = cache.reference.get(&item) {
        return *known;
    }
    let point = get_item_reference_point(board, item);
    cache.reference.insert(item, point);
    point
}

fn get_item_reference_point(board: &Board, item: ItemId) -> Option<FloatPoint> {
    if let Some(center) = board.drill_center(item) {
        return Some(center.to_float());
    }
    match board.get_item(item)? {
        Item::Trace(trace) => {
            let first = trace.first_corner()?.to_float();
            let last = trace.last_corner()?.to_float();
            Some(FloatPoint::new(
                (first.x + last.x) / 2.0,
                (first.y + last.y) / 2.0,
            ))
        }
        _ => None,
    }
}
