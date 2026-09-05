// Plan 6 Task 15 ground-truth probe: `autoroute/path/FoundConnectionInserter.java:23-807` — the
// only class in Plan 6 that mutates the board's item set. It turns a located
// `FoundConnectionLocator`'s `connectionItems` into board traces and vias.
//
// It is not a differential driver — there is no Rust twin and `run.sh` does not know it — but
// every literal in `crates/fr-router/tests/inserter.rs` is read off its stdout, so it is committed
// here to keep those numbers reproducible.
//
// It declares `package app.freerouting.autoroute.path` so it can reach the private constructor
// `FoundConnectionInserter(RoutingBoard, AutorouteControl)`, the package-private
// `insertNeckdown` (`:525-537`) and — by reflection, since they are `private` — `tryNeckDown`
// (`:539-676`) and `insertFanoutMicroNeckdown` (`:455-523`). Its boards are `P6T13Probe`'s and
// `P6T14Probe`'s, reached through `P6T14Probe`'s package-private helpers, plus one fixture of its
// own (`buildNeck`) whose trace half width is wide enough that a pin's neckdown half width is
// strictly below it — the precondition `tryNeckDown:553` tests.
//
// It compiles together with `P6T11Probe.java`, `P6T13Probe.java` and `P6T14Probe.java` against
// the clone's HEAD jar exactly as `run.sh`'s jar mode does:
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential/java/probes
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p6t15 \
//       P6T11Probe.java P6T13Probe.java P6T14Probe.java P6T15Probe.java
//   for m in simple via around totrace diag viafail neck micro; do
//     echo "=== mode $m ==="
//     "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//         -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p6t15:$JAR" \
//         app.freerouting.autoroute.path.P6T15Probe "$m" 2>/dev/null \
//       | grep -Ev '^[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9:.]+ +(WARN|INFO|DEBUG|ERROR|TRACE) '
//   done > crates/fr-router/tests/data/p6t15-inserter.txt
//
// The `=== mode X ===` banners of the committed transcript come from this loop, not from the
// probe. The `grep` strips `FRLogger`'s timestamped lines — `FoundConnectionInserter` is by far
// the chattiest class in `autoroute/path` (`FRLogger.trace` at `:54`, `:190`, `:221`, `:240`,
// `:281`, `:298`, `:359`, `:378`, `:411`, `:433` and the `traceFanoutDiagnostic` sites).
//
// Every double is printed with `Double.toString`, which is what
// `fr_dsn::format::double::java_double_to_string` reproduces on the Rust side.
//
// Modes:
//   simple   `buildSimple`'s two-pin board, all three regimes: one `ResultItem`, no layer change,
//            so `getInstance` inserts exactly one trace and no via (`:66`'s `insertVia` is the
//            `inputFromLayer == inputToLayer` early return of `:684-686`)
//   via      `P6T13Probe.build`'s board, whose found connection crosses two `ExpansionDrill`s:
//            three `ResultItem`s on layers 0, 1, 0, so the layer changes at `:66` insert **two**
//            vias and `:74`'s final `insertVia` is the no-op. All three regimes
//   around    the same board with `viasAllowed` off, so the connection walks the long way round
//            the net-2 blocker: 26 / 25 / 7 corners through the `:171-405` segment loop, which is
//            the only fixture here that takes the `:264-319` VIOLATION_CORRECTED arm
//   totrace  `buildSimple` plus a net-1 trace as the destination item, so
//            `connection.targetItem instanceof PolylineTrace` (`:77`) holds and `:79`'s
//            `connectToTrace` runs — here on a corner that is already *on* the target trace, so
//            `RoutingBoard.connectToTrace:1123-1126` returns at once
//   diag     the same with a **slanted** target trace, so the corner is not on it. This is the
//            quirk #186 fixture: `connection.targetItem` is a live Java reference to a trace the
//            insert has already split in two, and `connectToTrace` works off the dead object's
//            undivided polyline — inserting a stub against it and removing the trace tails at
//            *its* two end corners, i.e. both halves of the split
//   viafail  two ways to make `insertVia` answer false on the `via` board, i.e. the two arms of
//            `:721-752`: an empty `ctrl.viaRule` (`foundSuitableSpan == false`, `:722-729`) and a
//            user-fixed foreign-net via sitting on the drill location, which makes
//            `ForcedViaInserter.check` (`:708`) refuse every candidate (`:730-734`). Both make
//            `getInstance` answer **null** after the board has already been mutated
//   neck     `buildNeck`'s wide-trace board: `tryNeckDown` (`:539-676`) and `insertNeckdown`
//            (`:525-537`) over a fixed table, called directly
//   micro    `insertFanoutMicroNeckdown` (`:455-523`) over a fixed table, called directly. The
//            inserted trace's half width identifies which element of the `LinkedHashSet` at
//            `:462-471` won, which is what pins the set's **insertion** order: on the far corner
//            pair every candidate fits, so with a null `startPin` the candidate list is
//            [69, 75, 60, 50] and the winner is 69, where any sorted set would answer 50
package app.freerouting.autoroute.path;

import app.freerouting.autoroute.maze.AutorouteControl;
import app.freerouting.autoroute.maze.AutorouteEngine;
import app.freerouting.autoroute.maze.MazeSearchEngine;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.items.Via;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.trace.PolylineTrace;
import app.freerouting.core.library.Padstack;
import app.freerouting.geometry.planar.ConvexShape;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.IntOctagon;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.Line;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import app.freerouting.rules.ViaInfo;
import app.freerouting.rules.ViaRule;
import app.freerouting.settings.RouterSettings;
import java.lang.reflect.Constructor;
import java.lang.reflect.Method;
import java.util.Map;
import java.util.Set;
import java.util.SortedSet;
import java.util.TreeMap;
import java.util.TreeSet;

/** Task 15 ground truth: `FoundConnectionInserter`. */
public class P6T15Probe {

  static final AngleRestriction[] REGIMES = {
    AngleRestriction.NINETY_DEGREE, AngleRestriction.FORTYFIVE_DEGREE, AngleRestriction.NONE
  };

  public static void main(String[] args) throws Exception {
    String mode = args.length > 0 ? args[0] : "simple";
    switch (mode) {
      case "simple" -> simple();
      case "via" -> via();
      case "around" -> around();
      case "totrace" -> toTrace();
      case "diag" -> diag();
      case "viafail" -> viaFail();
      case "neck" -> neck();
      case "micro" -> micro();
      default -> throw new IllegalArgumentException("unknown mode " + mode);
    }
  }

  // =============================================================================================
  // Dumps — the same shape as `P6T15bProbe`'s, plus a `Via` arm
  // =============================================================================================

  static String ln(Line l) {
    return "("
        + ((IntPoint) l.a).x
        + ","
        + ((IntPoint) l.a).y
        + ")->("
        + ((IntPoint) l.b).x
        + ","
        + ((IntPoint) l.b).y
        + ")";
  }

  static String pt(Polyline p, int no) {
    Point corner = p.corner(no);
    if (corner instanceof IntPoint ip) {
      return "(" + ip.x + "," + ip.y + ")";
    }
    FloatPoint f = p.cornerApprox(no);
    return "~(" + Double.toString(f.x) + "," + Double.toString(f.y) + ")";
  }

  static String poly(Polyline p) {
    if (p == null) {
      return "null";
    }
    StringBuilder sb = new StringBuilder();
    sb.append("n=").append(p.lines.length).append(" lines=[");
    for (int i = 0; i < p.lines.length; i++) {
      if (i > 0) {
        sb.append(",");
      }
      sb.append(ln(p.lines[i]));
    }
    sb.append("] corners=[");
    for (int i = 0; i < p.cornerCount(); i++) {
      if (i > 0) {
        sb.append(",");
      }
      sb.append(pt(p, i));
    }
    sb.append("]");
    return sb.toString();
  }

  /** A `Point` answer, exactly: `null`, an `IntPoint`, or a rational one through `toFloat`. */
  static String ptOf(Point p) {
    if (p == null) {
      return "null";
    }
    if (p instanceof IntPoint ip) {
      return "(" + ip.x + "," + ip.y + ")";
    }
    FloatPoint f = p.toFloat();
    return "~(" + Double.toString(f.x) + "," + Double.toString(f.y) + ")";
  }

  static String pointOf(Point p) {
    FloatPoint f = p.toFloat().round().toFloat();
    return "(" + (int) f.x + "," + (int) f.y + ")";
  }

  static String nets(int[] netNos) {
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < netNos.length; i++) {
      if (i > 0) {
        sb.append(",");
      }
      sb.append(netNos[i]);
    }
    return sb.append("]").toString();
  }

  /** One line per item, in `getItems()` order (descending id, quirk #63). */
  static String boardDump(RoutingBoard board) {
    StringBuilder out = new StringBuilder();
    out.append("    maxId=").append(board.communication.idGenerator.maxGeneratedId());
    for (Item item : board.getItems()) {
      StringBuilder sb = new StringBuilder();
      sb.append("\n    item id=")
          .append(item.getId())
          .append(" type=")
          .append(item.getClass().getSimpleName())
          .append(" nets=")
          .append(nets(item.netNumbers))
          .append(" cl=")
          .append(item.clearanceClassIndex());
      if (item instanceof PolylineTrace trace) {
        sb.append(" layer=")
            .append(trace.getLayer())
            .append(" hw=")
            .append(trace.getHalfWidth())
            .append(" ")
            .append(poly(trace.polyline()));
      } else if (item instanceof Via viaItem) {
        Padstack padstack = viaItem.getPadstack();
        sb.append(" center=")
            .append(pointOf(viaItem.getCenter()))
            .append(" padstack=")
            .append(padstack.name)
            .append(" layers=")
            .append(viaItem.firstLayer())
            .append("..")
            .append(viaItem.lastLayer());
      } else if (item instanceof Pin pin) {
        sb.append(" center=").append(pointOf(pin.getCenter()));
      }
      out.append(sb);
    }
    return out.toString();
  }

  static void dump(RoutingBoard board) {
    System.out.println(boardDump(board));
  }

  static String regime(AngleRestriction a) {
    return a.toString();
  }

  // =============================================================================================
  // Search, locate, insert
  // =============================================================================================

  /** One located connection plus the control the search and the insert share. */
  record Located(
      FoundConnectionLocator locator,
      AutorouteControl ctrl,
      SortedSet<Item> ripped,
      Map<Item, Integer> ripupCosts) {}

  static RouterSettings settings(RoutingBoard board, boolean neckdown) {
    RouterSettings result = new RouterSettings(board);
    result.setAutomaticNeckdown(neckdown);
    return result;
  }

  static AutorouteControl control(RoutingBoard board, int netNo, boolean neckdown) {
    RouterSettings s = settings(board, neckdown);
    return new AutorouteControl(board, netNo, s, s.getViaCosts(), s.getTraceCosts());
  }

  static AutorouteEngine engine(RoutingBoard board, int netNo) throws Exception {
    return P6T14Probe.engine(netNo);
  }

  /**
   * `AutorouteEngine.autorouteConnection:181-266` without the ripup bookkeeping: run the maze
   * search, locate its result under `restriction` and hand back the locator with the very
   * `AutorouteControl` the search used — `FoundConnectionInserter.getInstance` reads
   * `ctrl.traceHalfWidth`, `ctrl.viaRule` and `ctrl.netNumber` off the same object.
   */
  static Located locate(
      RoutingBoard board,
      AngleRestriction restriction,
      int netNo,
      Set<Item> start,
      Set<Item> dest,
      boolean noVias,
      boolean neckdown)
      throws Exception {
    AutorouteEngine autorouteEngine = engine(board, netNo);
    AutorouteControl ctrl = control(board, netNo, neckdown);
    if (noVias) {
      ctrl.viasAllowed = false;
      ctrl.ripupAllowed = true;
      ctrl.ripupCosts = 1000;
    }
    MazeSearchEngine maze = MazeSearchEngine.getInstance(start, dest, autorouteEngine, ctrl);
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
            ctrl,
            autorouteEngine.autorouteSearchTree,
            restriction,
            ripped,
            ripupCosts);
    return new Located(locator, ctrl, ripped, ripupCosts);
  }

  /** The `connectionItems` list, as `P6T14Probe.dump` prints it. */
  static void dumpItems(FoundConnectionLocator locator) {
    System.out.println(
        "  startItem="
            + (locator.startItem == null ? "null" : locator.startItem.getId())
            + " startLayer="
            + locator.startLayer
            + " targetItem="
            + (locator.targetItem == null ? "null" : locator.targetItem.getId())
            + " targetLayer="
            + locator.targetLayer);
    System.out.println("  connectionItems n=" + locator.connectionItems.size());
    int i = 0;
    for (FoundConnectionLocatorAnyAngle.ResultItem item : locator.connectionItems) {
      StringBuilder sb = new StringBuilder("    [" + i + "] layer=" + item.layer + " corners=");
      sb.append(item.corners.length);
      for (IntPoint corner : item.corners) {
        sb.append(" (").append(corner.x).append(",").append(corner.y).append(")");
      }
      System.out.println(sb);
      i++;
    }
  }

  /** Locate, print the located connection, then insert and print what the insert left behind. */
  static void insertAndDump(RoutingBoard board, Located located) {
    dumpItems(located.locator);
    System.out.println("  ripped n=" + located.ripped.size());
    int before = board.communication.idGenerator.maxGeneratedId();
    FoundConnectionInserter inserter =
        FoundConnectionInserter.getInstance(located.locator, board, located.ctrl);
    System.out.println(
        "  insert=" + (inserter == null ? "null" : "ok") + " maxIdBefore=" + before);
    dump(board);
  }

  // =============================================================================================
  // mode `simple`
  // =============================================================================================

  static void simple() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      P6T14Probe.buildSimple();
      RoutingBoard board = P6T14Probe.board();
      System.out.println("=== " + regime(restriction));
      Located located =
          locate(
              board, restriction, 1, P6T14Probe.setOf(2), P6T14Probe.setOf(3), false, false);
      insertAndDump(board, located);
    }
  }

  // =============================================================================================
  // mode `via`
  // =============================================================================================

  static void via() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      P6T14Probe.build();
      RoutingBoard board = P6T14Probe.board();
      System.out.println("=== " + regime(restriction));
      Located located =
          locate(
              board, restriction, 1, P6T14Probe.setOf(2), P6T14Probe.setOf(3), false, false);
      insertAndDump(board, located);
    }
  }

  // =============================================================================================
  // mode `around`
  // =============================================================================================

  static void around() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      P6T14Probe.build();
      RoutingBoard board = P6T14Probe.board();
      System.out.println("=== " + regime(restriction));
      Located located =
          locate(board, restriction, 1, P6T14Probe.setOf(2), P6T14Probe.setOf(3), true, false);
      insertAndDump(board, located);
    }
  }

  // =============================================================================================
  // mode `totrace`
  // =============================================================================================

  /** `buildSimple` plus a net-1 trace, so the connection's target item is a `PolylineTrace`. */
  static void buildToTrace() throws Exception {
    P6T14Probe.buildSimple();
    RoutingBoard board = P6T14Probe.board();
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(-400, 600), new IntPoint(400, 600)}),
        0,
        30,
        new int[] {1},
        1,
        FixedState.UNFIXED); // id 4
  }

  /**
   * `buildToTrace` with a **slanted** target trace, so that the located connection's first corner
   * rounds to a point that is *not* on the target polyline. That is the only shape in this file
   * where `:79`'s `connectToTrace` does more than `RoutingBoard.connectToTrace:1123-1126`'s
   * "the point is already on the trace" early return.
   */
  static void buildDiagTrace() throws Exception {
    P6T14Probe.buildSimple();
    RoutingBoard board = P6T14Probe.board();
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(-400, 600), new IntPoint(400, 653)}),
        0,
        30,
        new int[] {1},
        1,
        FixedState.UNFIXED); // id 4
  }

  static void diag() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      buildDiagTrace();
      RoutingBoard board = P6T14Probe.board();
      System.out.println("=== " + regime(restriction));
      Located located =
          locate(
              board, restriction, 1, P6T14Probe.setOf(2, 3), P6T14Probe.setOf(4), false, false);
      insertAndDump(board, located);
    }
  }

  static void toTrace() throws Exception {
    for (AngleRestriction restriction : REGIMES) {
      buildToTrace();
      RoutingBoard board = P6T14Probe.board();
      System.out.println("=== " + regime(restriction));
      Located located =
          locate(
              board, restriction, 1, P6T14Probe.setOf(2, 3), P6T14Probe.setOf(4), false, false);
      insertAndDump(board, located);
    }
  }

  // =============================================================================================
  // mode `viafail`
  // =============================================================================================

  static void viaFail() throws Exception {
    // (a) `:701`'s loop finds no padstack spanning both layers, so `foundSuitableSpan` stays
    // false and `:722-729`'s arm answers false — `getInstance` returns null at `:67`.
    P6T14Probe.build();
    RoutingBoard board = P6T14Probe.board();
    System.out.println("=== emptyRule");
    Located located =
        locate(
            board,
            AngleRestriction.NINETY_DEGREE,
            1,
            P6T14Probe.setOf(2),
            P6T14Probe.setOf(3),
            false,
            false);
    located.ctrl.viaRule = new ViaRule("empty");
    insertAndDump(board, located);

    // (b) a user-fixed foreign-net via sitting on the drill location, so every candidate's
    // `ForcedViaInserter.check` (`:708`) refuses and `:730-734`'s arm answers false.
    P6T14Probe.build();
    board = P6T14Probe.board();
    System.out.println("=== blockedTrace");
    located =
        locate(
            board,
            AngleRestriction.NINETY_DEGREE,
            1,
            P6T14Probe.setOf(2),
            P6T14Probe.setOf(3),
            false,
            false);
    // The **second** layer change, so that `:66`'s first via and the first two traces all
    // succeed and the refusal lands on `ForcedViaInserter.check` rather than on the trace whose
    // end corner the blocker would sit on.
    IntPoint drill = viaLocation(located.locator, 1);
    System.out.println("  drill=" + ptOf(drill));
    board.insertVia(viaPad(), drill, new int[] {2}, 1, FixedState.USER_FIXED, false);
    insertAndDump(board, located);

    // (c) a via rule whose only padstack does span both layers — so `foundSuitableSpan` is true —
    // but whose pad is far too large to place anywhere on this board, so `ForcedViaInserter.check`
    // (`:708`) refuses every candidate and `:730-734`'s arm answers false. This is the brief's
    // "a refused forced via check is not an error": the caller sees `null`, not a throw.
    P6T14Probe.build();
    board = P6T14Probe.board();
    System.out.println("=== refusedCheck");
    located =
        locate(
            board,
            AngleRestriction.NINETY_DEGREE,
            1,
            P6T14Probe.setOf(2),
            P6T14Probe.setOf(3),
            false,
            false);
    IntOctagon hugeShape = new IntOctagon(-3000, -3000, 3000, 3000, -6000, 6000, -6000, 6000);
    ConvexShape[] huge = {hugeShape, hugeShape};
    Padstack hugePad = board.library.padstacks.add("huge", huge, true, false);
    ViaInfo hugeInfo = new ViaInfo("huge", hugePad, 1, false, board.rules);
    board.rules.viaInfos.add(hugeInfo);
    ViaRule hugeRule = new ViaRule("huge");
    hugeRule.appendVia(hugeInfo);
    located.ctrl.viaRule = hugeRule;
    insertAndDump(board, located);
  }

  /** `P6T13Probe.viaPad`, the two-layer octagon padstack, through the field it is stored in. */
  static Padstack viaPad() throws Exception {
    java.lang.reflect.Field f =
        Class.forName("app.freerouting.autoroute.maze.P6T13Probe").getDeclaredField("viaPad");
    f.setAccessible(true);
    return (Padstack) f.get(null);
  }

  /** Layer change number `which` of the located connection — a location `:66`'s via goes to. */
  static IntPoint viaLocation(FoundConnectionLocator locator, int which) {
    int currentLayer = locator.targetLayer;
    int seen = 0;
    for (FoundConnectionLocatorAnyAngle.ResultItem item : locator.connectionItems) {
      if (item.layer != currentLayer) {
        if (seen == which) {
          return item.corners[0];
        }
        seen++;
      }
      currentLayer = item.layer;
    }
    throw new IllegalStateException("no layer change #" + which + " in the located connection");
  }

  // =============================================================================================
  // modes `neck` and `micro`: the fixture
  // =============================================================================================

  /**
   * `P6T13Probe.buildSimple` with a wider default trace half width and a user-fixed foreign-net
   * blocker.
   *
   * <p>The width matters: `tryNeckDown:553` gives up unless the pin's neckdown half width —
   * `Pin.getTraceNeckdownHalfwidth`, `max(0.5 * minWidth - 1, 1)`, so 49 for the 100-unit `smd`
   * pad and 69 for the 140-unit `thru` octagon — is **strictly below** `ctrl.traceHalfWidth`, and
   * `buildSimple`'s 30 is below both. The blocker matters too: with nothing in the way
   * `board.checkTraceSegment` (`:566`) answers `Integer.MAX_VALUE` and `:574-576` returns
   * `fromCorner` before inserting anything.
   */
  static void buildNeck(int traceHalfWidth) throws Exception {
    P6T14Probe.buildSimple();
    RoutingBoard board = P6T14Probe.board();
    board.rules.setDefaultTraceHalfWidths(traceHalfWidth);
    board.rules.nets.add("N2", 1, false);
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(0, -900), new IntPoint(0, 900)}),
        0,
        30,
        new int[] {2},
        1,
        FixedState.USER_FIXED); // id 4
  }

  /** The `FoundConnectionInserter` its private constructor (`:31-34`) builds. */
  static FoundConnectionInserter inserter(RoutingBoard board, AutorouteControl ctrl)
      throws Exception {
    Constructor<FoundConnectionInserter> c =
        FoundConnectionInserter.class.getDeclaredConstructor(
            RoutingBoard.class, AutorouteControl.class);
    c.setAccessible(true);
    return c.newInstance(board, ctrl);
  }

  static Method tryNeckDownMethod() throws Exception {
    Method m =
        FoundConnectionInserter.class.getDeclaredMethod(
            "tryNeckDown", Point.class, Point.class, int.class, Pin.class, boolean.class);
    m.setAccessible(true);
    return m;
  }

  static Method microMethod() throws Exception {
    Method m =
        FoundConnectionInserter.class.getDeclaredMethod(
            "insertFanoutMicroNeckdown",
            Point.class,
            Point.class,
            int.class,
            int[].class,
            Pin.class,
            Pin.class);
    m.setAccessible(true);
    return m;
  }

  static Pin pin(RoutingBoard board, int id) {
    return (Pin) board.getItem(id);
  }

  // =============================================================================================
  // mode `neck`
  // =============================================================================================

  /**
   * `tryNeckDown` (`:539-676`) and `insertNeckdown` (`:525-537`) over a fixed table.
   *
   * <p>The `smd` pin is item 2 at (-400, 0) — layer 0 only — and the `thru` pin is item 3 at
   * (400, 0). The blocker at x = 0 is what makes `checkTraceSegment` answer a finite length, so
   * the `:579-660` arm runs and inserts up to three full-width segments before the neck itself.
   */
  static void neck() throws Exception {
    int[] widths = {100, 60};
    IntPoint smdCenter = new IntPoint(-400, 0);
    IntPoint thruCenter = new IntPoint(400, 0);
    IntPoint[][] pairs = {
      {new IntPoint(-200, 0), smdCenter},
      {new IntPoint(-100, 0), smdCenter},
      {new IntPoint(200, 0), thruCenter},
      {new IntPoint(-200, 300), smdCenter},
      {smdCenter, thruCenter},
    };
    for (int width : widths) {
      for (int pinChoice = 0; pinChoice < 2; pinChoice++) {
        for (IntPoint[] pair : pairs) {
          buildNeck(width);
          RoutingBoard board = P6T14Probe.board();
          AutorouteControl ctrl = control(board, 1, true);
          Pin thePin = pinChoice == 0 ? pin(board, 2) : pin(board, 3);
          FoundConnectionInserter inst = inserter(board, ctrl);
          Point result =
              (Point)
                  tryNeckDownMethod().invoke(inst, pair[0], pair[1], 0, thePin, true);
          System.out.println(
              "tryNeckDown hw="
                  + width
                  + " pin="
                  + thePin.getId()
                  + " from="
                  + ptOf(pair[0])
                  + " to="
                  + ptOf(pair[1])
                  + " result="
                  + ptOf(result)
                  + " isFrom="
                  + (result == pair[0])
                  + " isTo="
                  + (result == pair[1]));
          dump(board);
        }
      }
    }
    // `insertNeckdown` (`:525-537`): the two-pin dispatch above `tryNeckDown`. The second pair is
    // the one that answers **true**: at half width 100 the blocker's keep-out reaches |x| <= 334
    // but the 49-unit neck's only reaches |x| <= 303, so `checkTraceSegment` (`:566`) answers ~0,
    // `:579-580` sets `neckDownEndPoint = fromCorner` and `:662`'s single neck segment reaches
    // `toCorner` — which is `insertNeckdown`'s own `fromCorner`, the reference `:528` tests.
    IntPoint[][] pairs2 = {
      {new IntPoint(-200, 0), thruCenter},
      {smdCenter, new IntPoint(-310, 0)},
      {new IntPoint(-310, 0), smdCenter},
    };
    for (int width : widths) {
      for (IntPoint[] pair : pairs2) {
        for (int startChoice = 0; startChoice < 2; startChoice++) {
          for (int endChoice = 0; endChoice < 2; endChoice++) {
            buildNeck(width);
            RoutingBoard board = P6T14Probe.board();
            AutorouteControl ctrl = control(board, 1, true);
            Pin startPin = startChoice == 0 ? null : pin(board, 2);
            Pin endPin = endChoice == 0 ? null : pin(board, 3);
            FoundConnectionInserter inst = inserter(board, ctrl);
            boolean result = inst.insertNeckdown(pair[0], pair[1], 0, startPin, endPin);
            System.out.println(
                "insertNeckdown hw="
                    + width
                    + " from="
                    + ptOf(pair[0])
                    + " to="
                    + ptOf(pair[1])
                    + " startPin="
                    + (startPin == null ? "null" : Integer.toString(startPin.getId()))
                    + " endPin="
                    + (endPin == null ? "null" : Integer.toString(endPin.getId()))
                    + " result="
                    + result);
            dump(board);
          }
        }
      }
    }
  }

  // =============================================================================================
  // mode `micro`
  // =============================================================================================

  /**
   * `insertFanoutMicroNeckdown` (`:455-523`), whose `LinkedHashSet` at `:462-471` this mode
   * exists to pin.
   *
   * <p>The candidate list is, in order: the start pin's neckdown half width, the end pin's, then
   * `max(1, 3 * base / 4)`, `max(1, 3 * base / 5)` and `max(1, base / 2)`; duplicates are dropped
   * where they land, not where they would sort. With `base = 100` and both pins that is
   * [49, 69, 75, 60, 50] and the winner is 49 — which a sorted set would also answer. With a
   * **null start pin** it is [69, 75, 60, 50] and the winner is 69, where a sorted set answers
   * 50: that row is the discriminating one.
   */
  static void micro() throws Exception {
    int[] widths = {100, 66};
    // The far pair leaves room for **every** candidate, so the winner is literally the set's
    // first element; the near pair sits 300 from the fixed net-2 blocker, whose keep-out for a
    // trace of half width `h` reaches |x| <= 30 + 210 + h, so 75 and 60 fail there and 50 wins.
    IntPoint[][] pairs = {
      {new IntPoint(-700, 0), new IntPoint(-400, 0)},
      {new IntPoint(-300, 0), new IntPoint(-400, 0)},
    };
    for (int width : widths) {
      for (IntPoint[] pair : pairs) {
        for (int startChoice = 0; startChoice < 2; startChoice++) {
          for (int endChoice = 0; endChoice < 2; endChoice++) {
            buildNeck(width);
            RoutingBoard board = P6T14Probe.board();
            AutorouteControl ctrl = control(board, 1, true);
            Pin startPin = startChoice == 0 ? null : pin(board, 2);
            Pin endPin = endChoice == 0 ? null : pin(board, 3);
            FoundConnectionInserter inst = inserter(board, ctrl);
            boolean result =
                (Boolean)
                    microMethod()
                        .invoke(inst, pair[0], pair[1], 0, new int[] {1}, startPin, endPin);
            System.out.println(
                "micro base="
                    + width
                    + " from="
                    + ptOf(pair[0])
                    + " to="
                    + ptOf(pair[1])
                    + " startPin="
                    + (startPin == null ? "null" : Integer.toString(startPin.getId()))
                    + " endPin="
                    + (endPin == null ? "null" : Integer.toString(endPin.getId()))
                    + " result="
                    + result);
            dump(board);
          }
        }
      }
    }
    // `:457-460`: a null/equal target is the early false, before the set is built at all.
    buildNeck(100);
    RoutingBoard board = P6T14Probe.board();
    AutorouteControl ctrl = control(board, 1, true);
    FoundConnectionInserter inst = inserter(board, ctrl);
    IntPoint from = new IntPoint(-700, 0);
    boolean same =
        (Boolean)
            microMethod().invoke(inst, from, from, 0, new int[] {1}, pin(board, 2), null);
    System.out.println("micro same=" + same);
    dump(board);
  }
}
