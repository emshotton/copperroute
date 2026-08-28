package app.freerouting.geometry.planar;

public class T17b {
  static Point[] pts(int[][] v) {
    Point[] r = new Point[v.length];
    for (int i = 0; i < v.length; i++) r[i] = new IntPoint(v[i][0], v[i][1]);
    return r;
  }
  public static void main(String[] a) {
    PolygonShape p1 = new PolygonShape(pts(new int[][]{{0,0},{10,0},{10,10},{0,10}}));
    PolygonShape p2 = new PolygonShape(pts(new int[][]{{5,5},{20,5},{20,20},{5,20}}));
    IntBox box = new IntBox(5,5,20,20);
    Circle c = new Circle(new IntPoint(0,0), 10);
    Simplex sx = box.toSimplex();
    IntOctagon oc = box.toIntOctagon();
    System.out.println("p1.intersects((Shape)box)=" + p1.intersects((Shape) box));
    System.out.println("box.intersects((Shape)p1)=" + box.intersects((Shape) p1));
    System.out.println("p1.intersects((Shape)c)=" + p1.intersects((Shape) c));
    System.out.println("c.intersects((Shape)p1)=" + c.intersects((Shape) p1));
    System.out.println("p1.intersects((Shape)oc)=" + p1.intersects((Shape) oc));
    System.out.println("p1.intersects((Shape)sx)=" + p1.intersects((Shape) sx));
    System.out.println("p1.intersects(box)=" + p1.intersects(box));
    try {
      System.out.println("p1.intersects((Shape)p2)=" + p1.intersects((Shape) p2));
    } catch (StackOverflowError e) {
      System.out.println("p1.intersects((Shape)p2) -> StackOverflowError");
    }
    // getBorder / getHoles
    System.out.println("p1.getBorder() == p1 ? " + (p1.getBorder() == p1));
    System.out.println("p1.getHoles().length=" + p1.getHoles().length);
    // isContainedIn
    System.out.println("p1.isContainedIn(box)=" + p1.isContainedIn(box));
    System.out.println("p1.isContainedIn(new IntBox(-1,-1,11,11))=" + p1.isContainedIn(new IntBox(-1,-1,11,11)));
    // nearestPointApprox
    System.out.println("p1.nearestPointApprox(20,5)=" + p1.nearestPointApprox(new FloatPoint(20.0,5.0)));
    // boundingOctagon of square
    System.out.println("p1.boundingOctagon()=" + p1.boundingOctagon());
    System.out.println("p1.boundingTile()=" + p1.boundingTile() + " " + p1.boundingTile().getClass().getSimpleName());
    System.out.println("p1.borderLine(0)=" + p1.borderLine(0) + " borderLine(3)=" + p1.borderLine(3));
    System.out.println("p1.borderLine(4)=" + p1.borderLine(4));
    // triangle contains
    PolygonShape tri = new PolygonShape(pts(new int[][]{{0,0},{10,0},{10,10}}));
    System.out.println("tri corners=" + java.util.Arrays.toString(tri.corners));
    System.out.println("tri.contains(8,4)=" + tri.contains((Point) new IntPoint(8,4)));
    System.out.println("tri.contains(2,8)=" + tri.contains((Point) new IntPoint(2,8)));
    // circle bounding tile with max segment
    Circle big = new Circle(new IntPoint(0,0), 1000);
    System.out.println("big.boundingTile(1000)=" + big.boundingTile(1000));
    TileShape bt = big.boundingTile(100);
    System.out.println("big.boundingTile(100) class=" + bt.getClass().getSimpleName() + " lines=" + bt.borderLineCount() + " area=" + bt.area());
    System.out.println("bt.contains(999,0)=" + bt.contains(new IntPoint(999,0)) + " contains(1100,0)=" + bt.contains(new IntPoint(1100,0)));
    // Circle toString
    System.out.println("[" + new Circle(new IntPoint(0,0), 1000) + "]");
    System.out.println("[" + new Circle(new IntPoint(2,3), 7) + "]");
    // Circle offsets
    Circle cc = new Circle(new IntPoint(0,0), 10);
    System.out.println("offset(2.4)=" + cc.offset(2.4).radius + " shrink(2.4)=" + cc.shrink(2.4).radius + " shrink(100)=" + cc.shrink(100).radius + " enlarge(2.5)=" + cc.enlarge(2.5).radius);
    // PolylineShape members on IntBox
    IntBox ub = new IntBox(0,0,10,10);
    System.out.println("ub.equalsCorner((10,10))=" + ub.equalsCorner(new IntPoint(10,10)));
    System.out.println("ub.polarLineSegment(-10,5)=" + ub.polarLineSegment(new FloatPoint(-10.0,5.0)));
    System.out.println("ub.intersects(Line(5,-5,5,15))=" + ub.intersects(new Line(5,-5,5,15)));
    System.out.println("ub.intersects(Line(20,-5,20,15))=" + ub.intersects(new Line(20,-5,20,15)));
    System.out.println("ub.boundedCorners().length=" + ub.boundedCorners().length);
    // PolygonShape transformations
    System.out.println("p1.turn90(4)=" + java.util.Arrays.toString(p1.turn90Degree(4, new IntPoint(0,0)).corners));
    System.out.println("p1.translateBy((5,5))=" + p1.translateBy(new IntVector(5,5)).boundingBox());
  }
}
