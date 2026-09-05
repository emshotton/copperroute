package app.freerouting.autoroute.pipeline;

import app.freerouting.autoroute.AutorouteAttemptResult;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Connectable;
import app.freerouting.board.model.items.ConductionArea;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Via;
import app.freerouting.board.trace.PolylineTrace;
import app.freerouting.core.RoutingJob;
import app.freerouting.core.StoppableThread;
import app.freerouting.core.library.Padstack;
import app.freerouting.drc.DesignRulesChecker;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;
import app.freerouting.rules.Net;
import app.freerouting.settings.RouterSettings;
import app.freerouting.settings.sources.DefaultSettings;
import java.io.FileDescriptor;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.SortedSet;
import java.util.TreeSet;

/**
 * Plan 7 Task 9 differential driver, pass level: one whole autoroute pass —
 * {@code AutoroutePassRunner.runSingleThread} (AutoroutePassRunner.java:151-336) — over a real DSN
 * board, against the port's {@code AutoroutePassRunner::run_single_thread}.
 *
 * <p>Usage: {@code P7T2 <dsn> [passNo] [maxItems]}. {@code passNo} defaults to {@code 1};
 * {@code maxItems} is an integer or the token {@code all}, which is {@code settings.maxItems =
 * null} — the CLI's default and the value that leaves {@code :213}'s guard false forever.
 *
 * <p>This class also owns the four helpers {@link P7T1} shares, so the two drivers cannot describe
 * different boards or different routers.
 *
 * <h2>The shape of the run, and why there are two halves</h2>
 *
 * <p>{@code runSingleThread} is a single 180-line method with no seam a driver can print from: it
 * loops over items, calls {@code router.autorouteItem} and returns one {@code boolean}. Driving it
 * directly would give a one-line transcript, which localises nothing on a 294-connection board. So
 * the driver does both:
 *
 * <ol>
 *   <li><b>{@code [transcript]}</b> — the method's loop <b>transcribed</b> here, line for line
 *       against {@code :158-330}, calling the <i>real</i> {@code getAutorouteItems},
 *       {@code autorouteItem} and {@code removeTails} and printing {@code p6t1}'s JSON line per
 *       {@code (item, net index)} plus the tail-removal deltas. This is the same technique
 *       {@code P6T1.route} uses for {@code AutorouteConnectionRouter.route}'s steps 1-5.
 *   <li><b>{@code [real]}</b> — a <b>freshly loaded board</b>, the same {@code passNo - 1} warm-up
 *       passes, and then the real {@code BatchAutorouter.autoroutePass(passNo)}, i.e.
 *       {@code runSingleThread} itself. Its return value and its board are printed, and the board
 *       is compared against the transcript's by {@code BasicBoard.getHash()}.
 * </ol>
 *
 * <p>The second half is what makes the first half evidence rather than a parallel implementation:
 * if the transcription ever drifted from the method, {@code equalsTranscript} would go false on
 * the Java side alone and the run would diff against a port that had not changed. The port's twin
 * runs the same two halves, comparing with {@code Board::structural_hash} — an
 * <b>equality decision</b>, never a hash value, which is the only thing comparable across the two
 * languages (controller ruling AH).
 *
 * <h2>The router</h2>
 *
 * <p>{@code new BatchAutorouter(RoutingJob)} (BatchAutorouter.java:110-122) — the production
 * constructor, not {@code P7T8Probe}'s seven-argument one. {@code runSingleThread} dereferences
 * {@code router.job} on four paths ({@code :214}, {@code :271}, {@code :274}, {@code :332}) and
 * {@code router.thread} on three ({@code :203}, {@code :208}, {@code :219}), so both must be real:
 * a {@code null} {@code job} would turn the first failed item into a {@code NullPointerException}
 * that {@code :331}'s own catch would swallow, and the pass would silently return {@code false}.
 * The {@code StoppableThread} is {@link NeverStarted}, which is never {@code start()}ed — it is a
 * flag holder, and {@code requestStop} at {@code :219} is the only thing that writes it.
 *
 * <h2>The budget</h2>
 *
 * <p>Ruling AI asks parity runs to disable the wall clock on both sides. As {@code P7T8Probe}'s
 * class comment records, the Java side cannot: {@code TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP} is a
 * compile-time constant that {@code javac} inlines at both call sites. The port runs with
 * {@code RouterBudget::disabled()} against this side's live 1000 ms limit, so a MATCH proves the
 * limit never trips on the corpus rather than assuming it.
 */
public final class P7T2 {

  private P7T2() {}

  /**
   * A {@link StoppableThread} that is never started. {@code runSingleThread} only ever calls
   * {@code isStopAutoRouterRequested()} and {@code requestStop()} on it, both of which are plain
   * synchronized field accesses on {@code StoppableThread} itself (StoppableThread.java:20-42) and
   * need no running thread.
   */
  static final class NeverStarted extends StoppableThread {
    @Override
    protected void threadAction() {
      // Never run: this object is a flag holder, not a task.
    }
  }

  public static void main(String[] args) throws Exception {
    if (args.length < 1) {
      System.err.println("usage: P7T2 <dsn> [passNo] [maxItems|all]");
      System.exit(2);
    }
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    int passNo = args.length > 1 ? Integer.parseInt(args[1]) : 1;
    String maxItemsArg = args.length > 2 && !args[2].isBlank() ? args[2] : "all";
    Integer maxItems = "all".equals(maxItemsArg) ? null : Integer.valueOf(maxItemsArg);

    Path jar =
        Paths.get(RoutingBoard.class.getProtectionDomain().getCodeSource().getLocation().toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d fixture=%s passNo=%d maxItems=%s%n",
        jar,
        Files.size(jar),
        Files.getLastModifiedTime(jar).toMillis(),
        dsn.getFileName(),
        passNo,
        maxItemsArg);
    System.err.println("java-version " + System.getProperty("java.version"));

    // ---- half one: the transcription -----------------------------------------------------
    RoutingBoard board = loadBoard(dsn);
    RouterSettings settings = buildSettings(board);
    settings.maxItems = maxItems;
    BatchAutorouter router = newRouter(board, settings);
    for (int previous = 1; previous < passNo; previous++) {
      runPass(router, previous);
    }
    out.println("[transcript]");
    dumpAutorouteItems(out, router, board);
    boolean transcriptReturned = transcribeRunSingleThread(out, router, board, passNo);
    out.println(
        "TRANSCRIPT returned="
            + transcriptReturned
            + " "
            + boardShape(board)
            + " totalItemsRouted="
            + router.totalItemsRouted);
    String transcriptHash = board.getHash();

    // ---- half two: the real method -------------------------------------------------------
    RoutingBoard realBoard = loadBoard(dsn);
    RouterSettings realSettings = buildSettings(realBoard);
    realSettings.maxItems = maxItems;
    BatchAutorouter realRouter = newRouter(realBoard, realSettings);
    for (int previous = 1; previous < passNo; previous++) {
      runPass(realRouter, previous);
    }
    out.println("[real]");
    boolean realReturned = realRouter.autoroutePass(passNo);
    out.println(
        "REAL returned="
            + realReturned
            + " "
            + boardShape(realBoard)
            + " totalItemsRouted="
            + realRouter.totalItemsRouted
            + " equalsTranscript="
            + realBoard.getHash().equals(transcriptHash));
  }

  // -----------------------------------------------------------------------------------------
  // Shared helpers — P7T1 uses all four
  // -----------------------------------------------------------------------------------------

  /** {@code P6T1.loadBoard} without the optional {@code .rules} file, which neither driver uses. */
  static RoutingBoard loadBoard(Path dsn) throws Exception {
    BoardReadResult result;
    String designName = dsn.getFileName().toString();
    try (FileInputStream in = new FileInputStream(dsn.toFile())) {
      result = DsnReader.readBoard(in, null, null, designName);
    }
    return switch (result) {
      case BoardReadResult.Success s -> (RoutingBoard) s.board();
      case BoardReadResult.OutlineMissing o -> (RoutingBoard) o.board();
      default -> throw new IllegalStateException("board did not read: " + result);
    };
  }

  /** {@code P6T1.main}'s three settings lines — the priority-0 source of the headless ladder. */
  static RouterSettings buildSettings(RoutingBoard board) {
    RouterSettings settings = new DefaultSettings().getSettings();
    settings.setLayerCount(board.getLayerCount());
    settings.applyBoardSpecificOptimizations(board);
    return settings;
  }

  /**
   * {@code new BatchAutorouter(RoutingJob)} (BatchAutorouter.java:110-122) — the production
   * constructor. See the class comment for why {@code job} and {@code thread} must both be real.
   */
  static BatchAutorouter newRouter(RoutingBoard board, RouterSettings settings) {
    RoutingJob job = new RoutingJob();
    job.board = board;
    job.routerSettings = settings;
    job.thread = new NeverStarted();
    return new BatchAutorouter(job);
  }

  /** {@code BatchAutorouter.autoroutePass} (`:415-421`), i.e. the real `runSingleThread`. */
  static boolean runPass(BatchAutorouter router, int passNo) {
    return router.autoroutePass(passNo);
  }

  // -----------------------------------------------------------------------------------------
  // getAutorouteItems — the `p7t1` payload, shared with P7T1
  // -----------------------------------------------------------------------------------------

  /**
   * {@code getAutorouteItems}'s answer plus its hidden state, printed before any routing. See
   * {@link P7T1}'s class comment for the format and for why {@code handledItems} is traced.
   */
  static void dumpAutorouteItems(PrintStream out, BatchAutorouter router, RoutingBoard board)
      throws Exception {
    List<Item> items = router.getAutorouteItems(board);
    out.println("COUNT " + items.size());
    for (int ordinal = 0; ordinal < items.size(); ordinal++) {
      Item item = items.get(ordinal);
      StringBuilder sb = new StringBuilder();
      sb.append("ITEM ")
          .append(ordinal)
          .append(' ')
          .append(item.getId())
          .append(' ')
          .append(item.getClass().getSimpleName())
          .append(" nets=")
          .append(item.netCount())
          .append(" [");
      for (int i = 0; i < item.netCount(); i++) {
        if (i > 0) {
          sb.append(',');
        }
        sb.append(item.getNetNumber(i));
      }
      out.println(sb.append(']'));
    }

    // The per-step `handledItems` trace, rebuilt from the method's own rule (`:363-370`). The
    // walk below is `:351-360` verbatim; `HANDLED-FINAL` checks it against the real field.
    SortedSet<Integer> handled = new TreeSet<>();
    int step = 0;
    java.util.Iterator<app.freerouting.datastructures.UndoableObjects.UndoableObjectNode> it =
        board.itemList.startReadObject();
    for (; ; ) {
      app.freerouting.datastructures.UndoableObjects.Storable currentObject =
          board.itemList.readObject(it);
      if (currentObject == null) {
        break;
      }
      if (!(currentObject instanceof Connectable) || !(currentObject instanceof Item currentItem)) {
        continue;
      }
      if (currentItem.isRoutable()) {
        continue;
      }
      // `:359` reads the set as it stands *now*, which is why this walk must mirror the method's
      // order exactly rather than recomputing per item.
      if (handledContains(handled, currentItem)) {
        continue;
      }
      for (int i = 0; i < currentItem.netCount(); i++) {
        Set<Item> connectedSet = currentItem.getConnectedSet(currentItem.getNetNumber(i));
        for (Item connected : connectedSet) {
          if (connected.netCount() <= 1) {
            handled.add(connected.getId());
          }
        }
        out.println("HANDLED " + step + " " + handled.size() + " " + idList(handled));
        step++;
      }
    }

    // The real field, read after the call. `reusableHandledItems` is cleared at `:348` and
    // rebuilt by the call, so its contents now are exactly the set the call finished with.
    Field field = BatchAutorouter.class.getDeclaredField("reusableHandledItems");
    field.setAccessible(true);
    @SuppressWarnings("unchecked")
    Set<Item> real = (Set<Item>) field.get(router);
    SortedSet<Integer> realIds = new TreeSet<>();
    for (Item item : real) {
      realIds.add(item.getId());
    }
    out.println(
        "HANDLED-FINAL "
            + realIds.size()
            + " "
            + idList(realIds)
            + " transcriptionAgrees="
            + realIds.equals(handled));
  }

  private static boolean handledContains(SortedSet<Integer> handled, Item item) {
    return handled.contains(item.getId());
  }

  private static String idList(Collection<Integer> ids) {
    StringBuilder sb = new StringBuilder("[");
    boolean first = true;
    for (int id : ids) {
      if (!first) {
        sb.append(',');
      }
      first = false;
      sb.append(id);
    }
    return sb.append(']').toString();
  }

  // -----------------------------------------------------------------------------------------
  // runSingleThread — transcribed, AutoroutePassRunner.java:158-330
  // -----------------------------------------------------------------------------------------

  /**
   * {@code runSingleThread}'s body, transcribed line for line with a JSON line printed per
   * {@code (item, net index)}. Everything it calls is the real thing; only the loop is here.
   *
   * <p>The five {@code log*} helpers, the nine {@code profile*} counters and the three
   * {@code fireBoardUpdatedEvent} calls are omitted: the first two are {@code FRLogger} payloads
   * and the third fires into a listener list that this driver leaves empty, exactly as the
   * headless CLI does.
   */
  static boolean transcribeRunSingleThread(
      PrintStream out, BatchAutorouter router, RoutingBoard board, int passNo) {
    // :158.
    List<Item> autorouteItemList = router.getAutorouteItems(board);
    // :163-166.
    if (autorouteItemList.isEmpty()) {
      out.println("EMPTY");
      return false;
    }

    int notRouted = 0;
    int routed = 0;
    int skipped = 0;
    int rippedItemCount = 0;
    int itemsToGoCount = autorouteItemList.size();
    int k = 0;

    // :202.
    for (Item currentItem : autorouteItemList) {
      // :203-205.
      if (router.thread.isStopAutoRouterRequested()) {
        break;
      }
      // :207.
      for (int i = 0; i < currentItem.netCount(); i++) {
        // :208-210.
        if (router.thread.isStopAutoRouterRequested()) {
          break;
        }
        // :212-221.
        if (router.settings.maxItems != null
            && router.settings.maxItems > 0
            && router.totalItemsRouted >= router.settings.maxItems) {
          out.println("MAXITEMS k=" + k + " totalItemsRouted=" + router.totalItemsRouted);
          router.thread.requestStop();
          break;
        }
        // :222-223.
        router.totalItemsRouted++;
        board.startMarkingChangedArea();

        // :225-226.
        SortedSet<Item> rippedItemList = new TreeSet<>();
        Map<Item, Integer> rippedItemCosts = new LinkedHashMap<>();

        StringBuilder sb = new StringBuilder();
        sb.append("{\"k\":").append(k++)
            .append(",\"item\":").append(currentItem.getId())
            .append(",\"netIndex\":").append(i)
            .append(",\"net\":").append(currentItem.getNetNumber(i));
        int maxIdBefore = board.communication.idGenerator.maxGeneratedId();

        // :238-245.
        AutorouteAttemptResult result =
            router.autorouteItem(
                currentItem, currentItem.getNetNumber(i), rippedItemList, rippedItemCosts, passNo);

        sb.append(",\"state\":\"").append(result.state).append('"');
        sb.append(",\"details\":").append(quote(result.details));
        sb.append(",\"ripped\":").append(ids(rippedItemList));
        // The port's map is a `BTreeMap<ItemId, i32>`; both sides sort by item id, the `p6t1`
        // convention (P6T1.routeOne's comment says why).
        sb.append(",\"ripupCosts\":[");
        List<Map.Entry<Item, Integer>> costs = new ArrayList<>(rippedItemCosts.entrySet());
        costs.sort(java.util.Comparator.comparingInt(e -> e.getKey().getId()));
        boolean first = true;
        for (Map.Entry<Item, Integer> entry : costs) {
          if (!first) {
            sb.append(',');
          }
          first = false;
          sb.append('[').append(entry.getKey().getId()).append(',').append(entry.getValue())
              .append(']');
        }
        sb.append(']');
        sb.append(",\"maxIdBefore\":").append(maxIdBefore);
        sb.append(",\"maxIdAfter\":")
            .append(board.communication.idGenerator.maxGeneratedId());
        appendInsertedGeometry(sb, board, maxIdBefore);

        // :260-289.
        switch (result.state) {
          case ROUTED -> ++routed;
          case ALREADY_CONNECTED, NO_UNCONNECTED_NETS, CONNECTED_TO_PLANE -> ++skipped;
          default -> {
            board.failureLog.recordFailure(currentItem, passNo, result.state, result.details);
            sb.append(",\"failures\":").append(board.failureLog.getFailureCount(currentItem));
            ++notRouted;
          }
        }
        // :290-291.
        --itemsToGoCount;
        rippedItemCount += rippedItemList.size();
        out.println(sb.append('}'));
        out.flush();
      }
    }

    // :296-303, with the deltas the brief asks for.
    out.println("TAILS-BEFORE " + boardShape(board));
    if (router.removeUnconnectedVias) {
      router.removeTails(Item.StopConnectionOption.NONE);
    } else {
      router.removeTails(Item.StopConnectionOption.FANOUT_VIA);
    }
    out.println("TAILS-AFTER " + boardShape(board));

    // :313-320.
    out.println(
        "COUNTERS pass="
            + passNo
            + " queued="
            + itemsToGoCount
            + " skipped="
            + skipped
            + " ripped="
            + rippedItemCount
            + " failed="
            + notRouted
            + " routed="
            + routed
            + " incomplete="
            + router.calculateIncompleteCount(board));

    // :330.
    return routed > 0 || notRouted > 0;
  }

  // -----------------------------------------------------------------------------------------
  // Rendering — the `p6t1` conventions
  // -----------------------------------------------------------------------------------------

  /**
   * The board's shape as four counts. It is the closing "tail-removal and {@code optChangedArea}
   * deltas" line: item, trace and via counts plus the incomplete count, which is what
   * {@code removeTails}' two statements can move.
   */
  static String boardShape(RoutingBoard board) {
    int items = 0;
    int traces = 0;
    int vias = 0;
    for (Item item : board.getItems()) {
      items++;
      if (item instanceof PolylineTrace) {
        traces++;
      } else if (item instanceof Via) {
        vias++;
      }
    }
    DesignRulesChecker drc = new DesignRulesChecker(board, null);
    return "items="
        + items
        + " traces="
        + traces
        + " vias="
        + vias
        + " incompletes="
        + drc.getIncompleteCount()
        + " traceLength=\""
        + Double.toString(board.cumulativeTraceLength())
        + "\"";
  }

  /** {@code P6T1.appendInsertedGeometry} — every item this connection's id generator produced. */
  static void appendInsertedGeometry(StringBuilder sb, RoutingBoard board, int maxIdBefore) {
    List<Item> inserted = new ArrayList<>();
    for (Item item : board.getItems()) {
      if (item.getId() > maxIdBefore) {
        inserted.add(item);
      }
    }
    java.util.Collections.reverse(inserted);

    sb.append(",\"traces\":[");
    boolean first = true;
    for (Item item : inserted) {
      if (!(item instanceof PolylineTrace trace)) {
        continue;
      }
      if (!first) {
        sb.append(',');
      }
      first = false;
      sb.append("{\"id\":").append(trace.getId())
          .append(",\"layer\":").append(trace.getLayer())
          .append(",\"halfWidth\":").append(trace.getHalfWidth())
          .append(",\"corners\":").append(corners(trace.polyline()))
          .append('}');
    }
    sb.append("],\"vias\":[");
    first = true;
    for (Item item : inserted) {
      if (!(item instanceof Via via)) {
        continue;
      }
      if (!first) {
        sb.append(',');
      }
      first = false;
      Padstack padstack = via.getPadstack();
      sb.append("{\"id\":").append(via.getId())
          .append(",\"center\":\"").append(pt(via.getCenter())).append('"')
          .append(",\"padstack\":").append(quote(padstack.name))
          .append(",\"firstLayer\":").append(via.firstLayer())
          .append(",\"lastLayer\":").append(via.lastLayer())
          .append('}');
    }
    sb.append(']');

    int other = 0;
    for (Item item : inserted) {
      if (!(item instanceof PolylineTrace) && !(item instanceof Via)) {
        other++;
      }
    }
    sb.append(",\"otherInserted\":").append(other);
  }

  /** {@code P6T1.pt}. */
  static String pt(Point p) {
    if (p == null) {
      return "null";
    }
    if (p instanceof IntPoint ip) {
      return "(" + ip.x + "," + ip.y + ")";
    }
    FloatPoint f = p.toFloat();
    return "~(" + Double.toString(f.x) + "," + Double.toString(f.y) + ")";
  }

  /** {@code P6T1.corners}. */
  static String corners(Polyline p) {
    if (p == null) {
      return "null";
    }
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < p.cornerCount(); i++) {
      if (i > 0) {
        sb.append(',');
      }
      Point corner = p.corner(i);
      if (corner instanceof IntPoint ip) {
        sb.append("\"(").append(ip.x).append(',').append(ip.y).append(")\"");
      } else {
        FloatPoint f = p.cornerApprox(i);
        sb.append("\"~(")
            .append(Double.toString(f.x))
            .append(',')
            .append(Double.toString(f.y))
            .append(")\"");
      }
    }
    return sb.append(']').toString();
  }

  /** {@code P6T1.ids} — a `TreeSet<Item>`, i.e. descending id (quirk #44). */
  static String ids(Set<Item> items) {
    StringBuilder sb = new StringBuilder("[");
    boolean first = true;
    for (Item item : items) {
      if (!first) {
        sb.append(',');
      }
      first = false;
      sb.append(item.getId());
    }
    return sb.append(']').toString();
  }

  /** {@code P6T1.quote}. */
  static String quote(String value) {
    if (value == null) {
      return "null";
    }
    StringBuilder sb = new StringBuilder("\"");
    for (int i = 0; i < value.length(); i++) {
      char c = value.charAt(i);
      switch (c) {
        case '"' -> sb.append("\\\"");
        case '\\' -> sb.append("\\\\");
        case '\n' -> sb.append("\\n");
        case '\r' -> sb.append("\\r");
        case '\t' -> sb.append("\\t");
        default -> {
          if (c < 0x20) {
            sb.append(String.format("\\u%04x", (int) c));
          } else {
            sb.append(c);
          }
        }
      }
    }
    return sb.append('"').toString();
  }

  /** Kept so `ConductionArea` and `Net` are imported for the reader who checks `:383-389`. */
  static boolean connectedSetHoldsPlane(Set<Item> connectedSet, Net net) {
    return net != null
        && net.containsPlane()
        && connectedSet.stream().anyMatch(ConductionArea.class::isInstance);
  }
}
