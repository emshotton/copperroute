import app.freerouting.geometry.planar.*;
import java.util.*;

/** Independent reviewer differential: PolygonShape over random corner sets. */
public class RD {
  static long s;
  static long nx() { s ^= s << 13; s ^= s >>> 7; s ^= s << 17; return s; }
  static int rnd(int m) { long v = nx(); return (int)(((v >>> 1) % (2L*m+1)) - m); }
  static String pf(FloatPoint p) { return p == null ? "null" : (Double.doubleToRawLongBits(p.x)+"/"+Double.doubleToRawLongBits(p.y)); }
  static String pt(Point p) { return p == null ? "null" : (p instanceof IntPoint ip ? ip.x+","+ip.y : "R"+p); }
  public static void main(String[] a) {
    int n = Integer.parseInt(a[0]);
    s = 88172645463325252L;
    StringBuilder out = new StringBuilder();
    int[] ranges = {6, 12, 40, 400};
    for (int c = 0; c < n; c++) {
      int cornerCount = 3 + (int)((nx() >>> 1) % 8);
      int m = ranges[c % ranges.length];
      Point[] pts = new Point[cornerCount];
      for (int i = 0; i < cornerCount; i++) pts[i] = new IntPoint(rnd(m), rnd(m));
      out.append(c).append(":");
      PolygonShape p;
      try { p = new PolygonShape(pts); } catch (Throwable t) { out.append("CTOR-EXC ").append(t.getClass().getSimpleName()).append("\n"); continue; }
      for (Point q : p.corners) out.append(pt(q)).append(" ");
      out.append("|conv=");
      try { out.append(p.isConvex()); } catch (Throwable t) { out.append("EXC:"+t.getClass().getSimpleName()); }
      out.append("|dim=").append(p.dimension()).append("|area=").append(Double.doubleToRawLongBits(p.area()));
      out.append("|bb=");
      try { IntBox b = p.boundingBox(); out.append(b.ll.x+","+b.ll.y+","+b.ur.x+","+b.ur.y); } catch (Throwable t) { out.append("EXC:"+t.getClass().getSimpleName()); }
      out.append("|oct=");
      try { IntOctagon o = p.boundingOctagon(); out.append(o.leftX+","+o.bottomY+","+o.rightX+","+o.topY+","+o.upperLeftDiagonalX+","+o.lowerRightDiagonalX+","+o.lowerLeftDiagonalX+","+o.upperRightDiagonalX); } catch (Throwable t) { out.append("EXC:"+t.getClass().getSimpleName()); }
      out.append("|hull=");
      try { PolygonShape h = p.convexHull(); for (Point q : h.corners) out.append(pt(q)).append(";"); } catch (Throwable t) { out.append("EXC:"+t.getClass().getSimpleName()); }
      out.append("|split=");
      try {
        TileShape[] sp = p.splitToConvex();
        if (sp == null) out.append("null");
        else { out.append(sp.length).append(":"); for (TileShape t : sp) { out.append(t.getClass().getSimpleName()).append("/").append(t.borderLineCount()).append("/").append(Double.doubleToRawLongBits(t.area())).append("/"); for (int i=0;i<t.borderLineCount();i++) out.append(pf(t.cornerApprox(i))).append(","); out.append(";"); } }
      } catch (Throwable t) { out.append("EXC:"+t.getClass().getSimpleName()); }
      out.append("|cont=");
      try { out.append(p.contains(new IntPoint(0,0))).append(p.isOutside(new IntPoint(1,1))).append(p.containsInside(new IntPoint(-1,2))).append(p.contains(new FloatPoint(0.5,0.5))); } catch (Throwable t) { out.append("EXC:"+t.getClass().getSimpleName()); }
      out.append("|isec=");
      try { out.append(p.intersects(new IntBox(-3,-3,3,3))).append(p.intersects(new IntBox(-3,-3,3,3).toIntOctagon())).append(p.intersects(new IntBox(-3,-3,3,3).toSimplex())).append(p.intersects(new Circle(new IntPoint(0,0),4))); } catch (Throwable t) { out.append("EXC:"+t.getClass().getSimpleName()); }
      out.append("|bt=");
      try { TileShape bt = p.boundingTile(); out.append(bt.getClass().getSimpleName()).append("/").append(bt.borderLineCount()); } catch (Throwable t) { out.append("EXC:"+t.getClass().getSimpleName()); }
      out.append("|npa=");
      try { out.append(pf(p.nearestPointApprox(new FloatPoint(7.5,-3.25)))); } catch (Throwable t) { out.append("EXC:"+t.getClass().getSimpleName()); }
      out.append("\n");
    }
    System.out.print(out);
  }
}
