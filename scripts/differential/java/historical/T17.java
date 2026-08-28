package app.freerouting.geometry.planar;

public class T17 {
  static Point[] pts(int[][] v) {
    Point[] r = new Point[v.length];
    for (int i = 0; i < v.length; i++) r[i] = new IntPoint(v[i][0], v[i][1]);
    return r;
  }
  static PolygonShape square() {
    return new PolygonShape(pts(new int[][]{{0,0},{10,0},{10,10},{0,10}}));
  }
  static PolygonShape lshape() {
    return new PolygonShape(pts(new int[][]{{0,0},{20,0},{20,10},{10,10},{10,20},{0,20}}));
  }
  static String corners(PolygonShape s) {
    StringBuilder sb = new StringBuilder();
    for (Point p : s.corners) sb.append(p).append(" ");
    return sb.toString();
  }
  public static void main(String[] a) {
    PolygonShape sq = square();
    PolygonShape ls = lshape();
    System.out.println("square corners: " + corners(sq));
    System.out.println("lshape corners: " + corners(ls));
    System.out.println("sq.isConvex=" + sq.isConvex());
    System.out.println("sq.area=" + sq.area());
    System.out.println("sq.dimension=" + sq.dimension());
    TileShape[] sp = sq.splitToConvex();
    System.out.println("sq.splitToConvex.len=" + (sp == null ? "null" : sp.length));
    System.out.println("ls.isConvex=" + ls.isConvex());
    System.out.println("ls.area=" + ls.area());
    TileShape[] lp = ls.splitToConvex();
    System.out.println("ls.splitToConvex.len=" + (lp == null ? "null" : lp.length));
    double sum = 0;
    for (TileShape t : lp) { sum += t.area(); System.out.println("  piece " + t.getClass().getSimpleName() + " area=" + t.area() + " " + t); }
    System.out.println("ls pieces area sum=" + sum);
    System.out.println("ls.convexHull corners=" + corners(ls.convexHull()));
    System.out.println("ls.convexHull.area=" + ls.convexHull().area());
    System.out.println("ls.boundingBox=" + ls.boundingBox());
    // orientation
    PolygonShape cw = new PolygonShape(pts(new int[][]{{0,0},{0,10},{10,10},{10,0}}));
    System.out.println("cw corners=" + corners(cw));
    System.out.println("cw.isConvex=" + cw.isConvex());
    // containment
    System.out.println("ls.contains(5,15)=" + ls.contains((Point) new IntPoint(5,15)));
    System.out.println("ls.contains(15,15)=" + ls.contains((Point) new IntPoint(15,15)));
    System.out.println("ls.contains(float 15,5)=" + ls.contains(new FloatPoint(15.0,5.0)));
    System.out.println("ls.isOutside(25,5)=" + ls.isOutside(new IntPoint(25,5)));
    System.out.println("ls.intersects(IntBox(15,15,30,30))=" + ls.intersects(new IntBox(15,15,30,30)));
    System.out.println("sq.intersects(IntBox(5,5,30,30))=" + sq.intersects(new IntBox(5,5,30,30)));
    // polylineshapeops
    System.out.println("sq.borderLineCount=" + sq.borderLineCount());
    System.out.println("sq.circumference=" + sq.circumference());
    System.out.println("sq.centreOfGravity=" + sq.centreOfGravity());
    System.out.println("sq.equalsCorner(10,10)=" + sq.equalsCorner(new IntPoint(10,10)));
    System.out.println("sq.nextNo(3)=" + sq.nextNo(3));
    System.out.println("sq.prevNo(0)=" + sq.prevNo(0));
    // Circle
    Circle c = new Circle(new IntPoint(0,0), 10);
    System.out.println("c.area=" + c.area() + " pi*100=" + (Math.PI*100.0));
    System.out.println("c.boundingBox=" + c.boundingBox());
    System.out.println("c.contains(5,5)=" + c.contains((Point) new IntPoint(5,5)));
    System.out.println("c.contains(8,8)=" + c.contains((Point) new IntPoint(8,8)));
    System.out.println("c.containsOnBorder(10,0)=" + c.containsOnBorder(new IntPoint(10,0)));
    System.out.println("c.distance(13,0)=" + c.distance(new FloatPoint(13.0,0.0)));
    System.out.println("c.smallestRadius=" + c.smallestRadius());
    System.out.println("c.boundingTile=" + c.boundingTile());
    System.out.println("c.boundingTile.contains(7,7)=" + c.boundingTile().contains(new IntPoint(7,7)));
    System.out.println("c.boundingOctagon=" + c.boundingOctagon());
    System.out.println("c.boundingOctagon.isNormalized=" + c.boundingOctagon().isNormalized());
    System.out.println("c.intersects(IntBox(9,9,20,20))=" + c.intersects(new IntBox(9,9,20,20)));
    System.out.println("c.intersects(IntBox(11,11,20,20))=" + c.intersects(new IntBox(11,11,20,20)));
    System.out.println("c.intersects(Circle(15,0,6))=" + c.intersects(new Circle(new IntPoint(15,0),6)));
    System.out.println("c.intersects(Circle(20,0,6))=" + c.intersects(new Circle(new IntPoint(20,0),6)));
    // PolylineArea
    PolygonShape border = new PolygonShape(pts(new int[][]{{0,0},{30,0},{30,30},{0,30}}));
    PolygonShape hole = new PolygonShape(pts(new int[][]{{10,10},{20,10},{20,20},{10,20}}));
    PolylineArea pa = new PolylineArea(border, new PolylineShape[]{hole});
    System.out.println("pa.isBounded=" + pa.isBounded());
    System.out.println("pa.boundingBox=" + pa.boundingBox());
    System.out.println("pa.contains(5,5)=" + pa.contains((Point) new IntPoint(5,5)));
    System.out.println("pa.contains(15,15)=" + pa.contains((Point) new IntPoint(15,15)));
    TileShape[] pp = pa.splitToConvex();
    System.out.println("pa.splitToConvex.len=" + (pp == null ? "null" : pp.length));
    double s2 = 0;
    for (TileShape t : pp) { s2 += t.area(); System.out.println("  pa piece " + t.getClass().getSimpleName() + " area=" + t.area() + " " + t); }
    System.out.println("pa pieces area sum=" + s2);
    for (TileShape t : pp) System.out.println("  pa piece containsInside(15,15)=" + t.containsInside(new IntPoint(15,15)));
    // Shape dispatch
    Circle cc = new Circle(new IntPoint(0,0), 10);
    IntBox bb = new IntBox(5,5,20,20);
    System.out.println("cc.intersects((Shape)bb)=" + cc.intersects((Shape) bb));
    System.out.println("bb.intersects((Shape)cc)=" + bb.intersects((Shape) cc));
    System.out.println("bb.boundingBox=" + bb.boundingBox());
    System.out.println("orth.bounds(cc)=" + OrthogonalBoundingDirections.INSTANCE.bounds(cc).getClass().getSimpleName() + " " + OrthogonalBoundingDirections.INSTANCE.bounds(cc));
  }
}
