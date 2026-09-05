use fr_board::ids::TreeObject;
use fr_board::{AngleRestriction, ItemCtx, ItemLookup, Node, NodeId, ShapeSearchTree};
use fr_geometry::{IntBox, IntOctagon, Line, LineSegment, RegularTileShape, Side, TileShape};

use crate::autoroute::expansion::{ExpansionRoomStore, IncompleteFreeSpaceExpansionRoom};

pub trait AutorouteSearchTreeExt {
    #[allow(clippy::too_many_arguments)]
    fn complete_shape(
        &self,
        room: &IncompleteFreeSpaceExpansionRoom,
        net_no: i32,
        ignore_object: Option<TreeObject>,
        ignore_shape: Option<&TileShape>,
        items: &impl ItemLookup,
        rooms: &ExpansionRoomStore,
        ctx: &ItemCtx<'_>,
    ) -> Vec<IncompleteFreeSpaceExpansionRoom>;

    fn divide_large_room(
        &self,
        rooms: Vec<IncompleteFreeSpaceExpansionRoom>,
        board_bounds: &IntBox,
    ) -> Vec<IncompleteFreeSpaceExpansionRoom>;
}

impl AutorouteSearchTreeExt for ShapeSearchTree {
    fn complete_shape(
        &self,
        room: &IncompleteFreeSpaceExpansionRoom,
        net_no: i32,
        ignore_object: Option<TreeObject>,
        ignore_shape: Option<&TileShape>,
        items: &impl ItemLookup,
        rooms: &ExpansionRoomStore,
        ctx: &ItemCtx<'_>,
    ) -> Vec<IncompleteFreeSpaceExpansionRoom> {
        match self.angle() {
            AngleRestriction::NinetyDegree => complete_shape_90(
                self,
                room,
                net_no,
                ignore_object,
                ignore_shape,
                items,
                rooms,
                ctx,
            ),
            AngleRestriction::FortyFiveDegree => complete_shape_45(
                self,
                room,
                net_no,
                ignore_object,
                ignore_shape,
                items,
                rooms,
                ctx,
            ),
            AngleRestriction::None => complete_shape_base(
                self,
                room,
                net_no,
                ignore_object,
                ignore_shape,
                items,
                rooms,
                ctx,
            ),
        }
    }

    fn divide_large_room(
        &self,
        rooms: Vec<IncompleteFreeSpaceExpansionRoom>,
        board_bounds: &IntBox,
    ) -> Vec<IncompleteFreeSpaceExpansionRoom> {
        let result = divide_large_room_base(rooms, board_bounds);
        if self.angle() != AngleRestriction::FortyFiveDegree {
            return result;
        }
        let mut result = result;
        for current_room in &mut result {
            let shape = current_room
                .get_shape()
                .and_then(TileShape::bounding_octagon)
                .unwrap_or_else(|| {
                    panic!(
                        "ShapeSearchTree45Degree.divideLargeRoom: a room with no bounding octagon \
                         — Java NPEs at ShapeSearchTree45Degree.java:294"
                    )
                });
            current_room.set_shape(Some(TileShape::Octagon(shape)));
            let contained = current_room
                .get_contained_shape()
                .and_then(TileShape::bounding_octagon)
                .unwrap_or_else(|| {
                    panic!(
                        "ShapeSearchTree45Degree.divideLargeRoom: a room whose contained shape has \
                         no bounding octagon — Java NPEs at ShapeSearchTree45Degree.java:295"
                    )
                });
            current_room.set_contained_shape(Some(TileShape::Octagon(contained)));
        }
        result
    }
}

fn divide_large_room_base(
    room_list: Vec<IncompleteFreeSpaceExpansionRoom>,
    board_bounding_box: &IntBox,
) -> Vec<IncompleteFreeSpaceExpansionRoom> {
    if room_list.len() != 1 {
        return room_list;
    }
    let current_room = &room_list[0];
    let room_bounding_box = current_room
        .get_shape()
        .unwrap_or_else(|| {
            panic!(
                "ShapeSearchTree.divideLargeRoom: a room with no shape — Java NPEs at \
                 ShapeSearchTree.java:1101"
            )
        })
        .bounding_box();
    if room_bounding_box.height().wrapping_mul(2) <= board_bounding_box.height()
        || room_bounding_box.width().wrapping_mul(2) <= board_bounding_box.width()
    {
        return room_list;
    }
    let max_section_width =
        0.5 * f64::from(board_bounding_box.height().max(board_bounding_box.width()));
    let section_arr = current_room
        .get_shape()
        .expect("checked above")
        .divide_into_sections(max_section_width);
    let mut result = Vec::with_capacity(section_arr.len());
    for current_section in section_arr {
        let current_shape_to_be_contained =
            current_section.intersection(current_room.get_contained_shape().unwrap_or_else(|| {
                panic!(
                    "ShapeSearchTree.divideLargeRoom: a room with no contained shape — Java NPEs \
                     at ShapeSearchTree.java:1111"
                )
            }));
        result.push(IncompleteFreeSpaceExpansionRoom::new(
            Some(current_section),
            current_room.get_layer(),
            Some(current_shape_to_be_contained),
        ));
    }
    result
}

fn object_is_trace_obstacle(object: TreeObject, net_no: i32, items: &impl ItemLookup) -> bool {
    match object {
        TreeObject::Item(id) => item_of(items, id).is_trace_obstacle(net_no),
        TreeObject::Room(_) => true,
    }
}

fn object_shape_layer(
    object: TreeObject,
    shape_index: usize,
    items: &impl ItemLookup,
    rooms: &ExpansionRoomStore,
    ctx: &ItemCtx<'_>,
) -> usize {
    match object {
        TreeObject::Item(id) => item_of(items, id).shape_layer(shape_index, ctx),
        TreeObject::Room(id) => room_of(rooms, id).get_layer(),
    }
}

fn object_tree_shape(
    tree: &ShapeSearchTree,
    object: TreeObject,
    shape_index: usize,
    items: &impl ItemLookup,
    rooms: &ExpansionRoomStore,
    ctx: &ItemCtx<'_>,
) -> TileShape {
    match object {
        TreeObject::Item(id) => tree
            .get_tree_shape(item_of(items, id), shape_index, ctx)
            .map(std::borrow::Cow::into_owned)
            .unwrap_or_else(|| {
                panic!(
                    "ShapeSearchTree.completeShape: item {id} has a leaf for shape {shape_index} \
                     but no shape for it — Java NPEs here too"
                )
            }),
        TreeObject::Room(id) => room_of(rooms, id)
            .get_shape()
            .unwrap_or_else(|| {
                panic!(
                    "ShapeSearchTree.completeShape: expansion room {id:?} has a tree leaf but no \
                     shape — Java NPEs here too"
                )
            })
            .clone(),
    }
}

fn item_of(items: &impl ItemLookup, id: fr_board::ItemId) -> &fr_board::Item {
    items.item(id).unwrap_or_else(|| {
        panic!("ShapeSearchTree.completeShape: item {id} has a leaf but is not in the item list")
    })
}

fn room_of(
    rooms: &ExpansionRoomStore,
    id: fr_board::RoomId,
) -> &crate::autoroute::expansion::CompleteFreeSpaceExpansionRoom {
    rooms.complete_room(id).unwrap_or_else(|| {
        panic!(
            "ShapeSearchTree.completeShape: expansion room {id:?} has a tree leaf but no live room \
             in the store — Java would read a stale reference"
        )
    })
}

fn node_bounds(node: &Node<TreeObject>) -> RegularTileShape {
    match node {
        Node::Inner { bounds, .. } | Node::Leaf { bounds, .. } => *bounds,
        Node::Free { .. } => unreachable!("ShapeTree::node rejects freed slots"),
    }
}

#[allow(clippy::too_many_arguments)]
fn complete_shape_base(
    tree: &ShapeSearchTree,
    room: &IncompleteFreeSpaceExpansionRoom,
    net_number: i32,
    ignore_object: Option<TreeObject>,
    ignore_shape: Option<&TileShape>,
    items: &impl ItemLookup,
    rooms: &ExpansionRoomStore,
    ctx: &ItemCtx<'_>,
) -> Vec<IncompleteFreeSpaceExpansionRoom> {
    let Some(contained_shape) = room.get_contained_shape() else {
        return Vec::new();
    };
    let Some(root) = tree.tree().root() else {
        return Vec::new();
    };
    let mut start_shape = TileShape::Box(*ctx.bounding_box);
    if let Some(shape) = room.get_shape() {
        start_shape = start_shape.intersection(shape);
    }
    let mut bounding_shape = tree.tree().bounding_shape(&start_shape).unwrap_or_else(|| {
        panic!(
            "ShapeSearchTree.completeShape: the start shape has no bound in this tree's \
             directions — Java NPEs at ShapeSearchTree.java:617"
        )
    });
    let mut result: Vec<IncompleteFreeSpaceExpansionRoom> = Vec::new();
    if start_shape.dimension() == 2 {
        result.push(IncompleteFreeSpaceExpansionRoom::new(
            Some(start_shape),
            room.get_layer(),
            Some(contained_shape.clone()),
        ));
    }

    let room_layer = room.get_layer();
    let mut overlapping_leaves: Vec<(TreeObject, usize)> = Vec::new();
    let mut node_stack: Vec<NodeId> = vec![root];
    while let Some(current_node) = node_stack.pop() {
        let node = *tree.tree().node(current_node);
        if !node_bounds(&node)
            .to_tile_shape()
            .intersects(&bounding_shape.to_tile_shape())
        {
            continue;
        }
        match node {
            Node::Leaf {
                object,
                shape_index,
                ..
            } => overlapping_leaves.push((object, shape_index)),
            Node::Inner {
                first_child,
                second_child,
                ..
            } => {
                node_stack.push(first_child);
                node_stack.push(second_child);
            }
            Node::Free { .. } => unreachable!("ShapeTree::node rejects freed slots"),
        }
    }
    overlapping_leaves.sort();

    for (current_object, shape_index) in overlapping_leaves {
        if !object_is_trace_obstacle(current_object, net_number, items)
            || object_shape_layer(current_object, shape_index, items, rooms, ctx) != room_layer
            || Some(current_object) == ignore_object
        {
            continue;
        }
        let current_object_shape =
            object_tree_shape(tree, current_object, shape_index, items, rooms, ctx);
        let mut new_result: Vec<IncompleteFreeSpaceExpansionRoom> = Vec::new();
        let mut new_bounding_shape = RegularTileShape::Octagon(IntOctagon::EMPTY);

        for current_incomplete_room in &result {
            let mut something_changed = false;
            let room_shape = current_incomplete_room.get_shape().unwrap_or_else(|| {
                panic!(
                    "ShapeSearchTree.completeShape: an incomplete room with no shape — Java \
                     NPEs at ShapeSearchTree.java:648"
                )
            });
            let intersection = room_shape.intersection(&current_object_shape);
            if intersection.dimension() == 2 {
                let ignore_expansion_room = matches!(current_object, TreeObject::Room(_))
                    && ignore_shape.is_some_and(|shape| shape.contains_tile(&intersection));
                if !ignore_expansion_room {
                    something_changed = true;
                    new_result.extend(restrain_shape_base(
                        current_incomplete_room,
                        &current_object_shape,
                    ));
                    for tmp_room in &new_result {
                        new_bounding_shape = union_of(
                            tree,
                            new_bounding_shape,
                            tmp_room.get_shape().expect("just built with a shape"),
                        );
                    }
                }
            }
            if !something_changed {
                new_result.push(current_incomplete_room.clone());
                new_bounding_shape = union_of(tree, new_bounding_shape, room_shape);
            }
        }
        result = new_result;
        bounding_shape = new_bounding_shape;
    }
    let _ = bounding_shape;
    tree.divide_large_room(result, ctx.bounding_box)
}

fn union_of(
    tree: &ShapeSearchTree,
    accumulated: RegularTileShape,
    shape: &TileShape,
) -> RegularTileShape {
    match tree.tree().bounding_shape(shape) {
        Some(bounds) => accumulated.union(&bounds),
        None => panic!(
            "ShapeSearchTree.completeShape: a restrained room has no bound in this tree's \
             directions — Java NPEs at ShapeSearchTree.java:677"
        ),
    }
}

fn restrain_shape_base(
    incomplete_room: &IncompleteFreeSpaceExpansionRoom,
    obstacle_shape: &TileShape,
) -> Vec<IncompleteFreeSpaceExpansionRoom> {
    let mut result: Vec<IncompleteFreeSpaceExpansionRoom> = Vec::new();
    let obstacle_simplex = TileShape::Simplex(obstacle_shape.to_simplex());

    let shape_to_be_contained = incomplete_room
        .get_contained_shape()
        .map(|shape| TileShape::Simplex(shape.to_simplex()));
    let room_shape = incomplete_room.get_shape();
    let Some(shape_to_be_contained) = shape_to_be_contained else {
        return result;
    };
    if shape_to_be_contained.is_empty() {
        return result;
    }
    let layer = incomplete_room.get_layer();

    let mut cut_line: Option<Line> = None;
    let mut cut_line_distance = -1.0_f64;
    for i in 0..obstacle_simplex.border_line_count() {
        if !room_shape_is_intersected(room_shape, &obstacle_simplex, i) {
            continue;
        }
        let current_line = obstacle_simplex
            .border_line(i)
            .expect("i is below border_line_count");
        let current_min_distance = shape_to_be_contained.distance_to_the_left(&current_line);
        if current_min_distance > cut_line_distance {
            cut_line_distance = current_min_distance;
            cut_line = Some(current_line.opposite());
        }
    }

    if let Some(cut_line) = cut_line {
        let mut result_piece = TileShape::get_instance_from_line(cut_line);
        if let Some(room_shape) = room_shape {
            result_piece = room_shape.intersection(&result_piece);
        }
        if result_piece.dimension() >= 2 {
            result.push(IncompleteFreeSpaceExpansionRoom::new(
                Some(result_piece),
                layer,
                Some(shape_to_be_contained),
            ));
        }
        return result;
    }

    if shape_to_be_contained.dimension() < 1 {
        return result;
    }

    let mut cut_line: Option<Line> = None;
    for i in 0..obstacle_simplex.border_line_count() {
        if !room_shape_is_intersected(room_shape, &obstacle_simplex, i) {
            continue;
        }
        let current_line = obstacle_simplex
            .border_line(i)
            .expect("i is below border_line_count");
        if shape_to_be_contained.side_of_line(&current_line) == Side::Collinear {
            cut_line = Some(current_line.opposite());
            break;
        }
    }
    let Some(cut_line) = cut_line else {
        return result;
    };

    let cut_half_plane = TileShape::get_instance_from_line(cut_line);
    let new_shape_to_be_contained = shape_to_be_contained.intersection(&cut_half_plane);
    let result_piece = match room_shape {
        None => cut_half_plane.clone(),
        Some(room_shape) => room_shape.intersection(&cut_half_plane),
    };
    if result_piece.dimension() >= 2 {
        result.push(IncompleteFreeSpaceExpansionRoom::new(
            Some(result_piece),
            layer,
            Some(new_shape_to_be_contained),
        ));
    }
    let opposite_half_plane = TileShape::get_instance_from_line(cut_line.opposite());
    let rest_piece = match room_shape {
        None => opposite_half_plane.clone(),
        Some(room_shape) => room_shape.intersection(&opposite_half_plane),
    };
    if rest_piece.dimension() >= 2 {
        let rest_shape_to_be_contained = shape_to_be_contained.intersection(&opposite_half_plane);
        let rest_incomplete_room = IncompleteFreeSpaceExpansionRoom::new(
            Some(rest_piece),
            layer,
            Some(rest_shape_to_be_contained),
        );
        result.extend(restrain_shape_base(&rest_incomplete_room, obstacle_shape));
    }
    result
}

fn room_shape_is_intersected(
    room_shape: Option<&TileShape>,
    obstacle_simplex: &TileShape,
    line_no: usize,
) -> bool {
    let room_shape = room_shape.unwrap_or_else(|| {
        panic!(
            "ShapeSearchTree.restrainShape: a whole-plane room reaches \
             isIntersectedInteriorBy — Java NPEs at ShapeSearchTree.java:733"
        )
    });
    match LineSegment::from_tile_shape(obstacle_simplex, line_no) {
        Some(segment) => room_shape.is_intersected_interior_by(&segment),
        None => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn complete_shape_45(
    tree: &ShapeSearchTree,
    room: &IncompleteFreeSpaceExpansionRoom,
    net_number: i32,
    ignore_object: Option<TreeObject>,
    ignore_shape: Option<&TileShape>,
    items: &impl ItemLookup,
    rooms: &ExpansionRoomStore,
    ctx: &ItemCtx<'_>,
) -> Vec<IncompleteFreeSpaceExpansionRoom> {
    let Some(contained_raw) = room.get_contained_shape() else {
        return Vec::new();
    };
    let Some(shape_to_be_contained) = contained_raw.bounding_octagon() else {
        return Vec::new();
    };
    let Some(root) = tree.tree().root() else {
        return Vec::new();
    };
    let mut start_shape = ctx.bounding_box.bounding_octagon();
    if let Some(shape) = room.get_shape() {
        let TileShape::Octagon(_) = shape else {
            return Vec::new();
        };
        start_shape = shape
            .bounding_octagon()
            .expect("an IntOctagon has a bounding octagon")
            .intersection(&start_shape);
    }

    if p7t14b_fp_ledger() {
        p7t14b_treefp(tree, root);
    }
    let mut bounding_shape = start_shape;
    let room_layer = room.get_layer();
    let mut result = vec![IncompleteFreeSpaceExpansionRoom::new(
        Some(TileShape::Octagon(start_shape)),
        room_layer,
        Some(TileShape::Octagon(shape_to_be_contained)),
    )];
    let mut node_stack: Vec<NodeId> = vec![root];

    while let Some(current_node) = node_stack.pop() {
        let node = *tree.tree().node(current_node);
        if !node_bounds(&node)
            .to_tile_shape()
            .intersects_octagon(&bounding_shape)
        {
            continue;
        }
        let (current_object, shape_index) = match node {
            Node::Leaf {
                object,
                shape_index,
                ..
            } => (object, shape_index),
            Node::Inner {
                first_child,
                second_child,
                ..
            } => {
                node_stack.push(first_child);
                node_stack.push(second_child);
                continue;
            }
            Node::Free { .. } => unreachable!("ShapeTree::node rejects freed slots"),
        };

        let is_obstacle = object_is_trace_obstacle(current_object, net_number, items);
        let same_layer =
            object_shape_layer(current_object, shape_index, items, rooms, ctx) == room_layer;
        let ignored_object = Some(current_object) == ignore_object;
        if !(is_obstacle && same_layer && !ignored_object) {
            continue;
        }

        let current_object_shape =
            object_tree_shape(tree, current_object, shape_index, items, rooms, ctx)
                .bounding_octagon()
                .unwrap_or_else(|| {
                    panic!(
                        "ShapeSearchTree45Degree.completeShape: an obstacle shape with no \
                         bounding octagon — Java NPEs at ShapeSearchTree45Degree.java:191"
                    )
                });
        if p7t14b_cs_ledger() {
            let kind = match current_object {
                TreeObject::Room(_) => "CompleteFreeSpaceExpansionRoom",
                TreeObject::Item(_) => "Item",
            };
            let mut line = format!(
                "CSO obj={kind} idx={shape_index} oct={} rooms={}",
                p7t14b_oct(&current_object_shape),
                result.len()
            );
            for room in &result {
                let octagon = room
                    .get_shape()
                    .and_then(TileShape::bounding_octagon)
                    .unwrap_or(IntOctagon::EMPTY);
                line.push(' ');
                line.push_str(&p7t14b_oct(&octagon));
            }
            eprintln!("{line}");
        }
        let mut new_result: Vec<IncompleteFreeSpaceExpansionRoom> = Vec::new();
        let mut new_bounding_shape = IntOctagon::EMPTY;
        for current_room in &result {
            let current_shape = match current_room.get_shape() {
                Some(TileShape::Octagon(shape)) => *shape,
                _ => panic!(
                    "ShapeSearchTree45Degree.completeShape: a room whose shape is not an \
                     IntOctagon — Java throws ClassCastException at \
                     ShapeSearchTree45Degree.java:190"
                ),
            };
            if !current_shape.overlaps(&current_object_shape) {
                new_result.push(current_room.clone());
                new_bounding_shape = new_bounding_shape.union_box(&current_shape.bounding_box());
                continue;
            }
            if matches!(current_object, TreeObject::Room(_))
                && let Some(ignore_shape) = ignore_shape
            {
                let intersection = current_shape.intersection(&current_object_shape);
                if ignore_shape.contains_tile(&TileShape::Octagon(intersection)) {
                    if !ignore_shape.contains_tile(&TileShape::Octagon(current_shape)) {
                        new_result.push(current_room.clone());
                        new_bounding_shape =
                            new_bounding_shape.union_box(&current_shape.bounding_box());
                    }
                    continue;
                }
            }
            new_result.extend(restrain_shape_45(current_room, &current_object_shape));
            for tmp_shape in &new_result {
                new_bounding_shape = new_bounding_shape.union_box(
                    &tmp_shape
                        .get_shape()
                        .expect("just built with a shape")
                        .bounding_box(),
                );
            }
        }
        result = new_result;
        bounding_shape = new_bounding_shape;
    }

    let mut result = tree.divide_large_room(result, ctx.bounding_box);
    result.retain(|expansion_room| {
        let contained = expansion_room.get_contained_shape().unwrap_or_else(|| {
            panic!(
                "ShapeSearchTree45Degree.completeShape: a room with no contained shape — Java \
                 NPEs at ShapeSearchTree45Degree.java:279"
            )
        });
        let shape = expansion_room.get_shape().unwrap_or_else(|| {
            panic!(
                "ShapeSearchTree45Degree.completeShape: a room with no shape — Java NPEs at \
                 ShapeSearchTree45Degree.java:279"
            )
        });
        !contained.contains_tile(shape)
    });
    result
}

fn restrain_shape_45(
    incomplete_room: &IncompleteFreeSpaceExpansionRoom,
    obstacle_shape: &IntOctagon,
) -> Vec<IncompleteFreeSpaceExpansionRoom> {
    let mut result: Vec<IncompleteFreeSpaceExpansionRoom> = Vec::new();

    let Some(contained_shape) = incomplete_room.get_contained_shape() else {
        return result;
    };
    if contained_shape.is_empty() {
        return result;
    }
    let shape_to_be_contained = if contained_shape.is_int_octagon() {
        contained_shape.bounding_octagon().unwrap_or_else(|| {
            panic!(
                "ShapeSearchTree45Degree.restrainShape: an isIntOctagon() shape with no bounding \
                 octagon — Java NPEs at ShapeSearchTree45Degree.java:324"
            )
        })
    } else if matches!(contained_shape, TileShape::Simplex(_)) {
        match contained_shape.bounding_octagon() {
            Some(octagon) => octagon,
            None => return Vec::new(),
        }
    } else {
        return Vec::new();
    };

    let room_shape = match incomplete_room.get_shape() {
        Some(shape @ TileShape::Octagon(_)) => shape.bounding_octagon().unwrap_or_else(|| {
            panic!(
                "ShapeSearchTree45Degree.restrainShape: an IntOctagon with no bounding octagon — \
                 Java NPEs at ShapeSearchTree45Degree.java:338"
            )
        }),
        Some(shape @ TileShape::Simplex(_)) => match shape.bounding_octagon() {
            Some(octagon) => octagon,
            None => return Vec::new(),
        },
        _ => return Vec::new(),
    };

    let mut cut_line_distance = -1.0_f64;
    let mut restraining_line_no: i32 = -1;
    for obstacle_line_no in 0..8usize {
        let current_distance =
            signed_line_distance(obstacle_shape, obstacle_line_no, &shape_to_be_contained);
        if current_distance > cut_line_distance
            && obstacle_segment_touches_inside(obstacle_shape, obstacle_line_no, &room_shape)
        {
            cut_line_distance = current_distance;
            restraining_line_no = obstacle_line_no as i32;
        }
    }
    if cut_line_distance >= 0.0 {
        let restrained_shape = calc_outside_restrained_shape(
            obstacle_shape,
            restraining_line_no as usize,
            &room_shape,
        );
        result.push(IncompleteFreeSpaceExpansionRoom::new(
            Some(TileShape::Octagon(restrained_shape)),
            incomplete_room.get_layer(),
            Some(TileShape::Octagon(shape_to_be_contained)),
        ));
        return result;
    }

    if shape_to_be_contained.dimension() < 1 {
        return result;
    }

    let mut restraining_line_no: i32 = -1;
    for obstacle_line_no in 0..8usize {
        if obstacle_segment_touches_inside(obstacle_shape, obstacle_line_no, &room_shape) {
            let current_line = obstacle_shape.border_line(obstacle_line_no);
            if TileShape::Octagon(shape_to_be_contained).side_of_line(&current_line)
                == Side::Collinear
            {
                restraining_line_no = obstacle_line_no as i32;
                break;
            }
        }
    }
    if restraining_line_no < 0 {
        return result;
    }

    let restrained_shape =
        calc_outside_restrained_shape(obstacle_shape, restraining_line_no as usize, &room_shape);
    if restrained_shape.dimension() == 2 {
        let new_shape_to_be_contained = shape_to_be_contained.intersection(&restrained_shape);
        if new_shape_to_be_contained.dimension() > 0 {
            result.push(IncompleteFreeSpaceExpansionRoom::new(
                Some(TileShape::Octagon(restrained_shape)),
                incomplete_room.get_layer(),
                Some(TileShape::Octagon(new_shape_to_be_contained)),
            ));
        }
    }

    let rest_piece =
        calc_inside_restrained_shape(obstacle_shape, restraining_line_no as usize, &room_shape);
    if rest_piece.dimension() >= 2 {
        let rest_shape_to_be_contained = shape_to_be_contained.intersection(&rest_piece);
        if rest_shape_to_be_contained.dimension() >= 0 {
            let rest_incomplete_room = IncompleteFreeSpaceExpansionRoom::new(
                Some(TileShape::Octagon(rest_piece)),
                incomplete_room.get_layer(),
                Some(TileShape::Octagon(rest_shape_to_be_contained)),
            );
            result.extend(restrain_shape_45(&rest_incomplete_room, obstacle_shape));
        }
    }
    result
}

fn obstacle_segment_touches_inside(
    obstacle_shape: &IntOctagon,
    obstacle_border_line_no: usize,
    room_shape: &IntOctagon,
) -> bool {
    let mut current_border_line_no = obstacle_border_line_no;
    let current_obstacle_corner_x = obstacle_shape.corner_x(obstacle_border_line_no);
    let current_obstacle_corner_y = obstacle_shape.corner_y(obstacle_border_line_no);
    for _ in 0..5 {
        if room_shape.side_of_border_line(
            current_obstacle_corner_x,
            current_obstacle_corner_y,
            current_border_line_no,
        ) != Side::OnTheLeft
        {
            return false;
        }
        current_border_line_no = (current_border_line_no + 1) % 8;
    }

    let next_obstacle_border_line_no = (obstacle_border_line_no + 1) % 8;
    let next_obstacle_corner_x = obstacle_shape.corner_x(next_obstacle_border_line_no);
    let next_obstacle_corner_y = obstacle_shape.corner_y(next_obstacle_border_line_no);
    let mut current_border_line_no = (obstacle_border_line_no + 5) % 8;
    for _ in 0..3 {
        if room_shape.side_of_border_line(
            next_obstacle_corner_x,
            next_obstacle_corner_y,
            current_border_line_no,
        ) != Side::OnTheLeft
        {
            return false;
        }
        current_border_line_no = (current_border_line_no + 1) % 8;
    }
    true
}

fn signed_line_distance(
    obstacle_shape: &IntOctagon,
    obstacle_line_no: usize,
    contained_shape: &IntOctagon,
) -> f64 {
    match obstacle_line_no {
        0 => f64::from(obstacle_shape.bottom_y.wrapping_sub(contained_shape.top_y)),
        2 => f64::from(contained_shape.left_x.wrapping_sub(obstacle_shape.right_x)),
        4 => f64::from(contained_shape.bottom_y.wrapping_sub(obstacle_shape.top_y)),
        6 => f64::from(obstacle_shape.left_x.wrapping_sub(contained_shape.right_x)),
        1 => {
            0.5 * f64::from(
                contained_shape
                    .upper_left_diagonal_x
                    .wrapping_sub(obstacle_shape.lower_right_diagonal_x),
            )
        }
        3 => {
            0.5 * f64::from(
                contained_shape
                    .lower_left_diagonal_x
                    .wrapping_sub(obstacle_shape.upper_right_diagonal_x),
            )
        }
        5 => {
            0.5 * f64::from(
                obstacle_shape
                    .upper_left_diagonal_x
                    .wrapping_sub(contained_shape.lower_right_diagonal_x),
            )
        }
        7 => {
            0.5 * f64::from(
                obstacle_shape
                    .lower_left_diagonal_x
                    .wrapping_sub(contained_shape.upper_right_diagonal_x),
            )
        }
        _ => 0.0,
    }
}

fn calc_outside_restrained_shape(
    obstacle_shape: &IntOctagon,
    obstacle_line_no: usize,
    room_shape: &IntOctagon,
) -> IntOctagon {
    let mut result = *room_shape;
    match obstacle_line_no {
        0 => result.top_y = obstacle_shape.bottom_y,
        2 => result.left_x = obstacle_shape.right_x,
        4 => result.bottom_y = obstacle_shape.top_y,
        6 => result.right_x = obstacle_shape.left_x,
        1 => result.upper_left_diagonal_x = obstacle_shape.lower_right_diagonal_x,
        3 => result.lower_left_diagonal_x = obstacle_shape.upper_right_diagonal_x,
        5 => result.lower_right_diagonal_x = obstacle_shape.upper_left_diagonal_x,
        7 => result.upper_right_diagonal_x = obstacle_shape.lower_left_diagonal_x,
        _ => {}
    }
    result.normalize()
}

fn calc_inside_restrained_shape(
    obstacle_shape: &IntOctagon,
    obstacle_line_no: usize,
    room_shape: &IntOctagon,
) -> IntOctagon {
    let mut result = *room_shape;
    match obstacle_line_no {
        0 => result.bottom_y = obstacle_shape.bottom_y,
        2 => result.right_x = obstacle_shape.right_x,
        4 => result.top_y = obstacle_shape.top_y,
        6 => result.left_x = obstacle_shape.left_x,
        1 => result.lower_right_diagonal_x = obstacle_shape.lower_right_diagonal_x,
        3 => result.upper_right_diagonal_x = obstacle_shape.upper_right_diagonal_x,
        5 => result.upper_left_diagonal_x = obstacle_shape.upper_left_diagonal_x,
        7 => result.lower_left_diagonal_x = obstacle_shape.lower_left_diagonal_x,
        _ => {}
    }
    result.normalize()
}

#[allow(clippy::too_many_arguments)]
fn complete_shape_90(
    tree: &ShapeSearchTree,
    room: &IncompleteFreeSpaceExpansionRoom,
    net_number: i32,
    ignore_object: Option<TreeObject>,
    ignore_shape: Option<&TileShape>,
    items: &impl ItemLookup,
    rooms: &ExpansionRoomStore,
    ctx: &ItemCtx<'_>,
) -> Vec<IncompleteFreeSpaceExpansionRoom> {
    let Some(TileShape::Box(shape_to_be_contained)) = room.get_contained_shape() else {
        return Vec::new();
    };
    let Some(root) = tree.tree().root() else {
        return Vec::new();
    };
    let mut start_shape = *ctx.bounding_box;
    if let Some(shape) = room.get_shape() {
        let TileShape::Box(room_box) = shape else {
            return Vec::new();
        };
        start_shape = room_box.intersection(&start_shape);
    }
    let mut bounding_shape = start_shape;
    let room_layer = room.get_layer();
    let mut result = vec![IncompleteFreeSpaceExpansionRoom::new(
        Some(TileShape::Box(start_shape)),
        room_layer,
        Some(TileShape::Box(*shape_to_be_contained)),
    )];

    let mut node_stack: Vec<NodeId> = vec![root];
    while let Some(current_node) = node_stack.pop() {
        let node = *tree.tree().node(current_node);
        if !node_bounds(&node)
            .to_tile_shape()
            .intersects_box(&bounding_shape)
        {
            continue;
        }
        let (current_object, shape_index) = match node {
            Node::Leaf {
                object,
                shape_index,
                ..
            } => (object, shape_index),
            Node::Inner {
                first_child,
                second_child,
                ..
            } => {
                node_stack.push(first_child);
                node_stack.push(second_child);
                continue;
            }
            Node::Free { .. } => unreachable!("ShapeTree::node rejects freed slots"),
        };

        let is_obstacle = object_is_trace_obstacle(current_object, net_number, items);
        let same_layer =
            object_shape_layer(current_object, shape_index, items, rooms, ctx) == room_layer;
        let ignored_object = Some(current_object) == ignore_object;
        if !(is_obstacle && same_layer && !ignored_object) {
            continue;
        }

        let current_object_shape =
            object_tree_shape(tree, current_object, shape_index, items, rooms, ctx).bounding_box();
        let mut new_result: Vec<IncompleteFreeSpaceExpansionRoom> = Vec::new();
        let mut new_bounding_shape = IntBox::EMPTY;
        for current_room in &result {
            let current_shape = match current_room.get_shape() {
                Some(TileShape::Box(shape)) => *shape,
                _ => panic!(
                    "ShapeSearchTree90Degree.completeShape: a room whose shape is not an \
                     IntBox — Java throws ClassCastException at \
                     ShapeSearchTree90Degree.java:109"
                ),
            };
            if !current_shape.overlaps(&current_object_shape) {
                new_result.push(current_room.clone());
                new_bounding_shape = new_bounding_shape.union(&current_shape.bounding_box());
                continue;
            }
            if matches!(current_object, TreeObject::Room(_))
                && let Some(ignore_shape) = ignore_shape
            {
                let intersection = current_shape.intersection(&current_object_shape);
                if ignore_shape.contains_tile(&TileShape::Box(intersection)) {
                    if !ignore_shape.contains_tile(&TileShape::Box(current_shape)) {
                        new_result.push(current_room.clone());
                        new_bounding_shape =
                            new_bounding_shape.union(&current_shape.bounding_box());
                    }
                    continue;
                }
            }
            new_result.extend(restrain_shape_90(current_room, &current_object_shape));
            for tmp_shape in &new_result {
                new_bounding_shape = new_bounding_shape.union(
                    &tmp_shape
                        .get_shape()
                        .expect("just built with a shape")
                        .bounding_box(),
                );
            }
        }
        result = new_result;
        bounding_shape = new_bounding_shape;
    }
    result
}

#[allow(unused_assignments)]
fn restrain_shape_90(
    incomplete_room: &IncompleteFreeSpaceExpansionRoom,
    obstacle_shape: &IntBox,
) -> Vec<IncompleteFreeSpaceExpansionRoom> {
    let mut result: Vec<IncompleteFreeSpaceExpansionRoom> = Vec::new();

    let Some(contained_shape) = incomplete_room.get_contained_shape() else {
        return result;
    };
    if contained_shape.is_empty() {
        return result;
    }
    let room_shape = incomplete_room
        .get_shape()
        .unwrap_or_else(|| {
            panic!(
                "ShapeSearchTree90Degree.restrainShape: a whole-plane room — Java NPEs at \
                 ShapeSearchTree90Degree.java:215"
            )
        })
        .bounding_box();
    let shape_to_be_contained = contained_shape.bounding_box();
    let mut cut_line_distance: i32 = 0;
    let mut restrained_shape: Option<IntBox> = None;

    if room_shape.ll.x < obstacle_shape.ur.x
        && room_shape.ur.x > obstacle_shape.ur.x
        && room_shape.ur.y > obstacle_shape.ll.y
        && room_shape.ll.y < obstacle_shape.ur.y
    {
        let current_distance = shape_to_be_contained.ll.x.wrapping_sub(obstacle_shape.ur.x);
        if current_distance > cut_line_distance {
            cut_line_distance = current_distance;
            restrained_shape = Some(IntBox::from_coords(
                obstacle_shape.ur.x,
                room_shape.ll.y,
                room_shape.ur.x,
                room_shape.ur.y,
            ));
        }
    }
    if room_shape.ll.x < obstacle_shape.ll.x
        && room_shape.ur.x > obstacle_shape.ll.x
        && room_shape.ur.y > obstacle_shape.ll.y
        && room_shape.ll.y < obstacle_shape.ur.y
    {
        let current_distance = obstacle_shape.ll.x.wrapping_sub(shape_to_be_contained.ur.x);
        if current_distance > cut_line_distance {
            cut_line_distance = current_distance;
            restrained_shape = Some(IntBox::from_coords(
                room_shape.ll.x,
                room_shape.ll.y,
                obstacle_shape.ll.x,
                room_shape.ur.y,
            ));
        }
    }
    if room_shape.ll.y < obstacle_shape.ll.y
        && room_shape.ur.y > obstacle_shape.ll.y
        && room_shape.ur.x > obstacle_shape.ll.x
        && room_shape.ll.x < obstacle_shape.ur.x
    {
        let current_distance = obstacle_shape.ll.y.wrapping_sub(shape_to_be_contained.ur.y);
        if current_distance > cut_line_distance {
            cut_line_distance = current_distance;
            restrained_shape = Some(IntBox::from_coords(
                room_shape.ll.x,
                room_shape.ll.y,
                room_shape.ur.x,
                obstacle_shape.ll.y,
            ));
        }
    }
    if room_shape.ll.y < obstacle_shape.ur.y
        && room_shape.ur.y > obstacle_shape.ur.y
        && room_shape.ur.x > obstacle_shape.ll.x
        && room_shape.ll.x < obstacle_shape.ur.x
    {
        let current_distance = shape_to_be_contained.ll.y.wrapping_sub(obstacle_shape.ur.y);
        if current_distance > cut_line_distance {
            cut_line_distance = current_distance;
            restrained_shape = Some(IntBox::from_coords(
                room_shape.ll.x,
                obstacle_shape.ur.y,
                room_shape.ur.x,
                room_shape.ur.y,
            ));
        }
    }
    if let Some(restrained_shape) = restrained_shape {
        result.push(IncompleteFreeSpaceExpansionRoom::new(
            Some(TileShape::Box(restrained_shape)),
            incomplete_room.get_layer(),
            Some(TileShape::Box(shape_to_be_contained)),
        ));
        return result;
    }

    let is = shape_to_be_contained.intersection(obstacle_shape);
    if is.is_empty() {
        return result;
    }
    let (new_shape_1, new_shape_2) =
        if is.ll.x > room_shape.ll.x && is.ll.x == obstacle_shape.ll.x && is.ll.x < room_shape.ur.x
        {
            (
                Some(IntBox::from_coords(
                    room_shape.ll.x,
                    room_shape.ll.y,
                    is.ll.x,
                    room_shape.ur.y,
                )),
                IntBox::from_coords(is.ll.x, room_shape.ll.y, room_shape.ur.x, room_shape.ur.y),
            )
        } else if is.ur.x > room_shape.ll.x
            && is.ur.x == obstacle_shape.ur.x
            && is.ur.x < room_shape.ur.x
        {
            (
                Some(IntBox::from_coords(
                    is.ur.x,
                    room_shape.ll.y,
                    room_shape.ur.x,
                    room_shape.ur.y,
                )),
                IntBox::from_coords(room_shape.ll.x, room_shape.ll.y, is.ur.x, room_shape.ur.y),
            )
        } else if is.ll.y > room_shape.ll.y
            && is.ll.y == obstacle_shape.ll.y
            && is.ll.y < room_shape.ur.y
        {
            (
                Some(IntBox::from_coords(
                    room_shape.ll.x,
                    room_shape.ll.y,
                    room_shape.ur.x,
                    is.ll.y,
                )),
                IntBox::from_coords(room_shape.ll.x, is.ll.y, room_shape.ur.x, room_shape.ur.y),
            )
        } else if is.ur.y > room_shape.ll.y
            && is.ur.y == obstacle_shape.ur.y
            && is.ur.y < room_shape.ur.y
        {
            (
                Some(IntBox::from_coords(
                    room_shape.ll.x,
                    is.ur.y,
                    room_shape.ur.x,
                    room_shape.ur.y,
                )),
                IntBox::from_coords(room_shape.ll.x, room_shape.ll.y, room_shape.ur.x, is.ur.y),
            )
        } else {
            (None, IntBox::EMPTY)
        };

    if let Some(new_shape_1) = new_shape_1 {
        let new_shape_to_be_contained = shape_to_be_contained.intersection(&new_shape_1);
        if new_shape_to_be_contained.dimension() > 0 {
            result.push(IncompleteFreeSpaceExpansionRoom::new(
                Some(TileShape::Box(new_shape_1)),
                incomplete_room.get_layer(),
                Some(TileShape::Box(new_shape_to_be_contained)),
            ));
            let new_incomplete_room = IncompleteFreeSpaceExpansionRoom::new(
                Some(TileShape::Box(new_shape_2)),
                incomplete_room.get_layer(),
                Some(TileShape::Box(
                    shape_to_be_contained.intersection(&new_shape_2),
                )),
            );
            result.extend(restrain_shape_90(&new_incomplete_room, obstacle_shape));
        }
    }
    result
}

fn p7t14b_fp_ledger() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P7T14B_FP").is_some());
    *ON
}

pub(crate) fn p7t14b_cs_ledger() -> bool {
    static ON: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("P7T14B_CS").is_some());
    *ON
}

fn p7t14b_oct(octagon: &IntOctagon) -> String {
    format!(
        "({},{},{},{},{},{},{},{})",
        octagon.left_x,
        octagon.bottom_y,
        octagon.right_x,
        octagon.top_y,
        octagon.upper_left_diagonal_x,
        octagon.lower_right_diagonal_x,
        octagon.lower_left_diagonal_x,
        octagon.upper_right_diagonal_x
    )
}

fn p7t14b_treefp(tree: &ShapeSearchTree, root: NodeId) {
    let mut hash: u64 = 1469598103934665603;
    let mut node_count: u64 = 0;
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        let node = *tree.tree().node(id);
        node_count += 1;
        let bounds = node_bounds(&node).bounding_box();
        let is_leaf = u64::from(matches!(node, Node::Leaf { .. }));
        for field in [
            i64::from(bounds.ll.x) as u64,
            i64::from(bounds.ll.y) as u64,
            i64::from(bounds.ur.x) as u64,
            i64::from(bounds.ur.y) as u64,
            is_leaf,
        ] {
            hash = (hash ^ field).wrapping_mul(1099511628211);
        }
        if let Node::Inner {
            first_child,
            second_child,
            ..
        } = node
        {
            stack.push(first_child);
            stack.push(second_child);
        }
    }
    eprintln!("TREEFP n={node_count} h={hash:x}");
}
