package app.freerouting.autoroute.pipeline;

import app.freerouting.autoroute.AutorouteAttemptResult;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.DrillItem;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.core.scoring.BoardStatistics;
import app.freerouting.datastructures.TimeLimit;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.rules.ViaRule;
import app.freerouting.settings.RouterSettings;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;
import java.util.SortedSet;
import java.util.TreeSet;

/**
 * Plan 7 Task 11 differential driver: {@code BatchFanout}'s component/pin ordering
 * (BatchFanout.java:35-78, :631-693, :695-778) and {@code RoutingBoard.fanout}
 * (RoutingBoard.java:978-1110), against the port's {@code BatchFanout::new} /
 * {@code FanoutComponent} / {@code FanoutPin} and {@code RoutingBoardExt::fanout}.
 *
 * <p>Usage: {@code P7T5 <dsn> [passNo] [sortingOrder] [mode]}, {@code mode} in {@code order} |
 * {@code pin} | {@code pass} | {@code board}. The first two are Task 11's, the last two Task
 * 12's. In mode {@code board} the second argument is read as {@code maxPasses} rather than as a
 * pass number, and {@code 0} keeps whatever {@code settings.fanout.maxPasses} holds (20).
 *
 * <h2>Mode {@code order}</h2>
 *
 * <p>Builds a {@code BatchFanout} once per {@code pinSortingOrder} string — the four the
 * comparator recognises ({@code inner_first}, {@code outer_first},
 * {@code distanceToClosestOnNet}, {@code surroundingsDensity}) plus one string it does not
 * ({@code not_a_sorting_order}) — and prints the whole of {@code sortedComponents} and, inside
 * each component, the whole of {@code smdPins}, in iteration order. Every {@code double} goes
 * through {@code Double.toString}, so the transcript is exact rather than rounded.
 *
 * <p>The {@code sortingOrder} argument is <b>ignored</b> in this mode: the point is to compare all
 * five orders in one run, so that a port that got one branch of {@code Pin.compareTo:742-777}
 * right and another wrong cannot pass.
 *
 * <h2>Mode {@code pin}</h2>
 *
 * <p>Walks {@code sortedComponents × smdPins} in the order the {@code sortingOrder} argument
 * selects and calls the <b>real</b> {@code RoutingBoard.fanout} on each pin, with
 * {@code BatchFanout.fanoutPass}' own arguments: {@code startRipupCosts * (passNo + 1)} for the
 * ripup costs (or {@code -1} when {@code fanout.ripupAllowed} is off, BatchFanout.java:179-183),
 * and {@code board.startMarkingChangedArea()} before each call (`:279`). Per pin it prints the
 * {@code AutorouteAttemptResult}, the ids the call <b>removed</b> and the geometry it
 * <b>inserted</b>, plus the running board shape.
 *
 * <p>The removed-id set is what stands in for {@code rippedItemList}: that set is a local of
 * {@code fanout} (`:1058`) and is never returned, so no probe can read it — but the items it
 * carries are deleted from the board by {@code AutorouteEngine.autorouteConnection:237-245}, and
 * a delta of {@code board.getItems()} is that deletion. It is therefore also the observable that
 * catches the two-attempt strategy of {@code :1064-1085}, which hands the <b>same</b>
 * {@code rippedItemList} to the retry.
 *
 * <h2>Mode {@code pass} (Task 12)</h2>
 *
 * <p>One {@code fanoutPass} end to end, in the two halves {@link P7T9} established:
 *
 * <ol>
 *   <li><b>{@code [transcript]}</b> — {@code fanoutPass}' body ({@code :166-506}) transcribed here,
 *       line for line, calling the real {@code RoutingBoard.fanout}; per pin it prints mode
 *       {@code pin}'s JSON <i>plus</i> the five counters, {@code pinsToGo},
 *       {@code totalItemsFanouted} and the running {@code extraViasThisPass}, so a divergence
 *       localises to a pin rather than to a pass.
 *   <li><b>{@code [real]}</b> — a freshly loaded board and the <b>real</b>
 *       {@code fanoutPass(passNo, null)}, reached with {@code Method.setAccessible(true)} because
 *       it is {@code private}. Its return value and the four instance fields it writes are
 *       printed, and its board is compared against the transcript's by {@code getHash()}.
 * </ol>
 *
 * <p>The second half is what makes the first evidence rather than a parallel implementation, and
 * it is also where the port's {@code BatchFanout::fanout_pass} is actually exercised: the port
 * widens that method to {@code pub} for exactly the reason this side needs reflection.
 *
 * <h2>Mode {@code board} (Task 12)</h2>
 *
 * <p>The whole {@code fanoutBoard} ({@code :81-163}), in the same two halves. The transcript
 * prints one line per pass — {@code routedCount}, the via count, the packed {@code boardState} of
 * {@code :133}, {@code identicalPasses}, {@code isTimedOut} and, when the loop gets that far, the
 * {@code :153-155} hash-equality <b>decision</b> — and then the stop that ended the loop. The
 * {@code [real]} half calls the public {@code BatchFanout.fanoutBoard} on a fresh board and prints
 * the {@code FanoutRunSummary} tuple minus its wall-clock component.
 *
 * <h2>{@code P7T5_HASH_MODE} — the warm-hash contract (ruling AH)</h2>
 *
 * <p>{@code warm} (the default) calls {@link #normalizeByProducts} before every {@code getHash()}
 * the transcript takes, which is {@code p7t10}'s acceptance mode: it fills {@code DrillItem}'s
 * four lazy caches and resets {@code Item.smallestClearance}, so quirk #200 cannot move a hash
 * between two boards that are the same. Task 3 measured that Java's <b>raw</b> decisions diverge
 * one-way from its own history — 3 of 350 {@code p7t10} steps are {@code FANOUTSTOP false ->
 * true} — so decision parity is defined against {@code warm}, and {@code raw} is the measurement
 * of the exposure rather than a port bug. The port has one hash and it is warm by construction
 * ({@code Board::structural_hash} skips every by-product; see the audit table in
 * {@code crates/fr-board/src/board/snapshot.rs}), so this knob moves this side only, and both
 * sides print it.
 *
 * <h2>The loop is transcribed, the routing is not</h2>
 *
 * <p>In every mode the control flow is transcribed here and everything it calls is the real
 * thing; the {@code FRLogger} payloads, the {@code BoardStatistics} progress snapshots and the
 * throttled {@code publishProgress} ticks are dropped, exactly as the port drops them. The
 * throttle is the reason no mode compares progress events at all: {@code ProgressThrottler}
 * (core/ProgressThrottler.java:15-26) is a wall clock, so how many ticks a pass publishes depends
 * on how fast the machine is.
 *
 * <h2>The budget (ruling AI)</h2>
 *
 * <p>The per-pin {@code TimeLimit} is {@code Integer.MAX_VALUE} milliseconds on both sides. Mode
 * {@code pin} passes it directly; modes {@code pass} and {@code board} cannot, because
 * {@code fanoutPass:231-232} builds its own from {@code settings.fanout.maxMillisecondsPerPin} —
 * so {@link #buildSettings} writes {@code Integer.MAX_VALUE} into <b>that setting</b>, on both
 * sides, and the multiplication by {@code passNo + 1} saturates back to it in the {@code (int)}
 * cast. {@code settings.fanout.timeout} is left {@code null}, which is what {@code
 * DefaultSettings} ships, so {@code deadlineMs} is never set and neither side has a stage clock.
 * The
 * 1000 ms {@code timeLimitToPreventEndlessLoop} inside {@code fanout} (`:1100`) is a
 * <b>local</b> {@code final int} that {@code javac} inlines, so it cannot be patched from here —
 * the port runs {@code RouterBudget::disabled()} against this side's live limit, and a MATCH
 * therefore <i>proves</i> the limit never trips on the corpus rather than assuming it. Same
 * shape as {@code P7T8Probe}'s recorded constraint.
 */
public final class P7T5 {

  /** The four strings {@code Pin.compareTo:744-771} tests, plus one it does not. */
  static final String[] SORTING_ORDERS = {
    "inner_first", "outer_first", "distanceToClosestOnNet", "surroundingsDensity",
    "not_a_sorting_order"
  };

  /** Modes that end a fanout run and therefore need the full settings ladder. */
  static final java.util.Set<String> MODES =
      java.util.Set.of("order", "pin", "pass", "board");

  private P7T5() {}

  public static void main(String[] args) throws Exception {
    if (args.length < 1) {
      System.err.println("usage: P7T5 <dsn> [passNo|maxPasses] [sortingOrder] [order|pin|pass|board]");
      System.exit(2);
    }
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    int passNo = args.length > 1 && !args[1].isBlank() ? Integer.parseInt(args[1]) : 0;
    String sortingOrder = args.length > 2 && !args[2].isBlank() ? args[2] : "outer_first";
    String mode = args.length > 3 && !args[3].isBlank() ? args[3] : "order";
    if (!MODES.contains(mode)) {
      System.err.println("P7T5: mode must be one of " + MODES + ", not '" + mode + "'");
      System.exit(2);
    }
    String hashMode = System.getenv("P7T5_HASH_MODE");
    if (hashMode == null || hashMode.isBlank()) {
      hashMode = "warm";
    }
    if (!"warm".equals(hashMode) && !"raw".equals(hashMode)) {
      System.err.println("P7T5: P7T5_HASH_MODE must be `warm` or `raw`, not '" + hashMode + "'");
      System.exit(2);
    }

    Path jar =
        Paths.get(RoutingBoard.class.getProtectionDomain().getCodeSource().getLocation().toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d fixture=%s passNo=%d sortingOrder=%s mode=%s hashMode=%s%n",
        jar,
        Files.size(jar),
        Files.getLastModifiedTime(jar).toMillis(),
        dsn.getFileName(),
        passNo,
        sortingOrder,
        mode,
        hashMode);
    System.err.println("java-version " + System.getProperty("java.version"));

    switch (mode) {
      case "order" -> {
        for (String order : SORTING_ORDERS) {
          RoutingBoard board = P7T2.loadBoard(dsn);
          RouterSettings settings = P7T2.buildSettings(board);
          settings.fanout.pinSortingOrder = order;
          dumpOrder(out, board, settings, order);
        }
      }
      case "pin" -> {
        RoutingBoard board = P7T2.loadBoard(dsn);
        RouterSettings settings = P7T2.buildSettings(board);
        settings.fanout.pinSortingOrder = sortingOrder;
        dumpFanoutRun(out, board, settings, passNo);
      }
      case "pass" -> dumpPass(out, dsn, sortingOrder, passNo, hashMode);
      default -> dumpBoardRun(out, dsn, sortingOrder, passNo, hashMode);
    }
  }

  /**
   * {@code P7T2.buildSettings} plus the two knobs modes {@code pass} and {@code board} need: the
   * per-pin budget off (ruling AI, see the class comment) and, in mode {@code board},
   * {@code maxPasses}.
   */
  static RouterSettings buildSettings(RoutingBoard board, String sortingOrder, int maxPasses) {
    RouterSettings settings = P7T2.buildSettings(board);
    settings.fanout.pinSortingOrder = sortingOrder;
    // `:175-178` — the base the `:231-232` TimeLimit is built from.
    settings.fanout.maxMillisecondsPerPin = (long) Integer.MAX_VALUE;
    if (maxPasses > 0) {
      settings.fanout.maxPasses = maxPasses;
    }
    return settings;
  }

  // -----------------------------------------------------------------------------------------
  // Mode `order` — the constructor's two containers
  // -----------------------------------------------------------------------------------------

  /**
   * Prints {@code sortedComponents × smdPins} for one {@code pinSortingOrder}.
   *
   * <p>{@code BatchFanout}'s constructor is {@code private} and {@code sortedComponents},
   * {@code Component} and {@code Component.Pin} are all {@code private} members of a
   * {@code private static} nested class, so nothing here is reachable from an ordinary caller
   * even in the same package. Reflection with {@code setAccessible(true)} is the same technique
   * {@code P7T2.reusableHandledItems} and the {@code P6T1x} probes use, and it reads the
   * <b>real</b> objects the real constructor built rather than re-deriving them.
   */
  static void dumpOrder(PrintStream out, RoutingBoard board, RouterSettings settings, String order)
      throws Exception {
    Object instance = newBatchFanout(board, settings);
    SortedSet<?> sortedComponents = (SortedSet<?>) read(instance, "sortedComponents");
    out.println(
        "[order] sortingOrder="
            + P7T2.quote(order)
            + " components="
            + sortedComponents.size()
            + " totalSmdPinCount="
            + read(instance, "totalSmdPinCount")
            + " alreadyConnectedPinCount="
            + read(instance, "alreadyConnectedPinCount"));
    int componentIndex = 0;
    for (Object component : sortedComponents) {
      app.freerouting.board.model.structure.Component boardComponent =
          (app.freerouting.board.model.structure.Component) read(component, "boardComponent");
      FloatPoint gravity = (FloatPoint) read(component, "gravityCenterOfSmdPins");
      SortedSet<?> smdPins = (SortedSet<?>) read(component, "smdPins");
      out.println(
          "COMPONENT "
              + componentIndex
              + " id="
              + boardComponent.id
              + " name="
              + P7T2.quote(boardComponent.name)
              + " smdPinCount="
              + read(component, "smdPinCount")
              + " sortedPins="
              + smdPins.size()
              + " gravity=("
              + Double.toString(gravity.x)
              + ","
              + Double.toString(gravity.y)
              + ")");
      int pinIndex = 0;
      for (Object pin : smdPins) {
        Pin boardPin = (Pin) read(pin, "boardPin");
        out.println(
            "  PIN "
                + pinIndex
                + " id="
                + boardPin.getId()
                + " pinIndex="
                + boardPin.pinIndex
                + " name="
                + P7T2.quote(boardPin.name())
                + " distToCentre="
                + Double.toString((Double) read(pin, "distanceToComponentCenter"))
                + " distToClosestOnNet="
                + Double.toString((Double) read(pin, "distanceToClosestOnNet"))
                + " surroundingsDensity="
                + read(pin, "surroundingsDensity"));
        pinIndex++;
      }
      componentIndex++;
    }
  }

  // -----------------------------------------------------------------------------------------
  // Mode `pin` — RoutingBoard.fanout, once per SMD pin, in that order
  // -----------------------------------------------------------------------------------------

  /**
   * {@code BatchFanout.fanoutPass}' two nested loops (`:220-221`) and the three lines inside them
   * that touch the board (`:231-232`, `:279`, `:281-283`), with the real
   * {@code RoutingBoard.fanout}.
   *
   * <p>The {@code canUseVias} skip of `:238-259` is transcribed too, because it decides which pins
   * are visited at all and therefore which board the next pin sees.
   */
  static void dumpFanoutRun(
      PrintStream out, RoutingBoard board, RouterSettings settings, int passNo) throws Exception {
    Object instance = newBatchFanout(board, settings);
    SortedSet<?> sortedComponents = (SortedSet<?>) read(instance, "sortedComponents");

    // `:173` and `:179-183`.
    int ripupCosts = settings.getStartRipupCosts() * (passNo + 1);
    boolean ripupAllowed =
        (settings.fanout == null || settings.fanout.ripupAllowed == null)
            || Boolean.TRUE.equals(settings.fanout.ripupAllowed);
    int effectiveRipupCosts = ripupAllowed ? ripupCosts : -1;
    out.println(
        "[pin] ripupCosts="
            + effectiveRipupCosts
            + " fallbackToBoardVias="
            + settings.fanout.fallbackToBoardVias
            + " "
            + P7T2.boardShape(board));

    int index = 0;
    for (Object component : sortedComponents) {
      app.freerouting.board.model.structure.Component boardComponent =
          (app.freerouting.board.model.structure.Component) read(component, "boardComponent");
      SortedSet<?> smdPins = (SortedSet<?>) read(component, "smdPins");
      for (Object pin : smdPins) {
        Pin boardPin = (Pin) read(pin, "boardPin");
        String fullPinName = boardComponent.name + "-" + boardPin.name();
        int netNumber = boardPin.getNetNumber(0);

        // `:238-259` — the "no vias and no fallback" skip.
        app.freerouting.rules.Net net = board.rules.nets.get(netNumber);
        if (net != null) {
          app.freerouting.rules.NetClass netClass = net.getNetClass();
          ViaRule viaRule = netClass != null ? netClass.getViaRule() : null;
          boolean hasBoardVias =
              !board.rules.viaRules.isEmpty() && board.rules.viaRules.firstElement().viaCount() > 0;
          boolean fallbackAllowed =
              settings.fanout != null
                  && Boolean.TRUE.equals(settings.fanout.fallbackToBoardVias)
                  && hasBoardVias;
          boolean canUseVias = (viaRule != null && viaRule.viaCount() > 0) || fallbackAllowed;
          if (!canUseVias) {
            out.println("SKIP " + index + " pin=" + P7T2.quote(fullPinName) + " net=" + netNumber);
            index++;
            continue;
          }
        }

        List<Integer> before = itemIds(board);
        int maxIdBefore = board.communication.idGenerator.maxGeneratedId();
        board.startMarkingChangedArea(); // `:279`
        AutorouteAttemptResult result =
            board.fanout(
                boardPin,
                settings,
                effectiveRipupCosts,
                new P7T2.NeverStarted(),
                new TimeLimit(Integer.MAX_VALUE));
        List<Integer> after = itemIds(board);

        StringBuilder sb = new StringBuilder();
        sb.append("{\"k\":").append(index);
        sb.append(",\"pin\":").append(boardPin.getId());
        sb.append(",\"name\":").append(P7T2.quote(fullPinName));
        sb.append(",\"net\":").append(netNumber);
        sb.append(",\"state\":\"").append(result.state).append('"');
        sb.append(",\"details\":").append(P7T2.quote(result.details));
        sb.append(",\"removed\":").append(removed(before, after));
        sb.append(",\"maxIdBefore\":").append(maxIdBefore);
        sb.append(",\"maxIdAfter\":").append(board.communication.idGenerator.maxGeneratedId());
        P7T2.appendInsertedGeometry(sb, board, maxIdBefore);
        out.println(sb.append('}'));
        index++;
      }
    }
    out.println("[final] " + P7T2.boardShape(board));
  }

  // -----------------------------------------------------------------------------------------
  // Reflection and rendering helpers
  // -----------------------------------------------------------------------------------------

  /** {@code new BatchFanout(board, settings, thread)} — the {@code private} constructor `:35`. */
  static BatchFanout newBatchFanout(RoutingBoard board, RouterSettings settings) throws Exception {
    Constructor<BatchFanout> ctor =
        BatchFanout.class.getDeclaredConstructor(
            RoutingBoard.class, RouterSettings.class, app.freerouting.core.StoppableThread.class);
    ctor.setAccessible(true);
    return ctor.newInstance(board, settings, new P7T2.NeverStarted());
  }

  /** {@code fanoutPass(int, FanoutProgressListener)} — the {@code private} method `:166`. */
  static Method fanoutPassMethod() throws Exception {
    Method method =
        BatchFanout.class.getDeclaredMethod(
            "fanoutPass", int.class, BatchFanout.FanoutProgressListener.class);
    method.setAccessible(true);
    return method;
  }

  /** One declared field of {@code target}'s own class, however private, written. */
  static void write(Object target, String fieldName, Object value) throws Exception {
    Field field = target.getClass().getDeclaredField(fieldName);
    field.setAccessible(true);
    field.set(target, value);
  }

  /**
   * {@code board.getHash()}, with {@link #normalizeByProducts} first in {@code warm} mode. See
   * the class comment: the port's hash is warm by construction, so this is the mode decision
   * parity is defined against (controller ruling AH, and the Task 3 caveat).
   */
  static String hashOf(RoutingBoard board, String hashMode) {
    if ("warm".equals(hashMode)) {
      normalizeByProducts(board);
    }
    return board.getHash();
  }

  /**
   * {@code P7T10.normalizeByProducts}, copied rather than shared because {@code P7T10} is in a
   * different package and this driver is compiled from the jar. Fills {@code DrillItem}'s four
   * lazy caches (idempotent) and resets the one accumulator, {@code Item.smallestClearance}
   * (Item.java:47), which {@code Item.clearanceViolations} only ever lowers. Quirk #200.
   */
  static void normalizeByProducts(RoutingBoard board) {
    for (Item item : board.getItems()) {
      item.smallestClearance = -1.0;
      if (item instanceof DrillItem drill) {
        drill.getCenter();
        drill.firstLayer();
        drill.lastLayer();
        drill.smallestRadius();
      }
    }
  }

  /** {@code EscapeStatistics.fromBoardStatistics(new BoardStatistics(board, null, false))}. */
  static String escapeLine(RoutingBoard board) {
    BoardStatistics stats = new BoardStatistics(board, null, false);
    BatchFanout.EscapeStatistics escape = BatchFanout.EscapeStatistics.fromBoardStatistics(stats);
    return "totalSmdPins="
        + escape.totalSmdPins()
        + " escapedCount="
        + escape.escapedCount()
        + " escapedPercentage="
        + Double.toString(escape.escapedPercentage())
        + " pinsToEscape="
        + stats.fanout.pinsToEscape;
  }

  /** {@code fanoutPass:173} and {@code :179-183}. */
  static int effectiveRipupCosts(RouterSettings settings, int passNo) {
    int ripupCosts = settings.getStartRipupCosts() * (passNo + 1);
    boolean ripupAllowed =
        (settings.fanout == null || settings.fanout.ripupAllowed == null)
            || Boolean.TRUE.equals(settings.fanout.ripupAllowed);
    return ripupAllowed ? ripupCosts : -1;
  }

  // -----------------------------------------------------------------------------------------
  // Mode `pass` — fanoutPass, transcribed and then real
  // -----------------------------------------------------------------------------------------

  /** See the class comment's "Mode {@code pass}". */
  static void dumpPass(PrintStream out, Path dsn, String sortingOrder, int passNo, String hashMode)
      throws Exception {
    RoutingBoard board = P7T2.loadBoard(dsn);
    RouterSettings settings = buildSettings(board, sortingOrder, 0);
    BatchFanout instance = newBatchFanout(board, settings);
    out.println(
        "[pass] passNo="
            + passNo
            + " ripupCosts="
            + effectiveRipupCosts(settings, passNo)
            + " maxMillisecondsPerPin="
            + settings.fanout.maxMillisecondsPerPin
            + " totalSmdPinCount="
            + read(instance, "totalSmdPinCount")
            + " alreadyConnectedPinCount="
            + read(instance, "alreadyConnectedPinCount")
            + " "
            + P7T2.boardShape(board));

    out.println("[transcript]");
    int routedCount = transcribePass(out, instance, board, settings, passNo);
    out.println("TRANSCRIPT routed=" + routedCount + " " + P7T2.boardShape(board));
    out.println("ESCAPE " + escapeLine(board));
    String transcriptHash = hashOf(board, hashMode);

    RoutingBoard realBoard = P7T2.loadBoard(dsn);
    RouterSettings realSettings = buildSettings(realBoard, sortingOrder, 0);
    BatchFanout realInstance = newBatchFanout(realBoard, realSettings);
    out.println("[real]");
    int realRouted = (Integer) fanoutPassMethod().invoke(realInstance, passNo, null);
    out.println(
        "REAL routed="
            + realRouted
            + " totalItemsFanouted="
            + realInstance.totalItemsFanouted
            + " extraViasTotal="
            + read(realInstance, "extraViasTotal")
            + " lastNotRoutedCount="
            + read(realInstance, "lastNotRoutedCount")
            + " isTimedOut="
            + read(realInstance, "isTimedOut")
            + " "
            + P7T2.boardShape(realBoard));
    out.println("ESCAPE " + escapeLine(realBoard));
    out.println("EQUALS-TRANSCRIPT " + hashOf(realBoard, hashMode).equals(transcriptHash));
  }

  /**
   * {@code fanoutPass}' body ({@code :166-506}) transcribed, with the real
   * {@code RoutingBoard.fanout} and none of the bookkeeping the port also drops — the eight
   * {@code FRLogger.trace} payloads, the three {@code BoardStatistics} snapshots and the two
   * progress publishers, whose throttle is a wall clock.
   */
  static int transcribePass(
      PrintStream out,
      BatchFanout instance,
      RoutingBoard board,
      RouterSettings settings,
      int passNo)
      throws Exception {
    // `:168-172`.
    int pinsToGo = (Integer) read(instance, "totalSmdPinCount");
    int routedCount = 0;
    int notRoutedCount = 0;
    int insertErrorCount = 0;
    int alreadyConnectedCount = 0;
    final int viasBeforePass = board.getVias().size();
    // `:173`, `:175-183`.
    int ripupCosts = settings.getStartRipupCosts() * (passNo + 1);
    long baseMillisPerPin =
        settings.fanout != null && settings.fanout.maxMillisecondsPerPin != null
            ? settings.fanout.maxMillisecondsPerPin
            : 10000L;
    int effectiveRipupCosts = effectiveRipupCosts(settings, passNo);

    SortedSet<?> sortedComponents = (SortedSet<?>) read(instance, "sortedComponents");
    boolean maxLimitReached = false;
    int index = 0;
    // `:220-221`.
    for (Object component : sortedComponents) {
      app.freerouting.board.model.structure.Component boardComponent =
          (app.freerouting.board.model.structure.Component) read(component, "boardComponent");
      SortedSet<?> smdPins = (SortedSet<?>) read(component, "smdPins");
      for (Object pin : smdPins) {
        // `:222-230`.
        if (settings.fanout != null
            && settings.fanout.maxItems != null
            && settings.fanout.maxItems > 0
            && instance.totalItemsFanouted >= settings.fanout.maxItems) {
          out.println(
              "MAXITEMS k=" + index + " totalItemsFanouted=" + instance.totalItemsFanouted);
          maxLimitReached = true;
          break;
        }
        // `:231-232`.
        double maxMilliseconds = baseMillisPerPin * (passNo + 1);
        final TimeLimit timeLimit = new TimeLimit((int) maxMilliseconds);
        Pin boardPin = (Pin) read(pin, "boardPin");
        // `:233-236`.
        String fullPinName = boardComponent.name + "-" + boardPin.name();
        int netNumber = boardPin.getNetNumber(0);
        int targetCount = boardPin.getUnconnectedSet(netNumber).size();

        // `:238-259` — the "no vias and no fallback" skip.
        app.freerouting.rules.Net net = board.rules.nets.get(netNumber);
        if (net != null) {
          app.freerouting.rules.NetClass netClass = net.getNetClass();
          ViaRule viaRule = netClass != null ? netClass.getViaRule() : null;
          boolean hasBoardVias =
              !board.rules.viaRules.isEmpty() && board.rules.viaRules.firstElement().viaCount() > 0;
          boolean fallbackAllowed =
              settings.fanout != null
                  && Boolean.TRUE.equals(settings.fanout.fallbackToBoardVias)
                  && hasBoardVias;
          boolean canUseVias = (viaRule != null && viaRule.viaCount() > 0) || fallbackAllowed;
          if (!canUseVias) {
            --pinsToGo;
            out.println(
                "SKIP "
                    + index
                    + " pin="
                    + P7T2.quote(fullPinName)
                    + " net="
                    + netNumber
                    + " pinsToGo="
                    + pinsToGo);
            index++;
            continue;
          }
        }

        List<Integer> before = itemIds(board);
        int maxIdBefore = board.communication.idGenerator.maxGeneratedId();
        board.startMarkingChangedArea(); // `:279`
        AutorouteAttemptResult result =
            board.fanout(
                boardPin, settings, effectiveRipupCosts, new P7T2.NeverStarted(), timeLimit);
        List<Integer> after = itemIds(board);

        // `:286-381` — the five-way switch.
        switch (result.state) {
          case ROUTED -> {
            ++routedCount;
            instance.totalItemsFanouted++;
          }
          case ALREADY_CONNECTED -> ++alreadyConnectedCount;
          case FAILED -> {
            ++notRoutedCount;
            instance.totalItemsFanouted++;
          }
          case INSERT_ERROR -> {
            ++insertErrorCount;
            instance.totalItemsFanouted++;
          }
          default -> {}
        }
        --pinsToGo; // `:382`
        int extraViasThisPass = Math.max(0, board.getVias().size() - viasBeforePass); // `:383`

        StringBuilder sb = new StringBuilder();
        sb.append("{\"k\":").append(index);
        sb.append(",\"pin\":").append(boardPin.getId());
        sb.append(",\"name\":").append(P7T2.quote(fullPinName));
        sb.append(",\"net\":").append(netNumber);
        sb.append(",\"targets\":").append(targetCount);
        sb.append(",\"state\":\"").append(result.state).append('"');
        sb.append(",\"details\":").append(P7T2.quote(result.details));
        sb.append(",\"removed\":").append(removed(before, after));
        sb.append(",\"maxIdBefore\":").append(maxIdBefore);
        sb.append(",\"maxIdAfter\":").append(board.communication.idGenerator.maxGeneratedId());
        sb.append(",\"routed\":").append(routedCount);
        sb.append(",\"notRouted\":").append(notRoutedCount);
        sb.append(",\"insertErrors\":").append(insertErrorCount);
        sb.append(",\"alreadyConnected\":").append(alreadyConnectedCount);
        sb.append(",\"pinsToGo\":").append(pinsToGo);
        sb.append(",\"totalItemsFanouted\":").append(instance.totalItemsFanouted);
        sb.append(",\"extraVias\":").append(extraViasThisPass);
        P7T2.appendInsertedGeometry(sb, board, maxIdBefore);
        out.println(sb.append('}'));
        index++;
      }
      if (maxLimitReached) { // `:435-437`
        break;
      }
    }
    // `:439`.
    int extraViasThisPass = Math.max(0, board.getVias().size() - viasBeforePass);
    out.println(
        "PASS-END routed="
            + routedCount
            + " notRouted="
            + notRoutedCount
            + " insertErrors="
            + insertErrorCount
            + " alreadyConnected="
            + alreadyConnectedCount
            + " pinsToGo="
            + pinsToGo
            + " totalItemsFanouted="
            + instance.totalItemsFanouted
            + " extraViasThisPass="
            + extraViasThisPass
            + " ripupCosts="
            + ripupCosts);
    return routedCount;
  }

  // -----------------------------------------------------------------------------------------
  // Mode `board` — fanoutBoard, transcribed and then real
  // -----------------------------------------------------------------------------------------

  /** See the class comment's "Mode {@code board}". */
  static void dumpBoardRun(
      PrintStream out, Path dsn, String sortingOrder, int maxPassesArg, String hashMode)
      throws Exception {
    RoutingBoard board = P7T2.loadBoard(dsn);
    RouterSettings settings = buildSettings(board, sortingOrder, maxPassesArg);
    out.println(
        "[board-run] maxPasses="
            + settings.fanout.maxPasses
            + " maxItems="
            + settings.fanout.maxItems
            + " timeout="
            + settings.fanout.timeoutString
            + " "
            + P7T2.boardShape(board));

    out.println("[transcript]");
    BatchFanout instance = newBatchFanout(board, settings);
    Method fanoutPass = fanoutPassMethod();
    // `:101-109`.
    int maxPasses =
        settings.fanout != null && settings.fanout.maxPasses != null
            ? settings.fanout.maxPasses
            : 20;
    final int stagnationPassLimit = 3;
    int completedPasses = 0;
    long previousBoardState = Long.MIN_VALUE;
    int identicalPasses = 0;
    String lastBoardHash = hashOf(board, hashMode);
    String stop = "MAXPASSES";
    // `:110-157`.
    for (int i = 0; i < maxPasses; ++i) {
      // `:111-116` — unreachable here, `timeoutString` is null on both sides.
      Long deadlineMs = (Long) read(instance, "deadlineMs");
      if (deadlineMs != null && System.currentTimeMillis() >= deadlineMs) {
        write(instance, "isTimedOut", Boolean.TRUE);
        stop = "DEADLINE";
        break;
      }
      // `:117-122`.
      if (settings.fanout != null
          && settings.fanout.maxItems != null
          && settings.fanout.maxItems > 0
          && instance.totalItemsFanouted >= settings.fanout.maxItems) {
        stop = "MAXITEMS";
        break;
      }
      // `:123-124`.
      int routedCount = (Integer) fanoutPass.invoke(instance, i, null);
      completedPasses++;
      int viaCount = board.getVias().size();
      long boardState = ((long) routedCount << 32) ^ viaCount; // `:133`
      boolean timedOut = (Boolean) read(instance, "isTimedOut");
      out.println(
          "PASS "
              + i
              + " routed="
              + routedCount
              + " vias="
              + viaCount
              + " boardState="
              + boardState
              + " isTimedOut="
              + timedOut
              + " totalItemsFanouted="
              + instance.totalItemsFanouted
              + " extraViasTotal="
              + read(instance, "extraViasTotal")
              + " lastNotRouted="
              + read(instance, "lastNotRoutedCount"));
      // `:125-127`.
      if (routedCount == 0) {
        stop = "NOTHING-ROUTED";
        break;
      }
      // `:134-147`.
      if (boardState == previousBoardState) {
        identicalPasses++;
        out.println("STAGNATION pass=" + i + " identicalPasses=" + identicalPasses);
        if (identicalPasses >= stagnationPassLimit) {
          stop = "STAGNATED";
          break;
        }
      } else {
        identicalPasses = 0;
        previousBoardState = boardState;
      }
      // `:149-151`.
      if (timedOut) {
        stop = "TIMED-OUT";
        break;
      }
      // `:152-156` — the hash-equality **decision**, never the value (ruling AH).
      String currentBoardHash = hashOf(board, hashMode);
      boolean hashEqual = currentBoardHash.equals(lastBoardHash);
      out.println("HASHEQ pass=" + i + " equal=" + hashEqual);
      if (hashEqual) {
        stop = "UNCHANGED-HASH";
        break;
      }
      lastBoardHash = currentBoardHash;
    }
    out.println(
        "STOP "
            + stop
            + " completedPasses="
            + completedPasses
            + " isTimedOut="
            + read(instance, "isTimedOut")
            + " totalItemsFanouted="
            + instance.totalItemsFanouted);
    out.println("TRANSCRIPT " + P7T2.boardShape(board));
    out.println("ESCAPE " + escapeLine(board));
    String transcriptHash = hashOf(board, hashMode);

    // `:81-83` — the public three-argument overload, on a freshly loaded board.
    RoutingBoard realBoard = P7T2.loadBoard(dsn);
    RouterSettings realSettings = buildSettings(realBoard, sortingOrder, maxPassesArg);
    out.println("[real]");
    BatchFanout.FanoutRunSummary summary =
        BatchFanout.fanoutBoard(realBoard, realSettings, new P7T2.NeverStarted());
    BatchFanout.EscapeStatistics escape = summary.escapeStatistics();
    out.println(
        "REAL completedPassCount="
            + summary.completedPassCount()
            + " isTimedOut="
            + summary.isTimedOut()
            + " escapeTotalSmdPins="
            + escape.totalSmdPins()
            + " escapedCount="
            + escape.escapedCount()
            + " escapedPercentage="
            + Double.toString(escape.escapedPercentage())
            + " "
            + P7T2.boardShape(realBoard));
    out.println("ESCAPE " + escapeLine(realBoard));
    out.println("EQUALS-TRANSCRIPT " + hashOf(realBoard, hashMode).equals(transcriptHash));

    out.println("[board]");
    P7T9.dumpBoard(out, board);
  }

  /** One declared field of {@code target}'s own class, however private. */
  static Object read(Object target, String fieldName) throws Exception {
    Field field = target.getClass().getDeclaredField(fieldName);
    field.setAccessible(true);
    return field.get(target);
  }

  /** Every live item id, ascending — the set a fanout call can shrink. */
  static List<Integer> itemIds(RoutingBoard board) {
    SortedSet<Integer> ids = new TreeSet<>();
    for (Item item : board.getItems()) {
      ids.add(item.getId());
    }
    return new ArrayList<>(ids);
  }

  /** The ids in {@code before} that {@code after} no longer has, ascending. */
  static String removed(List<Integer> before, List<Integer> after) {
    StringBuilder sb = new StringBuilder("[");
    boolean first = true;
    for (Integer id : before) {
      if (after.contains(id)) {
        continue;
      }
      if (!first) {
        sb.append(',');
      }
      first = false;
      sb.append(id);
    }
    return sb.append(']').toString();
  }
}
