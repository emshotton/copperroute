use std::path::Path;

use fr_geometry::{FloatPoint, IntBox};
use fr_router::ExpansionCostFactor;
use fr_router::autoroute::maze::DestinationDistance;

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

fn transcript() -> String {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/p6t8-destination-distance.txt");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn fmt(value: f64) -> String {
    if value == f64::from(i32::MAX) {
        return "INT_MAX".to_string();
    }
    format!("{value:.6}")
}

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
