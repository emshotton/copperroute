//! Port of `app.freerouting.geometry.planar.RegularTileShape`: tile shapes whose border lines may
//! have only directions out of a fixed set — the axis-parallel [`IntBox`] and the 45-degree
//! [`IntOctagon`].
//!
//! Java declares the class `abstract sealed ... permits IntBox, IntOctagon` and resolves every
//! cross-type operation by the two auxiliary overloads (`compare(IntBox, int)` /
//! `compare(IntOctagon, int)`, `union(IntBox)` / `union(IntOctagon)`, `isContainedIn(IntBox)` /
//! `isContainedIn(IntOctagon)`). Those per-type halves are already ported on `IntBox` and
//! `IntOctagon`; this enum only reproduces the dispatch, keeping Java's argument order — in
//! particular `x.compare(y, e)` is Java's `y.compare(x, e).negate()`, and `x.union(y)` is Java's
//! `y.union(x)`.
//!
//! This is the key type of the R-tree in Plan 2.

use crate::int_box::IntBox;
use crate::int_octagon::IntOctagon;
use crate::side::Side;
use crate::tile_shape::TileShape;

/// A [`TileShape`] whose border lines run in a fixed set of directions
/// (RegularTileShape.java:7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegularTileShape {
    /// Java `IntBox`: the 4 orthogonal directions.
    Box(IntBox),
    /// Java `IntOctagon`: the 8 multiples of 45 degree.
    Octagon(IntOctagon),
}

impl From<IntBox> for RegularTileShape {
    fn from(value: IntBox) -> Self {
        RegularTileShape::Box(value)
    }
}

impl From<IntOctagon> for RegularTileShape {
    fn from(value: IntOctagon) -> Self {
        RegularTileShape::Octagon(value)
    }
}

impl From<RegularTileShape> for TileShape {
    fn from(value: RegularTileShape) -> Self {
        value.to_tile_shape()
    }
}

impl RegularTileShape {
    /// Compares the edge lines of index `edge_index` of this regular tile shape and `other`.
    /// Returns `Side::OnTheLeft`, if the edge line of this shape is to the left of the edge line
    /// of `other`; `Side::Collinear`, if the edge lines are equal; and `Side::OnTheRight`, if
    /// this edge line is to the right of the edge line of `other`
    /// (RegularTileShape.java:9-21, IntBox.java:506-565, IntOctagon.java:742-841).
    ///
    /// Java implements this as `other.compare(this, edgeIndex).negate()` in *both* subclasses,
    /// so the arguments are swapped and the result negated here as well.
    pub fn compare(&self, other: &RegularTileShape, edge_index: usize) -> Side {
        match (self, other) {
            (RegularTileShape::Box(a), RegularTileShape::Box(b)) => {
                b.compare(a, edge_index).negate()
            }
            (RegularTileShape::Box(a), RegularTileShape::Octagon(b)) => {
                b.compare_box(a, edge_index).negate()
            }
            (RegularTileShape::Octagon(a), RegularTileShape::Box(b)) => {
                b.compare_octagon(a, edge_index).negate()
            }
            (RegularTileShape::Octagon(a), RegularTileShape::Octagon(b)) => {
                b.compare_octagon(a, edge_index).negate()
            }
        }
    }

    /// Calculates the smallest `RegularTileShape` containing this shape and `other`
    /// (RegularTileShape.java:23-30). The union of two boxes is a box; every other combination
    /// widens to an octagon, exactly as Java's overload resolution does.
    pub fn union(&self, other: &RegularTileShape) -> RegularTileShape {
        match (self, other) {
            (RegularTileShape::Box(a), RegularTileShape::Box(b)) => {
                RegularTileShape::Box(b.union(a))
            }
            (RegularTileShape::Box(a), RegularTileShape::Octagon(b)) => {
                RegularTileShape::Octagon(b.union_box(a))
            }
            (RegularTileShape::Octagon(a), RegularTileShape::Box(b)) => {
                RegularTileShape::Octagon(b.union_octagon(a))
            }
            (RegularTileShape::Octagon(a), RegularTileShape::Octagon(b)) => {
                RegularTileShape::Octagon(b.union(a))
            }
        }
    }

    /// Returns true, if this shape contains `other` completely (RegularTileShape.java:32-33).
    /// Java routes it through `other.isContainedIn(this)`.
    pub fn contains(&self, other: &RegularTileShape) -> bool {
        match (self, other) {
            (RegularTileShape::Box(a), RegularTileShape::Box(b)) => b.is_contained_in(a),
            (RegularTileShape::Box(a), RegularTileShape::Octagon(b)) => b.is_contained_in(a),
            (RegularTileShape::Octagon(a), RegularTileShape::Box(b)) => {
                b.is_contained_in_octagon(a)
            }
            (RegularTileShape::Octagon(a), RegularTileShape::Octagon(b)) => {
                b.is_contained_in_octagon(a)
            }
        }
    }

    /// Java `isContainedIn(IntBox other)` (RegularTileShape.java:35-37).
    pub fn is_contained_in_box(&self, other: &IntBox) -> bool {
        match self {
            RegularTileShape::Box(b) => b.is_contained_in(other),
            RegularTileShape::Octagon(o) => o.is_contained_in(other),
        }
    }

    /// Java `isContainedIn(IntOctagon other)` (RegularTileShape.java:39-40).
    pub fn is_contained_in_octagon(&self, other: &IntOctagon) -> bool {
        match self {
            RegularTileShape::Box(b) => b.is_contained_in_octagon(other),
            RegularTileShape::Octagon(o) => o.is_contained_in_octagon(other),
        }
    }

    /// Widens this shape to the full [`TileShape`] enum. Java gets this for free through
    /// `RegularTileShape extends TileShape`.
    pub fn to_tile_shape(&self) -> TileShape {
        match self {
            RegularTileShape::Box(b) => TileShape::Box(*b),
            RegularTileShape::Octagon(o) => TileShape::Octagon(*o),
        }
    }

    /// Java `boundingBox()` (IntBox.java:242-244, IntOctagon.java:112-114).
    pub fn bounding_box(&self) -> IntBox {
        match self {
            RegularTileShape::Box(b) => b.bounding_box(),
            RegularTileShape::Octagon(o) => o.bounding_box(),
        }
    }

    /// Java `area()` (IntBox.java:68-70, IntOctagon.java:207-232).
    pub fn area(&self) -> f64 {
        match self {
            RegularTileShape::Box(b) => b.area(),
            RegularTileShape::Octagon(o) => o.area(),
        }
    }

    /// Java `getId()` (IntBox.java:247-249, IntOctagon.java:165-176): a deterministic
    /// tie-breaking id.
    pub fn get_id(&self) -> i32 {
        match self {
            RegularTileShape::Box(b) => b.get_id(),
            RegularTileShape::Octagon(o) => o.get_id(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bounding_directions::ShapeBoundingDirections;
    use crate::int_box::IntBox;
    use crate::int_point::IntPoint;
    use crate::simplex::Simplex;

    #[test]
    fn union_and_contains_mixed() {
        let a = RegularTileShape::Box(IntBox::from_coords(0, 0, 10, 10));
        let o = RegularTileShape::Octagon(IntBox::from_coords(5, 5, 20, 20).to_int_octagon());
        let u = a.union(&o);
        assert!(u.contains(&a));
        assert!(u.contains(&o));
        assert_eq!(u.bounding_box(), IntBox::from_coords(0, 0, 20, 20));
        assert!(matches!(
            a.union(&RegularTileShape::Box(IntBox::from_coords(1, 1, 2, 2))),
            RegularTileShape::Box(_)
        ));
    }

    #[test]
    fn bounding_directions_pick_variant() {
        let tri = Simplex::from_points(&[
            IntPoint::new(0, 0),
            IntPoint::new(10, 0),
            IntPoint::new(0, 10),
        ]);
        assert!(matches!(
            ShapeBoundingDirections::Orthogonal.bounds_simplex(&tri),
            Some(RegularTileShape::Box(_))
        ));
        assert!(matches!(
            ShapeBoundingDirections::FortyfiveDegree.bounds_simplex(&tri),
            Some(RegularTileShape::Octagon(_))
        ));
        let oct = ShapeBoundingDirections::FortyfiveDegree
            .bounds_simplex(&tri)
            .expect("the triangle is bounded");
        assert!((oct.area() - 50.0).abs() < 1e-9); // the 45° bound of a right triangle is exact
    }

    #[test]
    fn compare_edges_against_java_conventions() {
        let a = RegularTileShape::Box(IntBox::from_coords(0, 0, 10, 10));
        let b = RegularTileShape::Box(IntBox::from_coords(0, 2, 10, 10));
        // edge 0 is the lower edge line; b's is higher, so b's is on the left of a's.
        assert_eq!(b.compare(&a, 0), Side::OnTheLeft);
        assert_eq!(a.compare(&b, 0), Side::OnTheRight);
        assert_eq!(a.compare(&a, 0), Side::Collinear);
        // an IntBox compared against the octagon of the same box is collinear on every edge
        let oct = RegularTileShape::Octagon(IntBox::from_coords(0, 0, 10, 10).to_int_octagon());
        for edge in 0..8 {
            assert_eq!(a.compare(&oct, edge), Side::Collinear, "edge {edge}");
        }
    }

    #[test]
    fn containment_and_ids() {
        let b = IntBox::from_coords(0, 0, 10, 10);
        let small = RegularTileShape::Box(IntBox::from_coords(1, 1, 2, 2));
        assert!(small.is_contained_in_box(&b));
        assert!(small.is_contained_in_octagon(&b.to_int_octagon()));
        assert_eq!(RegularTileShape::Box(b).get_id(), b.get_id());
        assert_eq!(RegularTileShape::Box(b).area(), 100.0);
        assert_eq!(RegularTileShape::Box(b).to_tile_shape(), TileShape::Box(b));
    }
}
