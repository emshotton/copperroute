use crate::int_box::IntBox;
use crate::int_octagon::IntOctagon;
use crate::side::Side;
use crate::tile_shape::TileShape;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegularTileShape {
    Box(IntBox),
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

    pub fn intersects(&self, other: &RegularTileShape) -> bool {
        match (self, other) {
            (RegularTileShape::Box(a), RegularTileShape::Box(b)) => a.intersects(b),
            (RegularTileShape::Box(a), RegularTileShape::Octagon(b)) => b.intersects_box(a),
            (RegularTileShape::Octagon(a), RegularTileShape::Box(b)) => a.intersects_box(b),
            (RegularTileShape::Octagon(a), RegularTileShape::Octagon(b)) => a.intersects_octagon(b),
        }
    }

    pub fn intersects_octagon(&self, other: &IntOctagon) -> bool {
        match self {
            RegularTileShape::Box(b) => b.intersects_octagon(other),
            RegularTileShape::Octagon(o) => o.intersects_octagon(other),
        }
    }

    pub fn intersects_box(&self, other: &IntBox) -> bool {
        match self {
            RegularTileShape::Box(b) => b.intersects(other),
            RegularTileShape::Octagon(o) => o.intersects_box(other),
        }
    }

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

    pub fn is_contained_in_box(&self, other: &IntBox) -> bool {
        match self {
            RegularTileShape::Box(b) => b.is_contained_in(other),
            RegularTileShape::Octagon(o) => o.is_contained_in(other),
        }
    }

    pub fn is_contained_in_octagon(&self, other: &IntOctagon) -> bool {
        match self {
            RegularTileShape::Box(b) => b.is_contained_in_octagon(other),
            RegularTileShape::Octagon(o) => o.is_contained_in_octagon(other),
        }
    }

    pub fn to_tile_shape(&self) -> TileShape {
        match self {
            RegularTileShape::Box(b) => TileShape::Box(*b),
            RegularTileShape::Octagon(o) => TileShape::Octagon(*o),
        }
    }

    pub fn bounding_box(&self) -> IntBox {
        match self {
            RegularTileShape::Box(b) => b.bounding_box(),
            RegularTileShape::Octagon(o) => o.bounding_box(),
        }
    }

    pub fn area(&self) -> f64 {
        match self {
            RegularTileShape::Box(b) => b.area(),
            RegularTileShape::Octagon(o) => o.area(),
        }
    }

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
        assert!((oct.area() - 50.0).abs() < 1e-9);
    }

    #[test]
    fn compare_edges_against_java_conventions() {
        let a = RegularTileShape::Box(IntBox::from_coords(0, 0, 10, 10));
        let b = RegularTileShape::Box(IntBox::from_coords(0, 2, 10, 10));
        assert_eq!(b.compare(&a, 0), Side::OnTheLeft);
        assert_eq!(a.compare(&b, 0), Side::OnTheRight);
        assert_eq!(a.compare(&a, 0), Side::Collinear);
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

    fn every_regular_pair() -> Vec<(RegularTileShape, RegularTileShape)> {
        let coords = [
            (0, 0, 10, 10),
            (5, 5, 20, 20),
            (10, 10, 20, 20),
            (11, 11, 20, 20),
            (-5, -5, 30, 30),
            (0, 20, 10, 30),
        ];
        let shapes: Vec<RegularTileShape> = coords
            .iter()
            .flat_map(|(a, b, c, d)| {
                let boxed = IntBox::from_coords(*a, *b, *c, *d);
                [
                    RegularTileShape::Box(boxed),
                    RegularTileShape::Octagon(boxed.to_int_octagon()),
                ]
            })
            .collect();
        shapes
            .iter()
            .flat_map(|a| shapes.iter().map(move |b| (*a, *b)))
            .collect()
    }

    #[test]
    fn intersects_agrees_with_the_widened_tile_shape_test() {
        for (a, b) in every_regular_pair() {
            assert_eq!(
                a.intersects(&b),
                a.to_tile_shape().intersects(&b.to_tile_shape()),
                "{a:?} against {b:?}"
            );
        }
    }

    #[test]
    fn intersects_octagon_agrees_with_the_widened_tile_shape_test() {
        for (a, b) in every_regular_pair() {
            let octagon = match b {
                RegularTileShape::Box(boxed) => boxed.to_int_octagon(),
                RegularTileShape::Octagon(oct) => oct,
            };
            assert_eq!(
                a.intersects_octagon(&octagon),
                a.to_tile_shape().intersects_octagon(&octagon),
                "{a:?} against {octagon:?}"
            );
        }
    }

    #[test]
    fn intersects_box_agrees_with_the_widened_tile_shape_test() {
        for (a, b) in every_regular_pair() {
            let boxed = b.bounding_box();
            assert_eq!(
                a.intersects_box(&boxed),
                a.to_tile_shape().intersects_box(&boxed),
                "{a:?} against {boxed:?}"
            );
        }
    }
}
