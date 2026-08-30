// Plan 6 Task 11 ground-truth probe: `MazeSearchEngine`'s construction, `init` and pop loop
// (`autoroute/maze/MazeSearchEngine.java:75-152`, `:300-384`, `:969-1103`). It is not a
// differential driver — there is no Rust twin and `run.sh` does not know it — but every literal in
// `crates/fr-router/tests/maze_search.rs` is read off its stdout, so it is committed here to keep
// those numbers reproducible.
//
// It declares `package app.freerouting.autoroute.maze` so it can read `MazeSearchEngine`'s
// package-private `mazeExpansionList` / `ctrl` / `destinationDistance` and `MazeListElement`'s
// package-private fields directly, and reflect into the two private `destinationDoor` /
// `sectionNoOfDestinationDoor` fields. It compiles against the clone's HEAD jar exactly as
// `run.sh`'s jar mode does:
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential/java/probes
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p6t11 P6T11Probe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p6t11:$JAR" \
//       app.freerouting.autoroute.maze.P6T11Probe <mode>
//
// The board is `P6T7Probe.build`'s two-pin board (itself `P6T3.build`'s any-angle board) with the
// two traces replaced by one obstacle box, two nets declared and an empty `ViaRule` on the default
// net class -- `AutorouteControl.initNet:230` -> `rebuildViaInfo:235` dereferences the via rule and
// `initNet:206` the net, and a positive net the board's `Nets` does not have throws (quirk #173).
// The traces had to go: under the "wide" clearance class the net-2 trace's *compensated* tree shape
// covers x in [-1220, -380], which swallows the start pin's centre at (-500, 0) -- and a pin's
// `getTraceConnectionShape` is a bare point (`DrillItem.java:359-361`), so `completeShape` then
// answers no room at all and `init` reports false for a reason unrelated to the method under test.
// The bounding box is deliberately small: quirk #162 can hang `completeShape` on a wide board, and
// `init` completes rooms.
//
// Modes:
//   items      the board's item list, so the Rust twin can name the same ids
//   init       `init` on start={pin id 2} dest={pin id 3}: the completed rooms, their target
//              doors and the seeded queue, in order
//   startorder a two-item start set: the completed rooms and the five-element queue
//   startrooms the same, aborted on the fourth stop call, dumping `incompleteExpansionRooms` in
//              list order — where `TreeSet<Item>`'s descending-id walk is observable
//   nostart    `getInstance` with an empty start set, and with an empty destination set
//   stop<n>    a `Stoppable` that trips on its n-th call, n = 1..8: which of `init`'s four sites
//              fired, and the partial state it left
//   pops       `occupyNextElement` reaching a destination target door, then `findConnection`
//   occupied   `occupyNextElement` skipping already-occupied sections without expanding
//   fanout     `init` under a fanout control whose escape window refuses every seeded element
//   small      `doorIsSmall` over three door boxes x seven widths x the three angle restrictions
//   project    `segmentProjection` on six hand-built pairs
//   tiepin     `reduceTraceShapesAtTiePins` on a pin carrying two nets
package app.freerouting.autoroute.maze;

import app.freerouting.autoroute.expansion.CompleteFreeSpaceExpansionRoom;
import app.freerouting.autoroute.expansion.ExpandableObject;
import app.freerouting.autoroute.expansion.TargetItemExpansionDoor;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.model.structure.Layer;
import app.freerouting.board.model.structure.LayerStructure;
import app.freerouting.board.state.Communication;
import app.freerouting.core.library.Package;
import app.freerouting.core.library.Padstack;
import app.freerouting.datastructures.Stoppable;
import app.freerouting.geometry.planar.ConvexShape;
import app.freerouting.geometry.planar.FloatLine;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.IntOctagon;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import app.freerouting.geometry.planar.PolylineShape;
import app.freerouting.geometry.planar.Shape;
import app.freerouting.geometry.planar.TileShape;
import app.freerouting.rules.BoardRules;
import app.freerouting.rules.ClearanceMatrix;
import app.freerouting.settings.RouterSettings;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.Set;
import java.util.TreeSet;

/** Task 11 ground truth: MazeSearchEngine's init and pop loop on a hand-built board. */
public class P6T11Probe {

  static final IntBox BOUNDING_BOX = new IntBox(-10000, -10000, 10000, 10000);
  static RoutingBoard board;

  public static void main(String[] args) throws Exception {
    String mode = args.length > 0 ? args[0] : "init";
    if (mode.startsWith("stop")) {
      stopSite(Integer.parseInt(mode.substring(4)));
      return;
    }
    switch (mode) {
      case "items" -> items();
      case "startorder" -> startOrder();
      case "startrooms" -> startRooms();
      case "tiepin" -> tiePin();
      case "init" -> init();
      case "nostart" -> noStart();
      case "pops" -> pops();
      case "occupied" -> occupied();
      case "fanout" -> fanout();
      case "small" -> small();
      case "project" -> project();
      default -> throw new IllegalArgumentException("mode " + mode);
    }
  }

  // --- the board: `P6T7Probe.build`'s two-pin board plus two declared nets --------------------

  static void build() {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    cm.appendClass("wide");
    cm.setValue(2, 1, 600);
    cm.setValue(2, 2, 800);
    BoardRules rules = new BoardRules(ls, cm);
    rules.setTraceAngleRestriction(AngleRestriction.NONE);
    Communication comm = new Communication();
    board = new RoutingBoard(BOUNDING_BOX, ls, new PolylineShape[0], 0, rules, comm);
    // `AutorouteControl.initNet:230` -> `rebuildViaInfo:235` dereferences the net class's via
    // rule with no null check, so the default class needs one. An empty rule keeps `viaCount()`
    // at 0, which is `viaClearanceClass = 1` and a zero-length `viaInfos` (:236-240).
    app.freerouting.rules.ViaRule emptyRule = new app.freerouting.rules.ViaRule("empty");
    board.rules.viaRules.add(emptyRule);
    board.rules.getDefaultNetClass().setViaRule(emptyRule);
    board.rules.nets.add("N1", 1, false);
    board.rules.nets.add("N2", 1, false);
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
      new Package.Pin("P1", smdPad.id, new app.freerouting.geometry.planar.IntVector(-500, 0), 0),
      new Package.Pin("P2", thruPad.id, new app.freerouting.geometry.planar.IntVector(500, 0), 0)
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
    // One obstacle between the two pins, so the start room has a shape worth asserting on. A
    // foreign-net *trace* would not do: with the "wide" clearance class its compensated shape
    // swallows the pin centre, and `ShapeSearchTree.completeShape` then answers no room at all.
    board.insertObstacle(new IntBox(700, -1000, 900, 1000), 0, 1, FixedState.UNFIXED);
  }

  // --- helpers --------------------------------------------------------------------------------

  static Item itemById(int id) {
    for (Item current : board.getItems()) {
      if (current.getId() == id) {
        return current;
      }
    }
    throw new IllegalStateException("no item " + id);
  }

  static Set<Item> setOf(int... ids) {
    Set<Item> result = new TreeSet<>();
    for (int id : ids) {
      result.add(itemById(id));
    }
    return result;
  }

  static AutorouteControl control(int netNo) {
    RouterSettings settings = new RouterSettings(board);
    return new AutorouteControl(
        board, netNo, settings, settings.getViaCosts(), settings.getTraceCosts());
  }

  static AutorouteEngine engine(Stoppable stoppable, int netNo) {
    AutorouteEngine result = new AutorouteEngine(board, 1, false);
    result.initConnection(netNo, stoppable, null);
    return result;
  }

  static String fp(FloatPoint p) {
    return p == null ? "null" : String.format("(%.6f,%.6f)", p.x, p.y);
  }

  static String fl(FloatLine l) {
    return l == null ? "null" : fp(l.a) + "-" + fp(l.b);
  }

  static String describe(ExpandableObject door) {
    if (door == null) {
      return "null";
    }
    String kind = door.getClass().getSimpleName();
    String extra = "";
    if (door instanceof TargetItemExpansionDoor target) {
      extra =
          " item="
              + target.item.getId()
              + " treeEntryNo="
              + target.treeEntryNo
              + " room="
              + (target.room == null ? "null" : String.valueOf(target.room.getId()))
              + " dest="
              + target.isDestinationDoor();
    }
    return kind + " id=" + door.getId() + extra;
  }

  static void dumpQueue(MazeSearchEngine maze) {
    System.out.println("queue n=" + maze.mazeExpansionList.size());
    int i = 0;
    for (MazeListElement e : maze.mazeExpansionList) {
      System.out.println(
          "  ["
              + i
              + "] door="
              + describe(e.door)
              + " section="
              + e.sectionNoOfDoor
              + " backtrack="
              + describe(e.backtrackDoor)
              + " sectionOfBacktrack="
              + e.sectionNoOfBacktrackDoor
              + String.format(" expansion=%.9f sorting=%.9f", e.expansionValue, e.sortingValue)
              + " nextRoom="
              + (e.nextRoom == null ? "null" : e.nextRoom.getId())
              + " nextRoomLayer="
              + (e.nextRoom == null ? "-" : e.nextRoom.getLayer())
              + " shapeEntry="
              + fl(e.shapeEntry)
              + " roomRipped="
              + e.roomRipped
              + " adjustment="
              + e.adjustment
              + " alreadyChecked="
              + e.alreadyChecked
              + " ripupCost="
              + e.ripupCost);
      i++;
    }
  }

  static ExpandableObject destinationDoor(MazeSearchEngine maze) throws Exception {
    Field f = MazeSearchEngine.class.getDeclaredField("destinationDoor");
    f.setAccessible(true);
    return (ExpandableObject) f.get(maze);
  }

  static int sectionNoOfDestinationDoor(MazeSearchEngine maze) throws Exception {
    Field f = MazeSearchEngine.class.getDeclaredField("sectionNoOfDestinationDoor");
    f.setAccessible(true);
    return f.getInt(maze);
  }

  /** A `Stoppable` that answers true from its `trip`-th call onwards; 0 means never. */
  static final class Counter implements Stoppable {
    final int trip;
    int calls;

    Counter(int trip) {
      this.trip = trip;
    }

    @Override
    public void requestStop() {}

    @Override
    public boolean isStopRequested() {
      calls++;
      return trip > 0 && calls >= trip;
    }
  }

  // --- modes ----------------------------------------------------------------------------------

  static void items() {
    build();
    for (Item current : board.getItems()) {
      System.out.println(
          "item id="
              + current.getId()
              + " "
              + current.getClass().getSimpleName()
              + " nets="
              + java.util.Arrays.toString(current.netNumbers)
              + " treeShapeCount="
              + current.treeShapeCount(board.searchTreeManager.getDefaultTree()));
    }
  }

  static void init() throws Exception {
    build();
    Counter counter = new Counter(0);
    AutorouteEngine autorouteEngine = engine(counter, 1);
    AutorouteControl ctrl = control(1);
    Set<Item> start = setOf(2);
    Set<Item> dest = setOf(3);
    MazeSearchEngine maze = new MazeSearchEngine(autorouteEngine, ctrl);
    Method initMethod =
        MazeSearchEngine.class.getDeclaredMethod("init", Set.class, Set.class);
    initMethod.setAccessible(true);
    boolean ok = (Boolean) initMethod.invoke(maze, start, dest);
    System.out.println("init=" + ok);
    System.out.println("stopCalls=" + counter.calls);
    System.out.println("startInfo item2=" + itemById(2).getAutorouteInfo().isStartInfo());
    System.out.println("startInfo item3=" + itemById(3).getAutorouteInfo().isStartInfo());
    System.out.println("startInfo item4=" + itemById(4).getAutorouteInfo().isStartInfo());
    System.out.println("completeRooms n=" + completeRoomCount(autorouteEngine));
    System.out.println("incompleteRooms n=" + incompleteRoomCount(autorouteEngine));
    dumpRooms(autorouteEngine);
    dumpQueue(maze);
    System.out.println("destinationDoor=" + describe(destinationDoor(maze)));
  }

  @SuppressWarnings("unchecked")
  static void dumpRooms(AutorouteEngine autorouteEngine) throws Exception {
    Field f = AutorouteEngine.class.getDeclaredField("completeExpansionRooms");
    f.setAccessible(true);
    java.util.List<CompleteFreeSpaceExpansionRoom> list =
        (java.util.List<CompleteFreeSpaceExpansionRoom>) f.get(autorouteEngine);
    if (list == null) {
      return;
    }
    for (CompleteFreeSpaceExpansionRoom room : list) {
      IntBox b = room.getShape().boundingBox();
      System.out.println(
          "room id="
              + room.getId()
              + " layer="
              + room.getLayer()
              + " box=["
              + b.ll.x
              + ","
              + b.ll.y
              + ".."
              + b.ur.x
              + ","
              + b.ur.y
              + "] doors="
              + room.getDoors().size()
              + " targetDoors="
              + room.getTargetDoors().size());
      for (TargetItemExpansionDoor d : room.getTargetDoors()) {
        System.out.println("    target " + describe(d));
      }
    }
  }

  /**
   * `init` with a **two-item** start set: the order the start rooms are created in is
   * `TreeSet<Item>`'s, i.e. descending id, and it reaches the output as the room ids.
   */
  static void startOrder() throws Exception {
    build();
    Counter counter = new Counter(0);
    AutorouteEngine autorouteEngine = engine(counter, 1);
    AutorouteControl ctrl = control(1);
    MazeSearchEngine maze = new MazeSearchEngine(autorouteEngine, ctrl);
    Method initMethod = MazeSearchEngine.class.getDeclaredMethod("init", Set.class, Set.class);
    initMethod.setAccessible(true);
    System.out.println("init=" + initMethod.invoke(maze, setOf(2, 3), setOf(3)));
    System.out.println("stopCalls=" + counter.calls);
    dumpRooms(autorouteEngine);
    dumpQueue(maze);
  }

  /**
   * The order `init` creates the start rooms in, made visible by aborting on the fourth stop call
   * — the first of the room-completion loop — and dumping `incompleteExpansionRooms` in list
   * order.
   */
  @SuppressWarnings("unchecked")
  static void startRooms() throws Exception {
    build();
    Counter counter = new Counter(4);
    AutorouteEngine autorouteEngine = engine(counter, 1);
    AutorouteControl ctrl = control(1);
    MazeSearchEngine maze = new MazeSearchEngine(autorouteEngine, ctrl);
    Method initMethod = MazeSearchEngine.class.getDeclaredMethod("init", Set.class, Set.class);
    initMethod.setAccessible(true);
    System.out.println("init=" + initMethod.invoke(maze, setOf(2, 3), setOf(3)));
    System.out.println("stopCalls=" + counter.calls);
    Field fi = AutorouteEngine.class.getDeclaredField("incompleteExpansionRooms");
    fi.setAccessible(true);
    java.util.List<app.freerouting.autoroute.expansion.IncompleteFreeSpaceExpansionRoom> list =
        (java.util.List<app.freerouting.autoroute.expansion.IncompleteFreeSpaceExpansionRoom>)
            fi.get(autorouteEngine);
    System.out.println("incompleteRooms n=" + (list == null ? -1 : list.size()));
    if (list != null) {
      int i = 0;
      for (app.freerouting.autoroute.expansion.IncompleteFreeSpaceExpansionRoom r : list) {
        System.out.println(
            "  ["
                + (i++)
                + "] layer="
                + r.getLayer()
                + " contained="
                + box(r.getContainedShape().boundingBox()));
      }
    }
  }

  static int completeRoomCount(AutorouteEngine autorouteEngine) throws Exception {
    Field f = AutorouteEngine.class.getDeclaredField("completeExpansionRooms");
    f.setAccessible(true);
    java.util.List<?> list = (java.util.List<?>) f.get(autorouteEngine);
    return list == null ? -1 : list.size();
  }

  static void noStart() throws Exception {
    build();
    Counter counter = new Counter(0);
    AutorouteEngine autorouteEngine = engine(counter, 1);
    AutorouteControl ctrl = control(1);
    MazeSearchEngine maze =
        MazeSearchEngine.getInstance(new TreeSet<>(), setOf(3), autorouteEngine, ctrl);
    System.out.println("emptyStart instance=" + (maze == null ? "null" : "ok"));
    System.out.println("emptyStart stopCalls=" + counter.calls);

    build();
    counter = new Counter(0);
    autorouteEngine = engine(counter, 1);
    ctrl = control(1);
    maze = MazeSearchEngine.getInstance(setOf(2), new TreeSet<>(), autorouteEngine, ctrl);
    System.out.println("emptyDest instance=" + (maze == null ? "null" : "ok"));
    System.out.println("emptyDest stopCalls=" + counter.calls);
  }

  static void stopSite(int trip) throws Exception {
    build();
    Counter counter = new Counter(trip);
    AutorouteEngine autorouteEngine = engine(counter, 1);
    AutorouteControl ctrl = control(1);
    MazeSearchEngine maze = new MazeSearchEngine(autorouteEngine, ctrl);
    Method initMethod = MazeSearchEngine.class.getDeclaredMethod("init", Set.class, Set.class);
    initMethod.setAccessible(true);
    boolean ok = (Boolean) initMethod.invoke(maze, setOf(2), setOf(3));
    System.out.println("trip=" + trip + " init=" + ok);
    System.out.println("stopCalls=" + counter.calls);
    System.out.println("startInfo item2=" + itemById(2).getAutorouteInfo().isStartInfo());
    System.out.println("startInfo item3=" + itemById(3).getAutorouteInfo().isStartInfo());
    System.out.println("completeRooms n=" + completeRoomCount(autorouteEngine));
    System.out.println("incompleteRooms n=" + incompleteRoomCount(autorouteEngine));
    dumpQueue(maze);
  }

  static int incompleteRoomCount(AutorouteEngine autorouteEngine) throws Exception {
    Field f = AutorouteEngine.class.getDeclaredField("incompleteExpansionRooms");
    f.setAccessible(true);
    java.util.List<?> list = (java.util.List<?>) f.get(autorouteEngine);
    return list == null ? -1 : list.size();
  }

  /**
   * The destination-door terminator: seed `init` normally, then push a *destination* target door
   * with a lower sorting value and pop once.
   */
  static void pops() throws Exception {
    build();
    Counter counter = new Counter(0);
    AutorouteEngine autorouteEngine = engine(counter, 1);
    AutorouteControl ctrl = control(1);
    MazeSearchEngine maze =
        MazeSearchEngine.getInstance(setOf(2), setOf(3), autorouteEngine, ctrl);
    System.out.println("instance=" + (maze == null ? "null" : "ok"));
    dumpQueue(maze);

    // The room that `init` seeded from, and a destination door on it.
    TargetItemExpansionDoor destDoor = null;
    CompleteFreeSpaceExpansionRoom room = null;
    for (MazeListElement e : maze.mazeExpansionList) {
      room = (CompleteFreeSpaceExpansionRoom) e.nextRoom;
      break;
    }
    for (TargetItemExpansionDoor current : room.getTargetDoors()) {
      if (current.isDestinationDoor()) {
        destDoor = current;
        break;
      }
    }
    System.out.println("destDoor=" + describe(destDoor));
    FloatPoint centre = destDoor.getShape().centreOfGravity();
    MazeListElement seeded =
        new MazeListElement(
            destDoor,
            0,
            null,
            0,
            0,
            -1.0,
            room,
            new FloatLine(centre, centre),
            false,
            MazeSearchElement.Adjustment.NONE,
            false);
    maze.mazeExpansionList.add(seeded);
    System.out.println("afterPush n=" + maze.mazeExpansionList.size());
    boolean more = maze.occupyNextElement();
    System.out.println("occupyNextElement=" + more);
    System.out.println("afterPop n=" + maze.mazeExpansionList.size());
    System.out.println("destinationDoor=" + describe(destinationDoor(maze)));
    System.out.println("sectionNoOfDestinationDoor=" + sectionNoOfDestinationDoor(maze));
    System.out.println(
        "destDoorSectionOccupied=" + destDoor.getMazeSearchElement(0).isOccupied);
    MazeSearchEngine.Result result = maze.findConnection();
    System.out.println(
        "findConnection="
            + (result == null
                ? "null"
                : describe(result.destinationDoor) + " section=" + result.sectionNoOfDoor));
  }

  /** An already-occupied section is popped, dropped and not expanded; the loop goes on. */
  static void occupied() throws Exception {
    build();
    Counter counter = new Counter(0);
    AutorouteEngine autorouteEngine = engine(counter, 1);
    AutorouteControl ctrl = control(1);
    MazeSearchEngine maze =
        MazeSearchEngine.getInstance(setOf(2), setOf(3), autorouteEngine, ctrl);
    dumpQueue(maze);

    MazeListElement first = maze.mazeExpansionList.iterator().next();
    CompleteFreeSpaceExpansionRoom room = (CompleteFreeSpaceExpansionRoom) first.nextRoom;
    // Occupy every seeded element's own section, so the pop loop must skip all of them and reach
    // the destination door pushed below. Nothing here expands.
    for (MazeListElement e : maze.mazeExpansionList) {
      e.door.getMazeSearchElement(e.sectionNoOfDoor).isOccupied = true;
    }

    TargetItemExpansionDoor destDoor = null;
    for (TargetItemExpansionDoor current : room.getTargetDoors()) {
      if (current.isDestinationDoor()) {
        destDoor = current;
        break;
      }
    }
    FloatPoint centre = destDoor.getShape().centreOfGravity();
    maze.mazeExpansionList.add(
        new MazeListElement(
            destDoor,
            0,
            null,
            0,
            0,
            1.0e9,
            room,
            new FloatLine(centre, centre),
            false,
            MazeSearchElement.Adjustment.NONE,
            false));
    System.out.println("beforePop n=" + maze.mazeExpansionList.size());
    boolean more = maze.occupyNextElement();
    System.out.println("occupyNextElement=" + more);
    System.out.println("afterPop n=" + maze.mazeExpansionList.size());
    System.out.println("destinationDoor=" + describe(destinationDoor(maze)));
    System.out.println(
        "skippedSectionBacktrack="
            + describe(first.door.getMazeSearchElement(first.sectionNoOfDoor).backtrackDoor));
  }

  /** `init` under a fanout control whose escape window refuses every seeded element. */
  static void fanout() throws Exception {
    build();
    Counter counter = new Counter(0);
    AutorouteEngine autorouteEngine = engine(counter, 1);
    AutorouteControl ctrl = control(1);
    ctrl.isFanout = true;
    ctrl.fanoutStartPinCenter = new IntPoint(9000, 9000);
    ctrl.fanoutStartPinLayer = 0;
    System.out.println(
        "resolution=" + board.communication.getResolution(app.freerouting.board.model.structure.Unit.UM));
    System.out.println(
        "maxEscapeLengthMm="
            + (ctrl.settings.fanout == null ? "null" : ctrl.settings.fanout.maxEscapeLengthMm));
    MazeSearchEngine maze =
        MazeSearchEngine.getInstance(setOf(2), setOf(3), autorouteEngine, ctrl);
    System.out.println("instance=" + (maze == null ? "null" : "ok"));
    if (maze != null) {
      dumpQueue(maze);
    }
  }

  /** `doorIsSmall` over the three angle restrictions, by reflection. */
  static void small() throws Exception {
    build();
    AutorouteEngine autorouteEngine = engine(new Counter(0), 1);
    AutorouteControl ctrl = control(1);
    MazeSearchEngine maze = new MazeSearchEngine(autorouteEngine, ctrl);
    Method m =
        MazeSearchEngine.class.getDeclaredMethod(
            "doorIsSmall",
            app.freerouting.autoroute.expansion.ExpansionDoor.class,
            double.class);
    m.setAccessible(true);
    // Two complete free space rooms whose shapes overlap in a 100x40 box.
    for (AngleRestriction restriction :
        new AngleRestriction[] {
          AngleRestriction.NINETY_DEGREE, AngleRestriction.FORTYFIVE_DEGREE, AngleRestriction.NONE
        }) {
      board.rules.setTraceAngleRestriction(restriction);
      for (int[] geom : new int[][] {{0, 0, 100, 40}, {0, 0, 40, 100}, {0, 0, 30, 30}}) {
        CompleteFreeSpaceExpansionRoom a =
            new CompleteFreeSpaceExpansionRoom(
                new IntBox(geom[0] - 500, geom[1] - 500, geom[2], geom[3]), 0, 1);
        CompleteFreeSpaceExpansionRoom b =
            new CompleteFreeSpaceExpansionRoom(
                new IntBox(geom[0], geom[1], geom[2] + 500, geom[3] + 500), 0, 2);
        app.freerouting.autoroute.expansion.ExpansionDoor door =
            new app.freerouting.autoroute.expansion.ExpansionDoor(a, b, 2);
        TileShape shape = door.getShape();
        for (double width : new double[] {10.0, 42.0, 45.0, 101.0, 105.0, 108.0, 200.0}) {
          System.out.println(
              "doorIsSmall restriction="
                  + restriction
                  + " door=["
                  + shape.boundingBox().ll.x
                  + ","
                  + shape.boundingBox().ll.y
                  + ".."
                  + shape.boundingBox().ur.x
                  + ","
                  + shape.boundingBox().ur.y
                  + "] width="
                  + width
                  + " => "
                  + m.invoke(maze, door, width));
        }
      }
    }
    board.rules.setTraceAngleRestriction(AngleRestriction.NONE);

    // The two clauses of `:775-777` that answer `false` without looking at any shape: a door of
    // dimension 2 whose rooms are not both `CompleteFreeSpaceExpansionRoom`s.
    app.freerouting.autoroute.expansion.ObstacleExpansionRoom obstacleRoom =
        new app.freerouting.autoroute.expansion.ObstacleExpansionRoom(
            itemById(4), 0, autorouteEngine.autorouteSearchTree);
    CompleteFreeSpaceExpansionRoom free =
        new CompleteFreeSpaceExpansionRoom(new IntBox(0, 0, 1000, 1000), 0, 7);
    for (int dim : new int[] {1, 2}) {
      app.freerouting.autoroute.expansion.ExpansionDoor mixed =
          new app.freerouting.autoroute.expansion.ExpansionDoor(free, obstacleRoom, dim);
      System.out.println(
          "doorIsSmall mixedRooms dimension=" + dim + " width=1.0e9 => " + m.invoke(maze, mixed, 1.0e9));
    }
  }

  /**
   * `reduceTraceShapesAtTiePins` (`:154-172`) on a board whose pin carries two nets: the tree
   * shapes of the foreign-net trace before and after.
   */
  static void tiePin() throws Exception {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    BoardRules rules = new BoardRules(ls, cm);
    rules.setTraceAngleRestriction(AngleRestriction.NONE);
    board = new RoutingBoard(BOUNDING_BOX, ls, new PolylineShape[0], 0, rules, new Communication());
    app.freerouting.rules.ViaRule emptyRule = new app.freerouting.rules.ViaRule("empty");
    board.rules.viaRules.add(emptyRule);
    board.rules.getDefaultNetClass().setViaRule(emptyRule);
    board.rules.nets.add("N1", 1, false);
    board.rules.nets.add("N2", 1, false);
    board.library.padstacks = new app.freerouting.core.library.Padstacks(ls);
    board.library.packages = new app.freerouting.core.library.Packages(board.library.padstacks);
    ConvexShape[] thru = {
      new IntBox(-200, -200, 200, 200), new IntBox(-200, -200, 200, 200)
    };
    Padstack thruPad = board.library.padstacks.add("thru", thru, true, false);
    Package.Pin[] pins = {
      new Package.Pin("P1", thruPad.id, new app.freerouting.geometry.planar.IntVector(0, 0), 0)
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
    // A tie pin: two nets on one pin.
    board.insertPin(1, 0, new int[] {1, 2}, 1, FixedState.UNFIXED);
    // A foreign-net trace ending at the pin centre, and an own-net one.
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(0, 0), new IntPoint(0, 2000)}),
        0,
        60,
        new int[] {2},
        1,
        FixedState.UNFIXED);
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(0, 0), new IntPoint(2000, 0)}),
        0,
        60,
        new int[] {1},
        1,
        FixedState.UNFIXED);
    for (Item current : board.getItems()) {
      System.out.println(
          "item id=" + current.getId() + " " + current.getClass().getSimpleName()
              + " nets=" + java.util.Arrays.toString(current.netNumbers)
              + " netCount=" + current.netCount());
    }
    AutorouteEngine autorouteEngine = engine(new Counter(0), 1);
    for (String phase : new String[] {"before", "after"}) {
      if (phase.equals("after")) {
        Method m =
            MazeSearchEngine.class.getDeclaredMethod(
                "reduceTraceShapesAtTiePins",
                java.util.Collection.class,
                int.class,
                app.freerouting.board.searchtree.ShapeSearchTree.class);
        m.setAccessible(true);
        m.invoke(null, setOf(2), 1, autorouteEngine.autorouteSearchTree);
      }
      for (Item current : board.getItems()) {
        if (!(current instanceof app.freerouting.board.trace.PolylineTrace)) {
          continue;
        }
        int n = current.treeShapeCount(autorouteEngine.autorouteSearchTree);
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < n; i++) {
          TileShape ts = current.getTreeShape(autorouteEngine.autorouteSearchTree, i);
          sb.append(" ").append(ts == null ? "null" : box(ts.boundingBox()));
        }
        System.out.println(phase + " trace id=" + current.getId() + " n=" + n + sb);
      }
    }
  }

  /** `segmentProjection` on hand-built pairs. */
  static void project() throws Exception {
    Method m =
        MazeSearchEngine.class.getDeclaredMethod(
            "segmentProjection", FloatLine.class, FloatLine.class);
    m.setAccessible(true);
    FloatLine[][] cases = {
      {line(0, 0, 100, 0), line(0, 10, 100, 10)},
      {line(0, 0, 100, 0), line(50, 10, 200, 10)},
      {line(0, 0, 100, 0), line(200, 10, 300, 10)},
      {line(100, 0, 0, 0), line(0, 10, 100, 10)},
      {line(20, 0, 80, 0), line(0, 10, 100, 10)},
      {line(0, 0, 100, 100), line(0, 10, 100, 110)},
    };
    for (FloatLine[] c : cases) {
      System.out.println(
          "segmentProjection from=" + fl(c[0]) + " to=" + fl(c[1]) + " => " + fl((FloatLine)
              m.invoke(null, c[0], c[1])));
    }
  }

  static String box(IntBox b) {
    return "[" + b.ll.x + "," + b.ll.y + ".." + b.ur.x + "," + b.ur.y + "]";
  }

  static FloatLine line(double ax, double ay, double bx, double by) {
    return new FloatLine(new FloatPoint(ax, ay), new FloatPoint(bx, by));
  }
}
