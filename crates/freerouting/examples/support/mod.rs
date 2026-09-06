use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::time::Instant;

use fr_board::{Board, Item, ItemId};

struct Contacts<'a> {
    board: &'a Board,
    entries: BTreeMap<ItemId, Rc<[ItemId]>>,
    hits: usize,
}

impl<'a> Contacts<'a> {
    fn new(board: &'a Board) -> Self {
        Self {
            board,
            entries: BTreeMap::new(),
            hits: 0,
        }
    }

    fn get(&mut self, id: ItemId) -> Rc<[ItemId]> {
        if let Some(contacts) = self.entries.get(&id) {
            self.hits += 1;
            return Rc::clone(contacts);
        }
        let contacts: Rc<[ItemId]> = self
            .board
            .normal_contacts(id)
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .into();
        self.entries.insert(id, Rc::clone(&contacts));
        contacts
    }

    fn visit(
        &mut self,
        id: ItemId,
        visited: &mut BTreeSet<ItemId>,
        target: ItemId,
        previous: ItemId,
        ignore_areas: bool,
    ) -> bool {
        if ignore_areas && matches!(self.board.get_item(id), Some(Item::ConductionArea(_))) {
            return false;
        }
        for &contact in self.get(id).iter() {
            if contact == previous {
                continue;
            }
            if contact == target {
                return true;
            }
            if visited.insert(contact) && self.visit(contact, visited, target, id, ignore_areas) {
                return true;
            }
        }
        false
    }

    fn is_cycle(&mut self, id: ItemId) -> bool {
        let Some(item @ Item::Trace(_)) = self.board.get_item(id) else {
            return false;
        };
        if self.board.is_overlap(id) {
            return true;
        }
        let start = self.board.trace_start_contacts(id);
        let mut visited = start.clone();
        let ignore_areas = item
            .net_nos()
            .first()
            .and_then(|net| self.board.rules.nets.get(*net))
            .is_some_and(|net| {
                self.board
                    .rules
                    .net_classes
                    .get(net.get_net_class())
                    .get_ignore_cycles_with_areas()
            });
        start
            .into_iter()
            .rev()
            .any(|contact| self.visit(contact, &mut visited, id, id, ignore_areas))
    }
}

pub fn benchmark_cache(
    board: &Board,
    query_limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let traces: Vec<_> = board
        .get_traces()
        .into_iter()
        .filter(|&id| board.get_item(id).is_some_and(|item| !item.is_user_fixed()))
        .take(query_limit)
        .collect();
    let mut rows = Vec::new();
    for round in 0..4 {
        let mut cache = Contacts::new(board);
        let (mut baseline, mut cached) = (Vec::new(), Vec::new());
        let (mut baseline_s, mut cached_s) = (0.0, 0.0);
        for use_cache in [round % 2 == 0, round % 2 != 0] {
            let started = Instant::now();
            if use_cache {
                cached = traces
                    .iter()
                    .map(|&id| cache.is_cycle(id))
                    .collect::<Vec<_>>();
                cached_s = started.elapsed().as_secs_f64();
            } else {
                baseline = traces
                    .iter()
                    .map(|&id| board.is_trace_cycle(id))
                    .collect::<Vec<_>>();
                baseline_s = started.elapsed().as_secs_f64();
            }
        }
        if baseline != cached {
            return Err("cached cycle answers differ from reference".into());
        }
        rows.push(serde_json::json!({
            "round": round, "baseline_s": baseline_s, "cached_s": cached_s,
            "cache_hits": cache.hits, "cache_misses": cache.entries.len(),
            "cached_edges": cache.entries.values().map(|v| v.len()).sum::<usize>(),
            "cycles": baseline.iter().filter(|&&v| v).count(),
        }));
    }
    println!(
        "{}",
        serde_json::json!({"queries": traces.len(), "answers_match": true, "rounds": rows})
    );
    Ok(())
}
