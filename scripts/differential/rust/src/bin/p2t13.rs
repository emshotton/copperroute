//! Rust twin of `scripts/differential/java/P2T13.java` (Plan 2 Task 13).
//!
//! Feeds identical point sets to `copper_board::datastructures::delaunay::PlanarDelaunayTriangulation`
//! and prints `getEdgeLines()` in iteration order, plus the shuffle permutation and `validate()`.
//! See `scripts/differential/README.md`.

use copper_board::datastructures::delaunay::{DelaunayCorner, PlanarDelaunayTriangulation};
use copper_board::ids::ItemId;
use copper_geometry::int_point::IntPoint;
use copper_geometry::point::Point;

static mut STATE: u64 = 0;

fn next() -> u64 {
    unsafe {
        STATE ^= STATE << 13;
        STATE ^= STATE >> 7;
        STATE ^= STATE << 17;
        STATE
    }
}

fn rnd(bound: u64) -> i32 {
    (next() % bound) as i32
}

fn p(x: i32, y: i32) -> Point {
    Point::Int(IntPoint::new(x, y))
}

fn fmt(point: &Point) -> String {
    match point {
        Point::Int(ip) => format!("{},{}", ip.x, ip.y),
        Point::Rational(_) => "rational".to_string(),
    }
}

fn obj_id(object: Option<ItemId>) -> String {
    match object {
        None => "none".to_string(),
        Some(id) => id.0.to_string(),
    }
}

/// `java.util.Random` + `Collections.shuffle`, duplicated here (the ones inside `copper-board` are
/// private) so the driver can print the permutation the same way the Java driver does.
struct JavaRandom {
    seed: i64,
}

impl JavaRandom {
    fn new(seed: i64) -> JavaRandom {
        JavaRandom {
            seed: (seed ^ 0x5DEECE66D_i64) & ((1 << 48) - 1),
        }
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.seed = self
            .seed
            .wrapping_mul(0x5DEECE66D_i64)
            .wrapping_add(0xB)
            & ((1 << 48) - 1);
        (self.seed >> (48 - bits)) as i32
    }

    fn next_int(&mut self, bound: i32) -> i32 {
        let mut r = self.next(31);
        let m = bound - 1;
        if bound & m == 0 {
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

fn permutation(n: usize) -> Vec<usize> {
    let mut list: Vec<usize> = (0..n).collect();
    let mut rng = JavaRandom::new(99);
    let mut i = list.len();
    while i > 1 {
        let j = rng.next_int(i as i32) as usize;
        list.swap(i - 1, j);
        i -= 1;
    }
    list
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let n: usize = args.first().map_or(50, |a| a.parse().unwrap());
    let seed: u64 = args.get(1).map_or(42, |a| a.parse().unwrap());
    let mode: i32 = args.get(2).map_or(0, |a| a.parse().unwrap());
    unsafe { STATE = seed };

    // One entry per Java `Storable`: its id and its corners.
    let mut objects: Vec<(u32, Vec<Point>)> = Vec::new();
    let mut next_id = 1u32;
    let mut add = |objects: &mut Vec<(u32, Vec<Point>)>, corners: Vec<Point>| {
        objects.push((next_id, corners));
        next_id += 1;
    };
    match mode {
        1 => {
            add(&mut objects, vec![p(0, 0)]);
            add(&mut objects, vec![p(1000, 0)]);
            add(&mut objects, vec![p(1000, 1000)]);
            add(&mut objects, vec![p(0, 1000)]);
        }
        2 => {
            add(&mut objects, vec![p(0, 0)]);
            add(&mut objects, vec![p(500, 500)]);
            add(&mut objects, vec![p(1000, 1000)]);
        }
        3 => {
            add(&mut objects, vec![p(0, 0)]);
            add(&mut objects, vec![p(1000, 0)]);
            add(&mut objects, vec![p(1000, 1000)]);
            add(&mut objects, vec![p(0, 1000)]);
            add(&mut objects, vec![p(300, 400)]);
            add(&mut objects, vec![p(300, 400)]);
            add(&mut objects, vec![p(300, 400)]);
        }
        4 => {
            for _ in 0..n {
                let a = p(rnd(200_000) - 100_000, rnd(200_000) - 100_000);
                let b = p(rnd(200_000) - 100_000, rnd(200_000) - 100_000);
                add(&mut objects, vec![a, b]);
            }
        }
        5 => {
            for x in 0..5 {
                for y in 0..5 {
                    add(&mut objects, vec![p(x * 1000, y * 1000)]);
                }
            }
        }
        6 => {
            for _ in 0..n {
                add(&mut objects, vec![p(rnd(40) - 20, rnd(40) - 20)]);
            }
        }
        7 => {
            for i in 0..n {
                let angle = 2.0 * std::f64::consts::PI * (i as f64) / (n as f64);
                add(
                    &mut objects,
                    vec![p(
                        (30_000.0 * angle.cos()).round() as i32,
                        (30_000.0 * angle.sin()).round() as i32,
                    )],
                );
            }
        }
        _ => {
            for _ in 0..n {
                add(
                    &mut objects,
                    vec![p(rnd(200_000) - 100_000, rnd(200_000) - 100_000)],
                );
            }
        }
    }

    let mut corners: Vec<DelaunayCorner> = Vec::new();
    let mut inputs = String::new();
    for (id, points) in &objects {
        for point in points {
            inputs.push_str(&format!(" {}@{}", id, fmt(point)));
            corners.push(DelaunayCorner::new(ItemId(*id), point.clone()));
        }
    }
    println!(
        "mode={} objects={} corners={}",
        mode,
        objects.len(),
        corners.len()
    );
    println!("in:{inputs}");
    let perm = permutation(corners.len());
    println!(
        "perm=[{}]",
        perm.iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let triangulation = PlanarDelaunayTriangulation::new(&corners);
    let edges = triangulation.get_edge_lines();
    println!("count={}", edges.len());
    for (i, edge) in edges.iter().enumerate() {
        println!(
            "E{} so={} sp={} eo={} ep={}",
            i,
            obj_id(edge.start_object),
            fmt(&edge.start_point),
            obj_id(edge.end_object),
            fmt(&edge.end_point)
        );
    }
    println!("validate={}", triangulation.validate());
}
