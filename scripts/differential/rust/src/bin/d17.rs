use fr_geometry::circle::Circle;
use fr_geometry::float_point::FloatPoint;
use fr_geometry::int_box::IntBox;
use fr_geometry::int_point::IntPoint;
use fr_geometry::point::Point;
use fr_geometry::polygon_shape::PolygonShape;
use fr_geometry::polyline_area::PolylineArea;
use fr_geometry::polyline_shape::PolylineShapeOps;
use fr_geometry::shape::{Shape, ShapeOps};
use fr_geometry::tile_shape::TileShape;
use std::fmt::Write as _;
use std::io::Write as _;
use std::panic;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn range(&mut self, bound: i32) -> i32 {
        let v = self.next() >> 1;
        (v % bound as u64) as i32
    }
}

fn b(v: f64) -> String {
    format!("{:x}", v.to_bits())
}
fn bx(x: &IntBox) -> String {
    format!("B({},{},{},{})", x.ll.x, x.ll.y, x.ur.x, x.ur.y)
}
fn cls(t: &TileShape) -> &'static str {
    match t {
        TileShape::Box(_) => "Box",
        TileShape::Octagon(_) => "Octagon",
        TileShape::Simplex(_) => "Simplex",
    }
}
fn pts_str(pts: &[Point]) -> String {
    let mut s = String::from("[");
    for p in pts {
        let Point::Int(p) = p else { unreachable!() };
        let _ = write!(s, "({},{}),", p.x, p.y);
    }
    s.push(']');
    s
}
fn tile_str(t: &TileShape) -> String {
    let mut s = format!("{}/{}/{}/", cls(t), t.border_line_count(), b(t.area()));
    for f in t.corner_approx_arr() {
        let _ = write!(s, "{}:{},", b(f.x), b(f.y));
    }
    s
}
/// Runs `f`, mapping a panic onto the Java exception it stands for.
fn guard<T>(f: impl FnOnce() -> T + panic::UnwindSafe) -> Result<T, String> {
    panic::catch_unwind(f).map_err(|e| {
        let msg = e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        // Every panic this port raises on these paths stands for a Java NullPointerException.
        if msg.contains("PolygonShape.splitToConvex failed")
            || msg.contains("PolylineArea.splitToConvex failed")
            || msg.contains("TileShape.cutout returned null")
            || msg.contains("TileShape.nearestPointApprox returned null")
            || msg.contains("no nearest point on a shape without border lines")
            || msg.contains("no nearest border point on a shape without border lines")
        {
            "EXC:NullPointerException".to_string()
        } else {
            format!("EXC:{msg}")
        }
    })
}

fn fmt(r: Result<bool, String>) -> String {
    match r {
        Ok(v) => v.to_string(),
        Err(e) => e,
    }
}

fn main() {
    panic::set_hook(Box::new(|_| {}));
    let args: Vec<String> = std::env::args().collect();
    let cases: i32 = args[1].parse().unwrap();
    let mode: i32 = args[2].parse().unwrap();
    let mut rng = Rng(0x2545F4914F6CDD1D);
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let ranges = [6i32, 12, 40, 400];
    for c in 0..cases {
        let mut sb = String::new();
        let _ = write!(sb, "{c}|");
        let range = ranges[rng.range(ranges.len() as i32) as usize];
        if mode == 0 {
            let n = 3 + rng.range(8);
            let pts: Vec<Point> = (0..n)
                .map(|_| Point::Int(IntPoint::new(rng.range(range), rng.range(range))))
                .collect();
            let (px, py) = (rng.range(range), rng.range(range));
            let (bx0, by0) = (rng.range(range), rng.range(range));
            let (bw, bh) = (1 + rng.range(range), 1 + rng.range(range));
            let radius = 1 + rng.range(range);
            let _ = write!(sb, "{}|", pts_str(&pts));
            let ps = guard(|| PolygonShape::from_points(&pts));
            match ps {
                Err(_) => sb.push_str("CTOR-EXC:ArrayIndexOutOfBoundsException"),
                Ok(ps) => {
                    let _ = write!(sb, "{}", pts_str(ps.corners()));
                    let _ = write!(sb, "|{}", ps.is_convex());
                    let _ = write!(sb, "|{}", b(ps.area()));
                    let _ = write!(
                        sb,
                        "|{}/{}/{}",
                        ps.dimension(),
                        ps.is_empty(),
                        ps.is_bounded()
                    );
                    let _ = write!(sb, "|{}", bx(&ps.bounding_box()));
                    let _ = write!(sb, "|{}", ps.bounding_octagon());
                    sb.push('|');
                    match guard(|| pts_str(ps.convex_hull().corners())) {
                        Ok(v) => sb.push_str(&v),
                        Err(e) => sb.push_str(&e),
                    }
                    sb.push('|');
                    match guard(|| ps.split_to_convex()) {
                        Ok(None) => sb.push_str("null"),
                        Ok(Some(sp)) => {
                            let _ = write!(sb, "{}{{", sp.len());
                            for t in &sp {
                                let _ = write!(sb, "{};", tile_str(t));
                            }
                            sb.push('}');
                        }
                        Err(e) => sb.push_str(&e),
                    }
                    sb.push('|');
                    {
                        let probe = Point::Int(IntPoint::new(px, py));
                        let parts: Vec<String> = vec![
                            fmt(guard(|| ps.contains(&probe))),
                            fmt(guard(|| ps.is_outside(&probe))),
                            fmt(guard(|| ps.contains_inside(&probe))),
                            fmt(guard(|| ps.contains_float(&FloatPoint::new(
                                px as f64 + 0.5,
                                py as f64 + 0.5
                            )))),
                        ];
                        sb.push_str(&parts.join("/"));
                    }
                    sb.push('|');
                    {
                        let bo = IntBox::from_coords(bx0, by0, bx0 + bw, by0 + bh);
                        let parts: Vec<String> = vec![
                            fmt(guard(|| ps.intersects_box(&bo))),
                            fmt(guard(|| ps.intersects_octagon(&bo.to_int_octagon()))),
                            fmt(guard(|| ps.intersects_simplex(&bo.to_simplex()))),
                            fmt(guard(|| ps.intersects_circle(&Circle::new(
                                IntPoint::new(px, py),
                                radius
                            )))),
                        ];
                        sb.push_str(&parts.join("/"));
                    }
                    sb.push('|');
                    match guard(|| {
                        let bt = ps.bounding_tile();
                        format!("{}/{}", cls(&bt), b(bt.area()))
                    }) {
                        Ok(v) => sb.push_str(&v),
                        Err(e) => sb.push_str(&e),
                    }
                    sb.push('|');
                    match guard(|| ps.nearest_point_approx(&FloatPoint::new(px as f64, py as f64)))
                    {
                        Ok(None) => sb.push_str("null"),
                        Ok(Some(np)) => {
                            let _ = write!(sb, "{}:{}", b(np.x), b(np.y));
                        }
                        Err(e) => sb.push_str(&e),
                    }
                }
            }
        } else if mode == 1 {
            let n = 3 + rng.range(6);
            let pts: Vec<Point> = (0..n)
                .map(|_| Point::Int(IntPoint::new(rng.range(range), rng.range(range))))
                .collect();
            let hn = 3 + rng.range(4);
            let hpts: Vec<Point> = (0..hn)
                .map(|_| Point::Int(IntPoint::new(rng.range(range), rng.range(range))))
                .collect();
            let (px, py) = (rng.range(range), rng.range(range));
            let _ = write!(sb, "{}|{}|", pts_str(&pts), pts_str(&hpts));
            let built = guard(|| {
                let border = PolygonShape::from_points(&pts);
                let hole = PolygonShape::from_points(&hpts);
                PolylineArea::new(border.into(), vec![hole.into()])
            });
            match built {
                Err(_) => sb.push_str("CTOR-EXC:ArrayIndexOutOfBoundsException"),
                Ok(pa) => {
                    let _ = write!(
                        sb,
                        "{}/{}/{}",
                        pa.dimension(),
                        pa.is_bounded(),
                        pa.is_empty()
                    );
                    let _ = write!(sb, "|{}", bx(&pa.bounding_box()));
                    let _ = write!(sb, "|{}", pa.bounding_octagon().unwrap());
                    sb.push('|');
                    match guard(|| pa.split_to_convex(None)) {
                        Ok(None) => sb.push_str("null"),
                        Ok(Some(sp)) => {
                            let _ = write!(sb, "{}{{", sp.len());
                            for t in &sp {
                                let _ = write!(sb, "{};", tile_str(t));
                            }
                            sb.push('}');
                        }
                        Err(e) => sb.push_str(&e),
                    }
                    sb.push('|');
                    {
                        let parts: Vec<String> = vec![
                            fmt(guard(|| pa.contains(&Point::Int(IntPoint::new(px, py))))),
                            fmt(guard(|| pa.contains_float(&FloatPoint::new(
                                px as f64 + 0.5,
                                py as f64 + 0.5
                            )))),
                        ];
                        sb.push_str(&parts.join("/"));
                    }
                    sb.push('|');
                    match guard(|| pa.nearest_point_approx(&FloatPoint::new(px as f64, py as f64)))
                    {
                        Ok(None) => sb.push_str("null"),
                        Ok(Some(np)) => {
                            let _ = write!(sb, "{}:{}", b(np.x), b(np.y));
                        }
                        Err(e) => sb.push_str(&e),
                    }
                    sb.push('|');
                    match guard(|| pa.corner_approx_arr().len()) {
                        Ok(v) => {
                            let _ = write!(sb, "{v}");
                        }
                        Err(e) => sb.push_str(&e),
                    }
                }
            }
        } else {
            let (cx, cy) = (rng.range(range) - range / 2, rng.range(range) - range / 2);
            let r = rng.range(range);
            let (px, py) = (rng.range(range) - range / 2, rng.range(range) - range / 2);
            let (bx0, by0) = (rng.range(range), rng.range(range));
            let (bw, bh) = (1 + rng.range(range), 1 + rng.range(range));
            let ci = Circle::new(IntPoint::new(cx, cy), r);
            let _ = write!(sb, "{cx},{cy},{r}|");
            let _ = write!(
                sb,
                "{}/{}/{}",
                b(ci.area()),
                b(ci.circumference()),
                ci.dimension()
            );
            let _ = write!(sb, "|{}", bx(&ci.bounding_box()));
            let _ = write!(sb, "|{}", ci.bounding_octagon());
            let _ = write!(sb, "|{}", ci.bounding_octagon().is_normalized());
            let probe = FloatPoint::new(px as f64, py as f64);
            let _ = write!(sb, "|{}/{}", b(ci.distance(&probe)), b(ci.border_distance(&probe)));
            let ip = Point::Int(IntPoint::new(px, py));
            let _ = write!(
                sb,
                "|{}/{}/{}/{}",
                ci.contains(&ip),
                ci.is_outside(&ip),
                ci.contains_inside(&ip),
                ci.contains_on_border(&ip)
            );
            let bo = IntBox::from_coords(bx0, by0, bx0 + bw, by0 + bh);
            let radius2 = 1 + rng.range(range);
            let _ = write!(
                sb,
                "|{}/{}/{}/{}",
                ci.intersects_box(&bo),
                ci.intersects_octagon(&bo.to_int_octagon()),
                ci.intersects_simplex(&bo.to_simplex()),
                ci.intersects_circle(&Circle::new(IntPoint::new(px, py), radius2))
            );
            let _ = write!(sb, "|{}", ci.is_contained_in(&bo));
            let _ = write!(
                sb,
                "|{}/{}/{}",
                ci.offset(2.5).radius,
                ci.shrink(2.5).radius,
                ci.enlarge(-1.5).radius
            );
            sb.push('|');
            let seg = 1 + rng.range(range);
            match guard(|| {
                let bt = ci.bounding_tile_max_seg(seg);
                format!("{}/{}/{}", cls(&bt), bt.border_line_count(), b(bt.area()))
            }) {
                Ok(v) => sb.push_str(&v),
                Err(e) => sb.push_str(&e),
            }
            let _ = write!(sb, "|{ci}");
        }
        let _ = writeln!(out, "{sb}");
    }
    let _ = out.flush();
    // Referenced so the `Shape`/`ShapeOps` imports are exercised too.
    let _ = Shape::Circle(Circle::new(IntPoint::new(0, 0), 1)).dimension();
    let _: &dyn PolylineShapeOps = &TileShape::Box(IntBox::from_coords(0, 0, 1, 1));
}
