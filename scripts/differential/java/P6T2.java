package app.freerouting.board.searchtree;

import app.freerouting.autoroute.expansion.CompleteFreeSpaceExpansionRoom;
import app.freerouting.autoroute.expansion.IncompleteFreeSpaceExpansionRoom;
import app.freerouting.board.facade.BasicBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.geometry.planar.Area;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.model.structure.Layer;
import app.freerouting.board.model.structure.LayerStructure;
import app.freerouting.board.state.Communication;
import app.freerouting.core.library.Package;
import app.freerouting.core.library.Padstack;
import app.freerouting.geometry.planar.ConvexShape;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.IntOctagon;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.IntVector;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import app.freerouting.geometry.planar.PolylineShape;
import app.freerouting.geometry.planar.Shape;
import app.freerouting.geometry.planar.TileShape;
import app.freerouting.rules.BoardRules;
import app.freerouting.rules.ClearanceMatrix;
import java.util.Collection;

/**
 * Plan 6 Task 3: the {@code ShapeSearchTree.completeShape} / {@code divideLargeRoom} differential
 * driver — the two methods {@code p2t10} explicitly skipped ("every public ShapeSearchTree method
 * except completeShape/divideLargeRoom", scripts/differential/README.md).
 *
 * <p>Declares {@code package app.freerouting.board.searchtree} so it can call the {@code
 * protected} {@code divideLargeRoom} directly rather than only through {@code completeShape}.
 *
 * <p>It builds the same {@code BasicBoard} {@code P2T10.java} builds — two layers, a two-pin
 * component, two traces, an empty outline — plus {@code n} random obstacle areas drawn from the
 * shared xorshift stream every other randomised driver uses, gets {@code
 * searchTreeManager.getAutorouteTree(1)} for each of the three angle regimes, seeds the tree with
 * three {@code CompleteFreeSpaceExpansionRoom}s so the {@code instanceof
 * CompleteFreeSpaceExpansionRoom} branch is live, and then, for {@code rooms} random seed
 * boxes/octagons/layers, calls {@code completeShape} and {@code divideLargeRoom} and prints every
 * returned room.
 *
 * <p>args: seed n rooms. {@code run.sh p6t2 <seed> <n> <rooms>} diffs this against {@code
 * p6t2.rs}.
 */
public class P6T2 {

  static final int RANGE = 9000;
  static final IntBox BOUNDING_BOX = new IntBox(-10000, -10000, 10000, 10000);

  static long state;

  static BasicBoard board;
  static CompleteFreeSpaceExpansionRoom[] seedRooms;
  static IntBox[] seedRoomBoxes;

  static long next() {
    state ^= state << 13;
    state ^= state >>> 7;
    state ^= state << 17;
    return state;
  }

  static int rnd(int bound) {
    return (int) Long.remainderUnsigned(next(), bound);
  }

  static int randCoord(int range) {
    return rnd(2 * range + 1) - range;
  }

  static IntBox randomBox(int range, int minSize, int maxSize) {
    int w = minSize + rnd(maxSize - minSize + 1);
    int h = minSize + rnd(maxSize - minSize + 1);
    int x = randCoord(range);
    int y = randCoord(range);
    return new IntBox(x, y, x + w, y + h);
  }

  /** A box with its four corners clipped by independent random amounts, then normalized. */
  static IntOctagon randomOctagon(IntBox b) {
    int d1 = rnd(300);
    int d2 = rnd(300);
    int d3 = rnd(300);
    int d4 = rnd(300);
    return new IntOctagon(
            b.ll.x,
            b.ll.y,
            b.ur.x,
            b.ur.y,
            b.ll.x - b.ur.y + d1,
            b.ur.x - b.ll.y - d2,
            b.ll.x + b.ll.y + d3,
            b.ur.x + b.ur.y - d4)
        .normalize();
  }

  /** A small box inside `outer`, or a degenerate one at its lower-left corner if it is tiny. */
  static IntBox randomInside(IntBox outer) {
    int w = 10 + rnd(300);
    int h = 10 + rnd(300);
    int spanX = Math.max(1, outer.ur.x - outer.ll.x - w);
    int spanY = Math.max(1, outer.ur.y - outer.ll.y - h);
    int x = outer.ll.x + rnd(spanX);
    int y = outer.ll.y + rnd(spanY);
    return new IntBox(x, y, x + w, y + h);
  }

  public static void main(String[] args) {
    long seed = args.length > 0 ? Long.parseLong(args[0]) : 42L;
    int obstacleCount = args.length > 1 ? Integer.parseInt(args[1]) : 20;
    int roomCount = args.length > 2 ? Integer.parseInt(args[2]) : 2000;

    // `FRLogger` writes its warnings to stdout, and `completeShape` warns for every seed room
    // whose shape is of the wrong class for the regime (…90Degree.java:54, …45Degree.java:135) —
    // which is a third of them by construction. The port drops every `FRLogger` payload
    // (`global-constraints.md`), so the Java side must be silenced or the two can never agree.
    app.freerouting.logger.FRLogger.disableLogging();

    System.out.println(
        "mode=p6t2 seed=" + seed + " obstacles=" + obstacleCount + " rooms=" + roomCount);

    for (int mode = 0; mode < 3; mode++) {
      // The stream is reset per regime, so each regime sees the same boards and the same seed
      // rooms and only the tree class differs.
      state = seed == 0 ? 0x9E3779B97F4A7C15L : seed;
      build(mode);
      for (int i = 0; i < obstacleCount; i++) {
        insertRandomObstacle();
      }
      ShapeSearchTree tree = board.searchTreeManager.getAutorouteTree(1);
      System.out.println(
          "regime="
              + mode
              + " angle="
              + board.rules.getTraceAngleRestriction()
              + " tree="
              + tree
              + " size="
              + tree.size()
              + " items="
              + board.getItems().size());
      dumpItems();
      insertSeedRooms(tree);
      for (int i = 0; i < roomCount; i++) {
        runOne(tree, mode, i);
      }
    }
  }

  /** The `P2T10.build` board, verbatim. */
  static void build(int mode) {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    cm.appendClass("wide");
    cm.setValue(2, 1, 600);
    cm.setValue(2, 2, 800);
    BoardRules rules = new BoardRules(ls, cm);
    if (mode == 1) {
      rules.setTraceAngleRestriction(AngleRestriction.NINETY_DEGREE);
    } else if (mode == 2) {
      rules.setTraceAngleRestriction(AngleRestriction.FORTYFIVE_DEGREE);
    } else {
      rules.setTraceAngleRestriction(AngleRestriction.NONE);
    }
    Communication comm = new Communication();
    board = new BasicBoard(BOUNDING_BOX, ls, new PolylineShape[0], 0, rules, comm);
    board.library.padstacks = new app.freerouting.core.library.Padstacks(ls);
    board.library.packages = new app.freerouting.core.library.Packages(board.library.padstacks);

    ConvexShape[] smd = {new IntBox(-50, -50, 50, 50), null};
    Padstack smdPad = board.library.padstacks.add("smd", smd, false, false);
    ConvexShape[] thru = {
      new IntOctagon(-70, -70, 70, 70, -140, 140, -140, 140),
      new IntOctagon(-70, -70, 70, 70, -140, 140, -140, 140)
    };
    Padstack thruPad = board.library.padstacks.add("thru", thru, true, false);

    Package.Pin[] pins = {
      new Package.Pin("P1", smdPad.id, new IntVector(-500, 0), 0),
      new Package.Pin("P2", thruPad.id, new IntVector(500, 0), 0)
    };
    Package pkg =
        board.library.packages.add(
            "pkg",
            pins,
            new Shape[0],
            new double[0],
            new boolean[0],
            new Package.Keepout[0],
            new Package.Keepout[0],
            new Package.Keepout[0],
            true);
    board.components.add(new IntPoint(0, 0), 0, true, pkg);

    board.insertPin(1, 0, new int[] {1}, 1, FixedState.UNFIXED);
    board.insertPin(1, 1, new int[] {1}, 1, FixedState.UNFIXED);
    Polyline traceLine =
        new Polyline(
            new Point[] {
              new IntPoint(-500, 0), new IntPoint(0, 0), new IntPoint(0, 400), new IntPoint(500, 400)
            });
    board.insertTraceWithoutCleaning(traceLine, 0, 30, new int[] {1}, 1, FixedState.UNFIXED);
    Polyline other =
        new Polyline(
            new Point[] {new IntPoint(-800, 300), new IntPoint(-800, 900), new IntPoint(300, 900)});
    board.insertTraceWithoutCleaning(other, 0, 40, new int[] {2}, 2, FixedState.UNFIXED);
  }

  static void insertRandomObstacle() {
    Area shape = randomBox(RANGE, 100, 2500);
    int layer = rnd(2);
    board.insertObstacle(shape, layer, 1, FixedState.UNFIXED);
  }

  /**
   * Three complete expansion rooms, inserted into the autoroute tree exactly as {@code
   * AutorouteEngine.addCompleteRoom} does ({@code AutorouteEngine.java:534}). Without them the
   * {@code instanceof CompleteFreeSpaceExpansionRoom} branch of all three {@code completeShape}s
   * is unreachable.
   */
  static void insertSeedRooms(ShapeSearchTree tree) {
    seedRooms = new CompleteFreeSpaceExpansionRoom[3];
    seedRoomBoxes = new IntBox[3];
    for (int i = 0; i < seedRooms.length; i++) {
      IntBox box = randomBox(RANGE, 500, 3000);
      int layer = rnd(2);
      seedRoomBoxes[i] = box;
      seedRooms[i] = new CompleteFreeSpaceExpansionRoom(box, layer, i + 1);
      tree.insert(seedRooms[i]);
      System.out.println(
          "  seedRoom " + (i + 1) + " layer=" + layer + " shape=" + shp(box));
    }
    System.out.println("  treeSizeWithRooms=" + tree.size());
  }

  static void runOne(ShapeSearchTree tree, int mode, int index) {
    int shapeKind = rnd(3);
    IntBox shapeBox = randomBox(RANGE, 200, 6000);
    IntOctagon shapeOct = randomOctagon(shapeBox);
    // Three quarters of the seed rooms get a contained shape that really sits inside the room
    // shape, which is what `AutorouteEngine` always hands in; the rest get an independent box, so
    // the "shapeToBeContained is somewhere else entirely" paths stay covered.
    IntBox insideBox = randomInside(shapeBox);
    IntBox looseBox = randomBox(RANGE, 10, 400);
    IntBox containedBox = rnd(4) == 0 ? looseBox : insideBox;
    int layer = rnd(2);
    int netNumber = 1 + rnd(3);
    int ignoreKind = rnd(4);
    int ignoreRoom = rnd(3);
    int ignoreShapeKind = rnd(3);
    IntBox ignoreBox = randomBox(RANGE, 100, 4000);
    // A third of the ignore shapes are one of the seed expansion rooms grown by a random margin,
    // which is what makes `ignoreShape.contains(intersection)` — the one branch of all three
    // `completeShape`s that reads `ignoreShape` at all — actually fire.
    int grow = rnd(2000);
    IntBox grownRoomBox =
        new IntBox(
            seedRoomBoxes[ignoreRoom].ll.x - grow,
            seedRoomBoxes[ignoreRoom].ll.y - grow,
            seedRoomBoxes[ignoreRoom].ur.x + grow,
            seedRoomBoxes[ignoreRoom].ur.y + grow);

    TileShape roomShape;
    if (shapeKind == 0) {
      roomShape = null;
    } else if (shapeKind == 1) {
      roomShape = shapeBox;
    } else {
      roomShape = shapeOct;
    }
    IncompleteFreeSpaceExpansionRoom room =
        new IncompleteFreeSpaceExpansionRoom(roomShape, layer, containedBox);

    SearchTreeObject ignoreObject = null;
    if (ignoreKind == 1) {
      ignoreObject = seedRooms[ignoreRoom];
    } else if (ignoreKind == 2) {
      ignoreObject = board.getItem(2 + ignoreRoom);
    }
    TileShape ignoreShape =
        ignoreShapeKind == 0 ? null : (ignoreShapeKind == 1 ? ignoreBox : grownRoomBox);

    System.out.println(
        "call regime="
            + mode
            + " i="
            + index
            + " layer="
            + layer
            + " net="
            + netNumber
            + " shapeKind="
            + shapeKind
            + " shape="
            + shp(roomShape)
            + " contained="
            + shp(containedBox)
            + " ignoreObject="
            + describeIgnore(ignoreObject)
            + " ignoreShape="
            + shp(ignoreShape));

    Collection<IncompleteFreeSpaceExpansionRoom> completed =
        tree.completeShape(room, netNumber, ignoreObject, ignoreShape);
    dumpRooms("  completeShape", completed);
    Collection<IncompleteFreeSpaceExpansionRoom> divided =
        tree.divideLargeRoom(completed, board.getBoundingBox());
    dumpRooms("  divideLargeRoom", divided);
  }

  static String describeIgnore(SearchTreeObject object) {
    if (object == null) {
      return "null";
    }
    if (object instanceof Item item) {
      return "item" + item.getId();
    }
    if (object instanceof CompleteFreeSpaceExpansionRoom room) {
      return "room" + room.getId();
    }
    return object.getClass().getSimpleName();
  }

  static void dumpRooms(String label, Collection<IncompleteFreeSpaceExpansionRoom> rooms) {
    System.out.println(label + " n=" + rooms.size());
    int i = 0;
    for (IncompleteFreeSpaceExpansionRoom room : rooms) {
      TileShape shape = room.getShape();
      TileShape contained = room.getContainedShape();
      System.out.println(
          "    ["
              + i
              + "] layer="
              + room.getLayer()
              + " dim="
              + (shape == null ? "null" : String.valueOf(shape.dimension()))
              + " shape="
              + shp(shape)
              + " corners="
              + corners(shape)
              + " contained="
              + shp(contained)
              + " containedCorners="
              + corners(contained));
      ++i;
    }
  }

  static void dumpItems() {
    for (Item it : board.getItems()) {
      System.out.println(
          "  item id="
              + it.getId()
              + " class="
              + it.getClass().getSimpleName()
              + " layers="
              + it.firstLayer()
              + ".."
              + it.lastLayer()
              + " tileShapeCount="
              + it.tileShapeCount()
              + " bbox="
              + b(it.boundingBox()));
    }
  }

  static String corners(TileShape shape) {
    if (shape == null) {
      return "null";
    }
    FloatPoint[] arr = shape.cornerApproxArr();
    StringBuilder sb = new StringBuilder("(");
    for (int i = 0; i < arr.length; i++) {
      if (i > 0) {
        sb.append(';');
      }
      sb.append(Double.toString(arr[i].x)).append(',').append(Double.toString(arr[i].y));
    }
    return sb.append(')').toString();
  }

  static String b(IntBox x) {
    return "[" + x.ll.x + "," + x.ll.y + ".." + x.ur.x + "," + x.ur.y + "]";
  }

  static String shp(Shape s) {
    if (s == null) {
      return "null";
    }
    if (s instanceof IntBox x) {
      return "Box" + b(x);
    }
    if (s instanceof IntOctagon o) {
      return "Oct["
          + o.leftX
          + ","
          + o.bottomY
          + ","
          + o.rightX
          + ","
          + o.topY
          + ","
          + o.upperLeftDiagonalX
          + ","
          + o.lowerRightDiagonalX
          + ","
          + o.lowerLeftDiagonalX
          + ","
          + o.upperRightDiagonalX
          + "]";
    }
    if (s instanceof app.freerouting.geometry.planar.Simplex sx) {
      StringBuilder sb = new StringBuilder("Simplex{");
      for (int i = 0; i < sx.borderLineCount(); i++) {
        app.freerouting.geometry.planar.Line l = sx.borderLine(i);
        sb.append("(").append(l.a.toString()).append("->").append(l.b.toString()).append(")");
      }
      return sb.append("}").toString();
    }
    return s.getClass().getSimpleName() + b(s.boundingBox());
  }
}
