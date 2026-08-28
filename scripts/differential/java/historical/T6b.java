package app.freerouting.geometry.planar;

import java.util.*;

/** Task 6 scratch harness B: Pin.nearestTraceExitCorner / calcNearestExitRestrictionDirection. */
public class T6b {

  record Restr(Direction direction, double minLength) {}

  static List<Restr> restrictions(Direction... ds) {
    List<Restr> r = new LinkedList<>();
    for (Direction d : ds) r.add(new Restr(d, 0));
    return r;
  }

  // Pin.nearestTraceExitCorner (Pin.java:634-673), inlined.
  static FloatPoint nearestTraceExitCorner(
      List<Restr> traceExitRestrictions,
      Shape pinShape,
      Point pinCenter,
      double edgeToTurnDist,
      int traceHalfWidth,
      FloatPoint fromPoint) {
    if (traceExitRestrictions.isEmpty()) return null;
    if (!(pinShape instanceof TileShape)) return null;
    if (edgeToTurnDist < 0) return null;
    TileShape offsetPinShape = (TileShape) ((TileShape) pinShape).offset(edgeToTurnDist + traceHalfWidth);
    double minExitCornerDistance = Double.MAX_VALUE;
    FloatPoint nearestExitCorner = null;
    for (Restr r : traceExitRestrictions) {
      int no = offsetPinShape.intersectingBorderLineNo(pinCenter, r.direction());
      Line ray = new Line(pinCenter, r.direction());
      FloatPoint corner = ray.intersectionApprox(offsetPinShape.borderLine(no));
      double d = corner.distanceSquare(fromPoint);
      if (d < minExitCornerDistance) {
        minExitCornerDistance = d;
        nearestExitCorner = corner;
      }
    }
    return nearestExitCorner;
  }

  // Pin.calcNearestExitRestrictionDirection (Pin.java:565-632), inlined.
  static Direction calcNearest(
      List<Restr> traceExitRestrictions,
      Shape pinShape,
      Point pinCenter,
      double edgeToTurnDist,
      int traceHalfWidth,
      Polyline tracePolyline) {
    if (traceExitRestrictions.isEmpty()) return null;
    if (!(pinShape instanceof TileShape)) return null;
    if (edgeToTurnDist < 0) return null;
    TileShape offsetPinShape = (TileShape) ((TileShape) pinShape).offset(edgeToTurnDist + traceHalfWidth);
    int[][] entries = offsetPinShape.entrancePoints(tracePolyline);
    System.out.println("  entries=" + entries.length);
    if (entries.length == 0) return null;
    int[] latest = entries[entries.length - 1];
    FloatPoint traceEntry = tracePolyline.lines[latest[0]].intersectionApprox(offsetPinShape.borderLine(latest[1]));
    System.out.println("  traceEntry=" + traceEntry);
    double minExitCornerDistance = Double.MAX_VALUE;
    FloatPoint nearestExitCorner = null;
    Direction pinExitDirection = null;
    final double tolerance = 1;
    for (Restr r : traceExitRestrictions) {
      int no = offsetPinShape.intersectingBorderLineNo(pinCenter, r.direction());
      Line ray = new Line(pinCenter, r.direction());
      FloatPoint corner = ray.intersectionApprox(offsetPinShape.borderLine(no));
      double d = corner.distanceSquare(traceEntry);
      boolean found = false;
      if (d + tolerance < minExitCornerDistance) {
        found = true;
      } else if (d < minExitCornerDistance + tolerance) {
        for (int i = 1; i < tracePolyline.cornerCount(); i++) {
          FloatPoint c = tracePolyline.cornerApprox(i);
          double cd = c.distanceSquare(corner);
          double od = c.distanceSquare(nearestExitCorner);
          if (cd + tolerance < od) { found = true; break; }
          else if (cd > od + tolerance) break;
        }
      }
      if (found) { minExitCornerDistance = d; pinExitDirection = r.direction(); nearestExitCorner = corner; }
    }
    return pinExitDirection;
  }

  public static void main(String[] args) {
    // The 200x100 pad placed at (900,1950)..(1100,2050), centred on (1000,2000).
    IntBox pad = new IntBox(900, 1950, 1100, 2050);
    Point center = new IntPoint(1000, 2000);
    List<Restr> four = restrictions(Direction.RIGHT, Direction.LEFT, Direction.UP, Direction.DOWN);

    System.out.println("=== nearestTraceExitCorner, edgeToTurnDist=10, halfWidth=5 ===");
    for (FloatPoint from : new FloatPoint[] {
        new FloatPoint(2000, 2000), new FloatPoint(0, 2000),
        new FloatPoint(1000, 5000), new FloatPoint(1000, -5000)}) {
      System.out.println("  from=" + from + " -> " + nearestTraceExitCorner(four, pad, center, 10, 5, from));
    }
    System.out.println("  edgeToTurnDist=-1 -> " + nearestTraceExitCorner(four, pad, center, -1, 5, new FloatPoint(2000, 2000)));

    System.out.println("=== calcNearestExitRestrictionDirection ===");
    // A trace leaving the pin centre and heading right, then up.
    Point[] corners = new Point[] { new IntPoint(1000, 2000), new IntPoint(3000, 2000), new IntPoint(3000, 4000) };
    Polyline pl = new Polyline(corners);
    System.out.println("  polyline cornerCount=" + pl.cornerCount());
    System.out.println("  dir=" + calcNearest(four, pad, center, 10, 5, pl));

    Point[] corners2 = new Point[] { new IntPoint(1000, 2000), new IntPoint(1000, 4000), new IntPoint(3000, 4000) };
    Polyline pl2 = new Polyline(corners2);
    System.out.println("  dir2=" + calcNearest(four, pad, center, 10, 5, pl2));

    System.out.println("=== offset pad border lines ===");
    TileShape off = (TileShape) pad.offset(15);
    System.out.println("  offset bbox=[" + off.boundingBox().ll.x + "," + off.boundingBox().ll.y
        + " .. " + off.boundingBox().ur.x + "," + off.boundingBox().ur.y + "]");
  }
}
