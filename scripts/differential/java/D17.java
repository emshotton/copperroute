package app.freerouting.geometry.planar;

import java.io.PrintStream;

/** Differential harness for Task 17: PolygonShape / PolylineArea / Circle. */
public class D17 {
  static long state;
  static long next() {
    state ^= state << 13;
    state ^= state >>> 7;
    state ^= state << 17;
    return state;
  }
  static int nextRange(int bound) {
    long v = next() >>> 1;
    return (int) (v % bound);
  }
  interface BoolSupplier { boolean get(); }
  static String g(BoolSupplier f) {
    try { return String.valueOf(f.get()); }
    catch (Throwable t) { return "EXC:" + t.getClass().getSimpleName(); }
  }
  static String b(double v) { return Long.toHexString(Double.doubleToRawLongBits(v)); }
  static String bx(IntBox x) { return "B(" + x.ll.x + "," + x.ll.y + "," + x.ur.x + "," + x.ur.y + ")"; }
  static String cls(TileShape t) {
    if (t instanceof IntBox) return "Box";
    if (t instanceof IntOctagon) return "Octagon";
    return "Simplex";
  }
  static void appendPts(StringBuilder sb, Point[] pts) {
    sb.append('[');
    for (Point p : pts) { sb.append(p).append(','); }
    sb.append(']');
  }
  static void appendTile(StringBuilder sb, TileShape t) {
    sb.append(cls(t)).append('/').append(t.borderLineCount()).append('/').append(b(t.area())).append('/');
    FloatPoint[] ca = t.cornerApproxArr();
    for (FloatPoint f : ca) sb.append(b(f.x)).append(':').append(b(f.y)).append(',');
  }

  public static void main(String[] args) throws Exception {
    int cases = Integer.parseInt(args[0]);
    int mode = Integer.parseInt(args[1]); // 0 = polygon, 1 = polyline area, 2 = circle
    state = 0x2545F4914F6CDD1DL;
    PrintStream out = new PrintStream(new java.io.BufferedOutputStream(System.out, 1 << 20));
    int[] ranges = {6, 12, 40, 400};
    for (int c = 0; c < cases; c++) {
      StringBuilder sb = new StringBuilder();
      sb.append(c).append('|');
      int range = ranges[nextRange(ranges.length)];
      if (mode == 0) {
        int n = 3 + nextRange(8);
        Point[] pts = new Point[n];
        for (int i = 0; i < n; i++) pts[i] = new IntPoint(nextRange(range), nextRange(range));
        int px = nextRange(range), py = nextRange(range);
        int bx = nextRange(range), by = nextRange(range), bw = 1 + nextRange(range), bh = 1 + nextRange(range);
        int probeRadius = 1 + nextRange(range);
        appendPts(sb, pts);
        sb.append('|');
        try {
          final PolygonShape ps = new PolygonShape(pts);
          appendPts(sb, ps.corners);
          sb.append('|').append(ps.isConvex());
          sb.append('|').append(b(ps.area()));
          sb.append('|').append(ps.dimension()).append('/').append(ps.isEmpty()).append('/').append(ps.isBounded());
          sb.append('|').append(bx(ps.boundingBox()));
          sb.append('|').append(ps.boundingOctagon());
          sb.append('|');
          try { appendPts(sb, ps.convexHull().corners); } catch (Throwable t) { sb.append("EXC:").append(t.getClass().getSimpleName()); }
          sb.append('|');
          try {
            TileShape[] sp = ps.splitToConvex();
            if (sp == null) sb.append("null");
            else { sb.append(sp.length).append('{'); for (TileShape t : sp) { appendTile(sb, t); sb.append(';'); } sb.append('}'); }
          } catch (Throwable t) { sb.append("EXC:").append(t.getClass().getSimpleName()); }
          sb.append('|');
          {
            final IntPoint probe = new IntPoint(px, py);
            sb.append(g(() -> ps.contains((Point) probe))).append('/').append(g(() -> ps.isOutside(probe)))
              .append('/').append(g(() -> ps.containsInside(probe)))
              .append('/').append(g(() -> ps.contains(new FloatPoint(px + 0.5, py + 0.5))));
          }
          sb.append('|');
          {
            final IntBox box = new IntBox(bx, by, bx + bw, by + bh);
            sb.append(g(() -> ps.intersects(box))).append('/').append(g(() -> ps.intersects(box.toIntOctagon())))
              .append('/').append(g(() -> ps.intersects(box.toSimplex())))
              .append('/').append(g(() -> ps.intersects(new Circle(new IntPoint(px, py), probeRadius))));
          }
          sb.append('|');
          try { sb.append(cls(ps.boundingTile())).append('/').append(b(ps.boundingTile().area())); }
          catch (Throwable t) { sb.append("EXC:").append(t.getClass().getSimpleName()); }
          sb.append('|');
          try { FloatPoint np = ps.nearestPointApprox(new FloatPoint(px, py));
                sb.append(np == null ? "null" : b(np.x) + ":" + b(np.y)); }
          catch (Throwable t) { sb.append("EXC:").append(t.getClass().getSimpleName()); }
        } catch (Throwable t) {
          sb.append("CTOR-EXC:").append(t.getClass().getSimpleName());
        }
      } else if (mode == 1) {
        int n = 3 + nextRange(6);
        Point[] pts = new Point[n];
        for (int i = 0; i < n; i++) pts[i] = new IntPoint(nextRange(range), nextRange(range));
        int hn = 3 + nextRange(4);
        Point[] hpts = new Point[hn];
        for (int i = 0; i < hn; i++) hpts[i] = new IntPoint(nextRange(range), nextRange(range));
        int px = nextRange(range), py = nextRange(range);
        appendPts(sb, pts); sb.append('|'); appendPts(sb, hpts); sb.append('|');
        try {
          PolygonShape border = new PolygonShape(pts);
          PolygonShape hole = new PolygonShape(hpts);
          final PolylineArea pa = new PolylineArea(border, new PolylineShape[]{hole});
          sb.append(pa.dimension()).append('/').append(pa.isBounded()).append('/').append(pa.isEmpty());
          sb.append('|').append(bx(pa.boundingBox())).append('|').append(pa.boundingOctagon());
          sb.append('|');
          try {
            TileShape[] sp = pa.splitToConvex();
            if (sp == null) sb.append("null");
            else { sb.append(sp.length).append('{'); for (TileShape t : sp) { appendTile(sb, t); sb.append(';'); } sb.append('}'); }
          } catch (Throwable t) { sb.append("EXC:").append(t.getClass().getSimpleName()); }
          sb.append('|');
          sb.append(g(() -> pa.contains((Point) new IntPoint(px, py))))
            .append('/').append(g(() -> pa.contains(new FloatPoint(px + 0.5, py + 0.5))));
          sb.append('|');
          try { FloatPoint np = pa.nearestPointApprox(new FloatPoint(px, py));
                sb.append(np == null ? "null" : b(np.x) + ":" + b(np.y)); }
          catch (Throwable t) { sb.append("EXC:").append(t.getClass().getSimpleName()); }
          sb.append('|');
          try { FloatPoint[] ca = pa.cornerApproxArr(); sb.append(ca.length); }
          catch (Throwable t) { sb.append("EXC:").append(t.getClass().getSimpleName()); }
        } catch (Throwable t) {
          sb.append("CTOR-EXC:").append(t.getClass().getSimpleName());
        }
      } else if (mode == 3) {
        // NOTE(differential harness, Task 18): the original driver called a
        // `PolygonShape.splitPiecesForDiff()` that never existed on the real,
        // unmodified `app.freerouting.geometry.planar.PolygonShape` — this
        // mode does not compile against the pristine Java source and has no
        // Rust twin in d17.rs (which only implements mode 0/1). Dropped
        // rather than invent a method on the Java side; see
        // scripts/differential/README.md.
        sb.append("unsupported: mode 3 was never a real PolygonShape method");
      } else {
        int cx = nextRange(range) - range / 2, cy = nextRange(range) - range / 2;
        int r = nextRange(range);
        int px = nextRange(range) - range / 2, py = nextRange(range) - range / 2;
        int bx = nextRange(range), by = nextRange(range), bw = 1 + nextRange(range), bh = 1 + nextRange(range);
        Circle ci = new Circle(new IntPoint(cx, cy), r);
        sb.append(cx).append(',').append(cy).append(',').append(r).append('|');
        sb.append(b(ci.area())).append('/').append(b(ci.circumference())).append('/').append(ci.dimension());
        sb.append('|').append(bx(ci.boundingBox())).append('|').append(ci.boundingOctagon());
        sb.append('|').append(ci.boundingOctagon().isNormalized());
        sb.append('|').append(b(ci.distance(new FloatPoint(px, py)))).append('/').append(b(ci.borderDistance(new FloatPoint(px, py))));
        sb.append('|').append(ci.contains((Point) new IntPoint(px, py))).append('/').append(ci.isOutside(new IntPoint(px, py)))
          .append('/').append(ci.containsInside(new IntPoint(px, py))).append('/').append(ci.containsOnBorder(new IntPoint(px, py)));
        IntBox box = new IntBox(bx, by, bx + bw, by + bh);
        sb.append('|').append(ci.intersects(box)).append('/').append(ci.intersects(box.toIntOctagon()))
          .append('/').append(ci.intersects(box.toSimplex()))
          .append('/').append(ci.intersects(new Circle(new IntPoint(px, py), 1 + nextRange(range))));
        sb.append('|').append(ci.isContainedIn(box));
        sb.append('|').append(ci.offset(2.5).radius).append('/').append(ci.shrink(2.5).radius).append('/').append(ci.enlarge(-1.5).radius);
        sb.append('|');
        try { TileShape bt = ci.boundingTile(1 + nextRange(range));
              sb.append(cls(bt)).append('/').append(bt.borderLineCount()).append('/').append(b(bt.area())); }
        catch (Throwable t) { sb.append("EXC:").append(t.getClass().getSimpleName()); }
        sb.append('|').append(ci);
      }
      out.println(sb);
    }
    out.flush();
  }
}
