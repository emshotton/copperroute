use std::panic::{self, AssertUnwindSafe};

use copper_geometry::float_point::FloatPoint;
use copper_geometry::int_box::IntBox;
use copper_geometry::int_octagon::IntOctagon;
use copper_geometry::int_point::IntPoint;
use copper_geometry::int_vector::IntVector;
use copper_geometry::line::Line;
use copper_geometry::line_segment::LineSegment;
use copper_geometry::point::Point;
use copper_geometry::polygon::Polygon;
use copper_geometry::polyline::{Polyline, PolylineError};
use copper_geometry::simplex::Simplex;
use copper_geometry::tile_shape::TileShape;
use copper_geometry::vector::Vector;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn r(&mut self, lo: i32, hi: i32) -> i32 {
        let v = (self.next() >> 1) % ((hi - lo + 1) as u64);
        lo + v as i32
    }
}

fn f(d: f64) -> String { format!("b{}", d.to_bits() as i64) }
fn cpt(p: &Point) -> String {
    match p {
        Point::Int(q) => format!("I{}/{}", q.x, q.y),
        Point::Rational(r) => format!("R{}/{}/{}", r.x, r.y, r.z),
    }
}
fn ocpt(p: &Option<Point>) -> String { match p { None => "null".into(), Some(x) => cpt(x) } }
fn fp(p: &FloatPoint) -> String { format!("({},{})", f(p.x), f(p.y)) }
fn ofp(p: &Option<FloatPoint>) -> String { match p { None => "null".into(), Some(x) => fp(x) } }
fn ln(l: &Line) -> String { format!("[I{}/{};I{}/{}]", l.a.x, l.a.y, l.b.x, l.b.y) }
fn boxs(b: &IntBox) -> String { format!("B({},{},{},{})", b.ll.x, b.ll.y, b.ur.x, b.ur.y) }
fn octs(o: &IntOctagon) -> String {
    format!("O({},{},{},{},{},{},{},{})", o.left_x, o.bottom_y, o.right_x, o.top_y,
        o.upper_left_diagonal_x, o.lower_right_diagonal_x, o.lower_left_diagonal_x, o.upper_right_diagonal_x)
}
fn ts(s: &TileShape) -> String {
    match s {
        TileShape::Box(b) => boxs(b),
        TileShape::Octagon(o) => octs(o),
        TileShape::Simplex(sx) => {
            let mut b = String::from("S{");
            for i in 0..sx.border_line_count() {
                if i > 0 { b.push(' '); }
                b.push_str(&ln(&sx.border_line(i).unwrap()));
            }
            b.push('}');
            b
        }
    }
}
fn ts_arr(a: &[TileShape]) -> String {
    let mut b = format!("[{}:", a.len());
    for s in a { b.push_str(&ts(s)); b.push(';'); }
    b.push(']'); b
}
fn pl(p: &Polyline) -> String {
    let mut b = format!("P[{}:", p.lines().len());
    for l in p.lines() { b.push_str(&ln(l)); b.push(';'); }
    b.push(']'); b
}
fn plr(p: Result<Polyline, PolylineError>) -> String {
    match p { Ok(v) => pl(&v), Err(_) => "EXC".into() }
}
fn pl_arr(a: &[Polyline]) -> String {
    let mut b = format!("[{}:", a.len());
    for p in a { b.push_str(&pl(p)); b.push(';'); }
    b.push(']'); b
}
fn pt_arr(a: &[Point]) -> String {
    let mut b = format!("[{}:", a.len());
    for p in a { b.push_str(&cpt(p)); b.push(';'); }
    b.push(']'); b
}
fn fp_arr(a: &[FloatPoint]) -> String {
    let mut b = format!("[{}:", a.len());
    for p in a { b.push_str(&fp(p)); b.push(';'); }
    b.push(']'); b
}
fn ii_arr(a: &[[usize; 2]]) -> String {
    let mut b = format!("[{}:", a.len());
    for p in a { b.push_str(&format!("{}/{};", p[0], p[1])); }
    b.push(']'); b
}
fn seg(s: &LineSegment) -> String {
    format!("SEG{{{} {} {}}}", ln(&s.get_start_closing_line()), ln(&s.get_line()), ln(&s.get_end_closing_line()))
}
fn oseg(s: &Option<LineSegment>) -> String { match s { None => "null".into(), Some(x) => seg(x) } }

fn emit(out: &mut String, name: &str, t: impl FnOnce() -> String) {
    let v = match panic::catch_unwind(AssertUnwindSafe(t)) {
        Ok(v) => v,
        Err(_) => "EXC".to_string(),
    };
    out.push_str(name); out.push('='); out.push_str(&v); out.push('\n');
}

fn gen_points(rng: &mut Rng, lo: i32, hi: i32, c: i32) -> Vec<Point> {
    let n = rng.r(lo, hi);
    (0..n).map(|_| Point::Int(IntPoint::new(rng.r(-c, c), rng.r(-c, c)))).collect()
}
fn gen_line(rng: &mut Rng, c: i32) -> Line {
    let a = IntPoint::new(rng.r(-c, c), rng.r(-c, c));
    let mut b = IntPoint::new(rng.r(-c, c), rng.r(-c, c));
    if a == b { b = IntPoint::new(b.x + 1, b.y); }
    Line::new(a, b)
}
fn gen_box(rng: &mut Rng) -> IntBox {
    let x = rng.r(-6, 6); let y = rng.r(-6, 6); let w = rng.r(1, 12); let h = rng.r(1, 12);
    IntBox::from_coords(x, y, x + w, y + h)
}

fn main() {
    panic::set_hook(Box::new(|_| {}));
    let args: Vec<String> = std::env::args().collect();
    let iters: i32 = args[1].parse().unwrap();
    let seed: i64 = args[2].parse().unwrap();
    let mode: i32 = args[3].parse().unwrap();
    let mut rng = Rng(seed as u64);
    let mut out = String::new();
    for _ in 0..iters {
        if mode == 3 {
            let pool_size = rng.r(2, 4);
            let mut pool: Vec<Line> = Vec::new();
            for _ in 0..pool_size {
                let l = gen_line(&mut rng, 4);
                pool.push(l);
                pool.push(l.opposite());
            }
            let n = rng.r(3, 10);
            let arr: Vec<Line> = (0..n).map(|_| pool[rng.r(0, pool.len() as i32 - 1) as usize]).collect();
            let a2 = arr.clone();
            emit(&mut out, "in", || {
                let mut b = format!("[{}:", a2.len());
                for l in &a2 { b.push_str(&ln(l)); b.push(';'); }
                b.push(']'); b
            });
            emit(&mut out, "fromLines", || plr(Polyline::from_lines(arr)));
            continue;
        }
        let c = if mode == 1 { 9 } else { 6 };
        let pts: Vec<Point> = if mode == 4 {
            let n = rng.r(3, 10);
            let mut x = rng.r(-8, 8); let mut y = rng.r(-8, 8);
            let dx = [1, 1, 0, -1, -1, -1, 0, 1];
            let dy = [0, 1, 1, 1, 0, -1, -1, -1];
            let mut w = Vec::new();
            for _ in 0..n {
                let d = rng.r(0, 7) as usize; let len = rng.r(1, 8);
                x += dx[d] * len; y += dy[d] * len;
                w.push(Point::Int(IntPoint::new(x, y)));
            }
            w
        } else if mode == 1 { gen_points(&mut rng, 1, 6, c) } else { gen_points(&mut rng, 4, 12, c) };
        let hw = if mode == 1 { rng.r(0, 4) } else { rng.r(1, 10) };
        emit(&mut out, "pts", || pt_arr(&pts));
        let poly = Polygon::new(pts.clone());
        emit(&mut out, "polyCorners", || pt_arr(poly.corner_array()));
        emit(&mut out, "polyRevert", || pt_arr(poly.revert_corners().corner_array()));
        emit(&mut out, "polyWind", || poly.winding_number_after_closing().to_string());
        let p = Polyline::from_points(&pts);
        emit(&mut out, "p", || pl(&p));
        emit(&mut out, "cornerCount", || p.corner_count().to_string());
        emit(&mut out, "corners", || pt_arr(&p.corners()));
        emit(&mut out, "cornerApproxArr", || fp_arr(&p.corner_approx_arr()));
        emit(&mut out, "isEmpty", || p.is_empty().to_string());
        emit(&mut out, "isPoint", || p.is_point().to_string());
        emit(&mut out, "isOrth", || p.is_orthogonal().to_string());
        emit(&mut out, "is45", || p.is_multiple_of_45_degree().to_string());
        emit(&mut out, "first", || ocpt(&p.first_corner()));
        emit(&mut out, "last", || ocpt(&p.last_corner()));
        emit(&mut out, "lenApprox", || f(p.length_approx()));
        let a1 = rng.r(0, 6); let a2v = rng.r(0, 6);
        let (alo, ahi) = (a1.min(a2v) as usize, a1.max(a2v) as usize);
        emit(&mut out, "lenApproxAB", || f(p.length_approx_between(alo, ahi)));
        emit(&mut out, "bbox", || boxs(&p.bounding_box()));
        emit(&mut out, "bboxAB", || boxs(&p.bounding_box_between(alo, ahi)));
        emit(&mut out, "boctAB", || octs(&p.bounding_octagon_between(alo, ahi)));
        emit(&mut out, "reverse", || plr(p.reverse()));
        emit(&mut out, "offsetShapes", || ts_arr(&p.offset_shapes(hw)));
        emit(&mut out, "offsetShapesAB", || ts_arr(&p.offset_shapes_between(hw, alo, ahi)));
        let k = rng.r(0, 5) as usize;
        emit(&mut out, "offsetShape", || match p.offset_shape(hw, k) { None => "null".into(), Some(s) => ts(&s) });
        emit(&mut out, "offsetBox", || match p.offset_box(hw, k) { None => "null".into(), Some(b) => boxs(&b) });
        let tv = IntVector::new(rng.r(-3, 3), rng.r(-3, 3));
        emit(&mut out, "translate", || plr(p.translate_by(&Vector::Int(tv))));
        let fac = rng.r(0, 7);
        let tp = IntPoint::new(rng.r(-3, 3), rng.r(-3, 3));
        emit(&mut out, "turn90", || plr(p.turn_90_degree(fac, &tp)));
        let mv = IntPoint::new(rng.r(-3, 3), rng.r(-3, 3));
        emit(&mut out, "mirrorV", || plr(p.mirror_vertical(&mv)));
        let mh = IntPoint::new(rng.r(-3, 3), rng.r(-3, 3));
        emit(&mut out, "mirrorH", || plr(p.mirror_horizontal(&mh)));
        let ang = rng.r(0, 12) as f64 * 0.5;
        let rp = FloatPoint::new(rng.r(-4, 4) as f64, rng.r(-4, 4) as f64);
        emit(&mut out, "rotateApprox", || pl(&p.rotate_approx(ang, &rp)));
        let q = FloatPoint::new(rng.r(-12, 12) as f64, rng.r(-12, 12) as f64);
        emit(&mut out, "nearest", || ofp(&p.nearest_point_approx(&q)));
        emit(&mut out, "distance", || f(p.distance(&q)));
        let qi = Point::Int(IntPoint::new(rng.r(-12, 12), rng.r(-12, 12)));
        emit(&mut out, "contains", || p.contains(&qi).to_string());
        emit(&mut out, "projection", || oseg(&p.projection_line(&qi)));
        emit(&mut out, "skipLines", || plr(p.skip_lines(alo, ahi)));
        let nlc = rng.r(0, 8) as usize;
        let sl = rng.r(1, 15) as f64;
        emit(&mut out, "shorten", || plr(p.shorten(nlc, sl)));
        let pts2 = if mode == 1 { gen_points(&mut rng, 1, 6, c) } else { gen_points(&mut rng, 4, 12, c) };
        let p2 = Polyline::from_points(&pts2);
        emit(&mut out, "combine", || match p.combine(&p2) { Ok(v) => pl(&v), Err(_) => "EXC".into() });
        let sh1 = IntPoint::new(rng.r(-c, c), rng.r(-c, c));
        let sh2 = IntPoint::new(rng.r(-c, c), rng.r(-c, c));
        emit(&mut out, "combineShared", || {
            let pc = p.corners();
            if pc.is_empty() { return "skip".into(); }
            let end_pt = pc[pc.len() - 1].clone();
            let q2 = vec![end_pt, Point::Int(sh1), Point::Int(sh2)];
            match p.combine(&Polyline::from_points(&q2)) { Ok(v) => pl(&v), Err(_) => "EXC".into() }
        });
        let sh3 = IntPoint::new(rng.r(-c, c), rng.r(-c, c));
        let sh4 = IntPoint::new(rng.r(-c, c), rng.r(-c, c));
        emit(&mut out, "combineSharedStart", || {
            let pc = p.corners();
            if pc.is_empty() { return "skip".into(); }
            let start_pt = pc[0].clone();
            let q2 = vec![start_pt, Point::Int(sh3), Point::Int(sh4)];
            match p.combine(&Polyline::from_points(&q2)) { Ok(v) => pl(&v), Err(_) => "EXC".into() }
        });
        let si = rng.r(0, 6) as usize;
        let el = gen_line(&mut rng, c);
        emit(&mut out, "split", || match p.split(si, &el) { Ok(None) => "null".into(), Ok(Some(a)) => pl_arr(&a), Err(_) => "EXC".into() });
        let lsn = rng.r(0, 6) as usize;
        emit(&mut out, "lineSegment", || oseg(&LineSegment::from_polyline(&p, lsn)));
        emit(&mut out, "toPolyline", || match LineSegment::from_polyline(&p, lsn) { None => "EXC".into(), Some(s) => match s.to_polyline() { Ok(pl_) => pl(&pl_), Err(_) => "EXC".into() } });
        let bx = gen_box(&mut rng);
        let srp = FloatPoint::new(rng.r(-4, 4) as f64, rng.r(-4, 4) as f64);
        emit(&mut out, "shapeRotate", || ts(&TileShape::Box(bx).rotate_approx(ang, &srp)));
        emit(&mut out, "entrance", || ii_arr(&TileShape::Box(bx).entrance_points(&p)));
        emit(&mut out, "cutout", || match TileShape::Box(bx).cutout_polyline(&p) { Ok(v) => pl_arr(&v), Err(_) => "EXC".into() });
        let sx: Simplex = bx.to_simplex();
        emit(&mut out, "simplexEntrance", || ii_arr(&TileShape::Simplex(sx.clone()).entrance_points(&p)));
        emit(&mut out, "simplexCutout", || match TileShape::Simplex(sx.clone()).cutout_polyline(&p) { Ok(v) => pl_arr(&v), Err(_) => "EXC".into() });
    }
    print!("{}", out);
}
