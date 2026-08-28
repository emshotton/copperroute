package app.freerouting.geometry.planar;

import java.util.*;

/** Task 6 scratch harness: Pin.getShape / Pin.getTraceExitRestrictions math on real geometry. */
public class T6 {

  // ---- Padstack.getTraceExitDirections (Padstack.java:164-195), inlined --------------------
  static Collection<Direction> traceExitDirections(ConvexShape shape, double factor) {
    Collection<Direction> result = new LinkedList<>();
    if (shape == null) return result;
    if (!(shape instanceof IntBox) && !(shape instanceof IntOctagon)) return result;
    IntBox currentBox = shape.boundingBox();
    double width = currentBox.width();
    double height = currentBox.height();
    boolean allDirs = Math.max(width, height) < factor * Math.min(width, height);
    if (allDirs || width >= height) {
      result.add(Direction.RIGHT);
      result.add(Direction.LEFT);
    }
    if (allDirs || width <= height) {
      result.add(Direction.UP);
      result.add(Direction.DOWN);
    }
    return result;
  }

  // ---- Pin.getShape (Pin.java:164-245), inlined for one shape index ------------------------
  static ConvexShape pinShape(
      ConvexShape padstackShape,
      Vector relLocation,
      double pinRotation,
      double componentRotation,
      boolean placedOnFront,
      boolean flipStyleRotateFirst,
      Vector componentTranslation) {
    boolean mirrorOnYaxis = !placedOnFront && !flipStyleRotateFirst;
    Vector rel = mirrorOnYaxis ? relLocation.mirrorAtYAxis() : relLocation;
    ConvexShape currentShape = padstackShape;
    if (currentShape == null) return null;
    if (pinRotation % 90 == 0) {
      int f = ((int) pinRotation) / 90;
      if (f != 0) currentShape = (ConvexShape) currentShape.turn90Degree(f, Point.ZERO);
    } else {
      currentShape = (ConvexShape) currentShape.rotateApprox(Math.toRadians(pinRotation), FloatPoint.ZERO);
    }
    if (mirrorOnYaxis) currentShape = (ConvexShape) currentShape.mirrorVertical(Point.ZERO);
    ConvexShape translatedShape = (ConvexShape) currentShape.translateBy(rel);
    if (componentRotation % 90 == 0) {
      int f = ((int) componentRotation) / 90;
      if (f != 0) translatedShape = (ConvexShape) translatedShape.turn90Degree(f, Point.ZERO);
    } else {
      translatedShape = (ConvexShape) translatedShape.rotateApprox(Math.toRadians(componentRotation), FloatPoint.ZERO);
    }
    if (!placedOnFront && flipStyleRotateFirst) {
      translatedShape = (ConvexShape) translatedShape.mirrorVertical(Point.ZERO);
    }
    return (ConvexShape) translatedShape.translateBy(componentTranslation);
  }

  // ---- Pin.relativeLocation (Pin.java:65-89), inlined --------------------------------------
  static Vector relativeLocation(
      Vector packagePinRelLocation, double componentRotation, boolean placedOnFront, boolean flipStyleRotateFirst) {
    Vector relLocation = packagePinRelLocation;
    if (!placedOnFront && !flipStyleRotateFirst) relLocation = packagePinRelLocation.mirrorAtYAxis();
    if (componentRotation % 90 == 0) {
      int f = ((int) componentRotation) / 90;
      if (f != 0) relLocation = relLocation.turn90Degree(f);
    } else {
      FloatPoint approx = relLocation.toFloat();
      approx = approx.rotate(Math.toRadians(componentRotation), FloatPoint.ZERO);
      relLocation = approx.round().differenceBy(Point.ZERO);
    }
    if (!placedOnFront && flipStyleRotateFirst) relLocation = relLocation.mirrorAtYAxis();
    return relLocation;
  }

  // ---- Pin.getTraceExitRestrictions (Pin.java:265-331), inlined ----------------------------
  static void exitRestrictions(
      ConvexShape padstackShapeOnLayer,
      double padXyFactor,
      Shape pinShapeOnLayer,
      Point pinCenter,
      double componentRotation,
      double pinRotationInDegree) {
    Collection<Direction> padstackExitDirections = traceExitDirections(padstackShapeOnLayer, padXyFactor);
    System.out.println("padstackExitDirections=" + padstackExitDirections);
    if (padstackExitDirections.isEmpty()) return;
    if (!(pinShapeOnLayer instanceof TileShape padShape)) {
      System.out.println("not a TileShape");
      return;
    }
    FloatPoint centerApprox = pinCenter.toFloat();
    for (Direction d : padstackExitDirections) {
      double currentRotationInDegree = componentRotation + pinRotationInDegree;
      Direction currentExitDirection;
      if (currentRotationInDegree % 45 == 0) {
        int f = ((int) currentRotationInDegree) / 45;
        currentExitDirection = d.turn45Degree(f);
      } else {
        double a = Math.toRadians(currentRotationInDegree) + d.angleApprox();
        currentExitDirection = Direction.getInstanceApprox(a);
      }
      int no = padShape.intersectingBorderLineNo(pinCenter, currentExitDirection);
      if (no < 0) {
        System.out.println("  border line not found for " + currentExitDirection);
        continue;
      }
      Line exitLine = new Line(pinCenter, currentExitDirection);
      FloatPoint nearest = exitLine.intersectionApprox(padShape.borderLine(no));
      System.out.println("  dir=" + currentExitDirection + " borderLineNo=" + no
          + " nearest=" + nearest + " minLength=" + centerApprox.distance(nearest));
    }
  }

  static String bx(Shape s){IntBox b=s.boundingBox();return "["+b.ll.x+","+b.ll.y+" .. "+b.ur.x+","+b.ur.y+"]";}

  public static void main(String[] args) {
    System.out.println("=== THT pin: 2-layer padstack, component rot 90, front, rel (30,0) ===");
    IntBox p0 = new IntBox(-10, -20, 10, 20);
    IntBox p1 = new IntBox(-15, -15, 15, 15);
    Vector rel = new IntVector(30, 0);
    Vector trans = new IntVector(1000, 2000);
    ConvexShape s0 = pinShape(p0, rel, 0, 90, true, false, trans);
    ConvexShape s1 = pinShape(p1, rel, 0, 90, true, false, trans);
    System.out.println("shape0 bbox=" + bx(s0));
    System.out.println("shape1 bbox=" + bx(s1));
    Vector relLoc = relativeLocation(rel, 90, true, false);
    System.out.println("relativeLocation=" + relLoc.toFloat());
    Point center = Point.ZERO.translateBy(trans).translateBy(relLoc);
    System.out.println("center=" + center);
    System.out.println("shape0.containsInside(center)=" + s0.containsInside(center));
    System.out.println("shape1.containsInside(center)=" + s1.containsInside(center));

    System.out.println("=== SMD pad exit restrictions: 200x100 box, factor 3.0, no rotation ===");
    IntBox pad = new IntBox(-100, -50, 100, 50);
    Vector rel2 = new IntVector(0, 0);
    Vector trans2 = new IntVector(1000, 2000);
    ConvexShape padShape = pinShape(pad, rel2, 0, 0, true, false, trans2);
    System.out.println("padShape bbox=" + bx(padShape));
    Point center2 = new IntPoint(1000, 2000);
    exitRestrictions(pad, 3.0, padShape, center2, 0, 0);

    System.out.println("=== same pad, factor 1.5 (pinCount > 3) ===");
    exitRestrictions(pad, 1.5, padShape, center2, 0, 0);

    System.out.println("=== same pad rotated 90 degrees by the component ===");
    ConvexShape padShape90 = pinShape(pad, rel2, 0, 90, true, false, trans2);
    System.out.println("padShape90 bbox=" + bx(padShape90));
    exitRestrictions(pad, 3.0, padShape90, center2, 90, 0);

    System.out.println("=== border line dump of the unrotated pad ===");
    TileShape ts = (TileShape) padShape;
    for (int i = 0; i < ts.borderLineCount(); i++) {
      System.out.println("  line " + i + " = " + ts.borderLine(i) + " dir=" + ts.borderLine(i).direction());
    }
  }
}
