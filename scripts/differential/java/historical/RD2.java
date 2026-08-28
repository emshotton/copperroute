import app.freerouting.geometry.planar.*;
import java.util.*;
public class RD2 {
  static long s;
  static long nx() { s ^= s << 13; s ^= s >>> 7; s ^= s << 17; return s; }
  static int rnd(int m) { long v = nx(); return (int)(((v >>> 1) % (2L*m+1)) - m); }
  static String pf(FloatPoint p) { return p == null ? "null" : (Double.doubleToRawLongBits(p.x)+"/"+Double.doubleToRawLongBits(p.y)); }
  static PolygonShape poly(int m) {
    int cc = 3 + (int)((nx() >>> 1) % 6);
    Point[] pts = new Point[cc];
    for (int i = 0; i < cc; i++) pts[i] = new IntPoint(rnd(m), rnd(m));
    return new PolygonShape(pts);
  }
  static void app(StringBuilder o, String tag, java.util.function.Supplier<String> f) {
    o.append("|").append(tag).append("=");
    try { o.append(f.get()); } catch (Throwable t) { o.append("EXC"); }
  }
  public static void main(String[] a) {
    int n = Integer.parseInt(a[0]);
    s = 1234567891011L;
    StringBuilder o = new StringBuilder();
    int[] R = {8, 20, 60};
    for (int c = 0; c < n; c++) {
      int m = R[c % R.length];
      o.append(c);
      // ---- Circle
      Circle ci = new Circle(new IntPoint(rnd(m), rnd(m)), rnd(m));
      app(o, "cr", () -> ci.radius+"");
      app(o, "ca", () -> Double.doubleToRawLongBits(ci.area())+"/"+Double.doubleToRawLongBits(ci.circumference())+"/"+ci.dimension());
      app(o, "cb", () -> { IntBox b = ci.boundingBox(); return b.ll.x+","+b.ll.y+","+b.ur.x+","+b.ur.y; });
      app(o, "co", () -> { IntOctagon q = ci.boundingOctagon(); return q.leftX+","+q.bottomY+","+q.rightX+","+q.topY+","+q.upperLeftDiagonalX+","+q.lowerRightDiagonalX+","+q.lowerLeftDiagonalX+","+q.upperRightDiagonalX+"/"+q.isNormalized(); });
      app(o, "cd", () -> Double.doubleToRawLongBits(ci.distance(new FloatPoint(1.5,-2.5)))+"/"+Double.doubleToRawLongBits(ci.borderDistance(new FloatPoint(1.5,-2.5)))+"/"+Double.doubleToRawLongBits(ci.smallestRadius()));
      app(o, "cc", () -> ""+ci.contains(new IntPoint(1,1))+ci.containsInside(new IntPoint(1,1))+ci.containsOnBorder(new IntPoint(1,1))+ci.contains(new FloatPoint(0.5,0.5))+ci.isOutside(new IntPoint(2,-3)));
      app(o, "ci", () -> ""+ci.intersects(new IntBox(-3,-3,3,3))+ci.intersects(new IntBox(-3,-3,3,3).toIntOctagon())+ci.intersects(new IntBox(-3,-3,3,3).toSimplex())+ci.intersects(new Circle(new IntPoint(2,2),3))+ci.isContainedIn(new IntBox(-30,-30,30,30)));
      app(o, "cs", () -> ci.offset(1.6).radius+"/"+ci.shrink(1.6).radius+"/"+ci.enlarge(1.6).radius+"/"+Double.doubleToRawLongBits(ci.maxWidth()));
      app(o, "ct", () -> { TileShape t = ci.boundingTile(Math.max(1, Math.abs(rnd(m))+1)); return t.getClass().getSimpleName()+"/"+t.borderLineCount(); });
      app(o, "cS", () -> ci.toString());
      // ---- PolylineArea
      PolygonShape bd = poly(m);
      int hc = (int)((nx() >>> 1) % 3);
      PolylineShape[] holes = new PolylineShape[hc];
      for (int i = 0; i < hc; i++) holes[i] = poly(m/2 == 0 ? 1 : m/2);
      PolylineArea pa = new PolylineArea(bd, holes);
      app(o, "ad", () -> pa.dimension()+"/"+pa.isBounded()+"/"+pa.isEmpty()+"/"+pa.isContainedIn(new IntBox(-100,-100,100,100)));
      app(o, "ab", () -> { IntBox b = pa.boundingBox(); return b.ll.x+","+b.ll.y+","+b.ur.x+","+b.ur.y; });
      app(o, "ao", () -> { IntOctagon q = pa.boundingOctagon(); return q.leftX+","+q.bottomY+","+q.rightX+","+q.topY; });
      app(o, "ac", () -> ""+pa.contains(new IntPoint(0,0))+pa.contains(new FloatPoint(0.25,0.25)));
      app(o, "as", () -> { TileShape[] sp = pa.splitToConvex(); if (sp == null) return "null"; StringBuilder q = new StringBuilder(sp.length+":"); for (TileShape t : sp) { q.append(t.getClass().getSimpleName()).append("/").append(t.borderLineCount()).append("/").append(Double.doubleToRawLongBits(t.area())).append("/"); for (int i=0;i<t.borderLineCount();i++) q.append(pf(t.cornerApprox(i))).append(","); q.append(";"); } return q.toString(); });
      app(o, "an", () -> pf(pa.nearestPointApprox(new FloatPoint(3.5,4.5))));
      app(o, "aa", () -> { FloatPoint[] r = pa.cornerApproxArr(); StringBuilder q = new StringBuilder(); for (FloatPoint p : r) q.append(pf(p)).append(","); return q.toString(); });
      app(o, "at", () -> { PolylineArea t = pa.turn90Degree(1, new IntPoint(1,2)).mirrorVertical(new IntPoint(0,1)).translateBy(new IntVector(3,-1)); IntBox b = t.boundingBox(); return b.ll.x+","+b.ll.y+","+b.ur.x+","+b.ur.y; });
      o.append("\n");
    }
    System.out.print(o);
  }
}
