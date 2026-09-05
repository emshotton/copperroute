use fr_geometry::int_box::IntBox;
use fr_geometry::int_point::IntPoint;
use fr_geometry::line::Line;
use fr_geometry::line_segment::LineSegment;
use fr_geometry::point::Point;
use fr_geometry::simplex::Simplex;
use fr_geometry::tile_shape::TileShape;

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
macro_rules! e {
    ($n:expr, $v:expr) => {{
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $v));
        println!("{}={}", $n, match r { Ok(x) => x, Err(_) => "EXC".to_string() });
    }};
}
fn seg(ax: i32, ay: i32, bx: i32, by: i32) -> LineSegment {
    let a = IntPoint::new(ax, ay); let b = IntPoint::new(bx, by);
    let m = Line::new(a, b);
    let p = m.direction().turn_45_degree(2);
    LineSegment::new(Line::from_direction(a, &p), m, Line::from_direction(b, &p))
}
fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    let s = seg(0, 0, 20, 7);
    e!("negW.stair", ip_arr(&s.stair_approximation(-2.0, true)));
    e!("negW.stair45", ip_arr(&s.stair_approximation_45(-2.0, true)));
    e!("negW.stairSmall", ip_arr(&s.stair_approximation(-0.1, true)));
    e!("zeroW.stair", ip_arr(&s.stair_approximation(0.0, true)));
    e!("hugeW.stair", ip_arr(&s.stair_approximation(1e18, true)));
    e!("nanW.stair", ip_arr(&s.stair_approximation(f64::NAN, true)));
    e!("infW.stair", ip_arr(&s.stair_approximation(f64::INFINITY, true)));
    let es = TileShape::Simplex(Simplex::EMPTY);
    e!("emptySimplex.border", i_arr(&s.border_intersections(&es)));
    e!("emptySimplex.interior", format!("{}", es.is_intersected_interior_by(&s)));
    let eb = TileShape::Box(IntBox::from_coords(5, 5, 0, 0));
    e!("emptyBox.border", i_arr(&s.border_intersections(&eb)));
    e!("emptyBox.interior", format!("{}", eb.is_intersected_interior_by(&s)));
    let hp = TileShape::get_instance_from_line(Line::from_coords(0, 0, 0, 1));
    e!("halfplane.border", i_arr(&s.border_intersections(&hp)));
    e!("halfplane.interior", format!("{}", hp.is_intersected_interior_by(&s)));
    e!("negLen.changeLen", { let t = s.change_length_approx(-5.0); let l = t.get_end_closing_line(); format!("{}/{};{}/{}", l.a.x, l.a.y, l.b.x, l.b.y) });
    e!("negLen.endPoint", { let t = s.change_length_approx(-5.0); match t.end_point() { Point::Int(ip) => format!("I{}/{}", ip.x, ip.y), Point::Rational(r) => format!("R{}/{}/{}", r.x, r.y, r.z) } });
    e!("empty.fromShape0", match LineSegment::from_tile_shape(&es, 0) { None => "SEGnull".to_string(), Some(_) => "SEG".to_string() });
}
