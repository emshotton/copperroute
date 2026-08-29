package app.freerouting.drc;

import app.freerouting.board.facade.BasicBoard;
import app.freerouting.board.model.items.ConductionArea;
import app.freerouting.board.model.items.Connectable;
import app.freerouting.board.model.items.DrillItem;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.items.Trace;
import app.freerouting.datastructures.PlanarDelaunayTriangulation;
import app.freerouting.datastructures.Signum;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.Point;
import app.freerouting.rules.Net;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collection;
import java.util.Comparator;
import java.util.HashSet;
import java.util.LinkedList;
import java.util.List;
import java.util.Set;
import java.util.SortedSet;
import java.util.TreeSet;

/**
 * Plan 5 Task 10 differential driver, algorithm level: the three raw lists behind {@code
 * generateReport}, so that a matching report cannot mask a compensating pair of errors (plan-5
 * ruling 14).
 *
 * <p>Usage: {@code P5T2 <dsn> [rules|-] [ses|-] <mode>}. The board is loaded by {@link
 * P5T1#loadBoard} — the same three calls in the same order — so the two drivers cannot drift apart
 * on their input; {@code run.sh} compiles {@code P5T1.java} alongside this class for that reason.
 *
 * <h2>The three modes</h2>
 *
 * <dl>
 *   <dt>{@code 0} — {@code getAllClearanceViolations()} (DesignRulesChecker.java:53-89)
 *   <dd>{@code V <firstId> <secondId> <layer> <expectedClearance> <actualClearance>} for every
 *       entry, <b>in list order</b>, then {@code VCOUNT <n>}. The doubles go through {@code
 *       Double.toString}, so the port compares exact bits through {@code
 *       fr_dsn::format::double::java_double_to_string}. This is a bit-parity surface: the list is
 *       the deduplicated walk over {@code board.getItems()} and is hash-independent.
 *   <dt>{@code 1} — {@code getAllUnconnectedItems()} (DesignRulesChecker.java:91-178)
 *   <dd>{@code U <type> <firstKind> <secondKind|-> <id,id,…>} for every entry, in list order, then
 *       {@code UCOUNT <n>} and one {@code UTYPE <type> <n>} line per type in a fixed order. Three
 *       renderings are deliberate:
 *       <ul>
 *         <li>{@code allItems} is printed <b>sorted ascending</b>. For a net entry it is {@code
 *             connectedSets.get(0)} followed by {@code connectedSets.get(1)} (`:143-145`), two
 *             {@code HashSet<Item>}s over a class with no {@code hashCode} override — identity-hash
 *             order, nothing to port (quirk #144, ruling 3).
 *         <li>A dangling entry's {@code allItems} is {@code Arrays.asList(firstItem, null)}
 *             (UnconnectedItems.java:41) — a two-element list whose second element is <b>null</b>,
 *             which nothing in Java ever reads. The port's {@code Vec<ItemId>} cannot hold a null
 *             and stores the singleton, so this driver drops the nulls rather than reporting a
 *             length mismatch. That divergence is recorded at
 *             {@code crates/fr-drc/src/unconnected.rs}'s {@code new_typed}.
 *         <li>{@code firstItem} and {@code secondItem} are printed as their <b>kind class</b> —
 *             {@code Pin}, {@code Trace} or {@code other} — and not as ids. For a net entry they
 *             are {@code findRepresentativeItem}'s answer over a {@code HashSet<Item>} (`:138-139`,
 *             `:181-200`), so <em>which</em> Pin comes back is identity-hash order (ruling 3 fixes
 *             the port's at the lowest id); the <em>class</em> survives any hash order, because the
 *             loop returns a Pin whenever the set holds one and a Trace whenever it holds no Pin
 *             but a Trace. This is exactly the projection {@code
 *             crates/fr-drc/tests/data/UnconnectedProbe.java} takes, for the same reason. Nothing
 *             is lost on the two dangling kinds: their single item's id is the {@code allItems}
 *             column.
 *       </ul>
 *       Everything else — the entry order, the counts — is compared as is. This is the second
 *       bit-parity surface.
 *   <dt>{@code 2} — the ratsnest, through the real accessors (DesignRulesChecker.java:542-798)
 *   <dd>{@code MAXCONN <n>}, {@code INCOMPLETE <n>}, one {@code NET <netNumber> <airlines>
 *       <groups> <lengthViolation>} line per net from 1 to {@code maxNetNumber()},
 *       {@code ALCOUNT <n>}, then one {@code AL <netNumber> <lowId> <highId>} line per airline.
 *       Everything above {@code AL} is <b>strict</b>: plan-5 ruling 4 measured those counters
 *       hash-independent under {@code -XX:hashCode=0..4}. The {@code AL} block is not, on this
 *       mode: the airline endpoints come out of a Delaunay triangulation whose corner insertion
 *       order is {@code calculateNetItems}' {@code HashSet} order (NetIncompletes.java:295, :299),
 *       which is the jar's here and the port's ascending item id there, so {@code sweep-p5t2.sh}
 *       counts the differing lines and grades them against a recorded budget. Mode 3 is where the
 *       endpoints are compared for real.
 *       <p>Each airline is printed as the <b>unordered</b> pair {@code min(fromId,toId)},
 *       {@code max(fromId,toId)}, and the lines are sorted by {@code (net, low, high)}. That is the
 *       convention of the committed {@code *.airlines-union.txt} files ({@code net=<n> a=<lo>
 *       b=<hi>}), and it drops one artifact of the insertion order — which end of an edge a run
 *       calls "from" — that is not a second fact about the board.
 *   <dt>{@code 3} — the same output from a re-seeded transcription of {@code NetIncompletes}
 *   <dd>The ratsnest's <b>parity surface</b>: the algorithm's one free choice, the seed of {@code
 *       calculateNetItems}' outer loop, pinned to the port's ascending item id (ruling 3). See
 *       {@link #reseededRatsnest}. Endpoints included, this matches the port exactly.
 *       <p>Mode 2's canonical {@code AL} block is printed unchanged, and a second block follows
 *       it: {@code ALD <netNumber> <fromId> <toId>}, one line per airline <b>in Kruskal's
 *       acceptance order</b> and with the edge's own direction. With the seed pinned on both
 *       sides, "which end is from" and "which airline was accepted first" stop being hash noise
 *       and become facts about the algorithm, so mode 3 compares them rather than normalising them
 *       away. {@code AL} keeps saying what the airline set is; {@code ALD} says how it was built.
 *   <dt>{@code 4} — {@code TRANSCRIPTION equal 0}
 *   <dd>The check on mode 3's transcription: the same code seeded Java's own way, compared inside
 *       the JVM against the real {@code getAllAirlines()}. Both sides of that comparison include
 *       the {@code ALD} block, so the self-check covers the direction and the acceptance order
 *       mode 3 now gates on. See {@link #transcriptionSelfCheck}.
 * </dl>
 */
public final class P5T2 {

  private P5T2() {}

  public static void main(String[] args) throws Exception {
    if (args.length < 2) {
      System.err.println("usage: P5T2 <dsn> [rules|-] [ses|-] <mode 0-4>");
      System.exit(2);
    }
    // `FRLogger` writes to stdout on at least one corpus fixture; see `P5T1.main`.
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    int mode = Integer.parseInt(args[args.length - 1]);
    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    Path rules = args.length > 2 ? P5T1.optionalPath(args, 1) : null;
    Path ses = args.length > 3 ? P5T1.optionalPath(args, 2) : null;

    Path jar =
        Paths.get(
                DesignRulesChecker.class
                    .getProtectionDomain()
                    .getCodeSource()
                    .getLocation()
                    .toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d fixture=%s rules=%s ses=%s mode=%d%n",
        jar,
        Files.size(jar),
        Files.getLastModifiedTime(jar).toMillis(),
        dsn.getFileName(),
        P5T1.name(rules),
        P5T1.name(ses),
        mode);

    BasicBoard board = P5T1.loadBoard(dsn, rules, ses);
    DesignRulesChecker drc = new DesignRulesChecker(board, null);

    switch (mode) {
      case 0 -> clearanceViolations(out, drc);
      case 1 -> unconnectedItems(out, drc);
      case 2 -> print(out, ratsnest(board, drc, false));
      case 3 -> print(out, reseededRatsnest(board, true, true));
      case 4 -> transcriptionSelfCheck(out, board, drc);
      default -> {
        System.err.println("mode must be 0, 1, 2, 3 or 4");
        System.exit(2);
      }
    }
    out.flush();
  }

  /** Mode 0. */
  private static void clearanceViolations(PrintStream out, DesignRulesChecker drc) {
    Collection<ClearanceViolation> violations = drc.getAllClearanceViolations();
    for (ClearanceViolation violation : violations) {
      out.printf(
          "V %d %d %d %s %s%n",
          violation.firstItem.getId(),
          violation.secondItem.getId(),
          violation.layer,
          Double.toString(violation.expectedClearance),
          Double.toString(violation.actualClearance));
    }
    out.printf("VCOUNT %d%n", violations.size());
  }

  /** Mode 1. */
  private static void unconnectedItems(PrintStream out, DesignRulesChecker drc) {
    Collection<UnconnectedItems> entries = drc.getAllUnconnectedItems();
    int nets = 0;
    int trackDangling = 0;
    int viaDangling = 0;
    for (UnconnectedItems entry : entries) {
      switch (entry.type) {
        case "track_dangling" -> trackDangling++;
        case "via_dangling" -> viaDangling++;
        default -> nets++;
      }
      // `allItems` holds a trailing `null` for the two dangling kinds (UnconnectedItems.java:41);
      // the port's `Vec<ItemId>` cannot, so the nulls are dropped here.
      List<Long> ids = new ArrayList<>();
      for (Item item : entry.allItems) {
        if (item != null) {
          ids.add((long) item.getId());
        }
      }
      ids.sort(Comparator.naturalOrder());
      StringBuilder joined = new StringBuilder();
      for (int i = 0; i < ids.size(); i++) {
        if (i > 0) {
          joined.append(',');
        }
        joined.append(ids.get(i));
      }
      out.printf(
          "U %s %s %s %s%n",
          entry.type,
          kindClass(entry.firstItem),
          entry.secondItem == null ? "-" : kindClass(entry.secondItem),
          joined);
    }
    out.printf("UCOUNT %d%n", entries.size());
    out.printf("UTYPE unconnectedItems %d%n", nets);
    out.printf("UTYPE track_dangling %d%n", trackDangling);
    out.printf("UTYPE via_dangling %d%n", viaDangling);
  }

  /**
   * {@code findRepresentativeItem}'s three-way preference (DesignRulesChecker.java:186-200),
   * which is what survives the {@code HashSet} order — see mode 1 in the class comment.
   */
  private static String kindClass(Item item) {
    if (item instanceof Pin) {
      return "Pin";
    }
    if (item instanceof Trace) {
      return "Trace";
    }
    return "other";
  }

  /**
   * Mode 2, through the real `DesignRulesChecker` accessors.
   *
   * <p>{@code directional} appends the {@code ALD} block — see {@link #directionalAirlineLines}.
   * Mode 2 prints without it (its endpoints are hash-ordered and graded against a budget); the
   * transcription self-check asks for it, so mode 4 compares the two seedings' airlines with their
   * direction and their acceptance order intact.
   */
  private static List<String> ratsnest(
      BasicBoard board, DesignRulesChecker drc, boolean directional) {
    drc.calculateAllIncompletes();
    List<String> lines = new ArrayList<>();
    lines.add("MAXCONN " + drc.maxConnections);
    lines.add("INCOMPLETE " + drc.getIncompleteCount());

    int maxNetNo = board.rules.nets.maxNetNumber();
    for (int netNumber = 1; netNumber <= maxNetNo; netNumber++) {
      NetIncompletes netIncompletes = drc.getNetIncompletes(netNumber);
      lines.add(
          "NET "
              + netNumber
              + " "
              + netIncompletes.count()
              + " "
              + netIncompletes.getConnectedGroupCount()
              + " "
              + Double.toString(netIncompletes.getLengthViolation()));
    }

    AirLine[] airlines = drc.getAllAirlines();
    lines.add("ALCOUNT " + airlines.length);
    List<long[]> unordered = new ArrayList<>(airlines.length);
    for (AirLine airline : airlines) {
      long from = airline.fromItem.getId();
      long to = airline.toItem.getId();
      unordered.add(new long[] {airline.net.netNumber, Math.min(from, to), Math.max(from, to)});
    }
    lines.addAll(airlineLines(unordered));
    if (directional) {
      // `getAllAirlines` walks `netIncompletes` in net order and each net's `incompletes` in
      // Kruskal's acceptance order (DesignRulesChecker.java:787-793), which is the order this
      // block preserves.
      List<long[]> ordered = new ArrayList<>(airlines.length);
      for (AirLine airline : airlines) {
        ordered.add(
            new long[] {airline.net.netNumber, airline.fromItem.getId(), airline.toItem.getId()});
      }
      lines.addAll(directionalAirlineLines(ordered));
    }
    return lines;
  }

  /**
   * The `ALD` block: {@code ALD <net> <fromId> <toId>}, one line per airline, <b>in Kruskal's
   * acceptance order</b> and with the edge's own direction — neither sorted nor normalised.
   *
   * <p>This is the strict complement of {@link #airlineLines}. `AL` deliberately drops two
   * artifacts of the triangulation's insertion order (which end an edge calls "from", and where an
   * airline sits in the accepted sequence) because on mode 2 they are hash noise. On modes 3 and 4
   * the seed is pinned on both sides, so they are facts about the algorithm and are compared.
   */
  private static List<String> directionalAirlineLines(List<long[]> ordered) {
    List<String> lines = new ArrayList<>(ordered.size());
    for (long[] triple : ordered) {
      lines.add("ALD " + triple[0] + " " + triple[1] + " " + triple[2]);
    }
    return lines;
  }

  /** The `AL` block: the unordered pair, sorted by `(net, low, high)`. */
  private static List<String> airlineLines(List<long[]> unordered) {
    unordered.sort(
        Comparator.<long[]>comparingLong(a -> a[0])
            .thenComparingLong(a -> a[1])
            .thenComparingLong(a -> a[2]));
    List<String> lines = new ArrayList<>(unordered.size());
    for (long[] triple : unordered) {
      lines.add("AL " + triple[0] + " " + triple[1] + " " + triple[2]);
    }
    return lines;
  }

  private static void print(PrintStream out, List<String> lines) {
    for (String line : lines) {
      out.println(line);
    }
  }

  /**
   * Mode 4: the transcription check. `reseededRatsnest(board, false)` keeps Java's own `HashSet`
   * seed, so it must reproduce {@link #ratsnest} — i.e. the jar's real
   * `DesignRulesChecker.getAllAirlines()` — line for line. The Rust twin prints the `equal 0` line
   * unconditionally, exactly as `p4t1`'s twin prints `JSON_SOURCE_EMPTY`: this side computes the
   * fact, that side states it, and the harness's diff is the assertion.
   */
  private static void transcriptionSelfCheck(
      PrintStream out, BasicBoard board, DesignRulesChecker drc) {
    List<String> real = ratsnest(board, drc, true);
    List<String> transcribed = reseededRatsnest(board, false, true);
    int differing = 0;
    for (int i = 0; i < Math.max(real.size(), transcribed.size()); i++) {
      String a = i < real.size() ? real.get(i) : null;
      String b = i < transcribed.size() ? transcribed.get(i) : null;
      if (a == null || !a.equals(b)) {
        if (differing < 10) {
          System.err.println("transcription line " + i + ": real=" + a + " transcribed=" + b);
        }
        differing++;
      }
    }
    out.printf("TRANSCRIPTION %s %d%n", differing == 0 ? "equal" : "differs", differing);
  }

  // ---------------------------------------------------------------------------------------------
  // Modes 3 and 4: `NetIncompletes` re-run with a chosen seed order
  // ---------------------------------------------------------------------------------------------

  /**
   * `drc.NetIncompletes`' constructor (NetIncompletes.java:57-226), `calculateNetItems`
   * (`:293-322`), `joinConnectedSets` (`:328-337`), `Edge` (`:344-380`) and `calcLengthViolation`
   * (`:259-275`), transcribed here so that the **one** free choice in the algorithm can be varied.
   *
   * <p>That choice is the seed of `calculateNetItems`' outer loop: Java takes
   * `uniqueItems.iterator().next()` off a `HashSet<Item>` over a class with no `hashCode` override
   * (`:295`, `:299`), so it is identity-hash ordered and there is no order to port. Plan-5 ruling 3
   * fixes the port's at the **lowest item id**. Everything else — the filter, the order *within* a
   * component (`Item.getConnectedSet` returns a `TreeSet` keyed by the reversed `Item.compareTo`,
   * so it is descending id, ruling 15), the corner flattening, the `TreeSet<Edge>` and Kruskal — is
   * Java's and is copied statement for statement.
   *
   * <p>So:
   *
   * <ul>
   *   <li><b>mode 4</b> (`ascendingSeed == false`) keeps Java's `HashSet` seed and must reproduce
   *       {@code DesignRulesChecker.getAllAirlines()} — i.e. {@link #ratsnest}'s own lines — line
   *       for line, {@code ALD} block included. That is the transcription check: it is what says
   *       this re-implementation is the same algorithm as the jar's, and `sweep-p5t2.sh` runs it as
   *       a Java-against-Java diff with no Rust side at all.
   *   <li><b>mode 3</b> (`ascendingSeed == true`) seeds ascending by item id, which is exactly what
   *       the port does. It is the parity surface for the ratsnest: with the one free choice
   *       pinned the same way on both sides, the airlines themselves — not merely their counts, and
   *       with their direction and acceptance order via {@code ALD} — must match, and on the corpus
   *       they do.
   * </ul>
   *
   * <p>The output format is mode 2's ({@code directional} adds the {@code ALD} block on top), so
   * the three can be diffed against each other directly.
   */
  private static List<String> reseededRatsnest(
      BasicBoard board, boolean ascendingSeed, boolean directional) {
    int maxNetNo = board.rules.nets.maxNetNumber();
    List<Collection<Item>> netItemLists = new ArrayList<>(maxNetNo);
    for (int i = 0; i < maxNetNo; i++) {
      netItemLists.add(new LinkedList<>());
    }
    // DesignRulesChecker.java:549-560.
    for (Item item : board.getItems()) {
      if (item instanceof Connectable) {
        for (int i = 0; i < item.netCount(); i++) {
          netItemLists.get(item.getNetNumber(i) - 1).add(item);
        }
      }
    }
    // DesignRulesChecker.java:566-578.
    int maxConnections = 0;
    for (Collection<Item> list : netItemLists) {
      if (list.isEmpty()) {
        continue;
      }
      long endpointCount =
          list.stream().filter(i -> i instanceof Pin || i instanceof ConductionArea).count();
      maxConnections += (int) Math.max(0, endpointCount - 1);
    }

    List<Ratsnest> perNet = new ArrayList<>(maxNetNo);
    int incompleteCount = 0;
    for (int netNumber = 1; netNumber <= maxNetNo; netNumber++) {
      Ratsnest ratsnest =
          new Ratsnest(netNumber, netItemLists.get(netNumber - 1), board, ascendingSeed);
      perNet.add(ratsnest);
      incompleteCount += ratsnest.airlines.size();
    }

    List<String> lines = new ArrayList<>();
    lines.add("MAXCONN " + maxConnections);
    lines.add("INCOMPLETE " + incompleteCount);
    for (int netNumber = 1; netNumber <= maxNetNo; netNumber++) {
      Ratsnest ratsnest = perNet.get(netNumber - 1);
      lines.add(
          "NET "
              + netNumber
              + " "
              + ratsnest.airlines.size()
              + " "
              + ratsnest.connectedGroupCount
              + " "
              + Double.toString(ratsnest.lengthViolation));
    }
    List<long[]> unordered = new ArrayList<>();
    for (int netNumber = 1; netNumber <= maxNetNo; netNumber++) {
      for (long[] airline : perNet.get(netNumber - 1).airlines) {
        unordered.add(
            new long[] {
              netNumber, Math.min(airline[0], airline[1]), Math.max(airline[0], airline[1])
            });
      }
    }
    lines.add("ALCOUNT " + unordered.size());
    lines.addAll(airlineLines(unordered));
    if (directional) {
      List<long[]> ordered = new ArrayList<>(unordered.size());
      for (int netNumber = 1; netNumber <= maxNetNo; netNumber++) {
        for (long[] airline : perNet.get(netNumber - 1).airlines) {
          ordered.add(new long[] {netNumber, airline[0], airline[1]});
        }
      }
      lines.addAll(directionalAirlineLines(ordered));
    }
    return lines;
  }

  /** One net's ratsnest: `NetIncompletes`' constructor with the seed order made a parameter. */
  private static final class Ratsnest {

    /** `{fromItem.getId(), toItem.getId()}` per airline, in Kruskal's acceptance order. */
    final List<long[]> airlines = new ArrayList<>();

    int connectedGroupCount;
    double lengthViolation;

    Ratsnest(int netNumber, Collection<Item> netItems, BasicBoard board, boolean ascendingSeed) {
      Net net = board.rules.nets.get(netNumber);

      // NetIncompletes.java:80-116.
      Collection<Item> filteredItems = new LinkedList<>();
      for (Item item : netItems) {
        if (item.isTail()) {
          continue;
        }
        if (!(item instanceof ConductionArea)
            && !(item instanceof DrillItem)
            && item.getNormalContacts().isEmpty()) {
          continue;
        }
        filteredItems.add(item);
      }

      // NetIncompletes.java:135.
      NetItem[] groupedNetItems = calculateNetItems(filteredItems, netNumber, ascendingSeed);

      // NetIncompletes.java:137-141.
      Set<Collection<Item>> uniqueConnectedSets = new HashSet<>();
      for (NetItem netItem : groupedNetItems) {
        uniqueConnectedSets.add(netItem.connectedSet);
      }
      this.connectedGroupCount = uniqueConnectedSets.size();

      // NetIncompletes.java:154-163 — `calcLengthViolation` is deliberately not reached.
      if (groupedNetItems.length <= 1) {
        this.connectedGroupCount = groupedNetItems.length;
        return;
      }

      // NetIncompletes.java:166-184.
      Collection<PlanarDelaunayTriangulation.Storable> triangulationObjects =
          new LinkedList<>(Arrays.asList(groupedNetItems));
      PlanarDelaunayTriangulation triangulation =
          new PlanarDelaunayTriangulation(triangulationObjects);
      SortedSet<Edge> sortedEdges = new TreeSet<>();
      for (PlanarDelaunayTriangulation.ResultEdge currentLine : triangulation.getEdgeLines()) {
        sortedEdges.add(
            new Edge(
                (NetItem) currentLine.startObject,
                currentLine.startPoint.toFloat(),
                (NetItem) currentLine.endObject,
                currentLine.endPoint.toFloat()));
      }

      // NetIncompletes.java:190-205.
      for (Edge currentEdge : sortedEdges) {
        if (currentEdge.fromItem.connectedSet == currentEdge.toItem.connectedSet) {
          continue;
        }
        this.airlines.add(
            new long[] {currentEdge.fromItem.item.getId(), currentEdge.toItem.item.getId()});
        joinConnectedSets(
            groupedNetItems, currentEdge.fromItem.connectedSet, currentEdge.toItem.connectedSet);
      }

      // NetIncompletes.java:225 -> :259-275.
      calcLengthViolation(net);
    }

    /** NetIncompletes.java:259-275. */
    private void calcLengthViolation(Net net) {
      double maxLength = net.getNetClass().getMaximumTraceLength();
      double minLength = net.getNetClass().getMinimumTraceLength();
      if (maxLength <= 0 && minLength <= 0) {
        this.lengthViolation = 0;
        return;
      }
      double newViolation = 0;
      double traceLength = net.getTraceLength();
      if (maxLength > 0 && traceLength > maxLength) {
        newViolation = traceLength - maxLength;
      }
      if (minLength > 0 && traceLength < minLength && this.airlines.isEmpty()) {
        newViolation = traceLength - minLength;
      }
      this.lengthViolation = newViolation;
    }

    /**
     * NetIncompletes.java:293-322. The `uniqueItems` set is the only line that differs between the
     * two modes: a `HashSet` reproduces the jar, a `TreeSet` ordered by item id reproduces the
     * port's ruling-3 seed order.
     */
    private static NetItem[] calculateNetItems(
        Collection<Item> itemList, int netNumber, boolean ascendingSeed) {
      List<NetItem> result = new ArrayList<>();
      Set<Item> uniqueItems =
          ascendingSeed
              ? new TreeSet<>(Comparator.comparingInt(Item::getId))
              : new HashSet<>();
      uniqueItems.addAll(itemList);

      while (!uniqueItems.isEmpty()) {
        Item startItem = uniqueItems.iterator().next();
        Collection<Item> currentConnectedSet = startItem.getConnectedSet(netNumber);

        Collection<Item> itemsInComponent = new ArrayList<>();
        for (Item itemInSet : currentConnectedSet) {
          if (uniqueItems.contains(itemInSet)) {
            itemsInComponent.add(itemInSet);
          }
        }
        // The jar loops forever here when `itemsInComponent` is empty (the seed does not carry the
        // net); the port drops the seed and carries on, and so does this, so that a corpus fixture
        // that reached it would report rather than hang. No fixture does.
        if (itemsInComponent.isEmpty()) {
          uniqueItems.remove(startItem);
          continue;
        }
        for (Item currentItem : itemsInComponent) {
          result.add(new NetItem(currentItem, currentConnectedSet));
        }
        uniqueItems.removeAll(itemsInComponent);
      }
      return result.toArray(new NetItem[0]);
    }

    /** NetIncompletes.java:328-337. */
    private static void joinConnectedSets(
        NetItem[] netItems, Collection<Item> fromConnectedSet, Collection<Item> toConnectedSet) {
      for (NetItem netItem : netItems) {
        if (netItem.connectedSet == fromConnectedSet) {
          toConnectedSet.add(netItem.item);
          netItem.connectedSet = toConnectedSet;
        }
      }
    }
  }

  /** NetIncompletes.java:383-397. */
  private static final class NetItem implements PlanarDelaunayTriangulation.Storable {

    final Item item;
    Collection<Item> connectedSet;

    NetItem(Item item, Collection<Item> connectedSet) {
      this.item = item;
      this.connectedSet = connectedSet;
    }

    @Override
    public Point[] getTriangulationCorners() {
      return this.item.getRatsnestCorners();
    }
  }

  /** NetIncompletes.java:344-380. */
  private static final class Edge implements Comparable<Edge> {

    final NetItem fromItem;
    final FloatPoint fromCorner;
    final NetItem toItem;
    final FloatPoint toCorner;
    final double lengthSquare;

    Edge(NetItem fromItem, FloatPoint fromCorner, NetItem toItem, FloatPoint toCorner) {
      this.fromItem = fromItem;
      this.fromCorner = fromCorner;
      this.toItem = toItem;
      this.toCorner = toCorner;
      this.lengthSquare = toCorner.distanceSquare(fromCorner);
    }

    @Override
    public int compareTo(Edge other) {
      double result = this.lengthSquare - other.lengthSquare;
      if (result == 0) {
        result = this.fromCorner.x - other.fromCorner.x;
        if (result == 0) {
          result = this.fromCorner.y - other.fromCorner.y;
        }
        if (result == 0) {
          result = this.toCorner.x - other.toCorner.x;
        }
        if (result == 0) {
          result = this.toCorner.y - other.toCorner.y;
        }
      }
      return Signum.asInt(result);
    }
  }
}
