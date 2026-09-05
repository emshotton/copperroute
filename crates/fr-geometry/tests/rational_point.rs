use fr_geometry::{IntPoint, Line, Point, RationalPoint};
use num_bigint::BigInt;
use num_traits::One;

/// The same point, as an `IntPoint` and as the `RationalPoint` `(x/1, y/1)`.
fn both_ways(x: i32, y: i32) -> (IntPoint, RationalPoint) {
    (
        IntPoint::new(x, y),
        RationalPoint::new(BigInt::from(x), BigInt::from(y), BigInt::one()),
    )
}

#[test]
fn perpendicular_projection_matches_the_hand_derived_witnesses() {
    // W1
    let horizontal = Line::from_coords(0, 10, 10, 10);
    let (ip, rp) = both_ways(0, 0);
    assert_eq!(
        ip.perpendicular_projection(&horizontal),
        Point::Int(IntPoint::new(0, 10)),
        "W1: the IntPoint arm is the correct one and is unchanged"
    );
    assert_eq!(
        rp.perpendicular_projection(&horizontal),
        Point::Int(IntPoint::new(0, 10)),
        "W1: fixed: T11 (#5) — was (0,-10), the sign of the det term"
    );

    // W2
    let slanted = Line::from_coords(0, 5, 10, 15);
    let (ip, rp) = both_ways(10, 5);
    assert_eq!(
        ip.perpendicular_projection(&slanted),
        Point::Int(IntPoint::new(5, 10)),
        "W2: the IntPoint arm"
    );
    assert_eq!(
        rp.perpendicular_projection(&slanted),
        Point::Int(IntPoint::new(5, 10)),
        "W2: fixed: T11 (#5) — was (5,5), which is not on the line at all"
    );
}

/// The control the answer key pairs with the witnesses: a line **through the origin** has
/// `det == 0`, so the wrong sign multiplies zero and the two arms agreed even before the fix.
/// Keeping it here states why the generator below must exclude such lines.
#[test]
fn a_line_through_the_origin_agreed_even_before_the_fix() {
    let through_origin = Line::from_coords(0, 0, 10, 0);
    assert_eq!(through_origin.a.determinant(&through_origin.b), 0);
    let (ip, rp) = both_ways(3, 7);
    assert_eq!(
        ip.perpendicular_projection(&through_origin),
        Point::Int(IntPoint::new(3, 0))
    );
    assert_eq!(
        rp.perpendicular_projection(&through_origin),
        Point::Int(IntPoint::new(3, 0))
    );
}

/// The plan's binding test: over a **generated** line set none of whose lines passes through the
/// origin, `RationalPoint::perpendicular_projection` and `IntPoint::perpendicular_projection`
/// agree exactly — no tolerance, because both are exact rational arithmetic over the same
/// formula and a projection that divides out lands in the same `Point::Int` either way.
///
/// The `det != 0` filter is asserted rather than assumed: without it the set would contain lines
/// on which the bug is invisible, and a bug-blind case in a generated set silently weakens the
/// whole test.
#[test]
fn perpendicular_projection_agrees_with_int_point() {
    let mut compared = 0usize;
    let mut skipped_through_origin = 0usize;

    for ax in [-7i32, -1, 0, 3, 11] {
        for ay in [-5i32, 0, 2, 9] {
            for bx in [-9i32, -2, 4, 13] {
                for by in [-6i32, 1, 8, 15] {
                    let a = IntPoint::new(ax, ay);
                    let b = IntPoint::new(bx, by);
                    if a == b {
                        continue; // not a line
                    }
                    let line = Line::new(a, b);
                    if line.a.determinant(&line.b) == 0 {
                        skipped_through_origin += 1;
                        continue; // the bug is invisible here — see the module doc
                    }
                    for px in [-8i32, 0, 5, 17] {
                        for py in [-4i32, 0, 6, 12] {
                            let (ip, rp) = both_ways(px, py);
                            assert_eq!(
                                rp.perpendicular_projection(&line),
                                ip.perpendicular_projection(&line),
                                "line {line:?}, point ({px},{py})"
                            );
                            compared += 1;
                        }
                    }
                }
            }
        }
    }

    assert!(
        skipped_through_origin > 0,
        "the filter must actually be filtering — otherwise it is not guarding anything"
    );
    assert!(
        compared > 1000,
        "only {compared} comparisons made; the generated set collapsed"
    );
}
