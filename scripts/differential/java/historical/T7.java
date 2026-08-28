package app.freerouting.geometry.planar;

/**
 * Plan 2 Task 7 driver: reproduces `ObstacleArea.getArea()` (ObstacleArea.java:119-144),
 * `ObstacleArea.splitToConvex()` (:320-326) and `BoardOutline.getKeepoutArea()`
 * (BoardOutline.java:183-189) over the real geometry/planar classes.
 */
public class T7 {

  static void dumpTiles(String label, TileShape[] tiles) {
    if (tiles == null) {
      System.out.println(label + ": null");
      return;
    }
    System.out.println(label + ": count=" + tiles.length);
    for (int i = 0; i < tiles.length; i++) {
      IntBox b = tiles[i].boundingBox();
      System.out.println(
          "  ["
              + i
              + "] class="
              + tiles[i].getClass().getSimpleName()
              + " bbox=["
              + b.ll.x
              + "," + b.ll.y + " .. " + b.ur.x + "," + b.ur.y + "]"
              + " dim=" + tiles[i].dimension()
              + " corners=" + cornersOf(tiles[i]));
    }
  }

  static String cornersOf(PolylineShape s) {
    StringBuilder sb = new StringBuilder("{");
    int n = s.borderLineCount();
    for (int i = 0; i < n; i++) {
      if (i > 0) sb.append(" ");
      FloatPoint c = s.cornerApprox(i);
      sb.append("(").append((long) c.x).append(",").append((long) c.y).append(")");
    }
    return sb.append("}").toString();
  }

  /** ObstacleArea.getArea (ObstacleArea.java:119-144), inlined. */
  static Area getArea(
      Area relativeArea,
      Vector translation,
      double rotationInDegree,
      boolean sideChanged,
      boolean flipStyleRotateFirst) {
    Area turnedArea = relativeArea;
    if (sideChanged && !flipStyleRotateFirst) {
      turnedArea = turnedArea.mirrorVertical(Point.ZERO);
    }
    if (rotationInDegree != 0) {
      double rotation = rotationInDegree;
      if (rotation % 90 == 0) {
        turnedArea = turnedArea.turn90Degree(((int) rotation) / 90, Point.ZERO);
      } else {
        turnedArea = turnedArea.rotateApprox(Math.toRadians(rotation), FloatPoint.ZERO);
      }
    }
    if (sideChanged && flipStyleRotateFirst) {
      turnedArea = turnedArea.mirrorVertical(Point.ZERO);
    }
    return turnedArea.translateBy(translation);
  }

  public static void main(String[] args) {
    // ---- 1. the L-shaped relative area --------------------------------------------------
    Point[] lCorners =
        new Point[] {
          new IntPoint(0, 0),
          new IntPoint(20, 0),
          new IntPoint(20, 10),
          new IntPoint(10, 10),
          new IntPoint(10, 20),
          new IntPoint(0, 20)
        };
    PolygonShape lShape = new PolygonShape(new Polygon(lCorners));
    System.out.println("L corners after normalisation: " + cornersOf(lShape));
    System.out.println("L bbox: " + lShape.boundingBox());
    dumpTiles("L splitToConvex", lShape.splitToConvex());

    Area lArea = lShape;
    Vector translation = new IntVector(100, 200);

    Area a0 = getArea(lArea, translation, 0.0, false, false);
    System.out.println("\n-- getArea(t=(100,200), rot=0, sideChanged=false)");
    System.out.println("bbox=" + a0.boundingBox());
    dumpTiles("splitToConvex", a0.splitToConvex());

    Area a90 = getArea(lArea, translation, 90.0, false, false);
    System.out.println("\n-- getArea(t=(100,200), rot=90, sideChanged=false)");
    System.out.println("bbox=" + a90.boundingBox());
    dumpTiles("splitToConvex", a90.splitToConvex());

    Area aFlip = getArea(lArea, translation, 90.0, true, false);
    System.out.println("\n-- getArea(t=(100,200), rot=90, sideChanged=true, rotateFirst=false)");
    System.out.println("bbox=" + aFlip.boundingBox());
    dumpTiles("splitToConvex", aFlip.splitToConvex());

    Area aFlipRotFirst = getArea(lArea, translation, 90.0, true, true);
    System.out.println("\n-- getArea(t=(100,200), rot=90, sideChanged=true, rotateFirst=true)");
    System.out.println("bbox=" + aFlipRotFirst.boundingBox());
    dumpTiles("splitToConvex", aFlipRotFirst.splitToConvex());

    Area a45 = getArea(lArea, translation, 45.0, false, false);
    System.out.println("\n-- getArea(t=(100,200), rot=45, sideChanged=false)");
    System.out.println("bbox=" + a45.boundingBox());
    dumpTiles("splitToConvex", a45.splitToConvex());

    // ---- 2. turn90Degree / rotateApprox on the translation vector -----------------------
    System.out.println("\n-- ObstacleArea.turn90Degree(1, pole=(50,50)) on translation (100,200)");
    Point relLocation = Point.ZERO.translateBy(translation);
    Vector t90 = relLocation.turn90Degree(1, new IntPoint(50, 50)).differenceBy(Point.ZERO);
    System.out.println("translation after = " + t90);

    System.out.println("\n-- ObstacleArea.rotateApprox(30, pole=(50,50)) on translation (100,200)");
    FloatPoint newTranslation =
        translation.toFloat().rotate(Math.toRadians(30.0), new FloatPoint(50, 50));
    Vector t30 = newTranslation.round().differenceBy(Point.ZERO);
    System.out.println("float = " + newTranslation + "  rounded translation = " + t30);

    System.out.println("\n-- ObstacleArea.changePlacementSide(pole=(50,50)) on translation (100,200)");
    Vector tm = relLocation.mirrorVertical(new IntPoint(50, 50)).differenceBy(Point.ZERO);
    System.out.println("translation after = " + tm);

    // ---- 3. BoardOutline.getKeepoutArea -------------------------------------------------
    IntBox boardBox = new IntBox(new IntPoint(0, 0), new IntPoint(1000, 1000));
    PolylineShape outline =
        new PolygonShape(
            new Polygon(
                new Point[] {
                  new IntPoint(100, 100),
                  new IntPoint(900, 100),
                  new IntPoint(900, 900),
                  new IntPoint(100, 900)
                }));
    PolylineArea keepout = new PolylineArea(boardBox, new PolylineShape[] {outline});
    System.out.println("\n-- BoardOutline.getKeepoutArea (board box 0,0..1000,1000; outline 100,100..900,900)");
    System.out.println("keepout bbox=" + keepout.boundingBox());
    System.out.println("keepout dimension=" + keepout.dimension());
    dumpTiles("keepout splitToConvex", keepout.splitToConvex());
    System.out.println("outline borderLineCount=" + outline.borderLineCount());

    // the same, with the outline expressed as an IntBox (the other PolylineShape subclass)
    PolylineArea keepoutBox =
        new PolylineArea(
            boardBox,
            new PolylineShape[] {new IntBox(new IntPoint(100, 100), new IntPoint(900, 900))});
    System.out.println("\n-- same, outline as IntBox");
    dumpTiles("keepout splitToConvex", keepoutBox.splitToConvex());
    System.out.println("IntBox borderLineCount=" + new IntBox(new IntPoint(100,100), new IntPoint(900,900)).borderLineCount());

    // two outline shapes
    PolylineShape outline2 =
        new PolygonShape(
            new Polygon(
                new Point[] {
                  new IntPoint(100, 100),
                  new IntPoint(400, 100),
                  new IntPoint(400, 400),
                  new IntPoint(100, 400)
                }));
    PolylineShape outline3 =
        new PolygonShape(
            new Polygon(
                new Point[] {
                  new IntPoint(600, 600),
                  new IntPoint(900, 600),
                  new IntPoint(900, 900),
                  new IntPoint(600, 900)
                }));
    PolylineArea keepout2 =
        new PolylineArea(boardBox, new PolylineShape[] {outline2, outline3});
    System.out.println("\n-- keepout with two outline polygons");
    dumpTiles("keepout splitToConvex", keepout2.splitToConvex());
    System.out.println("lineCount = " + (outline2.borderLineCount() + outline3.borderLineCount()));
  }
}
