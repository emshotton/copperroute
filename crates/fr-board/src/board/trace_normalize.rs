use std::collections::BTreeMap;

use fr_geometry::{IntOctagon, Line, Point, Polyline, TileShape};

use crate::datastructures::StopCheck;
use crate::error::BoardError;
use crate::ids::{ItemId, TreeObject};
use crate::items::Item;

use super::{Board, item_ctx};

/// `P7T8B_CHANGE` — one `CHG` line per `change_trace` call that reaches the identity comparison.
fn p7t8b_change_ledger() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P7T8B_CHANGE").is_some());
    *ON
}

/// `P7T8B_LINES` — adds both polylines' end points to every `CHG` line. Large; off by default.
fn p7t8b_lines_ledger() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P7T8B_LINES").is_some());
    *ON
}

fn join_lines(lines: &[Line]) -> String {
    lines
        .iter()
        .map(|l| format!("({},{})/({},{})", l.a.x, l.a.y, l.b.x, l.b.y))
        .collect::<Vec<_>>()
        .join(",")
}

pub const MAX_NORMALIZATION_DEPTH: u32 = 16;

impl Board {
    pub fn combine_trace(&mut self, id: ItemId) -> Result<bool, BoardError> {
        let mut something_changed = false;
        while self.items.get(&id).is_some_and(Item::is_on_the_board)
            && (self.combine_trace_at_start(id, true)? || self.combine_trace_at_end(id, true)?)
        {
            something_changed = true;
        }
        Ok(something_changed)
    }

    pub fn combine_trace_at_start(
        &mut self,
        id: ItemId,
        ignore_areas: bool,
    ) -> Result<bool, BoardError> {
        let Some(Item::Trace(trace)) = self.items.get(&id) else {
            return Ok(false);
        };
        let Some(start_corner) = trace.first_corner() else {
            return Ok(false);
        };
        let Some(other_id) = self.single_combine_contact(id, &start_corner, ignore_areas) else {
            return Ok(false);
        };
        let Some(reverse_order) =
            self.combine_partner(id, other_id, &start_corner, CombineEnd::Start)
        else {
            return Ok(false);
        };
        self.combine_join(
            id,
            other_id,
            reverse_order,
            CombineEnd::Start,
            &start_corner,
        )
    }

    pub fn combine_trace_at_end(
        &mut self,
        id: ItemId,
        ignore_areas: bool,
    ) -> Result<bool, BoardError> {
        let Some(Item::Trace(trace)) = self.items.get(&id) else {
            return Ok(false);
        };
        let Some(end_corner) = trace.last_corner() else {
            return Ok(false);
        };
        let Some(other_id) = self.single_combine_contact(id, &end_corner, ignore_areas) else {
            return Ok(false);
        };
        let Some(reverse_order) = self.combine_partner(id, other_id, &end_corner, CombineEnd::End)
        else {
            return Ok(false);
        };
        self.combine_join(id, other_id, reverse_order, CombineEnd::End, &end_corner)
    }

    fn single_combine_contact(
        &self,
        id: ItemId,
        corner: &Point,
        ignore_areas: bool,
    ) -> Option<ItemId> {
        let mut contacts: Vec<ItemId> = self
            .trace_normal_contacts_at(id, corner, false)
            .into_iter()
            .collect();
        if ignore_areas {
            contacts.retain(|c| !matches!(self.items.get(c), Some(Item::ConductionArea(_))));
        }
        if contacts.len() != 1 {
            return None;
        }
        Some(contacts[0])
    }

    fn combine_partner(
        &self,
        id: ItemId,
        other_id: ItemId,
        corner: &Point,
        at: CombineEnd,
    ) -> Option<bool> {
        let this_item = self.items.get(&id)?;
        let other_item = self.items.get(&other_id)?;
        let (Item::Trace(this), Item::Trace(other)) = (this_item, other_item) else {
            return None;
        };
        if other.get_layer() != this.get_layer()
            || !other_item.nets_equal(this_item)
            || other.get_half_width() != this.get_half_width()
            || other_item.get_fixed_state() != this_item.get_fixed_state()
            || other_item.is_deletion_forbidden(&self.rules)
            || this_item.is_deletion_forbidden(&self.rules)
        {
            return None;
        }
        let (straight, reversed) = match at {
            CombineEnd::Start => (other.last_corner(), other.first_corner()),
            CombineEnd::End => (other.first_corner(), other.last_corner()),
        };
        if straight.as_ref() == Some(corner) {
            Some(false)
        } else if reversed.as_ref() == Some(corner) {
            Some(true)
        } else {
            None
        }
    }

    fn combine_join(
        &mut self,
        id: ItemId,
        other_id: ItemId,
        reverse_order: bool,
        at: CombineEnd,
        corner: &Point,
    ) -> Result<bool, BoardError> {
        self.save_for_undo(id);
        let (this_lines, layer) = match self.items.get(&id) {
            Some(Item::Trace(t)) => (t.polyline().lines().to_vec(), t.get_layer()),
            _ => return Ok(false),
        };
        let other_lines: Vec<Line> = match self.items.get(&other_id) {
            Some(Item::Trace(t)) if reverse_order => t
                .polyline()
                .lines()
                .iter()
                .rev()
                .map(Line::opposite)
                .collect(),
            Some(Item::Trace(t)) => t.polyline().lines().to_vec(),
            _ => return Ok(false),
        };
        debug_assert!(
            this_lines.len() >= 3 && other_lines.len() >= 3,
            "combine: both polylines have at least three lines, as every board inserter \
             guarantees (BasicBoard.java:185-187)"
        );

        let skip_line = match at {
            CombineEnd::Start => {
                other_lines[other_lines.len() - 2].is_equal_or_opposite(&this_lines[1])
            }
            CombineEnd::End => {
                this_lines[this_lines.len() - 2].is_equal_or_opposite(&other_lines[1])
            }
        };
        let mut new_line_count = this_lines.len() + other_lines.len() - 2;
        if skip_line {
            new_line_count -= 1;
        }
        let (head, tail) = match at {
            CombineEnd::Start => (&other_lines, &this_lines),
            CombineEnd::End => (&this_lines, &other_lines),
        };
        let mut new_lines: Vec<Line> = Vec::with_capacity(new_line_count);
        new_lines.extend_from_slice(&head[..head.len() - 1]);
        if skip_line {
            new_lines.pop();
        }
        new_lines.extend_from_slice(&tail[1..]);
        debug_assert_eq!(new_lines.len(), new_line_count);
        let joined_polyline = Polyline::from_lines(new_lines)?;

        let has_tree_entries = self.trace_has_default_entries(id, other_id);
        if joined_polyline.lines().len() != new_line_count || !has_tree_entries {
            self.replace_trace_geometry(id, joined_polyline);
        } else {
            let mut to_no = match at {
                CombineEnd::Start => other_lines.len(),
                CombineEnd::End => this_lines.len(),
            };
            if skip_line {
                to_no -= 1;
            }
            let from_entry_no = match at {
                CombineEnd::Start => other_lines.len() - 3,
                CombineEnd::End => this_lines.len() - 3,
            };
            match at {
                CombineEnd::Start => {
                    self.merge_trace_entries_in_front(
                        other_id,
                        id,
                        &joined_polyline,
                        from_entry_no,
                        to_no,
                    );
                }
                CombineEnd::End => {
                    self.merge_trace_entries_at_end(
                        other_id,
                        id,
                        &joined_polyline,
                        from_entry_no,
                        to_no,
                    );
                }
            }
            if let Some(other) = self.items.get_mut(&other_id) {
                other.clear_tree_entries();
            }
            if let Some(Item::Trace(this)) = self.items.get_mut(&id) {
                this.set_polyline(joined_polyline);
            }
        }
        let collapsed = matches!(
            self.items.get(&id),
            Some(Item::Trace(t)) if t.polyline().lines().len() < 3
        );
        if collapsed {
            self.remove_item(id);
        }
        self.remove_item(other_id);
        self.join_changed_area(&corner.to_float(), layer);
        Ok(true)
    }

    pub fn split_trace(
        &mut self,
        id: ItemId,
        clip: Option<&IntOctagon>,
    ) -> Result<Vec<ItemId>, BoardError> {
        self.split_trace_checked(id, clip, &|| false)
    }

    pub fn split_trace_checked(
        &mut self,
        id: ItemId,
        clip: Option<&IntOctagon>,
        stop: StopCheck<'_>,
    ) -> Result<Vec<ItemId>, BoardError> {
        let mut result: Vec<ItemId> = Vec::new();
        let Some(item) = self.items.get(&id) else {
            return Ok(result);
        };
        if !item.nets_normal() {
            result.push(id);
            return Ok(result);
        }
        let Item::Trace(trace) = item else {
            return Ok(vec![id]);
        };
        let layer = trace.get_layer();
        let lines = trace.polyline().clone();
        let snapshot = item.clone();
        let is_user_fixed = item.is_user_fixed();
        let net_nos = item.net_nos().to_vec();

        let mut own_trace_split = false;
        for i in 0..lines.lines().len().saturating_sub(2) {
            if let (Some(clip), Item::Trace(snapshot_trace)) = (clip, &snapshot)
                && !snapshot_trace.clip_intersects_segment(i, clip)
            {
                continue;
            }
            let Some(current_shape) = self.split_current_shape(id, &snapshot, i) else {
                continue;
            };
            let Some(current_line_segment) = fr_geometry::LineSegment::from_polyline(&lines, i + 1)
            else {
                continue;
            };
            let mut entry_items: BTreeMap<ItemId, Item> = BTreeMap::new();
            let mut entries =
                self.split_overlapping_entries(&current_shape, layer, &mut entry_items);
            let mut cursor = 0usize;
            while cursor < entries.len() {
                if stop() {
                    return Err(BoardError::Stopped);
                }
                if !self.items.get(&id).is_some_and(Item::is_on_the_board) {
                    return Ok(result);
                }
                let found_entry = entries[cursor];
                cursor += 1;
                let TreeObject::Item(found_id) = found_entry.object else {
                    continue;
                };
                let shape_index = found_entry.shape_index;
                let Some(found_item) = self
                    .items
                    .get(&found_id)
                    .or_else(|| entry_items.get(&found_id))
                else {
                    continue;
                };
                if found_id == id {
                    if shape_index + 1 >= i && shape_index <= i + 1 {
                        continue;
                    }
                    let same = if i < shape_index {
                        lines.corner(i + 1) == lines.corner(shape_index)
                    } else {
                        lines.corner(shape_index + 1) == lines.corner(i)
                    };
                    if same {
                        continue;
                    }
                }
                if !found_item.shares_net_no(&net_nos) {
                    continue;
                }
                match found_item {
                    Item::Trace(found_trace) => {
                        let found_lines = found_trace.polyline().clone();
                        let Some(found_line_segment) =
                            fr_geometry::LineSegment::from_polyline(&found_lines, shape_index + 1)
                        else {
                            continue;
                        };
                        let mut split_pieces: Vec<ItemId> = Vec::new();
                        let mut found_trace_split = false;
                        if found_id != id {
                            let intersecting =
                                found_line_segment.intersection(&current_line_segment);
                            for line in &intersecting {
                                let pieces =
                                    self.split_trace_at_line(found_id, shape_index + 1, line)?;
                                let Some(pieces) = pieces else { continue };
                                for piece in pieces.into_iter().flatten() {
                                    found_trace_split = true;
                                    split_pieces.push(piece);
                                }
                                if found_trace_split {
                                    entries = self.split_overlapping_entries(
                                        &current_shape,
                                        layer,
                                        &mut entry_items,
                                    );
                                    cursor = 0;
                                    break;
                                }
                            }
                            if !found_trace_split {
                                split_pieces.push(found_id);
                            }
                        }
                        let intersecting = current_line_segment.intersection(&found_line_segment);
                        for line in &intersecting {
                            let pieces = self.split_trace_at_line(id, i + 1, line)?;
                            let Some(pieces) = pieces else { continue };
                            own_trace_split = true;
                            for piece in pieces.into_iter().flatten() {
                                result.extend(self.split_trace_checked(piece, clip, stop)?);
                            }
                            break;
                        }
                        if found_trace_split || own_trace_split {
                            for piece in &split_pieces {
                                self.remove_if_cycle_checked(*piece, stop)?;
                            }
                            for piece in result.clone() {
                                self.remove_if_cycle_checked(piece, stop)?;
                            }
                        }
                        if own_trace_split {
                            break;
                        }
                    }
                    Item::Pin(_) | Item::Via(_) => {
                        let Some(split_point) = self.drill_center(found_id) else {
                            continue;
                        };
                        if current_line_segment.contains(&split_point)
                            && let Point::Int(int_point) = split_point
                        {
                            let direction = current_line_segment
                                .get_line()
                                .direction()
                                .turn_45_degree(2);
                            let split_line = Line::from_direction(int_point, &direction);
                            self.split_trace_at_line(id, i + 1, &split_line)?;
                        }
                    }
                    Item::ConductionArea(_) if !is_user_fixed => {
                        let mut ignore_areas = false;
                        if let Some(first_net) = net_nos.first()
                            && let Some(net) = self.rules.nets.get(*first_net)
                        {
                            ignore_areas = self
                                .rules
                                .net_classes
                                .get(net.get_net_class())
                                .get_ignore_cycles_with_areas();
                        }
                        if !ignore_areas
                            && self.trace_start_contacts(id).contains(&found_id)
                            && self.trace_end_contacts(id).contains(&found_id)
                        {
                            // This trace can be removed because of a cycle with the area.
                            self.remove_item(id);
                            return Ok(result);
                        }
                    }
                    _ => {}
                }
            }
            if own_trace_split {
                break;
            }
        }
        if !own_trace_split {
            result.push(id);
        }
        Ok(result)
    }

    fn split_current_shape(
        &mut self,
        id: ItemId,
        snapshot: &Item,
        index: usize,
    ) -> Option<TileShape> {
        if self.items.contains_key(&id) {
            let tree = self.default_tree_id();
            return self.item_tree_shape(id, tree, index);
        }
        let ctx = item_ctx!(self);
        self.trees
            .get_default_tree()
            .calculate_tree_shapes(snapshot, &ctx)
            .get(index)
            .cloned()
            .flatten()
    }

    fn split_overlapping_entries(
        &mut self,
        shape: &TileShape,
        layer: usize,
        entry_items: &mut BTreeMap<ItemId, Item>,
    ) -> Vec<crate::datastructures::TreeEntry<TreeObject>> {
        let ctx = item_ctx!(self);
        let entries = self.trees.get_default_tree().overlapping_tree_entries(
            shape,
            Some(layer),
            &[],
            &self.items,
            &ctx,
        );
        for entry in &entries {
            if let TreeObject::Item(entry_id) = entry.object
                && let Some(item) = self.items.get(&entry_id)
            {
                entry_items.insert(entry_id, item.clone());
            }
        }
        entries
    }

    pub fn split_trace_at_line(
        &mut self,
        id: ItemId,
        line_index: usize,
        new_end_line: &Line,
    ) -> Result<Option<[Option<ItemId>; 2]>, BoardError> {
        let Some(item) = self.items.get(&id) else {
            return Ok(None);
        };
        if !item.is_on_the_board() {
            return Ok(None);
        }
        if item.is_deletion_forbidden(&self.rules) {
            return Ok(None);
        }
        let Item::Trace(trace) = item else {
            return Ok(None);
        };
        let Some(pieces) = trace.split_polyline_at_line(line_index, new_end_line)? else {
            return Ok(None);
        };
        if self.split_inside_drill_pad_prohibited(id, line_index, new_end_line) {
            return Ok(None);
        }
        let Some(Item::Trace(trace)) = self.items.get(&id) else {
            return Ok(None);
        };
        let layer = trace.get_layer();
        let half_width = trace.get_half_width();
        let item = &self.items[&id];
        let net_nos = item.net_nos().to_vec();
        let clearance_class = item.clearance_class();
        let fixed_state = item.get_fixed_state();
        let [first, second] = pieces;
        self.remove_item(id);
        let a = self.insert_trace_without_cleaning(
            first,
            layer,
            half_width,
            net_nos.clone(),
            clearance_class,
            fixed_state,
        );
        let b = self.insert_trace_without_cleaning(
            second,
            layer,
            half_width,
            net_nos,
            clearance_class,
            fixed_state,
        );
        Ok(Some([a, b]))
    }

    pub fn split_trace_at_point(
        &mut self,
        id: ItemId,
        point: &Point,
    ) -> Result<Option<[Option<ItemId>; 2]>, BoardError> {
        let Some(Item::Trace(trace)) = self.items.get(&id) else {
            return Ok(None);
        };
        let segment_count = trace.tile_shape_count();
        for i in 0..segment_count {
            let Some(Item::Trace(trace)) = self.items.get(&id) else {
                return Ok(None);
            };
            let Some(split_line) = trace.perpendicular_split_line(i, point) else {
                continue;
            };
            if let Some(pieces) = self.split_trace_at_line(id, i + 1, &split_line)? {
                return Ok(Some(pieces));
            }
        }
        Ok(None)
    }

    fn split_inside_drill_pad_prohibited(
        &self,
        id: ItemId,
        line_index: usize,
        line: &Line,
    ) -> bool {
        let Some(Item::Trace(trace)) = self.items.get(&id) else {
            return false;
        };
        let Some(this_line) = trace.polyline().lines().get(line_index) else {
            return false;
        };
        let intersection = this_line.intersection(line);
        let layer = trace.get_layer();
        let this_item = &self.items[&id];
        let mut pad_found = false;
        for other_id in self
            .pick_items(&intersection, Some(layer))
            .into_iter()
            .rev()
        {
            let Some(other) = self.items.get(&other_id) else {
                continue;
            };
            if !other.shares_net(this_item) {
                continue;
            }
            match other {
                Item::Pin(_) => {
                    if self.drill_center(other_id).as_ref() == Some(&intersection) {
                        return false;
                    }
                    pad_found = true;
                }
                Item::Trace(other_trace) => {
                    if other_id != id
                        && (other_trace.first_corner().as_ref() == Some(&intersection)
                            || other_trace.last_corner().as_ref() == Some(&intersection))
                    {
                        return false;
                    }
                }
                _ => {}
            }
        }
        pad_found
    }

    pub fn normalize_trace(
        &mut self,
        id: ItemId,
        clip: Option<&IntOctagon>,
    ) -> Result<bool, BoardError> {
        self.normalize_trace_at_depth(id, clip, 0)
    }

    pub fn normalize_trace_checked(
        &mut self,
        id: ItemId,
        clip: Option<&IntOctagon>,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        self.normalize_trace_at_depth_checked(id, clip, 0, stop)
    }

    pub fn normalize_trace_at_depth(
        &mut self,
        id: ItemId,
        clip: Option<&IntOctagon>,
        depth: u32,
    ) -> Result<bool, BoardError> {
        self.normalize_trace_at_depth_checked(id, clip, depth, &|| false)
    }

    pub fn normalize_trace_at_depth_checked(
        &mut self,
        id: ItemId,
        clip: Option<&IntOctagon>,
        depth: u32,
        stop: StopCheck<'_>,
    ) -> Result<bool, BoardError> {
        if stop() {
            return Err(BoardError::Stopped);
        }
        if depth > MAX_NORMALIZATION_DEPTH {
            return Ok(false);
        }
        let split_pieces = self.split_trace_checked(id, clip, stop)?;
        let mut result = split_pieces.len() != 1;
        for piece in split_pieces {
            if !self.items.get(&piece).is_some_and(Item::is_on_the_board) {
                continue;
            }
            let trace_combined = self.combine_trace(piece)?;
            let degenerate = matches!(
                self.items.get(&piece),
                Some(Item::Trace(t))
                    if t.corner_count() == 2 && t.first_corner() == t.last_corner()
            );
            if degenerate {
                if !self.items[&piece].is_deletion_forbidden(&self.rules) {
                    self.remove_item(piece);
                    result = true;
                }
            } else if trace_combined {
                self.normalize_trace_at_depth_checked(piece, clip, depth + 1, stop)?;
                result = true;
            }
        }
        Ok(result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CombineEnd {
    Start,
    End,
}

impl Board {
    pub fn change_trace(&mut self, id: ItemId, new_polyline: Polyline) {
        let Some(item) = self.items.get(&id) else {
            return;
        };
        if !item.is_on_the_board() {
            if let Some(Item::Trace(trace)) = self.items.get_mut(&id) {
                trace.set_polyline(new_polyline);
            }
            return;
        }
        let Item::Trace(trace) = item else {
            return;
        };
        let layer = trace.get_layer();
        let old_lines = trace.polyline().lines().to_vec();
        let new_lines = new_polyline.lines();
        self.save_for_undo(id);

        let last_index = new_lines.len().min(old_lines.len());
        let mut index_of_first_different_line = last_index;
        for i in 0..last_index {
            if !new_lines[i].is_same_object(&old_lines[i]) {
                index_of_first_different_line = i;
                break;
            }
        }
        if p7t8b_change_ledger() {
            let mut last: i64 = -1;
            for i in 1..=last_index {
                if !new_lines[new_lines.len() - i].is_same_object(&old_lines[old_lines.len() - i]) {
                    last = (new_lines.len() - i) as i64;
                    break;
                }
            }
            let ret = if index_of_first_different_line == last_index {
                "early960"
            } else if last < 0 {
                "early972"
            } else {
                "change"
            };
            let keep_s = index_of_first_different_line.saturating_sub(2);
            let keep_e = (new_lines.len() as i64 - last - 3).max(0);
            let idrel: String = (0..last_index)
                .map(|i| {
                    if new_lines[i].is_same_object(&old_lines[i]) {
                        '.'
                    } else {
                        'X'
                    }
                })
                .collect();
            let vrel: String = (0..last_index)
                .map(|i| {
                    if new_lines[i] == old_lines[i] {
                        '.'
                    } else {
                        'X'
                    }
                })
                .collect();
            let mut line = format!(
                "CHG id={} layer={} old={} new={} first={} last={} keepS={} keepE={} ret={} \
                 idrel={idrel} vrel={vrel}",
                id.0,
                layer,
                old_lines.len(),
                new_lines.len(),
                index_of_first_different_line,
                last,
                keep_s,
                keep_e,
                ret,
            );
            if p7t8b_lines_ledger() {
                line.push_str(" old=[");
                line.push_str(&join_lines(&old_lines));
                line.push_str("] new=[");
                line.push_str(&join_lines(new_lines));
                line.push(']');
            }
            eprintln!("{line}");
        }
        if index_of_first_different_line == last_index {
            return;
        }
        let mut index_of_last_different_line: i64 = -1;
        for i in 1..=last_index {
            if !new_lines[new_lines.len() - i].is_same_object(&old_lines[old_lines.len() - i]) {
                index_of_last_different_line = (new_lines.len() - i) as i64;
                break;
            }
        }
        if index_of_last_different_line < 0 {
            return;
        }
        let keep_at_start_count = index_of_first_different_line.saturating_sub(2);
        let keep_at_end_count =
            (new_lines.len() as i64 - index_of_last_different_line - 3).max(0) as usize;
        self.change_trace_entries(id, &new_polyline, keep_at_start_count, keep_at_end_count);
        if let Some(Item::Trace(trace)) = self.items.get_mut(&id) {
            trace.set_polyline(new_polyline);
        }
        let clip_shape = self.changed_area.as_ref().map(|area| area.get_area(layer));
        let _ = self.normalize_trace(id, clip_shape.as_ref());
    }
}
