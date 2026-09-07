use std::collections::BTreeSet;

use copper_geometry::SplitMix64;
use copper_geometry::int_point::IntPoint;
use copper_geometry::limits::CRIT_INT;
use copper_geometry::point::Point;
use copper_geometry::rational_point::RationalPoint;
use copper_geometry::side::Side;
use num_bigint::BigInt;

use crate::ids::ItemId;

const SEED: i64 = 99;

fn shuffle(list: &mut [CornerId], rng: &mut SplitMix64) {
    let mut i = list.len();
    while i > 1 {
        let j = rng.next_int(i as i32) as usize;
        list.swap(i - 1, j);
        i -= 1;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelaunayCorner {
    pub object: ItemId,
    pub point: Point,
}

impl DelaunayCorner {
    pub fn new(object: ItemId, point: Point) -> DelaunayCorner {
        DelaunayCorner { object, point }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelaunayEdge {
    pub start_point: Point,
    pub start_object: Option<ItemId>,
    pub end_point: Point,
    pub end_object: Option<ItemId>,
}

impl DelaunayEdge {
    pub fn length_square(&self) -> f64 {
        self.end_point
            .to_float()
            .distance_square(&self.start_point.to_float())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CornerId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct EdgeId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct TriangleId(usize);

#[derive(Debug, Clone)]
struct Corner {
    object: Option<ItemId>,
    coor: Point,
}

#[derive(Debug, Clone)]
struct Edge {
    start_corner: CornerId,
    end_corner: CornerId,
    left_triangle: Option<TriangleId>,
    right_triangle: Option<TriangleId>,
}

#[derive(Debug, Clone)]
struct Triangle {
    edge_lines: [EdgeId; 3],
    first_parent: Option<TriangleId>,
    children: Vec<TriangleId>,
    is_on_the_left_of_edge_line: Option<[bool; 3]>,
}

#[derive(Debug, Clone)]
pub struct PlanarDelaunayTriangulation {
    corners: Vec<Corner>,
    edges: Vec<Edge>,
    triangles: Vec<Triangle>,
    anchor: Option<TriangleId>,
    degenerate_edges: Vec<EdgeId>,
}

impl PlanarDelaunayTriangulation {
    pub fn new(corners: &[DelaunayCorner]) -> PlanarDelaunayTriangulation {
        let mut this = PlanarDelaunayTriangulation {
            corners: Vec::with_capacity(corners.len() + 3),
            edges: Vec::new(),
            triangles: Vec::new(),
            anchor: None,
            degenerate_edges: Vec::new(),
        };

        let mut corner_list: Vec<CornerId> = corners
            .iter()
            .map(|corner| this.new_corner(Some(corner.object), corner.point.clone()))
            .collect();

        let mut rng = SplitMix64::new(SEED);
        shuffle(&mut corner_list, &mut rng);

        let bounding_coor = CRIT_INT * 4;
        let bounding_corners = [
            this.new_corner(
                None,
                Point::Int(IntPoint::new(-bounding_coor, -bounding_coor / 2)),
            ),
            this.new_corner(
                None,
                Point::Int(IntPoint::new(bounding_coor, -bounding_coor / 2)),
            ),
            // The third corner's x is a half-integer, not `CRIT_INT`-scaled like the other two,
            // so it can never coincide with a real (always-integer) design coordinate the way an
            // on-grid corner could.
            this.new_corner(
                None,
                Point::Rational(RationalPoint::new(
                    BigInt::from(1),
                    BigInt::from(bounding_coor) * 2,
                    BigInt::from(2),
                )),
            ),
        ];

        let edge_lines = [
            this.new_edge(bounding_corners[0], bounding_corners[1]),
            this.new_edge(bounding_corners[1], bounding_corners[2]),
            this.new_edge(bounding_corners[2], bounding_corners[0]),
        ];

        let start_triangle = this.new_triangle(edge_lines, None);

        for edge in edge_lines {
            this.edges[edge.0].left_triangle = Some(start_triangle);
        }

        this.graph_insert(start_triangle, None);

        for current_corner in corner_list {
            match this.position_locate(current_corner) {
                Some(triangle_to_split) => {
                    this.split(triangle_to_split, current_corner);
                }
                None => debug_assert!(false, "no triangle contains the corner being inserted"),
            }
        }

        this
    }

    pub fn get_edge_lines(&self) -> Vec<DelaunayEdge> {
        let mut result = Vec::new();
        for &edge in &self.degenerate_edges {
            result.push(self.result_edge(edge));
        }
        if let Some(anchor) = self.anchor {
            let mut result_edges: BTreeSet<EdgeId> = BTreeSet::new();
            self.get_leaf_edges(anchor, &mut result_edges);
            for edge in result_edges {
                result.push(self.result_edge(edge));
            }
        }
        result
    }

    pub fn validate(&self) -> bool {
        match self.anchor {
            Some(anchor) => self.validate_triangle(anchor),
            None => {
                debug_assert!(false, "validate on a triangulation with no anchor");
                false
            }
        }
    }

    fn new_corner(&mut self, object: Option<ItemId>, coor: Point) -> CornerId {
        self.corners.push(Corner { object, coor });
        CornerId(self.corners.len() - 1)
    }

    fn new_edge(&mut self, start_corner: CornerId, end_corner: CornerId) -> EdgeId {
        self.edges.push(Edge {
            start_corner,
            end_corner,
            left_triangle: None,
            right_triangle: None,
        });
        EdgeId(self.edges.len() - 1)
    }

    fn new_triangle(
        &mut self,
        edge_lines: [EdgeId; 3],
        first_parent: Option<TriangleId>,
    ) -> TriangleId {
        self.triangles.push(Triangle {
            edge_lines,
            first_parent,
            children: Vec::new(),
            is_on_the_left_of_edge_line: None,
        });
        TriangleId(self.triangles.len() - 1)
    }

    fn result_edge(&self, edge: EdgeId) -> DelaunayEdge {
        let edge = &self.edges[edge.0];
        let start = &self.corners[edge.start_corner.0];
        let end = &self.corners[edge.end_corner.0];
        DelaunayEdge {
            start_point: start.coor.clone(),
            start_object: start.object,
            end_point: end.coor.clone(),
            end_object: end.object,
        }
    }

    fn split(&mut self, triangle: TriangleId, corner: CornerId) -> bool {
        let mut containing_edge: Option<EdgeId> = None;
        for i in 0..3 {
            let current_edge = self.triangles[triangle.0].edge_lines[i];
            let (start, end, left) = {
                let edge = &self.edges[current_edge.0];
                (edge.start_corner, edge.end_corner, edge.left_triangle)
            };
            let current_side = if left == Some(triangle) {
                self.side_of(corner, start, end)
            } else {
                self.side_of(corner, end, start)
            };
            if current_side == Side::OnTheRight {
                return false;
            } else if current_side == Side::Collinear {
                if let Some(containing) = containing_edge {
                    let Some(common_corner) = self.common_corner(current_edge, containing) else {
                        return false;
                    };
                    if self.corners[corner.0].object == self.corners[common_corner.0].object {
                        return false;
                    }
                    let degenerate = self.new_edge(corner, common_corner);
                    self.degenerate_edges.push(degenerate);
                    return true;
                }
                containing_edge = Some(current_edge);
            }
        }

        match containing_edge {
            None => {
                let Some(new_triangles) = self.split_at_inner_point(triangle, corner) else {
                    return false;
                };
                for current_triangle in new_triangles {
                    self.graph_insert(current_triangle, Some(triangle));
                }
                for i in 0..3 {
                    let edge = self.triangles[triangle.0].edge_lines[i];
                    self.legalize_edge(corner, edge);
                }
            }
            Some(containing_edge) => {
                let Some(neighbour_to_split) = self.other_neighbour(containing_edge, triangle)
                else {
                    debug_assert!(false, "containing edge has no second neighbour triangle");
                    return false;
                };

                let Some(new_triangles) =
                    self.split_at_border_point(triangle, corner, neighbour_to_split)
                else {
                    return false;
                };

                self.graph_insert(new_triangles[0], Some(triangle));
                self.graph_insert(new_triangles[1], Some(triangle));
                self.graph_insert(new_triangles[2], Some(neighbour_to_split));
                self.graph_insert(new_triangles[3], Some(neighbour_to_split));

                for i in 0..3 {
                    let current_edge = self.triangles[triangle.0].edge_lines[i];
                    if current_edge != containing_edge {
                        self.legalize_edge(corner, current_edge);
                    }
                }
                for i in 0..3 {
                    let current_edge = self.triangles[neighbour_to_split.0].edge_lines[i];
                    if current_edge != containing_edge {
                        self.legalize_edge(corner, current_edge);
                    }
                }
            }
        }
        true
    }

    fn legalize_edge(&mut self, corner: CornerId, edge: EdgeId) -> bool {
        if self.is_legal(edge) {
            return false;
        }
        let neighbours = {
            let e = &self.edges[edge.0];
            (e.left_triangle, e.right_triangle)
        };
        let (Some(left_triangle), Some(right_triangle)) = neighbours else {
            debug_assert!(false, "an illegal edge always has two neighbour triangles");
            return false;
        };

        let triangle_to_change = if self.opposite_corner(left_triangle, edge) == Some(corner) {
            right_triangle
        } else if self.opposite_corner(right_triangle, edge) == Some(corner) {
            left_triangle
        } else {
            // FRLogger.warn("PlanarDelaunayTriangulation.legalize_edge: edge lines inconsistent")
            return false;
        };

        let Some(flipped_edge) = self.flip(edge) else {
            debug_assert!(false, "flip failed on an edge with two neighbour triangles");
            return false;
        };

        let (flipped_left, flipped_right) = {
            let e = &self.edges[flipped_edge.0];
            (e.left_triangle, e.right_triangle)
        };
        let (Some(flipped_left), Some(flipped_right)) = (flipped_left, flipped_right) else {
            debug_assert!(false, "a flipped edge always has two neighbour triangles");
            return false;
        };
        self.graph_insert(flipped_left, Some(left_triangle));
        self.graph_insert(flipped_right, Some(left_triangle));
        self.graph_insert(flipped_left, Some(right_triangle));
        self.graph_insert(flipped_right, Some(right_triangle));

        for i in 0..3 {
            let current_edge = self.triangles[triangle_to_change.0].edge_lines[i];
            if current_edge != edge {
                self.legalize_edge(corner, current_edge);
            }
        }
        true
    }

    fn side_of(&self, corner: CornerId, p1: CornerId, p2: CornerId) -> Side {
        self.corners[corner.0]
            .coor
            .side_of(&self.corners[p1.0].coor, &self.corners[p2.0].coor)
    }

    fn graph_insert(&mut self, triangle: TriangleId, parent: Option<TriangleId>) {
        self.initialize_is_on_the_left_of_edge_line_array(triangle);
        match parent {
            None => self.anchor = Some(triangle),
            Some(parent) => self.triangles[parent.0].children.push(triangle),
        }
    }

    fn position_locate(&self, corner: CornerId) -> Option<TriangleId> {
        let anchor = self.anchor?;
        if self.triangles[anchor.0].children.is_empty() {
            return Some(anchor);
        }
        for index in 0..self.triangles[anchor.0].children.len() {
            let current_child = self.triangles[anchor.0].children[index];
            if let Some(result) = self.position_locate_reku(corner, current_child) {
                return Some(result);
            }
        }
        // FRLogger.warn("TriangleGraph.position_locate: containing triangle not found")
        None
    }

    fn position_locate_reku(&self, corner: CornerId, triangle: TriangleId) -> Option<TriangleId> {
        if !self.contains(triangle, corner) {
            return None;
        }
        if self.is_leaf(triangle) {
            return Some(triangle);
        }
        for index in 0..self.triangles[triangle.0].children.len() {
            let current_child = self.triangles[triangle.0].children[index];
            if let Some(result) = self.position_locate_reku(corner, current_child) {
                return Some(result);
            }
        }
        // FRLogger.warn("TriangleGraph.position_locate_reku: containing triangle not found")
        None
    }

    fn get_left_triangle(&self, edge: EdgeId) -> Option<TriangleId> {
        self.edges[edge.0].left_triangle
    }

    fn set_left_triangle(&mut self, edge: EdgeId, triangle: Option<TriangleId>) {
        self.edges[edge.0].left_triangle = triangle;
    }

    #[allow(dead_code)]
    fn get_right_triangle(&self, edge: EdgeId) -> Option<TriangleId> {
        self.edges[edge.0].right_triangle
    }

    fn set_right_triangle(&mut self, edge: EdgeId, triangle: Option<TriangleId>) {
        self.edges[edge.0].right_triangle = triangle;
    }

    fn common_corner(&self, edge: EdgeId, other: EdgeId) -> Option<CornerId> {
        let this = &self.edges[edge.0];
        let other = &self.edges[other.0];
        if other.start_corner == this.start_corner || other.end_corner == this.start_corner {
            Some(this.start_corner)
        } else if other.start_corner == this.end_corner || other.end_corner == this.end_corner {
            Some(this.end_corner)
        } else {
            None
        }
    }

    fn other_neighbour(&self, edge: EdgeId, triangle: TriangleId) -> Option<TriangleId> {
        let e = &self.edges[edge.0];
        if e.left_triangle == Some(triangle) {
            e.right_triangle
        } else if e.right_triangle == Some(triangle) {
            e.left_triangle
        } else {
            // FRLogger.warn("Edge.other_neighbour: inconsistent neighbour triangle")
            None
        }
    }

    fn is_legal(&self, edge: EdgeId) -> bool {
        let (start_corner, end_corner, left, right) = {
            let e = &self.edges[edge.0];
            (
                e.start_corner,
                e.end_corner,
                e.left_triangle,
                e.right_triangle,
            )
        };
        let (Some(left), Some(right)) = (left, right) else {
            return true;
        };
        let (Some(left_opposite), Some(right_opposite)) = (
            self.opposite_corner(left, edge),
            self.opposite_corner(right, edge),
        ) else {
            debug_assert!(
                false,
                "an edge is always an edge line of both its neighbours"
            );
            return true;
        };

        let inside_circle = self.corners[right_opposite.0].coor.inside_circumcircle(
            &self.corners[start_corner.0].coor,
            &self.corners[left_opposite.0].coor,
            &self.corners[end_corner.0].coor,
        );
        !inside_circle
    }

    fn flip(&mut self, edge: EdgeId) -> Option<EdgeId> {
        let (left_triangle, right_triangle) = {
            let e = &self.edges[edge.0];
            (e.left_triangle?, e.right_triangle?)
        };

        let flipped_start = self.opposite_corner(right_triangle, edge)?;
        let flipped_end = self.opposite_corner(left_triangle, edge)?;
        let flipped_edge = self.new_edge(flipped_start, flipped_end);

        let first_parent = left_triangle;

        let left_index = self.triangles[left_triangle.0]
            .edge_lines
            .iter()
            .position(|&e| e == edge);
        let right_index = self.triangles[right_triangle.0]
            .edge_lines
            .iter()
            .position(|&e| e == edge);
        let (Some(left_index), Some(right_index)) = (left_index, right_index) else {
            // FRLogger.warn("Edge.flip: edge line inconsistent")
            return None;
        };

        let left_prev_edge = self.triangles[left_triangle.0].edge_lines[(left_index + 2) % 3];
        let left_next_edge = self.triangles[left_triangle.0].edge_lines[(left_index + 1) % 3];
        let right_prev_edge = self.triangles[right_triangle.0].edge_lines[(right_index + 2) % 3];
        let right_next_edge = self.triangles[right_triangle.0].edge_lines[(right_index + 1) % 3];

        let new_left_triangle = self.new_triangle(
            [flipped_edge, left_prev_edge, right_next_edge],
            Some(first_parent),
        );
        self.set_left_triangle(flipped_edge, Some(new_left_triangle));
        if self.get_left_triangle(left_prev_edge) == Some(left_triangle) {
            self.set_left_triangle(left_prev_edge, Some(new_left_triangle));
        } else {
            self.set_right_triangle(left_prev_edge, Some(new_left_triangle));
        }
        if self.get_left_triangle(right_next_edge) == Some(right_triangle) {
            self.set_left_triangle(right_next_edge, Some(new_left_triangle));
        } else {
            self.set_right_triangle(right_next_edge, Some(new_left_triangle));
        }

        let new_right_triangle = self.new_triangle(
            [flipped_edge, right_prev_edge, left_next_edge],
            Some(first_parent),
        );
        self.set_right_triangle(flipped_edge, Some(new_right_triangle));
        if self.get_left_triangle(right_prev_edge) == Some(right_triangle) {
            self.set_left_triangle(right_prev_edge, Some(new_right_triangle));
        } else {
            self.set_right_triangle(right_prev_edge, Some(new_right_triangle));
        }
        if self.get_left_triangle(left_next_edge) == Some(left_triangle) {
            self.set_left_triangle(left_next_edge, Some(new_right_triangle));
        } else {
            self.set_right_triangle(left_next_edge, Some(new_right_triangle));
        }

        Some(flipped_edge)
    }

    fn validate_edge(&self, edge: EdgeId) -> bool {
        let mut result = true;
        let e = &self.edges[edge.0];
        for (neighbour, _side) in [(e.left_triangle, "left"), (e.right_triangle, "right")] {
            match neighbour {
                None => {
                    if self.corners[e.start_corner.0].object.is_some()
                        || self.corners[e.end_corner.0].object.is_some()
                    {
                        // FRLogger.warn("Edge.validate: <side> triangle may be null only for
                        result = false;
                    }
                }
                Some(triangle) => {
                    if !self.triangles[triangle.0].edge_lines.contains(&edge) {
                        // FRLogger.warn("Edge.validate: <side> triangle does not contain this
                        result = false;
                    }
                }
            }
        }
        result
    }

    fn is_leaf(&self, triangle: TriangleId) -> bool {
        self.triangles[triangle.0].children.is_empty()
    }

    fn get_corner(&self, triangle: TriangleId, no: usize) -> Option<CornerId> {
        if no >= 3 {
            // FRLogger.warn("Triangle.get_corner: no out of range")
            return None;
        }
        let current_edge = &self.edges[self.triangles[triangle.0].edge_lines[no].0];
        if current_edge.left_triangle == Some(triangle) {
            Some(current_edge.start_corner)
        } else if current_edge.right_triangle == Some(triangle) {
            Some(current_edge.end_corner)
        } else {
            // FRLogger.warn("Triangle.get_corner: inconsistent edge lines")
            None
        }
    }

    fn opposite_corner(&self, triangle: TriangleId, edge_line: EdgeId) -> Option<CornerId> {
        let edge_line_no = self.triangles[triangle.0]
            .edge_lines
            .iter()
            .position(|&e| e == edge_line)?;
        let next_edge =
            &self.edges[self.triangles[triangle.0].edge_lines[(edge_line_no + 1) % 3].0];
        Some(if next_edge.left_triangle == Some(triangle) {
            next_edge.end_corner
        } else {
            next_edge.start_corner
        })
    }

    fn contains(&self, triangle: TriangleId, corner: CornerId) -> bool {
        let Some(is_on_the_left) = self.triangles[triangle.0].is_on_the_left_of_edge_line else {
            // FRLogger.warn("Triangle.contains: array isOnTheLeftOfEdgeLine not initialized")
            return false;
        };
        for (i, &on_the_left) in is_on_the_left.iter().enumerate() {
            let (start, end) = {
                let e = &self.edges[self.triangles[triangle.0].edge_lines[i].0];
                (e.start_corner, e.end_corner)
            };
            let current_side = self.side_of(corner, start, end);
            if on_the_left {
                if current_side == Side::OnTheRight {
                    return false;
                }
            } else if current_side == Side::OnTheLeft {
                return false;
            }
        }
        true
    }

    fn get_leaf_edges(&self, triangle: TriangleId, result_edges: &mut BTreeSet<EdgeId>) {
        if self.is_leaf(triangle) {
            for i in 0..3 {
                let current_edge = self.triangles[triangle.0].edge_lines[i];
                let e = &self.edges[current_edge.0];
                if self.corners[e.start_corner.0].object.is_some()
                    && self.corners[e.end_corner.0].object.is_some()
                {
                    result_edges.insert(current_edge);
                }
            }
        } else {
            for index in 0..self.triangles[triangle.0].children.len() {
                let current_child = self.triangles[triangle.0].children[index];
                if self.triangles[current_child.0].first_parent == Some(triangle) {
                    self.get_leaf_edges(current_child, result_edges);
                }
            }
        }
    }

    fn split_at_inner_point(
        &mut self,
        triangle: TriangleId,
        corner: CornerId,
    ) -> Option<[TriangleId; 3]> {
        for i in 0..3 {
            let dead_corner = self.get_corner(triangle, i)?;
            self.new_edge(dead_corner, corner);
        }

        let corner_0 = self.get_corner(triangle, 0)?;
        let corner_1 = self.get_corner(triangle, 1)?;
        let corner_2 = self.get_corner(triangle, 2)?;
        let edge_lines = self.triangles[triangle.0].edge_lines;

        let first_split_edge = self.new_edge(corner_1, corner);
        let second_split_edge = self.new_edge(corner, corner_0);
        let new_triangle_0 = self.new_triangle(
            [edge_lines[0], first_split_edge, second_split_edge],
            Some(triangle),
        );

        let third_split_edge = self.new_edge(corner_2, corner);
        let new_triangle_1 = self.new_triangle(
            [edge_lines[1], third_split_edge, first_split_edge],
            Some(triangle),
        );

        let new_triangle_2 = self.new_triangle(
            [edge_lines[2], second_split_edge, third_split_edge],
            Some(triangle),
        );

        let new_triangles = [new_triangle_0, new_triangle_1, new_triangle_2];

        for new_triangle in new_triangles {
            let current_edge = self.triangles[new_triangle.0].edge_lines[0];
            if self.get_left_triangle(current_edge) == Some(triangle) {
                self.set_left_triangle(current_edge, Some(new_triangle));
            } else {
                self.set_right_triangle(current_edge, Some(new_triangle));
            }
        }

        self.set_left_triangle(first_split_edge, Some(new_triangle_0));
        self.set_right_triangle(first_split_edge, Some(new_triangle_1));

        self.set_left_triangle(third_split_edge, Some(new_triangle_1));
        self.set_right_triangle(third_split_edge, Some(new_triangle_2));

        self.set_left_triangle(second_split_edge, Some(new_triangle_0));
        self.set_right_triangle(second_split_edge, Some(new_triangle_2));

        Some(new_triangles)
    }

    fn split_at_border_point(
        &mut self,
        triangle: TriangleId,
        corner: CornerId,
        neighbour_to_split: TriangleId,
    ) -> Option<[TriangleId; 4]> {
        let mut this_touching_edge_no: Option<usize> = None;
        let mut neighbour_touching_edge_no: Option<usize> = None;
        let mut touching_edge: Option<EdgeId> = None;
        let mut other_touching_edge: Option<EdgeId> = None;
        for i in 0..3 {
            let current_edge = self.triangles[triangle.0].edge_lines[i];
            let (start, end) = {
                let e = &self.edges[current_edge.0];
                (e.start_corner, e.end_corner)
            };
            if self.side_of(corner, start, end) == Side::Collinear {
                this_touching_edge_no = Some(i);
                touching_edge = Some(current_edge);
            }
            let current_edge = self.triangles[neighbour_to_split.0].edge_lines[i];
            let (start, end) = {
                let e = &self.edges[current_edge.0];
                (e.start_corner, e.end_corner)
            };
            if self.side_of(corner, start, end) == Side::Collinear {
                neighbour_touching_edge_no = Some(i);
                other_touching_edge = Some(current_edge);
            }
        }
        let (Some(this_touching_edge_no), Some(neighbour_touching_edge_no)) =
            (this_touching_edge_no, neighbour_touching_edge_no)
        else {
            // FRLogger.warn("Triangle.split_at_border_point: touching edge not found")
            return None;
        };
        if touching_edge != other_touching_edge {
            // FRLogger.warn("Triangle.split_at_border_point: edges inconsistent")
            return None;
        }
        let touching_edge = touching_edge?;

        let (touching_start, touching_end) = {
            let e = &self.edges[touching_edge.0];
            (e.start_corner, e.end_corner)
        };
        let (first_common_new_edge, second_common_new_edge) =
            if self.get_left_triangle(touching_edge) == Some(triangle) {
                let first = self.new_edge(touching_start, corner);
                (first, self.new_edge(corner, touching_end))
            } else {
                let first = self.new_edge(touching_end, corner);
                (first, self.new_edge(corner, touching_start))
            };

        let prev_edge = self.triangles[triangle.0].edge_lines[(this_touching_edge_no + 2) % 3];
        let prev_edge_is_left = self.get_left_triangle(prev_edge) == Some(triangle);
        let this_splitting_edge = if prev_edge_is_left {
            let start = self.edges[prev_edge.0].start_corner;
            self.new_edge(corner, start)
        } else {
            let end = self.edges[prev_edge.0].end_corner;
            self.new_edge(corner, end)
        };
        let new_triangle_0 = self.new_triangle(
            [prev_edge, first_common_new_edge, this_splitting_edge],
            Some(triangle),
        );
        if prev_edge_is_left {
            self.set_left_triangle(prev_edge, Some(new_triangle_0));
        } else {
            self.set_right_triangle(prev_edge, Some(new_triangle_0));
        }
        self.set_left_triangle(first_common_new_edge, Some(new_triangle_0));
        self.set_left_triangle(this_splitting_edge, Some(new_triangle_0));

        let next_edge = self.triangles[triangle.0].edge_lines[(this_touching_edge_no + 1) % 3];
        let new_triangle_1 = self.new_triangle(
            [this_splitting_edge, second_common_new_edge, next_edge],
            Some(triangle),
        );
        self.set_right_triangle(this_splitting_edge, Some(new_triangle_1));
        self.set_left_triangle(second_common_new_edge, Some(new_triangle_1));
        if self.get_left_triangle(next_edge) == Some(triangle) {
            self.set_left_triangle(next_edge, Some(new_triangle_1));
        } else {
            self.set_right_triangle(next_edge, Some(new_triangle_1));
        }

        let neighbour_next_edge =
            self.triangles[neighbour_to_split.0].edge_lines[(neighbour_touching_edge_no + 1) % 3];
        let neighbour_next_is_left =
            self.get_left_triangle(neighbour_next_edge) == Some(neighbour_to_split);
        let neighbour_splitting_edge = if neighbour_next_is_left {
            let end = self.edges[neighbour_next_edge.0].end_corner;
            self.new_edge(end, corner)
        } else {
            let start = self.edges[neighbour_next_edge.0].start_corner;
            self.new_edge(start, corner)
        };
        let new_triangle_2 = self.new_triangle(
            [
                neighbour_splitting_edge,
                first_common_new_edge,
                neighbour_next_edge,
            ],
            Some(neighbour_to_split),
        );
        self.set_left_triangle(neighbour_splitting_edge, Some(new_triangle_2));
        self.set_right_triangle(first_common_new_edge, Some(new_triangle_2));
        if neighbour_next_is_left {
            self.set_left_triangle(neighbour_next_edge, Some(new_triangle_2));
        } else {
            self.set_right_triangle(neighbour_next_edge, Some(new_triangle_2));
        }

        let prev_edge =
            self.triangles[neighbour_to_split.0].edge_lines[(neighbour_touching_edge_no + 2) % 3];
        let new_triangle_3 = self.new_triangle(
            [prev_edge, second_common_new_edge, neighbour_splitting_edge],
            Some(neighbour_to_split),
        );
        if self.get_left_triangle(prev_edge) == Some(neighbour_to_split) {
            self.set_left_triangle(prev_edge, Some(new_triangle_3));
        } else {
            self.set_right_triangle(prev_edge, Some(new_triangle_3));
        }
        self.set_right_triangle(second_common_new_edge, Some(new_triangle_3));
        self.set_right_triangle(neighbour_splitting_edge, Some(new_triangle_3));

        Some([
            new_triangle_0,
            new_triangle_1,
            new_triangle_2,
            new_triangle_3,
        ])
    }

    fn validate_triangle(&self, triangle: TriangleId) -> bool {
        let mut result = true;
        if self.is_leaf(triangle) {
            let mut prev_edge = self.triangles[triangle.0].edge_lines[2];
            for i in 0..3 {
                let current_edge = self.triangles[triangle.0].edge_lines[i];
                if !self.validate_edge(current_edge) {
                    result = false;
                }
                let prev = &self.edges[prev_edge.0];
                let prev_end_corner = if prev.left_triangle == Some(triangle) {
                    prev.end_corner
                } else {
                    prev.start_corner
                };
                let current = &self.edges[current_edge.0];
                let current_start_corner = if current.left_triangle == Some(triangle) {
                    current.start_corner
                } else if current.right_triangle == Some(triangle) {
                    current.end_corner
                } else {
                    // FRLogger.warn("Triangle.validate: edge inconsistent")
                    return false;
                };
                if current_start_corner != prev_end_corner {
                    // FRLogger.warn("Triangle.validate: corner inconsistent")
                    result = false;
                }
                prev_edge = current_edge;
            }
        } else {
            for index in 0..self.triangles[triangle.0].children.len() {
                let current_child = self.triangles[triangle.0].children[index];
                if self.triangles[current_child.0].first_parent == Some(triangle) {
                    let _ = self.validate_triangle(current_child);
                }
            }
        }
        result
    }

    fn initialize_is_on_the_left_of_edge_line_array(&mut self, triangle: TriangleId) {
        if self.triangles[triangle.0]
            .is_on_the_left_of_edge_line
            .is_some()
        {
            return;
        }
        let edge_lines = self.triangles[triangle.0].edge_lines;
        let flags = [
            self.edges[edge_lines[0].0].left_triangle == Some(triangle),
            self.edges[edge_lines[1].0].left_triangle == Some(triangle),
            self.edges[edge_lines[2].0].left_triangle == Some(triangle),
        ];
        self.triangles[triangle.0].is_on_the_left_of_edge_line = Some(flags);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangulate(points: &[(i32, i32)]) -> PlanarDelaunayTriangulation {
        let corners: Vec<DelaunayCorner> = points
            .iter()
            .enumerate()
            .map(|(index, &(x, y))| {
                DelaunayCorner::new(ItemId(index as u32 + 1), Point::Int(IntPoint::new(x, y)))
            })
            .collect();
        PlanarDelaunayTriangulation::new(&corners)
    }

    type EdgeTuple = (u32, (i32, i32), u32, (i32, i32));

    fn edge_tuples(triangulation: &PlanarDelaunayTriangulation) -> Vec<EdgeTuple> {
        fn xy(point: &Point) -> (i32, i32) {
            match point {
                Point::Int(p) => (p.x, p.y),
                Point::Rational(_) => panic!("integer input never produces a rational corner"),
            }
        }
        triangulation
            .get_edge_lines()
            .iter()
            .map(|edge| {
                (
                    edge.start_object.expect("result edges carry objects").0,
                    xy(&edge.start_point),
                    edge.end_object.expect("result edges carry objects").0,
                    xy(&edge.end_point),
                )
            })
            .collect()
    }

    fn coordinate_edges(
        triangulation: &PlanarDelaunayTriangulation,
    ) -> BTreeSet<((i32, i32), (i32, i32))> {
        edge_tuples(triangulation)
            .into_iter()
            .map(|(_, start, _, end)| {
                if start < end {
                    (start, end)
                } else {
                    (end, start)
                }
            })
            .collect()
    }

    fn grid(size: i32) -> Vec<(i32, i32)> {
        (0..size)
            .flat_map(|x| (0..size).map(move |y| (x * 1000, y * 1000)))
            .collect()
    }

    fn deep_validate(triangulation: &PlanarDelaunayTriangulation) -> bool {
        fn walk(t: &PlanarDelaunayTriangulation, triangle: TriangleId) -> bool {
            if t.is_leaf(triangle) {
                return t.validate_triangle(triangle);
            }
            let mut ok = true;
            for index in 0..t.triangles[triangle.0].children.len() {
                let child = t.triangles[triangle.0].children[index];
                if t.triangles[child.0].first_parent == Some(triangle) && !walk(t, child) {
                    ok = false;
                }
            }
            ok
        }
        match triangulation.anchor {
            Some(anchor) => walk(triangulation, anchor),
            None => false,
        }
    }

    fn permutation(n: usize) -> Vec<usize> {
        let mut list: Vec<CornerId> = (0..n).map(CornerId).collect();
        shuffle(&mut list, &mut SplitMix64::new(SEED));
        list.iter().map(|c| c.0).collect()
    }

    struct XorShift64(u64);

    impl XorShift64 {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }

        fn bounded(&mut self, bound: u64) -> i32 {
            (self.next() % bound) as i32
        }
    }

    fn random_points(count: usize, seed: u64) -> Vec<(i32, i32)> {
        let mut rng = XorShift64(seed);
        (0..count)
            .map(|_| {
                (
                    rng.bounded(200_000) - 100_000,
                    rng.bounded(200_000) - 100_000,
                )
            })
            .collect()
    }

    fn hull_boundary_point_count(points: &[(i32, i32)]) -> usize {
        let cross = |o: (i32, i32), a: (i32, i32), b: (i32, i32)| -> i128 {
            (i128::from(a.0) - i128::from(o.0)) * (i128::from(b.1) - i128::from(o.1))
                - (i128::from(a.1) - i128::from(o.1)) * (i128::from(b.0) - i128::from(o.0))
        };
        let mut sorted: Vec<(i32, i32)> = points.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        if sorted.len() < 3 {
            return sorted.len();
        }
        let build = |iter: &mut dyn Iterator<Item = (i32, i32)>| -> Vec<(i32, i32)> {
            let mut chain: Vec<(i32, i32)> = Vec::new();
            for point in iter {
                while chain.len() >= 2
                    && cross(chain[chain.len() - 2], chain[chain.len() - 1], point) < 0
                {
                    chain.pop();
                }
                chain.push(point);
            }
            chain
        };
        let mut lower = build(&mut sorted.iter().copied());
        let mut upper = build(&mut sorted.iter().rev().copied());
        lower.pop();
        upper.pop();
        lower.append(&mut upper);
        lower.sort_unstable();
        lower.dedup();
        lower.len()
    }

    #[test]
    fn the_seeded_shuffle_is_a_deterministic_permutation() {
        assert_eq!(permutation(3), vec![1, 2, 0]);
        assert_eq!(permutation(4), vec![2, 3, 0, 1]);
        assert_eq!(permutation(7), vec![6, 2, 3, 5, 4, 0, 1]);
        assert_eq!(permutation(12), vec![9, 5, 6, 2, 11, 4, 7, 1, 10, 8, 0, 3]);
    }

    #[test]
    fn a_square_pin_grid_keeps_every_edge() {
        for (size, expected) in [(2, 5), (5, 56), (6, 85)] {
            let triangulation = triangulate(&grid(size));
            assert_eq!(triangulation.get_edge_lines().len(), expected);
            assert!(deep_validate(&triangulation));
        }
    }

    #[test]
    fn a_column_of_points_at_x_equals_one_keeps_every_edge() {
        let points: Vec<(i32, i32)> = (0..6).map(|y| (1, y * 1000)).collect();
        let triangulation = triangulate(&points);
        assert_eq!(triangulation.get_edge_lines().len(), 5);
        assert!(deep_validate(&triangulation));
    }

    #[test]
    fn skewed_quadrilateral_has_the_five_edges_a_square_should_have() {
        let points = [(0, 0), (1000, 7), (1013, 1000), (11, 1007)];
        let triangulation = triangulate(&points);
        assert_eq!(triangulation.get_edge_lines().len(), 5);
        assert_eq!(hull_boundary_point_count(&points), 4);
        assert!(deep_validate(&triangulation));
    }

    #[test]
    fn collinear_triple_gives_the_two_segments_of_the_path() {
        let triangulation = triangulate(&[(0, 0), (500, 500), (1000, 1000)]);
        assert_eq!(
            coordinate_edges(&triangulation),
            BTreeSet::from([((0, 0), (500, 500)), ((500, 500), (1000, 1000)),])
        );
        assert!(deep_validate(&triangulation));
    }

    #[test]
    fn duplicate_points_become_degenerate_edges_first_in_the_result() {
        let triangulation = triangulate(&[
            (0, 0),
            (1000, 0),
            (1000, 1000),
            (0, 1000),
            (300, 400),
            (300, 400),
            (300, 400),
        ]);
        let edges = edge_tuples(&triangulation);
        assert_eq!(edges.len(), 10);
        assert!(edges[0].0 == 6 && edges[0].2 == 7);
        assert!(deep_validate(&triangulation));
    }

    #[test]
    fn the_edge_set_is_independent_of_insertion_order() {
        let points = random_points(50, 1_337);
        let forward = triangulate(&points);
        let reversed = triangulate(&points.iter().copied().rev().collect::<Vec<_>>());

        assert_eq!(coordinate_edges(&forward), coordinate_edges(&reversed));
    }

    #[test]
    fn a_seven_by_seven_dense_draw_keeps_every_edge() {
        let mut rng = XorShift64(0x13_82_09);
        for _ in 0..2000 {
            let mut points = BTreeSet::new();
            while points.len() < 49 {
                points.insert((rng.bounded(7001) - 3500, rng.bounded(7001) - 3500));
            }
            let points: Vec<_> = points.into_iter().collect();
            let triangulation = triangulate(&points);
            let expected = 3 * points.len() - 3 - hull_boundary_point_count(&points);
            assert_eq!(triangulation.get_edge_lines().len(), expected);
        }
    }

    #[test]
    fn a_duplicate_of_the_same_item_is_dropped_silently() {
        let point = Point::Int(IntPoint::new(300, 400));
        let corners = vec![
            DelaunayCorner::new(ItemId(1), Point::Int(IntPoint::new(0, 0))),
            DelaunayCorner::new(ItemId(2), Point::Int(IntPoint::new(1000, 0))),
            DelaunayCorner::new(ItemId(3), Point::Int(IntPoint::new(1000, 1000))),
            DelaunayCorner::new(ItemId(4), Point::Int(IntPoint::new(0, 1000))),
            DelaunayCorner::new(ItemId(5), point.clone()),
            DelaunayCorner::new(ItemId(5), point),
        ];
        let triangulation = PlanarDelaunayTriangulation::new(&corners);
        let edges = triangulation.get_edge_lines();
        assert!(
            edges.iter().all(|edge| edge.start_point != edge.end_point),
            "no degenerate edge may be produced for two corners of the same item"
        );
        assert!(deep_validate(&triangulation));
    }

    #[test]
    fn fifty_random_points_are_a_full_triangulation() {
        let points = random_points(50, 42);
        let mut distinct = points.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), 50, "the 50 sample points must be distinct");

        let triangulation = triangulate(&points);
        assert!(triangulation.validate());
        assert!(deep_validate(&triangulation));

        let hull = hull_boundary_point_count(&points);
        assert_eq!(
            triangulation.get_edge_lines().len(),
            3 * 50 - 3 - hull,
            "a triangulation of n points with h on the hull has 3n - 3 - h edges (h = {hull})"
        );
    }

    #[test]
    fn points_on_a_circle_are_a_full_triangulation() {
        let points: Vec<(i32, i32)> = (0..12)
            .map(|i| {
                let angle = 2.0 * std::f64::consts::PI * f64::from(i) / 12.0;
                (
                    (30_000.0 * angle.cos()).round() as i32,
                    (30_000.0 * angle.sin()).round() as i32,
                )
            })
            .collect();
        let triangulation = triangulate(&points);
        assert_eq!(hull_boundary_point_count(&points), 12);
        assert_eq!(triangulation.get_edge_lines().len(), 2 * 12 - 3);
        assert!(deep_validate(&triangulation));
    }

    #[test]
    fn every_result_edge_joins_two_distinct_items() {
        let points = random_points(30, 7);
        let triangulation = triangulate(&points);
        for edge in triangulation.get_edge_lines() {
            assert!(edge.start_object.is_some() && edge.end_object.is_some());
            assert_ne!(edge.start_object, edge.end_object);
            assert!(edge.length_square() > 0.0);
        }
    }

    #[test]
    fn empty_and_single_corner_inputs_produce_no_edges() {
        assert!(
            PlanarDelaunayTriangulation::new(&[])
                .get_edge_lines()
                .is_empty()
        );
        assert!(triangulate(&[(17, 23)]).get_edge_lines().is_empty());
    }

    #[test]
    fn validate_is_vacuous_on_an_inner_node() {
        let mut triangulation = triangulate(&[(0, 0), (1000, 7), (1013, 1000), (11, 1007)]);
        let leaf = (0..triangulation.triangles.len())
            .map(TriangleId)
            .find(|&t| triangulation.is_leaf(t))
            .expect("a triangulation has leaves");
        let broken_edge = triangulation.triangles[leaf.0].edge_lines[0];
        triangulation.edges[broken_edge.0].left_triangle = None;
        triangulation.edges[broken_edge.0].right_triangle = None;

        assert!(!deep_validate(&triangulation), "the real check must fail");
        assert!(
            triangulation.validate(),
            "Java's validate() reports success anyway (line 957 discards the child result)"
        );
    }

    #[test]
    fn is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PlanarDelaunayTriangulation>();
        assert_send_sync::<DelaunayCorner>();
        assert_send_sync::<DelaunayEdge>();
    }
}
