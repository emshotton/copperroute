package app.freerouting.geometry.planar;
public class E15 {
  static String ipArr(IntPoint[] a) {
    if (a == null) return "null";
    StringBuilder b = new StringBuilder("[" + a.length + ":");
    for (IntPoint s : a) b.append(s == null ? "null" : (s.x + "/" + s.y)).append(";");
    return b.append("]").toString();
  }
  static String iArr(int[] a) { StringBuilder b=new StringBuilder("["+a.length+":"); for(int s:a) b.append(s).append(";"); return b.append("]").toString(); }
  interface Th { String get() throws Throwable; }
  static void e(String n, Th t){ String v; try{v=t.get();}catch(Throwable x){v="EXC";} System.out.println(n+"="+v); }
  static LineSegment seg(int ax,int ay,int bx,int by){
    IntPoint a=new IntPoint(ax,ay), b=new IntPoint(bx,by);
    Line m=new Line(a,b); Direction p=m.direction().turn45Degree(2);
    return new LineSegment(new Line(a,p), m, new Line(b,p));
  }
  public static void main(String[] a){
    final LineSegment s = seg(0,0,20,7);
    e("negW.stair", () -> ipArr(s.stairApproximation(-2.0, true)));
    e("negW.stair45", () -> ipArr(s.stairApproximation45(-2.0, true)));
    e("negW.stairSmall", () -> ipArr(s.stairApproximation(-0.1, true)));
    e("zeroW.stair", () -> ipArr(s.stairApproximation(0.0, true)));
    e("hugeW.stair", () -> ipArr(s.stairApproximation(1e18, true)));
    e("nanW.stair", () -> ipArr(s.stairApproximation(Double.NaN, true)));
    e("infW.stair", () -> ipArr(s.stairApproximation(Double.POSITIVE_INFINITY, true)));
    final TileShape es = Simplex.EMPTY;
    e("emptySimplex.border", () -> iArr(s.borderIntersections(es)));
    e("emptySimplex.interior", () -> "" + es.isIntersectedInteriorBy(s));
    final TileShape eb = new IntBox(5,5,0,0);
    e("emptyBox.border", () -> iArr(s.borderIntersections(eb)));
    e("emptyBox.interior", () -> "" + eb.isIntersectedInteriorBy(s));
    final TileShape hp = TileShape.getInstance(new Line(new IntPoint(0,0), new IntPoint(0,1)));
    e("halfplane.border", () -> iArr(s.borderIntersections(hp)));
    e("halfplane.interior", () -> "" + hp.isIntersectedInteriorBy(s));
    e("negLen.changeLen", () -> { LineSegment t=s.changeLengthApprox(-5.0); Line l=t.getEndClosingLine(); IntPoint la=(IntPoint)l.a, lb=(IntPoint)l.b; return la.x+"/"+la.y+";"+lb.x+"/"+lb.y; });
    e("negLen.endPoint", () -> { LineSegment t=s.changeLengthApprox(-5.0); Point q=t.endPoint(); return q instanceof IntPoint ip ? ("I"+ip.x+"/"+ip.y) : ("R"+((RationalPoint)q).x+"/"+((RationalPoint)q).y+"/"+((RationalPoint)q).z); });
    e("empty.fromShape0", () -> { LineSegment t=new LineSegment(es, 0); return t.getLine()==null?"SEGnull":"SEG"; });
  }
}
