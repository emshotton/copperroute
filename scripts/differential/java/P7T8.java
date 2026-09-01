package app.freerouting.autoroute.pipeline;

import app.freerouting.autoroute.ItemRouteResult;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Trace;
import app.freerouting.board.model.items.Via;
import app.freerouting.core.RoutingJob;
import app.freerouting.core.scoring.BoardStatistics;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.settings.FanoutSettings;
import app.freerouting.settings.RouterSettings;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;
import java.util.TreeSet;

/**
 * Plan 7 Task 13 differential driver: {@code BatchOptimizer}'s item half — the protected inner
 * class {@code ReadSortedRouteItems} ({@code BatchOptimizer.java:563-659}), {@code optRouteItem}
 * ({@code :395-514}), {@code containsOnlyUnfixedTraces} ({@code :85-92}) and
 * {@code BatchAutorouter.autoroutePassesForOptimizingItem} ({@code BatchAutorouter.java:245-281}).
 *
 * <p>Declares {@code package app.freerouting.autoroute.pipeline} because {@code
 * ReadSortedRouteItems} is a <b>protected inner class</b> and {@code optRouteItem} is
 * {@code protected}: being in the package is what lets the driver write
 * {@code optimizer.new ReadSortedRouteItems()} and call the method without reflection, which is
 * the same argument {@code P7T5} makes for {@code BatchFanout}'s private members.
 * {@code P7T2.java} is compiled alongside for {@code loadBoard}/{@code buildSettings}/
 * {@code newRouter}/{@code boardShape} and {@code P7T9.java} for {@code dumpBoard}, so the
 * {@code p7t*} drivers cannot describe different boards.
 *
 * <p>Usage: {@code P7T8 <dsn> <mode> [routePasses] [items|all]}. {@code mode} is
 * {@code sequence} or {@code item}; {@code routePasses} goes into {@code settings.maxPasses} for
 * the routing prologue (default 1) and {@code items} bounds the optimizer walk (default 5,
 * {@code all} for the whole board).
 *
 * <h2>The prologue: a real routed board</h2>
 *
 * <p>The optimizer only means anything on a board that has been routed, so every mode starts by
 * routing one with the <i>real</i> {@code BatchAutorouter.runBatchLoop()} — the exact call
 * {@code p7t9} already pins byte for byte on both sides at 18/18 MATCH. The routed board is
 * printed as a {@code ROUTED} line before the optimizer touches it, so a prologue divergence is
 * visible as a prologue divergence rather than as an optimizer one. {@code router.board} is read
 * back after the call rather than reusing the local, because the best-board policy may have
 * replaced it (quirk #209).
 *
 * <h2>Mode {@code sequence}: the visit order, and nothing else</h2>
 *
 * <p>A fresh {@code ReadSortedRouteItems} is walked to exhaustion on the routed board with
 * <b>no</b> mutation between calls, so the transcript is the pure ordering of {@code :573-654}:
 * one {@code SEQ} line per returned item carrying its id, its class, the {@code (x, y)} key the
 * comparison chain used ({@code getCenter().toFloat()} for a via, the {@code compareCorner} of
 * {@code :617-622} for a trace), its layer, and the cursor {@code getCurrentPosition()} reports
 * afterwards. The two {@code OPTPOS} lines pin {@code BatchOptimizer.getCurrentPosition}
 * ({@code :520-525}) on both sides of the {@code sortedRouteItems == null} test.
 *
 * <h2>Mode {@code item}: {@code optRouteItem} against the board the previous one left</h2>
 *
 * <p>This is plan-7 ruling 12's pin. The loop is {@code optRoutePass}' ({@code :327-331}) with the
 * stop conditions removed: {@code next()}, then the <i>real</i> {@code optRouteItem(item,
 * withPreferredDirections, false)}, then {@code next()} again on the board that call mutated. The
 * two fields {@code optRoutePass} seeds are set the way it sets them —
 * {@code useIncreasedRipupCosts = true} ({@code runBatchLoop:132}) and
 * {@code minCumulativeTraceLength = statsBefore.traces.totalWeightedLength} ({@code :288}) — so
 * the {@code ItemRouteResult} the port builds is compared against the one Java builds under the
 * same seeds rather than under a zeroed field.
 *
 * <p>Beside each call the driver <b>transcribes</b> {@code :412-432} (the two ripped sets) and
 * {@code :453-463} (the ripup costs) and prints them, because both are locals of the real method
 * and there is nothing to reflect into. That is the {@code P7T2} technique: the transcription is
 * evidence only because the real call runs next to it on the same board, and the {@code RESULT}
 * and {@code BOARD} lines that follow are the real method's own answer.
 *
 * <h2>The optimizer's own stop flag</h2>
 *
 * <p>The {@code RoutingJob} handed to {@code createForHeadless} carries a <b>new</b>
 * {@code NeverStarted}, not the routing prologue's. {@code AutorouteBatchLoop.java:271} calls
 * {@code requestStopAutoRouter()} when the pass loop reaches {@code maxPasses}, and
 * {@code autoroutePassesForOptimizingItem}'s loop head ({@code :268}) is
 * {@code !job.thread.isStopAutoRouterRequested()} — so reusing the prologue's flag would run
 * <b>zero</b> autoroute passes per item and the driver would compare two untouched boards. That
 * is a real consequence of sharing one job rather than a driver artefact:
 * {@code RoutingPipeline.java:117} gates the optimizer stage on {@code isStopRequested()}, which
 * is {@code ALL}, so a {@code -mp}-bounded run does enter the optimizer with the auto-router
 * already stopped. Task 14/15 owns that seam; this driver is about {@code optRouteItem} doing
 * work.
 *
 * <h2>The budget</h2>
 *
 * <p>Ruling AI: the port runs {@code RouterBudget::disabled()} against this side's live 1000 ms
 * {@code TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP}, which {@code javac} inlines and no reflection can
 * reach ({@code P7T8Probe}'s class comment records the measurement). A MATCH under that asymmetry
 * proves the limit never trips on the corpus. The optimizer's own deadline
 * ({@code BatchOptimizer.deadlineMs}) is never set: {@code runBatchLoop} is Task 14's and this
 * driver does not call it, so {@code settings.optimizer.timeoutString} is never read.
 */
public final class P7T8 {

  private P7T8() {}

  public static void main(String[] args) throws Exception {
    if (args.length < 1) {
      System.err.println("usage: P7T8 <dsn> [mode] [routePasses] [items|all]");
      System.exit(2);
    }
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    String mode = args.length > 1 && !args[1].isBlank() ? args[1] : "sequence";
    if (!"sequence".equals(mode) && !"item".equals(mode)) {
      System.err.println("P7T8: mode must be 'sequence' or 'item', not: " + mode);
      System.exit(2);
    }
    int routePasses = args.length > 2 && !args[2].isBlank() ? Integer.parseInt(args[2]) : 1;
    int maxItems =
        args.length > 3 && !args[3].isBlank()
            ? ("all".equals(args[3]) ? Integer.MAX_VALUE : Integer.parseInt(args[3]))
            : 5;

    Path jar =
        Paths.get(RoutingBoard.class.getProtectionDomain().getCodeSource().getLocation().toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d fixture=%s mode=%s routePasses=%d items=%s%n",
        jar,
        Files.size(jar),
        Files.getLastModifiedTime(jar).toMillis(),
        dsn.getFileName(),
        mode,
        routePasses,
        maxItems == Integer.MAX_VALUE ? "all" : Integer.toString(maxItems));
    System.err.println("java-version " + System.getProperty("java.version"));

    // ---- the prologue: a real routed board -------------------------------------------------
    RoutingBoard loaded = P7T2.loadBoard(dsn);
    RouterSettings settings = buildSettings(loaded, routePasses);
    BatchAutorouter router = P7T2.newRouter(loaded, settings);
    out.println("[route]");
    boolean routerReturned = router.runBatchLoop();
    // `:552` — the loop writes its answer into `router.board`, which the restore arms may have
    // replaced (quirk #209).
    RoutingBoard board = router.board;
    out.println("ROUTED returned=" + routerReturned + " " + P7T2.boardShape(board));

    // The optimizer, built the way `RoutingPipeline` builds it for a headless job (`:51-53`), on
    // a **fresh** `StoppableThread` — see the class comment's "the optimizer's own stop flag".
    RoutingJob job = new RoutingJob();
    job.board = board;
    job.routerSettings = settings;
    job.thread = new P7T2.NeverStarted();
    BatchOptimizer optimizer = BatchOptimizer.createForHeadless(job);

    if ("sequence".equals(mode)) {
      dumpSequence(out, optimizer, board);
    } else {
      dumpItems(out, optimizer, board, maxItems);
    }

    out.println("[board]");
    P7T9.dumpBoard(out, board);
    out.flush();
  }

  /**
   * {@code P7T2.buildSettings} plus the routing prologue's knobs — the {@code p7t9 router-only}
   * mode, so the board this driver optimizes is the board {@code p7t9} already pins.
   */
  static RouterSettings buildSettings(RoutingBoard board, int routePasses) {
    RouterSettings settings = P7T2.buildSettings(board);
    settings.maxPasses = routePasses;
    if (settings.fanout == null) {
      settings.fanout = new FanoutSettings();
    }
    settings.fanout.enabled = Boolean.FALSE;
    settings.fanout.maxMillisecondsPerPin = (long) Integer.MAX_VALUE;
    settings.setRunRouter(true);
    settings.setRunOptimizer(false);
    settings.saveIntermediateStages = Boolean.FALSE;
    return settings;
  }

  // -------------------------------------------------------------------------------------------
  // Mode `sequence` — ReadSortedRouteItems.next(), BatchOptimizer.java:573-654
  // -------------------------------------------------------------------------------------------

  static void dumpSequence(PrintStream out, BatchOptimizer optimizer, RoutingBoard board) {
    out.println("[sequence]");
    // `:520-525` — `getCurrentPosition()` before `sortedRouteItems` exists.
    out.println("OPTPOS-BEFORE " + pos(optimizer.getCurrentPosition()));
    BatchOptimizer.ReadSortedRouteItems reader = optimizer.new ReadSortedRouteItems();
    optimizer.sortedRouteItems = reader;
    out.println("OPTPOS-FRESH " + pos(optimizer.getCurrentPosition()));

    int n = 0;
    // A safety bound that no corpus board can reach: the reader is strictly monotone, so it
    // cannot return more items than the board holds.
    int bound = board.getItems().size() + 1;
    while (n < bound) {
      Item current = reader.next();
      if (current == null) {
        break;
      }
      out.println(
          "SEQ n="
              + n
              + " id="
              + current.getId()
              + " kind="
              + current.getClass().getSimpleName()
              + " key="
              + pos(keyOf(current))
              + " layer="
              + layerOf(current)
              + " cursor="
              + pos(optimizer.getCurrentPosition())
              + " cursorLayer="
              + reader.minItemLayer);
      n++;
    }
    out.println(
        "SEQ-END count="
            + n
            + " cursor="
            + pos(optimizer.getCurrentPosition())
            + " cursorLayer="
            + reader.minItemLayer);
  }

  /** The {@code (x, y)} the comparison chain keys on: {@code :585} for a via, {@code :617-622}
   * for a trace. */
  static FloatPoint keyOf(Item item) {
    if (item instanceof Via via) {
      return via.getCenter().toFloat();
    }
    Trace trace = (Trace) item;
    FloatPoint firstCorner = trace.firstCorner().toFloat();
    FloatPoint lastCorner = trace.lastCorner().toFloat();
    if (firstCorner.x < lastCorner.x
        || firstCorner.x == lastCorner.x && firstCorner.y < lastCorner.y) {
      return lastCorner;
    }
    return firstCorner;
  }

  /** {@code :586} for a via, {@code :623} for a trace. */
  static int layerOf(Item item) {
    if (item instanceof Via via) {
      return via.firstLayer();
    }
    return ((Trace) item).getLayer();
  }

  static String pos(FloatPoint p) {
    if (p == null) {
      return "null";
    }
    return "(" + Double.toString(p.x) + "," + Double.toString(p.y) + ")";
  }

  // -------------------------------------------------------------------------------------------
  // Mode `item` — optRouteItem, BatchOptimizer.java:395-514
  // -------------------------------------------------------------------------------------------

  static void dumpItems(
      PrintStream out, BatchOptimizer optimizer, RoutingBoard board, int maxItems) {
    out.println("[items]");
    // `runBatchLoop:132`.
    optimizer.useIncreasedRipupCosts = true;
    // `optRoutePass:283, :288`.
    BoardStatistics statisticsBefore = board.getStatistics();
    optimizer.minCumulativeTraceLength = statisticsBefore.traces.totalWeightedLength;
    out.println(
        "SEED useIncreasedRipupCosts="
            + optimizer.useIncreasedRipupCosts
            + " minCumulativeTraceLength="
            + Double.toString(optimizer.minCumulativeTraceLength));

    BatchOptimizer.ReadSortedRouteItems reader = optimizer.new ReadSortedRouteItems();
    optimizer.sortedRouteItems = reader;
    // `runBatchLoop:200` — pass 1, so `1 % 2 != 0`.
    boolean withPreferredDirections = true;

    int n = 0;
    while (n < maxItems) {
      Item currentItem = reader.next();
      if (currentItem == null) {
        out.println("NEXT-NULL n=" + n);
        break;
      }

      // The transcription of `:412-432`, printed beside the real call.
      Set<Item> rippedItems = new TreeSet<>();
      rippedItems.add(currentItem);
      if (currentItem instanceof Trace currentTrace) {
        Set<Item> currentContactList = currentTrace.getStartContacts();
        for (int i = 0; i < 2; i++) {
          if (BatchOptimizer.containsOnlyUnfixedTraces(currentContactList)) {
            rippedItems.addAll(currentContactList);
          }
          currentContactList = currentTrace.getEndContacts();
        }
      }
      Set<Item> rippedConnections = new TreeSet<>();
      for (Item item : rippedItems) {
        rippedConnections.addAll(item.getConnectionItems(Item.StopConnectionOption.NONE));
      }
      boolean anyUserFixed = false;
      for (Item item : rippedConnections) {
        if (item.isUserFixed()) {
          anyUserFixed = true;
          break;
        }
      }

      // The transcription of `:453-463`.
      int ripupCosts = optimizer.settings.getStartRipupCosts();
      if (optimizer.useIncreasedRipupCosts) {
        ripupCosts *= optimizer.settings.optimizer.additionalRipupCostFactorAtStart;
      }
      if (currentItem instanceof Trace) {
        ripupCosts =
            (int)
                Math.round(optimizer.settings.optimizer.traceRipupCostFactor * (double) ripupCosts);
      }

      out.println(
          "ITEM n="
              + n
              + " id="
              + currentItem.getId()
              + " kind="
              + currentItem.getClass().getSimpleName()
              + " nets="
              + netList(currentItem)
              + " rippedItems="
              + ids(rippedItems)
              + " rippedConnections="
              + ids(rippedConnections)
              + " anyUserFixed="
              + anyUserFixed
              + " ripupCosts="
              + ripupCosts
              + " maxAutoroutePasses="
              + optimizer.settings.optimizer.maxAutoroutePasses
              + " tracePullTightAccuracy="
              + optimizer.settings.tracePullTightAccuracy);

      int maxIdBefore = board.communication.idGenerator.maxGeneratedId();
      ItemRouteResult result = optimizer.optRouteItem(currentItem, withPreferredDirections, false);

      out.println(
          "RESULT n="
              + n
              + " itemId="
              + result.itemId()
              + " improved="
              + result.improved()
              + " viaCount="
              + result.viaCount()
              + " traceLength="
              + Double.toString(result.traceLength())
              + " incompleteBefore="
              + result.incompleteCountBefore()
              + " incompleteAfter="
              + result.incompleteCount()
              + " viaCountReduced="
              + result.viaCountReduced()
              + " lengthReduced="
              + Double.toString(result.lengthReduced())
              + " improvementPercentage="
              + Float.toString(result.improvementPercentage())
              + " minCumulativeTraceLength="
              + Double.toString(optimizer.minCumulativeTraceLength));
      out.println(
          "BOARD n="
              + n
              + " maxIdBefore="
              + maxIdBefore
              + " maxIdAfter="
              + board.communication.idGenerator.maxGeneratedId()
              + " "
              + P7T2.boardShape(board));
      out.flush();
      n++;
    }
    out.println("ITEMS-END count=" + n + " cursor=" + pos(optimizer.getCurrentPosition()));
  }

  /** Java's {@code TreeSet<Item>} is descending (quirk #44); the rendering reverses so the two
   * sides read the same way, the {@code p6t1} convention. */
  static String ids(Set<Item> items) {
    List<Integer> rendered = new ArrayList<>();
    for (Item item : items) {
      rendered.add(item.getId());
    }
    java.util.Collections.reverse(rendered);
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < rendered.size(); i++) {
      if (i > 0) {
        sb.append(',');
      }
      sb.append(rendered.get(i));
    }
    return sb.append(']').toString();
  }

  static String netList(Item item) {
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < item.netCount(); i++) {
      if (i > 0) {
        sb.append(',');
      }
      sb.append(item.getNetNumber(i));
    }
    return sb.append(']').toString();
  }
}
