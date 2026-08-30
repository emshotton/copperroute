// Plan 6 Task 16 ground-truth probe: `autoroute/maze/AutorouteEngine.java:130-280`
// (`autorouteConnection`) and `:282-287` (`describeConnection`), plus the Plan 6 half of
// `autoroute/pipeline/AutorouteConnectionRouter.java:30-100` (`route` steps 1-5, plan-6 ruling 2)
// transcribed inline — `AutorouteConnectionRouter` itself is package-private in
// `autoroute.pipeline` and its constructor needs a whole `BatchAutorouter`, which is Plan 7's.
//
// It is not a differential driver — there is no Rust twin and `run.sh` does not know it — but
// every literal in `crates/fr-router/tests/autoroute_connection.rs` is read off its stdout, so it
// is committed here to keep those numbers reproducible.
//
// It declares `package app.freerouting.autoroute.path` so it can reuse `P6T15Probe`'s
// package-private `boardDump`/`poly`/`nets`/`pointOf` dump helpers and `P6T14Probe`'s
// package-private `buildBlocked`/`setOf`/`board`, and it reflects into the private static
// `AutorouteEngine.describeConnection` (`:282-287`). `autorouteConnection`, `initConnection`, the
// `AutorouteEngine` constructor and `RoutingBoard.initAutoroute` are all public.
//
// It compiles together with `P6T11Probe.java`, `P6T13Probe.java`, `P6T14Probe.java` and
// `P6T15Probe.java` against the clone's HEAD jar exactly as `run.sh`'s jar mode does:
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential/java/probes
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p6t16 \
//       P6T11Probe.java P6T13Probe.java P6T14Probe.java P6T15Probe.java P6T16Probe.java
//   for m in plain via ripup nomaze nopath locatorfail stop stopafter inactive maintain \
//            route routeripup plane describe; do
//     echo "=== mode $m ==="
//     "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//         -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p6t16:$JAR" \
//         app.freerouting.autoroute.path.P6T16Probe "$m" 2>/dev/null \
//       | grep -Ev '^[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9:.]+ +(WARN|INFO|DEBUG|ERROR|TRACE) ' \\
//       | grep -Ev '^(\s+at |\s+\.\.\. |Caused by: |java\.)'
//   done > crates/fr-router/tests/data/p6t16-autoroute-connection.txt
//
// The `=== mode X ===` banners of the committed transcript come from this loop, not from the
// probe. The `grep` strips `FRLogger`'s timestamped lines.
//
// Modes:
//   plain       `P6T13Probe.buildSimple`'s two-pin board, all three regimes: a plain ROUTED with
//               an empty ripped set and one inserted trace
//   via         `P6T13Probe.build`'s board, all three regimes: the connection crosses two
//               `ExpansionDrill`s, so the insert lays down two vias (a layer change)
//   ripup       `P6T14Probe.buildBlocked` with `viasAllowed` off and ripup on, all three regimes:
//               the search rips the net-2 blocker, so `:238-263`'s ripped-connection deletion
//               runs with a non-empty `rippedItemList` and the board loses an item
//   nomaze      the blocker taken all the way to the outline, which makes
//               `MazeSearchEngine.getInstance` answer null: `:145-151`'s FAILED — plus the room
//               and tree-leaf counts afterwards, which is quirk #188's evidence that this one
//               early return skips the cleanup of `:198-205`
//   nopath      the same blocker 100 units short of the outline with `viasAllowed` off and ripup
//               off, so `findConnection` answers null: `:207-213`'s FAILED
//   locatorfail the `ripup` board with an *unmodifiable* `rippedItemList`, so
//               `backtrack:316-323`'s `add` throws inside `FoundConnectionLocator.getInstance` and
//               `:190` catches it: `:215-219`'s message-less FAILED. This is ruling 7's fourth
//               recovery boundary, and it is the only way to reach `:215` — `getInstance` itself
//               answers null only for a null `mazeSearchResult`, which `:180` has already tested
//   stop        `plain` with a `Stoppable` that always answers true, which trips inside
//               `MazeSearchEngine.init` and makes `getInstance` answer null: `:145-151`'s FAILED
//   stopafter   the same with a `Stoppable` that answers false a fixed number of times first, so
//               the trip lands in the pop loop and `findConnection` answers null: `:207-213`
//   inactive    `plain` with layer 0 turned into a dedicated **power plane**, which is the only
//               way a located `startLayer` can be one `layerActive` says is off: `:221-228`'s FAILED
//   maintain    `nopath` with `maintainDatabase = true`, so `:204` takes `resetAllDoors` rather
//               than `clear`: the complete-room list, `getRoomsWithTargetItems` and the whole
//               door/`precalculatedConnection` reset survive into the next `initConnection`. The
//               connection deliberately FAILs so that nothing is inserted — see the report
//   route       `AutorouteConnectionRouter.route:36-90` transcribed inline: a routed item, the
//               same item again (NO_UNCONNECTED_NETS, `:49-52`) and a net the board does not
//               have, which makes `new AutorouteControl` throw and `:155-158` — ruling 7's fifth
//               recovery boundary — answer a bare FAILED
//   routeripup  `route` on the blocker board over three `ripupPassNo` values, which is where
//               `:45`'s `ripupCosts = startRipupCosts * ripupPassNo` reaches the ripup cost model
//   plane       `:54-68`'s plane swap and its CONNECTED_TO_PLANE guard, read off the order
//               `describeConnection` prints the two sets in
//   describe    `describeConnection` (`:282-287`) over four set shapes, which pins both the
//               `", "` join and the **descending** id order of `TreeSet<Item>`
//               (`Item.compareTo:95-102` is `other.id - this.id`)
package app.freerouting.autoroute.path;

import app.freerouting.autoroute.AutorouteAttemptResult;
import app.freerouting.autoroute.AutorouteAttemptState;
import app.freerouting.autoroute.maze.AutorouteControl;
import app.freerouting.autoroute.maze.AutorouteEngine;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.ConductionArea;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.model.structure.Layer;
import app.freerouting.datastructures.Stoppable;
import app.freerouting.datastructures.TimeLimit;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.PolylineArea;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import app.freerouting.rules.Net;
import app.freerouting.rules.ViaRule;
import app.freerouting.settings.RouterSettings;
import java.lang.reflect.Method;
import java.util.Collections;
import java.util.Map;
import java.util.Set;
import java.util.SortedSet;
import java.util.TreeMap;
import java.util.TreeSet;

/** Task 16 ground truth: `AutorouteEngine.autorouteConnection` end to end. */
public class P6T16Probe {

  static final AngleRestriction[] REGIMES = {
    AngleRestriction.NINETY_DEGREE, AngleRestriction.FORTYFIVE_DEGREE, AngleRestriction.NONE
  };

  /** A `Stoppable` whose answer is fixed at construction. */
  record Stop(boolean stopped) implements Stoppable {
    @Override
    public void requestStop() {}

    @Override
    public boolean isStopRequested() {
      return stopped;
    }
  }

  public static void main(String[] args) throws Exception {
    String mode = args.length > 0 ? args[0] : "plain";
    switch (mode) {
      case "plain" -> plain();
      case "via" -> via();
      case "ripup" -> ripup();
      case "nomaze" -> noMaze();
      case "nopath" -> noPath();
      case "locatorfail" -> locatorFail();
      case "stop" -> stop();
      case "stopafter" -> stopAfter();
      case "inactive" -> inactive();
      case "maintain" -> maintain();
      case "route" -> route();
      case "routeripup" -> routeRipup();
      case "plane" -> plane();
      case "describe" -> describe();
      default -> throw new IllegalArgumentException("unknown mode " + mode);
    }
  }

  // =============================================================================================
  // Helpers
  // =============================================================================================

  static RoutingBoard board() throws Exception {
    return P6T14Probe.board();
  }

  static Set<Item> setOf(int... ids) throws Exception {
    return P6T14Probe.setOf(ids);
  }

  /** `AutorouteEngine.describeConnection` (`:282-287`), which is `private static`. */
  static String describeConnection(Set<Item> startSet, Set<Item> destSet) throws Exception {
    Method method =
        AutorouteEngine.class.getDeclaredMethod("describeConnection", Set.class, Set.class);
    method.setAccessible(true);
    return (String) method.invoke(null, startSet, destSet);
  }

  /**
   * `RoutingBoard.initAutoroute` (`:882-897`) with a fresh engine, so that `board.autorouteEngine`
   * is the very engine the connection runs on — which is what makes
   * `RoutingBoard.additionalUpdateAfterChange:100` see a non-null field.
   */
  static AutorouteEngine initAutoroute(
      RoutingBoard board, int netNo, int clearanceClass, Stoppable stoppable, boolean maintain) {
    return board.initAutoroute(netNo, clearanceClass, stoppable, null, maintain);
  }

  static AutorouteControl control(RoutingBoard board, int netNo) {
    RouterSettings settings = new RouterSettings(board);
    return new AutorouteControl(
        board, netNo, settings, settings.getViaCosts(), settings.getTraceCosts());
  }

  static String stateOf(AutorouteAttemptResult result) {
    return result.state.toString();
  }

  /** `AutorouteAttemptResult.toString` (`:21-24`), which is what the Rust `Display` reproduces. */
  static String resultOf(AutorouteAttemptResult result) {
    return result.toString();
  }

  static String rippedOf(SortedSet<Item> ripped) {
    StringBuilder sb = new StringBuilder("ripped n=" + ripped.size() + " [");
    boolean first = true;
    for (Item item : ripped) {
      if (!first) {
        sb.append(",");
      }
      first = false;
      sb.append(item.getId());
    }
    return sb.append("]").toString();
  }

  static String ripupCostsOf(Map<Item, Integer> costs) {
    StringBuilder sb = new StringBuilder("ripupCosts n=" + costs.size() + " [");
    boolean first = true;
    for (Map.Entry<Item, Integer> entry : costs.entrySet()) {
      if (!first) {
        sb.append(",");
      }
      first = false;
      sb.append(entry.getKey().getId()).append("=").append(entry.getValue());
    }
    return sb.append("]").toString();
  }

  /** A `TreeMap` over `Item`, i.e. Java's descending-id map order (`Item.compareTo:95-102`). */
  static Map<Item, Integer> costMap() {
    return new TreeMap<>();
  }

  /**
   * One `autorouteConnection` call, printed the way every mode of this probe prints it: the
   * result, the ripped ids, the per-item ripup costs and the whole board afterwards.
   */
  static void run(
      RoutingBoard board,
      AutorouteEngine engine,
      AutorouteControl ctrl,
      Set<Item> start,
      Set<Item> dest,
      SortedSet<Item> ripped,
      Map<Item, Integer> ripupCosts) {
    int before = board.communication.idGenerator.maxGeneratedId();
    AutorouteAttemptResult result;
    try {
      result = engine.autorouteConnection(start, dest, ctrl, ripped, ripupCosts);
    } catch (RuntimeException e) {
      // `AutorouteConnectionRouter.route:155-158` is the only handler above `autorouteConnection`;
      // no mode of this probe should reach it except `route`, which brackets the call itself.
      System.out.println("  threw " + e.getClass().getName());
      return;
    }
    System.out.println("  result=" + resultOf(result));
    System.out.println("  state=" + stateOf(result));
    System.out.println("  " + rippedOf(ripped));
    System.out.println("  " + ripupCostsOf(ripupCosts));
    System.out.println("  maxIdBefore=" + before);
    System.out.println(P6T15Probe.boardDump(board));
  }

  /**
   * The default shape of a mode: build the board, set the regime, `initAutoroute`, build the
   * control and route `start` -> `dest` on net 1.
   */
  static AutorouteEngine oneRegime(
      AngleRestriction restriction, boolean noVias, boolean ripupAllowed, int[] start, int[] dest)
      throws Exception {
    RoutingBoard board = board();
    board.rules.setTraceAngleRestriction(restriction);
    AutorouteEngine engine = initAutoroute(board, 1, 1, new Stop(false), false);
    AutorouteControl ctrl = control(board, 1);
    if (noVias) {
      ctrl.viasAllowed = false;
    }
    if (ripupAllowed) {
      ctrl.ripupAllowed = true;
      ctrl.ripupCosts = 1000;
    }
    SortedSet<Item> ripped = new TreeSet<>();
    Map<Item, Integer> ripupCosts = costMap();
    run(board, engine, ctrl, setOf(start), setOf(dest), ripped, ripupCosts);
    return engine;
  }

  // =============================================================================================
  // The modes
  // =============================================================================================

  static void plain() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      P6T14Probe.buildSimple();
      System.out.println("=== " + restriction);
      oneRegime(restriction, false, false, new int[] {2}, new int[] {3});
    }
  }

  static void via() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      P6T14Probe.build();
      System.out.println("=== " + restriction);
      oneRegime(restriction, false, false, new int[] {2}, new int[] {3});
    }
  }

  static void ripup() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      P6T14Probe.buildBlocked();
      System.out.println("=== " + restriction);
      AutorouteEngine unused = oneRegime(restriction, true, true, new int[] {2}, new int[] {3});
    }
  }

  /**
   * `buildBlocked`'s trace taken all the way to the board outline, which is the case
   * `P6T14Probe.buildBlocked`'s doc names: `MazeSearchEngine.getInstance` answers null because
   * `init` cannot seed the destination.
   */
  static void buildSealed() throws Exception {
    P6T14Probe.buildSimple();
    RoutingBoard board = board();
    board.rules.nets.add("N2", 1, false);
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(0, -1000), new IntPoint(0, 1000)}),
        0,
        30,
        new int[] {2},
        1,
        FixedState.UNFIXED); // id 4
  }

  /**
   * `:145-151`. Note the room dump afterwards: this early return sits **before** the cleanup of
   * `:198-205`, so the complete expansion rooms `MazeSearchEngine.init` built — and their leaves
   * in the compensated autoroute search tree — survive the failure even though
   * `maintainDatabase` is false and every other exit would have called `clear()`. That asymmetry
   * is quirk #188.
   */
  static void noMaze() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      buildSealed();
      System.out.println("=== " + restriction);
      AutorouteEngine engine = oneRegime(restriction, true, false, new int[] {2}, new int[] {3});
      dumpRooms(engine, "  after-connection");
      System.out.println("  treeSize=" + treeSize(engine));
    }
  }

  /** The number of leaves in the engine's compensated autoroute tree. */
  static int treeSize(AutorouteEngine engine) throws Exception {
    java.lang.reflect.Field field =
        AutorouteEngine.class.getDeclaredField("autorouteSearchTree");
    field.setAccessible(true);
    Object tree = field.get(engine);
    java.lang.reflect.Method size = tree.getClass().getMethod("size");
    size.setAccessible(true);
    return (Integer) size.invoke(tree);
  }

  static void noPath() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      P6T14Probe.buildBlocked();
      System.out.println("=== " + restriction);
      oneRegime(restriction, true, false, new int[] {2}, new int[] {3});
    }
  }

  /**
   * Ruling 7's fourth recovery boundary. `FoundConnectionLocator.backtrack:316-323` calls
   * `rippedItemList.add(...)` for every ripped item, so an unmodifiable set makes it throw
   * `UnsupportedOperationException` — an `Exception`, which `AutorouteEngine.java:190` catches
   * into a null `autorouteResult` and `:215-219` degrades to a FAILED whose message has no
   * "because" clause. Nothing else in `autorouteConnection` touches `rippedItemList` before that
   * point, so this isolates the boundary exactly.
   */
  static void locatorFail() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      P6T14Probe.buildBlocked();
      RoutingBoard board = board();
      board.rules.setTraceAngleRestriction(restriction);
      AutorouteEngine engine = initAutoroute(board, 1, 1, new Stop(false), false);
      AutorouteControl ctrl = control(board, 1);
      ctrl.viasAllowed = false;
      ctrl.ripupAllowed = true;
      ctrl.ripupCosts = 1000;
      SortedSet<Item> ripped = Collections.unmodifiableSortedSet(new TreeSet<>());
      Map<Item, Integer> ripupCosts = costMap();
      System.out.println("=== " + restriction);
      run(board, engine, ctrl, setOf(2), setOf(3), ripped, ripupCosts);
    }
  }

  static void stop() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      P6T14Probe.buildSimple();
      RoutingBoard board = board();
      board.rules.setTraceAngleRestriction(restriction);
      AutorouteEngine engine = initAutoroute(board, 1, 1, new Stop(true), false);
      AutorouteControl ctrl = control(board, 1);
      SortedSet<Item> ripped = new TreeSet<>();
      Map<Item, Integer> ripupCosts = costMap();
      System.out.println("=== " + restriction);
      run(board, engine, ctrl, setOf(2), setOf(3), ripped, ripupCosts);
    }
  }

  /**
   * `:221-228`, the only arm of `autorouteConnection` that needs a *successful* search on a layer
   * `ctrl.layerActive` says is off. A plain `layerActive[0] = false` cannot produce it — with a
   * **signal** layer `MazeSearchEngine.expandToRoomDoors:396-399` returns before expanding
   * anything and the search answers null instead. The lever is a dedicated **power plane**:
   * `AutorouteControl.java:151-158` forces `layerActive` off for a layer whose `isSignal` is
   * false, and `:397-399`'s guard then does *not* fire, so `expandToTargetDoors` still runs and
   * the located `startLayer` is a disabled one.
   */
  static void inactive() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      P6T14Probe.buildSimple();
      RoutingBoard board = board();
      board.layerStructure.layers[0] = new Layer("front", false);
      board.rules.setTraceAngleRestriction(restriction);
      AutorouteEngine engine = initAutoroute(board, 1, 1, new Stop(false), false);
      AutorouteControl ctrl = control(board, 1);
      SortedSet<Item> ripped = new TreeSet<>();
      Map<Item, Integer> ripupCosts = costMap();
      System.out.println("=== " + restriction);
      System.out.println("  layerActive=[" + ctrl.layerActive[0] + "," + ctrl.layerActive[1] + "]");
      run(board, engine, ctrl, setOf(2), setOf(3), ripped, ripupCosts);
    }
  }

  /** A `Stoppable` that answers false `limit` times and true from then on. */
  static final class StopAfter implements Stoppable {
    private final int limit;
    private int calls;

    StopAfter(int limit) {
      this.limit = limit;
    }

    @Override
    public void requestStop() {}

    @Override
    public boolean isStopRequested() {
      return calls++ >= limit;
    }

    int calls() {
      return calls;
    }
  }

  /**
   * The stop check tripping **after** `MazeSearchEngine.getInstance` has seeded the queue, so the
   * pop loop of `occupyNextElement:323` bails and `findConnection` answers null: `:207-213`'s
   * FAILED. The call count is itself the assertion — plan-6 ruling 6 fixes the six sites that may
   * consult `isStopRequested`, so a port that checks a seventh (or skips one) lands on a different
   * row.
   *
   * <p>Every limit here is chosen so that the connection fails **before** the insert. A limit
   * that lets the search finish (13 or more for the 90-degree regime on this board, where the
   * whole search costs 14 checks) routes on the JVM but not in the port: plan-3 ruling F and
   * plan-6 ruling 6 deliberately put a stop check inside `BasicBoard.splitTraces` and
   * `normalizeTraces`, which Java does not have, so an already-tripped flag aborts the insert
   * there. That is a recorded divergence, not a parity target — see the crate README.
   */
  static void stopAfter() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      for (int limit : new int[] {8, 12, 13}) {
        P6T14Probe.buildSimple();
        RoutingBoard board = board();
        board.rules.setTraceAngleRestriction(restriction);
        StopAfter stoppable = new StopAfter(limit);
        AutorouteEngine engine = initAutoroute(board, 1, 1, stoppable, false);
        AutorouteControl ctrl = control(board, 1);
        SortedSet<Item> ripped = new TreeSet<>();
        Map<Item, Integer> ripupCosts = costMap();
        System.out.println("=== " + restriction + " limit=" + limit);
        run(board, engine, ctrl, setOf(2), setOf(3), ripped, ripupCosts);
        System.out.println("  stopCalls=" + stoppable.calls());
      }
    }
  }

  /**
   * `maintainDatabase = true`, so `:201-205` takes `resetAllDoors` instead of `clear` and the
   * complete-room list carries into the next connection. The connection is `noPath`'s, i.e. one
   * that fails: nothing is inserted, so no `BoardItemRepository`/`PolylineTrace` mutation runs
   * `RoutingBoard.additionalUpdateAfterChange` behind the engine's back and the two sides compare
   * the room database alone.
   *
   * <p>`getRoomsWithTargetItems` (`:620-633`) is a `SortedSet<CompleteFreeSpaceExpansionRoom>`
   * whose comparator is `other.id - this.id`, i.e. **descending** by room id.
   */
  static void maintain() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      P6T14Probe.buildBlocked();
      RoutingBoard board = board();
      board.rules.setTraceAngleRestriction(restriction);
      AutorouteEngine engine = initAutoroute(board, 1, 1, new Stop(false), true);
      AutorouteControl ctrl = control(board, 1);
      ctrl.viasAllowed = false;
      SortedSet<Item> ripped = new TreeSet<>();
      Map<Item, Integer> ripupCosts = costMap();
      System.out.println("=== " + restriction);
      run(board, engine, ctrl, setOf(2), setOf(3), ripped, ripupCosts);
      dumpRooms(engine, "  after-connection");
      // The carry-over: `initConnection` on the **same** net leaves the rooms in place
      // (`:97-98`'s second test), and only then does the descending walk of
      // `getRoomsWithTargetItems` have anything to answer.
      engine.initConnection(1, new Stop(false), null);
      dumpRooms(engine, "  after-same-net");
      engine.initConnection(2, new Stop(false), null);
      dumpRooms(engine, "  after-other-net");
    }
  }

  @SuppressWarnings("unchecked")
  static void dumpRooms(AutorouteEngine engine, String label) throws Exception {
    java.lang.reflect.Field field =
        AutorouteEngine.class.getDeclaredField("completeExpansionRooms");
    field.setAccessible(true);
    Iterable<?> rooms = (Iterable<?>) field.get(engine);
    StringBuilder ids = new StringBuilder();
    int count = 0;
    for (Object room : rooms == null ? java.util.List.of() : rooms) {
      Method getId = room.getClass().getMethod("getId");
      if (count > 0) {
        ids.append(",");
      }
      ids.append(getId.invoke(room));
      count++;
    }
    System.out.println(label + " completeRooms n=" + count + " [" + ids + "]");
    Set<Item> targets = setOf(2, 3);
    Method roomsWithTargets =
        AutorouteEngine.class.getDeclaredMethod("getRoomsWithTargetItems", Set.class);
    roomsWithTargets.setAccessible(true);
    Set<?> withTargets = (Set<?>) roomsWithTargets.invoke(engine, targets);
    StringBuilder targetIds = new StringBuilder();
    boolean first = true;
    for (Object room : withTargets) {
      Method getId = room.getClass().getMethod("getId");
      if (!first) {
        targetIds.append(",");
      }
      first = false;
      targetIds.append(getId.invoke(room));
    }
    System.out.println(
        label + " roomsWithTargetItems n=" + withTargets.size() + " [" + targetIds + "]");
  }

  // =============================================================================================
  // `AutorouteConnectionRouter.route` steps 1-5 (plan-6 ruling 2), transcribed inline
  // =============================================================================================

  /**
   * `AutorouteConnectionRouter.route:36-90` with steps 6-8 (`optChangedArea`, the necked retry and
   * the strict-DRC rollback) left out — plan-6 ruling 2 puts those in Plan 7 — and `:70`'s
   * `router.setAirLine` dropped with the rest of the progress reporting.
   */
  static AutorouteAttemptResult routeSteps1to5(
      RoutingBoard board,
      Item item,
      int routeNetNo,
      RouterSettings settings,
      SortedSet<Item> rippedItemList,
      Map<Item, Integer> ripupCosts,
      int ripupPassNo,
      Stoppable stoppable) {
    try {
      // :37-40.
      Net routeNet = board.rules.nets.get(routeNetNo);
      boolean containsPlane = routeNet != null && routeNet.containsPlane();
      int currentViaCosts =
          containsPlane ? settings.getPlaneViaCosts() : settings.getViaCosts();

      // :42-47.
      AutorouteControl autorouteControl =
          new AutorouteControl(board, routeNetNo, settings, currentViaCosts,
              settings.getTraceCosts());
      autorouteControl.ripupAllowed = true;
      autorouteControl.ripupCosts = settings.getStartRipupCosts() * ripupPassNo;
      autorouteControl.removeUnconnectedVias = !settings.isFanoutEnabled();

      // :49-52.
      Set<Item> unconnectedSet = item.getUnconnectedSet(routeNetNo);
      if (unconnectedSet.isEmpty()) {
        return new AutorouteAttemptResult(AutorouteAttemptState.NO_UNCONNECTED_NETS);
      }

      // :54-68.
      Set<Item> connectedSet = item.getConnectedSet(routeNetNo);
      Set<Item> routeStartSet;
      Set<Item> routeDestSet;
      if (containsPlane) {
        for (Item currentItem : connectedSet) {
          if (currentItem instanceof ConductionArea) {
            return new AutorouteAttemptResult(AutorouteAttemptState.CONNECTED_TO_PLANE);
          }
        }
        routeStartSet = connectedSet;
        routeDestSet = unconnectedSet;
      } else {
        routeStartSet = unconnectedSet;
        routeDestSet = connectedSet;
      }

      // :71-74.
      double maxMilliseconds = 100000 * Math.pow(2, ripupPassNo - 1);
      maxMilliseconds = Math.min(maxMilliseconds, Integer.MAX_VALUE);
      TimeLimit timeLimit = new TimeLimit((int) maxMilliseconds);

      // :76-82.
      AutorouteEngine autorouteEngine =
          board.initAutoroute(
              routeNetNo,
              autorouteControl.traceClearanceClassIndex,
              stoppable,
              timeLimit,
              false);

      // :88-90.
      return autorouteEngine.autorouteConnection(
          routeStartSet, routeDestSet, autorouteControl, rippedItemList, ripupCosts);
    } catch (Exception e) {
      // :155-158 — ruling 7's fifth recovery boundary.
      return new AutorouteAttemptResult(AutorouteAttemptState.FAILED);
    }
  }

  static void route() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      P6T14Probe.buildSimple();
      RoutingBoard board = board();
      board.rules.setTraceAngleRestriction(restriction);
      RouterSettings settings = new RouterSettings(board);
      System.out.println("=== " + restriction);

      Item pin = board.getItem(2);
      SortedSet<Item> ripped = new TreeSet<>();
      Map<Item, Integer> ripupCosts = costMap();
      AutorouteAttemptResult first =
          routeSteps1to5(board, pin, 1, settings, ripped, ripupCosts, 1, new Stop(false));
      System.out.println("  first=" + resultOf(first));
      System.out.println("  " + rippedOf(ripped));
      System.out.println(P6T15Probe.boardDump(board));

      // The same item again: the net is now fully connected, so `:49-52` answers
      // NO_UNCONNECTED_NETS before any engine is built.
      AutorouteAttemptResult second =
          routeSteps1to5(board, pin, 1, settings, ripped, ripupCosts, 1, new Stop(false));
      System.out.println("  second=" + resultOf(second));

      // A positive net the board does not have makes `new AutorouteControl` throw a
      // NullPointerException at `AutorouteControl.java:219` (pinned by `P6T8Probe ctrl`), which
      // `:155-158` degrades to a **bare** FAILED — no details, unlike every FAILED
      // `autorouteConnection` itself produces.
      AutorouteAttemptResult boundary =
          routeSteps1to5(board, pin, 99, settings, ripped, ripupCosts, 1, new Stop(false));
      System.out.println("  boundary5=" + resultOf(boundary));
      System.out.println("  boundary5state=" + stateOf(boundary));
      System.out.println("  boundary5details=[" + boundary.details + "]");
    }
  }

  /**
   * `route` again, on the blocker board and over three `ripupPassNo` values, which is where
   * `:45`'s `ripupCosts = startRipupCosts * ripupPassNo` and `:46`'s `removeUnconnectedVias`
   * reach `MazeRipupResolver` and `:241-245`.
   */
  static void routeRipup() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      System.out.println("=== " + restriction);
      for (int pass : new int[] {1, 2, 4}) {
        P6T14Probe.buildBlocked();
        RoutingBoard board = board();
        board.rules.setTraceAngleRestriction(restriction);
        RouterSettings settings = new RouterSettings(board);
        SortedSet<Item> ripped = new TreeSet<>();
        Map<Item, Integer> ripupCosts = costMap();
        AutorouteAttemptResult result =
            routeSteps1to5(
                board, board.getItem(2), 1, settings, ripped, ripupCosts, pass, new Stop(false));
        System.out.println("  pass=" + pass + " " + resultOf(result));
        System.out.println("  " + rippedOf(ripped));
        System.out.println("  " + ripupCostsOf(ripupCosts));
        System.out.println(P6T15Probe.boardDump(board));
      }
    }
  }

  // =============================================================================================
  // The plane swap (`:54-68`)
  // =============================================================================================

  /**
   * `:57-68`. Two cases per regime, both on the sealed board so that the connection cannot route
   * and the FAILED message — whose two halves `describeConnection` prints start-set first —
   * shows which way round `:62-68` put them:
   *
   * <ul>
   *   <li>`plain` — net 1 is an ordinary net, so `routeStartSet` is the **unconnected** set;
   *   <li>`plane` — net 1 carries `containsPlane`, so the two are swapped;
   *   <li>`conduction` — the same with a `ConductionArea` of net 1 over the start pin, which is
   *       in `getConnectedSet` and makes `:58-60` answer CONNECTED_TO_PLANE before the swap.
   * </ul>
   */
  static void plane() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      System.out.println("=== " + restriction);
      for (String shape : new String[] {"plain", "plane", "conduction"}) {
        buildSealed();
        RoutingBoard board = board();
        board.rules.setTraceAngleRestriction(restriction);
        if (!shape.equals("plain")) {
          board.rules.nets.get(1).setContainsPlane(true);
        }
        if (shape.equals("conduction")) {
          board.insertConductionArea(
              new PolylineArea(
                  new IntBox(-600, -200, -200, 200),
                  new app.freerouting.geometry.planar.PolylineShape[0]),
              0,
              new int[] {1},
              1,
              false,
              FixedState.UNFIXED); // id 5
        }
        RouterSettings settings = new RouterSettings(board);
        SortedSet<Item> ripped = new TreeSet<>();
        Map<Item, Integer> ripupCosts = costMap();
        AutorouteAttemptResult result =
            routeSteps1to5(
                board, board.getItem(2), 1, settings, ripped, ripupCosts, 1, new Stop(false));
        System.out.println("  " + shape + "=" + resultOf(result));
      }
    }
  }

  // =============================================================================================
  // `describeConnection`
  // =============================================================================================

  static void describe() throws Exception {
    P6T14Probe.build();
    System.out.println("  empty=[" + describeConnection(setOf(), setOf()) + "]");
    System.out.println("  one=[" + describeConnection(setOf(2), setOf(3)) + "]");
    System.out.println("  many=[" + describeConnection(setOf(2, 4, 6), setOf(3, 5)) + "]");
    System.out.println("  vias=[" + describeConnection(setOf(7, 9), setOf(8, 10, 11)) + "]");
    for (Item item : board().getItems()) {
      System.out.println("  item id=" + item.getId() + " toString=[" + item + "]");
    }
  }
}
