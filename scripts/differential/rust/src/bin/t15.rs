use fr_geometry::int_direction::IntDirection;
use fr_geometry::float_point::FloatPoint;
use fr_geometry::int_box::IntBox;
use fr_geometry::int_octagon::IntOctagon;
use fr_geometry::int_point::IntPoint;
use fr_geometry::line::Line;
use fr_geometry::line_segment::LineSegment;
use fr_geometry::point::Point;
use fr_geometry::simplex::Simplex;
use fr_geometry::tile_shape::TileShape;

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
fn fp(p: &FloatPoint) -> String { format!("({},{})", f(p.x), f(p.y)) }
fn ln(l: &Line) -> String { format!("[{};{}]", cpt(&Point::Int(l.a)), cpt(&Point::Int(l.b))) }
fn oln(l: &Option<Line>) -> String { match l { None => "null".into(), Some(x) => ln(x) } }
fn boxs(b: &IntBox) -> String { format!("B({},{},{},{})", b.ll.x, b.ll.y, b.ur.x, b.ur.y) }
fn octs(o: &IntOctagon) -> String {
    format!("O({},{},{},{},{},{},{},{})", o.left_x, o.bottom_y, o.right_x, o.top_y,
        o.upper_left_diagonal_x, o.lower_right_diagonal_x, o.lower_left_diagonal_x, o.upper_right_diagonal_x)
}
fn simps(s: &Simplex) -> String {
    let mut b = String::from("S{");
    for i in 0..s.border_line_count() {
        if i > 0 { b.push(' '); }
        b.push_str(&oln(&s.border_line(i)));
    }
    b.push('}');
    b
}
fn ts(s: &TileShape) -> String {
    match s {
        TileShape::Box(b) => boxs(b),
        TileShape::Octagon(o) => octs(o),
        TileShape::Simplex(s) => simps(s),
    }
}
fn ln_arr(a: &[Line]) -> String {
    let mut b = format!("[{}:", a.len());
    for s in a { b.push_str(&ln(s)); b.push(';'); }
    b.push(']'); b
}
fn ip_arr(a: &[IntPoint]) -> String {
    let mut b = format!("[{}:", a.len());
    for s in a { b.push_str(&format!("{}/{}", s.x, s.y)); b.push(';'); }
    b.push(']'); b
}
fn i_arr(a: &[usize]) -> String {
    let mut b = format!("[{}:", a.len());
    for s in a { b.push_str(&format!("{}", s)); b.push(';'); }
    b.push(']'); b
}
fn segs(s: &LineSegment) -> String {
    format!("SEG{{{} {} {}}}", ln(&s.get_start_closing_line()), ln(&s.get_line()), ln(&s.get_end_closing_line()))
}
fn osegs(s: &Option<LineSegment>) -> String {
    match s { None => "SEGnull".into(), Some(x) => segs(x) }
}

fn gen_box(rng: &mut Rng) -> IntBox {
    let x = rng.r(-6, 6); let y = rng.r(-6, 6); let w = rng.r(1, 12); let h = rng.r(1, 12);
    IntBox::from_coords(x, y, x + w, y + h)
}
fn gen_oct(rng: &mut Rng) -> IntOctagon {
    let b = gen_box(rng);
    let a1 = rng.r(0, 4); let a2 = rng.r(0, 4); let a3 = rng.r(0, 4); let a4 = rng.r(0, 4);
    IntOctagon::new(b.ll.x, b.ll.y, b.ur.x, b.ur.y,
        b.ll.x - b.ur.y + a1, b.ur.x - b.ll.y - a2, b.ll.x + b.ll.y + a3, b.ur.x + b.ur.y - a4).normalize()
}
fn gen_simplex(rng: &mut Rng) -> Simplex {
    let n = rng.r(2, 5);
    let mut lines = Vec::new();
    for _ in 0..n {
        let x1 = rng.r(-7, 7); let y1 = rng.r(-7, 7);
        let mut dx = rng.r(-6, 6); let dy = rng.r(-6, 6);
        if dx == 0 && dy == 0 { dx = 1; }
        lines.push(Line::new(IntPoint::new(x1, y1), IntPoint::new(x1 + dx, y1 + dy)));
    }
    Simplex::from_lines(lines)
}
fn gen(rng: &mut Rng) -> TileShape {
    let k = rng.r(0, 2);
    if k == 0 { TileShape::Box(gen_box(rng)) }
    else if k == 1 { TileShape::Octagon(gen_oct(rng)) }
    else { TileShape::Simplex(gen_simplex(rng)) }
}
fn mkline(a: IntPoint, d: &IntDirection) -> Line {
    Line::from_direction(a, d)
}
fn gen_seg(rng: &mut Rng, c: i32) -> LineSegment {
    let mode = rng.r(0, 1);
    if mode == 0 {
        let ax = rng.r(-c, c); let ay = rng.r(-c, c); let bx0 = rng.r(-c, c); let by = rng.r(-c, c);
        let bx = if ax == bx0 && ay == by { ax + 1 } else { bx0 };
        let a = IntPoint::new(ax, ay);
        let b = IntPoint::new(bx, by);
        let middle = Line::new(a, b);
        let perp = middle.direction().turn_45_degree(2);
        return LineSegment::new(mkline(a, &perp), middle, mkline(b, &perp));
    }
    let mut l: Vec<Line> = Vec::new();
    for _ in 0..3 {
        let x1 = rng.r(-c, c); let y1 = rng.r(-c, c);
        let mut dx = rng.r(-c, c); let dy = rng.r(-c, c);
        if dx == 0 && dy == 0 { dx = 1; }
        l.push(Line::new(IntPoint::new(x1, y1), IntPoint::new(x1 + dx, y1 + dy)));
    }
    let perp = l[1].direction().turn_45_degree(2);
    if l[0].is_parallel(&l[1]) { l[0] = mkline(l[0].a, &perp); }
    if l[2].is_parallel(&l[1]) { l[2] = mkline(l[2].a, &perp); }
    LineSegment::new(l[0], l[1], l[2])
}
fn small(s: &LineSegment) -> bool {
    let a = s.start_point().to_float().round();
    let b = s.end_point().to_float().round();
    a.x.abs() < 5000 && a.y.abs() < 5000 && b.x.abs() < 5000 && b.y.abs() < 5000
}

macro_rules! emit {
    ($out:expr, $name:expr, $val:expr) => {
        {
            let v = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $val));
            let s = match v { Ok(x) => x, Err(_) => "EXC:panic".to_string() };
            $out.push_str(&format!("{}={}\n", $name, s));
        }
    };
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let iters: usize = std::env::args().nth(1).unwrap().parse().unwrap();
    let seed: i64 = std::env::args().nth(2).unwrap().parse().unwrap();
    let c: i32 = std::env::args().nth(3).map(|x| x.parse().unwrap()).unwrap_or(9);
    let mut rng = Rng(seed as u64);
    let mut out = String::new();
    for it in 0..iters {
        let shp = gen(&mut rng);
        let s1 = gen_seg(&mut rng, c);
        let s2 = gen_seg(&mut rng, c);
        let p = IntPoint::new(rng.r(-c, c), rng.r(-c, c));
        let w = rng.r(1, 40) as f64 / 10.0;
        let w2 = rng.r(1, 200) as f64 / 10.0;
        out.push_str(&format!("### {}\n", it));
        emit!(out, "SHP", ts(&shp));
        emit!(out, "S1", segs(&s1));
        emit!(out, "S2", segs(&s2));
        emit!(out, "P", cpt(&Point::Int(p)));
        emit!(out, "W", f(w));
        emit!(out, "S1.startPoint", cpt(&s1.start_point()));
        emit!(out, "S1.endPoint", cpt(&s1.end_point()));
        emit!(out, "S1.startPointApprox", fp(&s1.start_point_approx()));
        emit!(out, "S1.endPointApprox", fp(&s1.end_point_approx()));
        emit!(out, "S1.getLine", ln(&s1.get_line()));
        emit!(out, "S1.startClosing", ln(&s1.get_start_closing_line()));
        emit!(out, "S1.endClosing", ln(&s1.get_end_closing_line()));
        emit!(out, "S1.opposite", segs(&s1.opposite()));
        emit!(out, "S1.toSimplex", simps(&s1.to_simplex()));
        emit!(out, "S1.contains", format!("{}", s1.contains(&Point::Int(p))));
        emit!(out, "S1.containsStart", format!("{}", s1.contains(&s1.start_point())));
        emit!(out, "S1.boundingBox", boxs(&s1.bounding_box()));
        emit!(out, "S1.boundingOctagon", octs(&s1.bounding_octagon()));
        emit!(out, "S1.changeLen", segs(&s1.change_length_approx(w2)));
        emit!(out, "S1.changeLen0", segs(&s1.change_length_approx(0.0)));
        emit!(out, "S1.sortXY", segs(&s1.sort_endpoints_in_xy()));
        emit!(out, "S2.sortXY", segs(&s2.sort_endpoints_in_xy()));
        emit!(out, "S1.intersection", ln_arr(&s1.intersection(&s2)));
        emit!(out, "S2.intersection", ln_arr(&s2.intersection(&s1)));
        emit!(out, "S1.intersects", format!("{}", s1.intersects(&s2)));
        emit!(out, "S1.overlaps", format!("{}", s1.overlaps(&s2)));
        emit!(out, "S1.selfIntersection", ln_arr(&s1.intersection(&s1)));
        if small(&s1) {
            emit!(out, "S1.stairT", ip_arr(&s1.stair_approximation(w, true)));
            emit!(out, "S1.stairF", ip_arr(&s1.stair_approximation(w, false)));
            emit!(out, "S1.stair45T", ip_arr(&s1.stair_approximation_45(w, true)));
            emit!(out, "S1.stair45F", ip_arr(&s1.stair_approximation_45(w, false)));
            emit!(out, "S1.stairBigT", ip_arr(&s1.stair_approximation(w2, true)));
            emit!(out, "S1.stair45BigF", ip_arr(&s1.stair_approximation_45(w2, false)));
        }
        // collinear partner of s1, to stress the overlap branch of intersection()
        let cperp = s1.get_line().direction().turn_45_degree(2);
        let q1mode = rng.r(0, 2);
        let q1 = if q1mode == 0 { s1.end_point().to_float().round() } else { IntPoint::new(rng.r(-c, c), rng.r(-c, c)) };
        let q2mode = rng.r(0, 2);
        let q2 = if q2mode == 0 { s1.start_point().to_float().round() } else { IntPoint::new(rng.r(-c, c), rng.r(-c, c)) };
        let flip = rng.r(0, 1) == 0;
        let cmid = if flip { s1.get_line().opposite() } else { s1.get_line() };
        let s3 = LineSegment::new(mkline(q1, &cperp), cmid, mkline(q2, &cperp));
        emit!(out, "S3", segs(&s3));
        emit!(out, "S3.startPoint", cpt(&s3.start_point()));
        emit!(out, "S3.endPoint", cpt(&s3.end_point()));
        emit!(out, "S1.interS3", ln_arr(&s1.intersection(&s3)));
        emit!(out, "S3.interS1", ln_arr(&s3.intersection(&s1)));
        emit!(out, "S1.intersectsS3", format!("{}", s1.intersects(&s3)));
        emit!(out, "S1.overlapsS3", format!("{}", s1.overlaps(&s3)));
        emit!(out, "S3.overlapsS1", format!("{}", s3.overlaps(&s1)));
        emit!(out, "S3.sortXY", segs(&s3.sort_endpoints_in_xy()));
        emit!(out, "S3.toSimplex", simps(&s3.to_simplex()));
        emit!(out, "S1.containsQ1", format!("{}", s1.contains(&Point::Int(q1))));
        emit!(out, "S3.borderIntersections", i_arr(&s3.border_intersections(&shp)));
        emit!(out, "SHP.intersectedInteriorS3", format!("{}", shp.is_intersected_interior_by(&s3)));
        emit!(out, "S1.borderIntersections", i_arr(&s1.border_intersections(&shp)));
        emit!(out, "S2.borderIntersections", i_arr(&s2.border_intersections(&shp)));
        emit!(out, "SHP.intersectedInteriorS1", format!("{}", shp.is_intersected_interior_by(&s1)));
        emit!(out, "SHP.intersectedInteriorS2", format!("{}", shp.is_intersected_interior_by(&s2)));
        emit!(out, "S1.startPointApproxCached", fp(&s1.start_point_approx()));
        emit!(out, "S1.endPointApproxCached", fp(&s1.end_point_approx()));
        let blc = shp.border_line_count();
        for k in 0..=blc {
            emit!(out, format!("SHP.seg{}", k), osegs(&LineSegment::from_tile_shape(&shp, k)));
        }
    }
    print!("{}", out);
}
