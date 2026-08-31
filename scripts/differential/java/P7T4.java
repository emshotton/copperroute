package app.freerouting.autoroute.maze;

import app.freerouting.autoroute.AutorouteAttemptResult;
import app.freerouting.autoroute.maze.AutorouteControl.ExpansionCostFactor;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.ConductionArea;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.items.Via;
import app.freerouting.board.optimize.ViaOptimizer;
import app.freerouting.board.trace.PolylineTrace;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.Line;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import app.freerouting.settings.RouterSettings;
import app.freerouting.settings.sources.DefaultSettings;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.SortedSet;
import java.util.TreeSet;

/**
 * Plan 7 Tasks 6 and 7 differential driver: {@code board.optimize.ViaOptimizer}'s {@code
 * optViaLocation} (ViaOptimizer.java:33-158), {@code optPlaneOrFanoutVia} (:161-296), {@code
 * isWithinTolerance} (:719-732) and the three {@code repositionVia} overloads — A (:302-365),
 * B (:367-429) and C (:434-713) — over a real DSN board whose vias were placed by real routing.
 *
 * <p>Usage: {@code P7T4 <dsn> [mode] [accuracy] [routeK]}. Defaults: {@code mode = 0}, {@code
 * accuracy = 500}, {@code routeK = 12}.
 *
 * <p><b>Package.</b> The brief asks for {@code package app.freerouting.board.optimize} so the two
 * private methods are reachable without reflection. That package cannot see {@code P6T1.loadBoard}
 * / {@code pickConnections} / {@code route}, which are package-private statics in {@code
 * app.freerouting.autoroute.maze} — and without them this driver cannot describe the same board
 * {@code P7T3} does. The {@code P7T3} / {@code P7T7} / {@code P7T10} precedent wins: the driver
 * declares {@code autoroute.maze} and reaches {@code optPlaneOrFanoutVia}, {@code
 * isWithinTolerance} and the three private {@code repositionVia} overloads through {@code
 * setAccessible}, exactly as {@code P6T3} reaches {@code autoroute.expansion}'s private members.
 * Nothing about the measurement changes.
 *
 * <p>Modes:
 *
 * <ul>
 *   <li>{@code 0} — {@code optViaLocation(board, via, traceCosts, accuracy, 10)} over every via of
 *       the routed board, in {@code getItems()} order (descending id, quirk #63), with a non-null
 *       {@code traceCosts} of {@code (1.0, 1.0)} per layer — the array {@code
 *       TraceTightener.optChangedArea:160-164} hands it.
 *   <li>{@code 1} — {@code optPlaneOrFanoutVia(board, via, accuracy, 10)} directly, same walk.
 *   <li>{@code 2} — {@code isWithinTolerance} over 10 000 scripted triples plus 256 exact-boundary
 *       ones; no board is loaded, so the {@code <dsn>} argument is ignored (it is still required,
 *       and still printed, so the two sides' header lines agree).
 *   <li>{@code 3} — {@code repositionVia} <b>overload A</b> ({@code :302-365}) driven directly,
 *       over a scripted family of 27 {@code toLocation}s per via.
 *   <li>{@code 4} — {@code repositionVia} <b>overload B</b> ({@code :367-429}) driven directly,
 *       over 11 {@code toLocation}s x 2 role assignments per via.
 *   <li>{@code 5} — {@code repositionVia} <b>overload C</b> ({@code :434-713}) driven directly,
 *       with the arguments {@code optViaLocation:118-131} would build and five cost-pair
 *       combinations, so the weighted comparisons at {@code :492}, {@code :514}, {@code :555},
 *       {@code :597}, {@code :625}, {@code :663} and {@code :693} all decide both ways.
 *   <li>{@code 6} — mode 0 with {@code traceCosts = null}, which is {@code optViaLocation}'s
 *       {@code :113-116} else-branch ({@code ExpansionCostFactor(1, 1)} for both layers).
 *       <b>Numbered 6, not 3</b>: {@code task-7-brief.md:29} reserved modes {@code 3}, {@code 4}
 *       and {@code 5} for one {@code repositionVia} overload each, which is what they now are.
 * </ul>
 *
 * <p><b>The three overloads mutate nothing</b> — {@code checkTraceSegment} and {@code
 * DrillItemMover.check} (called with both recursion depths at zero) are read-only probes — so
 * modes 3-5 may call them many times per via and still end on the board the routing prologue
 * built. Each of the three prints that board afterwards, which is what pins "no item was inserted
 * and no id was burned".
 *
 * <p>Per via the driver prints, <b>before</b> the call: the via id, its centre, its normal-contact
 * ids (descending — {@code Item.compareTo} is {@code other.id - id}), and the dispatch class that
 * {@code :46-78} computes, replicated read-only here so a port that reaches the wrong overload is a
 * diff even when both answer {@code false}. Then the returned boolean and the via's centre after.
 * Finally the whole board in {@code P7T3}'s dump format.
 *
 * <p><b>The budget is disabled on both sides.</b> {@code ViaOptimizer} has no time limit of its
 * own; the routing prologue is {@code P7T3}'s, and the {@code optChangedArea} the port's {@code
 * DrillItemMover} tail can reach is entered with {@code RouterBudget::disabled()}. No reflection
 * into a constant is needed.
 */
public final class P7T4 {

  private P7T4() {}

  static PrintStream out;

  /** The dispatch classes of {@code optViaLocation:39-78}, computed read-only. */
  enum Dispatch {
    /** {@code :39-41} — {@code via.isShoveFixed()}. */
    SHOVE_FIXED,
    /** {@code :42-45} — {@code maxRecursionDepth <= 0}. Unreachable from this driver. */
    DEPTH_EXHAUSTED,
    /** {@code :47} — {@code contacts.size() == 1}, straight to the plane/fanout arm. */
    PLANE_OR_FANOUT_ONE_CONTACT,
    /** {@code :51-53} — {@code contacts.size() != 2} and not 1. */
    WRONG_CONTACT_COUNT,
    /** {@code :57-58} or {@code :67-68} — a {@code ConductionArea} contact promoted the via. */
    PLANE_OR_FANOUT_CONDUCTION,
    /** {@code :60} or {@code :70} — a contact that is neither a free trace nor a plane. */
    UNUSABLE_CONTACT,
    /** {@code :89-106} — two free traces, but the via is not at an endpoint of one of them. */
    NOT_AT_ENDPOINT,
    /** {@code :118-131} — two free traces at endpoints: the twelve-argument overload runs. */
    TWO_TRACES,
  }

  public static void main(String[] args) throws Exception {
    if (args.length < 1) {
      System.err.println("usage: P7T4 <dsn> [mode] [accuracy] [routeK]  (modes 0, 1, 2, 3, 4, 5, 6)");
      System.exit(2);
    }
    out = new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    int mode = args.length > 1 ? Integer.parseInt(args[1]) : 0;
    int accuracy = args.length > 2 ? Integer.parseInt(args[2]) : 500;
    int routeK = args.length > 3 ? Integer.parseInt(args[3]) : 12;

    Path jar =
        Paths.get(RoutingBoard.class.getProtectionDomain().getCodeSource().getLocation().toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d fixture=%s mode=%d accuracy=%d routeK=%d%n",
        jar,
        Files.size(jar),
        Files.getLastModifiedTime(jar).toMillis(),
        dsn.getFileName(),
        mode,
        accuracy,
        routeK);
    System.err.println("java-version " + System.getProperty("java.version"));

    if (mode == 2) {
      toleranceTriples();
      return;
    }

    RoutingBoard board = P6T1.loadBoard(dsn, null);
    RouterSettings settings = new DefaultSettings().getSettings();
    settings.setLayerCount(board.getLayerCount());
    settings.applyBoardSpecificOptimizations(board);

    for (P6T1.Connection connection : P6T1.pickConnections(board, routeK)) {
      Item item = board.getItem(connection.itemId());
      if (item == null) {
        out.println("route k=" + connection.k() + " item=" + connection.itemId() + " state=GONE");
        continue;
      }
      board.startMarkingChangedArea();
      SortedSet<Item> rippedItemList = new TreeSet<>();
      Map<Item, Integer> ripupCosts = new LinkedHashMap<>();
      AutorouteAttemptResult result =
          P6T1.route(board, settings, item, connection.netNo(), rippedItemList, ripupCosts, 1);
      out.println(
          "route k="
              + connection.k()
              + " item="
              + connection.itemId()
              + " net="
              + connection.netNo()
              + " state="
              + result.state
              + " ripped="
              + rippedItemList.size());
    }

    ExpansionCostFactor[] traceCosts = null;
    if (mode != 6) {
      traceCosts = new ExpansionCostFactor[board.getLayerCount()];
      for (int i = 0; i < traceCosts.length; i++) {
        traceCosts[i] = new ExpansionCostFactor(1.0, 1.0);
      }
    }
    out.println(
        "sweep regime="
            + board.rules.getTraceAngleRestriction()
            + " traceCosts="
            + (traceCosts == null ? "null" : traceCosts.length));

    // The via ids are snapshotted first: the calls insert and remove items, and Java's
    // `getItems()` iterator would not survive that either.
    List<Integer> viaIds = new ArrayList<>();
    for (Item item : board.getItems()) {
      if (item instanceof Via) {
        viaIds.add(item.getId());
      }
    }
    out.println("vias n=" + viaIds.size());

    if (mode == 3 || mode == 4 || mode == 5) {
      if (mode == 3) {
        driveOverloadA(board, viaIds);
      } else if (mode == 4) {
        driveOverloadB(board, viaIds);
      } else {
        driveOverloadC(board, viaIds);
      }
      dumpBoard(board);
      return;
    }

    Method optPlaneOrFanoutVia = null;
    if (mode == 1) {
      optPlaneOrFanoutVia =
          ViaOptimizer.class.getDeclaredMethod(
              "optPlaneOrFanoutVia", RoutingBoard.class, Via.class, int.class, int.class);
      optPlaneOrFanoutVia.setAccessible(true);
    }

    for (int viaId : viaIds) {
      Item item = board.getItem(viaId);
      if (!(item instanceof Via via)) {
        out.println("via id=" + viaId + " state=GONE");
        continue;
      }
      StringBuilder sb = new StringBuilder();
      sb.append("via id=")
          .append(viaId)
          .append(" center=")
          .append(pointOf(via.getCenter()))
          .append(" minWidth=")
          .append(Double.toString(via.minWidth()))
          .append(" contacts=")
          .append(contactIds(via))
          .append(" class=")
          .append(classify(via));
      boolean result;
      if (mode == 1) {
        result = (Boolean) optPlaneOrFanoutVia.invoke(null, board, via, accuracy, 10);
      } else {
        result = ViaOptimizer.optViaLocation(board, via, traceCosts, accuracy, 10);
      }
      sb.append(" result=").append(result);
      Item after = board.getItem(viaId);
      sb.append(" after=")
          .append(after instanceof Via afterVia ? pointOf(afterVia.getCenter()) : "gone");
      out.println(sb);
    }

    dumpBoard(board);
  }

  // --- mode 2 ----------------------------------------------------------------------------------

  /**
   * {@code isWithinTolerance(Point, Point, int)} (:719-732) over a scripted stream. The generator
   * is a 64-bit LCG written out in full so the Rust twin reproduces it with {@code wrapping_mul} /
   * {@code wrapping_add} and an unsigned shift; every value it feeds is an {@code IntPoint}, which
   * is the only {@code Point} subclass the three call sites can produce (trace corners and via
   * centres are integral on every routed board).
   *
   * <p>The last 256 triples are exact-boundary ones: {@code dx + dy == tolerance} by construction,
   * which is the {@code <=} the method turns on. Java's {@code p1 == null || p2 == null} guard
   * (:720-722) is unreachable from all three call sites — {@code firstCorner}, {@code lastCorner}
   * and {@code getCenter} never answer null — and the port therefore takes {@code Point} by value;
   * the guard is rostered, not exercised.
   */
  static void toleranceTriples() throws Exception {
    Method isWithinTolerance =
        ViaOptimizer.class.getDeclaredMethod(
            "isWithinTolerance", Point.class, Point.class, int.class);
    isWithinTolerance.setAccessible(true);

    long seed = 0x5DEECE66DL;
    for (int i = 0; i < 10000; i++) {
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int x1 = (int) (seed >>> 33) % 4001 - 2000;
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int y1 = (int) (seed >>> 33) % 4001 - 2000;
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int x2 = (int) (seed >>> 33) % 4001 - 2000;
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int y2 = (int) (seed >>> 33) % 4001 - 2000;
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int tolerance = (int) (seed >>> 33) % 121 - 20;
      IntPoint p1 = new IntPoint(x1, y1);
      IntPoint p2 = new IntPoint(x2, y2);
      boolean answer = (Boolean) isWithinTolerance.invoke(null, p1, p2, tolerance);
      out.println(
          "tol i="
              + i
              + " p1=("
              + x1
              + ","
              + y1
              + ") p2=("
              + x2
              + ","
              + y2
              + ") t="
              + tolerance
              + " -> "
              + answer);
    }
    // The boundary family: dx + dy is exactly `tolerance - 1`, `tolerance` and `tolerance + 1`.
    for (int i = 0; i < 256; i++) {
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int x1 = (int) (seed >>> 33) % 4001 - 2000;
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int y1 = (int) (seed >>> 33) % 4001 - 2000;
      int dx = i % 16;
      int tolerance = i / 16 + dx;
      for (int delta = -1; delta <= 1; delta++) {
        int dy = tolerance - dx + delta;
        IntPoint p1 = new IntPoint(x1, y1);
        IntPoint p2 = new IntPoint(x1 + dx, y1 + dy);
        boolean answer = (Boolean) isWithinTolerance.invoke(null, p1, p2, tolerance);
        out.println(
            "bnd i="
                + i
                + " d="
                + delta
                + " p1=("
                + x1
                + ","
                + y1
                + ") p2=("
                + (x1 + dx)
                + ","
                + (y1 + dy)
                + ") t="
                + tolerance
                + " -> "
                + answer);
      }
    }
  }

  // --- the read-only replica of the dispatch ----------------------------------------------------

  static String contactIds(Via via) {
    StringBuilder sb = new StringBuilder("[");
    boolean first = true;
    for (Item contact : via.getNormalContacts()) {
      if (!first) {
        sb.append(",");
      }
      first = false;
      sb.append(contact.getId()).append(":").append(contact.getClass().getSimpleName());
    }
    return sb.append("]").toString();
  }

  /** A read-only replica of {@code optViaLocation:39-106}'s classification. */
  static Dispatch classify(Via via) {
    if (via.isShoveFixed()) {
      return Dispatch.SHOVE_FIXED;
    }
    Collection<Item> contacts = via.getNormalContacts();
    if (contacts.size() == 1) {
      return Dispatch.PLANE_OR_FANOUT_ONE_CONTACT;
    }
    if (contacts.size() != 2) {
      return Dispatch.WRONG_CONTACT_COUNT;
    }
    PolylineTrace firstTrace = null;
    PolylineTrace secondTrace = null;
    boolean isPlaneOrFanoutVia = false;
    Iterator<Item> it = contacts.iterator();
    Item currentItem = it.next();
    if (currentItem.isShoveFixed() || !(currentItem instanceof PolylineTrace)) {
      if (currentItem instanceof ConductionArea) {
        isPlaneOrFanoutVia = true;
      } else {
        return Dispatch.UNUSABLE_CONTACT;
      }
    } else {
      firstTrace = (PolylineTrace) currentItem;
    }
    currentItem = it.next();
    if (currentItem.isShoveFixed() || !(currentItem instanceof PolylineTrace)) {
      if (currentItem instanceof ConductionArea) {
        isPlaneOrFanoutVia = true;
      } else {
        return Dispatch.UNUSABLE_CONTACT;
      }
    } else {
      secondTrace = (PolylineTrace) currentItem;
    }
    if (isPlaneOrFanoutVia) {
      return Dispatch.PLANE_OR_FANOUT_CONDUCTION;
    }
    Point viaCenter = via.getCenter();
    int tolerance = (int) (via.minWidth() / 2) + 1;
    if (!within(firstTrace.firstCorner(), viaCenter, tolerance)
        && !within(firstTrace.lastCorner(), viaCenter, tolerance)) {
      return Dispatch.NOT_AT_ENDPOINT;
    }
    if (!within(secondTrace.firstCorner(), viaCenter, tolerance)
        && !within(secondTrace.lastCorner(), viaCenter, tolerance)) {
      return Dispatch.NOT_AT_ENDPOINT;
    }
    return Dispatch.TWO_TRACES;
  }

  /** {@code isWithinTolerance:719-732}, re-transcribed so {@code classify} needs no reflection. */
  static boolean within(Point p1, Point p2, int tolerance) {
    if (p1 == null || p2 == null) {
      return false;
    }
    FloatPoint fp1 = p1.toFloat();
    FloatPoint fp2 = p2.toFloat();
    double dx = Math.abs(fp1.x - fp2.x);
    double dy = Math.abs(fp1.y - fp2.y);
    return (dx + dy) <= tolerance;
  }

  // --- modes 3, 4 and 5: one `repositionVia` overload each -------------------------------------

  /**
   * The scripted {@code toLocation} family mode 3 walks per via: the contact trace's two inner
   * corners, the via centre itself (which is overload A's {@code :312-314} early return) and six
   * offsets at each of four magnitudes — 1 is inside every pad, 100000 is off the end of most
   * traces, and the two in between straddle the {@code okLength} sentinel.
   */
  static List<IntPoint> targetsA(IntPoint center, PolylineTrace trace) {
    List<IntPoint> result = new ArrayList<>();
    Polyline p = trace.polyline();
    result.add(p.corner(1).toFloat().round());
    result.add(p.corner(p.cornerCount() - 2).toFloat().round());
    result.add(center);
    for (int d : new int[] {1, 1000, 10000, 100000}) {
      result.add(new IntPoint(center.x + d, center.y));
      result.add(new IntPoint(center.x, center.y + d));
      result.add(new IntPoint(center.x + d, center.y + d));
      result.add(new IntPoint(center.x - d, center.y));
      result.add(new IntPoint(center.x, center.y - d));
      result.add(new IntPoint(center.x - d, center.y - d));
    }
    return result;
  }

  /**
   * Mode 4's {@code toLocation} family: the four axis-parallel decomposition points overload C
   * builds at {@code :584}, {@code :615}, {@code :646} and {@code :682}, then the two from-corners,
   * the centre and four short offsets — the last of which straddle overload B's {@code :388-397}
   * refusal of a move of length &le; 1.5 under {@code AngleRestriction.NONE}.
   */
  static List<IntPoint> targetsB(IntPoint center, IntPoint c1, IntPoint c2) {
    List<IntPoint> result = new ArrayList<>();
    result.add(new IntPoint(center.x, c1.y));
    result.add(new IntPoint(c1.x, center.y));
    result.add(new IntPoint(center.x, c2.y));
    result.add(new IntPoint(c2.x, center.y));
    result.add(c1);
    result.add(c2);
    result.add(center);
    result.add(new IntPoint(center.x + 1, center.y));
    result.add(new IntPoint(center.x + 1, center.y + 1));
    result.add(new IntPoint(center.x + 1000, center.y + 1000));
    result.add(new IntPoint(center.x - 1000, center.y));
    return result;
  }

  /** Mode 5's cost pairs — {@code (horizontal, vertical)} per layer, five combinations. */
  static final double[][] COST_PAIRS = {
    {1.0, 1.0, 1.0, 1.0},
    {1.0, 2.0, 2.0, 1.0},
    {2.0, 1.0, 1.0, 2.0},
    {1.0, 1.0, 2.0, 2.0},
    {3.0, 1.0, 1.0, 3.0},
  };

  /** The trace contacts of a via, in {@code getNormalContacts()} order (descending id). */
  static List<PolylineTrace> traceContacts(Via via) {
    List<PolylineTrace> result = new ArrayList<>();
    for (Item currentContact : via.getNormalContacts()) {
      if (currentContact instanceof PolylineTrace trace) {
        result.add(trace);
      }
    }
    return result;
  }

  /**
   * {@code optViaLocation:89-96}, as a helper: the trace corner next to the via, or {@code
   * corner(1)} when the via is at neither end (so the scripted families are still defined).
   */
  static Point fromCornerOf(PolylineTrace trace, Point viaCenter, int tolerance) {
    Polyline p = trace.polyline();
    if (within(trace.firstCorner(), viaCenter, tolerance)) {
      return p.corner(1);
    }
    if (within(trace.lastCorner(), viaCenter, tolerance)) {
      return p.corner(p.cornerCount() - 2);
    }
    return p.corner(1);
  }

  /** Mode 3: {@code repositionVia} overload A ({@code :302-365}), driven directly. */
  static void driveOverloadA(RoutingBoard board, List<Integer> viaIds) throws Exception {
    Method repositionVia =
        ViaOptimizer.class.getDeclaredMethod(
            "repositionVia",
            RoutingBoard.class,
            Via.class,
            IntPoint.class,
            int.class,
            int.class,
            int.class);
    repositionVia.setAccessible(true);
    for (int viaId : viaIds) {
      Item item = board.getItem(viaId);
      if (!(item instanceof Via via)) {
        out.println("repA id=" + viaId + " state=GONE");
        continue;
      }
      List<PolylineTrace> traces = traceContacts(via);
      if (traces.isEmpty()) {
        out.println("repA id=" + viaId + " state=NO_TRACE_CONTACT");
        continue;
      }
      PolylineTrace trace = traces.get(0);
      IntPoint center = via.getCenter().toFloat().round();
      int hw = trace.getHalfWidth();
      int layer = trace.getLayer();
      int cl = trace.clearanceClassIndex();
      List<IntPoint> targets = targetsA(center, trace);
      for (int k = 0; k < targets.size(); k++) {
        IntPoint to = targets.get(k);
        Point answer = (Point) repositionVia.invoke(null, board, via, to, hw, layer, cl);
        out.println(
            "repA id="
                + viaId
                + " k="
                + k
                + " to=("
                + to.x
                + ","
                + to.y
                + ") hw="
                + hw
                + " layer="
                + layer
                + " cl="
                + cl
                + " -> "
                + (answer == null ? "null" : pointOf(answer)));
      }
    }
  }

  /** Mode 4: {@code repositionVia} overload B ({@code :367-429}), driven directly. */
  static void driveOverloadB(RoutingBoard board, List<Integer> viaIds) throws Exception {
    Method repositionVia =
        ViaOptimizer.class.getDeclaredMethod(
            "repositionVia",
            RoutingBoard.class,
            Via.class,
            IntPoint.class,
            int.class,
            int.class,
            int.class,
            IntPoint.class,
            int.class,
            int.class,
            int.class);
    repositionVia.setAccessible(true);
    for (int viaId : viaIds) {
      Item item = board.getItem(viaId);
      if (!(item instanceof Via via)) {
        out.println("repB id=" + viaId + " state=GONE");
        continue;
      }
      List<PolylineTrace> traces = traceContacts(via);
      if (traces.isEmpty()) {
        out.println("repB id=" + viaId + " state=NO_TRACE_CONTACT");
        continue;
      }
      PolylineTrace t1 = traces.get(0);
      PolylineTrace t2 = traces.size() > 1 ? traces.get(1) : traces.get(0);
      Point viaCenter = via.getCenter();
      int tolerance = (int) (via.minWidth() / 2) + 1;
      IntPoint center = viaCenter.toFloat().round();
      IntPoint c1 = fromCornerOf(t1, viaCenter, tolerance).toFloat().round();
      IntPoint c2 = fromCornerOf(t2, viaCenter, tolerance).toFloat().round();
      List<IntPoint> targets = targetsB(center, c1, c2);
      for (int k = 0; k < targets.size(); k++) {
        IntPoint to = targets.get(k);
        for (int r = 0; r < 2; r++) {
          // r = 0 is overload C's `!firstDelta.isOrthogonal()` role assignment (:599), r = 1 its
          // `!secondDelta.isOrthogonal()` one (:665): the two traces swap places.
          PolylineTrace moved = r == 0 ? t2 : t1;
          PolylineTrace connected = r == 0 ? t1 : t2;
          IntPoint connect = r == 0 ? c1 : c2;
          boolean answer =
              (Boolean)
                  repositionVia.invoke(
                      null,
                      board,
                      via,
                      to,
                      moved.getHalfWidth(),
                      moved.getLayer(),
                      moved.clearanceClassIndex(),
                      connect,
                      connected.getHalfWidth(),
                      connected.getLayer(),
                      connected.clearanceClassIndex());
          out.println(
              "repB id="
                  + viaId
                  + " k="
                  + k
                  + " r="
                  + r
                  + " to=("
                  + to.x
                  + ","
                  + to.y
                  + ") connect=("
                  + connect.x
                  + ","
                  + connect.y
                  + ") -> "
                  + answer);
        }
      }
    }
  }

  /** Mode 5: {@code repositionVia} overload C ({@code :434-713}), driven directly. */
  static void driveOverloadC(RoutingBoard board, List<Integer> viaIds) throws Exception {
    Method repositionVia =
        ViaOptimizer.class.getDeclaredMethod(
            "repositionVia",
            RoutingBoard.class,
            Via.class,
            int.class,
            int.class,
            int.class,
            ExpansionCostFactor.class,
            Point.class,
            int.class,
            int.class,
            int.class,
            ExpansionCostFactor.class,
            Point.class);
    repositionVia.setAccessible(true);
    for (int viaId : viaIds) {
      Item item = board.getItem(viaId);
      if (!(item instanceof Via via)) {
        out.println("repC id=" + viaId + " state=GONE");
        continue;
      }
      Dispatch dispatch = classify(via);
      if (dispatch != Dispatch.TWO_TRACES) {
        out.println("repC id=" + viaId + " class=" + dispatch + " state=SKIP");
        continue;
      }
      List<PolylineTrace> traces = traceContacts(via);
      PolylineTrace t1 = traces.get(0);
      PolylineTrace t2 = traces.get(1);
      Point viaCenter = via.getCenter();
      int tolerance = (int) (via.minWidth() / 2) + 1;
      Point c1 = fromCornerOf(t1, viaCenter, tolerance);
      Point c2 = fromCornerOf(t2, viaCenter, tolerance);
      for (int k = 0; k < COST_PAIRS.length; k++) {
        double[] pair = COST_PAIRS[k];
        ExpansionCostFactor costs1 = new ExpansionCostFactor(pair[0], pair[1]);
        ExpansionCostFactor costs2 = new ExpansionCostFactor(pair[2], pair[3]);
        Point answer =
            (Point)
                repositionVia.invoke(
                    null,
                    board,
                    via,
                    t1.getHalfWidth(),
                    t1.clearanceClassIndex(),
                    t1.getLayer(),
                    costs1,
                    c1,
                    t2.getHalfWidth(),
                    t2.clearanceClassIndex(),
                    t2.getLayer(),
                    costs2,
                    c2);
        out.println(
            "repC id="
                + viaId
                + " k="
                + k
                + " costs1=("
                + Double.toString(pair[0])
                + ","
                + Double.toString(pair[1])
                + ") costs2=("
                + Double.toString(pair[2])
                + ","
                + Double.toString(pair[3])
                + ") from1="
                + pointOf(c1)
                + " from2="
                + pointOf(c2)
                + " -> "
                + (answer == null ? "null" : pointOf(answer)));
      }
    }
  }

  // --- dumps (P7T3's, verbatim) -----------------------------------------------------------------

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
    return sb.append("]").toString();
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

  static void dumpBoard(RoutingBoard board) {
    out.println("maxId=" + board.communication.idGenerator.maxGeneratedId());
    for (Item item : board.getItems()) {
      StringBuilder sb = new StringBuilder();
      sb.append("item id=")
          .append(item.getId())
          .append(" type=")
          .append(item.getClass().getSimpleName())
          .append(" nets=")
          .append(nets(item.netNumbers))
          .append(" cl=")
          .append(item.clearanceClassIndex())
          .append(" fix=")
          .append(item.getFixedState());
      if (item instanceof PolylineTrace trace) {
        sb.append(" layer=")
            .append(trace.getLayer())
            .append(" hw=")
            .append(trace.getHalfWidth())
            .append(" ")
            .append(poly(trace.polyline()));
      } else if (item instanceof Via via) {
        sb.append(" center=").append(pointOf(via.getCenter()));
      } else if (item instanceof Pin pin) {
        sb.append(" center=").append(pointOf(pin.getCenter()));
      }
      out.println(sb);
    }
  }
}
