package app.freerouting.datastructures;

import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.Point;
import java.util.Collection;
import java.util.Collections;
import java.util.LinkedList;
import java.util.List;
import java.util.Random;

/**
 * Java ground truth for {@link PlanarDelaunayTriangulation} (Plan 2 Task 13). Twin: `p2t13`.
 *
 * <p>args: n seed mode
 *
 * <p>Modes:
 *
 * <ul>
 *   <li>0 — n random points, one single-corner object each, coordinates in [-100000, 100000).
 *   <li>1 — the four corners of a square (the brief's pinned case).
 *   <li>2 — a collinear triple.
 *   <li>3 — a square plus three coincident interior points (duplicate handling).
 *   <li>4 — n objects of *two* corners each, so `corner.object == commonCorner.object` (the
 *       same-object duplicate short-circuit at PlanarDelaunayTriangulation.java:157) is reachable.
 *   <li>5 — a 5x5 integer grid: maximal cocircularity and collinearity.
 *   <li>6 — n random points in the tiny range [-20, 20), which forces duplicates and collinear
 *       triples at a high rate.
 *   <li>7 — n random points on a circle of radius 30000 (every point cocircular).
 * </ul>
 *
 * <p>Prints the shuffle permutation the class's fixed-seed `Collections.shuffle` produces for the
 * corner count in play (re-derived here with an identical `LinkedList` + `new Random(99)`, because
 * the real one is private), then `getEdgeLines()` in iteration order, then `validate()`.
 */
public class P2T13 {

  static long state;

  static long next() {
    state ^= state << 13;
    state ^= state >>> 7;
    state ^= state << 17;
    return state;
  }

  static int rnd(int bound) {
    return (int) Long.remainderUnsigned(next(), bound);
  }

  static class Obj implements PlanarDelaunayTriangulation.Storable {

    final int id;
    final Point[] corners;

    Obj(int id, Point... corners) {
      this.id = id;
      this.corners = corners;
    }

    @Override
    public Point[] getTriangulationCorners() {
      return corners;
    }
  }

  static Point p(int x, int y) {
    return new IntPoint(x, y);
  }

  static String fmt(Point point) {
    if (point == null) {
      return "null";
    }
    IntPoint ip = (IntPoint) point;
    return ip.x + "," + ip.y;
  }

  static String objId(PlanarDelaunayTriangulation.Storable object) {
    return object == null ? "none" : Integer.toString(((Obj) object).id);
  }

  public static void main(String[] args) {
    int n = args.length > 0 ? Integer.parseInt(args[0]) : 50;
    long seed = args.length > 1 ? Long.parseLong(args[1]) : 42;
    int mode = args.length > 2 ? Integer.parseInt(args[2]) : 0;
    state = seed;

    List<PlanarDelaunayTriangulation.Storable> objects = new LinkedList<>();
    int nextId = 1;
    if (mode == 1) {
      objects.add(new Obj(nextId++, p(0, 0)));
      objects.add(new Obj(nextId++, p(1000, 0)));
      objects.add(new Obj(nextId++, p(1000, 1000)));
      objects.add(new Obj(nextId++, p(0, 1000)));
    } else if (mode == 2) {
      objects.add(new Obj(nextId++, p(0, 0)));
      objects.add(new Obj(nextId++, p(500, 500)));
      objects.add(new Obj(nextId++, p(1000, 1000)));
    } else if (mode == 3) {
      objects.add(new Obj(nextId++, p(0, 0)));
      objects.add(new Obj(nextId++, p(1000, 0)));
      objects.add(new Obj(nextId++, p(1000, 1000)));
      objects.add(new Obj(nextId++, p(0, 1000)));
      objects.add(new Obj(nextId++, p(300, 400)));
      objects.add(new Obj(nextId++, p(300, 400)));
      objects.add(new Obj(nextId++, p(300, 400)));
    } else if (mode == 4) {
      for (int i = 0; i < n; i++) {
        objects.add(
            new Obj(
                nextId++,
                p(rnd(200000) - 100000, rnd(200000) - 100000),
                p(rnd(200000) - 100000, rnd(200000) - 100000)));
      }
    } else if (mode == 5) {
      for (int x = 0; x < 5; x++) {
        for (int y = 0; y < 5; y++) {
          objects.add(new Obj(nextId++, p(x * 1000, y * 1000)));
        }
      }
    } else if (mode == 6) {
      for (int i = 0; i < n; i++) {
        objects.add(new Obj(nextId++, p(rnd(40) - 20, rnd(40) - 20)));
      }
    } else if (mode == 7) {
      for (int i = 0; i < n; i++) {
        double angle = 2.0 * Math.PI * i / n;
        objects.add(
            new Obj(
                nextId++,
                p((int) Math.round(30000.0 * Math.cos(angle)),
                    (int) Math.round(30000.0 * Math.sin(angle)))));
      }
    } else {
      for (int i = 0; i < n; i++) {
        objects.add(new Obj(nextId++, p(rnd(200000) - 100000, rnd(200000) - 100000)));
      }
    }

    // Re-derive the permutation the constructor's private shuffle produces, so the Rust twin can
    // be checked against it directly rather than only through the triangulation result.
    int cornerCount = 0;
    StringBuilder inputs = new StringBuilder();
    for (PlanarDelaunayTriangulation.Storable object : objects) {
      for (Point corner : object.getTriangulationCorners()) {
        inputs.append(" ").append(objId(object)).append("@").append(fmt(corner));
        cornerCount++;
      }
    }
    System.out.println("mode=" + mode + " objects=" + objects.size() + " corners=" + cornerCount);
    System.out.println("in:" + inputs);

    List<Integer> permutation = new LinkedList<>();
    for (int i = 0; i < cornerCount; i++) {
      permutation.add(i);
    }
    Random shuffler = new Random(99);
    shuffler.setSeed(99);
    Collections.shuffle(permutation, shuffler);
    System.out.println("perm=" + permutation);

    PlanarDelaunayTriangulation triangulation = new PlanarDelaunayTriangulation(objects);
    Collection<PlanarDelaunayTriangulation.ResultEdge> edges = triangulation.getEdgeLines();
    System.out.println("count=" + edges.size());
    int i = 0;
    for (PlanarDelaunayTriangulation.ResultEdge edge : edges) {
      System.out.println(
          "E"
              + i
              + " so="
              + objId(edge.startObject)
              + " sp="
              + fmt(edge.startPoint)
              + " eo="
              + objId(edge.endObject)
              + " ep="
              + fmt(edge.endPoint));
      i++;
    }
    System.out.println("validate=" + triangulation.validate());
  }
}
