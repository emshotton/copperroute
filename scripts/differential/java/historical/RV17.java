import app.freerouting.geometry.planar.*;
import java.util.*;

public class RV17 {
  static Point[] pts(int[][] v) {
    Point[] r = new Point[v.length];
    for (int i = 0; i < v.length; i++) r[i] = new IntPoint(v[i][0], v[i][1]);
    return r;
  }
  public static void main(String[] a) {
    // --- java.util.Random pinning
    int[] bounds = {2,3,4,5,6,7,8,16,100};
    for (int b : bounds) {
      Random r = new Random(99);
      StringBuilder sb = new StringBuilder("RNG bound " + b + ":");
      for (int i = 0; i < 12; i++) sb.append(" ").append(r.nextInt(b));
      System.out.println(sb);
    }
    PolygonShape sq = new PolygonShape(pts(new int[][]{{0,0},{10,0},{10,10},{0,10}}));
    PolygonShape cw = new PolygonShape(pts(new int[][]{{0,0},{0,10},{10,10},{10,0}}));
    PolygonShape L  = new PolygonShape(pts(new int[][]{{0,0},{20,0},{20,10},{10,10},{10,20},{0,20}}));
    System.out.println("sq corners " + Arrays.toString(sq.corners));
    System.out.println("cw corners " + Arrays.toString(cw.corners));
    System.out.println("sq.isConvex " + sq.isConvex() + " area " + sq.area());
    System.out.println("sq.split len " + sq.splitToConvex().length);
    System.out.println("L.isConvex " + L.isConvex() + " area " + L.area());
    TileShape[] lp = L.splitToConvex();
    System.out.println("L.split len " + lp.length);
    double sum = 0; for (TileShape t : lp) sum += t.area();
    System.out.println("L.split area sum " + sum + " classes " + lp[0].getClass().getSimpleName()+","+lp[1].getClass().getSimpleName());
    System.out.println("L.hull corners " + Arrays.toString(L.convexHull().corners) + " area " + L.convexHull().area());
    System.out.println("L.bbox " + L.boundingBox());
    System.out.println("L.contains(5,15) " + L.contains(new IntPoint(5,15)));
    System.out.println("L.contains(15,15) " + L.contains(new IntPoint(15,15)));
    System.out.println("L.containsFloat(15,5) " + L.contains(new FloatPoint(15,5)));
    System.out.println("L.isOutside(25,5) " + L.isOutside(new IntPoint(25,5)));
    System.out.println("L.intersects box(15,15,30,30) " + L.intersects(new IntBox(15,15,30,30)));
    System.out.println("sq.intersects box(5,5,30,30) " + sq.intersects(new IntBox(5,5,30,30)));
    System.out.println("sq.circumference " + sq.circumference() + " cog " + sq.centreOfGravity());
    System.out.println("sq.equalsCorner(10,10) " + sq.equalsCorner(new IntPoint(10,10)));
    System.out.println("sq.nextNo(3) " + sq.nextNo(3) + " prevNo(0) " + sq.prevNo(0));
    System.out.println("sq.borderLine(0) " + sq.borderLine(0) + " borderLine(3) " + sq.borderLine(3) + " borderLine(4) " + sq.borderLine(4));
    System.out.println("sq.boundingOctagon " + sq.boundingOctagon());
    System.out.println("sq.nearestPointApprox(20,5) " + sq.nearestPointApprox(new FloatPoint(20,5)));
    System.out.println("sq.smallestRadius " + sq.smallestRadius() + " enlarge0 " + (sq.enlarge(0)==sq) + " enlarge1 " + sq.enlarge(1));
    System.out.println("sq.translate(5,5).bbox " + sq.translateBy(new IntVector(5,5)).boundingBox());
    // --- Circle
    Circle c = new Circle(new IntPoint(0,0), 10);
    System.out.println("C.area " + c.area() + " bbox " + c.boundingBox() + " oct " + c.boundingOctagon());
    System.out.println("C.contains(5,5) " + c.contains(new IntPoint(5,5)) + " (8,8) " + c.contains(new IntPoint(8,8)));
    System.out.println("C.onBorder(10,0) " + c.containsOnBorder(new IntPoint(10,0)) + " dist(13,0) " + c.distance(new FloatPoint(13,0)));
    System.out.println("C.intersects box(9,9,20,20) " + c.intersects(new IntBox(9,9,20,20)) + " box(11,11,20,20) " + c.intersects(new IntBox(11,11,20,20)));
    System.out.println("C.intersects circle(15,0,6) " + c.intersects(new Circle(new IntPoint(15,0),6)) + " (20,0,6) " + c.intersects(new Circle(new IntPoint(20,0),6)));
    System.out.println("C.toString " + c + " | " + new Circle(new IntPoint(0,0),1000) + " | " + new Circle(new IntPoint(2,3),7));
    System.out.println("C.offset(2.4) " + c.offset(2.4).radius + " shrink(2.4) " + c.shrink(2.4).radius + " shrink(100) " + c.shrink(100).radius + " enlarge(2.5) " + c.enlarge(2.5).radius);
    Circle c2 = new Circle(new IntPoint(3,4),5);
    System.out.println("C2.containedIn(-2,-1,8,9) " + c2.isContainedIn(new IntBox(-2,-1,8,9)) + " (-1,-1,8,9) " + c2.isContainedIn(new IntBox(-1,-1,8,9)));
    Circle c1000 = new Circle(new IntPoint(0,0),1000);
    System.out.println("C1000.boundingTile(1000) is oct " + (c1000.boundingTile(1000) instanceof IntOctagon));
    TileShape bt = c1000.boundingTile(100);
    System.out.println("C1000.boundingTile(100) class " + bt.getClass().getSimpleName() + " area " + bt.area() + " contains(999,0) " + bt.contains(new IntPoint(999,0)) + " contains(1100,0) " + bt.contains(new IntPoint(1100,0)));
    // --- PolylineArea
    PolygonShape border = new PolygonShape(pts(new int[][]{{0,0},{30,0},{30,30},{0,30}}));
    PolygonShape hole = new PolygonShape(pts(new int[][]{{10,10},{20,10},{20,20},{10,20}}));
    PolylineArea pa = new PolylineArea(border, new PolylineShape[]{hole});
    TileShape[] pieces = pa.splitToConvex();
    double s2 = 0; for (TileShape t : pieces) s2 += t.area();
    System.out.println("PA pieces " + pieces.length + " area " + s2 + " bbox " + pa.boundingBox() + " contains(5,5) " + pa.contains(new IntPoint(5,5)) + " contains(15,15) " + pa.contains(new IntPoint(15,15)));
    System.out.println("PA cornerApproxArr len " + pa.cornerApproxArr().length);
    PolylineArea pab = new PolylineArea(new IntBox(0,0,30,30), new PolylineShape[]{new IntBox(10,10,20,20)});
    TileShape[] pb = pab.splitToConvex(); double s3=0; for (TileShape t: pb) s3+=t.area();
    System.out.println("PAbox pieces " + pb.length + " area " + s3 + " nearest(15,15) " + pab.nearestPointApprox(new FloatPoint(15,15)));
    // --- shape dispatch
    Shape sc = new Circle(new IntPoint(0,0),10);
    Shape sb = new IntBox(5,5,20,20);
    System.out.println("circle.intersects(box) " + sc.intersects(sb) + " box.intersects(circle) " + sb.intersects(sc));
    Shape sp = sq;
    IntOctagon oct = new IntBox(5,5,20,20).toIntOctagon();
    Simplex sx = new IntBox(5,5,20,20).toSimplex();
    System.out.println("poly.intersects box " + sp.intersects(sb) + " box.intersects poly " + sb.intersects(sp)
      + " poly.oct " + sq.intersects(oct) + " poly.simplex " + sq.intersects(sx)
      + " poly.circle " + sq.intersects(new Circle(new IntPoint(0,0),10)) + " circle.poly " + sc.intersects(sp));
    // polygon x polygon
    try {
      PolygonShape p2 = new PolygonShape(pts(new int[][]{{5,5},{20,5},{20,20},{5,20}}));
      Shape sp2 = p2;
      System.out.println("poly x poly " + sp.intersects(sp2));
    } catch (StackOverflowError e) { System.out.println("poly x poly -> StackOverflowError"); }
  }
}
