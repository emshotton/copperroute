package app.freerouting.geometry.planar;

public class T15 {
  static StringBuilder sb = new StringBuilder();
  static long seed;
  static int C = 9;

  static long next() {
    seed ^= seed << 13;
    seed ^= seed >>> 7;
    seed ^= seed << 17;
    return seed;
  }

  static int r(int lo, int hi) {
    long v = (next() >>> 1) % (long) (hi - lo + 1);
    return (int) (lo + v);
  }

  interface Th { String get() throws Throwable; }

  static void emit(String name, Th t) {
    String v;
    try { v = t.get(); } catch (Throwable e) { v = "EXC:" + e.getClass().getSimpleName(); }
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

  static String ln(Line l) {
    if (l == null) return "null";
    return "[" + cpt(l.a) + ";" + cpt(l.b) + "]";
  }

  static String box(IntBox b) { return b == null ? "null" : "B(" + b.ll.x + "," + b.ll.y + "," + b.ur.x + "," + b.ur.y + ")"; }

  static String oct(IntOctagon o) {
    return o == null ? "null" : "O(" + o.leftX + "," + o.bottomY + "," + o.rightX + "," + o.topY + ","
        + o.upperLeftDiagonalX + "," + o.lowerRightDiagonalX + "," + o.lowerLeftDiagonalX + "," + o.upperRightDiagonalX + ")";
  }

  static String simp(Simplex s) {
    StringBuilder b = new StringBuilder("S{");
    for (int i = 0; i < s.borderLineCount(); i++) {
      if (i > 0) b.append(" ");
      b.append(ln(s.borderLine(i)));
    }
    return b.append("}").toString();
  }

  static String ts(TileShape s) {
    if (s == null) return "null";
    if (s instanceof IntBox b) return box(b);
    if (s instanceof IntOctagon o) return oct(o);
    return simp((Simplex) s);
  }

  static String lnArr(Line[] a) {
    if (a == null) return "null";
    StringBuilder b = new StringBuilder("[" + a.length + ":");
    for (Line s : a) b.append(ln(s)).append(";");
    return b.append("]").toString();
  }

  static String ipArr(IntPoint[] a) {
    if (a == null) return "null";
    StringBuilder b = new StringBuilder("[" + a.length + ":");
    for (IntPoint s : a) b.append(s == null ? "null" : (s.x + "/" + s.y)).append(";");
    return b.append("]").toString();
  }

  static String iArr(int[] a) {
    if (a == null) return "null";
    StringBuilder b = new StringBuilder("[" + a.length + ":");
    for (int s : a) b.append(s).append(";");
    return b.append("]").toString();
  }

  static String segs(LineSegment s) {
    if (s == null) return "null";
    if (s.getLine() == null) return "SEGnull";
    return "SEG{" + ln(s.getStartClosingLine()) + " " + ln(s.getLine()) + " " + ln(s.getEndClosingLine()) + "}";
  }

  static IntBox genBox() {
    int x = r(-6, 6), y = r(-6, 6), w = r(1, 12), h = r(1, 12);
    return new IntBox(x, y, x + w, y + h);
  }

  static IntOctagon genOct() {
    IntBox b = genBox();
    int a1 = r(0, 4), a2 = r(0, 4), a3 = r(0, 4), a4 = r(0, 4);
    IntOctagon o = new IntOctagon(b.ll.x, b.ll.y, b.ur.x, b.ur.y,
        b.ll.x - b.ur.y + a1, b.ur.x - b.ll.y - a2, b.ll.x + b.ll.y + a3, b.ur.x + b.ur.y - a4);
    return o.normalize();
  }

  static Simplex genSimplex() {
    int n = r(2, 5);
    Line[] lines = new Line[n];
    for (int i = 0; i < n; i++) {
      int x1 = r(-7, 7), y1 = r(-7, 7), dx = r(-6, 6), dy = r(-6, 6);
      if (dx == 0 && dy == 0) dx = 1;
      lines[i] = new Line(new IntPoint(x1, y1), new IntPoint(x1 + dx, y1 + dy));
    }
    return Simplex.getInstance(lines);
  }

  static TileShape gen() {
    int k = r(0, 2);
    if (k == 0) return genBox();
    if (k == 1) return genOct();
    return genSimplex();
  }

  static LineSegment genSeg() {
    int mode = r(0, 1);
    if (mode == 0) {
      int ax = r(-C, C), ay = r(-C, C), bx = r(-C, C), by = r(-C, C);
      if (ax == bx && ay == by) bx = ax + 1;
      IntPoint a = new IntPoint(ax, ay);
      IntPoint b = new IntPoint(bx, by);
      Line middle = new Line(a, b);
      Direction perp = middle.direction().turn45Degree(2);
      return new LineSegment(new Line(a, perp), middle, new Line(b, perp));
    }
    Line[] l = new Line[3];
    for (int i = 0; i < 3; i++) {
      int x1 = r(-C, C), y1 = r(-C, C), dx = r(-C, C), dy = r(-C, C);
      if (dx == 0 && dy == 0) dx = 1;
      l[i] = new Line(new IntPoint(x1, y1), new IntPoint(x1 + dx, y1 + dy));
    }
    Direction perp = l[1].direction().turn45Degree(2);
    if (l[0].isParallel(l[1])) l[0] = new Line(l[0].a, perp);
    if (l[2].isParallel(l[1])) l[2] = new Line(l[2].a, perp);
    return new LineSegment(l[0], l[1], l[2]);
  }

  static boolean small(LineSegment s) {
    IntPoint a = s.startPoint().toFloat().round();
    IntPoint b = s.endPoint().toFloat().round();
    return Math.abs(a.x) < 5000 && Math.abs(a.y) < 5000 && Math.abs(b.x) < 5000 && Math.abs(b.y) < 5000;
  }

  public static void main(String[] args) {
    seed = Long.parseLong(args[1]);
    if (args.length > 2) C = Integer.parseInt(args[2]);
    int iters = Integer.parseInt(args[0]);
    for (int it = 0; it < iters; it++) {
      final TileShape shp = gen();
      final LineSegment s1 = genSeg();
      final LineSegment s2 = genSeg();
      final IntPoint p = new IntPoint(r(-C, C), r(-C, C));
      final double w = r(1, 40) / 10.0;
      final double w2 = r(1, 200) / 10.0;
      sb.append("### ").append(it).append("\n");
      emit("SHP", () -> ts(shp));
      emit("S1", () -> segs(s1));
      emit("S2", () -> segs(s2));
      emit("P", () -> cpt(p));
      emit("W", () -> f(w));
      emit("S1.startPoint", () -> cpt(s1.startPoint()));
      emit("S1.endPoint", () -> cpt(s1.endPoint()));
      emit("S1.startPointApprox", () -> fp(new LineSegment(s1.getStartClosingLine(), s1.getLine(), s1.getEndClosingLine()).startPointApprox()));
      emit("S1.endPointApprox", () -> fp(new LineSegment(s1.getStartClosingLine(), s1.getLine(), s1.getEndClosingLine()).endPointApprox()));
      emit("S1.getLine", () -> ln(s1.getLine()));
      emit("S1.startClosing", () -> ln(s1.getStartClosingLine()));
      emit("S1.endClosing", () -> ln(s1.getEndClosingLine()));
      emit("S1.opposite", () -> segs(s1.opposite()));
      emit("S1.toSimplex", () -> simp(s1.toSimplex()));
      emit("S1.contains", () -> "" + s1.contains(p));
      emit("S1.containsStart", () -> "" + s1.contains(s1.startPoint()));
      emit("S1.boundingBox", () -> box(s1.boundingBox()));
      emit("S1.boundingOctagon", () -> oct(s1.boundingOctagon()));
      emit("S1.changeLen", () -> segs(new LineSegment(s1.getStartClosingLine(), s1.getLine(), s1.getEndClosingLine()).changeLengthApprox(w2)));
      emit("S1.changeLen0", () -> segs(new LineSegment(s1.getStartClosingLine(), s1.getLine(), s1.getEndClosingLine()).changeLengthApprox(0.0)));
      emit("S1.sortXY", () -> segs(s1.sortEndpointsInXY()));
      emit("S2.sortXY", () -> segs(s2.sortEndpointsInXY()));
      emit("S1.intersection", () -> lnArr(s1.intersection(s2)));
      emit("S2.intersection", () -> lnArr(s2.intersection(s1)));
      emit("S1.intersects", () -> "" + s1.intersects(s2));
      emit("S1.overlaps", () -> "" + s1.overlaps(s2));
      emit("S1.selfIntersection", () -> lnArr(s1.intersection(s1)));
      if (small(s1)) {
        emit("S1.stairT", () -> ipArr(s1.stairApproximation(w, true)));
        emit("S1.stairF", () -> ipArr(s1.stairApproximation(w, false)));
        emit("S1.stair45T", () -> ipArr(s1.stairApproximation45(w, true)));
        emit("S1.stair45F", () -> ipArr(s1.stairApproximation45(w, false)));
        emit("S1.stairBigT", () -> ipArr(s1.stairApproximation(w2, true)));
        emit("S1.stair45BigF", () -> ipArr(s1.stairApproximation45(w2, false)));
      }
      // collinear partner of s1, to stress the overlap branch of intersection()
      final Direction cperp = s1.getLine().direction().turn45Degree(2);
      final int q1mode = r(0, 2);
      final IntPoint q1 = (q1mode == 0) ? s1.endPoint().toFloat().round() : new IntPoint(r(-C, C), r(-C, C));
      final int q2mode = r(0, 2);
      final IntPoint q2 = (q2mode == 0) ? s1.startPoint().toFloat().round() : new IntPoint(r(-C, C), r(-C, C));
      final boolean flip = r(0, 1) == 0;
      final Line cmid = flip ? s1.getLine().opposite() : s1.getLine();
      final LineSegment s3 = new LineSegment(new Line(q1, cperp), cmid, new Line(q2, cperp));
      emit("S3", () -> segs(s3));
      emit("S3.startPoint", () -> cpt(s3.startPoint()));
      emit("S3.endPoint", () -> cpt(s3.endPoint()));
      emit("S1.interS3", () -> lnArr(s1.intersection(s3)));
      emit("S3.interS1", () -> lnArr(s3.intersection(s1)));
      emit("S1.intersectsS3", () -> "" + s1.intersects(s3));
      emit("S1.overlapsS3", () -> "" + s1.overlaps(s3));
      emit("S3.overlapsS1", () -> "" + s3.overlaps(s1));
      emit("S3.sortXY", () -> segs(s3.sortEndpointsInXY()));
      emit("S3.toSimplex", () -> simp(s3.toSimplex()));
      emit("S1.containsQ1", () -> "" + s1.contains(q1));
      emit("S3.borderIntersections", () -> iArr(s3.borderIntersections(shp)));
      emit("SHP.intersectedInteriorS3", () -> "" + shp.isIntersectedInteriorBy(s3));
      emit("S1.borderIntersections", () -> iArr(s1.borderIntersections(shp)));
      emit("S2.borderIntersections", () -> iArr(s2.borderIntersections(shp)));
      emit("SHP.intersectedInteriorS1", () -> "" + shp.isIntersectedInteriorBy(s1));
      emit("SHP.intersectedInteriorS2", () -> "" + shp.isIntersectedInteriorBy(s2));
      emit("S1.startPointApproxCached", () -> fp(s1.startPointApprox()));
      emit("S1.endPointApproxCached", () -> fp(s1.endPointApprox()));
      final int blc = shp.borderLineCount();
      for (int k = 0; k <= blc; k++) {
        final int kk = k;
        emit("SHP.seg" + kk, () -> segs(new LineSegment(shp, kk)));
      }
    }
    System.out.print(sb);
  }
}
