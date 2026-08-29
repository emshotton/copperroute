//! Plan 6 Task 8: `autoroute.maze.DestinationDistance` (DestinationDistance.java:11-391) —
//! the maze's admissible lower bound on the remaining cost.
//!
//! # Where the numbers come from
//!
//! Every expected value below is read off the clone's **HEAD jar**, not off this port.
//! `scripts/differential/java/probes/P6T8Probe.java` mode `dd` constructs the real
//! `DestinationDistance` in seven configurations, reads the nine package-private cost fields the
//! constructor derives, and prints `calculate(FloatPoint, layer)`, `calculate(IntBox, layer)` and
//! `calculateCheapDistance(IntBox, layer)` over a fixed point/box grid. Its stdout is committed
//! verbatim as `tests/data/p6t8-destination-distance.txt` (248 rows) and this test replays it
//! line by line, so a regenerated probe and a stale expectation cannot silently disagree.
//!
//! The seven configurations exist to reach every early return of
//! `calculate(IntBox, int)` (`:122-379`): `activeLayerCount <= 1` (`:228`), `== 2` (`:259`),
//! `== 3` (`:281`) and the four-layer fall-through on layer 0; `<= 2` (`:321`) and `== 3`
//! (`:337`) on the solder side; the inner-layer arm (`:349-378`); the `boxIsEmpty` short circuit
//! (`:123-125`); and the three `…BoxIsEmpty` guards (`:222`, `:298`, `:353`) that decide whether
//! the one-layer `weightedDistance` term is taken at all.

use std::path::Path;

use fr_geometry::{FloatPoint, IntBox};
use fr_router::ExpansionCostFactor;
use fr_router::autoroute::maze::DestinationDistance;

// =================================================================================================
// The probe's configurations, rebuilt from scratch
// =================================================================================================

/// `P6T8Probe.COSTS_4`: four layers whose min/max horizontal-vs-vertical costs are all distinct,
/// so `minComponentSideTraceCost < minSolderSideTraceCost` (`:306`) and its mirror (`:235`) take
/// opposite branches, and `maxInnerSideTraceCost` (`:86-94`) really comes from an inner layer.
fn costs_4() -> Vec<ExpansionCostFactor> {
    vec![
        ExpansionCostFactor {
            horizontal: 1.0,
            vertical: 2.0,
        },
        ExpansionCostFactor {
            horizontal: 0.5,
            vertical: 1.2,
        },
        ExpansionCostFactor {
            horizontal: 2.5,
            vertical: 1.5,
        },
        ExpansionCostFactor {
            horizontal: 3.0,
            vertical: 5.0,
        },
    ]
}

/// `P6T8Probe.destinationDistance()`, case by case: the constructor arguments and the `join`
/// calls the probe makes, keyed by the tag it prints on every row.
fn case(tag: &str) -> DestinationDistance {
    let (costs, active, min_normal, min_cheap): (Vec<ExpansionCostFactor>, Vec<bool>, f64, f64) =
        match tag {
            "allActive4" | "empty4" | "compOnly4" => {
                (costs_4(), vec![true, true, true, true], 50.0, 40.0)
            }
            "active3" => (costs_4(), vec![true, true, false, true], 50.0, 40.0),
            "active2" => (costs_4(), vec![true, false, false, true], 50.0, 40.0),
            "active1solder" => (costs_4(), vec![false, false, false, true], 50.0, 40.0),
            "twoLayer" => (
                vec![
                    ExpansionCostFactor {
                        horizontal: 1.0,
                        vertical: 4.0,
                    },
                    ExpansionCostFactor {
                        horizontal: 3.0,
                        vertical: 1.0,
                    },
                ],
                vec![true, true],
                25.0,
                20.0,
            ),
            other => panic!("unknown probe case {other}"),
        };
    let mut d = DestinationDistance::new(&costs, &active, min_normal, min_cheap);
    match tag {
        "allActive4" | "active3" => {
            d.join(&IntBox::from_coords(0, 0, 100, 100), 0);
            d.join(&IntBox::from_coords(500, 500, 600, 600), 3);
            d.join(&IntBox::from_coords(200, 200, 300, 300), 1);
        }
        "empty4" => {}
        "active2" => {
            d.join(&IntBox::from_coords(0, 0, 100, 100), 0);
            d.join(&IntBox::from_coords(500, 500, 600, 600), 3);
        }
        "active1solder" => d.join(&IntBox::from_coords(500, 500, 600, 600), 3),
        "compOnly4" => d.join(&IntBox::from_coords(0, 0, 100, 100), 0),
        "twoLayer" => {
            d.join(&IntBox::from_coords(0, 0, 100, 100), 0);
            d.join(&IntBox::from_coords(500, 500, 600, 600), 1);
        }
        other => panic!("unknown probe case {other}"),
    }
    d
}

// =================================================================================================
// The probe transcript
// =================================================================================================

fn transcript() -> String {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/p6t8-destination-distance.txt");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// `P6T8Probe.f(double)`: `Integer.MAX_VALUE` prints as `INT_MAX`, everything else as `%.6f`.
fn fmt(value: f64) -> String {
    if value == f64::from(i32::MAX) {
        return "INT_MAX".to_string();
    }
    format!("{value:.6}")
}

/// Replays every row of the probe's stdout against the port.
///
/// This is one test rather than eight, because the probe transcript *is* the specification: a
/// row it prints that the port cannot answer is a failure, and a row the port answers that the
/// probe does not print would mean the transcript is stale. Both are checked.
#[test]
fn every_row_of_the_jvm_transcript_is_reproduced() {
    let text = transcript();
    let mut current: Option<(String, DestinationDistance)> = None;
    let mut rows = 0usize;
    let mut cost_rows = 0usize;

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(tag) = line.strip_prefix("case ") {
            current = Some((tag.to_string(), case(tag)));
            continue;
        }
        let (tag, d) = current.as_ref().expect("a `case` line before any data row");

        if let Some(rest) = line.strip_prefix("costs ") {
            // `minComp=… maxComp=… …` — the nine constructor-derived fields.
            let got = format!(
                "minComp={} maxComp={} minSold={} maxSold={} maxInner={} minCompInner={} \
                 minSoldInner={} minCompSoldInner={}",
                fmt(d.min_component_side_trace_cost),
                fmt(d.max_component_side_trace_cost),
                fmt(d.min_solder_side_trace_cost),
                fmt(d.max_solder_side_trace_cost),
                fmt(d.max_inner_side_trace_cost),
                fmt(d.min_component_inner_trace_cost),
                fmt(d.min_solder_inner_trace_cost),
                fmt(d.min_component_solder_inner_trace_cost),
            );
            assert_eq!(got, rest, "case {tag}: constructor-derived costs");
            cost_rows += 1;
            continue;
        }

        // `<tag> point(x,y) layer=N => V`
        if let Some((point, tail)) = parse_point_row(line, tag) {
            let (layer, expected) = tail;
            assert_eq!(
                fmt(d.calculate_from_point(&point, layer)),
                expected,
                "case {tag}: calculate(FloatPoint({}, {}), {layer})",
                point.x,
                point.y
            );
            rows += 1;
            continue;
        }

        // `<tag> box[a,b..c,d] layer=N => V cheap=C again=A`
        let (b, layer, expected, cheap, again) =
            parse_box_row(line, tag).unwrap_or_else(|| panic!("unparsed transcript row: {line}"));
        assert_eq!(
            fmt(d.calculate(&b, layer)),
            expected,
            "case {tag}: calculate({b:?}, {layer})"
        );
        assert_eq!(
            fmt(d.calculate_cheap_distance(&b, layer)),
            cheap,
            "case {tag}: calculateCheapDistance({b:?}, {layer})"
        );
        assert_eq!(
            fmt(d.calculate(&b, layer)),
            again,
            "case {tag}: calculate is unchanged by the cheap call (hazard J)"
        );
        rows += 1;
    }

    assert_eq!(cost_rows, 7, "the transcript has seven configurations");
    assert_eq!(rows, 234, "the transcript has 234 value rows");
}

fn parse_point_row(line: &str, tag: &str) -> Option<(FloatPoint, (usize, String))> {
    let rest = line.strip_prefix(tag)?.trim_start();
    let rest = rest.strip_prefix("point(")?;
    let (coords, rest) = rest.split_once(") layer=")?;
    let (x, y) = coords.split_once(',')?;
    let (layer, expected) = rest.split_once(" => ")?;
    Some((
        FloatPoint::new(x.parse().ok()?, y.parse().ok()?),
        (layer.trim().parse().ok()?, expected.to_string()),
    ))
}

#[allow(clippy::type_complexity)]
fn parse_box_row(line: &str, tag: &str) -> Option<(IntBox, usize, String, String, String)> {
    let rest = line.strip_prefix(tag)?.trim_start();
    let rest = rest.strip_prefix("box[")?;
    let (coords, rest) = rest.split_once("] layer=")?;
    let (ll, ur) = coords.split_once("..")?;
    let (llx, lly) = ll.split_once(',')?;
    let (urx, ury) = ur.split_once(',')?;
    let (layer, rest) = rest.split_once(" => ")?;
    let mut parts = rest.split_whitespace();
    let expected = parts.next()?.to_string();
    let cheap = parts.next()?.strip_prefix("cheap=")?.to_string();
    let again = parts.next()?.strip_prefix("again=")?.to_string();
    Some((
        IntBox::from_coords(
            llx.parse().ok()?,
            lly.parse().ok()?,
            urx.parse().ok()?,
            ury.parse().ok()?,
        ),
        layer.trim().parse().ok()?,
        expected,
        cheap,
        again,
    ))
}

// =================================================================================================
// The two behaviours the transcript pins but does not name
// =================================================================================================

/// Hazard J: `calculateCheapDistance` (`:382-390`) **mutates** `minNormalViaCost` and restores it
/// afterwards. The port passes the cheap cost down instead of writing a field, so it takes
/// `&self`; this asserts that the receiver's own answers are identical before and after.
#[test]
fn calculate_cheap_does_not_mutate_the_receiver() {
    let d = case("allActive4");
    let b = IntBox::from_coords(700, 200, 900, 400);

    let before: Vec<f64> = (0..4).map(|layer| d.calculate(&b, layer)).collect();
    let cheap: Vec<f64> = (0..4)
        .map(|layer| d.calculate_cheap_distance(&b, layer))
        .collect();
    let after: Vec<f64> = (0..4).map(|layer| d.calculate(&b, layer)).collect();

    assert_eq!(before, after, "the receiver is unchanged");
    assert!(
        cheap.iter().zip(&before).all(|(c, n)| c <= n),
        "the cheap bound is never above the normal one: {cheap:?} vs {before:?}"
    );
    assert_ne!(cheap, before, "and on this box it is strictly cheaper");
}

/// Java's `Math.min`/`Math.max(double, double)` **propagate** a NaN (`if (a != a) return a;`);
/// Rust's `f64::min`/`f64::max` **absorb** it (`x.min(NaN) == x`, IEEE 754-2019 `minimumNumber`).
/// Every one of the 25 `Math.min`/`Math.max` sites in this class is therefore
/// [`fr_geometry::java_min`] / [`fr_geometry::java_max`], the transcription
/// `IntBox::weighted_distance` already uses.
///
/// It matters because quirk #170 — `MazeListElement.compareTo`'s NaN fall-through, and the reason
/// the comparator is transcribed as a `<`/`>` chain rather than `total_cmp` — argues that a NaN
/// reaches `sortingValue` *through this method*. On `f64::min` it never could: the first
/// `result = result.min(tmp)` would discard it. The two decisions have to agree.
///
/// The literals are `P6T8Probe` mode `nan` (`tests/data/p6t8-nan-propagation.txt`): a NaN
/// horizontal trace cost on layer 0 leaves `minComp=2.0` but `maxComp=NaN`, and every one of the
/// four layer arms then answers `NaN`.
#[test]
fn a_nan_trace_cost_propagates_through_calculate_as_java_does() {
    let costs = vec![
        ExpansionCostFactor {
            horizontal: f64::NAN,
            vertical: 2.0,
        },
        ExpansionCostFactor {
            horizontal: 0.5,
            vertical: 1.2,
        },
        ExpansionCostFactor {
            horizontal: 2.5,
            vertical: 1.5,
        },
        ExpansionCostFactor {
            horizontal: 3.0,
            vertical: 5.0,
        },
    ];
    let mut d = DestinationDistance::new(&costs, &[true, true, true, true], 50.0, 40.0);
    d.join(&IntBox::from_coords(0, 0, 100, 100), 0);
    d.join(&IntBox::from_coords(500, 500, 600, 600), 3);
    d.join(&IntBox::from_coords(200, 200, 300, 300), 1);

    // `costs minComp=2.000000 maxComp=NaN minSold=3.000000 maxSold=5.000000 maxInner=NaN
    //  minCompInner=NaN minSoldInner=NaN minCompSoldInner=NaN` — `NaN < 2.0` is false, so `:64`'s
    // `else` puts the NaN in `maxComponentSideTraceCost`, and `Math.min` carries it into the four
    // derived costs from there.
    assert_eq!(d.min_component_side_trace_cost, 2.0);
    assert!(d.max_component_side_trace_cost.is_nan());
    assert_eq!(d.min_solder_side_trace_cost, 3.0);
    assert_eq!(d.max_solder_side_trace_cost, 5.0);
    assert!(d.max_inner_side_trace_cost.is_nan(), "Math.min at :86");
    assert!(d.min_component_inner_trace_cost.is_nan(), ":95");
    assert!(d.min_solder_inner_trace_cost.is_nan(), ":96");
    assert!(d.min_component_solder_inner_trace_cost.is_nan(), ":97-98");

    // `nan box[...] layer=N => NaN isNaN=true` for both boxes on all four layers.
    for layer in 0..4 {
        for b in [
            IntBox::from_coords(0, 0, 100, 100),
            IntBox::from_coords(700, 200, 900, 400),
        ] {
            assert!(
                d.calculate(&b, layer).is_nan(),
                "calculate({b:?}, {layer}) must answer NaN, as Java's Math.min does;                  f64::min would have discarded it"
            );
            assert!(d.calculate_cheap_distance(&b, layer).is_nan());
        }
    }

    // And this is the NaN quirk #170's fall-through consumes: `sortingValue = expansionValue +
    // destinationDistance.calculate(...)` (MazeSearchEngine.java:884).
    assert!((1.0 + d.calculate(&IntBox::from_coords(0, 0, 100, 100), 0)).is_nan());
}

/// `boxIsEmpty` (`:36`, `:123-125`): before any `join`, every layer answers `Integer.MAX_VALUE` —
/// **as a `double`**, so the port must answer `2147483647.0`, not `f64::MAX` and not infinity.
#[test]
fn nothing_joined_answers_integer_max_value_as_a_double() {
    let d = case("empty4");
    for layer in 0..4 {
        assert_eq!(
            d.calculate(&IntBox::from_coords(0, 0, 1, 1), layer),
            f64::from(i32::MAX)
        );
        assert_eq!(
            d.calculate_from_point(&FloatPoint::new(0.0, 0.0), layer),
            f64::from(i32::MAX)
        );
    }
}

/// `join` (`:102-114`) buckets by layer into exactly three boxes: layer 0, layer `layerCount - 1`
/// and "every inner layer", the last of which is one shared union. On a two-layer board the
/// inner bucket is unreachable, which is why `twoLayer` exists in the transcript.
#[test]
fn join_buckets_into_component_solder_and_one_shared_inner_box() {
    let costs = costs_4();
    let active = [true, true, true, true];
    let mut d = DestinationDistance::new(&costs, &active, 50.0, 40.0);
    d.join(&IntBox::from_coords(0, 0, 10, 10), 1);
    let only_layer_1 = d.calculate(&IntBox::from_coords(1000, 1000, 1001, 1001), 2);

    let mut e = DestinationDistance::new(&costs, &active, 50.0, 40.0);
    e.join(&IntBox::from_coords(0, 0, 10, 10), 2);
    let only_layer_2 = e.calculate(&IntBox::from_coords(1000, 1000, 1001, 1001), 2);

    assert_eq!(
        only_layer_1, only_layer_2,
        "layers 1 and 2 share one `innerSideBox`"
    );
}
