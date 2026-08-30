package app.freerouting.autoroute.maze;

import app.freerouting.autoroute.AutorouteAttemptResult;
import app.freerouting.autoroute.AutorouteAttemptState;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Connectable;
import app.freerouting.board.model.items.ConductionArea;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.trace.PolylineTrace;
import app.freerouting.board.model.items.Via;
import app.freerouting.datastructures.TimeLimit;
import app.freerouting.drc.DesignRulesChecker;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;
import app.freerouting.core.library.Padstack;
import app.freerouting.rules.Net;
import app.freerouting.settings.RouterSettings;
import app.freerouting.settings.sources.DefaultSettings;
import java.io.FileDescriptor;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.SortedSet;
import java.util.TreeSet;

/**
 * Plan 6 Task 17 differential driver, connection level: routes the first {@code k} connections of a
 * real DSN board through steps 1-5 of {@code AutorouteConnectionRouter.route}
 * (AutorouteConnectionRouter.java:36-90, plan-6 ruling 2's seam) and prints one JSON line per
 * connection, against the port's {@code fr_router::route_connection}.
 *
 * <p>Usage: {@code P6T1 <dsn> [maxItems] [ripupPassNo] [rules|-]}. Defaults: {@code maxItems = 8},
 * {@code ripupPassNo = 1}, no {@code .rules} file. The fourth slot exists for plan-6 ruling 9 (the
 * via-info / via-rule re-pointing register row): the deciding comparison is Java-with-rules against
 * the port-with-rules on {@code Issue593-BBD_Mars-64.dsn} plus
 * {@code crates/fr-router/tests/data/ruling-h-redeclare.rules} — see Task 8's report §4. It is
 * applied exactly as {@code Freerouting.initializeDrc} applies it (Freerouting.java:277-292): after
 * the DSN and before anything else, with the design name the input file's base name **without**
 * {@code .dsn}, which is what {@code RulesReader.java:100-110} compares against the
 * {@code (rules PCB <name>} header.
 *
 * <h2>Why the driver and not the pipeline</h2>
 *
 * <p>{@code BatchAutorouter}/{@code AutoroutePassRunner} own item selection, the pass loop, the
 * pull-tight after every routed connection and the necked retry — all of which are <b>Plan 7's</b>
 * and none of which exists in the port yet. So this driver reproduces exactly the slice plan-6
 * ruling 2 puts below the seam and nothing above it:
 *
 * <ol>
 *   <li>the connection list is picked <b>deterministically without the pipeline</b>: walk {@code
 *       board.getItems()} — which is {@code itemList} in <b>descending item id</b> (quirk #63) —
 *       and for every {@link Connectable} item, for every net index {@code i} of that item in
 *       Java's own {@code 0..netCount()} order (AutoroutePassRunner.java:207), keep the pair
 *       {@code (item, item.getNetNumber(i))} when {@code item.getUnconnectedSet(net)} is non-empty.
 *       The first {@code maxItems} pairs are the connections. The list is computed <b>once</b>,
 *       before any routing, exactly as {@code AutoroutePassRunner} computes {@code
 *       autorouteItemList} once per pass; an entry whose item a later connection ripped up is
 *       reported as {@code "GONE"} and skipped;
 *   <li>each connection runs {@code AutorouteControl} + the plane swap + the {@code TimeLimit} +
 *       {@code initAutoroute} + {@code autorouteConnection}, i.e. `route`'s `:37-90`, with
 *       `ripupAllowed = true`, `ripupCosts = startRipupCosts * ripupPassNo` (`:45`), {@code
 *       removeUnconnectedVias = !settings.isFanoutEnabled()} (the {@code RoutingJob} constructor's
 *       value, BatchAutorouter.java:110-121) and {@code retainAutorouteDatabase = false}
 *       (BatchAutorouterThread.java:90);
 *   <li>{@code board.startMarkingChangedArea()} is called before each connection because {@code
 *       AutoroutePassRunner.java:224} calls it there, and the presence of {@code
 *       board.changedArea} is <b>observable</b>: {@code TraceShover.insert:571-575} dereferences it
 *       with no null check (quirk #177). Leaving it out would route a different board;
 *   <li>nothing above the seam runs: no {@code optChangedArea}, no {@code retryConnectionNecked},
 *       no strict-DRC rollback, no {@code finishAutoroute}.
 * </ol>
 *
 * <h2>The settings</h2>
 *
 * <p>{@code new DefaultSettings().getSettings()} then {@code setLayerCount} then {@code
 * applyBoardSpecificOptimizations} — the two board-dependent steps {@code RouterSettings(board)}
 * performs (RouterSettings.java:127-131) on top of the priority-0 source of the real headless
 * ladder. It is <b>not</b> a bare {@code new RouterSettings()}: {@code DefaultSettings.java:103}
 * sets {@code automaticNeckdown = true}, so {@code FoundConnectionInserter.tryNeckDown} is live on
 * every pipeline-loaded board and a bare constructor would silently take it out of the comparison.
 * The remaining sources of the headless ladder contribute nothing here — none of the corpus DSNs
 * carries an {@code (autoroute_settings …)} scope, and there is no {@code .rules} file, no
 * environment and no command line in this driver.
 *
 * <p>{@code settings.maxThreads} is {@code availableProcessors() - 1} and therefore
 * machine-dependent; it is never read on a headless routing path (quirk #143) and is not printed.
 *
 * <h2>The output</h2>
 *
 * <p>Line 1 is {@code HEADER …} — the jar the port is a port of, its size and mtime, the fixture
 * and the two arguments; the {@code p4t1}/{@code p5t1} convention, and the reason a run against the
 * wrong jar is a diff rather than a silent pass. {@code scripts/gen-router-reference.sh} moves it
 * into {@code router.meta.txt} so the committed {@code router.jsonl} carries no machine-specific
 * path. The jar's version and {@code java -version} go to <b>stderr</b>, which {@code run.sh} does
 * not diff.
 *
 * <p>Then one JSON line per connection. Every coordinate is rendered by {@link #pt}: an {@code
 * IntPoint} as {@code "(x,y)"} and a rational one as {@code "~(x,y)"} through {@code
 * Double.toString} — a JSON <em>string</em> rather than a number pair, so that the exact rendering
 * is the comparison surface and no re-parse can round it. {@code traceLength} is a string for the
 * same reason.
 */
public final class P6T1 {

  private P6T1() {}

  public static void main(String[] args) throws Exception {
    if (args.length < 1) {
      System.err.println("usage: P6T1 <dsn> [maxItems] [ripupPassNo] [rules|-]");
      System.exit(2);
    }
    // `FRLogger` writes to stdout on several corpus fixtures (degenerate-wire warnings, the
    // router's own info lines), and the port has no logger to reproduce them with. Same guard as
    // `P5T1.java`: the driver's output goes to a private stream on the real stdout and
    // `System.out` is redirected into the void.
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    int maxItems = args.length > 1 ? Integer.parseInt(args[1]) : 8;
    int ripupPassNo = args.length > 2 ? Integer.parseInt(args[2]) : 1;
    Path rules = optionalPath(args, 3);

    Path jar =
        Paths.get(
                RoutingBoard.class.getProtectionDomain().getCodeSource().getLocation().toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d fixture=%s maxItems=%d ripupPassNo=%d rules=%s%n",
        jar, Files.size(jar), Files.getLastModifiedTime(jar).toMillis(),
        dsn.getFileName(), maxItems, ripupPassNo,
        rules == null ? "-" : rules.getFileName());
    System.err.println("java-version " + System.getProperty("java.version"));

    RoutingBoard board = loadBoard(dsn, rules);
    RouterSettings settings = new DefaultSettings().getSettings();
    settings.setLayerCount(board.getLayerCount());
    settings.applyBoardSpecificOptimizations(board);

    for (Connection connection : pickConnections(board, maxItems)) {
      out.println(routeOne(board, settings, connection, ripupPassNo));
      out.flush();
    }
  }

  // -----------------------------------------------------------------------------------------
  // Board and connection selection
  // -----------------------------------------------------------------------------------------

  /** The board {@code RoutingJob} loads: the DSN, then the optional {@code .rules} file. */
  static RoutingBoard loadBoard(Path dsn, Path rules) throws Exception {
    BoardReadResult result;
    String designName = dsn.getFileName().toString();
    try (FileInputStream in = new FileInputStream(dsn.toFile())) {
      result = DsnReader.readBoard(in, null, null, designName);
    }
    // `ReadScopeParameter.java:159` is the only construction on this path and it builds a
    // `RoutingBoard`, so the `BasicBoard` the sealed result carries always is one.
    RoutingBoard board =
        switch (result) {
          case BoardReadResult.Success s -> (RoutingBoard) s.board();
          case BoardReadResult.OutlineMissing o -> (RoutingBoard) o.board();
          default -> throw new IllegalStateException("board did not read: " + result);
        };
    if (rules != null) {
      try (FileInputStream in = new FileInputStream(rules.toFile())) {
        // `designName` is `RoutingJob.name`, the base name **without** `.dsn`
        // (RoutingJob.java:457); `RulesReader.java:100-110` compares it against the
        // `(rules PCB <name>` header and takes the mismatch branch when they differ, so both
        // sides must pass the same string. The three-argument overload is the one the DRC path
        // uses too (RulesReader.java:47-49): the fourth argument only receives the file's
        // `(autoroute_settings …)`, which never reaches the board.
        String rulesDesignName =
            designName.endsWith(".dsn")
                ? designName.substring(0, designName.length() - ".dsn".length())
                : designName;
        if (!app.freerouting.io.specctra.RulesReader.read(in, rulesDesignName, board)) {
          throw new IllegalStateException("rules were rejected: " + rules);
        }
      }
    }
    return board;
  }

  /** {@code args[i]}, with a missing argument, an empty one and {@code -} all meaning "absent". */
  static Path optionalPath(String[] args, int i) {
    if (i >= args.length) {
      return null;
    }
    String value = args[i];
    if (value == null || value.isBlank() || "-".equals(value)) {
      return null;
    }
    return Paths.get(value).toAbsolutePath().normalize();
  }

  /** One {@code (item, net)} pair of the connection list, resolved by id at routing time. */
  record Connection(int k, int itemId, int netNo) {}

  /**
   * The connection list, computed once before any routing. See the class comment: {@code
   * getItems()} order (descending id, quirk #63) × the item's own net index order.
   */
  static List<Connection> pickConnections(RoutingBoard board, int maxItems) {
    List<Connection> result = new ArrayList<>();
    int k = 0;
    for (Item item : board.getItems()) {
      if (!(item instanceof Connectable)) {
        continue;
      }
      for (int i = 0; i < item.netCount(); i++) {
        int netNo = item.getNetNumber(i);
        if (item.getUnconnectedSet(netNo).isEmpty()) {
          continue;
        }
        result.add(new Connection(++k, item.getId(), netNo));
        if (result.size() >= maxItems) {
          return result;
        }
      }
    }
    return result;
  }

  // -----------------------------------------------------------------------------------------
  // One connection: `AutorouteConnectionRouter.route:36-90`
  // -----------------------------------------------------------------------------------------

  static String routeOne(
      RoutingBoard board, RouterSettings settings, Connection connection, int ripupPassNo) {
    StringBuilder sb = new StringBuilder();
    sb.append("{\"k\":").append(connection.k())
        .append(",\"item\":").append(connection.itemId())
        .append(",\"net\":").append(connection.netNo());

    Item item = board.getItem(connection.itemId());
    if (item == null) {
      // A connection whose item an earlier connection ripped up. Java's pass runner would have
      // rebuilt `autorouteItemList` before the next pass; this driver is inside one pass.
      return sb.append(",\"state\":\"GONE\"}").toString();
    }

    board.startMarkingChangedArea();
    SortedSet<Item> rippedItemList = new TreeSet<>();
    Map<Item, Integer> ripupCosts = new LinkedHashMap<>();
    int maxIdBefore = board.communication.idGenerator.maxGeneratedId();

    AutorouteAttemptResult result =
        route(board, settings, item, connection.netNo(), rippedItemList, ripupCosts, ripupPassNo);

    sb.append(",\"state\":\"").append(result.state).append('"');
    sb.append(",\"details\":").append(quote(result.details));
    sb.append(",\"ripped\":").append(ids(rippedItemList));
    // `ripupCosts` is a `LinkedHashMap<Item, Integer>` here — insertion order, i.e. the order
    // `MazeRipupResolver` priced the candidates in — and a `BTreeMap<ItemId, i32>` in the port
    // (Task 16), where that order is not representable. Nothing below plan-6 ruling 2's seam reads
    // the map back in order (`MazeRipupResolver.java` only ever `get`s and `put`s by item), so
    // **both sides sort by item id** and the comparison is over the map's contents. Without the
    // sort every multi-rip connection of `router-dac2020-bm01` diffs on nothing but the ordering
    // — measured: k = 252, 261, 279, 286 and 293, all five identical once sorted.
    sb.append(",\"ripupCosts\":[");
    List<Map.Entry<Item, Integer>> costs = new ArrayList<>(ripupCosts.entrySet());
    costs.sort(java.util.Comparator.comparingInt(e -> e.getKey().getId()));
    boolean first = true;
    for (Map.Entry<Item, Integer> entry : costs) {
      if (!first) {
        sb.append(',');
      }
      first = false;
      sb.append('[').append(entry.getKey().getId()).append(',').append(entry.getValue()).append(']');
    }
    sb.append(']');
    sb.append(",\"maxIdBefore\":").append(maxIdBefore);
    sb.append(",\"maxIdAfter\":").append(board.communication.idGenerator.maxGeneratedId());
    appendInsertedGeometry(sb, board, maxIdBefore);
    appendMetrics(sb, board, connection.netNo());
    return sb.append('}').toString();
  }

  /**
   * Steps 1-5 of {@code AutorouteConnectionRouter.route} (`:36-90`), including its own {@code catch
   * (Exception)} — plan-6 ruling 7's fifth recovery boundary, which degrades to a bare {@code
   * FAILED} with no details (`:155-158`).
   */
  static AutorouteAttemptResult route(
      RoutingBoard board,
      RouterSettings settings,
      Item item,
      int routeNetNo,
      SortedSet<Item> rippedItemList,
      Map<Item, Integer> ripupCosts,
      int ripupPassNo) {
    try {
      // :37-40.
      Net routeNet = board.rules.nets.get(routeNetNo);
      boolean containsPlane = routeNet != null && routeNet.containsPlane();
      int currentViaCosts =
          containsPlane ? settings.getPlaneViaCosts() : settings.getViaCosts();

      // :42-47.
      AutorouteControl ctrl =
          new AutorouteControl(
              board, routeNetNo, settings, currentViaCosts, settings.getTraceCosts());
      ctrl.ripupAllowed = true;
      ctrl.ripupCosts = settings.getStartRipupCosts() * ripupPassNo;
      ctrl.removeUnconnectedVias = !settings.isFanoutEnabled();

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

      // `:70` `setAirLine` is a GUI field with no reader below the seam, so it is not run here.

      // :71-74.
      double maxMilliseconds = 100000 * Math.pow(2, ripupPassNo - 1);
      maxMilliseconds = Math.min(maxMilliseconds, Integer.MAX_VALUE);
      TimeLimit timeLimit = new TimeLimit((int) maxMilliseconds);

      // :76-82. `retainAutorouteDatabase` is the benchmark-only system property, hard-coded
      // `false` at BatchAutorouterThread.java:90.
      AutorouteEngine autorouteEngine =
          board.initAutoroute(
              routeNetNo, ctrl.traceClearanceClassIndex, null, timeLimit, false);

      // :88-90.
      return autorouteEngine.autorouteConnection(
          routeStartSet, routeDestSet, ctrl, rippedItemList, ripupCosts);
    } catch (Exception e) {
      // :155-158.
      return new AutorouteAttemptResult(AutorouteAttemptState.FAILED);
    }
  }

  // -----------------------------------------------------------------------------------------
  // Rendering
  // -----------------------------------------------------------------------------------------

  /**
   * Ruling 1(b): every item the connection inserted, i.e. every item whose id the connection's own
   * id generator produced. The board walk is {@code getItems()}' descending id order, reversed so
   * the list reads in insertion order.
   */
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

    // Anything the connection inserted that is neither a trace nor a via would be invisible in
    // the two lists above, so its count is printed rather than dropped.
    int other = 0;
    for (Item item : inserted) {
      if (!(item instanceof PolylineTrace) && !(item instanceof Via)) {
        other++;
      }
    }
    sb.append(",\"otherInserted\":").append(other);
  }

  /** Ruling 1(c): spec §9's four metrics, recomputed on the live board after the connection. */
  static void appendMetrics(StringBuilder sb, RoutingBoard board, int netNo) {
    DesignRulesChecker drc = new DesignRulesChecker(board, null);
    int incompletes = drc.getIncompleteCount();
    int violations = drc.getAllClearanceViolations().size();
    Net net = board.rules.nets.get(netNo);
    int vias = net != null ? net.getViaCount() : 0;
    sb.append(",\"metrics\":{\"incompletes\":").append(incompletes)
        .append(",\"vias\":").append(vias)
        .append(",\"traceLength\":\"").append(Double.toString(board.cumulativeTraceLength()))
        .append("\",\"violations\":").append(violations)
        .append('}');
  }

  /** `P6T15Probe.pt`'s rendering: an exact `IntPoint`, or a `Double.toString` pair marked `~`. */
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

  /** A JSON string literal, or `null`. Only `"` and `\` occur in the values printed here. */
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
}
