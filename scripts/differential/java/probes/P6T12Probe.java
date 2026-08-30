// Plan 6 Task 12 ground-truth probe: `MazeSearchEngine`'s room-door expansion and cost model
// (`autoroute/maze/MazeSearchEngine.java:390-626`, `:629-705`, `:707-761`, `:791-966`,
// `:1105-1128`, `:1130-1205`, `:1207-1215`) and `autoroute/maze/MazeTraceShover.java:32-314`.
// It is not a differential driver — there is no Rust twin and `run.sh` does not know it — but
// every literal in `crates/fr-router/tests/maze_expand.rs` is read off its stdout, so it is
// committed here to keep those numbers reproducible.
//
// It declares `package app.freerouting.autoroute.maze` so it can read `MazeSearchEngine`'s
// package-private `mazeExpansionList` / `ctrl` / `destinationDistance`, `MazeListElement`'s
// package-private fields and `MazeTraceShover.DoorSection`'s, and reflect into the eight private
// methods under test. It compiles together with `P6T11Probe.java`, whose `build()` supplies the
// shared board, against the clone's HEAD jar exactly as `run.sh`'s jar mode does:
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential/java/probes
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p6t12 P6T11Probe.java P6T12Probe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p6t12:$JAR" \
//       app.freerouting.autoroute.maze.P6T12Probe <mode> 2>/dev/null \
//     | grep -Ev '^[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9:.]+ +(WARN|INFO|DEBUG|ERROR) '
//
// The `grep` strips `FRLogger`'s timestamped lines, which mode `thick` emits on stdout from
// `MazeSearchEngine:1119` and which would otherwise make the transcript non-reproducible.
//
// Modes:
//   ctrl       the `AutorouteControl` fields the expansion reads, on the shared board
//   pop        one `occupyNextElement` past `expandToRoomDoors`, with vias switched off so the
//              drill pages of `:601-623` (Task 13) stay out of it: the queue before and after
//   pop2       the same, two pops
//   inactive   `expandToRoomDoors` with `layerActive[0] = false`: nothing expanded, `true` returned
//   bend       `expandToDoorSection` over three direction pairs straddling sin^2 = 0.01, plus the
//              `addCosts`/`adjustment` matrix that drives `roomRipped` and `ripupCost`
//   thick      `roomShapeIsThick` on a trace room and on a via room
//   neck       `checkNeckDownAtDestPin` on a room with and without a pin target door
//   smalldoor  `expandToRoomDoors` through a door that `doorIsSmall` refuses
//   snapshot   the door-list snapshot of `:559`: the doors visited in one round
//   shove      `shoveTraceRoom` / `MazeTraceShover.checkShoveTraceLine` on a trace obstacle room
//   stale      `expandToTargetDoors`' stale-tree-entry `continue` (`:656-668`)
package app.freerouting.autoroute.maze;

import app.freerouting.autoroute.expansion.CompleteExpansionRoom;
import app.freerouting.autoroute.expansion.CompleteFreeSpaceExpansionRoom;
import app.freerouting.autoroute.expansion.ExpandableObject;
import app.freerouting.autoroute.expansion.ExpansionDoor;
import app.freerouting.autoroute.expansion.ObstacleExpansionRoom;
import app.freerouting.autoroute.expansion.TargetItemExpansionDoor;
import app.freerouting.board.model.items.Item;
import app.freerouting.geometry.planar.FloatLine;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.LinkedList;

/** Task 12 ground truth: the room-door expansion, the cost model and the trace shover. */
public class P6T12Probe {

  public static void main(String[] args) throws Exception {
    String mode = args.length > 0 ? args[0] : "pop";
    switch (mode) {
      case "ctrl" -> ctrl();
      case "pop" -> pop(1);
      case "pop2" -> pop(2);
      case "inactive" -> inactive();
      case "bend" -> bend();
      case "thick" -> thick();
      case "neck" -> neck();
      case "smalldoor" -> smallDoor();
      case "snapshot" -> snapshot();
      case "shove" -> shove();
      case "stale" -> stale();
      default -> throw new IllegalArgumentException("mode " + mode);
    }
  }

  // --- reflection handles ----------------------------------------------------------------------

  static Method priv(String name, Class<?>... params) throws Exception {
    Method m = MazeSearchEngine.class.getDeclaredMethod(name, params);
    m.setAccessible(true);
    return m;
  }

  static MazeSearchEngine instance(AutorouteEngine autorouteEngine, AutorouteControl control)
      throws Exception {
    return new MazeSearchEngine(autorouteEngine, control);
  }

  // --- modes -----------------------------------------------------------------------------------

  static void ctrl() {
    P6T11Probe.build();
    AutorouteControl c = P6T11Probe.control(1);
    System.out.println("netNumber=" + c.netNumber);
    System.out.println("layerCount=" + c.layerCount);
    System.out.println("layerActive=" + java.util.Arrays.toString(c.layerActive));
    System.out.println("traceHalfWidth=" + java.util.Arrays.toString(c.traceHalfWidth));
    System.out.println(
        "compensatedTraceHalfWidth=" + java.util.Arrays.toString(c.compensatedTraceHalfWidth));
    System.out.println("bendCosts=" + java.util.Arrays.toString(c.bendCosts));
    for (int i = 0; i < c.layerCount; i++) {
      System.out.println(
          "traceCosts[" + i + "] horizontal=" + c.traceCosts[i].horizontal()
              + " vertical=" + c.traceCosts[i].vertical());
    }
    System.out.println("withNeckdown=" + c.withNeckdown);
    System.out.println("viasAllowed=" + c.viasAllowed);
    System.out.println("ripupAllowed=" + c.ripupAllowed);
    System.out.println("isFanout=" + c.isFanout);
    System.out.println("maxShoveTraceRecursionDepth=" + c.maxShoveTraceRecursionDepth);
    System.out.println("maxShoveViaRecursionDepth=" + c.maxShoveViaRecursionDepth);
    System.out.println("maxSpringOverRecursionDepth=" + c.maxSpringOverRecursionDepth);
    System.out.println("traceClearanceClassIndex=" + c.traceClearanceClassIndex);
  }

  static void pop(int pops) throws Exception {
    P6T11Probe.build();
    AutorouteEngine autorouteEngine = P6T11Probe.engine(new P6T11Probe.Counter(0), 1);
    AutorouteControl control = P6T11Probe.control(1);
    // The drill-page block of `:601-623` is Task 13's; switching vias off keeps this mode inside
    // Task 12's surface without changing anything else.
    control.viasAllowed = false;
    MazeSearchEngine maze =
        MazeSearchEngine.getInstance(
            P6T11Probe.setOf(2), P6T11Probe.setOf(3), autorouteEngine, control);
    System.out.println("instance=" + (maze == null ? "null" : "ok"));
    System.out.println("--- before");
    P6T11Probe.dumpQueue(maze);
    for (int i = 0; i < pops; i++) {
      boolean more = maze.occupyNextElement();
      System.out.println("--- after pop " + (i + 1) + " occupyNextElement=" + more);
      System.out.println("completeRooms n=" + roomCount(autorouteEngine));
      P6T11Probe.dumpQueue(maze);
    }
  }

  static int roomCount(AutorouteEngine autorouteEngine) throws Exception {
    Field f = AutorouteEngine.class.getDeclaredField("completeExpansionRooms");
    f.setAccessible(true);
    java.util.List<?> list = (java.util.List<?>) f.get(autorouteEngine);
    return list == null ? -1 : list.size();
  }

  static void inactive() throws Exception {
    P6T11Probe.build();
    AutorouteEngine autorouteEngine = P6T11Probe.engine(new P6T11Probe.Counter(0), 1);
    AutorouteControl control = P6T11Probe.control(1);
    control.viasAllowed = false;
    MazeSearchEngine maze =
        MazeSearchEngine.getInstance(
            P6T11Probe.setOf(2), P6T11Probe.setOf(3), autorouteEngine, control);
    MazeListElement first = maze.mazeExpansionList.iterator().next();
    // `:396-401`: an inactive **signal** layer answers true with nothing expanded.
    control.layerActive[0] = false;
    System.out.println("layer0isSignal=" + P6T11Probe.board.layerStructure.layers[0].isSignal);
    Method m = priv("expandToRoomDoors", MazeListElement.class);
    System.out.println("expandToRoomDoors=" + m.invoke(maze, first));
    P6T11Probe.dumpQueue(maze);
  }

  // --- the cost model --------------------------------------------------------------------------

  /**
   * Two complete free-space rooms meeting on the vertical segment x = `x`, y in [`y` - 100,
   * `y` + 100], and the 1-dimensional door between them — whose `getShape().centreOfGravity()`
   * is therefore (`x`, `y`) and whose `getSectionSegments` allocates exactly one section.
   */
  static ExpansionDoor doorAt(int x, int y, int id1, int id2) {
    CompleteFreeSpaceExpansionRoom a =
        new CompleteFreeSpaceExpansionRoom(new IntBox(x - 500, y - 100, x, y + 100), 0, id1);
    CompleteFreeSpaceExpansionRoom b =
        new CompleteFreeSpaceExpansionRoom(new IntBox(x, y - 100, x + 500, y + 100), 0, id2);
    return new ExpansionDoor(a, b, 1);
  }

  static void bend() throws Exception {
    P6T11Probe.build();
    AutorouteEngine autorouteEngine = P6T11Probe.engine(new P6T11Probe.Counter(0), 1);
    AutorouteControl control = P6T11Probe.control(1);
    MazeSearchEngine maze =
        MazeSearchEngine.getInstance(
            P6T11Probe.setOf(2), P6T11Probe.setOf(3), autorouteEngine, control);
    Method m =
        priv(
            "expandToDoorSection",
            ExpandableObject.class,
            int.class,
            FloatLine.class,
            MazeListElement.class,
            int.class,
            MazeSearchElement.Adjustment.class);

    // The room the `from` element sits in, on layer 0.
    CompleteFreeSpaceExpansionRoom fromRoom =
        new CompleteFreeSpaceExpansionRoom(new IntBox(-5000, -5000, 5000, 5000), 0, 900);
    // The backtrack door's shape is centred on the origin, so `backtrackCog` is (0,0).
    ExpansionDoor backtrackDoor = doorAt(0, 0, 901, 902);

    // The shared board's settings answer `bendCosts = [0.0, 0.0]`, which would make the whole
    // `:856-873` block dead; the array is `final` but its contents are not, and the production
    // path fills it from the trace costs.
    control.bendCosts[0] = 100.0;
    System.out.println("bendCosts[0]=" + control.bendCosts[0]);
    System.out.println(
        "traceCosts[0] horizontal=" + control.traceCosts[0].horizontal()
            + " vertical=" + control.traceCosts[0].vertical());
    System.out.println("backtrackCog=" + P6T11Probe.fp(backtrackDoor.getShape().centreOfGravity()));

    // 99 * dy^2 > dx^2 is the threshold with dx = 1000; dy = 100 is below, dy = 101 above.
    for (int dy : new int[] {0, 100, 101, 1000}) {
      for (int[] costs : new int[][] {{0, 0}, {7, 0}, {7, 1}, {7, 2}}) {
        maze.mazeExpansionList.clear();
        ExpansionDoor toDoor = doorAt(2000, dy, 903, 904);
        System.out.println(
            "  sections=" + toDoor.getSectionSegments(control.compensatedTraceHalfWidth[0]).length);
        MazeListElement from =
            new MazeListElement(
                backtrackDoor,
                0,
                backtrackDoor,
                0,
                0.0,
                0.0,
                fromRoom,
                new FloatLine(new FloatPoint(1000, 0), new FloatPoint(1000, 0)),
                false,
                MazeSearchElement.Adjustment.NONE,
                false);
        FloatLine shapeEntry =
            new FloatLine(new FloatPoint(2000, dy), new FloatPoint(2000, dy));
        MazeSearchElement.Adjustment adjustment =
            MazeSearchElement.Adjustment.values()[costs[1]];
        Object ok = m.invoke(maze, toDoor, 0, shapeEntry, from, costs[0], adjustment);
        System.out.println(
            "expandToDoorSection dy=" + dy + " addCosts=" + costs[0] + " adjustment=" + adjustment
                + " => " + ok);
        P6T11Probe.dumpQueue(maze);
      }
    }

    // The second `roomRipped` clause of `:885-887`: a parent that was already checked and ripped.
    maze.mazeExpansionList.clear();
    ExpansionDoor toDoor = doorAt(2000, 0, 905, 906);
    System.out.println(
        "sections=" + toDoor.getSectionSegments(control.compensatedTraceHalfWidth[0]).length);
    MazeListElement rippedParent =
        new MazeListElement(
            backtrackDoor,
            0,
            backtrackDoor,
            0,
            0.0,
            0.0,
            fromRoom,
            new FloatLine(new FloatPoint(1000, 0), new FloatPoint(1000, 0)),
            true,
            MazeSearchElement.Adjustment.NONE,
            true);
    System.out.println(
        "expandToDoorSection rippedParent => "
            + m.invoke(
                maze,
                toDoor,
                0,
                new FloatLine(new FloatPoint(2000, 0), new FloatPoint(2000, 0)),
                rippedParent,
                0,
                MazeSearchElement.Adjustment.NONE));
    P6T11Probe.dumpQueue(maze);

    // The `:798-849` refusals: an occupied section, and a null shape entry.
    maze.mazeExpansionList.clear();
    toDoor.getMazeSearchElement(0).isOccupied = true;
    System.out.println(
        "expandToDoorSection occupied => "
            + m.invoke(
                maze,
                toDoor,
                0,
                new FloatLine(new FloatPoint(2000, 0), new FloatPoint(2000, 0)),
                rippedParent,
                0,
                MazeSearchElement.Adjustment.NONE));
    toDoor.getMazeSearchElement(0).isOccupied = false;
    System.out.println(
        "expandToDoorSection nullEntry => "
            + m.invoke(maze, toDoor, 0, null, rippedParent, 0, MazeSearchElement.Adjustment.NONE));
    System.out.println("queue n=" + maze.mazeExpansionList.size());
  }

  /**
   * A board carrying a thin trace, a thick trace and a via on net 2, so `roomShapeIsThick` has
   * one room of each of its three arms. It is the shared board plus three inserted items, which
   * is safe because this mode never calls `init` (`completeShape` is what the fixture trap of
   * Task 11 report section 4 is about).
   */
  static void thick() throws Exception {
    P6T11Probe.build();
    AutorouteEngine autorouteEngine = P6T11Probe.engine(new P6T11Probe.Counter(0), 1);
    AutorouteControl control = P6T11Probe.control(1);
    MazeSearchEngine maze = instance(autorouteEngine, control);
    Method m = priv("roomShapeIsThick", ObstacleExpansionRoom.class);
    System.out.println("compensatedTraceHalfWidth[0]=" + control.compensatedTraceHalfWidth[0]);
    System.out.println(
        "clearanceCompensationValue(1,0)="
            + autorouteEngine.autorouteSearchTree.clearanceCompensationValue(1, 0));
    // The obstacle area (item 4) is neither a trace nor a via: the `FRLogger.warn` arm.
    ObstacleExpansionRoom areaRoom =
        new ObstacleExpansionRoom(P6T11Probe.itemById(4), 0, autorouteEngine.autorouteSearchTree);
    System.out.println("obstacleArea => " + m.invoke(maze, areaRoom));
    // The through pin (item 3) is a DrillItem but not a Via: the same arm.
    ObstacleExpansionRoom pinRoom =
        new ObstacleExpansionRoom(P6T11Probe.itemById(3), 0, autorouteEngine.autorouteSearchTree);
    System.out.println("pin => " + m.invoke(maze, pinRoom));

    for (int halfWidth : new int[] {100, 1500, 1600, 2000}) {
      P6T11Probe.board.insertTraceWithoutCleaning(
          new Polyline(
              new Point[] {
                new IntPoint(-9000, 5000 + halfWidth), new IntPoint(-3000, 5000 + halfWidth)
              }),
          0,
          halfWidth,
          new int[] {2},
          1,
          FixedState.UNFIXED);
    }
    autorouteEngine = P6T11Probe.engine(new P6T11Probe.Counter(0), 1);
    control = P6T11Probe.control(1);
    maze = instance(autorouteEngine, control);
    for (Item current : P6T11Probe.board.getItems()) {
      if (!(current instanceof app.freerouting.board.trace.PolylineTrace trace)) {
        continue;
      }
      ObstacleExpansionRoom room =
          new ObstacleExpansionRoom(current, 0, autorouteEngine.autorouteSearchTree);
      System.out.println(
          "trace id=" + current.getId()
              + " halfWidth=" + trace.getHalfWidth()
              + " clearanceClass=" + current.clearanceClassIndex()
              + " roomShapeIsThick=" + m.invoke(maze, room));
    }
  }

  static void neck() throws Exception {
    P6T11Probe.build();
    AutorouteEngine autorouteEngine = P6T11Probe.engine(new P6T11Probe.Counter(0), 1);
    AutorouteControl control = P6T11Probe.control(1);
    MazeSearchEngine maze =
        MazeSearchEngine.getInstance(
            P6T11Probe.setOf(2), P6T11Probe.setOf(3), autorouteEngine, control);
    Method m = priv("checkNeckDownAtDestPin", CompleteExpansionRoom.class);
    for (MazeListElement e : maze.mazeExpansionList) {
      CompleteExpansionRoom room = e.nextRoom;
      System.out.println(
          "room=" + room.getId() + " layer=" + room.getLayer()
              + " targetDoors=" + room.getTargetDoors().size()
              + " checkNeckDownAtDestPin=" + m.invoke(maze, room));
    }
    CompleteFreeSpaceExpansionRoom bare =
        new CompleteFreeSpaceExpansionRoom(new IntBox(0, 0, 10, 10), 0, 950);
    System.out.println("bareRoom checkNeckDownAtDestPin=" + m.invoke(maze, bare));
  }

  /**
   * `expandToRoomDoors` entered through an `ExpansionDoor` rather than a target door, so `:405`
   * bites and `doorIsSmall` is consulted (`:415`). At the control's own half width the door is
   * big enough and the round expands four elements; at an absurd half width `doorIsSmall`
   * answers true, `:501-511` returns `somethingExpanded` (false) and nothing is expanded.
   */
  static void smallDoor() throws Exception {
    Method small = priv("doorIsSmall", ExpansionDoor.class, double.class);
    Method m = priv("expandToRoomDoors", MazeListElement.class);
    for (int halfWidth : new int[] {1600, 100000}) {
      P6T11Probe.build();
      AutorouteEngine autorouteEngine = P6T11Probe.engine(new P6T11Probe.Counter(0), 1);
      AutorouteControl control = P6T11Probe.control(1);
      control.viasAllowed = false;
      MazeSearchEngine maze =
          MazeSearchEngine.getInstance(
              P6T11Probe.setOf(2), P6T11Probe.setOf(3), autorouteEngine, control);
      MazeListElement seed = maze.mazeExpansionList.iterator().next();
      CompleteExpansionRoom room = seed.nextRoom;
      ExpansionDoor door = room.getDoors().iterator().next();
      FloatPoint centre = door.getShape().centreOfGravity();
      MazeListElement element =
          new MazeListElement(
              door,
              0,
              null,
              0,
              0.0,
              0.0,
              room,
              new FloatLine(centre, centre),
              false,
              MazeSearchElement.Adjustment.NONE,
              false);
      control.compensatedTraceHalfWidth[0] = halfWidth;
      maze.mazeExpansionList.clear();
      System.out.println(
          "room=" + room.getId() + " door=" + door.getId() + " dimension=" + door.dimension
              + " doorShape=" + P6T11Probe.box(door.getShape().boundingBox())
              + " shapeEntry=" + P6T11Probe.fp(centre));
      System.out.println(
          "halfWidth=" + halfWidth
              + " doorIsSmall=" + small.invoke(maze, door, 2.0 * (halfWidth + 2))
              + " expandToRoomDoors=" + m.invoke(maze, element));
      P6T11Probe.dumpQueue(maze);
    }
  }

  static void snapshot() throws Exception {
    P6T11Probe.build();
    AutorouteEngine autorouteEngine = P6T11Probe.engine(new P6T11Probe.Counter(0), 1);
    AutorouteControl control = P6T11Probe.control(1);
    control.viasAllowed = false;
    MazeSearchEngine maze =
        MazeSearchEngine.getInstance(
            P6T11Probe.setOf(2), P6T11Probe.setOf(3), autorouteEngine, control);
    MazeListElement seed = maze.mazeExpansionList.iterator().next();
    CompleteExpansionRoom room = seed.nextRoom;
    System.out.println("room=" + room.getId() + " doorsBefore=" + room.getDoors().size());
    LinkedList<ExpansionDoor> before = new LinkedList<>(room.getDoors());
    autorouteEngine.completeNeighbourRooms(room);
    System.out.println("doorsAfterCompletion=" + room.getDoors().size());
    int i = 0;
    for (ExpansionDoor d : room.getDoors()) {
      System.out.println(
          "  door[" + (i++) + "] id=" + d.getId() + " dimension=" + d.dimension
              + " inSnapshot=" + before.contains(d)
              + " shape=" + P6T11Probe.box(d.getShape().boundingBox()));
    }
  }

  /**
   * `shoveTraceRoom` (`:1130-1205`) and `MazeTraceShover.checkShoveTraceLine` (`:32-314`).
   *
   * The first half is the two early `true`s of `:39-44` on the shared board. The second builds a
   * net-2 obstacle trace whose half width and clearance class match the control's, so the
   * `:48-51` guard lets the algorithm through, hangs a 1-dimensional link door off the middle
   * segment's obstacle room, and reports the door sections the shover collects.
   */
  static void shove() throws Exception {
    P6T11Probe.build();
    AutorouteEngine autorouteEngine = P6T11Probe.engine(new P6T11Probe.Counter(0), 1);
    AutorouteControl control = P6T11Probe.control(1);
    control.viasAllowed = false;
    MazeSearchEngine maze =
        MazeSearchEngine.getInstance(
            P6T11Probe.setOf(2), P6T11Probe.setOf(3), autorouteEngine, control);
    System.out.println("instance=" + (maze == null ? "null" : "ok"));
    Method shoveRoom = priv("shoveTraceRoom", MazeListElement.class, ObstacleExpansionRoom.class);
    ObstacleExpansionRoom areaRoom =
        new ObstacleExpansionRoom(P6T11Probe.itemById(4), 0, autorouteEngine.autorouteSearchTree);
    MazeListElement seed = maze.mazeExpansionList.iterator().next();
    System.out.println("itemCountBefore=" + P6T11Probe.board.getItems().size());
    System.out.println("shoveTraceRoom(obstacleArea)=" + shoveRoom.invoke(maze, seed, areaRoom));
    System.out.println("itemCountAfter=" + P6T11Probe.board.getItems().size());
    java.util.Collection<MazeTraceShover.DoorSection> out = new LinkedList<>();
    System.out.println(
        "checkShoveTraceLine targetDoorFrom="
            + MazeTraceShover.checkShoveTraceLine(
                seed, areaRoom, P6T11Probe.board, control, false, out)
            + " sections=" + out.size());

    // --- the real trace room ------------------------------------------------------------------
    P6T11Probe.build();
    autorouteEngine = P6T11Probe.engine(new P6T11Probe.Counter(0), 1);
    control = P6T11Probe.control(1);
    int halfWidth = control.traceHalfWidth[0];
    P6T11Probe.board.insertTraceWithoutCleaning(
        new Polyline(
            new Point[] {
              new IntPoint(-7000, -8000),
              new IntPoint(0, -8000),
              new IntPoint(0, -4500),
              new IntPoint(6000, -4500)
            }),
        0,
        halfWidth,
        new int[] {2},
        control.traceClearanceClassIndex,
        FixedState.UNFIXED);
    autorouteEngine = P6T11Probe.engine(new P6T11Probe.Counter(0), 1);
    maze = instance(autorouteEngine, control);
    Item traceItem = null;
    for (Item current : P6T11Probe.board.getItems()) {
      if (current instanceof app.freerouting.board.trace.PolylineTrace) {
        traceItem = current;
      }
    }
    app.freerouting.board.trace.PolylineTrace trace =
        (app.freerouting.board.trace.PolylineTrace) traceItem;
    System.out.println(
        "trace id=" + trace.getId()
            + " halfWidth=" + trace.getHalfWidth()
            + " clearanceClass=" + trace.clearanceClassIndex()
            + " lines=" + trace.polyline().lines.length
            + " treeShapes=" + trace.treeShapeCount(autorouteEngine.autorouteSearchTree));
    for (int i = 0; i < trace.treeShapeCount(autorouteEngine.autorouteSearchTree); i++) {
      System.out.println(
          "  treeShape[" + i + "]="
              + P6T11Probe.box(
                  trace.getTreeShape(autorouteEngine.autorouteSearchTree, i).boundingBox()));
    }
    for (int cornerNo : new int[] {0, 1, 2}) {
      shoveOne(maze, control, autorouteEngine, trace, cornerNo);
    }
    // The `fromDoor.dimension == 2` branch of `:73-96`: a link door between the obstacle rooms of
    // two consecutive segments of the *same* trace, so `endPointsMatching` (`:321-322`) is the
    // identity arm.
    ObstacleExpansionRoom seg1 =
        new ObstacleExpansionRoom(trace, 1, autorouteEngine.autorouteSearchTree);
    ObstacleExpansionRoom seg2 =
        new ObstacleExpansionRoom(trace, 2, autorouteEngine.autorouteSearchTree);
    ExpansionDoor linkDoor = new ExpansionDoor(seg1, seg2, 2);
    seg2.addDoor(linkDoor);
    ExpansionDoor seg2Upper =
        new ExpansionDoor(
            new CompleteFreeSpaceExpansionRoom(
                new IntBox(3000, -2900, 7600, 3000), 0, 8500),
            seg2,
            1);
    seg2.addDoor(seg2Upper);
    ExpansionDoor seg2Lower =
        new ExpansionDoor(
            new CompleteFreeSpaceExpansionRoom(
                new IntBox(3000, -9000, 7600, -6100), 0, 8600),
            seg2,
            1);
    seg2.addDoor(seg2Lower);
    int linkSections = linkDoor.getSectionSegments(control.compensatedTraceHalfWidth[0]).length;
    FloatPoint linkCentre = linkDoor.getShape().centreOfGravity();
    System.out.println(
        "--- linkDoor id=" + linkDoor.getId()
            + " shape=" + P6T11Probe.box(linkDoor.getShape().boundingBox())
            + " maxWidth=" + linkDoor.getShape().maxWidth()
            + " sections=" + linkSections
            + " upper=" + seg2Upper.getId()
            + " lower=" + seg2Lower.getId());
    for (boolean left : new boolean[] {false, true}) {
      MazeListElement linkElement =
          new MazeListElement(
              linkDoor,
              0,
              null,
              0,
              0.0,
              0.0,
              seg2,
              new FloatLine(linkCentre, linkCentre),
              false,
              MazeSearchElement.Adjustment.NONE,
              false);
      java.util.Collection<MazeTraceShover.DoorSection> linkOut = new LinkedList<>();
      int itemsBefore = P6T11Probe.board.getItems().size();
      boolean ok =
          MazeTraceShover.checkShoveTraceLine(
              linkElement, seg2, P6T11Probe.board, control, left, linkOut);
      StringBuilder sb = new StringBuilder();
      for (MazeTraceShover.DoorSection ds : linkOut) {
        sb.append(" [door=")
            .append(ds.door.getId())
            .append(" section=")
            .append(ds.sectionIndex)
            .append(" line=")
            .append(P6T11Probe.fl(ds.sectionLine))
            .append("]");
      }
      System.out.println(
          "  link left=" + left + " => " + ok + " sections=" + linkOut.size() + sb
              + " itemsUnchanged=" + (itemsBefore == P6T11Probe.board.getItems().size()));
      // And through `shoveTraceRoom`, which is what routes the answer back into the queue.
      maze.mazeExpansionList.clear();
      Method shoveRoom2 = priv("shoveTraceRoom", MazeListElement.class, ObstacleExpansionRoom.class);
      System.out.println("  shoveTraceRoom left=" + left + " => " + shoveRoom2.invoke(maze, linkElement, seg2));
      P6T11Probe.dumpQueue(maze);
    }

    // Hazard N (`:65-66`): the room keeps the index it was created with while the trace's
    // polyline shrinks underneath it, so `traceCornerNo >= lines.length - 2` and the method
    // answers false with an empty door list — no warning, no throw.
    ObstacleExpansionRoom staleRoom =
        new ObstacleExpansionRoom(trace, 2, autorouteEngine.autorouteSearchTree);
    ExpansionDoor staleDoor =
        new ExpansionDoor(
            new CompleteFreeSpaceExpansionRoom(
                staleRoom.getShape().boundingBox().offset(1000), 0, 8900),
            staleRoom,
            1);
    staleRoom.addDoor(staleDoor);
    staleDoor.getSectionSegments(control.compensatedTraceHalfWidth[0]);
    FloatPoint staleAnchor = staleDoor.getShape().centreOfGravity();
    MazeListElement staleElement =
        new MazeListElement(
            staleDoor,
            0,
            null,
            0,
            0.0,
            0.0,
            staleRoom,
            new FloatLine(staleAnchor, staleAnchor),
            false,
            MazeSearchElement.Adjustment.NONE,
            false);
    java.lang.reflect.Method setPolyline =
        app.freerouting.board.trace.PolylineTrace.class.getDeclaredMethod(
            "setPolyline", Polyline.class);
    setPolyline.setAccessible(true);
    setPolyline.invoke(
        trace, new Polyline(new Point[] {new IntPoint(-7000, -8000), new IntPoint(0, -8000)}));
    System.out.println(
        "--- stale indexInItem=" + staleRoom.getIndexInItem()
            + " lines=" + trace.polyline().lines.length);
    java.util.Collection<MazeTraceShover.DoorSection> staleSections = new LinkedList<>();
    System.out.println(
        "  checkShoveTraceLine="
            + MazeTraceShover.checkShoveTraceLine(
                staleElement, staleRoom, P6T11Probe.board, control, false, staleSections)
            + " sections=" + staleSections.size());
  }

  static void shoveOne(
      MazeSearchEngine maze,
      AutorouteControl control,
      AutorouteEngine autorouteEngine,
      app.freerouting.board.trace.PolylineTrace trace,
      int cornerNo)
      throws Exception {
    ObstacleExpansionRoom obstacleRoom =
        new ObstacleExpansionRoom(trace, cornerNo, autorouteEngine.autorouteSearchTree);
    IntBox roomBox = obstacleRoom.getShape().boundingBox();
    int midX = (roomBox.ll.x + roomBox.ur.x) / 2;
    int midY = (roomBox.ll.y + roomBox.ur.y) / 2;
    // fromDoor on the upper edge, left half; a second door on the upper edge, right half; and one
    // on the lower edge, so the collector of `:236-312` has a candidate on either side.
    ExpansionDoor fromDoor =
        new ExpansionDoor(
            new CompleteFreeSpaceExpansionRoom(
                new IntBox(roomBox.ll.x, roomBox.ur.y, midX, roomBox.ur.y + 6000),
                0,
                8000 + cornerNo),
            obstacleRoom,
            1);
    ExpansionDoor upperRight =
        new ExpansionDoor(
            new CompleteFreeSpaceExpansionRoom(
                new IntBox(midX, roomBox.ur.y, roomBox.ur.x, roomBox.ur.y + 6000),
                0,
                8100 + cornerNo),
            obstacleRoom,
            1);
    ExpansionDoor lower =
        new ExpansionDoor(
            new CompleteFreeSpaceExpansionRoom(
                new IntBox(midX, roomBox.ll.y - 6000, roomBox.ur.x, roomBox.ll.y),
                0,
                8200 + cornerNo),
            obstacleRoom,
            1);
    ExpansionDoor leftEdge =
        new ExpansionDoor(
            new CompleteFreeSpaceExpansionRoom(
                new IntBox(roomBox.ll.x - 6000, midY, roomBox.ll.x, roomBox.ur.y),
                0,
                8300 + cornerNo),
            obstacleRoom,
            1);
    obstacleRoom.addDoor(fromDoor);
    obstacleRoom.addDoor(upperRight);
    obstacleRoom.addDoor(lower);
    obstacleRoom.addDoor(leftEdge);
    int sectionCount = fromDoor.getSectionSegments(control.compensatedTraceHalfWidth[0]).length;
    FloatLine doorSegment = fromDoor.getShape().diagonalCornerSegment();
    System.out.println(
        "--- cornerNo=" + cornerNo
            + " roomBox=" + P6T11Probe.box(roomBox)
            + " indexInItem=" + obstacleRoom.getIndexInItem()
            + " lines=" + trace.polyline().lines.length
            + " fromDoor=" + fromDoor.getId()
            + " upperRight=" + upperRight.getId()
            + " lower=" + lower.getId()
            + " leftEdge=" + leftEdge.getId()
            + " doorSections=" + sectionCount
            + " doorSegment=" + P6T11Probe.fl(doorSegment));
    for (int sectionNo : new int[] {0, sectionCount - 1}) {
      for (boolean left : new boolean[] {false, true}) {
        for (boolean atA : new boolean[] {true, false}) {
          FloatPoint anchor = atA ? doorSegment.a : doorSegment.b;
          MazeListElement element =
              new MazeListElement(
                  fromDoor,
                  sectionNo,
                  null,
                  0,
                  0.0,
                  0.0,
                  obstacleRoom,
                  new FloatLine(anchor, anchor),
                  false,
                  MazeSearchElement.Adjustment.NONE,
                  false);
          java.util.Collection<MazeTraceShover.DoorSection> sections = new LinkedList<>();
          int itemsBefore = P6T11Probe.board.getItems().size();
          boolean ok =
              MazeTraceShover.checkShoveTraceLine(
                  element, obstacleRoom, P6T11Probe.board, control, left, sections);
          StringBuilder sb = new StringBuilder();
          for (MazeTraceShover.DoorSection ds : sections) {
            sb.append(" [door=")
                .append(ds.door.getId())
                .append(" section=")
                .append(ds.sectionIndex)
                .append(" line=")
                .append(P6T11Probe.fl(ds.sectionLine))
                .append("]");
          }
          System.out.println(
              "  section=" + sectionNo + " left=" + left + " entryAtA=" + atA
                  + " => " + ok
                  + " sections=" + sections.size() + sb
                  + " itemsUnchanged=" + (itemsBefore == P6T11Probe.board.getItems().size()));
        }
      }
    }
  }

  static void stale() throws Exception {
    P6T11Probe.build();
    AutorouteEngine autorouteEngine = P6T11Probe.engine(new P6T11Probe.Counter(0), 1);
    AutorouteControl control = P6T11Probe.control(1);
    control.viasAllowed = false;
    MazeSearchEngine maze =
        MazeSearchEngine.getInstance(
            P6T11Probe.setOf(2), P6T11Probe.setOf(3), autorouteEngine, control);
    MazeListElement seed = maze.mazeExpansionList.iterator().next();
    CompleteExpansionRoom room = seed.nextRoom;
    Method m =
        priv(
            "expandToTargetDoors",
            MazeListElement.class,
            boolean.class,
            boolean.class,
            FloatPoint.class);
    FloatPoint mid = seed.shapeEntry.a.middlePoint(seed.shapeEntry.b);
    maze.mazeExpansionList.clear();
    System.out.println(
        "expandToTargetDoors healthy=" + m.invoke(maze, seed, true, false, mid));
    P6T11Probe.dumpQueue(maze);

    // Now make every target door's `treeEntryNo` stale and repeat: `:656-658` skips them all.
    maze.mazeExpansionList.clear();
    for (TargetItemExpansionDoor d : room.getTargetDoors()) {
      Field f = TargetItemExpansionDoor.class.getDeclaredField("treeEntryNo");
      f.setAccessible(true);
      f.setInt(d, 99);
      System.out.println(
          "targetDoor item=" + d.item.getId() + " treeEntryNo=" + d.treeEntryNo
              + " treeShapeCount=" + d.item.treeShapeCount(autorouteEngine.autorouteSearchTree));
    }
    System.out.println("expandToTargetDoors stale=" + m.invoke(maze, seed, true, false, mid));
    P6T11Probe.dumpQueue(maze);
    System.out.println("itemCount=" + P6T11Probe.board.getItems().size());
  }
}
