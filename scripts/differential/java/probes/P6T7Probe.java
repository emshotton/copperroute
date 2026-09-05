// Plan 6 Task 7 ground-truth probe: the drill pages, the page array and the expansion drills, on
// hand-built boards. It is not a differential driver — there is no Rust twin and `run.sh` does not
// know it — but every literal in `crates/fr-router/tests/drill.rs` is read off its stdout, so it
// is committed here to keep those numbers reproducible.
//
// It declares `package app.freerouting.autoroute.drill` so it can reflect into `DrillPageArray`'s
// private `pages` / `columnCount` / `rowCount` / `pageWidth` / `pageHeight` and into `DrillPage`'s
// private `drills` / `netNumber`, which are the state the tests assert on and which no public
// method exposes. It compiles against the clone's HEAD jar exactly as `run.sh`'s jar mode does:
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential/java/probes
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p6t7 P6T7Probe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p6t7:$JAR" \
//       app.freerouting.autoroute.drill.P6T7Probe <mode>
//
// Modes: 0 the page grid for three bounding boxes, 1 `overlappingPages`' mixed-type loop bounds,
// 2 `getDrills(engine, false)`, 3 `getDrills(engine, true)` (the SMD pin), 4 the obstacle cut-out
// loop entry by entry (the `prevObstacleShape` carry), 5 `calculateExpansionRooms` on a blocked
// layer, 6 `getDrills` under a stop check, 7 `getId` across a net change, 8 `getDrills` on an
// engine whose `incompleteExpansionRooms` list has never been created, 9 a drill whose upper
// layer alone is blocked, 10 the engine's own array and its two drill hooks.
package app.freerouting.autoroute.drill;

import app.freerouting.autoroute.expansion.CompleteExpansionRoom;
import app.freerouting.autoroute.expansion.CompleteFreeSpaceExpansionRoom;
import app.freerouting.autoroute.expansion.IncompleteFreeSpaceExpansionRoom;
import app.freerouting.autoroute.maze.AutorouteEngine;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.model.structure.Layer;
import app.freerouting.board.model.structure.LayerStructure;
import app.freerouting.board.searchtree.ShapeSearchTree;
import app.freerouting.board.state.Communication;
import app.freerouting.core.library.Package;
import app.freerouting.core.library.Padstack;
import app.freerouting.datastructures.ShapeTree.TreeEntry;
import app.freerouting.datastructures.Stoppable;
import app.freerouting.geometry.planar.Area;
import app.freerouting.geometry.planar.ConvexShape;
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
import java.lang.reflect.Field;
import java.util.Collection;
import java.util.LinkedList;

/** Task 7 ground truth: DrillPageArray, DrillPage and ExpansionDrill on hand-built boards. */
public class P6T7Probe {

  static final IntBox BOUNDING_BOX = new IntBox(-10000, -10000, 10000, 10000);
  static RoutingBoard board;

  public static void main(String[] args) throws Exception {
    int mode = args.length > 0 ? Integer.parseInt(args[0]) : 0;
    switch (mode) {
      case 0 -> grid();
      case 1 -> overlaps();
      case 2 -> drills(false);
      case 3 -> drills(true);
      case 4 -> cutoutLoop();
      case 5 -> blockedLayer();
      case 6 -> stopCheck();
      case 7 -> idAcrossNetChange();
      case 8 -> virginEngine();
      case 9 -> blockedUpperLayer();
      case 10 -> engineArray();
      default -> throw new IllegalArgumentException("mode");
    }
  }

  // --- the board: `P6T3.build`'s any-angle board, verbatim ------------------------------------

  static void build(IntBox bounds) {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    cm.appendClass("wide");
    cm.setValue(2, 1, 600);
    cm.setValue(2, 2, 800);
    BoardRules rules = new BoardRules(ls, cm);
    rules.setTraceAngleRestriction(AngleRestriction.NONE);
    Communication comm = new Communication();
    board = new RoutingBoard(bounds, ls, new PolylineShape[0], 0, rules, comm);
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

  // --- dumps ----------------------------------------------------------------------------------

  static String shp(TileShape s) {
    if (s == null) {
      return "null";
    }
    if (s.isEmpty()) {
      return "empty";
    }
    IntBox b = s.boundingBox();
    StringBuilder sb = new StringBuilder();
    sb.append(s.getClass().getSimpleName())
        .append("[")
        .append(b.ll.x)
        .append(",")
        .append(b.ll.y)
        .append("..")
        .append(b.ur.x)
        .append(",")
        .append(b.ur.y)
        .append("]dim=")
        .append(s.dimension())
        .append(" corners=");
    for (int i = 0; i < s.borderLineCount(); i++) {
      sb.append("(")
          .append(Math.round(s.cornerApprox(i).x))
          .append(",")
          .append(Math.round(s.cornerApprox(i).y))
          .append(")");
    }
    return sb.toString();
  }

  static String box(IntBox b) {
    return "[" + b.ll.x + "," + b.ll.y + ".." + b.ur.x + "," + b.ur.y + "]";
  }

  static DrillPage[][] pagesOf(DrillPageArray array) throws Exception {
    Field f = DrillPageArray.class.getDeclaredField("pages");
    f.setAccessible(true);
    return (DrillPage[][]) f.get(array);
  }

  static int intField(DrillPageArray array, String name) throws Exception {
    Field f = DrillPageArray.class.getDeclaredField(name);
    f.setAccessible(true);
    return f.getInt(array);
  }

  @SuppressWarnings("unchecked")
  static Collection<ExpansionDrill> drillsOf(DrillPage page) throws Exception {
    Field f = DrillPage.class.getDeclaredField("drills");
    f.setAccessible(true);
    return (Collection<ExpansionDrill>) f.get(page);
  }

  static void dumpArray(DrillPageArray array, String tag) throws Exception {
    System.out.println(
        tag
            + " columnCount="
            + intField(array, "columnCount")
            + " rowCount="
            + intField(array, "rowCount")
            + " pageWidth="
            + intField(array, "pageWidth")
            + " pageHeight="
            + intField(array, "pageHeight"));
    DrillPage[][] pages = pagesOf(array);
    for (int j = 0; j < pages.length; j++) {
      for (int i = 0; i < pages[j].length; i++) {
        System.out.println("    page[" + j + "][" + i + "] " + box(pages[j][i].shape));
      }
    }
  }

  // --- mode 0: the page grid ------------------------------------------------------------------

  static void grid() throws Exception {
    // A square board that does not divide evenly, so `pageWidth` is recomputed upward.
    build(new IntBox(-10000, -10000, 10000, 10000));
    dumpArray(new DrillPageArray(board, 7000), "square20000 maxPageWidth=7000");
    // The engine's own number (AutorouteEngine.java:89-90) for this board's default via diameter.
    int engineWidth = Math.max((int) (5 * board.rules.getDefaultViaDiameter()), 10000);
    System.out.println("defaultViaDiameter=" + board.rules.getDefaultViaDiameter()
        + " maxDrillPageWidth=" + engineWidth);
    dumpArray(new DrillPageArray(board, engineWidth), "square20000 engineWidth");
    // A wide, short board: columnCount and rowCount differ.
    build(new IntBox(-15000, -1000, 15000, 3000));
    dumpArray(new DrillPageArray(board, 10000), "wide30000x4000 maxPageWidth=10000");
    // A board smaller than one page.
    build(new IntBox(0, 0, 3000, 5000));
    dumpArray(new DrillPageArray(board, 10000), "small3000x5000 maxPageWidth=10000");
  }

  // --- mode 1: `overlappingPages`' mixed-type loop bounds --------------------------------------

  static void overlaps() throws Exception {
    build(new IntBox(-10000, -10000, 10000, 10000));
    DrillPageArray array = new DrillPageArray(board, 7000);
    dumpArray(array, "square20000 maxPageWidth=7000");
    IntBox bounds = board.boundingBox;
    int pageWidth = intField(array, "pageWidth");
    int pageHeight = intField(array, "pageHeight");
    IntBox[] probes = {
      // maxJ/maxI land on a fraction: the page containing the fractional bound must be kept.
      new IntBox(-10000, -10000, 1000, 1000),
      // maxJ/maxI land exactly on an integer: the page above must NOT be kept.
      new IntBox(-10000, -10000, -3000, -3000),
      // the whole board.
      new IntBox(-10000, -10000, 10000, 10000),
      // a shape entirely inside one page.
      new IntBox(-1000, -1000, -900, -900),
      // a shape whose intersection with the bounds is 1-dimensional along a page border.
      new IntBox(-3000, -10000, -3000, 10000),
      // a shape sticking out of the board.
      new IntBox(5000, 5000, 30000, 30000),
    };
    for (IntBox probe : probes) {
      IntBox shapeBox = probe.boundingBox().intersection(bounds);
      int minJ = (int) Math.floor(((double) (shapeBox.ll.y - bounds.ll.y)) / (double) pageHeight);
      double maxJ = ((double) (shapeBox.ur.y - bounds.ll.y)) / (double) pageHeight;
      int minI = (int) Math.floor(((double) (shapeBox.ll.x - bounds.ll.x)) / (double) pageWidth);
      double maxI = ((double) (shapeBox.ur.x - bounds.ll.x)) / (double) pageWidth;
      System.out.println(
          "probe " + box(probe) + " minJ=" + minJ + " maxJ=" + maxJ + " minI=" + minI
              + " maxI=" + maxI);
      Collection<DrillPage> result = array.overlappingPages(probe);
      System.out.println("  n=" + result.size());
      for (DrillPage page : result) {
        System.out.println("    " + box(page.shape));
      }
    }
    // `invalidate` is `overlappingPages` plus a reset, so pin it through the memoised drills.
    AutorouteEngine engine = new AutorouteEngine(board, 1, true);
    engine.initConnection(1, null, null);
    seedList(engine);
    DrillPage page = pagesOf(array)[0][0];
    page.getDrills(engine, false);
    System.out.println("beforeInvalidate drills=" + (drillsOf(page) == null ? "null"
        : String.valueOf(drillsOf(page).size())));
    array.invalidate(new IntBox(-10000, -10000, -9000, -9000));
    System.out.println("afterInvalidate drills=" + (drillsOf(page) == null ? "null"
        : String.valueOf(drillsOf(page).size())));
  }

  // --- modes 2 and 3: `getDrills` --------------------------------------------------------------

  /** The page around the two-pin component, small enough that the drill list is readable. */
  static DrillPage componentPage() {
    return new DrillPage(new IntBox(-1000, -1000, 1000, 1000), board);
  }

  /**
   * Creates `incompleteExpansionRooms` and leaves it empty.
   *
   * Without this every drill dies: `ExpansionDrill.calculateExpansionRooms:79` reaches
   * `AutorouteEngine.removeIncompleteExpansionRoom:370`, whose `incompleteExpansionRooms.remove`
   * has no null guard, and the list is lazily created by `addIncompleteExpansionRoom:344` only.
   * Mode 8 pins that; every other drill mode seeds the list first, which is the state a real
   * routing run is in by the time the maze reaches a drill page.
   */
  static void seedList(AutorouteEngine engine) {
    IncompleteFreeSpaceExpansionRoom seed =
        engine.addIncompleteExpansionRoom(null, 0, new IntBox(9000, 9000, 9100, 9100));
    engine.removeIncompleteExpansionRoom(seed);
  }

  static void dumpDrills(DrillPage page, AutorouteEngine engine, boolean attachSmd)
      throws Exception {
    Collection<ExpansionDrill> drills = page.getDrills(engine, attachSmd);
    System.out.println("attachSmd=" + attachSmd + " drills n=" + drills.size());
    for (ExpansionDrill drill : drills) {
      StringBuilder rooms = new StringBuilder();
      for (CompleteExpansionRoom room : drill.roomArr) {
        rooms.append(" ").append(room == null ? "null" : room.getClass().getSimpleName()
            + "#" + room.getId() + "@" + room.getLayer());
      }
      System.out.println(
          "    drill loc=(" + drill.location.toFloat().x + "," + drill.location.toFloat().y
              + ") layers=" + drill.firstLayer + ".." + drill.lastLayer
              + " id=" + drill.getId()
              + " shape=" + shp(drill.getShape())
              + " rooms:" + rooms);
    }
    System.out.println("  pageId=" + page.getId() + " pageDim=" + page.getDimension()
        + " mazeElements=" + page.mazeSearchElementCount());
  }

  static void drills(boolean attachSmd) throws Exception {
    build(BOUNDING_BOX);
    AutorouteEngine engine = new AutorouteEngine(board, 1, true);
    engine.initConnection(1, null, null);
    seedList(engine);
    DrillPage page = componentPage();
    dumpDrills(page, engine, attachSmd);
  }

  // --- mode 4: the obstacle cut-out loop, entry by entry ---------------------------------------

  static void cutoutLoop() throws Exception {
    build(BOUNDING_BOX);
    AutorouteEngine engine = new AutorouteEngine(board, 1, true);
    engine.initConnection(1, null, null);
    DrillPage page = componentPage();
    ShapeSearchTree searchTree = engine.autorouteSearchTree;
    Collection<TreeEntry> overlaps = new LinkedList<>();
    searchTree.overlappingTreeEntries(page.shape, -1, overlaps);
    System.out.println("netNumber=" + engine.getNetNumber() + " overlaps n=" + overlaps.size());
    TileShape prevObstacleShape = IntBox.EMPTY;
    int cutouts = 0;
    for (TreeEntry currentEntry : overlaps) {
      if (!(currentEntry.object instanceof Item currentItem)) {
        System.out.println("    entry object=" + currentEntry.object.getClass().getSimpleName()
            + " -> not an Item, skipped");
        continue;
      }
      String kind = currentItem.getClass().getSimpleName() + "#" + currentItem.getId()
          + "/" + currentEntry.shapeIndexInObject;
      if (currentItem.isDrillable(engine.getNetNumber())) {
        System.out.println("    entry " + kind + " -> drillable, skipped");
        continue;
      }
      boolean smdSkip = false;
      if (currentItem instanceof Pin pin) {
        smdSkip = pin.drillAllowed();
        System.out.println("    entry " + kind + " pin drillAllowed=" + pin.drillAllowed());
      }
      TileShape currentObstacleShape =
          currentItem.getTreeShape(searchTree, currentEntry.shapeIndexInObject);
      boolean contained = prevObstacleShape.contains(currentObstacleShape);
      String note = "";
      if (!contained) {
        TileShape currentCutoutShape = currentObstacleShape.intersection(page.shape);
        if (currentCutoutShape.dimension() == 2) {
          cutouts++;
          note = " CUTOUT " + shp(currentCutoutShape);
        } else {
          note = " cutoutDim=" + currentCutoutShape.dimension();
        }
      } else {
        note = " prevContains -> no cutout";
      }
      System.out.println("    entry " + kind + " smdWouldSkip=" + smdSkip
          + " shape=" + shp(currentObstacleShape) + note);
      prevObstacleShape = currentObstacleShape;
    }
    System.out.println("cutouts=" + cutouts);
  }

  // --- mode 5: `calculateExpansionRooms` on a blocked layer -------------------------------------

  static void dumpRooms(ExpansionDrill drill, String tag) {
    for (int i = 0; i < drill.roomArr.length; i++) {
      CompleteExpansionRoom r = drill.roomArr[i];
      System.out.println("    " + tag + " roomArr[" + i + "]=" + (r == null ? "null"
          : r.getClass().getSimpleName() + "#" + r.getId() + "@" + r.getLayer()));
    }
  }

  static void blockedLayer() throws Exception {
    build(BOUNDING_BOX);
    // A keepout on layer 1 only, around the origin: the drill location below is free on layer 0
    // and blocked on layer 1 in the compensated search tree.
    board.insertObstacle(new IntBox(-200, -200, 200, 200), 1, 1, FixedState.UNFIXED);
    AutorouteEngine engine = new AutorouteEngine(board, 1, true);
    engine.initConnection(1, null, null);
    seedList(engine);
    ExpansionDrill blocked =
        new ExpansionDrill(new IntBox(-50, -50, 50, 50), new IntPoint(0, 0), 0, 1);
    System.out.println("blocked calculateExpansionRooms=" + blocked.calculateExpansionRooms(engine));
    dumpRooms(blocked, "blocked");
    System.out.println("blocked getId=" + blocked.getId() + " dim=" + blocked.getDimension()
        + " mazeElements=" + blocked.mazeSearchElementCount()
        + " otherRoom=" + blocked.otherRoom(null));

    // The same drill on layer 0 alone succeeds, because the obstacle is on layer 1.
    ExpansionDrill layer0 =
        new ExpansionDrill(new IntBox(-50, -50, 50, 50), new IntPoint(0, 0), 0, 0);
    System.out.println("layer0 calculateExpansionRooms=" + layer0.calculateExpansionRooms(engine));
    dumpRooms(layer0, "layer0");
    System.out.println("layer0 getId=" + layer0.getId()
        + " mazeElements=" + layer0.mazeSearchElementCount());

    // A location free on both layers: two rooms, one per layer.
    ExpansionDrill free =
        new ExpansionDrill(new IntBox(785, 75, 885, 175), new IntPoint(835, 125), 0, 1);
    System.out.println("free calculateExpansionRooms=" + free.calculateExpansionRooms(engine));
    dumpRooms(free, "free");
    System.out.println("free getId=" + free.getId());
    // A second drill at the same location re-uses the rooms that are now in the tree.
    ExpansionDrill again =
        new ExpansionDrill(new IntBox(785, 75, 885, 175), new IntPoint(835, 125), 0, 1);
    System.out.println("again calculateExpansionRooms=" + again.calculateExpansionRooms(engine));
    dumpRooms(again, "again");
  }

  // --- mode 6: `getDrills` under a stop check ---------------------------------------------------

  static void stopCheck() throws Exception {
    build(BOUNDING_BOX);
    AutorouteEngine engine = new AutorouteEngine(board, 1, true);
    engine.initConnection(1, null, null);
    seedList(engine);
    engine.stoppableThread =
        new Stoppable() {
          @Override
          public void requestStop() {}

          @Override
          public boolean isStopRequested() {
            return true;
          }
        };
    DrillPage page = componentPage();
    try {
      Collection<ExpansionDrill> drills = page.getDrills(engine, false);
      System.out.println("no throw, drills n=" + drills.size());
    } catch (Throwable t) {
      System.out.println("threw " + t.getClass().getName() + " at " + t.getStackTrace()[0]);
    }
    System.out.println("afterThrow netNumber(field)=" + netNumberOf(page)
        + " drills=" + (drillsOf(page) == null ? "null" : String.valueOf(drillsOf(page).size())));
    // The memoised empty list is returned on the next call, stop check or not.
    engine.stoppableThread = null;
    Collection<ExpansionDrill> again = page.getDrills(engine, false);
    System.out.println("secondCall drills n=" + again.size());
  }

  static int netNumberOf(DrillPage page) throws Exception {
    Field f = DrillPage.class.getDeclaredField("netNumber");
    f.setAccessible(true);
    return f.getInt(page);
  }

  // --- mode 7: `getId` across a net change ------------------------------------------------------

  static void idAcrossNetChange() throws Exception {
    build(BOUNDING_BOX);
    AutorouteEngine engine = new AutorouteEngine(board, 1, false);
    DrillPage page = componentPage();
    System.out.println("fresh netNumber=" + netNumberOf(page) + " id=" + page.getId()
        + " shapeId=" + page.shape.getId());
    engine.initConnection(1, null, null);
    seedList(engine);
    Collection<ExpansionDrill> first = page.getDrills(engine, false);
    System.out.println("afterNet1 netNumber=" + netNumberOf(page) + " id=" + page.getId()
        + " drills=" + first.size());
    engine.initConnection(2, null, null);
    Collection<ExpansionDrill> second = page.getDrills(engine, false);
    System.out.println("afterNet2 netNumber=" + netNumberOf(page) + " id=" + page.getId()
        + " drills=" + second.size());
    // `reset` does not clear the memoised list, only the maze scratch.
    page.reset();
    System.out.println("afterReset netNumber=" + netNumberOf(page)
        + " drills=" + (drillsOf(page) == null ? "null" : String.valueOf(drillsOf(page).size())));
    page.invalidate();
    System.out.println("afterInvalidate drills="
        + (drillsOf(page) == null ? "null" : String.valueOf(drillsOf(page).size()))
        + " netNumber=" + netNumberOf(page) + " id=" + page.getId());
  }

  // --- mode 8: `getDrills` on a virgin engine ---------------------------------------------------

  static void virginEngine() throws Exception {
    build(BOUNDING_BOX);
    AutorouteEngine engine = new AutorouteEngine(board, 1, true);
    engine.initConnection(1, null, null);
    DrillPage page = componentPage();
    Collection<ExpansionDrill> drills = page.getDrills(engine, false);
    System.out.println("virgin drills n=" + drills.size());
    ExpansionDrill drill =
        new ExpansionDrill(new IntBox(785, 75, 885, 175), new IntPoint(835, 125), 0, 1);
    System.out.println("virgin calculateExpansionRooms=" + drill.calculateExpansionRooms(engine));
    seedList(engine);
    ExpansionDrill after =
        new ExpansionDrill(new IntBox(785, 75, 885, 175), new IntPoint(835, 125), 0, 1);
    System.out.println("seeded calculateExpansionRooms=" + after.calculateExpansionRooms(engine));
  }

  // --- mode 9: only the upper layer is blocked --------------------------------------------------

  static void blockedUpperLayer() throws Exception {
    build(BOUNDING_BOX);
    // A keepout on layer 1 covering the drill location, but not on layer 0.
    board.insertObstacle(new IntBox(700, -300, 1000, 300), 1, 1, FixedState.UNFIXED);
    AutorouteEngine engine = new AutorouteEngine(board, 1, true);
    engine.initConnection(1, null, null);
    seedList(engine);
    // Warm the room database exactly as mode 2 does, so layer 0 resolves to a single room.
    DrillPage page = componentPage();
    System.out.println("warm drills n=" + page.getDrills(engine, false).size());
    ExpansionDrill drill =
        new ExpansionDrill(new IntBox(785, 75, 885, 175), new IntPoint(835, 125), 0, 1);
    System.out.println("upperBlocked calculateExpansionRooms="
        + drill.calculateExpansionRooms(engine));
    dumpRooms(drill, "upperBlocked");
  }

  // --- mode 10: the engine's own array, `invalidateDrillPages` and `resetAllDoors` -------------

  static void engineArray() throws Exception {
    // A board whose bounding box is one 10 000-wide page, so the engine's own array is 1x1 and
    // its single page is exactly `componentPage()`'s box. On the full -10000..10000 board the
    // engine's pages are 10 000 units wide and completing a room in one of them trips quirk
    // #162's non-terminating `calculateNewIncompleteRooms`.
    build(new IntBox(-1000, -1000, 1000, 1000));
    AutorouteEngine engine = new AutorouteEngine(board, 1, true);
    engine.initConnection(1, null, null);
    seedList(engine);
    Field fa = AutorouteEngine.class.getDeclaredField("drillPageArray");
    fa.setAccessible(true);
    DrillPageArray array = (DrillPageArray) fa.get(engine);
    dumpArray(array, "engineArray");
    DrillPage page = pagesOf(array)[0][0];
    System.out.println("page00 " + box(page.shape)
        + " drills=" + page.getDrills(engine, false).size());
    System.out.println("page00 memo="
        + (drillsOf(page) == null ? "null" : String.valueOf(drillsOf(page).size())));
    engine.invalidateDrillPages(new IntBox(-900, -900, -800, -800));
    System.out.println("afterInvalidateDrillPages memo="
        + (drillsOf(page) == null ? "null" : String.valueOf(drillsOf(page).size())));
    int again = page.getDrills(engine, false).size();
    System.out.println("recomputed drills=" + again);
    array.reset();
    System.out.println("afterReset memo="
        + (drillsOf(page) == null ? "null" : String.valueOf(drillsOf(page).size())));
    // A shape that misses the board's bounding box invalidates nothing: `:79` intersects first.
    engine.invalidateDrillPages(new IntBox(-9000, -9000, -8000, -8000));
    System.out.println("afterMissingInvalidate memo="
        + (drillsOf(page) == null ? "null" : String.valueOf(drillsOf(page).size())));
    for (ExpansionDrill d : drillsOf(page)) {
      System.out.println("    drill loc=(" + d.location.toFloat().x + "," + d.location.toFloat().y
          + ") id=" + d.getId() + " shape=" + shp(d.getShape()));
    }
  }

  static CompleteFreeSpaceExpansionRoom unused; // keeps the import honest for the room dumps
}
