package app.freerouting.autoroute.pipeline;

import app.freerouting.autoroute.AutorouteAttemptResult;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
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
 * {@code pin}.
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
 * <h2>The loop is transcribed, the routing is not</h2>
 *
 * <p>{@code fanoutPass} (`:166-...`) is <b>Task 12's</b>, so this driver does not call it: it
 * transcribes the two nested {@code for}s and the three lines inside them that reach the board,
 * and drops the pass bookkeeping, the progress publishing and the {@code FRLogger} payloads. What
 * it calls for real is the method this task ports — {@code RoutingBoard.fanout} — and the
 * constructor that builds the order it walks.
 *
 * <h2>The budget (ruling AI)</h2>
 *
 * <p>The per-pin {@code TimeLimit} is {@code Integer.MAX_VALUE} milliseconds on both sides rather
 * than {@code baseMillisPerPin * (passNo + 1)}, so neither side can stop on a wall clock. The
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

  private P7T5() {}

  public static void main(String[] args) throws Exception {
    if (args.length < 1) {
      System.err.println("usage: P7T5 <dsn> [passNo] [sortingOrder] [order|pin]");
      System.exit(2);
    }
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    int passNo = args.length > 1 && !args[1].isBlank() ? Integer.parseInt(args[1]) : 0;
    String sortingOrder = args.length > 2 && !args[2].isBlank() ? args[2] : "outer_first";
    String mode = args.length > 3 && !args[3].isBlank() ? args[3] : "order";
    if (!"order".equals(mode) && !"pin".equals(mode)) {
      System.err.println("P7T5: mode must be `order` or `pin`, not '" + mode + "'");
      System.exit(2);
    }

    Path jar =
        Paths.get(RoutingBoard.class.getProtectionDomain().getCodeSource().getLocation().toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d fixture=%s passNo=%d sortingOrder=%s mode=%s%n",
        jar,
        Files.size(jar),
        Files.getLastModifiedTime(jar).toMillis(),
        dsn.getFileName(),
        passNo,
        sortingOrder,
        mode);
    System.err.println("java-version " + System.getProperty("java.version"));

    if ("order".equals(mode)) {
      for (String order : SORTING_ORDERS) {
        RoutingBoard board = P7T2.loadBoard(dsn);
        RouterSettings settings = P7T2.buildSettings(board);
        settings.fanout.pinSortingOrder = order;
        dumpOrder(out, board, settings, order);
      }
    } else {
      RoutingBoard board = P7T2.loadBoard(dsn);
      RouterSettings settings = P7T2.buildSettings(board);
      settings.fanout.pinSortingOrder = sortingOrder;
      dumpFanoutRun(out, board, settings, passNo);
    }
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
  static Object newBatchFanout(RoutingBoard board, RouterSettings settings) throws Exception {
    Constructor<?> ctor =
        BatchFanout.class.getDeclaredConstructor(
            RoutingBoard.class, RouterSettings.class, app.freerouting.core.StoppableThread.class);
    ctor.setAccessible(true);
    return ctor.newInstance(board, settings, new P7T2.NeverStarted());
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
