use copper_geometry::FloatPoint;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Corridor {
    axis: FloatPoint,
    along: [f64; 2],
    across: [f64; 2],
}

impl Corridor {
    pub fn outside_fraction(&self, from: &FloatPoint, to: &FloatPoint) -> f64 {
        if from == to {
            return 0.0;
        }
        let project = |p: &FloatPoint| {
            [
                p.x * self.axis.x + p.y * self.axis.y,
                p.y * self.axis.x - p.x * self.axis.y,
            ]
        };
        let a = project(from);
        let b = project(to);
        let mut enter: f64 = 0.0;
        let mut leave: f64 = 1.0;
        for (i, limits) in [self.along, self.across].iter().enumerate() {
            let delta = b[i] - a[i];
            if delta == 0.0 {
                if a[i] < limits[0] || a[i] > limits[1] {
                    return 1.0;
                }
                continue;
            }
            let t0 = (limits[0] - a[i]) / delta;
            let t1 = (limits[1] - a[i]) / delta;
            enter = enter.max(t0.min(t1));
            leave = leave.min(t0.max(t1));
            if enter >= leave {
                return 1.0;
            }
        }
        (1.0 - (leave - enter)).clamp(0.0, 1.0)
    }

    pub fn for_net(board: &copper_board::Board, net: i32) -> Option<Self> {
        use std::collections::BTreeMap;
        let mut centers: BTreeMap<(i32, i32), (f64, f64, usize)> = BTreeMap::new();
        for id in board.get_pins() {
            let item = board.get_item(id)?;
            if item.component_id() <= 0 {
                continue;
            }
            let Some(center) = board.drill_center(id) else {
                continue;
            };
            let center = center.to_float();
            for &n in item.net_nos() {
                if n <= 0 || board.rules.nets.get(n).is_some_and(|n| n.contains_plane()) {
                    continue;
                }
                let p = centers.entry((n, item.component_id())).or_default();
                p.0 += center.x;
                p.1 += center.y;
                p.2 += 1;
            }
        }
        let mut nets: BTreeMap<i32, Vec<(i32, FloatPoint)>> = BTreeMap::new();
        for ((n, component), (x, y, count)) in centers {
            nets.entry(n).or_default().push((
                component,
                FloatPoint::new(x / count as f64, y / count as f64),
            ));
        }
        let mut pairs: BTreeMap<(i32, i32), Vec<(i32, FloatPoint, FloatPoint)>> = BTreeMap::new();
        for (n, pins) in nets
            .iter()
            .filter(|(_, pins)| (2..=6).contains(&pins.len()))
        {
            for (i, (a, from)) in pins.iter().enumerate() {
                for (b, to) in &pins[i + 1..] {
                    pairs.entry((*a, *b)).or_default().push((*n, *from, *to));
                }
            }
        }
        let (axis, signals) = choose_chain(pairs.into_iter().collect(), net)?;
        let mut along = [f64::INFINITY, f64::NEG_INFINITY];
        let mut across = along;
        for (_, a, b) in &signals {
            for p in [a, b] {
                let u = p.x * axis.x + p.y * axis.y;
                let v = p.y * axis.x - p.x * axis.y;
                along[0] = along[0].min(u);
                along[1] = along[1].max(u);
                across[0] = across[0].min(v);
                across[1] = across[1].max(v);
            }
        }
        let capacity = (0..board.get_layer_count())
            .map(|layer| {
                let clearance = board
                    .rules
                    .clearance_matrix
                    .max_value_on_layer(layer)
                    .max(0) as f64;
                signals
                    .iter()
                    .map(|(n, _, _)| {
                        2.0 * f64::from(board.rules.get_trace_half_width(*n, layer).max(0))
                            + clearance
                    })
                    .sum::<f64>()
            })
            .fold(0.0, f64::max);
        let center = (across[0] + across[1]) * 0.5;
        let radius = ((across[1] - across[0]) * 0.5).max(capacity * 0.5);
        across = [center - radius, center + radius];
        along = [along[0] - capacity * 0.5, along[1] + capacity * 0.5];
        Some(Self {
            axis,
            along,
            across,
        })
    }
}

type Signal = (i32, FloatPoint, FloatPoint);
type Group = ((i32, i32), Vec<Signal>);

fn choose_chain(mut groups: Vec<Group>, net: i32) -> Option<(FloatPoint, Vec<Signal>)> {
    groups.retain(|(_, signals)| signals.len() >= 4);
    groups.sort_by_key(|(pair, signals)| (std::cmp::Reverse(signals.len()), *pair));
    let mut claimed = std::collections::BTreeSet::new();
    for (_, mut signals) in groups {
        signals.retain(|s| !claimed.contains(&s.0));
        if signals.len() < 4 {
            continue;
        }
        let dx = signals.iter().map(|(_, a, b)| b.x - a.x).sum::<f64>() / signals.len() as f64;
        let dy = signals.iter().map(|(_, a, b)| b.y - a.y).sum::<f64>() / signals.len() as f64;
        let length = dx.hypot(dy);
        if length == 0.0 {
            continue;
        }
        let axis = FloatPoint::new(dx / length, dy / length);
        while signals.len() >= 4 {
            let projected: Vec<_> = signals
                .iter()
                .map(|(n, a, b)| (*n, a.y * axis.x - a.x * axis.y, b.y * axis.x - b.x * axis.y))
                .collect();
            let chain = ordered_chain(&projected);
            if chain.is_empty() {
                break;
            }
            claimed.extend(chain.iter().copied());
            if !chain.contains(&net) {
                signals.retain(|s| !chain.contains(&s.0));
                continue;
            }
            signals.retain(|s| chain.contains(&s.0));
            return Some((axis, signals));
        }
    }
    None
}

fn ordered_chain(signals: &[(i32, f64, f64)]) -> Vec<i32> {
    let mut sorted = signals.to_vec();
    sorted.sort_by(|a, b| {
        a.1.total_cmp(&b.1)
            .then(a.2.total_cmp(&b.2))
            .then(a.0.cmp(&b.0))
    });
    let mut lengths = vec![1; sorted.len()];
    let mut previous = vec![None; sorted.len()];
    for i in 0..sorted.len() {
        for j in 0..i {
            if sorted[j].2 <= sorted[i].2 && lengths[j] + 1 > lengths[i] {
                lengths[i] = lengths[j] + 1;
                previous[i] = Some(j);
            }
        }
    }
    let Some(mut end) = (0..sorted.len()).max_by_key(|i| lengths[*i]) else {
        return Vec::new();
    };
    if lengths[end] < 4 {
        return Vec::new();
    }
    let mut result = vec![sorted[end].0];
    while let Some(i) = previous[end] {
        end = i;
        result.push(sorted[end].0);
    }
    result.reverse();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlapping_groups_do_not_count_lanes_assigned_to_another_corridor() {
        let signals = |nets: &[i32], x: f64| {
            nets.iter()
                .enumerate()
                .map(|(i, n)| {
                    (
                        *n,
                        FloatPoint::new(x, i as f64),
                        FloatPoint::new(x + 100.0, i as f64),
                    )
                })
                .collect()
        };
        let groups = vec![
            ((1, 2), signals(&[1, 2, 3, 4, 5], 0.0)),
            ((3, 4), signals(&[2, 3, 6, 7, 8, 9], 200.0)),
        ];
        assert!(
            choose_chain(groups.clone(), 1).is_none(),
            "the remaining three signals do not form a four-lane group"
        );
        let (_, a) = choose_chain(groups.clone(), 2).unwrap();
        let (_, b) = choose_chain(groups, 3).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.len(), 6);
    }

    #[test]
    fn cost_counts_only_the_part_of_a_segment_outside_the_band() {
        let c = Corridor {
            axis: FloatPoint::new(1.0, 0.0),
            along: [0.0, 10.0],
            across: [-1.0, 1.0],
        };
        assert_eq!(
            c.outside_fraction(&FloatPoint::new(2.0, 0.0), &FloatPoint::new(8.0, 0.0)),
            0.0
        );
        assert_eq!(
            c.outside_fraction(&FloatPoint::new(-5.0, 0.0), &FloatPoint::new(15.0, 0.0)),
            0.5
        );
        assert_eq!(
            c.outside_fraction(&FloatPoint::new(2.0, 5.0), &FloatPoint::new(8.0, 5.0)),
            1.0
        );
        assert_eq!(
            c.outside_fraction(&FloatPoint::new(0.0, 0.0), &FloatPoint::new(0.0, 0.0)),
            0.0
        );
    }

    #[test]
    fn corridor_cost_is_additive_under_segment_subdivision() {
        let c = Corridor {
            axis: FloatPoint::new(1.0, 0.0),
            along: [0.0, 10.0],
            across: [-1.0, 1.0],
        };
        let a = FloatPoint::new(-5.0, -2.0);
        let b = FloatPoint::new(15.0, 2.0);
        let m = a.middle_point(&b);
        let whole = a.distance(&b) * c.outside_fraction(&a, &b);
        let halves = a.distance(&m) * c.outside_fraction(&a, &m)
            + m.distance(&b) * c.outside_fraction(&m, &b);
        assert!(whole > 0.0);
        assert!((whole - halves).abs() < 1e-9);
        assert_eq!(c.outside_fraction(&a, &b), c.outside_fraction(&b, &a));
    }

    #[test]
    fn rotating_the_corridor_and_segment_preserves_the_penalty() {
        let a = Corridor {
            axis: FloatPoint::new(1.0, 0.0),
            along: [0.0, 10.0],
            across: [-1.0, 1.0],
        };
        let b = Corridor {
            axis: FloatPoint::new(0.0, 1.0),
            along: [0.0, 10.0],
            across: [-1.0, 1.0],
        };
        for (from, to) in [
            (FloatPoint::new(-5.0, -2.0), FloatPoint::new(15.0, 2.0)),
            (FloatPoint::new(1.0, 3.0), FloatPoint::new(8.0, 4.0)),
        ] {
            let fraction = a.outside_fraction(&from, &to);
            assert!((0.0..=1.0).contains(&fraction));
            assert_eq!(
                fraction,
                b.outside_fraction(
                    &FloatPoint::new(-from.y, from.x),
                    &FloatPoint::new(-to.y, to.x)
                )
            );
        }
    }

    #[test]
    fn a_crossing_signal_is_not_treated_as_an_ordered_lane() {
        let signals = [
            (10, 0.0, 0.0),
            (20, 1.0, 1.0),
            (99, 1.5, -1.0),
            (30, 2.0, 2.0),
            (40, 3.0, 3.0),
        ];
        assert_eq!(ordered_chain(&signals), [10, 20, 30, 40]);
        assert!(
            ordered_chain(&[(1, 0.0, 3.0), (2, 1.0, 2.0), (3, 2.0, 1.0), (4, 3.0, 0.0)]).is_empty()
        );
    }
}
