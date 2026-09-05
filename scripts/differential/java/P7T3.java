package app.freerouting.autoroute.maze;

import app.freerouting.autoroute.maze.AutorouteControl.ExpansionCostFactor;
import app.freerouting.autoroute.AutorouteAttemptResult;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.items.Via;
import app.freerouting.board.model.structure.AngleRestriction;
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
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.SortedSet;
import java.util.TreeSet;

/**
 * Plan 7 Task 5 differential driver, board level: {@code RoutingBoard.optChangedArea}
 * (RoutingBoard.java:151-161 -> :171-190 -> RoutingBoardOperations.java:52-79) and the {@code
 * TraceTightener.optChangedArea(ExpansionCostFactor[])} sweep it drives
 * (TraceTightener.java:121-169), over a real DSN board whose changed area was marked by real
 * routing.
 *
 * <p>Usage: {@code P7T3 <dsn> [mode] [accuracy] [routeK]}. Defaults: {@code mode = 0}, {@code
 * accuracy = 500}, {@code routeK = 6}.
 *
 * <p>The first {@code routeK} connections are routed with {@code P6T1}'s own machinery — its
 * {@code loadBoard} / {@code pickConnections} / {@code route} are package-private statics, which
 * is why this driver declares {@code package app.freerouting.autoroute.maze} rather than the
 * brief's {@code board.optimize}, and why {@code P6T1.java} is compiled alongside it (the {@code
 * P7T7} / {@code P7T10} precedent: the two drivers cannot then describe different boards). {@code
 * AutoroutePassRunner.java:224} calls {@code startMarkingChangedArea()} before every connection
 * and {@code P6T1.route} reproduces that, so after the loop {@code board.changedArea} holds
 * exactly the regions the router touched — which is the input {@code optChangedArea} is defined
 * over.
 *
 * <p>Modes, which change only what happens <b>after</b> the routing, so every mode sweeps the same
 * routed board:
 *
 * <ul>
 *   <li>{@code 0} — no vias offered to the optimiser ({@code traceCosts = null}) and the angle
 *       restriction forced to {@code NINETY_DEGREE};
 *   <li>{@code 1} — the same, {@code FORTYFIVE_DEGREE};
 *   <li>{@code 2} — the same, {@code NONE} (any angle);
 *   <li>{@code 3} — mode 2 plus {@code pinEdgeToTurnDist = 500}, which opens {@code
 *       PolylineTrace.pullTight:841-861} and therefore the {@code swap}/{@code
 *       correctConnectionToPin} pair inside the sweep;
 *   <li>{@code 4} — mode 2 plus a non-null {@code traceCosts}, which opens the {@code
 *       ViaOptimizer.optViaLocation} arm at {@code TraceTightener.java:160-165}. <b>This mode is
 *       expected to diff until Plan 7 Task 6 lands {@code ViaOptimizer}</b>; the port stubs the
 *       arm to {@code false} behind an {@code obligation:} marker naming that task.
 * </ul>
 *
 * <p><b>The budget is disabled on both sides.</b> Java's callers all pass the literal {@code
 * TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP = 1000} into {@code optChangedArea}'s {@code timeLimit}
 * parameter, and {@code TraceTightener}'s constructor builds a {@code TimeLimit} only when {@code
 * timeLimit > 0} (TraceTightener.java:73-77). At <b>this</b> entry point the limit is therefore a
 * plain argument, not a constant to be patched: the driver passes {@code 0}, which is Java's own
 * "no limit", and the port passes {@code RouterBudget::disabled()}, whose {@code
 * opt_changed_area_ms} is {@code 0} for the same reason. No reflection is needed or used, and a
 * wall-clock difference between the two languages therefore cannot move the result.
 *
 * <p>The output is the {@code HEADER} line ({@code P6T1}'s convention: a run against the wrong jar
 * is a diff, not a silent pass), one line per routed connection's outcome, then the {@code
 * changedArea} per layer before the sweep, then the whole board in {@code getItems()} order —
 * descending id, quirk #63 — in {@code P6T15aProbe}'s polyline format.
 */
public final class P7T3 {

  private P7T3() {}

  static PrintStream out;

  public static void main(String[] args) throws Exception {
    if (args.length < 1) {
      System.err.println("usage: P7T3 <dsn> [mode] [accuracy] [routeK]");
      System.exit(2);
    }
    out = new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    int mode = args.length > 1 ? Integer.parseInt(args[1]) : 0;
    int accuracy = args.length > 2 ? Integer.parseInt(args[2]) : 500;
    int routeK = args.length > 3 ? Integer.parseInt(args[3]) : 6;

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

    RoutingBoard board = P6T1.loadBoard(dsn, null);
    RouterSettings settings = new DefaultSettings().getSettings();
    settings.setLayerCount(board.getLayerCount());
    settings.applyBoardSpecificOptimizations(board);

    // `P6T1.route` rather than `P6T1.routeOne`: the per-connection JSON is `p6t1`'s comparison
    // surface and duplicating its renderer in this twin would buy nothing. What this driver needs
    // from the routing is the *board* and its `changedArea`, so it prints only the outcome.
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
          P6T1.route(
              board, settings, item, connection.netNo(), rippedItemList, ripupCosts, 1);
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

    // The regime and the pin-edge distance are set **after** the routing, so all five modes sweep
    // the same board.
    AngleRestriction regime =
        switch (mode) {
          case 0 -> AngleRestriction.NINETY_DEGREE;
          case 1 -> AngleRestriction.FORTYFIVE_DEGREE;
          default -> AngleRestriction.NONE;
        };
    board.rules.setTraceAngleRestriction(regime);
    if (mode == 3) {
      // The corpus DSNs already carry a positive `pinEdgeToTurnDist` (rpi-splitter's is 20320),
      // so `PolylineTrace.pullTight:841-861` is open in modes 1 and 2 as well. Mode 3 widens it
      // to five times that, which moves the `checkConnectionToPin:1074-1076` threshold past every
      // routed stub and turns the pair from occasionally-firing into routinely-firing.
      board.rules.setPinEdgeToTurnDist(100000);
    }
    ExpansionCostFactor[] traceCosts = null;
    if (mode == 4) {
      traceCosts = new ExpansionCostFactor[board.getLayerCount()];
      for (int i = 0; i < traceCosts.length; i++) {
        traceCosts[i] = new ExpansionCostFactor(1.0, 1.0);
      }
    }

    out.println(
        "sweep regime="
            + regime
            + " pinEdgeToTurnDist="
            + Double.toString(board.rules.getPinEdgeToTurnDist())
            + " traceCosts="
            + (traceCosts == null ? "null" : traceCosts.length));
    dumpChangedArea(board, "before");

    // `clipShape = null` runs `RoutingBoardOperations.java:64`'s branch — the reference comparison
    // ruling 9 is about. `stoppableThread = null` and `timeLimit = 0` are the disabled budget.
    board.optChangedArea(new int[0], null, accuracy, traceCosts, null, 0);

    dumpChangedArea(board, "after");
    dumpBoard(board);
  }

  static void dumpChangedArea(RoutingBoard board, String tag) {
    if (board.changedArea == null) {
      out.println("changedArea " + tag + "=null");
      return;
    }
    StringBuilder sb = new StringBuilder("changedArea " + tag + "=[");
    for (int i = 0; i < board.getLayerCount(); i++) {
      if (i > 0) {
        sb.append(",");
      }
      var area = board.changedArea.getArea(i);
      // `IntOctagon.toString()` is `Object`'s, i.e. the identity hash — which `-XX:hashCode=2`
      // pins to a constant and the port cannot reproduce. Print the eight coordinates instead;
      // `isEmpty()` is Java's reference test against the `EMPTY` singleton, which
      // `ChangedArea.getArea:107` returns for an untouched layer.
      sb.append(
          area.isEmpty()
              ? "empty"
              : "("
                  + area.leftX
                  + ","
                  + area.bottomY
                  + ","
                  + area.rightX
                  + ","
                  + area.topY
                  + ","
                  + area.upperLeftDiagonalX
                  + ","
                  + area.lowerRightDiagonalX
                  + ","
                  + area.lowerLeftDiagonalX
                  + ","
                  + area.upperRightDiagonalX
                  + ")");
    }
    out.println(sb.append("]"));
  }

  // --- dumps ---------------------------------------------------------------------------------

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
