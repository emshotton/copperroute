use fr_geometry::direction::Direction;
use fr_geometry::float_point::FloatPoint;
use fr_geometry::int_box::IntBox;
use fr_geometry::int_direction::IntDirection;
use fr_geometry::int_octagon::IntOctagon;
use fr_geometry::int_point::IntPoint;
use fr_geometry::int_vector::IntVector;
use fr_geometry::line::Line;
use fr_geometry::point::Point;
use fr_geometry::regular_tile_shape::RegularTileShape;
use fr_geometry::side::Side;
use fr_geometry::simplex::Simplex;
use fr_geometry::tile_shape::TileShape;
use fr_geometry::vector::Vector;
use fr_geometry::bounding_directions::{FortyfiveDegreeDirection, ShapeBoundingDirections};

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
fn ocpt(p: &Option<Point>) -> String { match p { None => "null".into(), Some(q) => cpt(q) } }
fn fp(p: &FloatPoint) -> String { format!("({},{})", f(p.x), f(p.y)) }
fn ofp(p: &Option<FloatPoint>) -> String { match p { None => "null".into(), Some(q) => fp(q) } }
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
fn ts_arr(a: &[TileShape]) -> String {
    let mut b = format!("[{}:", a.len());
    for s in a { b.push_str(&ts(s)); b.push(';'); }
    b.push(']'); b
}
fn ots_arr(a: &Option<Vec<TileShape>>) -> String {
    match a { None => "null".into(), Some(v) => ts_arr(v) }
}
fn fp_arr(a: &[FloatPoint]) -> String {
    let mut b = format!("[{}:", a.len());
    for s in a { b.push_str(&fp(s)); b.push(';'); }
    b.push(']'); b
}
fn ip_arr(a: &[IntPoint]) -> String {
    let mut b = format!("[{}:", a.len());
    for s in a { b.push_str(&format!("{}/{}", s.x, s.y)); b.push(';'); }
    b.push(']'); b
}
fn side(s: Side) -> &'static str {
    match s { Side::OnTheLeft => "onTheLeft", Side::OnTheRight => "onTheRight", Side::Collinear => "collinear" }
}
fn rts(r: &RegularTileShape) -> String {
    match r { RegularTileShape::Box(b) => boxs(b), RegularTileShape::Octagon(o) => octs(o) }
}
fn orts(r: &Option<RegularTileShape>) -> String { match r { None => "null".into(), Some(x) => rts(x) } }

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
    let mut rng = Rng(88172645463325252u64);
    let mut out = String::new();
    for it in 0..iters {
        let a = gen(&mut rng);
        let b = gen(&mut rng);
        let p = IntPoint::new(rng.r(-9, 9), rng.r(-9, 9));
        let p2 = IntPoint::new(rng.r(-9, 9), rng.r(-9, 9));
        let pf = FloatPoint::new(rng.r(-9, 9) as f64 + 0.5, rng.r(-9, 9) as f64 + 0.25);
        let l = Line::new(IntPoint::new(rng.r(-8, 8), rng.r(-8, 8)), IntPoint::new(rng.r(-8, 8), rng.r(-8, 8)));
        let _edge = rng.r(0, 3);
        let w = rng.r(1, 12) as f64;
        let pp = Point::Int(p);
        let pp2 = Point::Int(p2);
        out.push_str(&format!("### {}\n", it));
        if it == 0 {
            let eb = TileShape::Box(IntBox::from_coords(5, 5, 0, 0));
            emit!(out, "EB.divide", ts_arr(&eb.divide_into_sections(3.0)));
            emit!(out, "EB.divide0", ts_arr(&eb.divide_into_sections(0.0)));
            let db = TileShape::Box(IntBox::from_coords(0, 0, 10, 0));
            emit!(out, "DB.divide", ts_arr(&db.divide_into_sections(3.0)));
            let nb = TileShape::Box(IntBox::from_coords(0, 0, 10, 10));
            emit!(out, "NB.divide", ts_arr(&nb.divide_into_sections(4.0)));
            emit!(out, "NB.divide0", ts_arr(&nb.divide_into_sections(0.0)));
            emit!(out, "NB.divideNeg", ts_arr(&nb.divide_into_sections(-1.0)));
            let b5 = TileShape::Box(IntBox::from_coords(0, 0, 5, 5));
            emit!(out, "B5.divide15", ts_arr(&b5.divide_into_sections(1.5)));
            let o5 = TileShape::Octagon(IntBox::from_coords(0, 0, 5, 5).to_int_octagon());
            emit!(out, "O5.divide15", ts_arr(&o5.divide_into_sections(1.5)));
            let s5 = TileShape::Simplex(IntBox::from_coords(0, 0, 5, 5).to_simplex());
            emit!(out, "S5.divide15", ts_arr(&s5.divide_into_sections(1.5)));
            let eo = TileShape::Octagon(IntOctagon::new(5, 5, 0, 0, 0, 0, 0, 0));
            emit!(out, "EO.divide", ts_arr(&eo.divide_into_sections(3.0)));
            let es = TileShape::Simplex(Simplex::EMPTY);
            emit!(out, "ES.divide", ts_arr(&es.divide_into_sections(3.0)));
            emit!(out, "ES.circum", f(es.circumference()));
            emit!(out, "ES.area", f(es.area()));
            emit!(out, "ES.length", f(es.length()));
            emit!(out, "ES.centre", fp(&es.centre_of_gravity()));
            emit!(out, "ES.indexOfNearestCorner", format!("{}", es.index_of_nearest_corner(&Point::Int(IntPoint::new(1, 1)))));
            emit!(out, "ES.diagonal", match es.diagonal_corner_segment() { None => "null".to_string(), Some(fl) => format!("{}-{}", fp(&fl.a), fp(&fl.b)) });
            let hp = TileShape::get_instance_from_line(Line::new(IntPoint::new(0, 0), IntPoint::new(0, 1)));
            emit!(out, "HP.contains-1,0", format!("{}", hp.contains(&Point::Int(IntPoint::new(-1, 0)))));
            emit!(out, "HP.contains1,0", format!("{}", hp.contains(&Point::Int(IntPoint::new(1, 0)))));
            emit!(out, "HP.nearestBorderPoint", ocpt(&hp.nearest_border_point(&Point::Int(IntPoint::new(4, 7)))));
            emit!(out, "HP.nearestBorderPoints3", fp_arr(&hp.nearest_border_points_approx(&FloatPoint::new(4.5, 7.5), 3)));
        }
        emit!(out, "A", ts(&a));
        emit!(out, "B", ts(&b));
        emit!(out, "P", cpt(&pp));
        emit!(out, "L", ln(&l));
        emit!(out, "A.blc", format!("{}", a.border_line_count()));
        emit!(out, "A.dim", format!("{}", a.dimension()));
        emit!(out, "A.area", f(a.area()));
        emit!(out, "A.circum", f(a.circumference()));
        emit!(out, "A.len", f(a.length()));
        emit!(out, "A.maxw", f(a.max_width()));
        emit!(out, "A.minw", f(a.min_width()));
        emit!(out, "A.contains", format!("{}", a.contains(&pp)));
        emit!(out, "A.containsInside", format!("{}", a.contains_inside(&pp)));
        emit!(out, "A.isOutside", format!("{}", a.is_outside(&pp)));
        emit!(out, "A.containsOnBorderLineNo", match a.contains_on_border_line_no(&pp) { None => "-1".to_string(), Some(i) => format!("{}", i) });
        emit!(out, "A.containsOnBorder", format!("{}", a.contains_on_border(&pp)));
        emit!(out, "A.containsF", format!("{}", a.contains_float(&pf)));
        emit!(out, "A.containsFTol", format!("{}", a.contains_float_tol(&pf, 0.5)));
        emit!(out, "A.sideOfBorder", side(a.side_of_border(&pf, 0.5)).to_string());
        emit!(out, "A.containsTile", format!("{}", a.contains_tile(&b)));
        emit!(out, "A.containsApprox", format!("{}", a.contains_approx(&b)));
        emit!(out, "A.intersects", format!("{}", a.intersects(&b)));
        emit!(out, "A.intersection", ts(&a.intersection(&b)));
        emit!(out, "B.intersection", ts(&b.intersection(&a)));
        emit!(out, "A.intersectionSimplify", ts(&a.intersection_with_simplify(&b)));
        emit!(out, "A.distance", f(a.distance(&pf)));
        emit!(out, "A.borderDistance", f(a.border_distance(&pf)));
        emit!(out, "A.smallestRadius", f(a.smallest_radius()));
        emit!(out, "A.nearestPoint", ocpt(&a.nearest_point(&pp)));
        emit!(out, "A.nearestPointApprox", ofp(&a.nearest_point_approx(&pf)));
        emit!(out, "A.nearestBorderPoint", ocpt(&a.nearest_border_point(&pp)));
        emit!(out, "A.nearestBorderPointApprox", ofp(&a.nearest_border_point_approx(&pf)));
        emit!(out, "A.nearestBorderPoints1", fp_arr(&a.nearest_border_points_approx(&pf, 1)));
        emit!(out, "A.nearestBorderPoints3", fp_arr(&a.nearest_border_points_approx(&pf, 3)));
        emit!(out, "A.indexOfNearestCorner", format!("{}", a.index_of_nearest_corner(&pp)));
        emit!(out, "A.diagonalCornerSegment", match a.diagonal_corner_segment() { None => "null".to_string(), Some(fl) => format!("{}-{}", fp(&fl.a), fp(&fl.b)) });
        emit!(out, "A.nearestRelOutside3", fp_arr(&a.nearest_relative_outside_locations(&b, 3)));
        emit!(out, "A.touchingSides", match a.touching_sides(&b) { None => "[0:]".to_string(), Some(t) => format!("[2:{};{};]", t[0], t[1]) });
        emit!(out, "B.touchingSides", match b.touching_sides(&a) { None => "[0:]".to_string(), Some(t) => format!("[2:{};{};]", t[0], t[1]) });
        emit!(out, "A.distanceToTheLeft", f(a.distance_to_the_left(&l)));
        emit!(out, "A.sideOfLine", side(a.side_of_line(&l)).to_string());
        emit!(out, "L.isOnTheLeft", format!("{}", l.is_on_the_left(&a)));
        emit!(out, "L.isOnTheRight", format!("{}", l.is_on_the_right(&a)));
        emit!(out, "A.divideIntoSections", if a.is_bounded() { ts_arr(&a.divide_into_sections(w)) } else { "unbounded".to_string() });
        emit!(out, "A.cutout", if a.is_bounded() && b.is_bounded() { ots_arr(&a.cutout(&b)) } else { "unbounded".to_string() });
        emit!(out, "B.cutout", if a.is_bounded() && b.is_bounded() { ots_arr(&b.cutout(&a)) } else { "unbounded".to_string() });
        emit!(out, "A.splitToConvex", ts_arr(&a.split_to_convex()));
        emit!(out, "A.simplify", ts(&a.simplify()));
        emit!(out, "A.boundingBox", boxs(&a.bounding_box()));
        emit!(out, "A.boundingOctagon", match a.bounding_octagon() { None => "null".to_string(), Some(o) => octs(&o) });
        emit!(out, "A.boundingTile", ts(&a.bounding_tile()));
        emit!(out, "A.getId", format!("{}", a.get_id()));
        emit!(out, "A.isIntBox", format!("{}", a.is_int_box()));
        emit!(out, "A.isIntOctagon", format!("{}", a.is_int_octagon()));
        emit!(out, "A.toSimplex", simps(&a.to_simplex()));
        emit!(out, "A.borderLineIndex", match a.border_line_index(&l) { None => "-1".to_string(), Some(i) => format!("{}", i) });
        emit!(out, "A.borderLineIndexOwn", if a.border_line_count() == 0 { "n/a".to_string() } else { match a.border_line_index(&a.border_line(0).unwrap()) { None => "-1".to_string(), Some(i) => format!("{}", i) } });
        emit!(out, "A.turn90", ts(&a.turn_90_degree(1, &p)));
        emit!(out, "A.turn90x3", ts(&a.turn_90_degree(3, &p)));
        emit!(out, "A.mirrorV", ts(&a.mirror_vertical(&p)));
        emit!(out, "A.mirrorH", ts(&a.mirror_horizontal(&p)));
        emit!(out, "A.shrink", ts(&a.shrink(2.0)));
        emit!(out, "A.offset", ts(&a.offset(1.5)));
        emit!(out, "A.enlarge", ts(&a.enlarge(1.5)));
        emit!(out, "A.translate", ts(&a.translate_by(&Vector::Int(IntVector::new(3, -2)))));
        emit!(out, "A.isIntersectedInterior", format!("{}", a.is_intersected_interior_by_points(&pp, &pp2, &Line::new(p, p2))));
        emit!(out, "A.intersectingBorderLineNo", match a.intersecting_border_line_no(&pp, &Direction::Int(IntDirection::RIGHT45)) { None => "-1".to_string(), Some(i) => format!("{}", i) });
        emit!(out, "A.intersectingBorderLineNoUp", match a.intersecting_border_line_no(&pp, &Direction::Int(IntDirection::UP)) { None => "-1".to_string(), Some(i) => format!("{}", i) });
        emit!(out, "A.centreOfGravity", fp(&a.centre_of_gravity()));
        emit!(out, "A.cornerApproxArr", fp_arr(&a.corner_approx_arr()));
        emit!(out, "A.corner0", if a.border_line_count() == 0 { "n/a".to_string() } else { cpt(&a.corner(0)) });
        emit!(out, "A.cornerIsBounded0", format!("{}", a.corner_is_bounded(0)));
        emit!(out, "A.boundsOrth", orts(&ShapeBoundingDirections::Orthogonal.bounds_tile(&a)));
        emit!(out, "A.bounds45", orts(&ShapeBoundingDirections::FortyfiveDegree.bounds_tile(&a)));
        let ra = match &a { TileShape::Box(x) => Some(RegularTileShape::Box(*x)), TileShape::Octagon(x) => Some(RegularTileShape::Octagon(*x)), _ => None };
        let rb = match &b { TileShape::Box(x) => Some(RegularTileShape::Box(*x)), TileShape::Octagon(x) => Some(RegularTileShape::Octagon(*x)), _ => None };
        if let (Some(ra), Some(rb)) = (ra, rb) {
            emit!(out, "R.union", rts(&ra.union(&rb)));
            emit!(out, "R.unionRev", rts(&rb.union(&ra)));
            emit!(out, "R.contains", format!("{}", ra.contains(&rb)));
            emit!(out, "R.containsRev", format!("{}", rb.contains(&ra)));
            let emax = if matches!((&a, &b), (TileShape::Box(_), TileShape::Box(_))) { 4usize } else { 8usize };
            for e in 0..emax {
                emit!(out, &format!("R.compare{}", e), side(ra.compare(&rb, e)).to_string());
                emit!(out, &format!("R.compareRev{}", e), side(rb.compare(&ra, e)).to_string());
            }
            emit!(out, "R.isContainedInBox", format!("{}", ra.is_contained_in_box(&IntBox::from_coords(-5, -5, 12, 12))));
            emit!(out, "R.isContainedInOct", format!("{}", ra.is_contained_in_octagon(&IntBox::from_coords(-5, -5, 12, 12).to_int_octagon())));
        }
        if let TileShape::Octagon(oa) = &a {
            emit!(out, "O.borderPointR", cpt(&Point::Int(oa.border_point(&p, FortyfiveDegreeDirection::Right))));
            emit!(out, "O.borderPointR45", cpt(&Point::Int(oa.border_point(&p, FortyfiveDegreeDirection::Right45))));
            emit!(out, "O.borderPointU45", cpt(&Point::Int(oa.border_point(&p, FortyfiveDegreeDirection::Up45))));
            emit!(out, "O.borderPointL45", cpt(&Point::Int(oa.border_point(&p, FortyfiveDegreeDirection::Left45))));
            emit!(out, "O.borderPointD45", cpt(&Point::Int(oa.border_point(&p, FortyfiveDegreeDirection::Down45))));
            emit!(out, "O.borderPointD", cpt(&Point::Int(oa.border_point(&p, FortyfiveDegreeDirection::Down))));
            emit!(out, "O.nearestBorderProj1", ip_arr(&oa.nearest_border_projections(&p, 1)));
            emit!(out, "O.nearestBorderProj3", ip_arr(&oa.nearest_border_projections(&p, 3)));
            emit!(out, "O.nearestBorderProj8", ip_arr(&oa.nearest_border_projections(&p, 8)));
            let c = IntPoint::new((oa.left_x + oa.right_x) / 2, (oa.bottom_y + oa.top_y) / 2);
            emit!(out, "O.cBorderPointR45", cpt(&Point::Int(oa.border_point(&c, FortyfiveDegreeDirection::Right45))));
            emit!(out, "O.cBorderPointU45", cpt(&Point::Int(oa.border_point(&c, FortyfiveDegreeDirection::Up45))));
            emit!(out, "O.cNearestBorderProj3", ip_arr(&oa.nearest_border_projections(&c, 3)));
            emit!(out, "O.cNearestBorderProj8", ip_arr(&oa.nearest_border_projections(&c, 8)));
            emit!(out, "O.cNearestBorderProj5", ip_arr(&oa.nearest_border_projections(&c, 5)));
        }
    }
    print!("{}", out);
}
