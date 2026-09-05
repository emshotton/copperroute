package app.freerouting.geometry.planar;

import java.util.*;

public class T14 {
  static StringBuilder sb = new StringBuilder();
  static long seed;

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

  static String tsArr(TileShape[] a) {
    if (a == null) return "null";
    StringBuilder b = new StringBuilder("[" + a.length + ":");
    for (TileShape s : a) b.append(ts(s)).append(";");
    return b.append("]").toString();
  }

  static String fpArr(FloatPoint[] a) {
    if (a == null) return "null";
    int n = a.length;
    while (n > 0 && a[n - 1] == null) n--;
    boolean hole = false;
    for (int i = 0; i < n; i++) if (a[i] == null) hole = true;
    StringBuilder b = new StringBuilder("[" + n + ":");
    for (int i = 0; i < n; i++) b.append(fp(a[i])).append(";");
    if (hole) b.append("HOLE");
    return b.append("]").toString();
  }

  static String ipArr(IntPoint[] a) {
    if (a == null) return "null";
    int n = a.length;
    while (n > 0 && a[n - 1] == null) n--;
    boolean hole = false;
    for (int i = 0; i < n; i++) if (a[i] == null) hole = true;
    StringBuilder b = new StringBuilder("[" + n + ":");
    for (int i = 0; i < n; i++) b.append(a[i].x + "/" + a[i].y).append(";");
    if (hole) b.append("HOLE");
    return b.append("]").toString();
  }

  static String iArr(int[] a) {
    if (a == null) return "null";
    StringBuilder b = new StringBuilder("[" + a.length + ":");
    for (int s : a) b.append(s).append(";");
    return b.append("]").toString();
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

  public static void main(String[] args) {
    seed = 88172645463325252L;
    int iters = Integer.parseInt(args[0]);
    for (int it = 0; it < iters; it++) {
      final int itf = it;
      final TileShape a = gen();
      final TileShape b = gen();
      final IntPoint p = new IntPoint(r(-9, 9), r(-9, 9));
      final IntPoint p2 = new IntPoint(r(-9, 9), r(-9, 9));
      final FloatPoint pf = new FloatPoint(r(-9, 9) + 0.5, r(-9, 9) + 0.25);
      final Line l = new Line(new IntPoint(r(-8, 8), r(-8, 8)), new IntPoint(r(-8, 8), r(-8, 8)));
      final int edge = r(0, 3);
      final double w = r(1, 12);
      sb.append("### ").append(it).append("\n");
      if (it == 0) {
        final TileShape eb = new IntBox(5, 5, 0, 0);
        emit("EB.divide", () -> tsArr(eb.divideIntoSections(3.0)));
        emit("EB.divide0", () -> tsArr(eb.divideIntoSections(0.0)));
        final TileShape db = new IntBox(0, 0, 10, 0);
        emit("DB.divide", () -> tsArr(db.divideIntoSections(3.0)));
        final TileShape nb = new IntBox(0, 0, 10, 10);
        emit("NB.divide", () -> tsArr(nb.divideIntoSections(4.0)));
        emit("NB.divide0", () -> tsArr(nb.divideIntoSections(0.0)));
        emit("NB.divideNeg", () -> tsArr(nb.divideIntoSections(-1.0)));
        final TileShape b5 = new IntBox(0, 0, 5, 5);
        emit("B5.divide15", () -> tsArr(b5.divideIntoSections(1.5)));
        final TileShape o5 = new IntBox(0, 0, 5, 5).toIntOctagon();
        emit("O5.divide15", () -> tsArr(o5.divideIntoSections(1.5)));
        final TileShape s5 = new IntBox(0, 0, 5, 5).toSimplex();
        emit("S5.divide15", () -> tsArr(s5.divideIntoSections(1.5)));
        final TileShape eo = new IntOctagon(5, 5, 0, 0, 0, 0, 0, 0);
        emit("EO.divide", () -> tsArr(eo.divideIntoSections(3.0)));
        final TileShape es = Simplex.EMPTY;
        emit("ES.divide", () -> tsArr(es.divideIntoSections(3.0)));
        emit("ES.circum", () -> f(es.circumference()));
        emit("ES.area", () -> f(es.area()));
        emit("ES.length", () -> f(es.length()));
        emit("ES.centre", () -> fp(es.centreOfGravity()));
        emit("ES.indexOfNearestCorner", () -> "" + es.indexOfNearestCorner(new IntPoint(1, 1)));
        emit("ES.diagonal", () -> {
          FloatLine fl = es.diagonalCornerSegment();
          return fl == null ? "null" : fp(fl.a) + "-" + fp(fl.b);
        });
        final TileShape hp = TileShape.getInstance(new Line(new IntPoint(0, 0), new IntPoint(0, 1)));
        emit("HP.contains-1,0", () -> "" + hp.contains(new IntPoint(-1, 0)));
        emit("HP.contains1,0", () -> "" + hp.contains(new IntPoint(1, 0)));
        emit("HP.nearestBorderPoint", () -> cpt(hp.nearestBorderPoint(new IntPoint(4, 7))));
        emit("HP.nearestBorderPoints3", () -> fpArr(hp.nearestBorderPointsApprox(new FloatPoint(4.5, 7.5), 3)));
      }
      emit("A", () -> ts(a));
      emit("B", () -> ts(b));
      emit("P", () -> cpt(p));
      emit("L", () -> ln(l));
      emit("A.blc", () -> "" + a.borderLineCount());
      emit("A.dim", () -> "" + a.dimension());
      emit("A.area", () -> f(a.area()));
      emit("A.circum", () -> f(a.circumference()));
      emit("A.len", () -> f(a.length()));
      emit("A.maxw", () -> f(a.maxWidth()));
      emit("A.minw", () -> f(a.minWidth()));
      emit("A.contains", () -> "" + a.contains(p));
      emit("A.containsInside", () -> "" + a.containsInside(p));
      emit("A.isOutside", () -> "" + a.isOutside(p));
      emit("A.containsOnBorderLineNo", () -> "" + a.containsOnBorderLineNo(p));
      emit("A.containsOnBorder", () -> "" + a.containsOnBorder(p));
      emit("A.containsF", () -> "" + a.contains(pf));
      emit("A.containsFTol", () -> "" + a.contains(pf, 0.5));
      emit("A.sideOfBorder", () -> "" + a.sideOfBorder(pf, 0.5));
      emit("A.containsTile", () -> "" + a.contains(b));
      emit("A.containsApprox", () -> "" + a.containsApprox(b));
      emit("A.intersects", () -> "" + a.intersects(b));
      emit("A.intersection", () -> ts(a.intersection(b)));
      emit("B.intersection", () -> ts(b.intersection(a)));
      emit("A.intersectionSimplify", () -> ts(a.intersectionWithSimplify(b)));
      emit("A.distance", () -> f(a.distance(pf)));
      emit("A.borderDistance", () -> f(a.borderDistance(pf)));
      emit("A.smallestRadius", () -> f(a.smallestRadius()));
      emit("A.nearestPoint", () -> cpt(a.nearestPoint(p)));
      emit("A.nearestPointApprox", () -> fp(a.nearestPointApprox(pf)));
      emit("A.nearestBorderPoint", () -> cpt(a.nearestBorderPoint(p)));
      emit("A.nearestBorderPointApprox", () -> fp(a.nearestBorderPointApprox(pf)));
      emit("A.nearestBorderPoints1", () -> fpArr(a.nearestBorderPointsApprox(pf, 1)));
      emit("A.nearestBorderPoints3", () -> fpArr(a.nearestBorderPointsApprox(pf, 3)));
      emit("A.indexOfNearestCorner", () -> "" + a.indexOfNearestCorner(p));
      emit("A.diagonalCornerSegment", () -> {
        FloatLine fl = a.diagonalCornerSegment();
        return fl == null ? "null" : fp(fl.a) + "-" + fp(fl.b);
      });
      emit("A.nearestRelOutside3", () -> fpArr(a.nearestRelativeOutsideLocations(b, 3)));
      emit("A.touchingSides", () -> iArr(a.touchingSides(b)));
      emit("B.touchingSides", () -> iArr(b.touchingSides(a)));
      emit("A.distanceToTheLeft", () -> f(a.distanceToTheLeft(l)));
      emit("A.sideOfLine", () -> "" + a.sideOf(l));
      emit("L.isOnTheLeft", () -> "" + l.isOnTheLeft(a));
      emit("L.isOnTheRight", () -> "" + l.isOnTheRight(a));
      emit("A.divideIntoSections", () -> a.isBounded() ? tsArr(a.divideIntoSections(w)) : "unbounded");
      emit("A.cutout", () -> (a.isBounded() && b.isBounded()) ? tsArr(a.cutout(b)) : "unbounded");
      emit("B.cutout", () -> (a.isBounded() && b.isBounded()) ? tsArr(b.cutout(a)) : "unbounded");
      emit("A.splitToConvex", () -> tsArr(a.splitToConvex()));
      emit("A.simplify", () -> ts(a.simplify()));
      emit("A.boundingBox", () -> box(a.boundingBox()));
      emit("A.boundingOctagon", () -> oct(a.boundingOctagon()));
      emit("A.boundingTile", () -> ts(a.boundingTile()));
      emit("A.getId", () -> "" + a.getId());
      emit("A.isIntBox", () -> "" + a.isIntBox());
      emit("A.isIntOctagon", () -> "" + a.isIntOctagon());
      emit("A.toSimplex", () -> simp(a.toSimplex()));
      emit("A.borderLineIndex", () -> "" + a.borderLineIndex(l));
      emit("A.borderLineIndexOwn", () -> a.borderLineCount() == 0 ? "n/a" : "" + a.borderLineIndex(a.borderLine(0)));
      emit("A.turn90", () -> ts(a.turn90Degree(1, p)));
      emit("A.turn90x3", () -> ts(a.turn90Degree(3, p)));
      emit("A.mirrorV", () -> ts(a.mirrorVertical(p)));
      emit("A.mirrorH", () -> ts(a.mirrorHorizontal(p)));
      emit("A.shrink", () -> ts((TileShape) a.shrink(2.0)));
      emit("A.offset", () -> ts((TileShape) a.offset(1.5)));
      emit("A.enlarge", () -> ts((TileShape) a.enlarge(1.5)));
      emit("A.translate", () -> ts((TileShape) a.translateBy(new IntVector(3, -2))));
      emit("A.isIntersectedInterior", () -> "" + a.isIntersectedInteriorBy(p, p2, new Line(p, p2)));
      emit("A.intersectingBorderLineNo", () -> "" + a.intersectingBorderLineNo(p, IntDirection.RIGHT45));
      emit("A.intersectingBorderLineNoUp", () -> "" + a.intersectingBorderLineNo(p, IntDirection.UP));
      emit("A.centreOfGravity", () -> fp(a.centreOfGravity()));
      emit("A.cornerApproxArr", () -> fpArr(a.cornerApproxArr()));
      emit("A.corner0", () -> a.borderLineCount() == 0 ? "n/a" : cpt(a.corner(0)));
      emit("A.cornerIsBounded0", () -> "" + a.cornerIsBounded(0));
      emit("A.boundsOrth", () -> {
        RegularTileShape rr = OrthogonalBoundingDirections.INSTANCE.bounds((ConvexShape) a);
        return rr instanceof IntBox bb ? box(bb) : oct((IntOctagon) rr);
      });
      emit("A.bounds45", () -> {
        RegularTileShape rr = FortyfiveDegreeBoundingDirections.INSTANCE.bounds((ConvexShape) a);
        return rr == null ? "null" : (rr instanceof IntBox bb ? box(bb) : oct((IntOctagon) rr));
      });
      // RegularTileShape operations
      if (a instanceof RegularTileShape ra && b instanceof RegularTileShape rb) {
        emit("R.union", () -> {
          RegularTileShape u = ra.union(rb);
          return u instanceof IntBox bb ? box(bb) : oct((IntOctagon) u);
        });
        emit("R.unionRev", () -> {
          RegularTileShape u = rb.union(ra);
          return u instanceof IntBox bb ? box(bb) : oct((IntOctagon) u);
        });
        emit("R.contains", () -> "" + ra.contains(rb));
        emit("R.containsRev", () -> "" + rb.contains(ra));
        final int emax = (a instanceof IntBox && b instanceof IntBox) ? 4 : 8;
        for (int e = 0; e < emax; e++) {
          final int ee = e;
          emit("R.compare" + e, () -> "" + ra.compare(rb, ee));
          emit("R.compareRev" + e, () -> "" + rb.compare(ra, ee));
        }
        emit("R.isContainedInBox", () -> "" + ra.isContainedIn(new IntBox(-5, -5, 12, 12)));
        emit("R.isContainedInOct", () -> "" + ra.isContainedIn(new IntBox(-5, -5, 12, 12).toIntOctagon()));
      }
      if (a instanceof IntOctagon oa) {
        emit("O.borderPointR", () -> cpt(oa.borderPoint(p, FortyfiveDegreeDirection.RIGHT)));
        emit("O.borderPointR45", () -> cpt(oa.borderPoint(p, FortyfiveDegreeDirection.RIGHT45)));
        emit("O.borderPointU45", () -> cpt(oa.borderPoint(p, FortyfiveDegreeDirection.UP45)));
        emit("O.borderPointL45", () -> cpt(oa.borderPoint(p, FortyfiveDegreeDirection.LEFT45)));
        emit("O.borderPointD45", () -> cpt(oa.borderPoint(p, FortyfiveDegreeDirection.DOWN45)));
        emit("O.borderPointD", () -> cpt(oa.borderPoint(p, FortyfiveDegreeDirection.DOWN)));
        emit("O.nearestBorderProj1", () -> ipArr(oa.nearestBorderProjections(p, 1)));
        emit("O.nearestBorderProj3", () -> ipArr(oa.nearestBorderProjections(p, 3)));
        emit("O.nearestBorderProj8", () -> ipArr(oa.nearestBorderProjections(p, 8)));
        final IntPoint c = new IntPoint((oa.leftX + oa.rightX) / 2, (oa.bottomY + oa.topY) / 2);
        emit("O.cBorderPointR45", () -> cpt(oa.borderPoint(c, FortyfiveDegreeDirection.RIGHT45)));
        emit("O.cBorderPointU45", () -> cpt(oa.borderPoint(c, FortyfiveDegreeDirection.UP45)));
        emit("O.cNearestBorderProj3", () -> ipArr(oa.nearestBorderProjections(c, 3)));
        emit("O.cNearestBorderProj8", () -> ipArr(oa.nearestBorderProjections(c, 8)));
        emit("O.cNearestBorderProj5", () -> ipArr(oa.nearestBorderProjections(c, 5)));
      }
    }
    System.out.print(sb);
  }
}
