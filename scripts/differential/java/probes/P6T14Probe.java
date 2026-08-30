// Plan 6 Task 14 ground-truth probe: `autoroute/path/FoundConnectionLocator.java:30-569`,
// `autoroute/path/FoundConnectionLocator45Degree.java:27-356` and
// `autoroute/path/FoundConnectionLocatorAnyAngle.java:24-454` — the backtrack walk from a found
// `MazeSearchEngine.Result` to the list of `ResultItem`s the inserter turns into traces and vias.
//
// It is not a differential driver — there is no Rust twin and `run.sh` does not know it — but
// every literal in `crates/fr-router/tests/locator.rs` is read off its stdout, so it is committed
// here to keep those numbers reproducible.
//
// It declares `package app.freerouting.autoroute.path` so it can call the package-private static
// `FoundConnectionLocator.calculateAdditionalCorner` (`:390-404`), read the `protected`
// `backtrackArray` and `ResultItem.corners`/`.layer`, and name the package-private
// `FoundConnectionLocatorAnyAngle`. The board it searches is `P6T13Probe`'s, reached by
// reflection because that probe's helpers are package-private in `app.freerouting.autoroute.maze`
// — re-declaring the board here would be a second fixture to keep in step.
//
// It compiles together with `P6T11Probe.java` and `P6T13Probe.java` against the clone's HEAD jar
// exactly as `run.sh`'s jar mode does:
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential/java/probes
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p6t14 P6T11Probe.java P6T13Probe.java P6T14Probe.java
//   for m in corner share backtrack locate ripup ripped around reverse warn; do
//     echo "=== mode $m ==="
//     "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//         -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p6t14:$JAR" \
//         app.freerouting.autoroute.path.P6T14Probe "$m" 2>/dev/null \
//       | grep -Ev '^[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9:.]+ +(WARN|INFO|DEBUG|ERROR) '
//   done > crates/fr-router/tests/data/p6t14-locator.txt
//
// The `=== mode X ===` banners of the committed transcript come from this loop, not from the
// probe. The `grep` strips `FRLogger`'s timestamped lines.
//
// Every double is printed with `Double.toString`, which is what
// `fr_dsn::format::double::java_double_to_string` reproduces on the Rust side.
//
// Modes:
//   corner     `calculateAdditionalCorner` (:390-404) over a fixed input table in all three
//              regimes, i.e. `ninetyDegreeCorner` (:329-342) and `fortyfiveDegreeCorner`
//              (:343-384) branch by branch
//   share      `getInstance` (:185-207): the concrete class each of the three regimes builds
//   backtrack  the `backtrackArray` (:225-327) of the `find`-board search, element by element
//   locate     the whole walk on the `find` board, once per regime: `startItem`/`startLayer`,
//              `targetItem`/`targetLayer` and every `ResultItem` corner list
//   ripup      the same on `P6T13Probe`'s big board, whose found connection crosses **two**
//              `ExpansionDrill`s, so the walk splits into three `ResultItem`s on layers 0, 1, 0 —
//              the layer-change arm of the constructor loop (`:142-159`, `:441-448`)
//   ripped     the `find` board plus a net-2 trace across the channel and `viasAllowed` off, so
//              the search rips the trace up: an `ObstacleExpansionRoom` in the backtrack array
//              and `backtrack`'s `rippedItemList`/`ripupCosts` writes (`:316-323`)
//   around     `P6T13Probe`'s big board with `viasAllowed` off, so the connection has to go the
//              long way round the net-2 blocker: eight doors, and the corner lists that exercise
//              the any-angle turn corners (`:365-408`) and the clearance-correction loop
//              (`:284-326`) as well as the 45-degree `calcHorizontalFirst*` diagonals
//   reverse    the same board searched from pin 3 to pin 2 instead, once with `viasAllowed` off
//              and once with it on. It is the only fixture in this file that reaches
//              `leftTurnNextCorner` (`:391-408`); `around` mirrors into `rightTurnNextCorner`
//              only
package app.freerouting.autoroute.path;

import app.freerouting.autoroute.maze.AutorouteControl;
import app.freerouting.autoroute.maze.AutorouteEngine;
import app.freerouting.autoroute.expansion.CompleteFreeSpaceExpansionRoom;
import app.freerouting.autoroute.expansion.ExpandableObject;
import app.freerouting.autoroute.expansion.ExpansionDoor;
import app.freerouting.autoroute.maze.MazeSearchEngine;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.SortedSet;
import java.util.TreeMap;
import java.util.TreeSet;

/** Task 14 ground truth: the three found-connection locators. */
public class P6T14Probe {

  static final String P13 = "app.freerouting.autoroute.maze.P6T13Probe";

  static final AngleRestriction[] REGIMES = {
    AngleRestriction.NINETY_DEGREE, AngleRestriction.FORTYFIVE_DEGREE, AngleRestriction.NONE
  };

  public static void main(String[] args) throws Exception {
    String mode = args.length > 0 ? args[0] : "corner";
    switch (mode) {
      case "corner" -> corner();
      case "share" -> share();
      case "backtrack" -> backtrack();
      case "locate" -> locate();
      case "ripup" -> ripup();
      case "ripped" -> ripped();
      case "around" -> around();
      case "reverse" -> reverse();
      case "warn" -> warn();
      default -> throw new IllegalArgumentException("unknown mode " + mode);
    }
  }

  // =============================================================================================
  // Reflection into P6T13Probe, whose helpers are package-private in `autoroute.maze`
  // =============================================================================================

  static Method m13(String name, Class<?>... sig) throws Exception {
    Method result = Class.forName(P13).getDeclaredMethod(name, sig);
    result.setAccessible(true);
    return result;
  }

  static RoutingBoard board() throws Exception {
    Field f = Class.forName(P13).getDeclaredField("board");
    f.setAccessible(true);
    return (RoutingBoard) f.get(null);
  }

  /** `P6T13Probe.buildSimple()` — the two-pin 2000-unit board `find` searches. */
  static void buildSimple() throws Exception {
    m13("buildSimple").invoke(null);
  }

  /** `P6T13Probe.build()` — the 8000-unit board with the net-2 blocker and the two net-3 vias. */
  static void build() throws Exception {
    m13("build").invoke(null);
  }

  static AutorouteEngine engine(int netNo) throws Exception {
    return (AutorouteEngine) m13("engine", int.class).invoke(null, netNo);
  }

  static AutorouteControl control(int netNo) throws Exception {
    return (AutorouteControl) m13("control", int.class).invoke(null, netNo);
  }

  @SuppressWarnings("unchecked")
  static Set<Item> setOf(int... ids) throws Exception {
    return (Set<Item>) m13("setOf", int[].class).invoke(null, (Object) ids);
  }

  // =============================================================================================
  // Printing
  // =============================================================================================

  static String d(double x) {
    return Double.toString(x);
  }

  static String fp(FloatPoint p) {
    return p == null ? "null" : "(" + d(p.x) + "," + d(p.y) + ")";
  }

  static String ip(IntPoint p) {
    return p == null ? "null" : "(" + p.x + "," + p.y + ")";
  }

  static String regime(AngleRestriction a) {
    return a.toString();
  }

  static String itemId(Item item) {
    return item == null ? "null" : Integer.toString(item.getId());
  }

  // =============================================================================================
  // mode `corner`: calculateAdditionalCorner (:390-404)
  // =============================================================================================

  /**
   * Nine (from, to) pairs times both `horizontalFirst` values times the three regimes. The pairs
   * are chosen so that every branch of `fortyfiveDegreeCorner` (`:343-384`) is taken, including
   * both sides of its `absDx <= absDy` test, its `toPoint.y >= fromPoint.y` (`:353`, the only
   * non-strict one of the four) and its two `toPoint.x > fromPoint.x` tests.
   */
  static void corner() {
    double[][] pairs = {
      {0.0, 0.0, 100.0, 300.0}, // absDx < absDy, to above
      {0.0, 0.0, 100.0, -300.0}, // absDx < absDy, to below
      {0.0, 0.0, -100.0, 300.0},
      {0.0, 0.0, -100.0, -300.0},
      {0.0, 0.0, 300.0, 100.0}, // absDx > absDy, to the right
      {0.0, 0.0, -300.0, 100.0}, // absDx > absDy, to the left
      {0.0, 0.0, 300.0, -100.0},
      {0.0, 0.0, 200.0, 200.0}, // absDx == absDy: the `<=` arm
      {0.0, 0.0, 100.0, 0.0}, // absDy == 0, so the `>` arm with equal y
      {17.5, -3.25, -9.75, -3.25}, // absDy == 0 and to.x < from.x
      {17.5, -3.25, 17.5, 42.5}, // absDx == 0
      {-1.5, 2.5, -1.5, 2.5}, // from == to
    };
    for (double[] p : pairs) {
      FloatPoint from = new FloatPoint(p[0], p[1]);
      FloatPoint to = new FloatPoint(p[2], p[3]);
      for (boolean horizontalFirst : new boolean[] {true, false}) {
        StringBuilder line =
            new StringBuilder(
                "from=" + fp(from) + " to=" + fp(to) + " hf=" + horizontalFirst);
        for (AngleRestriction restriction : REGIMES) {
          FloatPoint result =
              FoundConnectionLocator.calculateAdditionalCorner(
                  from, to, horizontalFirst, restriction);
          line.append(" ").append(regime(restriction)).append("=").append(fp(result));
          if (restriction == AngleRestriction.NONE) {
            // `:401` returns `toPoint` itself, not a copy — the identity the corner-dedup loop of
            // `calculateNextTrace` (`:432`) tests with `!=`.
            line.append(result == to ? " same=true" : " same=false");
          }
        }
        System.out.println(line);
      }
    }
  }

  // =============================================================================================
  // mode `share`: getInstance (:185-207)
  // =============================================================================================

  static void share() throws Exception {
    buildSimple();
    for (AngleRestriction restriction : REGIMES) {
      Located located = locateSimple(restriction);
      System.out.println(
          "regime="
              + regime(restriction)
              + " class="
              + located.locator.getClass().getSimpleName());
    }
    // `getInstance` answers null for a null result, and only for that (`:192-194`).
    System.out.println(
        "nullResult="
            + FoundConnectionLocator.getInstance(
                null,
                control(1),
                engine(1).autorouteSearchTree,
                AngleRestriction.NONE,
                new TreeSet<>(),
                null));
  }

  // =============================================================================================
  // The search, rerun per regime
  // =============================================================================================

  /** One located connection plus the ripup bookkeeping `backtrack` filled in. */
  record Located(
      FoundConnectionLocator locator, SortedSet<Item> ripped, Map<Item, Integer> ripupCosts) {}

  /**
   * Rebuilds `P6T13Probe.buildSimple`'s board, reruns `findConnection` and locates the result
   * under `restriction`.
   *
   * <p>The board is rebuilt for every regime rather than the single search being located three
   * times, because `FoundConnectionLocator45Degree.calculateNextTraceCorners` (`:244`) calls
   * `ExpansionDoor.getSectionSegments`, which reallocates `sectionArr` (`ExpansionDoor.java:141`,
   * `:193-201`) whenever the section count differs from the one the search left — and that would
   * drop the `backtrackDoor`s a later locator run needs.
   */
  static Located locateSimple(AngleRestriction restriction) throws Exception {
    buildSimple();
    return locate(restriction, 1, setOf(2), setOf(3));
  }

  static Located locate(AngleRestriction restriction, int netNo, Set<Item> start, Set<Item> dest)
      throws Exception {
    return locate(restriction, netNo, start, dest, false);
  }

  static Located locate(
      AngleRestriction restriction,
      int netNo,
      Set<Item> start,
      Set<Item> dest,
      boolean ripupNoVias)
      throws Exception {
    AutorouteEngine autorouteEngine = engine(netNo);
    AutorouteControl control = control(netNo);
    if (ripupNoVias) {
      // Forbid the layer change the `ripup` mode's search takes, so the only way through the
      // net-2 blocker is to rip it up: that is what puts `roomRipped` on a backtrack element and
      // makes `backtrack` (`:316-323`) fill `rippedItemList` and `ripupCosts`.
      control.viasAllowed = false;
      control.ripupAllowed = true;
      control.ripupCosts = 1000;
    }
    MazeSearchEngine maze = MazeSearchEngine.getInstance(start, dest, autorouteEngine, control);
    if (maze == null) {
      throw new IllegalStateException("getInstance returned null");
    }
    MazeSearchEngine.Result result = maze.findConnection();
    if (result == null) {
      throw new IllegalStateException("findConnection returned null");
    }
    SortedSet<Item> ripped = new TreeSet<>();
    Map<Item, Integer> ripupCosts = new TreeMap<>();
    FoundConnectionLocator locator =
        FoundConnectionLocator.getInstance(
            result,
            control,
            autorouteEngine.autorouteSearchTree,
            restriction,
            ripped,
            ripupCosts);
    return new Located(locator, ripped, ripupCosts);
  }

  // =============================================================================================
  // mode `backtrack`: the backtrack array (:225-327)
  // =============================================================================================

  static void backtrack() throws Exception {
    Located located = locateSimple(AngleRestriction.NONE);
    dumpBacktrack(located);
  }

  static void dumpBacktrack(Located located) {
    FoundConnectionLocator.BacktrackElement[] arr = located.locator.backtrackArray;
    System.out.println("backtrack n=" + arr.length);
    for (int i = 0; i < arr.length; i++) {
      FoundConnectionLocator.BacktrackElement e = arr[i];
      System.out.println(
          "  ["
              + i
              + "] door="
              + e.door.getClass().getSimpleName()
              + " id="
              + e.door.getId()
              + " section="
              + e.sectionNoOfDoor
              + " nextRoom="
              + (e.nextRoom == null ? "null" : e.nextRoom.getClass().getSimpleName())
              + " nextRoomId="
              + (e.nextRoom == null ? "-" : Integer.toString(e.nextRoom.getId()))
              + " nextRoomLayer="
              + (e.nextRoom == null ? "-" : Integer.toString(e.nextRoom.getLayer())));
    }
  }

  // =============================================================================================
  // modes `locate` and `ripup`: the whole walk
  // =============================================================================================

  static void locate() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      Located located = locateSimple(restriction);
      System.out.println("=== " + regime(restriction));
      dump(located);
    }
  }

  static void ripup() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      build();
      Located located = locate(restriction, 1, setOf(2), setOf(3));
      System.out.println("=== " + regime(restriction));
      if (restriction == AngleRestriction.NINETY_DEGREE) {
        dumpBacktrack(located);
      }
      dump(located);
    }
  }

  /**
   * `buildSimple`'s board plus a net-2 trace straight across the channel between the two net-1
   * pins. With `viasAllowed` off the cheapest way through is to rip it up, so `backtrack`
   * (`:316-323`) fills `rippedItemList` and `ripupCosts`.
   *
   * <p>The trace stops 100 units short of the outline at each end on purpose: taken all the way
   * to the board edge it makes `MazeSearchEngine.getInstance` answer null (`init` fails), and
   * there is then no `Result` to locate at all.
   */
  static void buildBlocked() throws Exception {
    buildSimple();
    RoutingBoard board = board();
    board.rules.nets.add("N2", 1, false);
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(0, -900), new IntPoint(0, 900)}),
        0,
        30,
        new int[] {2},
        1,
        FixedState.UNFIXED); // id 4
  }

  static void ripped() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      buildBlocked();
      Located located = locate(restriction, 1, setOf(2), setOf(3), true);
      System.out.println("=== " + regime(restriction));
      if (restriction == AngleRestriction.NINETY_DEGREE) {
        dumpBacktrack(located);
      }
      dump(located);
    }
  }

  static void around() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      build();
      Located located = locate(restriction, 1, setOf(2), setOf(3), true);
      System.out.println("=== " + regime(restriction));
      if (restriction == AngleRestriction.NINETY_DEGREE) {
        dumpBacktrack(located);
      }
      dump(located);
    }
  }

  static void reverse() throws Exception {
    for (boolean noVias : new boolean[] {true, false}) {
      for (AngleRestriction restriction : REGIMES) {
        build();
        Located located = locate(restriction, 1, setOf(3), setOf(2), noVias);
        System.out.println("=== noVias=" + noVias + " " + regime(restriction));
        dump(located);
      }
    }
  }

  // =============================================================================================
  // mode `warn`: the constructor's two early returns (:103-111, :130-135)
  // =============================================================================================

  /** `new MazeSearchEngine.Result(door, section)` — the constructor is package-private. */
  static MazeSearchEngine.Result forgeResult(ExpandableObject door, int section) throws Exception {
    Constructor<MazeSearchEngine.Result> ctor =
        MazeSearchEngine.Result.class.getDeclaredConstructor(ExpandableObject.class, int.class);
    ctor.setAccessible(true);
    return ctor.newInstance(door, section);
  }

  /** `MazeSearchElement.backtrackDoor`, which is package-private in `autoroute.maze`. */
  static Object backtrackDoorOf(ExpandableObject door, int section) throws Exception {
    Object element = door.getMazeSearchElement(section);
    Field f = element.getClass().getDeclaredField("backtrackDoor");
    f.setAccessible(true);
    return f.get(element);
  }

  @SuppressWarnings("unchecked")
  static List<CompleteFreeSpaceExpansionRoom> completeRooms(AutorouteEngine autorouteEngine)
      throws Exception {
    Field f = AutorouteEngine.class.getDeclaredField("completeExpansionRooms");
    f.setAccessible(true);
    List<CompleteFreeSpaceExpansionRoom> rooms =
        (List<CompleteFreeSpaceExpansionRoom>) f.get(autorouteEngine);
    return rooms == null ? List.of() : rooms;
  }

  static void warn() throws Exception {
    buildSimple();
    AutorouteEngine autorouteEngine = engine(1);
    AutorouteControl control = control(1);
    MazeSearchEngine maze = MazeSearchEngine.getInstance(setOf(2), setOf(3), autorouteEngine, control);
    MazeSearchEngine.Result result = maze.findConnection();

    // The honest walk first, so the forged ones run over the same live rooms and doors.
    FoundConnectionLocator real =
        FoundConnectionLocator.getInstance(
            result,
            control,
            autorouteEngine.autorouteSearchTree,
            AngleRestriction.NONE,
            new TreeSet<>(),
            null);
    FoundConnectionLocator.BacktrackElement middle = real.backtrackArray[1];
    System.out.println(
        "middle door=" + middle.door.getClass().getSimpleName() + " section=" + middle.sectionNoOfDoor);

    // :130-135 — the destination door is an ExpansionDoor, so `targetItem`/`targetLayer` stay at
    // their defaults while `startItem`/`startLayer` are already set.
    System.out.println("=== unexpectedDestinationDoor");
    dumpWarn(
        FoundConnectionLocator.getInstance(
            forgeResult(middle.door, middle.sectionNoOfDoor),
            control,
            autorouteEngine.autorouteSearchTree,
            AngleRestriction.NONE,
            new TreeSet<>(),
            null));

    // :103-111 — a door section the search never reached, so `backtrack` yields one element and
    // `startInfo.door` is an ExpansionDoor. Every field stays at its default.
    ExpandableObject orphan = null;
    int orphanSection = -1;
    outer:
    for (CompleteFreeSpaceExpansionRoom room : completeRooms(autorouteEngine)) {
      for (ExpansionDoor door : room.getDoors()) {
        for (int i = 0; i < door.mazeSearchElementCount(); i++) {
          if (backtrackDoorOf(door, i) == null) {
            orphan = door;
            orphanSection = i;
            break outer;
          }
        }
      }
    }
    System.out.println(
        "=== orphanDoor type="
            + (orphan == null ? "null" : orphan.getClass().getSimpleName())
            + " section="
            + orphanSection);
    if (orphan != null) {
      dumpWarn(
          FoundConnectionLocator.getInstance(
              forgeResult(orphan, orphanSection),
              control,
              autorouteEngine.autorouteSearchTree,
              AngleRestriction.NONE,
              new TreeSet<>(),
              null));
    }
  }

  static void dumpWarn(FoundConnectionLocator locator) {
    System.out.println(
        "startItem="
            + itemId(locator.startItem)
            + " startLayer="
            + locator.startLayer
            + " targetItem="
            + itemId(locator.targetItem)
            + " targetLayer="
            + locator.targetLayer
            + " backtrack n="
            + locator.backtrackArray.length
            + " connectionItems="
            + (locator.connectionItems == null
                ? "null"
                : "n=" + locator.connectionItems.size()));
  }

  static void dump(Located located) {
    FoundConnectionLocator locator = located.locator;
    System.out.println(
        "startItem="
            + itemId(locator.startItem)
            + " startLayer="
            + locator.startLayer
            + " targetItem="
            + itemId(locator.targetItem)
            + " targetLayer="
            + locator.targetLayer);
    System.out.println(
        "connectionItems n="
            + (locator.connectionItems == null ? "null" : locator.connectionItems.size()));
    if (locator.connectionItems != null) {
      int index = 0;
      for (FoundConnectionLocator.ResultItem item : locator.connectionItems) {
        StringBuilder line =
            new StringBuilder("  [" + index + "] layer=" + item.layer + " corners=" + item.corners.length);
        for (IntPoint corner : item.corners) {
          line.append(" ").append(ip(corner));
        }
        System.out.println(line);
        index++;
      }
    }
    System.out.print("ripped n=" + located.ripped.size());
    for (Item item : located.ripped) {
      System.out.print(" " + item.getId() + ":" + located.ripupCosts.get(item));
    }
    System.out.println();
  }
}
