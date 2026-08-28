//! Port of `datastructures/PlanarDelaunayTriangulation.java`: the incremental, randomised
//! Delaunay triangulation the ratsnest builder runs over a net's items.
//!
//! # Shape of the port
//!
//! Java is an object graph: `Corner`, `Edge` and `Triangle` are (inner) classes linked by
//! references, compared with `==` (reference identity) throughout, and the search structure is a
//! DAG of `Triangle` nodes whose leaves are the current triangulation. Nothing is ever removed —
//! superseded triangles stay in the graph as inner nodes — so the port keeps three append-only
//! arenas ([`PlanarDelaunayTriangulation::corners`], `edges`, `triangles`) and replaces every
//! reference with an index ([`CornerId`], [`EdgeId`], [`TriangleId`]). Reference identity becomes
//! index equality, which is exact here precisely because slots are never reused.
//!
//! Java's `Storable` interface (an object that can hand out `Point[] getTriangulationCorners()`)
//! collapses into the flat input slice this constructor takes: the Java constructor's first loop
//! (PlanarDelaunayTriangulation.java:50-56) does nothing but flatten `objectList` into a list of
//! `Corner(object, point)`, so [`PlanarDelaunayTriangulation::new`] takes that flattened list
//! directly. `Storable` identity — Java compares `corner.object == commonCorner.object` by
//! reference at line 157 — becomes [`ItemId`] equality: the sole Java caller,
//! `drc/NetIncompletes.java:383-397`, wraps exactly one `Item` per `Storable`, so two corners
//! share a `Storable` iff they share an item id.
//!
//! # The result order is deterministic in Java, and reproduced exactly
//!
//! `getEdgeLines` (PlanarDelaunayTriangulation.java:99-122) appends, in this order:
//!
//! 1. `degenerateEdges`, a `LinkedList` (line 88) — insertion order;
//! 2. the leaf edges collected into a **`TreeSet<Edge>`** (line 110), whose `Edge.compareTo`
//!    (line 419-421) is `this.id - other.id` over the ids `newEdgeId()` hands out (line 267-270,
//!    starting at 1 and incrementing per `Edge` construction).
//!
//! Neither is a `HashSet`/`HashMap`, so nothing here is JVM-nondeterministic. Edges are allocated
//! into `self.edges` in exactly the order `new Edge(...)` runs in Java, so `EdgeId(i)` corresponds
//! to Java's `id == i + 1` and a `BTreeSet<EdgeId>` iterates in Java's `TreeSet<Edge>` order.
//!
//! That makes the *allocation order of edges* observable, which is why the dead `newEdges` array
//! at PlanarDelaunayTriangulation.java:740-743 has to be reproduced — see
//! [`PlanarDelaunayTriangulation::split_at_inner_point`].
//!
//! # Two Java behaviours that surprise, and are reproduced
//!
//! * The bounding triangle is finite (`Limits.CRIT_INT == 2^25`, lines 65-69), and the in-circle
//!   predicate is `FloatPoint.insideCircle`, which answers `false` — "legal, do not flip" —
//!   whenever `FloatPoint.circleCenter`'s slope formula degenerates. It degenerates when two of
//!   the three circle points share an x or a y coordinate. Since two of the three bounding
//!   corners sit on the axes, plain axis-aligned input loses edges: the four corners of a square
//!   come back as **4** edges, not the 5 a triangulation of a quadrilateral has. See
//!   `docs/java-quirks.md` and the `square_pins_javas_four_edges` test.
//! * `validate()` is vacuous on any non-trivial triangulation — `Triangle.validate` throws away
//!   its children's results (line 957). Reproduced; see [`PlanarDelaunayTriangulation::validate`].
//!
//! # Determinism of the input shuffle
//!
//! The constructor shuffles the corner list with a fixed seed (lines 29-31, 60-61) so results are
//! reproducible. The permutation is therefore part of the observable behaviour and
//! [`JavaRandom`] + [`shuffle`] reproduce `java.util.Random` and `java.util.Collections.shuffle`
//! bit for bit.

use std::collections::BTreeSet;

use fr_geometry::int_point::IntPoint;
use fr_geometry::limits::CRIT_INT;
use fr_geometry::point::Point;
use fr_geometry::side::Side;

use crate::ids::ItemId;

/// The fixed seed the Java class shuffles its input with (PlanarDelaunayTriangulation.java:29).
const SEED: i64 = 99;

/// `java.util.Random`, reproduced bit for bit.
///
/// The triangulation's result depends on the order corners are inserted in, and that order is
/// `Collections.shuffle(cornerList, randomGenerator)` with a fixed seed
/// (PlanarDelaunayTriangulation.java:60-61). `fr-geometry`'s `polygon_shape.rs` has an identical
/// private copy for `PolygonShape.splitToConvexRecu`; it is duplicated rather than shared because
/// it is an implementation detail of two unrelated ports, not a geometry primitive.
struct JavaRandom {
    seed: i64,
}

impl JavaRandom {
    const MULTIPLIER: i64 = 0x5DEECE66D_i64;
    const ADDEND: i64 = 0xB;
    const MASK: i64 = (1 << 48) - 1;

    /// `new Random(seed)` — equivalently `setSeed(seed)`, which is what line 60 calls on the
    /// class's shared static generator before every shuffle.
    fn new(seed: i64) -> JavaRandom {
        JavaRandom {
            seed: (seed ^ JavaRandom::MULTIPLIER) & JavaRandom::MASK,
        }
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self
            .seed
            .wrapping_mul(JavaRandom::MULTIPLIER)
            .wrapping_add(JavaRandom::ADDEND)
            & JavaRandom::MASK;
        (self.seed >> (48 - bits)) as i32
    }

    /// `java.util.Random.nextInt(int bound)`, including the power-of-two fast path and the
    /// rejection loop. Only ever called with `bound >= 2` from [`shuffle`].
    fn next_int(&mut self, bound: i32) -> i32 {
        debug_assert!(
            bound > 0,
            "java.util.Random.nextInt requires a positive bound"
        );
        let mut r = self.next(31);
        let m = bound - 1;
        if bound & m == 0 {
            // bound is a power of 2
            r = ((i64::from(bound) * i64::from(r)) >> 31) as i32;
        } else {
            let mut u = r;
            loop {
                r = u % bound;
                if u.wrapping_sub(r).wrapping_add(m) >= 0 {
                    break;
                }
                u = self.next(31);
            }
        }
        r
    }
}

/// `java.util.Collections.shuffle(List, Random)`.
///
/// Java's implementation branches on `size < SHUFFLE_THRESHOLD (5) || list instanceof
/// RandomAccess`: the fast path swaps in the list, the slow path copies to an array, swaps there
/// and writes back. **Both branches run the identical Fisher-Yates loop**
/// `for (int i = size; i > 1; i--) swap(..., i - 1, rnd.nextInt(i))`, so the permutation does not
/// depend on which branch a `LinkedList` (Java's choice, line 50) takes, and one loop reproduces
/// both.
fn shuffle(list: &mut [CornerId], rng: &mut JavaRandom) {
    let mut i = list.len();
    while i > 1 {
        let j = rng.next_int(i as i32) as usize;
        list.swap(i - 1, j);
        i -= 1;
    }
}

/// An input corner: a point together with the board item it belongs to.
///
/// Java's `Corner` (line 307-325) holds a `Storable`, which `NetIncompletes` implements with one
/// wrapper object per `Item`; the flattening the Java constructor does at lines 50-56 is done by
/// the caller here, so one `Item` contributing several ratsnest corners simply appears in this
/// slice several times with the same [`ItemId`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelaunayCorner {
    /// The item this corner belongs to.
    pub object: ItemId,
    /// The corner's exact position.
    pub point: Point,
}

impl DelaunayCorner {
    /// Creates a corner from an item id and a point.
    pub fn new(object: ItemId, point: Point) -> DelaunayCorner {
        DelaunayCorner { object, point }
    }
}

/// One line segment of the triangulation's result — Java's `ResultEdge` (line 280-304).
///
/// `start_object`/`end_object` are `Option` because Java's are nullable: the three corners of the
/// bounding triangle carry a `null` `Storable` (lines 67-69). Edges *between* triangulation
/// leaves that touch a bounding corner are filtered out before they reach the result
/// (line 720-723), so in practice both are `Some`; the degenerate-edge path (line 160) has no
/// such filter, which is why the type stays honest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DelaunayEdge {
    /// The start point of the line segment.
    pub start_point: Point,
    /// The item at the start point of the line segment.
    pub start_object: Option<ItemId>,
    /// The end point of the line segment.
    pub end_point: Point,
    /// The item at the end point of the line segment.
    pub end_object: Option<ItemId>,
}

impl DelaunayEdge {
    /// The squared length of this segment, in `f64`.
    ///
    /// Not a field of Java's `ResultEdge`: `NetIncompletes.java:357` computes
    /// `toCorner.distanceSquare(fromCorner)` over `FloatPoint`s of the two result points in its
    /// own `Edge` wrapper, and sorts the airline candidates by it. Provided here so Plan 5 does
    /// not have to re-derive the conversion.
    pub fn length_square(&self) -> f64 {
        self.end_point
            .to_float()
            .distance_square(&self.start_point.to_float())
    }
}

/// Index of a [`Corner`] in [`PlanarDelaunayTriangulation::corners`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct CornerId(usize);

/// Index of an [`Edge`] in [`PlanarDelaunayTriangulation::edges`].
///
/// Ordering is the whole point: Java's `Edge.compareTo` (line 419-421) is `this.id - other.id`
/// over ids handed out by `newEdgeId()` in construction order, and edges are pushed onto the
/// arena in exactly that order, so `EdgeId(i)` is Java's `id == i + 1` and `Ord` on `EdgeId` is
/// Java's `Edge` comparator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct EdgeId(usize);

/// Index of a [`Triangle`] in [`PlanarDelaunayTriangulation::triangles`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct TriangleId(usize);

/// Java `PlanarDelaunayTriangulation.Corner` (line 307-325).
#[derive(Debug, Clone)]
struct Corner {
    /// `null` for the three corners of the bounding triangle.
    object: Option<ItemId>,
    coor: Point,
}

/// Java `PlanarDelaunayTriangulation.Edge` (line 398-605), minus the `id` field: the arena index
/// *is* the id (see [`EdgeId`]).
#[derive(Debug, Clone)]
struct Edge {
    start_corner: CornerId,
    end_corner: CornerId,
    /// The triangle on the left side of this edge; `None` only for the bounding edges.
    left_triangle: Option<TriangleId>,
    /// The triangle on the right side of this edge; `None` only for the bounding edges.
    right_triangle: Option<TriangleId>,
}

/// Java `PlanarDelaunayTriangulation.Triangle` (line 612-977).
#[derive(Debug, Clone)]
struct Triangle {
    /// The 3 edge lines, sorted counter-clockwise around the border. Never mutated after
    /// construction, exactly like Java's `final Edge[] edgeLines`.
    edge_lines: [EdgeId; 3],
    /// Triangles resulting from an edge flip have two parents; `first_parent` is the one the
    /// sequential graph walk descends through, so no node is visited twice (line 617-622).
    first_parent: Option<TriangleId>,
    /// Children in the search DAG.
    children: Vec<TriangleId>,
    /// `isOnTheLeftOfEdgeLine` (line 627-632): whether this triangle was on the left of edge line
    /// `i` at the moment it was inserted into the graph. `None` until
    /// `initializeIsOnTheLeftOfEdgeLineArray` has run — Java's `null` array, which `contains`
    /// checks for (line 693).
    is_on_the_left_of_edge_line: Option<[bool; 3]>,
}

/// A Delaunay triangulation of a set of board-item corners.
///
/// Build it with [`PlanarDelaunayTriangulation::new`] and read the result with
/// [`PlanarDelaunayTriangulation::get_edge_lines`]. The algorithm is the randomised incremental
/// one from de Berg et al., chapter 9.3, as in Java.
//
// renamed: Edge, Triangle, TriangleGraph — Java's three private inner classes become the private
// `Edge`/`Triangle` arena structs and, for `TriangleGraph`, the `anchor` field plus the
// `graph_insert`/`position_locate`/`position_locate_reku` methods (a one-field wrapper class is
// not worth a struct here).
// not ported: Storable, getTriangulationCorners — the interface's only job is to hand out
// `Point[]`, which the caller now does by building the `&[DelaunayCorner]` slice directly.
#[derive(Debug, Clone)]
pub struct PlanarDelaunayTriangulation {
    corners: Vec<Corner>,
    edges: Vec<Edge>,
    triangles: Vec<Triangle>,
    /// `TriangleGraph.anchor` (line 334): the root of the search DAG.
    anchor: Option<TriangleId>,
    /// Edges whose start and end corner have the same coordinates (line 36-40, 88).
    degenerate_edges: Vec<EdgeId>,
}

impl PlanarDelaunayTriangulation {
    /// Creates a new triangulation from `corners` (Java's constructor, line 49-96).
    ///
    /// `corners` is the flattened `(object, point)` list Java's first loop builds; see the module
    /// documentation for why the `Storable` indirection is dropped.
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

        // create a random permutation of the corners.
        // use a fixed seed to get reproducible result
        let mut rng = JavaRandom::new(SEED);
        shuffle(&mut corner_list, &mut rng);

        // create a big triangle containing all corners in the list to start with.
        //
        // Java bug: `boundingCoor` is `Limits.CRIT_INT` (2^25), *not* an unreachable infinity, and
        // two of the three corners sit exactly on an axis. Both facts leak into the result — see
        // the module docs and docs/java-quirks.md.
        let bounding_coor = CRIT_INT;
        let bounding_corners = [
            this.new_corner(None, Point::Int(IntPoint::new(bounding_coor, 0))),
            this.new_corner(None, Point::Int(IntPoint::new(0, bounding_coor))),
            this.new_corner(
                None,
                Point::Int(IntPoint::new(-bounding_coor, -bounding_coor)),
            ),
        ];

        let edge_lines = [
            this.new_edge(bounding_corners[0], bounding_corners[1]),
            this.new_edge(bounding_corners[1], bounding_corners[2]),
            this.new_edge(bounding_corners[2], bounding_corners[0]),
        ];

        let start_triangle = this.new_triangle(edge_lines, None);

        // Set the left triangle of the edge lines to startTriangle.
        // The right triangles remains null.
        for edge in edge_lines {
            this.edges[edge.0].left_triangle = Some(start_triangle);
        }

        // Initialize the search graph (`new TriangleGraph(startTriangle)`, line 87 / 336-342).
        this.graph_insert(start_triangle, None);

        // Insert the corners in the corner list into the search graph.
        for current_corner in corner_list {
            match this.position_locate(current_corner) {
                Some(triangle_to_split) => {
                    this.split(triangle_to_split, current_corner);
                }
                // totalized: Java passes the `null` `positionLocate` returned straight into
                // `split`, which dereferences `triangle.edgeLines` and throws a
                // `NullPointerException` out of the constructor (line 93-94). Unreachable while
                // the graph is consistent — every corner is inside the bounding triangle — so the
                // port skips the corner instead of aborting the whole triangulation.
                None => debug_assert!(false, "no triangle contains the corner being inserted"),
            }
        }

        this
    }

    /// Returns all edge lines of the result of the Delaunay Triangulation (line 98-122).
    ///
    /// The order is Java's exactly: the degenerate edges first, in insertion order, then the leaf
    /// edges in increasing edge id. See the module documentation.
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

    /// Checks the consistency of the triangles in this triangulation. Used for debugging purposes
    /// (line 255-264).
    ///
    /// Java bug: this is vacuous on any triangulation that had at least one corner inserted.
    /// `Triangle.validate` (line 923-962) only accumulates into `result` on the **leaf** branch;
    /// the inner-node branch calls `currentChild.validate()` and discards the return value
    /// (line 957), so the anchor — an inner node as soon as the first corner splits it — always
    /// answers `true`. Reproduced verbatim rather than "fixed", because a caller that trusts a
    /// `true` here is trusting exactly what Java's caller trusts. The unit tests in this module
    /// therefore validate the arena themselves instead of relying on this.
    pub fn validate(&self) -> bool {
        match self.anchor {
            Some(anchor) => self.validate_triangle(anchor),
            // totalized: Java dereferences `this.searchGraph.anchor` unconditionally (line 257)
            // and throws a `NullPointerException` if it is null. It never is: the constructor
            // always inserts the bounding triangle.
            None => {
                debug_assert!(false, "validate on a triangulation with no anchor");
                false
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Arena allocation
    // ---------------------------------------------------------------------------------------

    fn new_corner(&mut self, object: Option<ItemId>, coor: Point) -> CornerId {
        self.corners.push(Corner { object, coor });
        CornerId(self.corners.len() - 1)
    }

    /// `new Edge(startCorner, endCorner)` (line 412-416), including `newEdgeId()` (line 267-270):
    /// the returned index is Java's `id - 1`.
    fn new_edge(&mut self, start_corner: CornerId, end_corner: CornerId) -> EdgeId {
        self.edges.push(Edge {
            start_corner,
            end_corner,
            left_triangle: None,
            right_triangle: None,
        });
        EdgeId(self.edges.len() - 1)
    }

    /// `new Triangle(edgeLines, firstParent)` (line 634-639).
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

    // ---------------------------------------------------------------------------------------
    // PlanarDelaunayTriangulation
    // ---------------------------------------------------------------------------------------

    /// Splits `triangle` into 3 new triangles at `corner`, if `corner` lies in the interior. If
    /// `corner` lies on the border, `triangle` and the corresponding neighbour are split into 2
    /// new triangles each at `corner`. If `corner` lies outside this triangle or on a corner,
    /// nothing is split; in that case the function returns `false` (line 124-217).
    fn split(&mut self, triangle: TriangleId, corner: CornerId) -> bool {
        // check, if corner is in the interior of this triangle or
        // if corner is contained in an edge line.
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
                // corner is outside this triangle
                return false;
            } else if current_side == Side::Collinear {
                if let Some(containing) = containing_edge {
                    // corner is equal to a corner of this triangle
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
                // split triangle into 3 new triangles by adding edges from
                // the corners of triangle to corner.
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
                // split this triangle and the neighbour triangle into 4 new triangles by adding
                // edges from the corners of the triangles to corner.
                //
                // totalized: `otherNeighbour` returns `null` both when `triangle` is not a
                // neighbour of the containing edge (line 462, logged) and when the containing edge
                // is one of the three bounding edges, whose far side really is `null`. Java feeds
                // either straight into `splitAtBorderPoint`, which dereferences it and throws
                // (line 812). Both cases are unreachable for input inside the bounding triangle.
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

                // There are exact four new triangles with the first 2 dividing triangle and
                // the last 2 dividing neighbourToSplit.
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

    /// Flips `edge`, if it is no legal edge of the Delaunay Triangulation. `corner` is the last
    /// inserted corner of the triangulation. Returns `true`, if the triangulation was changed
    /// (line 219-253).
    ///
    /// Recursive, as in Java. The recursion is bounded by the number of edges incident to the
    /// newly inserted corner, which is why neither language needs a depth guard.
    fn legalize_edge(&mut self, corner: CornerId, edge: EdgeId) -> bool {
        if self.is_legal(edge) {
            return false;
        }
        // `isLegal` returned false, so both neighbours are set (line 470-472).
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

        // totalized: `flip` returns `null` when the edge is not an edge line of both its
        // neighbours (line 511-514, logged); Java then dereferences it at line 240 and throws.
        let Some(flipped_edge) = self.flip(edge) else {
            debug_assert!(false, "flip failed on an edge with two neighbour triangles");
            return false;
        };

        // Update the search graph. Note that Java reads `edge.leftTriangle`/`edge.rightTriangle`
        // *after* the flip; `flip` never touches the flipped edge's own neighbour fields, so the
        // values captured above are the same ones.
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

        // Call this function recursively for the other edge lines of triangleToChange.
        for i in 0..3 {
            let current_edge = self.triangles[triangle_to_change.0].edge_lines[i];
            if current_edge != edge {
                self.legalize_edge(corner, current_edge);
            }
        }
        true
    }

    // ---------------------------------------------------------------------------------------
    // Corner
    // ---------------------------------------------------------------------------------------

    /// `Corner.sideOf` (line 317-324): `Side::OnTheLeft` if `corner` is on the left of the line
    /// from `p1` to `p2`, `Side::OnTheRight` if on the right, `Side::Collinear` otherwise.
    ///
    /// This is `Point.sideOf`, i.e. exact integer/rational arithmetic — not a float predicate.
    fn side_of(&self, corner: CornerId, p1: CornerId, p2: CornerId) -> Side {
        self.corners[corner.0]
            .coor
            .side_of(&self.corners[p1.0].coor, &self.corners[p2.0].coor)
    }

    // ---------------------------------------------------------------------------------------
    // TriangleGraph
    // ---------------------------------------------------------------------------------------

    /// `TriangleGraph.insert` (line 344-351).
    fn graph_insert(&mut self, triangle: TriangleId, parent: Option<TriangleId>) {
        self.initialize_is_on_the_left_of_edge_line_array(triangle);
        match parent {
            None => self.anchor = Some(triangle),
            Some(parent) => self.triangles[parent.0].children.push(triangle),
        }
    }

    /// Searches for the leaf triangle containing `corner`. It will not be unique, if `corner` lies
    /// on a triangle edge (line 353-372).
    fn position_locate(&self, corner: CornerId) -> Option<TriangleId> {
        let anchor = self.anchor?;
        if self.triangles[anchor.0].children.is_empty() {
            return Some(anchor);
        }
        // Java does *not* test `anchor.contains(corner)` first, only the children.
        for index in 0..self.triangles[anchor.0].children.len() {
            let current_child = self.triangles[anchor.0].children[index];
            if let Some(result) = self.position_locate_reku(corner, current_child) {
                return Some(result);
            }
        }
        // FRLogger.warn("TriangleGraph.position_locate: containing triangle not found")
        None
    }

    /// Recursive part of [`Self::position_locate`] (line 374-391).
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

    // ---------------------------------------------------------------------------------------
    // Edge
    // ---------------------------------------------------------------------------------------

    /// `Edge.getLeftTriangle` (line 423-425).
    fn get_left_triangle(&self, edge: EdgeId) -> Option<TriangleId> {
        self.edges[edge.0].left_triangle
    }

    /// `Edge.setLeftTriangle` (line 427-429).
    fn set_left_triangle(&mut self, edge: EdgeId, triangle: Option<TriangleId>) {
        self.edges[edge.0].left_triangle = triangle;
    }

    /// `Edge.getRightTriangle` (line 431-433).
    ///
    /// Kept for symmetry with the Java class, which has no caller for it either — every read of
    /// the right neighbour in the Java source goes through the field directly.
    #[allow(dead_code)]
    fn get_right_triangle(&self, edge: EdgeId) -> Option<TriangleId> {
        self.edges[edge.0].right_triangle
    }

    /// `Edge.setRightTriangle` (line 435-437).
    fn set_right_triangle(&mut self, edge: EdgeId, triangle: Option<TriangleId>) {
        self.edges[edge.0].right_triangle = triangle;
    }

    /// Returns the common corner of `edge` and `other`, or `None` if no common corner exists
    /// (line 439-449).
    ///
    /// Java's `Corner` does not override `equals`, so `other.startCorner.equals(this.startCorner)`
    /// is reference identity — reproduced exactly by [`CornerId`] equality. Two corners at the
    /// same coordinates are *not* equal here, which is what makes the degenerate-edge path work.
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

    /// Returns the neighbour triangle of `edge` which is different from `triangle`, or `None` if
    /// `triangle` is not a neighbour of `edge` (line 451-466).
    ///
    /// Java has two paths to `null` here — "not a neighbour" (logged) and "the far side is a
    /// bounding edge's `null`" — and both collapse into `None`, because both make the caller
    /// throw at the same place.
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

    /// Returns true if `edge` is a legal edge of the Delaunay Triangulation (line 468-485).
    ///
    /// Java bug (inherited from `FloatPoint`, not introduced here): this is the *only* float
    /// predicate in the algorithm, and `FloatPoint.insideCircle` answers `false` whenever
    /// `FloatPoint.circleCenter`'s slope formula degenerates — which it does whenever the start
    /// corner shares an x or a y coordinate with the left opposite corner, or the left opposite
    /// corner shares an x with the end corner. An edge that should be flipped is then left in
    /// place. Two of the three bounding corners lie on an axis, so this fires on ordinary
    /// axis-aligned input; see `docs/java-quirks.md`.
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
            // totalized: Java dereferences the `null` `oppositeCorner` returned when `edge` is not
            // an edge line of its own neighbour (line 677-680, logged) and throws.
            debug_assert!(
                false,
                "an edge is always an edge line of both its neighbours"
            );
            return true;
        };

        let inside_circle = self.corners[right_opposite.0]
            .coor
            .to_float()
            .inside_circle(
                &self.corners[start_corner.0].coor.to_float(),
                &self.corners[left_opposite.0].coor.to_float(),
                &self.corners[end_corner.0].coor.to_float(),
            );
        !inside_circle
    }

    /// Flips `edge` to the edge line between the opposite corners of the adjacent triangles.
    /// Returns the newly constructed edge (line 487-559).
    fn flip(&mut self, edge: EdgeId) -> Option<EdgeId> {
        let (left_triangle, right_triangle) = {
            let e = &self.edges[edge.0];
            (e.left_triangle?, e.right_triangle?)
        };

        // Create the flipped edge, so that the start corner of this edge is on the left
        // and the end corner of this edge on the right.
        //
        // totalized: Java would pass a `null` corner into the new `Edge` here (consuming an edge
        // id) rather than bail; the `null` only becomes an exception later, at the first
        // `startCorner.coor`.
        let flipped_start = self.opposite_corner(right_triangle, edge)?;
        let flipped_end = self.opposite_corner(left_triangle, edge)?;
        let flipped_edge = self.new_edge(flipped_start, flipped_end);

        let first_parent = left_triangle;

        // Calculate the index of this edge line in the left and right adjacent triangles.
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

        // Create the left triangle of the flipped edge.
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

        // Create the right triangle of the flipped edge.
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

    /// Checks the consistency of `edge` in its database. Used for debugging purposes
    /// (line 561-604).
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
                        // bounding edges")
                        result = false;
                    }
                }
                Some(triangle) => {
                    // check if the triangle contains this edge
                    if !self.triangles[triangle.0].edge_lines.contains(&edge) {
                        // FRLogger.warn("Edge.validate: <side> triangle does not contain this
                        // edge")
                        result = false;
                    }
                }
            }
        }
        result
    }

    // ---------------------------------------------------------------------------------------
    // Triangle
    // ---------------------------------------------------------------------------------------

    /// `Triangle.isLeaf` (line 641-644).
    fn is_leaf(&self, triangle: TriangleId) -> bool {
        self.triangles[triangle.0].children.is_empty()
    }

    /// Gets the corner with index `no` (line 646-663).
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

    /// Calculates the opposite corner of `triangle` to `edge_line`. Returns `None` if `edge_line`
    /// is not an edge line of `triangle` (line 665-689).
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

    /// Checks if `corner` is inside or on the border of `triangle` (line 691-713).
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
                // checking currentEdge.leftTriangle == this instead will not work, if this
                // triangle is an inner node.
                if current_side == Side::OnTheRight {
                    return false;
                }
            } else if current_side == Side::OnTheLeft {
                return false;
            }
        }
        true
    }

    /// Puts the edges of all leaves below `triangle` into `result_edges` (line 715-733).
    fn get_leaf_edges(&self, triangle: TriangleId, result_edges: &mut BTreeSet<EdgeId>) {
        if self.is_leaf(triangle) {
            for i in 0..3 {
                let current_edge = self.triangles[triangle.0].edge_lines[i];
                let e = &self.edges[current_edge.0];
                if self.corners[e.start_corner.0].object.is_some()
                    && self.corners[e.end_corner.0].object.is_some()
                {
                    // Skip edges containing a bounding corner.
                    result_edges.insert(current_edge);
                }
            }
        } else {
            for index in 0..self.triangles[triangle.0].children.len() {
                let current_child = self.triangles[triangle.0].children[index];
                // to prevent traversing nodes more than once
                if self.triangles[current_child.0].first_parent == Some(triangle) {
                    self.get_leaf_edges(current_child, result_edges);
                }
            }
        }
    }

    /// Splits `triangle` into 3 new triangles by adding edges from its corners to `corner`, which
    /// has to be located in the interior of `triangle` (line 735-791).
    fn split_at_inner_point(
        &mut self,
        triangle: TriangleId,
        corner: CornerId,
    ) -> Option<[TriangleId; 3]> {
        // Java bug: PlanarDelaunayTriangulation.java:740-743 fills a local `Edge[] newEdges` that
        // is never read again — three fully constructed `Edge` objects that belong to no triangle.
        // They are not harmless: each one draws an id from `newEdgeId()`, and edge ids are the
        // sort key of the `TreeSet` `getEdgeLines` returns, so dropping this loop would reorder
        // the result. Reproduced deliberately. See docs/java-quirks.md.
        for i in 0..3 {
            let dead_corner = self.get_corner(triangle, i)?;
            self.new_edge(dead_corner, corner);
        }

        let corner_0 = self.get_corner(triangle, 0)?;
        let corner_1 = self.get_corner(triangle, 1)?;
        let corner_2 = self.get_corner(triangle, 2)?;
        let edge_lines = self.triangles[triangle.0].edge_lines;

        // construct the 3 new triangles.
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

        // Set the new neighbour triangles of the edge lines.
        for new_triangle in new_triangles {
            let current_edge = self.triangles[new_triangle.0].edge_lines[0];
            if self.get_left_triangle(current_edge) == Some(triangle) {
                self.set_left_triangle(current_edge, Some(new_triangle));
            } else {
                self.set_right_triangle(current_edge, Some(new_triangle));
            }
            // The other neighbour triangle remains valid.
        }

        self.set_left_triangle(first_split_edge, Some(new_triangle_0));
        self.set_right_triangle(first_split_edge, Some(new_triangle_1));

        self.set_left_triangle(third_split_edge, Some(new_triangle_1));
        self.set_right_triangle(third_split_edge, Some(new_triangle_2));

        self.set_left_triangle(second_split_edge, Some(new_triangle_0));
        self.set_right_triangle(second_split_edge, Some(new_triangle_2));

        Some(new_triangles)
    }

    /// Splits `triangle` and `neighbour_to_split` into 4 new triangles by adding edges from the
    /// corners of the triangles to `corner`, which is assumed to be located on their common edge
    /// line. If that is not true, the function returns `None`. The first 2 result triangles come
    /// from splitting `triangle`, the last 2 from splitting `neighbour_to_split`
    /// (line 793-920).
    fn split_at_border_point(
        &mut self,
        triangle: TriangleId,
        corner: CornerId,
        neighbour_to_split: TriangleId,
    ) -> Option<[TriangleId; 4]> {
        // look for the triangle edge of this and the neighbour triangle containing corner;
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

        // Construct the new edge lines that 2 split triangles of this triangle
        // will be on the left side of the new common touching edges.
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

        // Construct the first split triangle of this triangle.
        let prev_edge = self.triangles[triangle.0].edge_lines[(this_touching_edge_no + 2) % 3];
        // construct the splitting edge line of this triangle, so that the first split
        // triangle lies on the left side, and the second split triangle on the right side.
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

        // Construct the second split triangle of this triangle.
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

        // construct the first split triangle of neighbourToSplit
        let neighbour_next_edge =
            self.triangles[neighbour_to_split.0].edge_lines[(neighbour_touching_edge_no + 1) % 3];
        // construct the splitting edge line of neighbourToSplit, so that the first split
        // triangle lies on the left side, and the second split triangle on the right side.
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

        // construct the second split triangle of neighbourToSplit
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

    /// Checks the consistency of `triangle` and its children. Used for debugging purposes
    /// (line 922-962).
    ///
    /// Java bug: the inner-node branch (line 954-959) calls `currentChild.validate()` without
    /// looking at the result, so this method reports a problem only when `triangle` is itself a
    /// leaf. Reproduced — see [`Self::validate`].
    fn validate_triangle(&self, triangle: TriangleId) -> bool {
        let mut result = true;
        if self.is_leaf(triangle) {
            let mut prev_edge = self.triangles[triangle.0].edge_lines[2];
            for i in 0..3 {
                let current_edge = self.triangles[triangle.0].edge_lines[i];
                if !self.validate_edge(current_edge) {
                    result = false;
                }
                // Check, if the end corner of the previous line equals to the start corner of
                // this line.
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
                // to avoid traversing nodes more than once.
                if self.triangles[current_child.0].first_parent == Some(triangle) {
                    // Java bug: the result is computed and thrown away (line 957).
                    let _ = self.validate_triangle(current_child);
                }
            }
        }
        result
    }

    /// `Triangle.initializeIsOnTheLeftOfEdgeLineArray` (line 964-976). Must be done as long as
    /// this triangle node is a leaf and after, for all its edge lines, the left or right triangle
    /// reference is set to this triangle.
    fn initialize_is_on_the_left_of_edge_line_array(&mut self, triangle: TriangleId) {
        if self.triangles[triangle.0]
            .is_on_the_left_of_edge_line
            .is_some()
        {
            return; // already initialized
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

    /// Builds a triangulation from `(x, y)` pairs, one distinct [`ItemId`] per pair, ids starting
    /// at 1 as `ItemIdGenerator` does.
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

    /// One result edge as `(start id, start point, end id, end point)`.
    type EdgeTuple = (u32, (i32, i32), u32, (i32, i32));

    /// The result as [`EdgeTuple`]s, in `get_edge_lines` order.
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

    /// The *real* structural check Java's `validate()` was meant to be, walking the arena
    /// directly. Java's own version is vacuous on an inner node (see
    /// [`PlanarDelaunayTriangulation::validate`]), so the tests below cannot lean on it.
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

    /// Java's `Collections.shuffle` permutation for `n` elements, as a list of source indices —
    /// the pinning test for [`shuffle`] and [`JavaRandom`].
    fn permutation(n: usize) -> Vec<usize> {
        let mut list: Vec<CornerId> = (0..n).map(CornerId).collect();
        shuffle(&mut list, &mut JavaRandom::new(SEED));
        list.iter().map(|c| c.0).collect()
    }

    /// A deterministic LCG for the 50-point set, mirrored exactly by the `p2t13` differential
    /// driver so both languages see the same points.
    struct Lcg(u64);

    impl Lcg {
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
        let mut lcg = Lcg(seed);
        (0..count)
            .map(|_| {
                (
                    lcg.bounded(200_000) - 100_000,
                    lcg.bounded(200_000) - 100_000,
                )
            })
            .collect()
    }

    /// Number of input points on the convex-hull boundary, **including** points that are collinear
    /// with their two hull neighbours: those are still vertices of the triangulation, so they
    /// still count as `h` in `3n - 3 - h`. Computed with exact `i128` cross products, entirely
    /// independently of the triangulation under test.
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
        // Monotone chain, keeping collinear boundary points (`< 0` instead of `<= 0`).
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
    fn java_random_and_shuffle_match_the_jvm() {
        // Pinned against a JDK 25 run of `new Random(99); setSeed(99);
        // Collections.shuffle(new LinkedList<>(0..n), r)` — the `perm=` line of the `p2t13`
        // differential driver.
        assert_eq!(permutation(3), vec![2, 0, 1]);
        assert_eq!(permutation(4), vec![1, 0, 3, 2]);
        assert_eq!(permutation(7), vec![1, 3, 0, 5, 6, 2, 4]);
        assert_eq!(permutation(12), vec![5, 0, 2, 3, 11, 4, 1, 8, 6, 9, 10, 7]);
    }

    #[test]
    fn square_pins_javas_four_edges() {
        // The brief expected 5 edges (a quadrilateral plus one diagonal). Java produces **4**:
        // the top side (1000,1000)-(0,1000) is missing. Pinned against
        // `run.sh p2t13 4 42 1` on a JDK 25.
        //
        // Why: with the shuffled insertion order [P2, P1, P4, P3] the last corner P3=(1000,1000)
        // lands in the triangle (P2, B0, B1) formed with two bounding corners, and the edge
        // B1(0,2^25)->P2(1000,0) that would have to flip to give P3-P4 tests *legal*. That test
        // is `P4.insideCircle(B1, P3, P2)`, and P2/P3 share an x coordinate, so
        // `FloatPoint.circleCenter`'s `slope2 = (0 - 1000) / (1000 - 1000)` is infinite and the
        // center comes back `(NaN, NaN)` — `insideCircle` then answers `false`, i.e. "legal".
        // Verified directly against Java: `circleCenter(B1, P3, P2) = (NaN, NaN)`.
        // See docs/java-quirks.md.
        let triangulation = triangulate(&[(0, 0), (1000, 0), (1000, 1000), (0, 1000)]);
        assert_eq!(
            edge_tuples(&triangulation),
            vec![
                (2, (1000, 0), 1, (0, 0)),
                (1, (0, 0), 4, (0, 1000)),
                (2, (1000, 0), 4, (0, 1000)),
                (2, (1000, 0), 3, (1000, 1000)),
            ]
        );
        // The diagonal Java picks is P2-P4, and it is never flipped to P1-P3: all four corners
        // are cocircular, and `insideCircle` uses a `- 1.0` tolerance (FloatPoint.java) that
        // makes a cocircular point count as outside.
        assert!(deep_validate(&triangulation));
    }

    #[test]
    fn skewed_quadrilateral_has_the_five_edges_a_square_should_have() {
        // The same four-corner shape, nudged so that no two of the points involved in an
        // in-circle test share a coordinate with each other or with a bounding corner. Now the
        // `3n - 3 - h` count holds: 3*4 - 3 - 4 = 5. This is the control that proves the square's
        // missing edge is the `circleCenter` degeneracy and not a porting error.
        let points = [(0, 0), (1000, 7), (1013, 1000), (11, 1007)];
        let triangulation = triangulate(&points);
        assert_eq!(triangulation.get_edge_lines().len(), 5);
        assert_eq!(hull_boundary_point_count(&points), 4);
        assert!(deep_validate(&triangulation));
    }

    #[test]
    fn collinear_triple_gives_the_two_segments_of_the_path() {
        // Three collinear points cannot bound a triangle; the result is the two segments of the
        // path through them, both directed away from the middle point.
        let triangulation = triangulate(&[(0, 0), (500, 500), (1000, 1000)]);
        assert_eq!(
            edge_tuples(&triangulation),
            vec![(1, (0, 0), 2, (500, 500)), (2, (500, 500), 3, (1000, 1000)),]
        );
        assert!(deep_validate(&triangulation));
    }

    #[test]
    fn duplicate_points_become_degenerate_edges_first_in_the_result() {
        // A square plus three items at the very same interior point. The first of the three is
        // inserted normally; the other two hit `split`'s two-collinear-edges branch
        // (PlanarDelaunayTriangulation.java:148-164) and each contributes one zero-length
        // "degenerate" edge, which `getEdgeLines` emits *before* every triangulation edge
        // (line 100-108). Pinned against `run.sh p2t13 7 42 3`.
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
        assert_eq!(
            edges,
            vec![
                // degenerate edges, in insertion order
                (7, (300, 400), 6, (300, 400)),
                (5, (300, 400), 6, (300, 400)),
                // triangulation edges, by edge id
                (2, (1000, 0), 4, (0, 1000)),
                (2, (1000, 0), 1, (0, 0)),
                (4, (0, 1000), 1, (0, 0)),
                (4, (0, 1000), 6, (300, 400)),
                (6, (300, 400), 2, (1000, 0)),
                (1, (0, 0), 6, (300, 400)),
                (2, (1000, 0), 3, (1000, 1000)),
            ]
        );
        // Item 6 — not 5 — is the one that made it into the triangulation: the shuffle
        // (perm=[1, 3, 0, 5, 6, 2, 4]) inserts corner index 5, i.e. item 6, before item 5.
        assert!(edges[0].0 == 7 && edges[0].2 == 6);
        assert!(deep_validate(&triangulation));
    }

    #[test]
    fn a_duplicate_of_the_same_item_is_dropped_silently() {
        // PlanarDelaunayTriangulation.java:157: when the coincident corner belongs to the *same*
        // object, `split` returns false without recording a degenerate edge — an item whose
        // `getRatsnestCorners` lists the same point twice must not produce a zero-length airline
        // to itself.
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
        // The generator must not hand out a duplicate, or `3n - 3 - h` would not apply.
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
        // Every point on the hull: 3n - 3 - n = 2n - 3. Cocircular by construction, so this is
        // also the case where the `- 1.0` tolerance in `insideCircle` decides every flip.
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
        // What `NetIncompletes` relies on: both endpoints carry an object (bounding corners are
        // filtered at PlanarDelaunayTriangulation.java:720-723) and no edge joins an item to
        // itself.
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
        // Pins the Java bug at PlanarDelaunayTriangulation.java:957: corrupt a leaf edge so that
        // the real check fails, and `validate()` still answers `true` because the anchor is an
        // inner node whose children's results are discarded.
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
