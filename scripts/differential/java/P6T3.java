package app.freerouting.autoroute.expansion;

import app.freerouting.autoroute.maze.AutorouteEngine;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.model.structure.Layer;
import app.freerouting.board.model.structure.LayerStructure;
import app.freerouting.board.searchtree.SearchTreeObject;
import app.freerouting.board.searchtree.ShapeSearchTree;
import app.freerouting.board.state.Communication;
import app.freerouting.core.library.Package;
import app.freerouting.core.library.Padstack;
import app.freerouting.datastructures.ShapeTree;
import app.freerouting.geometry.planar.Area;
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
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Collection;
import java.util.List;
import java.util.SortedSet;
import java.util.TreeSet;

/**
 * Plan 6 Task 4: the {@code SortedRoomNeighbours} differential driver.
 *
 * <p>Declares {@code package app.freerouting.autoroute.expansion} so it sits beside the class under
 * test, and reaches its {@code private} members — {@code calculateNeighbours}, the {@code
 * sortedNeighbours} / {@code ownNetObjects} / {@code completedRoom} fields and the {@code private
 * class SortedRoomNeighbour} — by reflection. That is the only way to observe the sorted neighbour
 * list at all: {@code calculate} consumes the instance and returns only the completed room, and
 * everything above {@code calculateNeighbours} needs an {@code AutorouteEngine}, which the Rust
 * port does not have until Plan 6 Task 6.
 *
 * <p><b>mode 0</b> (any-angle, the brief's mode) builds the {@code P2T10} board plus {@code n}
 * random obstacle areas, gets {@code searchTreeManager.getAutorouteTree(1)}, seeds it with three
 * {@code CompleteFreeSpaceExpansionRoom}s exactly as {@code AutorouteEngine.addCompleteRoom} does
 * ({@code AutorouteEngine.java:534}) so the tree really holds rooms as well as items, and then runs
 * {@code calculateNeighbours} for {@code rooms} random seed rooms — incomplete free-space rooms
 * (box or octagon) and obstacle rooms over a random item shape. Per call it prints every sorted
 * neighbour (touching side numbers, both corner flags, both corners, the neighbour object's kind
 * and id, its shape and the intersection), the own-net list, and the completed room's resulting
 * door list.
 *
 * <p><b>mode 1</b> is the hazard-F probe: it builds {@code SortedRoomNeighbour}s directly through
 * the inner class's constructor, inserts them into a {@code TreeSet} in a given order and prints
 * what survives — the comparator's non-transitivity and the {@code TreeSet}'s silent drop, with no
 * board in the way.
 *
 * <p><b>modes 6 and 7</b> (Task 5) are mode 4 for the two angle-restricted sorters: the board is
 * built in the 45-degree / 90-degree regime, so {@code getAutorouteTree} answers the matching
 * search-tree subclass, and the driver reflects into {@code
 * Sorted45DegreeRoomNeighbours.calculateNeighbours} / {@code
 * SortedOrthogonalRoomNeighbours.calculateNeighbours} and their own private inner {@code
 * SortedRoomNeighbour} classes — which are three different classes with three different field
 * sets. It also dumps {@code edgeInteriorTouchesObstacle}, the array {@code tryRemoveEdge} reads.
 *
 * <p><b>modes 8 and 9</b> are mode 5 for the same two regimes: the whole of {@code
 * SortedRoomNeighbours.complete} against a real {@code AutorouteEngine}, which dispatches on the
 * tree subclass, so the two subclasses' {@code tryRemoveEdge}, {@code
 * calculateNewIncompleteRooms}, {@code calculateEdgeIncompleteRoomsOfObstacleExpansionRoom} /
 * {@code calculateIncompleteRoomsWithEmptyNeighbours} and {@code insertIncompleteRoom} all run.
 *
 * <p>args: mode seed n rooms. {@code run.sh p6t3 <mode> <seed> <n> <rooms>}.
 */
public class P6T3 {

  static final int RANGE = 9000;
  static final IntBox BOUNDING_BOX = new IntBox(-10000, -10000, 10000, 10000);

  static long state;

  static RoutingBoard board;
  static int roomIdCounter;

  static Method calculateNeighbours;
  static Field fSortedNeighbours;
  static Field fOwnNetObjects;
  static Field fCompletedRoom;
  static Field fIncompleteExpansionRooms;
  static Class<?> neighbourClass;
  static Constructor<?> neighbourCtor;
  static Constructor<?> outerCtor;
  static Field fTouchingSideNoOfRoom;
  static Field fTouchingSideNoOfNeighbourRoom;
  static Field fRoomTouchIsCorner;
  static Field fNeighbourRoomTouchIsCorner;
  static Field fSearchTreeObject;
  static Field fNeighbourShape;
  static Field fIntersection;
  static Method mFirstCorner;
  static Method mLastCorner;

  // The two angle-restricted siblings (Task 5). Each has its OWN private inner
  // `SortedRoomNeighbour` with its own fields, so each needs its own reflection block.
  static Method calculateNeighbours45;
  static Field f45SortedNeighbours;
  static Field f45CompletedRoom;
  static Field f45EdgeInterior;
  static Class<?> neighbour45Class;
  static Field f45SearchTreeObject;
  static Field f45Shape;
  static Field f45Intersection;
  static Field f45FirstTouchingSide;
  static Field f45LastTouchingSide;

  static Method calculateNeighboursOrtho;
  static Field fOrthoSortedNeighbours;
  static Field fOrthoCompletedRoom;
  static Field fOrthoEdgeInterior;
  static Class<?> neighbourOrthoClass;
  static Field fOrthoSearchTreeObject;
  static Field fOrthoShape;
  static Field fOrthoIntersection;
  static Field fOrthoFirstTouchingSide;
  static Field fOrthoLastTouchingSide;

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

  static IntBox randomInside(IntBox outer) {
    int w = 10 + rnd(300);
    int h = 10 + rnd(300);
    int spanX = Math.max(1, outer.ur.x - outer.ll.x - w);
    int spanY = Math.max(1, outer.ur.y - outer.ll.y - h);
    int x = outer.ll.x + rnd(spanX);
    int y = outer.ll.y + rnd(spanY);
    return new IntBox(x, y, x + w, y + h);
  }

  public static void main(String[] args) throws Exception {
    int mode = args.length > 0 ? Integer.parseInt(args[0]) : 0;
    long seed = args.length > 1 ? Long.parseLong(args[1]) : 42L;
    int obstacleCount = args.length > 2 ? Integer.parseInt(args[2]) : 20;
    int roomCount = args.length > 3 ? Integer.parseInt(args[3]) : 1000;

    // `FRLogger` writes to stdout, and `calculateNeighbours` logs four `debug` messages for
    // degenerate geometry. The port drops every `FRLogger` payload (`global-constraints.md`).
    app.freerouting.logger.FRLogger.disableLogging();
    reflect();

    System.out.println(
        "mode=p6t3 submode="
            + mode
            + " seed="
            + seed
            + " obstacles="
            + obstacleCount
            + " rooms="
            + roomCount);

    state = seed == 0 ? 0x9E3779B97F4A7C15L : seed;
    if (mode == 1 || mode == 2) {
      comparatorProbe(roomCount, mode == 2);
      return;
    }
    if (mode == 3) {
      cornerTouchProbe(roomCount);
      return;
    }

    build(mode);
    for (int i = 0; i < obstacleCount; i++) {
      // modes 4, 6 and 7 snap every obstacle to a 500-unit grid. Without it a completed room's
      // corner never lands exactly on an obstacle's, so `calculateNeighbours`' whole dimension-0
      // branch (`SortedRoomNeighbours.java:286-326` — `equalsCorner`, `containsOnBorderLineNo`,
      // both corner flags) stays dead: mode 0 produces no corner touch at all on a random board.
      insertRandomObstacle(mode == 4 || mode == 6 || mode == 7);
    }
    ShapeSearchTree tree = board.searchTreeManager.getAutorouteTree(1);
    AutorouteEngine engine =
        (mode == 5 || mode == 8 || mode == 9) ? new AutorouteEngine(board, 1, false) : null;
    System.out.println(
        "regime="
            + regimeOf(mode)
            + " angle="
            + board.rules.getTraceAngleRestriction()
            + " size="
            + tree.size()
            + " items="
            + board.getItems().size());
    dumpItems();
    insertSeedRooms(tree, engine);
    for (int i = 0; i < roomCount; i++) {
      if (engine != null) {
        runOneComplete(tree, engine, i, mode);
      } else {
        runOne(tree, i, mode);
      }
    }
  }

  /** 0 for the any-angle regime, 1 for 90 degrees, 2 for 45 degrees — `p6t2`'s numbering. */
  static int regimeOf(int mode) {
    if (mode == 6 || mode == 8) {
      return 2;
    }
    if (mode == 7 || mode == 9) {
      return 1;
    }
    return 0;
  }

  // -----------------------------------------------------------------------------------------
  // Reflection
  // -----------------------------------------------------------------------------------------

  static void reflect() throws Exception {
    calculateNeighbours =
        SortedRoomNeighbours.class.getDeclaredMethod(
            "calculateNeighbours",
            ExpansionRoom.class,
            int.class,
            ShapeSearchTree.class,
            int.class);
    calculateNeighbours.setAccessible(true);
    fSortedNeighbours = SortedRoomNeighbours.class.getDeclaredField("sortedNeighbours");
    fSortedNeighbours.setAccessible(true);
    fOwnNetObjects = SortedRoomNeighbours.class.getDeclaredField("ownNetObjects");
    fOwnNetObjects.setAccessible(true);
    fCompletedRoom = SortedRoomNeighbours.class.getDeclaredField("completedRoom");
    fCompletedRoom.setAccessible(true);
    fIncompleteExpansionRooms =
        AutorouteEngine.class.getDeclaredField("incompleteExpansionRooms");
    fIncompleteExpansionRooms.setAccessible(true);
    outerCtor =
        SortedRoomNeighbours.class.getDeclaredConstructor(
            ExpansionRoom.class, CompleteExpansionRoom.class);
    outerCtor.setAccessible(true);

    neighbourClass = Class.forName("app.freerouting.autoroute.expansion.SortedRoomNeighbours$SortedRoomNeighbour");
    neighbourCtor = neighbourClass.getDeclaredConstructors()[0];
    neighbourCtor.setAccessible(true);
    fTouchingSideNoOfRoom = neighbourClass.getDeclaredField("touchingSideNoOfRoom");
    fTouchingSideNoOfRoom.setAccessible(true);
    fTouchingSideNoOfNeighbourRoom =
        neighbourClass.getDeclaredField("touchingSideNoOfNeighbourRoom");
    fTouchingSideNoOfNeighbourRoom.setAccessible(true);
    fRoomTouchIsCorner = neighbourClass.getDeclaredField("roomTouchIsCorner");
    fRoomTouchIsCorner.setAccessible(true);
    fNeighbourRoomTouchIsCorner = neighbourClass.getDeclaredField("neighbourRoomTouchIsCorner");
    fNeighbourRoomTouchIsCorner.setAccessible(true);
    fSearchTreeObject = neighbourClass.getDeclaredField("searchTreeObject");
    fSearchTreeObject.setAccessible(true);
    fNeighbourShape = neighbourClass.getDeclaredField("neighbourShape");
    fNeighbourShape.setAccessible(true);
    fIntersection = neighbourClass.getDeclaredField("intersection");
    fIntersection.setAccessible(true);
    mFirstCorner = neighbourClass.getDeclaredMethod("firstCorner");
    mFirstCorner.setAccessible(true);
    mLastCorner = neighbourClass.getDeclaredMethod("lastCorner");
    mLastCorner.setAccessible(true);

    calculateNeighbours45 =
        Sorted45DegreeRoomNeighbours.class.getDeclaredMethod(
            "calculateNeighbours",
            ExpansionRoom.class,
            int.class,
            ShapeSearchTree.class,
            int.class);
    calculateNeighbours45.setAccessible(true);
    f45SortedNeighbours = Sorted45DegreeRoomNeighbours.class.getDeclaredField("sortedNeighbours");
    f45SortedNeighbours.setAccessible(true);
    f45CompletedRoom = Sorted45DegreeRoomNeighbours.class.getDeclaredField("completedRoom");
    f45CompletedRoom.setAccessible(true);
    f45EdgeInterior =
        Sorted45DegreeRoomNeighbours.class.getDeclaredField("edgeInteriorTouchesObstacle");
    f45EdgeInterior.setAccessible(true);
    neighbour45Class =
        Class.forName(
            "app.freerouting.autoroute.expansion.Sorted45DegreeRoomNeighbours$SortedRoomNeighbour");
    f45SearchTreeObject = neighbour45Class.getDeclaredField("searchTreeObject");
    f45SearchTreeObject.setAccessible(true);
    f45Shape = neighbour45Class.getDeclaredField("shape");
    f45Shape.setAccessible(true);
    f45Intersection = neighbour45Class.getDeclaredField("intersection");
    f45Intersection.setAccessible(true);
    f45FirstTouchingSide = neighbour45Class.getDeclaredField("firstTouchingSide");
    f45FirstTouchingSide.setAccessible(true);
    f45LastTouchingSide = neighbour45Class.getDeclaredField("lastTouchingSide");
    f45LastTouchingSide.setAccessible(true);

    calculateNeighboursOrtho =
        SortedOrthogonalRoomNeighbours.class.getDeclaredMethod(
            "calculateNeighbours",
            ExpansionRoom.class,
            int.class,
            ShapeSearchTree.class,
            int.class);
    calculateNeighboursOrtho.setAccessible(true);
    fOrthoSortedNeighbours =
        SortedOrthogonalRoomNeighbours.class.getDeclaredField("sortedNeighbours");
    fOrthoSortedNeighbours.setAccessible(true);
    fOrthoCompletedRoom = SortedOrthogonalRoomNeighbours.class.getDeclaredField("completedRoom");
    fOrthoCompletedRoom.setAccessible(true);
    fOrthoEdgeInterior =
        SortedOrthogonalRoomNeighbours.class.getDeclaredField("edgeInteriorTouchesObstacle");
    fOrthoEdgeInterior.setAccessible(true);
    neighbourOrthoClass =
        Class.forName(
            "app.freerouting.autoroute.expansion.SortedOrthogonalRoomNeighbours$SortedRoomNeighbour");
    fOrthoSearchTreeObject = neighbourOrthoClass.getDeclaredField("searchTreeObject");
    fOrthoSearchTreeObject.setAccessible(true);
    fOrthoShape = neighbourOrthoClass.getDeclaredField("shape");
    fOrthoShape.setAccessible(true);
    fOrthoIntersection = neighbourOrthoClass.getDeclaredField("intersection");
    fOrthoIntersection.setAccessible(true);
    fOrthoFirstTouchingSide = neighbourOrthoClass.getDeclaredField("firstTouchingSide");
    fOrthoFirstTouchingSide.setAccessible(true);
    fOrthoLastTouchingSide = neighbourOrthoClass.getDeclaredField("lastTouchingSide");
    fOrthoLastTouchingSide.setAccessible(true);
  }

  // -----------------------------------------------------------------------------------------
  // The board — `P2T10.build`, verbatim, in the any-angle regime
  // -----------------------------------------------------------------------------------------

  static void build(int mode) {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    cm.appendClass("wide");
    cm.setValue(2, 1, 600);
    cm.setValue(2, 2, 800);
    BoardRules rules = new BoardRules(ls, cm);
    // Modes 6 and 8 drive the 45-degree tree, modes 7 and 9 the 90-degree one; every other mode
    // is any-angle. `SearchTreeManager.getAutorouteTree` picks the subclass from this value, and
    // `SortedRoomNeighbours.selectCalculationMode` then picks the sorter from the subclass.
    if (mode == 6 || mode == 8) {
      rules.setTraceAngleRestriction(AngleRestriction.FORTYFIVE_DEGREE);
    } else if (mode == 7 || mode == 9) {
      rules.setTraceAngleRestriction(AngleRestriction.NINETY_DEGREE);
    } else {
      rules.setTraceAngleRestriction(AngleRestriction.NONE);
    }
    Communication comm = new Communication();
    // A `RoutingBoard`, not a `BasicBoard`: mode 5 needs a real `AutorouteEngine`, whose
    // constructor takes one. Every other mode is unaffected — `RoutingBoard`'s overrides all
    // short-circuit on `this.autorouteEngine == null`, which `initAutoroute` is what sets.
    board = new RoutingBoard(BOUNDING_BOX, ls, new PolylineShape[0], 0, rules, comm);
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

  static void insertRandomObstacle(boolean grid) {
    Area shape = grid ? randomGridBox() : randomBox(RANGE, 100, 2500);
    int layer = rnd(2);
    board.insertObstacle(shape, layer, 1, FixedState.UNFIXED);
  }

  /** A box whose four ordinates are multiples of 500. */
  static IntBox randomGridBox() {
    int w = 500 * (1 + rnd(4));
    int h = 500 * (1 + rnd(4));
    int x = 500 * (rnd(33) - 16);
    int y = 500 * (rnd(33) - 16);
    return new IntBox(x, y, x + w, y + h);
  }

  static void insertSeedRooms(ShapeSearchTree tree, AutorouteEngine engine) {
    for (int i = 0; i < 3; i++) {
      IntBox box = randomBox(RANGE, 500, 3000);
      int layer = rnd(2);
      // In mode 5 the ids come from the engine's own counter, so that the counter the port keeps
      // in `ExpansionRoomStore` and Java's `expansionRoomInstanceCount` stay in lockstep.
      roomIdCounter = engine == null ? roomIdCounter + 1 : engine.generateRoomIdNo();
      CompleteFreeSpaceExpansionRoom room =
          new CompleteFreeSpaceExpansionRoom(box, layer, roomIdCounter);
      tree.insert(room);
      System.out.println(
          "  seedRoom " + roomIdCounter + " layer=" + layer + " shape=" + shp(box));
    }
    System.out.println("  treeSizeWithRooms=" + tree.size());
  }

  // -----------------------------------------------------------------------------------------
  // mode 0
  // -----------------------------------------------------------------------------------------

  @SuppressWarnings("unchecked")
  static void runOne(ShapeSearchTree tree, int index, int mode) throws Exception {
    int kind = rnd(4);
    int netNumber = 1 + rnd(3);
    int layer = rnd(2);
    IntBox containedBox = randomBox(RANGE, 10, 300);
    int itemId = rnd(4) == 0 ? 2 + rnd(4) : 4 + rnd(2);
    int roomIdNo = ++roomIdCounter;
    ExpansionRoom room;
    String description;
    if (kind == 3) {
      // An obstacle expansion room over a random tree shape of a random item, fetched exactly as
      // `SortedRoomNeighbours` itself fetches one (`:274-276`) so the item's autoroute info is
      // the single owner of it.
      Item item = board.getItem(itemId);
      int shapeCount = item.treeShapeCount(tree);
      if (shapeCount == 0) {
        System.out.println(
            "call i=" + index + " kind=obstacle item=" + itemId + " skipped=noShapes");
        return;
      }
      int indexInItem = rnd(shapeCount);
      room = item.getAutorouteInfo().getExpansionRoom(indexInItem, tree);
      description = "obstacle item=" + itemId + " indexInItem=" + indexInItem;
    } else {
      // The seed room the engine actually hands to `SortedRoomNeighbours`: the output of
      // `completeShape` over a whole-plane room around a small contained box, which is exactly
      // `AutorouteEngine.completeExpansionRoom` (`AutorouteEngine.java:449-450`). A random box is
      // useless here — it almost never *touches* anything, so every interesting branch of
      // `calculateNeighbours` stays dead.
      IncompleteFreeSpaceExpansionRoom seed =
          new IncompleteFreeSpaceExpansionRoom(null, layer, containedBox);
      Collection<IncompleteFreeSpaceExpansionRoom> completed =
          tree.completeShape(seed, netNumber, null, null);
      if (completed.isEmpty()) {
        System.out.println(
            "call i="
                + index
                + " kind=freeSpace layer="
                + layer
                + " contained="
                + shp(containedBox)
                + " skipped=noCompletedShape");
        return;
      }
      int pick = rnd(completed.size());
      IncompleteFreeSpaceExpansionRoom chosen = null;
      int seen = 0;
      for (IncompleteFreeSpaceExpansionRoom candidate : completed) {
        if (seen == pick) {
          chosen = candidate;
          break;
        }
        ++seen;
      }
      room = chosen;
      description =
          "freeSpace layer="
              + layer
              + " contained="
              + shp(containedBox)
              + " candidates="
              + completed.size()
              + " pick="
              + pick
              + " chosenContained="
              + shp(chosen.getContainedShape());
    }

    System.out.println(
        "call i="
            + index
            + " kind="
            + description
            + " net="
            + netNumber
            + " roomIdNo="
            + roomIdNo
            + " shape="
            + shp(room.getShape())
            + " roomLayer="
            + room.getLayer());

    if (mode == 6) {
      Object result = calculateNeighbours45.invoke(null, room, netNumber, tree, roomIdNo);
      if (result == null) {
        System.out.println("  result=null");
        return;
      }
      dumpRegimeNeighbours(
          (SortedSet<Object>) f45SortedNeighbours.get(result),
          f45FirstTouchingSide,
          f45LastTouchingSide,
          f45SearchTreeObject,
          f45Shape,
          f45Intersection);
      System.out.println("  edgeTouches=" + flags((boolean[]) f45EdgeInterior.get(result)));
      CompleteExpansionRoom completedRoom = (CompleteExpansionRoom) f45CompletedRoom.get(result);
      System.out.println("  completedRoom=" + desc(completedRoom));
      dumpDoors(completedRoom.getDoors());
      dumpTargetDoors(completedRoom);
      return;
    }
    if (mode == 7) {
      Object result = calculateNeighboursOrtho.invoke(null, room, netNumber, tree, roomIdNo);
      if (result == null) {
        System.out.println("  result=null");
        return;
      }
      dumpRegimeNeighbours(
          (SortedSet<Object>) fOrthoSortedNeighbours.get(result),
          fOrthoFirstTouchingSide,
          fOrthoLastTouchingSide,
          fOrthoSearchTreeObject,
          fOrthoShape,
          fOrthoIntersection);
      System.out.println("  edgeTouches=" + flags((boolean[]) fOrthoEdgeInterior.get(result)));
      CompleteExpansionRoom completedRoom = (CompleteExpansionRoom) fOrthoCompletedRoom.get(result);
      System.out.println("  completedRoom=" + desc(completedRoom));
      dumpDoors(completedRoom.getDoors());
      dumpTargetDoors(completedRoom);
      return;
    }
    Object result = calculateNeighbours.invoke(null, room, netNumber, tree, roomIdNo);
    if (result == null) {
      System.out.println("  result=null");
      return;
    }
    dumpNeighbours(result);
    dumpOwnNetObjects(result);
    CompleteExpansionRoom completedRoom = (CompleteExpansionRoom) fCompletedRoom.get(result);
    System.out.println("  completedRoom=" + desc(completedRoom));
    dumpDoors(completedRoom.getDoors());
  }

  /**
   * The sorted neighbour list of one of the two angle-restricted sorters. Their inner {@code
   * SortedRoomNeighbour}s are different classes from the base one and from each other — no corner
   * flags, no memoized corners, a first *and* a last touching side — so this dump is not {@code
   * describeNeighbour}'s.
   */
  static void dumpRegimeNeighbours(
      SortedSet<Object> neighbours,
      Field first,
      Field last,
      Field object,
      Field shape,
      Field intersection)
      throws Exception {
    System.out.println("  neighbours n=" + neighbours.size());
    int i = 0;
    for (Object n : neighbours) {
      TileShape neighbourShape = (TileShape) shape.get(n);
      TileShape isect = (TileShape) intersection.get(n);
      System.out.println(
          "    ["
              + i
              + "] fts="
              + first.getInt(n)
              + " lts="
              + last.getInt(n)
              + " obj="
              + describeObject((SearchTreeObject) object.get(n))
              + " nshape="
              + shp(neighbourShape)
              + " nshapeCorners="
              + corners(neighbourShape)
              + " isect="
              + shp(isect)
              + " isectCorners="
              + corners(isect));
      ++i;
    }
  }

  static String flags(boolean[] values) {
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < values.length; i++) {
      if (i > 0) {
        sb.append(',');
      }
      sb.append(values[i]);
    }
    return sb.append(']').toString();
  }

  static void dumpTargetDoors(CompleteExpansionRoom room) {
    Collection<TargetItemExpansionDoor> targetDoors = room.getTargetDoors();
    System.out.println("  targetDoors n=" + targetDoors.size());
    int t = 0;
    for (TargetItemExpansionDoor door : targetDoors) {
      System.out.println(
          "    ["
              + t
              + "] item="
              + door.item.getId()
              + " entry="
              + door.treeEntryNo
              + " dim="
              + door.getDimension()
              + " shape="
              + shp(door.getShape()));
      ++t;
    }
  }

  /**
   * mode 5: the **whole** of {@code SortedRoomNeighbours.complete} against a real {@code
   * AutorouteEngine} — `tryRemoveEdge` and its `completeShape` retry, `calculateNewIncompleteRooms`
   * (150 lines of corner-cutting geometry), `calculateIncompleteRoomsWithEmptyNeighbours` and
   * `calculateTargetDoors`, none of which mode 0 reaches. The port has no engine (that is Task 6),
   * so it drives the same five services out of its `ExpansionRoomStore`; what is compared is the
   * engine-visible result: the completed room, its doors and target doors, the seed room's shape
   * after `tryRemoveEdge` may have replaced it, and every incomplete expansion room the call left
   * on the engine's list.
   */
  static void runOneComplete(ShapeSearchTree tree, AutorouteEngine engine, int index, int mode)
      throws Exception {
    int kind = rnd(4);
    int netNumber = 1 + rnd(3);
    int layer = rnd(2);
    IntBox containedBox = randomBox(RANGE, 10, 300);
    int itemId = rnd(4) == 0 ? 2 + rnd(4) : 4 + rnd(2);
    ExpansionRoom room;
    String description;
    if (kind == 3) {
      Item item = board.getItem(itemId);
      int shapeCount = item.treeShapeCount(tree);
      if (shapeCount == 0) {
        System.out.println(
            "call i=" + index + " kind=obstacle item=" + itemId + " skipped=noShapes");
        return;
      }
      int indexInItem = rnd(shapeCount);
      room = item.getAutorouteInfo().getExpansionRoom(indexInItem, tree);
      description = "obstacle item=" + itemId + " indexInItem=" + indexInItem;
    } else {
      IncompleteFreeSpaceExpansionRoom seed =
          new IncompleteFreeSpaceExpansionRoom(null, layer, containedBox);
      Collection<IncompleteFreeSpaceExpansionRoom> completed =
          tree.completeShape(seed, netNumber, null, null);
      if (completed.isEmpty()) {
        System.out.println(
            "call i="
                + index
                + " kind=freeSpace layer="
                + layer
                + " contained="
                + shp(containedBox)
                + " skipped=noCompletedShape");
        return;
      }
      int pick = rnd(completed.size());
      IncompleteFreeSpaceExpansionRoom chosen = null;
      int seen = 0;
      for (IncompleteFreeSpaceExpansionRoom candidate : completed) {
        if (seen == pick) {
          chosen = candidate;
          break;
        }
        ++seen;
      }
      room = chosen;
      description =
          "freeSpace layer="
              + layer
              + " contained="
              + shp(containedBox)
              + " candidates="
              + completed.size()
              + " pick="
              + pick;
    }

    // **Quirk: `calculateNewIncompleteRooms` does not terminate** when the room's shape has more
    // border lines than its `toSimplex()` does. `:512` builds `roomSimplex =
    // this.fromRoom.getShape().toSimplex()` — and `Simplex.getInstance` drops redundant lines —
    // while `touchingSideNoOfRoom` was computed against the *un-simplified* shape at `:254` /
    // `:289-297`. If `firstTouchingSideNo` then names a line the simplex does not have, the
    // `for (;;)` at `:562` walks `prevNo` round the simplex for ever, allocating an
    // `IncompleteFreeSpaceExpansionRoom` per turn until the JVM runs out of heap. Seed 42 reaches
    // it at `i=124`. The driver skips those calls on **both** sides rather than tolerating a
    // difference; the port reproduces the loop, and `crates/fr-router/tests/sorted_neighbours.rs`
    // pins it with a bounded assertion instead.
    // Only the any-angle base class walks `toSimplex()` (`SortedRoomNeighbours.java:512`); the
    // two angle-restricted sorters index their own octagon / box sides, so quirk #162 cannot
    // arise in modes 8 and 9 and they are not skipped.
    TileShape fromShape = room.getShape();
    if (mode == 5 && fromShape.borderLineCount() != fromShape.toSimplex().borderLineCount()) {
      System.out.println(
          "call i="
              + index
              + " kind="
              + description
              + " net="
              + netNumber
              + " skipped=simplexSideCountDiffers borderLines="
              + fromShape.borderLineCount()
              + " simplexLines="
              + fromShape.toSimplex().borderLineCount());
      return;
    }

    engine.initConnection(netNumber, null, null);
    System.out.println(
        "call i="
            + index
            + " kind="
            + description
            + " net="
            + netNumber
            + " shape="
            + shp(room.getShape())
            + " roomLayer="
            + room.getLayer());

    CompleteExpansionRoom result;
    try {
      result = SortedRoomNeighbours.complete(room, engine);
    } catch (RuntimeException e) {
      System.out.println("  threw=" + e.getClass().getSimpleName());
      return;
    }
    System.out.println(
        "  result="
            + desc(result)
            + " shape="
            + shp(result.getShape())
            + " corners="
            + corners(result.getShape()));
    System.out.println(
        "  fromRoom=" + desc(room) + " shape=" + shp(room.getShape()));
    dumpDoors(result.getDoors());
    dumpTargetDoors(result);
    dumpIncompleteRooms(engine);
    // Reset the engine's incomplete-room list between calls. Nothing consumes it here — a real
    // run drains it through `AutorouteEngine.completeExpansionRoom` — so leaving it to grow makes
    // the dump quadratic and the JVM run out of heap. An **empty list** rather than `null`, so
    // that a later `removeIncompleteExpansionRoom` cannot NPE on it.
    fIncompleteExpansionRooms.set(engine, new java.util.ArrayList<IncompleteFreeSpaceExpansionRoom>());
  }

  @SuppressWarnings("unchecked")
  static void dumpIncompleteRooms(AutorouteEngine engine) throws Exception {
    java.util.List<IncompleteFreeSpaceExpansionRoom> incomplete =
        (java.util.List<IncompleteFreeSpaceExpansionRoom>) fIncompleteExpansionRooms.get(engine);
    int n = incomplete == null ? 0 : incomplete.size();
    System.out.println("  incompleteRooms n=" + n);
    for (int i = 0; i < n; i++) {
      IncompleteFreeSpaceExpansionRoom current = incomplete.get(i);
      System.out.println(
          "    ["
              + i
              + "] layer="
              + current.getLayer()
              + " shape="
              + shp(current.getShape())
              + " corners="
              + corners(current.getShape())
              + " contained="
              + shp(current.getContainedShape())
              + " doors="
              + current.getDoors().size());
    }
  }

  @SuppressWarnings("unchecked")
  static void dumpNeighbours(Object sorter) throws Exception {
    SortedSet<Object> neighbours = (SortedSet<Object>) fSortedNeighbours.get(sorter);
    System.out.println("  neighbours n=" + neighbours.size());
    int i = 0;
    for (Object n : neighbours) {
      System.out.println("    [" + i + "] " + describeNeighbour(n));
      ++i;
    }
  }

  static String describeNeighbour(Object n) throws Exception {
    SearchTreeObject object = (SearchTreeObject) fSearchTreeObject.get(n);
    TileShape neighbourShape = (TileShape) fNeighbourShape.get(n);
    TileShape intersection = (TileShape) fIntersection.get(n);
    return "tsr="
        + fTouchingSideNoOfRoom.getInt(n)
        + " tsn="
        + fTouchingSideNoOfNeighbourRoom.getInt(n)
        + " rtc="
        + fRoomTouchIsCorner.getBoolean(n)
        + " ntc="
        + fNeighbourRoomTouchIsCorner.getBoolean(n)
        + " obj="
        + describeObject(object)
        + " first="
        + pt((Point) mFirstCorner.invoke(n))
        + " last="
        + pt((Point) mLastCorner.invoke(n))
        + " nshape="
        + shp(neighbourShape)
        + " nshapeCorners="
        + corners(neighbourShape)
        + " isect="
        + shp(intersection)
        + " isectCorners="
        + corners(intersection);
  }

  @SuppressWarnings("unchecked")
  static void dumpOwnNetObjects(Object sorter) throws Exception {
    Collection<ShapeTree.TreeEntry> entries =
        (Collection<ShapeTree.TreeEntry>) fOwnNetObjects.get(sorter);
    System.out.println("  ownNet n=" + entries.size());
    int i = 0;
    for (ShapeTree.TreeEntry entry : entries) {
      System.out.println(
          "    ["
              + i
              + "] obj="
              + describeObject((SearchTreeObject) entry.object)
              + " idx="
              + entry.shapeIndexInObject);
      ++i;
    }
  }

  static void dumpDoors(List<ExpansionDoor> doors) {
    System.out.println("  doors n=" + doors.size());
    int i = 0;
    for (ExpansionDoor door : doors) {
      TileShape shape = door.getShape();
      System.out.println(
          "    ["
              + i
              + "] first="
              + desc(door.firstRoom)
              + " second="
              + desc(door.secondRoom)
              + " dim="
              + door.dimension
              + " shape="
              + shp(shape)
              + " corners="
              + corners(shape));
      ++i;
    }
  }

  // -----------------------------------------------------------------------------------------
  // mode 1 — the hazard-F comparator probe
  // -----------------------------------------------------------------------------------------

  static void comparatorProbe(int caseCount, boolean stress) throws Exception {
    for (int c = 0; c < caseCount; c++) {
      IntBox roomBox = randomBox(2000, 400, 2000);
      IncompleteFreeSpaceExpansionRoom fromRoom =
          new IncompleteFreeSpaceExpansionRoom(roomBox, 0, roomBox);
      CompleteFreeSpaceExpansionRoom completedRoom =
          new CompleteFreeSpaceExpansionRoom(roomBox, 0, 1);
      Object sorter = outerCtor.newInstance(fromRoom, completedRoom);

      int count = 2 + rnd(4);
      // In stress mode every neighbour of a probe sits on the **same** side of the room, so the
      // comparator's first key always ties and the refinements below it — the last-corner
      // distance, the `compareFrom` direction and the id — are what decide. That is where the
      // comparator stops being a total order (hazard F): a neighbour with `roomTouchIsCorner`
      // has both corners equal to the room's own corner, so a mixed triple can compare one pair
      // by direction and the other two pairs by id, and those three answers can form a cycle.
      int fixedSide = rnd(4);
      System.out.println(
          "probe c=" + c + " room=" + shp(roomBox) + " n=" + count + " stress=" + stress);
      TreeSet<Object> set = new TreeSet<>();
      for (int i = 0; i < count; i++) {
        // Neighbour shapes that really touch the room box on one of its four sides, so the
        // corner computations are the real ones rather than degenerate.
        int side = stress ? fixedSide : rnd(4);
        int span = 20 + rnd(400);
        int offset = rnd(Math.max(1, roomBox.ur.x - roomBox.ll.x - span));
        int offsetY = rnd(Math.max(1, roomBox.ur.y - roomBox.ll.y - span));
        IntBox neighbourBox;
        if (side == 0) {
          neighbourBox =
              new IntBox(
                  roomBox.ll.x + offset, roomBox.ll.y - 300, roomBox.ll.x + offset + span,
                  roomBox.ll.y);
        } else if (side == 1) {
          neighbourBox =
              new IntBox(
                  roomBox.ur.x, roomBox.ll.y + offsetY, roomBox.ur.x + 300,
                  roomBox.ll.y + offsetY + span);
        } else if (side == 2) {
          neighbourBox =
              new IntBox(
                  roomBox.ll.x + offset, roomBox.ur.y, roomBox.ll.x + offset + span,
                  roomBox.ur.y + 300);
        } else {
          neighbourBox =
              new IntBox(
                  roomBox.ll.x - 300, roomBox.ll.y + offsetY, roomBox.ll.x,
                  roomBox.ll.y + offsetY + span);
        }
        TileShape intersection = roomBox.intersection(neighbourBox);
        int[] touchingSides = roomBox.touchingSides(neighbourBox);
        int tsr = touchingSides.length == 2 ? touchingSides[0] : 0;
        int tsn = touchingSides.length == 2 ? touchingSides[1] : 0;
        boolean rtc = stress ? rnd(2) == 0 : rnd(4) == 0;
        boolean ntc = stress ? rnd(2) == 0 : rnd(3) == 0;
        SearchTreeObject object =
            new CompleteFreeSpaceExpansionRoom(neighbourBox, 0, stress ? 1 + rnd(4) : 1 + rnd(6));
        Object neighbour =
            neighbourCtor.newInstance(
                sorter, object, neighbourBox, intersection, tsr, tsn, rtc, ntc);
        boolean added;
        try {
          added = set.add(neighbour);
        } catch (RuntimeException e) {
          System.out.println("    add[" + i + "] threw " + e.getClass().getSimpleName());
          continue;
        }
        System.out.println(
            "    add["
                + i
                + "] added="
                + added
                + " size="
                + set.size()
                + " "
                + describeNeighbour(neighbour));
      }
      System.out.println("  survivors n=" + set.size());
      int j = 0;
      for (Object n : set) {
        System.out.println("    [" + j + "] " + describeNeighbour(n));
        ++j;
      }
    }
  }

  /**
   * The narrowest hazard-F probe: every neighbour has {@code roomTouchIsCorner}, so {@code
   * firstCorner()} and {@code lastCorner()} are both the room's own corner {@code
   * touchingSideNoOfRoom} and every distance delta is exactly 0. What is left to decide the order
   * is the {@code Direction.compareFrom} branch — reached only when *both* neighbours have {@code
   * neighbourRoomTouchIsCorner} — and otherwise the id difference. Mixing the two is what makes
   * the comparator non-transitive: a pair compared by direction can disagree with the two pairs
   * compared by id, and the three answers then form a cycle.
   *
   * <p>{@code touchingSideNoOfNeighbourRoom} is varied independently of the geometry, exactly as
   * the constructor allows — {@code SortedRoomNeighbour} stores whatever {@code
   * calculateNeighbours} hands it and only {@code borderLine(tsn)}'s *direction* is read here.
   */
  static void cornerTouchProbe(int caseCount) throws Exception {
    for (int c = 0; c < caseCount; c++) {
      IntBox roomBox = randomBox(2000, 400, 2000);
      IncompleteFreeSpaceExpansionRoom fromRoom =
          new IncompleteFreeSpaceExpansionRoom(roomBox, 0, roomBox);
      CompleteFreeSpaceExpansionRoom completedRoom =
          new CompleteFreeSpaceExpansionRoom(roomBox, 0, 1);
      Object sorter = outerCtor.newInstance(fromRoom, completedRoom);
      int count = 3 + rnd(3);
      System.out.println("corner c=" + c + " room=" + shp(roomBox) + " n=" + count);
      TreeSet<Object> set = new TreeSet<>();
      for (int i = 0; i < count; i++) {
        IntBox neighbourBox = randomBox(3000, 100, 1500);
        TileShape intersection = roomBox.intersection(neighbourBox);
        int tsr = rnd(4);
        int tsn = rnd(4);
        boolean ntc = rnd(2) == 0;
        SearchTreeObject object =
            new CompleteFreeSpaceExpansionRoom(neighbourBox, 0, 1 + rnd(5));
        Object neighbour =
            neighbourCtor.newInstance(
                sorter, object, neighbourBox, intersection, tsr, tsn, true, ntc);
        boolean added = set.add(neighbour);
        System.out.println(
            "    add["
                + i
                + "] added="
                + added
                + " size="
                + set.size()
                + " "
                + describeNeighbour(neighbour));
      }
      System.out.println("  survivors n=" + set.size());
      int j = 0;
      for (Object n : set) {
        System.out.println("    [" + j + "] " + describeNeighbour(n));
        ++j;
      }
    }
  }

  // -----------------------------------------------------------------------------------------
  // Formatting
  // -----------------------------------------------------------------------------------------

  static String describeObject(SearchTreeObject object) {
    if (object == null) {
      return "null";
    }
    if (object instanceof Item item) {
      return "item" + item.getId();
    }
    if (object instanceof CompleteFreeSpaceExpansionRoom room) {
      return "cfsr" + room.getId();
    }
    return object.getClass().getSimpleName() + object.getId();
  }

  static String desc(ExpansionRoom room) {
    if (room == null) {
      return "null";
    }
    if (room instanceof CompleteFreeSpaceExpansionRoom r) {
      return "cfsr" + r.getId();
    }
    if (room instanceof ObstacleExpansionRoom r) {
      return "obs" + r.getItem().getId() + "/" + r.getIndexInItem();
    }
    if (room instanceof IncompleteFreeSpaceExpansionRoom r) {
      return "inc" + r.getId();
    }
    return room.getClass().getSimpleName();
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
              + " routable="
              + it.isRoutable()
              + " bbox="
              + b(it.boundingBox()));
    }
  }

  static String pt(Point p) {
    if (p == null) {
      return "null";
    }
    FloatPoint f = p.toFloat();
    return "(" + Double.toString(f.x) + "," + Double.toString(f.y) + ")";
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
