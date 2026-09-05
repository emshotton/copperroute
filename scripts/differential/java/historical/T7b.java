package app.freerouting.geometry.planar;

public class T7b {
  static String v(Vector x) { IntVector iv = (IntVector) x; return "(" + iv.x + "," + iv.y + ")"; }
  static String b(IntBox x) { return "[" + x.ll.x + "," + x.ll.y + " .. " + x.ur.x + "," + x.ur.y + "]"; }

  public static void main(String[] args) {
    Vector translation = new IntVector(100, 200);
    Point relLocation = Point.ZERO.translateBy(translation);
    System.out.println("turn90(1, pole(50,50))  -> " + v(relLocation.turn90Degree(1, new IntPoint(50, 50)).differenceBy(Point.ZERO)));
    System.out.println("turn90(3, pole(50,50))  -> " + v(relLocation.turn90Degree(3, new IntPoint(50, 50)).differenceBy(Point.ZERO)));
    FloatPoint nt = translation.toFloat().rotate(Math.toRadians(30.0), new FloatPoint(50, 50));
    System.out.println("rotateApprox(30, pole(50,50)) float=" + nt + " -> " + v(nt.round().differenceBy(Point.ZERO)));
    System.out.println("mirrorVertical(pole(50,50)) -> " + v(relLocation.mirrorVertical(new IntPoint(50, 50)).differenceBy(Point.ZERO)));

    // rotationInDegree wrap-around (ObstacleArea.java:214-221)
    double rot = 300;
    rot += 2 * 90;
    while (rot >= 360) rot -= 360;
    while (rot < 0) rot += 360;
    System.out.println("300 + 2*90 wrapped = " + rot);
    rot = 30; rot += -2 * 90;
    while (rot >= 360) rot -= 360;
    while (rot < 0) rot += 360;
    System.out.println("30 + (-2)*90 wrapped = " + rot);

    // rotateApprox turnAngle for sideChanged && rotateFirst (ObstacleArea.java:229-232)
    double angle = 30.0;
    System.out.println("turnAngle when sideChanged&&rotateFirst = " + (360 - angle));

    // L-shape bbox
    Point[] lCorners = new Point[] {
      new IntPoint(0, 0), new IntPoint(20, 0), new IntPoint(20, 10),
      new IntPoint(10, 10), new IntPoint(10, 20), new IntPoint(0, 20)};
    PolygonShape l = new PolygonShape(new Polygon(lCorners));
    System.out.println("L bbox = " + b(l.boundingBox()));
    System.out.println("L translated bbox = " + b(((Area) l).translateBy(translation).boundingBox()));

    // non-symmetric L for the flip test: an F-ish polygon
    Point[] f = new Point[] {
      new IntPoint(0, 0), new IntPoint(30, 0), new IntPoint(30, 10), new IntPoint(10, 10),
      new IntPoint(10, 20), new IntPoint(0, 20)};
    PolygonShape fs = new PolygonShape(new Polygon(f));
    System.out.println("F bbox = " + b(fs.boundingBox()));
    TileShape[] fp = fs.splitToConvex();
    System.out.println("F splitToConvex count=" + fp.length);
    for (int i = 0; i < fp.length; i++) System.out.println("  [" + i + "] " + b(fp[i].boundingBox()));
    Area fmv = ((Area) fs).mirrorVertical(Point.ZERO);
    System.out.println("F mirrorVertical(ZERO) bbox = " + b(fmv.boundingBox()));
    TileShape[] fmp = fmv.splitToConvex();
    System.out.println("F mirrored splitToConvex count=" + fmp.length);
    for (int i = 0; i < fmp.length; i++) System.out.println("  [" + i + "] " + b(fmp[i].boundingBox()));

    // BoardOutline: an empty shapes array
    IntBox boardBox = new IntBox(new IntPoint(0, 0), new IntPoint(1000, 1000));
    PolylineArea empty = new PolylineArea(boardBox, new PolylineShape[0]);
    TileShape[] et = empty.splitToConvex();
    System.out.println("keepout with no outline shapes: count=" + (et == null ? -1 : et.length));
    for (int i = 0; et != null && i < et.length; i++) System.out.println("  [" + i + "] " + b(et[i].boundingBox()));

    // BoardOutline.boundingBox over an empty shapes array (BoardOutline.java:88-95)
    IntBox r = IntBox.EMPTY;
    System.out.println("IntBox.EMPTY = " + b(r) + " union with (100,100..900,900) = "
        + b(r.union(new IntBox(new IntPoint(100,100), new IntPoint(900,900)))));
  }
}
