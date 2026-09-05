// Plan 6 Task 9 ground-truth probe: `RoutingBoard`'s five autoroute-facing methods, the
// check-only half of `board.optimize.TraceShover` and `board.actions.DrillItemMover.check` /
// `.tryShoveViaPoints`, on the clone's HEAD jar. It is not a differential driver — there is no
// Rust twin and `run.sh` does not know it — but every literal in
// `crates/fr-router/tests/board_ext.rs` is read off its stdout, so it is committed here to keep
// those numbers reproducible.
//
// It declares `package app.freerouting.board.optimize` so it can call `TraceShover`'s
// package-private `getIgnoreItemsAtTiePins` (TraceShover.java:592-603), which no public method
// exposes; `AutorouteEngine`'s three private room lists are read by reflection instead, exactly
// as `P6T6Probe` reads them. It compiles against the clone's HEAD jar exactly as `run.sh`'s jar
// mode does:
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential/java/probes
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p6t9 P6T9Probe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p6t9:$JAR" \
//       app.freerouting.board.optimize.P6T9Probe <mode>
//
// Modes:
//   upd    `RoutingBoard.initAutoroute` -> `AutorouteEngine.initConnection` **with an item on the
//          new net**, which is the re-run `task-6-report.md` §8.1 asks for: it is the only mode
//          that exercises `initConnection:111-117` -> `additionalUpdateAfterChange`. Prints the
//          engine before and after, and a second run whose engine is reused/rebuilt so
//          `initAutoroute:888-891`'s three-way guard is pinned too.
//   poly   `RoutingBoard.checkForcedTracePolyline` (:408-448) over a grid of probe polylines, in
//          both the any-angle and the 90-degree regime.
//   seg    the **static** `TraceShover.check(RoutingBoard, LineSegment, ...)` (:57-229) over the
//          same grid, which answers a double (`Integer.MAX_VALUE` on complete success).
//   inst   the **instance** `TraceShover.check(TileShape, ShapeEntrySide, ...)` (:231-411) at a
//          ladder of `maxRecursionDepth` values, which is where the hard-coded
//          `maxShoveTraceRecursionDepth = 20` of `AutorouteControl` bites.
//   drill  `DrillItemMover.check` (:34-103) over its three answerable arms, and
//          `DrillItemMover.tryShoveViaPoints` (:256-325) in both angle regimes.
//   tie    `TraceShover.getIgnoreItemsAtTiePins` (:592-603).
package app.freerouting.board.optimize;

import app.freerouting.autoroute.expansion.CompleteFreeSpaceExpansionRoom;
import app.freerouting.autoroute.expansion.IncompleteFreeSpaceExpansionRoom;
import app.freerouting.autoroute.maze.AutorouteEngine;
import app.freerouting.board.actions.DrillItemMover;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Via;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.model.structure.Layer;
import app.freerouting.board.model.structure.LayerStructure;
import app.freerouting.board.model.structure.ShapeEntrySide;
import app.freerouting.board.searchtree.ShapeSearchTree;
import app.freerouting.board.state.Communication;
import app.freerouting.core.library.Package;
import app.freerouting.core.library.Padstack;
import app.freerouting.geometry.planar.ConvexShape;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.IntOctagon;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.IntVector;
import app.freerouting.geometry.planar.LineSegment;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import app.freerouting.geometry.planar.PolylineShape;
import app.freerouting.geometry.planar.Shape;
import app.freerouting.geometry.planar.TileShape;
import app.freerouting.geometry.planar.Vector;
import app.freerouting.rules.BoardRules;
import app.freerouting.rules.ClearanceMatrix;
import java.lang.reflect.Field;
import java.util.Collection;
import java.util.LinkedList;
import java.util.List;

/** Task 9 ground truth: RoutingBoardExt, TraceShover.check and DrillItemMover.check. */
public class P6T9Probe {

  static final IntBox BOUNDING_BOX = new IntBox(-10000, -10000, 10000, 10000);
  static RoutingBoard board;
  static Padstack thruPad;

  public static void main(String[] args) throws Exception {
    String mode = args.length > 0 ? args[0] : "upd";
    switch (mode) {
      case "upd" -> update();
      case "poly" -> forcedTracePolyline();
      case "seg" -> staticCheck();
      case "inst" -> instanceCheck();
      case "drill" -> drillItemMover();
      case "tie" -> tiePins();
      default -> throw new IllegalArgumentException("mode");
    }
  }

  // --- the board: `P6T7Probe.build`, which is `P6T3.build`'s any-angle board, verbatim ---------

  static void build(AngleRestriction angleRestriction) {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    cm.appendClass("wide");
    cm.setValue(2, 1, 600);
    cm.setValue(2, 2, 800);
    BoardRules rules = new BoardRules(ls, cm);
    rules.setTraceAngleRestriction(angleRestriction);
    Communication comm = new Communication();
    board =
        new RoutingBoard(BOUNDING_BOX, ls, new PolylineShape[0], 0, rules, comm);
    board.library.padstacks = new app.freerouting.core.library.Padstacks(ls);
    board.library.packages = new app.freerouting.core.library.Packages(board.library.padstacks);

    ConvexShape[] smd = {new IntBox(-50, -50, 50, 50), null};
    Padstack smdPad = board.library.padstacks.add("smd", smd, false, false);
    ConvexShape[] thru = {
      new IntOctagon(-70, -70, 70, 70, -140, 140, -140, 140),
      new IntOctagon(-70, -70, 70, 70, -140, 140, -140, 140)
    };
    thruPad = board.library.padstacks.add("thru", thru, true, false);

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

    // `Trace.isShoveFixed` (Trace.java:244-250) dereferences `rules.nets.get(netNo)`, so every
    // net a trace names has to exist before anything shoves.
    board.rules.nets.add("N1", 1, false);
    board.rules.nets.add("N2", 1, false);
    board.rules.nets.add("N3", 1, false);

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

  // --- dumps ----------------------------------------------------------------------------------

  static String shp(TileShape s) {
    if (s == null) {
      return "null";
    }
    if (s.isEmpty()) {
      return "empty";
    }
    IntBox b = s.boundingBox();
    return s.getClass().getSimpleName()
        + "["
        + b.ll.x
        + ","
        + b.ll.y
        + ".."
        + b.ur.x
        + ","
        + b.ur.y
        + "]dim="
        + s.dimension();
  }

  @SuppressWarnings("unchecked")
  static void dumpEngine(AutorouteEngine engine, String tag) throws Exception {
    Field fc = AutorouteEngine.class.getDeclaredField("completeExpansionRooms");
    fc.setAccessible(true);
    Field fi = AutorouteEngine.class.getDeclaredField("incompleteExpansionRooms");
    fi.setAccessible(true);
    Field fn = AutorouteEngine.class.getDeclaredField("expansionRoomInstanceCount");
    fn.setAccessible(true);
    List<CompleteFreeSpaceExpansionRoom> complete =
        (List<CompleteFreeSpaceExpansionRoom>) fc.get(engine);
    List<IncompleteFreeSpaceExpansionRoom> incomplete =
        (List<IncompleteFreeSpaceExpansionRoom>) fi.get(engine);
    System.out.println(
        tag
            + " counter="
            + fn.get(engine)
            + " complete="
            + (complete == null ? "null" : complete.size())
            + " incomplete="
            + (incomplete == null ? "null" : incomplete.size())
            + " treeSize="
            + engine.autorouteSearchTree.size()
            + " compensatedCl="
            + engine.autorouteSearchTree.compensatedClearanceClassNo);
    if (complete != null) {
      for (CompleteFreeSpaceExpansionRoom r : complete) {
        System.out.println(
            "    complete id="
                + r.getId()
                + " layer="
                + r.getLayer()
                + " shape="
                + shp(r.getShape())
                + " netDependent="
                + r.isNetDependent()
                + " doors="
                + r.getDoors().size());
      }
    }
  }

  // --- mode `upd` -----------------------------------------------------------------------------

  /**
   * `RoutingBoard.initAutoroute` with an item on the new net — the re-run `task-6-report.md` §8.1
   * asks for. Net 2 owns the second trace of the probe board, so `initConnection:111-117` reaches
   * `additionalUpdateAfterChange` for a real item and the completed rooms it overlaps go away.
   *
   * <p>Note that `new AutorouteEngine(board, ...)` does **not** install the engine on the board;
   * only `initAutoroute` does (RoutingBoard.java:892), and `additionalUpdateAfterChange:100`
   * returns at once while `board.autorouteEngine == null`. So this mode goes through
   * `initAutoroute`, which is also the only production caller of the constructor.
   */
  static void update() throws Exception {
    buildBare();
    System.out.println("mode=upd");
    AutorouteEngine engine = board.initAutoroute(1, 1, null, null, true);
    System.out.println("engineIsInstalled=" + (installedEngine() == engine));
    IncompleteFreeSpaceExpansionRoom seed =
        engine.addIncompleteExpansionRoom(null, 0, new IntBox(-600, -100, -400, 100));
    Collection<CompleteFreeSpaceExpansionRoom> result = engine.completeExpansionRoom(seed);
    System.out.println("completed n=" + result.size());
    dumpEngine(engine, "afterComplete");

    // The same clearance class, so `initAutoroute:888-891` reuses the engine and only
    // `initConnection` runs. Net 2 has an item, so the `:111-117` loop bites.
    AutorouteEngine reused = board.initAutoroute(2, 1, null, null, true);
    System.out.println("reusedTheEngine=" + (reused == engine));
    dumpEngine(reused, "afterInitNet2");

    // A different clearance class rebuilds the engine even with retain = true (`:890-891`).
    AutorouteEngine rebuilt = board.initAutoroute(2, 2, null, null, true);
    System.out.println("rebuiltOnClassChange=" + (rebuilt != reused));
    dumpEngine(rebuilt, "afterClassChange");

    // retain = false always rebuilds (`:889`).
    AutorouteEngine never = board.initAutoroute(2, 2, null, null, false);
    System.out.println("rebuiltWhenRetainIsFalse=" + (never != rebuilt));

    board.finishAutoroute();
    System.out.println("afterFinish engineIsNull=" + (installedEngine() == null));
  }

  /**
   * `P6T6Probe.buildBare` + `addTrace`, plus a **second trace on net 2** — which is the whole
   * point of this mode: `P6T6Probe` mode 3's net 2 had no items, so its `initConnection(2)` never
   * reached `additionalUpdateAfterChange` and `task-6-report.md` §8.1 asks for this re-run.
   */
  static void buildBare() {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    cm.appendClass("wide");
    cm.setValue(2, 1, 600);
    cm.setValue(2, 2, 800);
    BoardRules rules = new BoardRules(ls, cm);
    rules.setTraceAngleRestriction(AngleRestriction.NONE);
    board = new RoutingBoard(BOUNDING_BOX, ls, new PolylineShape[0], 0, rules, new Communication());
    board.library.padstacks = new app.freerouting.core.library.Padstacks(ls);
    board.library.packages = new app.freerouting.core.library.Packages(board.library.padstacks);
    board.rules.nets.add("N1", 1, false);
    board.rules.nets.add("N2", 1, false);
    // Only net 2 has an item, so **no** completed room can be net dependent and the removals
    // below can only have come from `initConnection:111-117`.
    Polyline net2 =
        new Polyline(new Point[] {new IntPoint(0, -400), new IntPoint(0, 400)});
    board.insertTraceWithoutCleaning(net2, 0, 30, new int[] {2}, 1, FixedState.UNFIXED);
  }

  // --- the probe grid -------------------------------------------------------------------------

  /** The polylines every shove mode is driven with, as `{name, corners...}`. */
  static Point[][] probeLines() {
    return new Point[][] {
      // straight across the net-1 trace's horizontal leg
      {new IntPoint(-300, -600), new IntPoint(-300, 600)},
      // across the same leg, but ending inside it
      {new IntPoint(-300, -600), new IntPoint(-300, 0)},
      // across the net-2 trace's vertical leg
      {new IntPoint(-1200, 600), new IntPoint(-400, 600)},
      // through the through-hole pin at (500, 0)
      {new IntPoint(500, -600), new IntPoint(500, 600)},
      // free space
      {new IntPoint(2000, 2000), new IntPoint(3000, 2000)},
      // three corners, crossing both traces
      {new IntPoint(-1200, 600), new IntPoint(-300, 600), new IntPoint(-300, -600)},
      // outside the board
      {new IntPoint(9800, 9800), new IntPoint(12000, 9800)},
    };
  }

  static String nameOf(int i) {
    return switch (i) {
      case 0 -> "acrossNet1";
      case 1 -> "intoNet1";
      case 2 -> "acrossNet2";
      case 3 -> "throughPin";
      case 4 -> "freeSpace";
      case 5 -> "twoSegments";
      case 6 -> "offBoard";
      default -> "?";
    };
  }

  // --- mode `poly` ----------------------------------------------------------------------------

  static void forcedTracePolyline() {
    for (AngleRestriction ar : new AngleRestriction[] {AngleRestriction.NONE, AngleRestriction.NINETY_DEGREE}) {
      build(ar);
      System.out.println("mode=poly angle=" + ar);
      Point[][] lines = probeLines();
      for (int i = 0; i < lines.length; i++) {
        Polyline p = new Polyline(lines[i]);
        for (int halfWidth : new int[] {30, 120}) {
          boolean ok =
              board.checkForcedTracePolyline(
                  p, halfWidth, 0, new int[] {3}, 1, 20, 5, 20);
          System.out.println(
              "  " + nameOf(i) + " halfWidth=" + halfWidth + " check=" + ok);
        }
      }
    }
  }

  // --- mode `seg` -----------------------------------------------------------------------------

  static void staticCheck() {
    build(AngleRestriction.NONE);
    System.out.println("mode=seg");
    Point[][] lines = probeLines();
    for (int i = 0; i < lines.length; i++) {
      if (lines[i].length != 2) {
        continue;
      }
      Polyline p = new Polyline(lines[i]);
      LineSegment segment = new LineSegment(p, 1);
      for (boolean left : new boolean[] {false, true}) {
        for (int halfWidth : new int[] {30, 120}) {
          double result =
              TraceShover.check(board, segment, left, 0, new int[] {3}, halfWidth, 1, 20, 5);
          System.out.println(
              "  "
                  + nameOf(i)
                  + " left="
                  + left
                  + " halfWidth="
                  + halfWidth
                  + " result="
                  + fmt(result));
        }
      }
    }
  }

  static String fmt(double d) {
    if (d == Integer.MAX_VALUE) {
      return "MAX";
    }
    return String.format(java.util.Locale.ROOT, "%.6f", d);
  }

  // --- mode `inst` ----------------------------------------------------------------------------

  static void instanceCheck() {
    build(AngleRestriction.NONE);
    System.out.println("mode=inst");
    TraceShover shover = new TraceShover(board);
    Point[][] lines = probeLines();
    for (int i = 0; i < lines.length; i++) {
      Polyline p = new Polyline(lines[i]);
      TileShape[] shapes = p.offsetShapes(120, 0, p.lines.length - 1);
      for (int s = 0; s < shapes.length; s++) {
        ShapeEntrySide fromSide = new ShapeEntrySide(p, s + 1, shapes[s]);
        for (int depth : new int[] {0, 1, 2, 20}) {
          boolean ok =
              shover.check(shapes[s], fromSide, null, 0, new int[] {3}, 1, depth, 5, 20, null);
          System.out.println(
              "  "
                  + nameOf(i)
                  + " shape="
                  + s
                  + " maxRecursionDepth="
                  + depth
                  + " check="
                  + ok
                  + " failing="
                  + idOf(board.getShoveFailingObstacle()));
        }
        // the spring-over budget, at the same recursion depth
        for (int springOver : new int[] {0, 1, 20}) {
          boolean ok =
              shover.check(shapes[s], fromSide, null, 0, new int[] {3}, 1, 20, 5, springOver, null);
          System.out.println(
              "  " + nameOf(i) + " shape=" + s + " maxSpringOver=" + springOver + " check=" + ok);
        }
      }
    }
  }

  static String idOf(Item item) {
    return item == null ? "none" : Integer.toString(item.getId());
  }

  // --- mode `drill` ---------------------------------------------------------------------------

  /**
   * `DrillItemMover.check`'s three answerable arms — `isShoveFixed` (:46-48), a contact that is
   * neither a trace nor a conduction area (:51-56), and the full run through
   * `ForcedPadRouter.checkForcedPad` — plus `tryShoveViaPoints` in both angle regimes.
   */
  static void drillItemMover() {
    for (AngleRestriction ar : new AngleRestriction[] {AngleRestriction.NONE, AngleRestriction.NINETY_DEGREE}) {
      build(ar);
      System.out.println("mode=drill angle=" + ar);

      Via free = board.insertVia(thruPad, new IntPoint(2000, 2000), new int[] {3}, 1, FixedState.UNFIXED, false);
      Via fixedVia =
          board.insertVia(thruPad, new IntPoint(3000, 2000), new int[] {3}, 1, FixedState.SHOVE_FIXED, false);
      // A via sitting on the through-hole pin at (500, 0), so its normal contacts include a Pin.
      Via onPin = board.insertVia(thruPad, new IntPoint(500, 0), new int[] {1}, 1, FixedState.UNFIXED, false);

      System.out.println("  freeViaId=" + free.getId() + " contacts=" + free.getNormalContacts().size());
      System.out.println(
          "  fixedViaId=" + fixedVia.getId() + " isShoveFixed=" + fixedVia.isShoveFixed());
      System.out.println(
          "  onPinViaId="
              + onPin.getId()
              + " contacts="
              + onPin.getNormalContacts().size()
              + " kinds="
              + kinds(onPin.getNormalContacts()));

      Vector delta = new IntVector(300, 0);
      for (Via via : new Via[] {free, fixedVia, onPin}) {
        Collection<Item> ignore = new LinkedList<>();
        boolean ok = DrillItemMover.check(via, delta, 20, 5, ignore, board, null);
        System.out.println(
            "  check viaId=" + via.getId() + " delta=(300,0) result=" + ok + " ignoreSize=" + ignore.size());
      }

      TileShape obstacle = new IntBox(1900, 1900, 2400, 2400);
      for (boolean extended : new boolean[] {false, true}) {
        IntPoint[] centers =
            DrillItemMover.tryShoveViaPoints(obstacle, 0, free, 1, extended, board);
        StringBuilder sb = new StringBuilder();
        for (IntPoint c : centers) {
          sb.append("(").append(c.x).append(",").append(c.y).append(")");
        }
        System.out.println(
            "  tryShoveViaPoints box extended=" + extended + " n=" + centers.length + " " + sb);
      }
      TileShape octagon = new IntOctagon(1900, 1900, 2400, 2400, -600, 4600, -600, 4600);
      for (boolean extended : new boolean[] {false, true}) {
        IntPoint[] centers =
            DrillItemMover.tryShoveViaPoints(octagon, 0, free, 1, extended, board);
        StringBuilder sb = new StringBuilder();
        for (IntPoint c : centers) {
          sb.append("(").append(c.x).append(",").append(c.y).append(")");
        }
        System.out.println(
            "  tryShoveViaPoints octagon extended=" + extended + " n=" + centers.length + " " + sb);
      }
    }
  }

  static String kinds(Collection<Item> items) {
    StringBuilder sb = new StringBuilder();
    for (Item i : items) {
      sb.append(i.getClass().getSimpleName()).append("#").append(i.getId()).append(" ");
    }
    return sb.toString().trim();
  }

  // --- mode `tie` -----------------------------------------------------------------------------

  static void tiePins() {
    build(AngleRestriction.NONE);
    System.out.println("mode=tie");
    TraceShover shover = new TraceShover(board);
    TileShape[] shapes = {
      new IntBox(-600, -100, -400, 100), // over the SMD pin at (-500, 0), net 1
      new IntBox(400, -100, 600, 100), // over the through pin at (500, 0), net 1
      new IntBox(2000, 2000, 2200, 2200), // free space
    };
    for (int i = 0; i < shapes.length; i++) {
      for (int[] nets : new int[][] {new int[] {1}, new int[] {3}, new int[0]}) {
        Collection<Item> ignore = shover.getIgnoreItemsAtTiePins(shapes[i], 0, nets);
        System.out.println(
            "  shape="
                + i
                + " nets="
                + java.util.Arrays.toString(nets)
                + " n="
                + ignore.size()
                + " ids="
                + ids(ignore));
      }
    }
  }

  static String ids(Collection<Item> items) {
    StringBuilder sb = new StringBuilder();
    for (Item i : items) {
      sb.append(i.getId()).append(" ");
    }
    return sb.toString().trim();
  }

  /** `RoutingBoard.autorouteEngine` (RoutingBoard.java:70) is private and has no getter. */
  static AutorouteEngine installedEngine() throws Exception {
    Field f = RoutingBoard.class.getDeclaredField("autorouteEngine");
    f.setAccessible(true);
    return (AutorouteEngine) f.get(board);
  }

  static ShapeSearchTree treeOf(AutorouteEngine engine) {
    return engine.autorouteSearchTree;
  }
}
