package app.freerouting.autoroute.maze;

import app.freerouting.autoroute.BoardHistory;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.DrillItem;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Trace;
import app.freerouting.board.model.items.Via;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.core.library.Padstack;
import app.freerouting.geometry.planar.ConvexShape;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.IntVector;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import app.freerouting.settings.RouterSettings;
import app.freerouting.settings.ScoringSettings;
import app.freerouting.settings.sources.DefaultSettings;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.Constructor;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.SortedSet;
import java.util.TreeSet;

/**
 * Plan 7 Task 3 (controller ruling AH) differential driver: the three <b>decisions</b> Java makes
 * by comparing two {@code BasicBoard.getHash()} values, against the port's
 * {@code Board::structural_hash}.
 *
 * <p>Usage: {@code P7T10 <dsn> <steps> [routeK] [mode]}. {@code mode} is {@code warm} (the
 * default) or {@code raw}; {@code routeK} defaults to 0.
 *
 * <h2>What is printed</h2>
 *
 * <p>Line 1 is {@code HEADER …} (the {@code p4t1}/{@code p6t1}/{@code p7t7} convention: the jar,
 * its size and mtime, the fixture and the arguments, so a run against the wrong jar is a diff).
 * Then, after <b>each</b> mutation step, three lines — decisions, <b>never</b> hash values, which
 * are not comparable across the two languages by construction (ruling AH,
 * {@code docs/java-quirks.md} #78):
 *
 * <pre>
 * FANOUTSTOP  &lt;bool&gt;   # BatchFanout.java:152-156's currentBoardHash.equals(lastBoardHash)
 * CONTAINS    &lt;bool&gt;   # BoardHistory.contains (BoardHistory.java:88-101) against a 5-entry history
 * RANK        &lt;int&gt;    # BoardHistory.getRank  (BoardHistory.java:173-186) against the same history
 * </pre>
 *
 * <h2>The mutation script</h2>
 *
 * <p>A shared xorshift64 stream (the {@code p2t15} generator, drawn call for call by
 * {@code p7t10.rs}) rolls one action per step: insert a trace, remove a trace, insert a via, move
 * a via, insert an obstacle area, re-fix an item, restore an earlier snapshot, or <b>no-op</b> —
 * the last is the step that must make {@code FANOUTSTOP} true, and the restore is what makes
 * {@code CONTAINS} true and {@code RANK} a real position rather than {@code -1}.
 *
 * <p>The history is a <b>5</b>-entry {@code BoardHistory} (the package-private cap constructor,
 * reached reflectively — see below), seeded from the board as it stands after each of the first
 * five steps; the same five boards are kept as deep copies so a later step can restore one.
 *
 * <h2>{@code warm} versus {@code raw}, and quirk #200</h2>
 *
 * <p>{@code getHash()} is an MD5 over {@code serialize(true)}, which writes {@code board.itemList}
 * — every {@code Item} with every <b>non-transient</b> field. {@code DrillItem.center}
 * (DrillItem.java:28) and the three {@code precalculated*} memos (:34-46) are such fields and are
 * filled <b>on demand</b>, so in the jar the first call that asks a pin where it is moves the
 * board's hash without changing the board ({@code docs/java-quirks.md} #200, measured in Plan 7
 * Task 2). Java's own {@code readObject} fills them too, by reinserting every item into a fresh
 * search tree, so a {@code deepCopy} of a board can hash differently from the board it copied.
 *
 * <p>{@code mode = warm} (the default, and the acceptance mode) calls {@link #normalizeByProducts} once
 * before the history is seeded: every drill item is asked for its centre and its layer span, so
 * every one of those caches is already filled and none of them can move again. What the driver
 * then measures is the <b>structural</b> decision, which is what ruling AH's parity gate is about.
 * {@code mode = raw} skips the warm-up and is the measurement of quirk #200's own exposure — its
 * diffs are Java-side instability, recorded as {@code XDIFF} rows with the quirk cited, not port
 * bugs.
 *
 * <h2>Two deviations from the plan's Task 3 text, both Java-wins</h2>
 *
 * <ol>
 *   <li><b>The package is {@code app.freerouting.autoroute.maze}, not {@code app.freerouting
 *       .autoroute}.</b> The brief's package would buy the package-private
 *       {@code BoardHistory(ScoringSettings, int)} constructor; {@code setAccessible(true)} buys
 *       it from any package on the classpath (unnamed module), which is what {@code P7T2Probe}
 *       already established. <em>This</em> package buys {@link P6T1}'s package-private
 *       {@code loadBoard} / {@code pickConnections} / {@code route} statics, which the driver
 *       needs so a routed fixture is routed <b>exactly</b> the way {@code p6t1} routes one.
 *       {@code P6T1.java} is compiled alongside (see {@code run.sh}'s {@code extra_jar_sources}).
 *   <li><b>There is no budget to disable.</b> The brief says to run with "{@code
 *       -Dfreerouting.opt_changed_area_ms=0}, the property the driver header documents". No such
 *       system property exists at HEAD — {@code TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP} is a
 *       {@code private static final int} in four {@code autoroute/pipeline} classes and
 *       {@code opt_changed_area_ms} appears only inside a log string
 *       (BatchAutorouter.java:226) — and, more to the point, nothing on this driver's path
 *       reads a clock: it loads a board, optionally routes {@code routeK} connections through
 *       plan-6 ruling 2's seam (which {@code p6t1} pins with no budget knob of its own), and then
 *       only inserts, removes and moves items. Ruling AI's requirement is satisfied vacuously,
 *       exactly as it is for {@code P7T2Probe}.
 * </ol>
 *
 * <h2>{@code -XX:hashCode=2} makes this driver quadratic, and why that is nobody's bug</h2>
 *
 * <p>{@code getHash()} serializes the board, and {@code ObjectOutputStream}'s back-reference
 * {@code HandleTable} buckets its entries by {@code System.identityHashCode}. The harness's shared
 * {@code -XX:hashCode=2} pins that to the <b>constant 1</b>, so every insert lands in one bucket
 * and {@code HandleTable.lookup} degenerates to a linear scan of everything written so far.
 * Measured on {@code tutorial_board.dsn} (439 items, large component-outline polygons): <b>6 s</b>
 * for 40 steps at {@code -XX:hashCode=0}, and <b>not past step 19 in 150 s</b> at
 * {@code -XX:hashCode=2}, with {@code jstack} showing {@code main} RUNNABLE inside
 * {@code ObjectOutputStream$HandleTable.lookup}. It is a JDK/flag interaction: the real jar runs
 * on a default JVM, where the table hashes properly.
 *
 * <p>None of this driver's three decisions depends on {@code Object.hashCode} — they are string
 * equalities between MD5 digests and positions in an {@code ArrayList} — and {@code run.sh}
 * therefore accepts {@code P7T10_HASH_MODE} to override the flag, with the README recording a
 * mode-0..4 sweep that <b>shows</b> the decisions are identical rather than asserting it. Run the
 * big stems with {@code P7T10_HASH_MODE=0}.
 *
 * <p>Compiled and run by {@code scripts/differential/run.sh p7t10}; the flags are the shared set
 * ({@code -Djava.awt.headless=true -Duser.language=en -Duser.country=US
 * -XX:+UnlockExperimentalVMOptions -XX:hashCode=${P7T10_HASH_MODE:-2}}).
 */
public final class P7T10 {

  /** Java's own cap for this driver's history — small, so `getRank` has a short list to scan. */
  private static final int HISTORY_CAP = 5;

  /** The half width every trace this driver inserts is given. */
  private static final int HALF_WIDTH = 30;

  /** The clearance class every item this driver inserts is given. */
  private static final int CLEARANCE_CLASS = 1;

  private P7T10() {}

  // -------------------------------------------------------------------------------------------
  // The shared pseudo-random stream — xorshift64, drawn call for call by `p7t10.rs`
  // -------------------------------------------------------------------------------------------

  private static long state;

  private static long next() {
    state ^= state << 13;
    state ^= state >>> 7;
    state ^= state << 17;
    return state;
  }

  private static int rnd(int bound) {
    return (int) Long.remainderUnsigned(next(), bound);
  }

  // -------------------------------------------------------------------------------------------

  private static RoutingBoard board;
  private static Padstack viaPadstack;
  private static IntBox area;
  private static final List<RoutingBoard> snapshots = new ArrayList<>();

  public static void main(String[] args) throws Exception {
    if (args.length < 2) {
      System.err.println("usage: P7T10 <dsn> <steps> [routeK] [mode]");
      System.exit(2);
    }
    // `FRLogger` writes to stdout on several corpus fixtures; the port has no logger to reproduce
    // them with. Same guard as `P5T1.java` / `P6T1.java` / `P7T7.java`.
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    int steps = Integer.parseInt(args[1]);
    int routeK = args.length > 2 ? Integer.parseInt(args[2]) : 0;
    String mode = args.length > 3 ? args[3] : "warm";
    if (!mode.equals("warm") && !mode.equals("raw")) {
      System.err.println("mode must be `warm` or `raw`");
      System.exit(2);
    }

    Path jar =
        Paths.get(RoutingBoard.class.getProtectionDomain().getCodeSource().getLocation().toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d fixture=%s steps=%d routeK=%d mode=%s%n",
        jar,
        Files.size(jar),
        Files.getLastModifiedTime(jar).toMillis(),
        dsn.getFileName(),
        steps,
        routeK,
        mode);
    System.err.println("java-version " + System.getProperty("java.version"));

    board = P6T1.loadBoard(dsn, null);
    RouterSettings settings = new DefaultSettings().getSettings();
    settings.setLayerCount(board.getLayerCount());
    settings.applyBoardSpecificOptimizations(board);

    // `routeK > 0`: route the first `routeK` connections through exactly `p6t1`'s machinery, so
    // that the decisions below are measured on a board that really is routed. `P7T7.java`'s loop,
    // verbatim.
    if (routeK > 0) {
      for (P6T1.Connection connection : P6T1.pickConnections(board, routeK)) {
        Item item = board.getItem(connection.itemId());
        if (item == null) {
          continue;
        }
        board.startMarkingChangedArea();
        SortedSet<Item> rippedItemList = new TreeSet<>();
        Map<Item, Integer> ripupCosts = new LinkedHashMap<>();
        P6T1.route(board, settings, item, connection.netNo(), rippedItemList, ripupCosts, 1);
      }
    }

    // The via padstack this driver inserts with — its own, appended to the board's library, so
    // the script does not depend on which padstacks a given fixture happens to carry. The library
    // is outside `serialize(true)`'s reach (`Item.board` is transient), so adding it moves no
    // hash.
    if (board.library.padstacks == null) {
      // A DSN with no `(library …)` scope leaves the field `null` — `empty_board.dsn` is the
      // corpus's one such fixture. The port's `BoardLibrary` always owns a `Padstacks`, so this
      // is driver setup, not a divergence: `P2T15.java` builds the same table by hand.
      board.library.padstacks =
          new app.freerouting.core.library.Padstacks(board.layerStructure);
    }
    int layerCount = board.getLayerCount();
    ConvexShape[] viaShapes = new ConvexShape[layerCount];
    for (int i = 0; i < layerCount; i++) {
      viaShapes[i] = new IntBox(-70, -70, 70, 70);
    }
    viaPadstack = board.library.padstacks.add("p7t10-via", viaShapes, true, false);

    // Every coordinate the script draws lands inside the middle half of the board's bounding box,
    // so an inserted item is always somewhere the board actually is.
    IntBox bbox = board.getBoundingBox();
    int dx = (bbox.ur.x - bbox.ll.x) / 4;
    int dy = (bbox.ur.y - bbox.ll.y) / 4;
    area = new IntBox(bbox.ll.x + dx, bbox.ll.y + dy, bbox.ur.x - dx, bbox.ur.y - dy);

    if (mode.equals("warm")) {
      normalizeByProducts(board);
    }

    // The seed is the fixture, not an argument: one stem, one stream.
    state = seedFor(dsn.getFileName().toString(), steps, routeK);

    ScoringSettings scoring = settings.scoring;
    BoardHistory history = newBoardHistory(scoring, HISTORY_CAP);

    // A debugging aid, off in every committed run: with `P7T10_STATE=1` both sides also print a
    // per-step board fingerprint, so a decision diff can be told apart from a board diff.
    boolean withState = "1".equals(System.getenv("P7T10_STATE"));

    String lastHash = null;
    for (int step = 1; step <= steps; step++) {
      mutate();
      if (mode.equals("warm")) {
        normalizeByProducts(board);
      }

      if (withState) {
        out.println(
            "STATE items="
                + board.getItems().size()
                + " traces="
                + board.getTraces().size()
                + " vias="
                + board.getVias().size());
      }
      // BatchFanout.java:152-156. Java breaks out of the fanout loop when the two are equal;
      // this driver records the decision and keeps going, which is the same comparison.
      String currentBoardHash = board.getHash();
      out.println("FANOUTSTOP " + currentBoardHash.equals(lastHash));
      lastHash = currentBoardHash;

      out.println("CONTAINS " + history.contains(board));
      out.println("RANK " + history.getRank(board));

      if (step <= HISTORY_CAP) {
        history.add(board);
        snapshots.add(board.deepCopy());
      }
    }
  }

  /**
   * The package-private {@code BoardHistory(ScoringSettings, int)} (BoardHistory.java:42-45),
   * reached with {@code setAccessible(true)} — see the class comment's deviation 1.
   */
  private static BoardHistory newBoardHistory(ScoringSettings scoring, int cap) throws Exception {
    Constructor<BoardHistory> ctor =
        BoardHistory.class.getDeclaredConstructor(ScoringSettings.class, int.class);
    ctor.setAccessible(true);
    return ctor.newInstance(scoring, cap);
  }

  /**
   * Puts every non-transient field that is a <b>by-product of measurement</b> rather than board
   * state into a canonical value, so that quirk #200 cannot move a hash between two steps that did
   * not change the board. See the class comment. Two halves:
   *
   * <ol>
   *   <li><b>Fill the lazy caches.</b> {@code DrillItem.getCenter} (:229) writes {@code center};
   *       {@code firstLayer} (:162) and {@code lastLayer} (:175) write {@code
   *       precalculatedFirstLayer}/{@code …LastLayer}; {@code smallestRadius} (:216) is what
   *       reaches {@code precalculatedMinWidth}. Filling them is idempotent, so once every item
   *       has been asked, none of the four can move again.
   *   <li><b>Reset the one accumulator.</b> {@code Item.smallestClearance} (Item.java:47) is
   *       {@code public double}, not {@code transient}, and {@code Item.clearanceViolations}
   *       (:451-453) only ever <em>lowers</em> it, guarded by {@code smallestClearance < 0}. So
   *       its value records how many DRC passes have run over the item, not what the item is —
   *       and a {@code BoardHistory.add} runs one ({@code new BoardStatistics(board)} at
   *       BoardHistory.java:198), <b>after</b> the entry's hash has been taken at {@code :197}.
   *       Filling cannot canonicalise a field that never resets, so this resets it to the {@code
   *       -1.0} declaration value instead. Without this, every restore of an earlier snapshot
   *       hashes differently from the history entry that snapshot came from, and Java's {@code
   *       contains} answers {@code false} for a board it demonstrably holds — measured on
   *       {@code Issue143-rpi_splitter.dsn}: 97 of 2 000 steps.
   * </ol>
   */
  private static void normalizeByProducts(RoutingBoard b) {
    for (Item item : b.getItems()) {
      item.smallestClearance = -1.0;
      if (item instanceof DrillItem drill) {
        drill.getCenter();
        drill.firstLayer();
        drill.lastLayer();
        drill.smallestRadius();
      }
    }
  }

  /** A stem-dependent, argument-dependent seed, so two runs of one stem draw the same stream. */
  private static long seedFor(String stem, int steps, int routeK) {
    long h = 0x9E3779B97F4A7C15L;
    for (int i = 0; i < stem.length(); i++) {
      h = h * 1000003L + stem.charAt(i);
    }
    h = h * 1000003L + steps;
    h = h * 1000003L + routeK;
    return h == 0 ? 0x9E3779B97F4A7C15L : h;
  }

  // -------------------------------------------------------------------------------------------
  // The mutation script — every draw below is mirrored draw for draw by `p7t10.rs`
  // -------------------------------------------------------------------------------------------

  private static void mutate() {
    int roll = rnd(100);
    if (roll < 22) {
      insertTrace();
    } else if (roll < 40) {
      removeTrace();
    } else if (roll < 56) {
      insertVia();
    } else if (roll < 68) {
      moveVia();
    } else if (roll < 78) {
      // The no-op: the step that must make FANOUTSTOP true.
      noop();
    } else if (roll < 88) {
      insertObstacle();
    } else if (roll < 96) {
      restoreSnapshot();
    } else {
      refixItem();
    }
  }

  /** Draws the same two ints the Rust twin draws, so a no-op still advances the stream. */
  private static void noop() {
    rnd(2);
    rnd(2);
  }

  private static int randX() {
    return area.ll.x + rnd(Math.max(1, area.ur.x - area.ll.x));
  }

  private static int randY() {
    return area.ll.y + rnd(Math.max(1, area.ur.y - area.ll.y));
  }

  private static int randLayer() {
    return rnd(board.getLayerCount());
  }

  private static int randNet() {
    return 1 + rnd(2);
  }

  private static void insertTrace() {
    int layer = randLayer();
    int netNo = randNet();
    IntPoint p1 = new IntPoint(randX(), randY());
    IntPoint p2 = new IntPoint(randX(), randY());
    if (p1.equals(p2)) {
      // `insertTraceWithoutCleaning` refuses a closed unfixed trace (BasicBoard.java:191-195);
      // both sides skip it rather than relying on that.
      return;
    }
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {p1, p2}),
        layer,
        HALF_WIDTH,
        new int[] {netNo},
        CLEARANCE_CLASS,
        FixedState.UNFIXED);
  }

  private static void removeTrace() {
    List<Trace> traces = new ArrayList<>(board.getTraces());
    if (traces.isEmpty()) {
      rnd(2);
      return;
    }
    // `getTraces()` walks `getItems()`, i.e. descending item id (quirk #63), on both sides.
    board.removeItem(traces.get(rnd(traces.size())));
  }

  private static void insertVia() {
    IntPoint centre = new IntPoint(randX(), randY());
    int netNo = randNet();
    boolean attach = rnd(2) == 0;
    try {
      board.insertVia(
          viaPadstack, centre, new int[] {netNo}, CLEARANCE_CLASS, FixedState.UNFIXED, attach);
    } catch (RuntimeException exception) {
      // `insertVia` splits every trace it lands on (BasicBoard.java:287-293) and a split can
      // throw quirk #22's `ArrayIndexOutOfBoundsException`; the board is then left part-split.
      // The port's `insert_via` answers `Err` at the same line and leaves the same board, so
      // both sides swallow it and carry on rather than describing different boards.
    }
  }

  private static void moveVia() {
    List<Via> vias = new ArrayList<>(board.getVias());
    if (vias.isEmpty()) {
      rnd(2);
      rnd(2);
      rnd(2);
      return;
    }
    Via via = vias.get(rnd(vias.size()));
    int mx = rnd(201) - 100;
    int my = rnd(201) - 100;
    try {
      via.moveBy(new IntVector(mx, my));
    } catch (RuntimeException exception) {
      // `DrillItem.moveBy` inserts a connecting trace per contacting layer (DrillItem.java:113-144)
      // and that insert can fail exactly as above.
    }
  }

  private static void insertObstacle() {
    int layer = randLayer();
    int x = randX();
    int y = randY();
    int w = 100 + rnd(900);
    board.insertObstacle(
        new IntBox(x, y, x + w, y + w), layer, CLEARANCE_CLASS, FixedState.UNFIXED);
  }

  private static void restoreSnapshot() {
    if (snapshots.isEmpty()) {
      rnd(2);
      return;
    }
    int index = rnd(snapshots.size());
    // `BoardHistoryEntry` stores `board.serialize(false)` and `restoreBoard` answers
    // `BasicBoard.deserialize` of it (BoardHistory.java:148); `deepCopy` is the same round trip,
    // so the restored board is hash-identical to the entry the history holds.
    board = snapshots.get(index).deepCopy();
  }

  private static void refixItem() {
    List<Item> items = new ArrayList<>(board.getItems());
    if (items.isEmpty()) {
      rnd(2);
      rnd(2);
      return;
    }
    Item item = items.get(rnd(items.size()));
    // `FixedState` is one of the fields `serialize(true)` writes and one of the fields the
    // widened `structural_hash` covers; flipping it is a board change with no geometry change.
    item.setFixedState(rnd(2) == 0 ? FixedState.UNFIXED : FixedState.SHOVE_FIXED);
  }
}
