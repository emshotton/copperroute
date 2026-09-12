use super::PcbError;
use crate::kicad::sexpr::{Node, Value};

pub const OUTLINE_TOLERANCE: f64 = 0.005;

/// Matches KiCad's own `DEFAULT_CHAINING_EPSILON_MM` (board.h): the max distance between two
/// endpoints for KiCad to treat them as connected when chaining a board outline, expressed
/// in the integer nanometres KiCad stores coordinates in.
const CHAINING_EPSILON_NM: f64 = 10_000.0;

fn num(text: &str) -> Result<f64, PcbError> {
    let value: f64 = text.parse().unwrap_or(f64::NAN);
    if !value.is_finite() || value.abs() > 100_000.0 {
        return Err(PcbError::new("outline", "Invalid outline coordinate."));
    }
    Ok(value)
}

fn xy(node: Option<&Node>) -> Result<(f64, f64), PcbError> {
    let node = node.ok_or_else(|| PcbError::new("outline", "Missing outline coordinate."))?;
    let x = num(node.atom(1).unwrap_or(""))?;
    let y = num(node.atom(2).unwrap_or(""))?;
    Ok((x, y))
}

fn point(node: &Node, key: &str) -> Result<(f64, f64), PcbError> {
    xy(node.child(key))
}

fn child_nodes(node: &Node) -> impl Iterator<Item = &Node> {
    node.values.iter().filter_map(|value| match value {
        Value::Node(child) => Some(child),
        Value::Atom(_) => None,
    })
}

pub fn footprint_point(fp: &Node, x: f64, y: f64) -> Result<(f64, f64), PcbError> {
    let anchor = point(fp, "at")?;
    let angle = match fp.child("at").and_then(|at| at.atom(3)) {
        Some(text) => num(text)?,
        None => 0.0,
    };
    let angle = angle * std::f64::consts::PI / 180.0;
    Ok((
        anchor.0 + x * angle.cos() + y * angle.sin(),
        anchor.1 - x * angle.sin() + y * angle.cos(),
    ))
}

fn sample(
    center: (f64, f64),
    radius: f64,
    start: f64,
    sweep: f64,
) -> Result<Vec<(f64, f64)>, PcbError> {
    if radius.is_nan() || radius <= 0.0 {
        return Err(PcbError::new("outline", "Degenerate outline arc."));
    }
    let step = (std::f64::consts::PI / 12.0)
        .min(2.0 * (1.0 - OUTLINE_TOLERANCE / radius).max(-1.0).acos());
    let count = (sweep.abs() / step).ceil();
    if !count.is_finite() || count > 4096.0 {
        return Err(PcbError::new(
            "outline",
            "Outline curve is too large to import.",
        ));
    }
    let count = count as usize;
    Ok((0..=count)
        .map(|i| {
            let t = start + sweep * (i as f64) / (count as f64);
            (center.0 + radius * t.cos(), center.1 + radius * t.sin())
        })
        .collect())
}

pub fn arc_points(
    a: (f64, f64),
    m: (f64, f64),
    b: (f64, f64),
) -> Result<Vec<(f64, f64)>, PcbError> {
    let d = 2.0 * (a.0 * (m.1 - b.1) + m.0 * (b.1 - a.1) + b.0 * (a.1 - m.1));
    if d.abs() < 1e-10 {
        return Err(PcbError::new("outline", "Degenerate outline arc."));
    }
    let aa = a.0 * a.0 + a.1 * a.1;
    let mm = m.0 * m.0 + m.1 * m.1;
    let bb = b.0 * b.0 + b.1 * b.1;
    let center = (
        (aa * (m.1 - b.1) + mm * (b.1 - a.1) + bb * (a.1 - m.1)) / d,
        (aa * (b.0 - m.0) + mm * (a.0 - b.0) + bb * (m.0 - a.0)) / d,
    );
    let angle_of = |p: (f64, f64)| (p.1 - center.1).atan2(p.0 - center.0);
    let norm = |v: f64| (v + std::f64::consts::PI * 4.0) % (std::f64::consts::PI * 2.0);
    let start = angle_of(a);
    let span = norm(angle_of(b) - start);
    let sweep = if norm(angle_of(m) - start) < span {
        span
    } else {
        span - std::f64::consts::PI * 2.0
    };
    let radius = (a.0 - center.0).hypot(a.1 - center.1);
    let mut points = sample(center, radius, start, sweep)?;
    let last = points.len() - 1;
    points[0] = a;
    points[last] = b;
    Ok(points)
}

pub(crate) fn native_arc_points(node: &Node) -> Result<Vec<(f64, f64)>, PcbError> {
    if node.child("mid").is_some() {
        return arc_points(
            point(node, "start")?,
            point(node, "mid")?,
            point(node, "end")?,
        );
    }
    let center = point(node, "start")?;
    let start_point = point(node, "end")?;
    let angle = num(node.value("angle").unwrap_or(""))?;
    let start_angle = (start_point.1 - center.1).atan2(start_point.0 - center.0);
    let radius = (start_point.0 - center.0).hypot(start_point.1 - center.1);
    let mut points = sample(
        center,
        radius,
        start_angle,
        angle * std::f64::consts::PI / 180.0,
    )?;
    points[0] = start_point;
    Ok(points)
}

pub(crate) fn bounding_box(points: &[(f64, f64)], margin: f64) -> [(f64, f64); 4] {
    let mut x0 = f64::INFINITY;
    let mut x1 = f64::NEG_INFINITY;
    let mut y0 = f64::INFINITY;
    let mut y1 = f64::NEG_INFINITY;
    for &(x, y) in points {
        x0 = x0.min(x);
        x1 = x1.max(x);
        y0 = y0.min(y);
        y1 = y1.max(y);
    }
    [
        (x0 - margin, y0 - margin),
        (x1 + margin, y0 - margin),
        (x1 + margin, y1 + margin),
        (x0 - margin, y1 + margin),
    ]
}

#[derive(Debug, Clone, PartialEq)]
pub struct OutlinePaths {
    pub paths: Vec<Vec<(f64, f64)>>,
    pub curved: bool,
}

pub fn outline_paths(root: &Node) -> Result<OutlinePaths, PcbError> {
    let mut paths = Vec::new();
    let mut curved = false;
    for node in child_nodes(root) {
        collect(node, None, &mut paths, &mut curved)?;
    }
    for fp in root.children("footprint").chain(root.children("module")) {
        for node in child_nodes(fp) {
            collect(node, Some(fp), &mut paths, &mut curved)?;
        }
    }
    Ok(OutlinePaths { paths, curved })
}

fn collect(
    node: &Node,
    fp: Option<&Node>,
    paths: &mut Vec<Vec<(f64, f64)>>,
    curved: &mut bool,
) -> Result<(), PcbError> {
    if node.value("layer").unwrap_or("") != "Edge.Cuts" {
        return Ok(());
    }
    let name = node.name();
    let kind = match name.strip_prefix("fp_") {
        Some(rest) => format!("gr_{rest}"),
        None => name.to_string(),
    };
    if kind == "target" || kind == "gr_target" {
        return Ok(());
    }
    let mut points: Vec<(f64, f64)> = match kind.as_str() {
        "gr_line" => vec![point(node, "start")?, point(node, "end")?],
        "gr_rect" => {
            let a = point(node, "start")?;
            let b = point(node, "end")?;
            vec![a, (b.0, a.1), b, (a.0, b.1), a]
        }
        "gr_poly" => {
            let mut points = match node.child("pts") {
                Some(pts) => pts
                    .children("xy")
                    .map(|xy_node| xy(Some(xy_node)))
                    .collect::<Result<Vec<_>, _>>()?,
                None => Vec::new(),
            };
            if !points.is_empty() {
                points.push(points[0]);
            }
            points
        }
        "gr_arc" => {
            *curved = true;
            native_arc_points(node)?
        }
        "gr_circle" => {
            *curved = true;
            let center = point(node, "center")?;
            let end = point(node, "end")?;
            let radius = (end.0 - center.0).hypot(end.1 - center.1);
            let mut points = sample(center, radius, 0.0, std::f64::consts::PI * 2.0)?;
            let last = points.len() - 1;
            points[last] = points[0];
            points
        }
        other => {
            return Err(PcbError::new(
                "outline",
                &format!("Unsupported board outline object: {other}"),
            ));
        }
    };
    if points.len() < 2 {
        return Err(PcbError::new("outline", "Empty outline path."));
    }
    if let Some(fp) = fp {
        points = points
            .into_iter()
            .map(|p| footprint_point(fp, p.0, p.1))
            .collect::<Result<Vec<_>, _>>()?;
    }
    paths.push(points);
    Ok(())
}

/// KiCad rounds every coordinate to a nanometre and then chains with
/// `SquaredEuclideanNorm() <= Square(epsilon)`, so a gap of exactly the epsilon closes.
/// Subtracting millimetre floats instead leaves such a gap outside the limit, because
/// `92.79 - 92.78` is `0.010000000000005116` in binary floating point.
fn equal(a: (f64, f64), b: (f64, f64)) -> bool {
    let nanometres = |value: f64| (value * 1e6).round();
    let dx = nanometres(a.0) - nanometres(b.0);
    let dy = nanometres(a.1) - nanometres(b.1);
    dx * dx + dy * dy <= CHAINING_EPSILON_NM * CHAINING_EPSILON_NM
}

fn area(points: &[(f64, f64)]) -> f64 {
    points
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let q = points[(i + 1) % points.len()];
            p.0 * q.1 - p.1 * q.0
        })
        .sum::<f64>()
        .abs()
}

fn centroid(points: &[(f64, f64)]) -> (f64, f64) {
    let n = points.len() as f64;
    let (sx, sy) = points
        .iter()
        .fold((0.0, 0.0), |(sx, sy), p| (sx + p.0, sy + p.1));
    (sx / n, sy / n)
}

/// Orders loops by descending area, largest first (so the board boundary sorts to the front),
/// with ties (including near-ties from `area`'s summation order and hole sampling, which are not
/// bit-identical across targets) broken by centroid and then by first point. Without this, two
/// loops whose true areas are equal or coincidentally close would order by whichever few-ULP
/// noise the local `f64::sin`/`cos` happened to produce, which is not guaranteed stable across
/// architectures or standard library versions — so the same board could import to different item
/// ids on different machines.
fn loop_order(a: &[(f64, f64)], b: &[(f64, f64)]) -> std::cmp::Ordering {
    const AREA_GRANULARITY: f64 = 1e6;
    let rounded_area = |points: &[(f64, f64)]| (area(points) * AREA_GRANULARITY).round() as i64;
    rounded_area(b)
        .cmp(&rounded_area(a))
        .then_with(|| {
            let (ax, ay) = centroid(a);
            let (bx, by) = centroid(b);
            ax.total_cmp(&bx).then_with(|| ay.total_cmp(&by))
        })
        .then_with(|| {
            a[0].0
                .total_cmp(&b[0].0)
                .then_with(|| a[0].1.total_cmp(&b[0].1))
        })
}

fn inside(p: (f64, f64), poly: &[(f64, f64)]) -> bool {
    let mut result = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[j];
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            result = !result;
        }
        j = i;
    }
    result
}

#[derive(Debug, Clone, PartialEq)]
pub struct Outline {
    pub boundary: Vec<(f64, f64)>,
    pub cutouts: Vec<Vec<(f64, f64)>>,
}

pub fn assemble_outline(
    paths: &OutlinePaths,
    warnings: &mut Vec<String>,
) -> Result<Outline, PcbError> {
    let mut edges: Vec<((f64, f64), (f64, f64))> = paths
        .paths
        .iter()
        .flat_map(|path| path.windows(2).map(|w| (w[0], w[1])))
        .collect();

    if edges.is_empty() {
        return Err(PcbError::new(
            "outline",
            "A closed Edge.Cuts outline is required.",
        ));
    }

    let mut loops: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut stray_edges = 0usize;
    while !edges.is_empty() {
        let (first, last) = edges.remove(0);
        let mut loop_points = vec![first];
        let mut consumed = 0usize;
        let mut end = last;
        let mut closed = true;
        while !equal(end, first) {
            loop_points.push(end);
            let Some(index) = edges
                .iter()
                .position(|(a, b)| equal(*a, end) || equal(*b, end))
            else {
                closed = false;
                break;
            };
            let (a, b) = edges.remove(index);
            consumed += 1;
            end = if equal(a, end) { b } else { a };
        }
        if !closed {
            stray_edges += 1 + consumed;
            continue;
        }
        if loop_points.len() < 3 {
            return Err(PcbError::new("outline", "Degenerate Edge.Cuts outline."));
        }
        loops.push(loop_points);
    }

    if loops.is_empty() {
        return Err(PcbError::new(
            "outline",
            "A closed Edge.Cuts outline is required.",
        ));
    }

    if stray_edges > 0 {
        warnings.push(format!(
            "{stray_edges} Edge.Cuts edges could not be closed into a loop and were ignored."
        ));
    }

    loops.sort_by(|a, b| loop_order(a, b));
    let boundary = loops.remove(0);

    if loops
        .iter()
        .any(|loop_points| loop_points.iter().any(|p| !inside(*p, &boundary)))
    {
        return Err(PcbError::new(
            "outline",
            "Separate board outlines are not supported yet.",
        ));
    }

    Ok(Outline {
        boundary,
        cutouts: loops,
    })
}
