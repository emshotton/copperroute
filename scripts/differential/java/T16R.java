package app.freerouting.geometry.planar;

public class T16R {
  static StringBuilder sb = new StringBuilder();
  static long seed;

  static long next() { seed ^= seed << 13; seed ^= seed >>> 7; seed ^= seed << 17; return seed; }
  static int r(int lo, int hi) { long v = (next() >>> 1) % (long) (hi - lo + 1); return (int) (lo + v); }

  interface Th { String get() throws Throwable; }
  static void emit(String name, Th t) {
    String v;
    try { v = t.get(); } catch (Throwable e) { v = "EXC"; }
    sb.append(name).append("=").append(v).append("\n");
  }

  static String f(double d) { return "b" + Double.doubleToRawLongBits(d); }
  static String cpt(Point p) {
    if (p == null) return "null";
    if (p instanceof IntPoint q) return "I" + q.x + "/" + q.y;
    RationalPoint rr = (RationalPoint) p;
    return "R" + rr.x + "/" + rr.y + "/" + rr.z;
  }
  static String fp(FloatPoint p) { return p == null ? "null" : "(" + f(p.x) + "," + f(p.y) + ")"; }
  static String ln(Line l) { return l == null ? "null" : "[" + cpt(l.a) + ";" + cpt(l.b) + "]"; }
  static String box(IntBox b) { return b == null ? "null" : "B(" + b.ll.x + "," + b.ll.y + "," + b.ur.x + "," + b.ur.y + ")"; }
  static String oct(IntOctagon o) {
    return o == null ? "null" : "O(" + o.leftX + "," + o.bottomY + "," + o.rightX + "," + o.topY + ","
        + o.upperLeftDiagonalX + "," + o.lowerRightDiagonalX + "," + o.lowerLeftDiagonalX + "," + o.upperRightDiagonalX + ")";
  }
  static String ts(TileShape s) {
    if (s == null) return "null";
    if (s instanceof IntBox b) return box(b);
    if (s instanceof IntOctagon o) return oct(o);
    StringBuilder b = new StringBuilder("S{");
    for (int i = 0; i < s.borderLineCount(); i++) { if (i > 0) b.append(" "); b.append(ln(s.borderLine(i))); }
    return b.append("}").toString();
  }
  static String tsArr(TileShape[] a) {
    if (a == null) return "null";
    StringBuilder b = new StringBuilder("[" + a.length + ":");
    for (TileShape s : a) { b.append(ts(s)).append(";"); }
    return b.append("]").toString();
  }
  static String pl(Polyline p) {
    if (p == null) return "null";
    StringBuilder b = new StringBuilder("P[" + p.lines.length + ":");
    for (Line l : p.lines) b.append(ln(l)).append(";");
    return b.append("]").toString();
  }
  static String plArr(Polyline[] a) {
    if (a == null) return "null";
    StringBuilder b = new StringBuilder("[" + a.length + ":");
    for (Polyline p : a) b.append(pl(p)).append(";");
    return b.append("]").toString();
  }
  static String ptArr(Point[] a) {
    if (a == null) return "null";
    StringBuilder b = new StringBuilder("[" + a.length + ":");
    for (Point p : a) b.append(cpt(p)).append(";");
    return b.append("]").toString();
  }
  static String fpArr(FloatPoint[] a) {
    if (a == null) return "null";
    StringBuilder b = new StringBuilder("[" + a.length + ":");
    for (FloatPoint p : a) b.append(fp(p)).append(";");
    return b.append("]").toString();
  }
  static String iiArr(int[][] a) {
    if (a == null) return "null";
    StringBuilder b = new StringBuilder("[" + a.length + ":");
    for (int[] p : a) b.append(p[0]).append("/").append(p[1]).append(";");
    return b.append("]").toString();
  }
  static String seg(LineSegment s) {
    if (s == null) return "null";
    return "SEG{" + ln(s.getStartClosingLine()) + " " + ln(s.getLine()) + " " + ln(s.getEndClosingLine()) + "}";
  }

  static Point[] genPoints(int lo, int hi, int c) {
    int n = r(lo, hi);
    Point[] pts = new Point[n];
    for (int i = 0; i < n; i++) pts[i] = new IntPoint(r(-c, c), r(-c, c));
    return pts;
  }

  static Line genLine(int c) {
    IntPoint a = new IntPoint(r(-c, c), r(-c, c));
    IntPoint b = new IntPoint(r(-c, c), r(-c, c));
    if (a.equals(b)) b = new IntPoint(b.x + 1, b.y);
    return new Line(a, b);
  }

  static IntBox genBox() {
    int x = r(-6, 6), y = r(-6, 6), w = r(1, 12), h = r(1, 12);
    return new IntBox(new IntPoint(x, y), new IntPoint(x + w, y + h));
  }

  public static void main(String[] args) {
    int iters = Integer.parseInt(args[0]);
    seed = Long.parseLong(args[1]);
    int mode = Integer.parseInt(args[2]);
    for (int it = 0; it < iters; it++) {
      if (mode == 3) {
        // targeted Polyline(Line[]) sweep: draw from a small pool of lines and their opposites
        int poolSize = r(2, 4);
        Line[] pool = new Line[poolSize * 2];
        for (int i = 0; i < poolSize; i++) { pool[2 * i] = genLine(4); pool[2 * i + 1] = pool[2 * i].opposite(); }
        int n = r(3, 10);
        final Line[] arr = new Line[n];
        for (int i = 0; i < n; i++) arr[i] = pool[r(0, pool.length - 1)];
        final Line[] arrCopy = arr.clone();
        emit("in", () -> { StringBuilder b = new StringBuilder("[" + arrCopy.length + ":"); for (Line l : arrCopy) b.append(ln(l)).append(";"); return b.append("]").toString(); });
        emit("fromLines", () -> pl(new Polyline(arr)));
        continue;
      }
      final int c = mode == 1 ? 9 : 6;
      final Point[] pts;
      if (mode == 4) {
        // 45/90-degree walk: the real trace geometry
        int n = r(3, 10);
        Point[] w = new Point[n];
        int x = r(-8, 8), y = r(-8, 8);
        int[] dx = {1, 1, 0, -1, -1, -1, 0, 1};
        int[] dy = {0, 1, 1, 1, 0, -1, -1, -1};
        for (int i = 0; i < n; i++) {
          int d = r(0, 7), len = r(1, 8);
          x += dx[d] * len; y += dy[d] * len;
          w[i] = new IntPoint(x, y);
        }
        pts = w;
      } else {
        pts = mode == 1 ? genPoints(1, 6, c) : genPoints(4, 12, c);
      }
      final int hw = mode == 1 ? r(0, 4) : r(1, 10);
      emit("pts", () -> ptArr(pts));
      final Polygon poly = new Polygon(pts);
      emit("polyCorners", () -> ptArr(poly.cornerArray()));
      emit("polyRevert", () -> ptArr(poly.revertCorners().cornerArray()));
      emit("polyWind", () -> String.valueOf(poly.windingNumberAfterClosing()));
      final Polyline p = new Polyline(pts);
      emit("p", () -> pl(p));
      emit("cornerCount", () -> String.valueOf(p.cornerCount()));
      emit("corners", () -> ptArr(p.corners()));
      emit("cornerApproxArr", () -> fpArr(p.cornerApproxArr()));
      emit("isEmpty", () -> String.valueOf(p.isEmpty()));
      emit("isPoint", () -> String.valueOf(p.isPoint()));
      emit("isOrth", () -> String.valueOf(p.isOrthogonal()));
      emit("is45", () -> String.valueOf(p.isMultipleOf45Degree()));
      emit("first", () -> cpt(p.firstCorner()));
      emit("last", () -> cpt(p.lastCorner()));
      emit("lenApprox", () -> f(p.lengthApprox()));
      final int a1 = r(0, 6), a2 = r(0, 6);
      emit("lenApproxAB", () -> f(p.lengthApprox(Math.min(a1, a2), Math.max(a1, a2))));
      emit("bbox", () -> box(p.boundingBox()));
      emit("bboxAB", () -> box(p.boundingBox(Math.min(a1, a2), Math.max(a1, a2))));
      emit("boctAB", () -> oct(p.boundingOctagon(Math.min(a1, a2), Math.max(a1, a2))));
      emit("reverse", () -> pl(p.reverse()));
      emit("offsetShapes", () -> tsArr(p.offsetShapes(hw)));
      emit("offsetShapesAB", () -> tsArr(p.offsetShapes(hw, Math.min(a1, a2), Math.max(a1, a2))));
      final int k = r(0, 5);
      emit("offsetShape", () -> ts(p.offsetShape(hw, k)));
      emit("offsetBox", () -> box(p.offsetBox(hw, k)));
      emit("translate", () -> pl(p.translateBy(new IntVector(r(-3, 3), r(-3, 3)))));
      final int fac = r(0, 7);
      emit("turn90", () -> pl(p.turn90Degree(fac, new IntPoint(r(-3, 3), r(-3, 3)))));
      emit("mirrorV", () -> pl(p.mirrorVertical(new IntPoint(r(-3, 3), r(-3, 3)))));
      emit("mirrorH", () -> pl(p.mirrorHorizontal(new IntPoint(r(-3, 3), r(-3, 3)))));
      final double ang = r(0, 12) * 0.5;
      emit("rotateApprox", () -> pl(p.rotateApprox(ang, new FloatPoint(r(-4, 4), r(-4, 4)))));
      final FloatPoint q = new FloatPoint(r(-12, 12), r(-12, 12));
      emit("nearest", () -> fp(p.nearestPointApprox(q)));
      emit("distance", () -> f(p.distance(q)));
      final IntPoint qi = new IntPoint(r(-12, 12), r(-12, 12));
      emit("contains", () -> String.valueOf(p.contains(qi)));
      emit("projection", () -> seg(p.projectionLine(qi)));
      emit("skipLines", () -> pl(p.skipLines(Math.min(a1, a2), Math.max(a1, a2))));
      final int nlc = r(0, 8);
      final double sl = r(1, 15);
      emit("shorten", () -> pl(p.shorten(nlc, sl)));
      final Point[] pts2 = mode == 1 ? genPoints(1, 6, c) : genPoints(4, 12, c);
      final Polyline p2 = new Polyline(pts2);
      emit("combine", () -> pl(p.combine(p2)));
      // combine with a polyline sharing an end corner
      final IntPoint sh1 = new IntPoint(r(-c, c), r(-c, c));
      final IntPoint sh2 = new IntPoint(r(-c, c), r(-c, c));
      emit("combineShared", () -> {
        Point[] pc = p.corners();
        if (pc.length == 0) return "skip";
        Point endPt = pc[pc.length - 1];
        Point[] q2 = new Point[] {endPt, sh1, sh2};
        return pl(p.combine(new Polyline(q2)));
      });
      final IntPoint sh3 = new IntPoint(r(-c, c), r(-c, c));
      final IntPoint sh4 = new IntPoint(r(-c, c), r(-c, c));
      emit("combineSharedStart", () -> {
        Point[] pc = p.corners();
        if (pc.length == 0) return "skip";
        Point startPt = pc[0];
        Point[] q2 = new Point[] {startPt, sh3, sh4};
        return pl(p.combine(new Polyline(q2)));
      });
      final int si = r(0, 6);
      final Line el = genLine(c);
      emit("split", () -> plArr(p.split(si, el)));
      final int lsn = r(0, 6);
      emit("lineSegment", () -> seg(new LineSegment(p, lsn)));
      emit("toPolyline", () -> { LineSegment s = new LineSegment(p, lsn); return pl(s.toPolyline()); });
      final IntBox bx = genBox();
      emit("shapeRotate", () -> ts(((TileShape) bx).rotateApprox(ang, new FloatPoint(r(-4, 4), r(-4, 4)))));
      emit("entrance", () -> iiArr(((TileShape) bx).entrancePoints(p)));
      emit("cutout", () -> plArr(((TileShape) bx).cutout(p)));
      final Simplex sx = bx.toSimplex();
      emit("simplexEntrance", () -> iiArr(((TileShape) sx).entrancePoints(p)));
      emit("simplexCutout", () -> plArr(((TileShape) sx).cutout(p)));
    }
    System.out.print(sb);
  }
}
