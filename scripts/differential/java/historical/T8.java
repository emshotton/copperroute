package app.freerouting.geometry.planar;

/**
 * Plan 2 Task 8 driver: the geometry bodies of `board/trace/PolylineTraceGeometry.java`,
 * `board/trace/PolylineTrace.java` (geometry half) and `board/model/items/Trace.java`
 * (`nearestEndPoint`), inlined over the real `geometry/planar` classes.
 *
 * `PolylineTrace` itself cannot be compiled standalone (it drags in BasicBoard and the whole
 * board stack), so each body is reproduced here verbatim, exactly as T6/T7 did for `Pin` and
 * `ObstacleArea`.
 */
public class T8 {

  // --- PolylineTraceGeometry.java, verbatim -------------------------------------------------
  static Point firstCorner(Polyline lines) { return lines.corner(0); }
  static Point lastCorner(Polyline lines) { return lines.corner(lines.lines.length - 2); }
  static int cornerCount(Polyline lines) { return lines.lines.length - 1; }
  static double length(Polyline lines) { return lines.lengthApprox(); }
  static IntBox boundingBox(Polyline lines, int halfWidth) {
    return lines.boundingBox().offset(halfWidth);
  }
  static int tileShapeCount(Polyline lines) { return Math.max(lines.lines.length - 2, 0); }
  static Polyline translate(Polyline lines, Vector vector) { return lines.translateBy(vector); }
  static Polyline turn90Degree(Polyline lines, int factor, IntPoint pole) {
    return lines.turn90Degree(factor, pole);
  }
  static Polyline rotateApprox(Polyline lines, double angleInDegree, FloatPoint pole) {
    return lines.rotateApprox(Math.toRadians(angleInDegree), pole);
  }
  static Polyline mirrorVertical(Polyline lines, IntPoint pole) { return lines.mirrorVertical(pole); }
  static TileShape connectionShape(Polyline lines, int index) {
    LineSegment currentLineSegment = new LineSegment(lines, index + 1);
    return currentLineSegment.toSimplex().simplify();
  }

  // --- Trace.nearestEndPoint (Trace.java:256-269), verbatim ----------------------------------
  static Point nearestEndPoint(Polyline lines, Point fromPoint) {
    Point p1 = firstCorner(lines);
    Point p2 = lastCorner(lines);
    FloatPoint fromPointFloat = fromPoint.toFloat();
    double d1 = fromPointFloat.distance(p1.toFloat());
    double d2 = fromPointFloat.distance(p2.toFloat());
    Point result;
    if (d1 < d2) { result = p1; } else { result = p2; }
    return result;
  }

  // --- PolylineTrace.split(Point) (PolylineTrace.java:699-712), geometry core ----------------
  static Polyline[] splitAtPoint(Polyline lines, Point point) {
    for (int i = 0; i < lines.lines.length - 2; i++) {
      LineSegment currentLineSegment = new LineSegment(lines, i + 1);
      if (currentLineSegment.contains(point)) {
        Direction splitLineDirection = currentLineSegment.getLine().direction().turn45Degree(2);
        Line splitLine = new Line(point, splitLineDirection);
        Polyline[] result = lines.split(i + 1, splitLine);
        if (result != null) { return result; }
      }
    }
    return null;
  }

  static String pt(Point p) { return p == null ? "null" : p.toString(); }
  static String bx(IntBox b) {
    return "[" + b.ll.x + "," + b.ll.y + " .. " + b.ur.x + "," + b.ur.y + "]";
  }

  static void dump(String name, Polyline pl, int halfWidth) {
    System.out.println("== " + name + " halfWidth=" + halfWidth);
    System.out.println("  lineCount=" + pl.lines.length);
    System.out.println("  firstCorner=" + pt(firstCorner(pl)));
    System.out.println("  lastCorner=" + pt(lastCorner(pl)));
    System.out.println("  cornerCount=" + cornerCount(pl));
    System.out.println("  length=" + length(pl));
    System.out.println("  boundingBox=" + bx(boundingBox(pl, halfWidth)));
    System.out.println("  tileShapeCount=" + tileShapeCount(pl));
    for (int i = 0; i < tileShapeCount(pl); i++) {
      TileShape cs = connectionShape(pl, i);
      System.out.println("  connectionShape[" + i + "]=" + cs.getClass().getSimpleName()
          + " bbox=" + bx(cs.boundingBox()) + " dim=" + cs.dimension());
    }
    for (int i = 0; i < pl.cornerCount(); i++) {
      System.out.println("  corner[" + i + "]=" + pt(pl.corner(i)));
    }
    TileShape[] offs = pl.offsetShapes(halfWidth);
    System.out.println("  offsetShapes=" + offs.length);
    for (int i = 0; i < offs.length; i++) {
      System.out.println("    offsetShape[" + i + "]=" + offs[i].getClass().getSimpleName()
          + " bbox=" + bx(offs[i].boundingBox()));
    }
  }

  static void dumpTransform(String name, Polyline pl) {
    Polyline t = translate(pl, new IntVector(1000, -2000));
    System.out.println("  translate(1000,-2000): first=" + pt(firstCorner(t)) + " last=" + pt(lastCorner(t)));
    Polyline r = turn90Degree(pl, 1, new IntPoint(0, 0));
    System.out.println("  turn90(1,origin): first=" + pt(firstCorner(r)) + " last=" + pt(lastCorner(r)));
    Polyline ra = rotateApprox(pl, 90.0, new FloatPoint(0, 0));
    System.out.println("  rotateApprox(90,origin): first=" + pt(firstCorner(ra)) + " last=" + pt(lastCorner(ra))
        + " lineCount=" + ra.lines.length);
    Polyline mv = mirrorVertical(pl, new IntPoint(0, 0));
    System.out.println("  mirrorVertical(origin): first=" + pt(firstCorner(mv)) + " last=" + pt(lastCorner(mv)));
  }

  public static void main(String[] args) {
    // A: PolylineTraceSplitTest.java:391 — a two-point trace.
    Polyline a = new Polyline(new IntPoint(10000, 10000), new IntPoint(20000, 10000));
    dump("A two-point (10000,10000)->(20000,10000)", a, 1000);
    dumpTransform("A", a);
    System.out.println("  nearestEndPoint((11000,10000))=" + pt(nearestEndPoint(a, new IntPoint(11000, 10000))));
    System.out.println("  nearestEndPoint((19000,10000))=" + pt(nearestEndPoint(a, new IntPoint(19000, 10000))));
    System.out.println("  nearestEndPoint((15000,10000))=" + pt(nearestEndPoint(a, new IntPoint(15000, 10000))));
    Polyline[] sa = splitAtPoint(a, new IntPoint(15000, 10000));
    System.out.println("  splitAtPoint((15000,10000))=" + (sa == null ? "null" : sa.length));
    if (sa != null) {
      for (int i = 0; i < sa.length; i++) {
        System.out.println("    piece[" + i + "] first=" + pt(firstCorner(sa[i])) + " last=" + pt(lastCorner(sa[i]))
            + " lineCount=" + sa[i].lines.length);
      }
    }
    System.out.println("  splitAtPoint((15000,12345))=" + (splitAtPoint(a, new IntPoint(15000, 12345)) == null ? "null" : "some"));
    System.out.println("  splitAtPoint(firstCorner)=" + (splitAtPoint(a, new IntPoint(10000, 10000)) == null ? "null" : "some"));

    // B: PolylineTraceSplitTest.java:233 — four collinear points.
    Polyline b = new Polyline(new Point[] {
        new IntPoint(0, 0), new IntPoint(10000, 0), new IntPoint(20000, 0), new IntPoint(30000, 0)});
    dump("B four collinear (0,0)..(30000,0)", b, 1000);
    dumpTransform("B", b);

    // C: PolylineTraceSplitTest.java:75 — the bug-report trace.
    Polyline c = new Polyline(new Point[] {
        new IntPoint(1291423, -987076), new IntPoint(1270000, -975000),
        new IntPoint(1250000, -970000), new IntPoint(1243227, -964893)});
    dump("C bug-report polyline", c, 1000);

    // D: an L-shaped trace, for split / connection shapes.
    Polyline d = new Polyline(new Point[] {
        new IntPoint(0, 0), new IntPoint(10000, 0), new IntPoint(10000, 10000)});
    dump("D L-shape (0,0)-(10000,0)-(10000,10000)", d, 500);
    dumpTransform("D", d);
    Polyline[] sd = splitAtPoint(d, new IntPoint(5000, 0));
    System.out.println("  splitAtPoint((5000,0))=" + (sd == null ? "null" : sd.length));
    if (sd != null) {
      for (int i = 0; i < sd.length; i++) {
        System.out.println("    piece[" + i + "] first=" + pt(firstCorner(sd[i])) + " last=" + pt(lastCorner(sd[i]))
            + " lineCount=" + sd[i].lines.length + " cornerCount=" + cornerCount(sd[i]));
      }
    }
    Polyline[] sd2 = splitAtPoint(d, new IntPoint(10000, 0));
    System.out.println("  splitAtPoint(corner (10000,0))=" + (sd2 == null ? "null" : sd2.length));
  }
}
