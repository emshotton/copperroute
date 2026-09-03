//! Plan 9 Task 11, quirk #7: `IntBox.borderLineIndex` and `IntOctagon.borderLineIndex` were
//! stubs that logged a warning and returned `-1` — for **any** line, including the shape's own
//! border lines.
//!
//! The specification was already in the same file. `borderLine(i)` says exactly which line each
//! index names, so `borderLineIndex` is its inverse and needs no external oracle:
//!
//! ```text
//! IntBox b = [ll .. ur]
//!   border_line(0) = Line((0, ll.y),  (1, ll.y))     lower, directed +x
//!   border_line(1) = Line((ur.x, 0),  (ur.x, 1))     right, directed +y
//!   border_line(2) = Line((0, ur.y),  (-1, ur.y))    upper, directed −x
//!   border_line(3) = Line((ll.x, 0),  (ll.x, −1))    left,  directed −y
//! ```
//!
//! `Simplex::border_line_index` (Simplex.java:668) has always been implemented, and it is
//! implemented **geometrically**: `Line::equals_geometric`, i.e. Java's own `Line.equals`. The
//! box and the octagon now answer the same way, so the three arms of
//! `TileShape::border_line_index` finally agree about what an index means.
//!
//! ## The two traps
//!
//! **Orientation.** The freerouting tile convention is *the shape lies on the RIGHT of every
//! border line*. The **reversed** line — the same point set, the opposite direction — is
//! therefore not that border line, and must answer `None`. An implementation that only tests
//! collinearity passes the round trip and is still wrong at the caller, because
//! `ShapeAndEntrySide` uses the index to decide which side the trace enters from.
//!
//! **Shared points.** For a box anchored at the origin, `border_line(0)` and `border_line(3)`
//! both pass through `(0,0)`. Equality by defining points alone is therefore not enough either.
//! `equals_geometric` is collinearity of *both* end points **plus** a positive direction
//! projection, which settles both traps at once.

use fr_geometry::{IntBox, IntOctagon, IntPoint, Line, TileShape};

fn box_() -> IntBox {
    IntBox::new(IntPoint::new(0, 0), IntPoint::new(100, 50))
}

/// An octagon with all eight sides genuinely present, so no two border lines coincide. The four
/// diagonals cut the `[-100,-100 .. 100,100]` box's corners off (`|x| + |y| = 200 > 150`), which
/// [`the_octagon_fixture_really_has_eight_distinct_sides`] asserts rather than assumes — a
/// degenerate octagon would make the round trip pass for the wrong reason, since `find` answers
/// the *first* matching index.
fn oct() -> IntOctagon {
    IntOctagon::new(-100, -100, 100, 100, -150, 150, -150, 150).normalize()
}

#[test]
fn the_octagon_fixture_really_has_eight_distinct_sides() {
    let o = oct();
    for i in 0..o.border_line_count() {
        for j in 0..o.border_line_count() {
            if i != j {
                assert!(
                    !o.border_line(i).equals_geometric(&o.border_line(j)),
                    "border lines {i} and {j} coincide; the fixture is degenerate"
                );
            }
        }
    }
}

/// Quirk #7, the round trip. `border_line_index(border_line(i)) == Some(i)` for every `i` — the
/// contract `border_line` itself states. Before the fix every one of these was `None`.
#[test]
fn border_line_index_is_geometric() {
    let b = box_();
    for i in 0..4 {
        assert_eq!(
            b.border_line_index(&b.border_line(i)),
            Some(i),
            "fixed: T11 (#7) — box border line {i} was not recognised as its own"
        );
    }

    let o = oct();
    for i in 0..o.border_line_count() {
        assert_eq!(
            o.border_line_index(&o.border_line(i)),
            Some(i),
            "fixed: T11 (#7) — octagon border line {i} was not recognised as its own"
        );
    }
}

/// The round trip survives the shape being re-described by *different* points on the same line,
/// which is the whole reason the comparison has to be geometric rather than structural. The box
/// `[0,0 .. 100,50]`'s lower border is `y = 0` directed `+x`; `(-7,0) -> (12,0)` is the same line.
#[test]
fn a_border_line_described_by_other_points_is_still_that_border_line() {
    let b = box_();
    assert_eq!(b.border_line_index(&Line::from_coords(-7, 0, 12, 0)), Some(0));
    assert_eq!(
        b.border_line_index(&Line::from_coords(100, -40, 100, 900)),
        Some(1)
    );
}

/// A line that is not a border line at all answers `None` — the stub's old answer, now for the
/// right reason.
#[test]
fn a_line_that_is_not_a_border_line_answers_none() {
    let b = box_();
    assert_eq!(b.border_line_index(&Line::from_coords(0, 0, 1, 1)), None);
    // Parallel to the lower border, but not on it.
    assert_eq!(b.border_line_index(&Line::from_coords(0, 3, 1, 3)), None);

    let o = oct();
    assert_eq!(o.border_line_index(&Line::from_coords(0, 0, 1, 1)), None);
}

/// **The orientation trap.** The reversed border line covers the same points and is *not* the
/// same border line: the tile lies on the right of `border_line(i)`, so a line running the other
/// way has the tile on its left and names no edge of this shape.
///
/// A collinearity-only implementation passes every other test in this file and fails here, which
/// is why this test exists separately.
#[test]
fn the_reversed_border_line_is_not_that_border_line() {
    let b = box_();
    for i in 0..4 {
        assert_eq!(
            b.border_line_index(&b.border_line(i).opposite()),
            None,
            "box border line {i} reversed must not answer {i}"
        );
    }

    let o = oct();
    for i in 0..o.border_line_count() {
        assert_eq!(
            o.border_line_index(&o.border_line(i).opposite()),
            None,
            "octagon border line {i} reversed must not answer {i}"
        );
    }
}

/// **The shared-point trap.** For a box at the origin, border lines 0 (lower, `+x`) and 3 (left,
/// `−y`) both pass through `(0,0)`; only the direction tells them apart. Each must answer its own
/// index and not the other's.
#[test]
fn two_border_lines_through_the_same_point_are_told_apart() {
    let b = box_();
    assert_eq!(b.border_line(0).a, IntPoint::new(0, 0));
    assert_eq!(b.border_line(3).a, IntPoint::new(0, 0));
    assert_eq!(b.border_line_index(&b.border_line(0)), Some(0));
    assert_eq!(b.border_line_index(&b.border_line(3)), Some(3));
}

/// The three arms of `TileShape::border_line_index` now agree. `Simplex`'s was always
/// implemented; the box's and the octagon's answered `None` for their own border lines, so a
/// caller holding a `TileShape` got a different answer for the same geometry depending on which
/// representation the shape happened to be in. That is what `ShapeAndEntrySide` was reading.
#[test]
fn the_three_tile_shape_arms_agree() {
    let b = TileShape::Box(box_());
    for i in 0..4 {
        let line = b.border_line(i).expect("box has 4 border lines");
        assert_eq!(b.border_line_index(&line), Some(i));
        // The same shape as a simplex — the arm that was already right.
        let as_simplex = TileShape::Simplex(box_().to_simplex());
        let j = as_simplex
            .border_line_index(&line)
            .expect("fixed: T11 (#7) — the simplex arm always knew this line");
        // The simplex describes the same border line with *different* defining points, which is
        // exactly why the comparison has to be geometric rather than structural.
        assert!(
            as_simplex
                .border_line(j)
                .expect("in range")
                .equals_geometric(&line),
            "the simplex arm names the same line"
        );
    }
}
