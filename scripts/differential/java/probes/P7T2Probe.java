// Plan 7 Task 2 ground-truth probe (controller ruling AF): `autoroute/BoardHistory.java` (202
// lines) — `add`, `clear`, `contains`, `remove`, `getMaxScore`, `restoreBoard`,
// `restoreBestBoard`, `size` and `getRank`, plus the **private nested** `BoardHistoryEntry`
// (`:188-201`) that no public method exposes.
//
// It is not a differential driver — there is no Rust twin and `run.sh` does not know it — but
// every literal in `crates/fr-router/tests/board_history.rs` comes from its stdout, which is
// committed verbatim as `crates/fr-router/tests/data/p7t2-board-history.txt` and replayed
// byte-for-byte by that test.
//
// ## Why the package is `autoroute.maze` and not the brief's `autoroute`
//
// The brief asked for `package app.freerouting.autoroute;`, "to reach the private nested entry by
// reflection". Two facts moved it:
//
//   1. Reflection does not need the package. `BoardHistory.boards` and the nested
//      `BoardHistoryEntry`'s four fields are **private**, so `setAccessible(true)` is required
//      either way, and on the classpath (the unnamed module) that call succeeds from any package.
//      The same call is what reaches the package-private `BoardHistory(ScoringSettings, int)`
//      constructor (`:42-45`) this probe needs for its cap-3 phase.
//   2. `app.freerouting.autoroute.maze` buys something the brief's package does not: `P6T1`'s
//      package-private `loadBoard`, `pickConnections` and `route` statics. The board pool below
//      has to be routed **exactly** the way `p6t1` routes one, or this probe and the port's tests
//      would describe different boards. `P6T1.java` is compiled alongside this file — the
//      `P5T2`/`P5T1` and `P7T7`/`P6T1` pattern, for the same reason.
//
// So the probe is in `autoroute.maze` and reaches every `BoardHistory` internal by reflection.
// Recorded in the task report as a brief deviation.
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p7t2 java/P6T1.java java/probes/P7T2Probe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p7t2:$JAR" \
//       app.freerouting.autoroute.maze.P7T2Probe \
//       ../../../freerouting/fixtures/Issue143-rpi_splitter.dsn 1 \
//     > ../../crates/fr-router/tests/data/p7t2-board-history.txt
//
// (`FRLogger` writes to stdout on this fixture; the probe swaps `System.out` for a null stream
// and prints through its own `PrintStream` on `FileDescriptor.out`, the `P6T1`/`P7T7` guard, so
// no `grep` filter is needed.)
//
// ## The board pool
//
// Seven boards, `B0`, `B1`, `B3`, `B4`, `B5`, `B6`, `B8`: `Bk` is `Issue143-rpi_splitter.dsn`
// with its first `k` connections routed through `P6T1.route` at `ripupPassNo = 1`. Each is built
// by an **independent** load and re-route rather than by snapshotting one board, so the pool does
// not depend on `RoutingBoard.deepCopy` — the very method the port substitutes for Java's
// serialize-clone, and therefore the one thing this probe must not assume. `B1X` is an eighth
// board: a second, independent build of `B1`, so `contains`/`add` can be shown answering on hash
// equality between two boards that are distinct objects. See `POOL_K` for why `k = 2` and `k = 7`
// are left out.
//
// ## What is printed
//
// A `HEADER` line (jar, size, mtime, fixture, arguments — the `p6t1` convention), then the pool,
// then two scripted phases of `BoardHistory` calls: `cap3` on a history built through the
// package-private two-argument constructor with `maxHistorySize = 3`, and `cap30` on the public
// constructor's default. After **every** call the probe prints the history's size and every
// entry's `(hash, score, restoreCount)` **in list order** — which is what makes
// `restoreBoard`'s in-place sort (`:143`) and its `restoreCount++` (`:147`) visible.
//
// Hashes print as **stable labels** `H0`, `H1`, … assigned in order of first appearance, never
// as values: Java's is a hex MD5 of `serialize(true)` and the port's is a `u64`, so only the
// equality *pattern* across entries is comparable (controller ruling AH). Scores print through
// `Float.toString`, which `fr_dsn::java_float_to_string` reproduces exactly.
package app.freerouting.autoroute.maze;

import app.freerouting.autoroute.BoardHistory;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.items.Via;
import app.freerouting.board.trace.PolylineTrace;
import app.freerouting.core.scoring.BoardStatistics;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.IntPoint;
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
import java.lang.reflect.Field;
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

public final class P7T2Probe {

  private P7T2Probe() {}

  /**
   * The pool's routed prefixes: {@code Bk} is the fixture with its first {@code k} connections
   * routed.
   *
   * <p><b>2, 7 and the rest of 0..8 are deliberately absent.</b> On this fixture connections 2 and
   * 3 <em>fail</em> ({@code p6t1}'s reference: {@code k=2 FAILED}, {@code k=3 FAILED}), and a
   * failed attempt inserts nothing but still asks pins for their centre — which fills the
   * <b>non-transient</b> {@code DrillItem.center} cache (DrillItem.java:28) that
   * {@code serialize(true)} writes. So {@code B1} and {@code B2} carry <b>byte-identical item
   * lists</b> (38 items, the same 38 ids, the same geometry) and <b>different</b>
   * {@code getHash()} values, and the port's {@code Board::structural_hash} — which hashes the
   * item graph, not a lazily-populated cache — calls them equal. That is a real
   * hash-decision divergence, it is recorded as a quirk row and as a Task 3 (ruling AH) input, and
   * it is <b>not</b> what this probe is for: keeping the pair in the pool would make every later
   * line of this transcript a diff about {@code structural_hash} rather than about
   * {@code BoardHistory}. {@code B5} and {@code B6} <em>are</em> kept as a pair, because there the
   * two languages agree they are the same board (connection 6 inserts nothing and burns no ids).
   */
  private static final int[] POOL_K = {0, 1, 3, 4, 5, 6, 8};

  /** `H0`, `H1`, … in order of first appearance — ruling AH's equality pattern. */
  private static final Map<String, String> HASH_LABELS = new LinkedHashMap<>();

  private static PrintStream out;
  private static ScoringSettings scoring;
  private static int callNo;

  public static void main(String[] args) throws Exception {
    if (args.length < 1) {
      System.err.println("usage: P7T2Probe <dsn> [ripupPassNo]");
      System.exit(2);
    }
    out = new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    int ripupPassNo = args.length > 1 ? Integer.parseInt(args[1]) : 1;

    Path jar =
        Paths.get(RoutingBoard.class.getProtectionDomain().getCodeSource().getLocation().toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d fixture=%s ripupPassNo=%d MAX_HISTORY_SIZE=%d%n",
        jar,
        Files.size(jar),
        Files.getLastModifiedTime(jar).toMillis(),
        dsn.getFileName(),
        ripupPassNo,
        BoardHistory.MAX_HISTORY_SIZE);
    System.err.println("java-version " + System.getProperty("java.version"));

    scoring = new DefaultSettings().getSettings().scoring;

    // --- the pool -------------------------------------------------------------------------
    out.println("=== boards ===");
    RoutingBoard[] b = new RoutingBoard[POOL_K.length];
    for (int i = 0; i < b.length; i++) {
      b[i] = buildBoard(dsn, POOL_K[i], ripupPassNo);
      describeBoard("B" + POOL_K[i], b[i]);
    }
    RoutingBoard b1x = buildBoard(dsn, 1, ripupPassNo);
    describeBoard("B1X", b1x);

    // --- phase 1: the package-private cap-3 constructor ------------------------------------
    out.println("=== phase cap3 ===");
    callNo = 0;
    Constructor<BoardHistory> ctor =
        BoardHistory.class.getDeclaredConstructor(ScoringSettings.class, int.class);
    ctor.setAccessible(true);
    BoardHistory h = ctor.newInstance(scoring, 3);

    // The empty history: `getMaxScore` answers 0 rather than -inf (`:118`), `restoreBestBoard`
    // answers null, `getRank` answers -1 and `contains` answers false.
    call(h, "size", String.valueOf(h.size()));
    call(h, "getMaxScore", f(h.getMaxScore()));
    call(h, "contains(B0)", String.valueOf(h.contains(b[0])));
    call(h, "getRank(B0)", String.valueOf(h.getRank(b[0])));
    restore(h, "restoreBestBoard", h.restoreBestBoard());

    // Under capacity: any board whose hash is new enters, with no score gate at all (`:53`).
    h.add(b[0]);
    call(h, "add(B0)", "-");
    h.add(b[0]);
    call(h, "add(B0) again", "-");
    h.add(b1x);
    call(h, "add(B1X-as-B1-twin)", "-");
    h.add(b[2]);
    call(h, "add(B3)", "-");
    call(h, "contains(B1)", String.valueOf(h.contains(b[1])));
    call(h, "contains(B4)", String.valueOf(h.contains(b[3])));

    // At capacity (3): `add` scores the candidate, scans for the worst entry and only evicts on a
    // strictly better score (`:53-77`). `B3` should beat the worst; `B0` should not.
    call(h, "size", String.valueOf(h.size()));
    h.add(b[3]);
    call(h, "add(B4) at capacity", "-");
    h.add(b[0]);
    call(h, "add(B0) at capacity", "-");
    call(h, "contains(B0)", String.valueOf(h.contains(b[0])));
    call(h, "getMaxScore", f(h.getMaxScore()));
    call(h, "getRank(B1)", String.valueOf(h.getRank(b[1])));
    call(h, "getRank(B3)", String.valueOf(h.getRank(b[2])));
    call(h, "getRank(B4)", String.valueOf(h.getRank(b[3])));

    // `restoreBoard` sorts the list in place, descending by score, and increments the chosen
    // entry's `restoreCount` (quirk labels: `:143` and `:147`).
    RoutingBoard r1 = h.restoreBoard(0);
    restore(h, "restoreBoard(0)", r1);
    call(h, "getRank(restored#1)", String.valueOf(h.getRank(r1)));
    RoutingBoard r2 = h.restoreBoard(1);
    restore(h, "restoreBoard(1)", r2);
    RoutingBoard r3 = h.restoreBoard(1);
    restore(h, "restoreBoard(1) again", r3);
    RoutingBoard r4 = h.restoreBoard(1);
    restore(h, "restoreBoard(1) a third time", r4);
    RoutingBoard r5 = h.restoreBoard(-7);
    restore(h, "restoreBoard(-7) is unlimited", r5);
    restore(h, "restoreBestBoard", h.restoreBestBoard());

    h.remove(b[1]);
    call(h, "remove(B1)", "-");
    h.remove(b[1]);
    call(h, "remove(B1) again", "-");
    call(h, "getRank(B1)", String.valueOf(h.getRank(b[1])));
    h.clear();
    call(h, "clear", "-");
    call(h, "getMaxScore", f(h.getMaxScore()));
    restore(h, "restoreBestBoard", h.restoreBestBoard());

    // --- phase 2: the public constructor's default cap --------------------------------------
    out.println("=== phase cap30 ===");
    callNo = 0;
    BoardHistory big = new BoardHistory(scoring);
    for (int i = 0; i < b.length; i++) {
      big.add(b[i]);
      call(big, "add(B" + POOL_K[i] + ")", "-");
    }
    call(big, "getMaxScore", f(big.getMaxScore()));
    RoutingBoard best = big.restoreBestBoard();
    restore(big, "restoreBestBoard", best);
    call(big, "getRank(best)", String.valueOf(big.getRank(best)));
    dumpItems(best);
    big.clear();
    call(big, "clear", "-");

    // --- phase 3: the eviction gate's `<=` -----------------------------------------------------
    //
    // `add`'s `:73` is `if (newScore <= worstScore) return;` — a board that **ties** the worst
    // entry is refused, not swapped in. Nothing in the pool ties, so the tie is built here:
    // `B1F` is `B1` with its highest-id trace marked `USER_FIXED`, which changes `getHash()`
    // (`fixedState` is a non-transient `Item` field, so `serialize(true)` writes it) and changes
    // **no** input of `calculateScore` (`:599-613` reads incompletes, violations, bends, trace
    // length, vias and `maximumCount` — none of them fixed-state dependent). So `B1F` is a
    // different board with an identical score, which is exactly the `<=` case.
    out.println("=== phase tie ===");
    callNo = 0;
    RoutingBoard b1f = buildBoard(dsn, 1, ripupPassNo);
    for (Item item : b1f.getItems()) {
      if (item instanceof PolylineTrace) {
        item.setFixedState(FixedState.USER_FIXED);
        break;
      }
    }
    describeBoard("B1F", b1f);
    BoardHistory tie = ctor.newInstance(scoring, 1);
    tie.add(b[1]);
    call(tie, "add(B1) at cap 1", "-");
    tie.add(b1f);
    call(tie, "add(B1F), an equal score and a new hash", "-");
    call(tie, "contains(B1F)", String.valueOf(tie.contains(b1f)));
    call(tie, "getRank(B1F)", String.valueOf(tie.getRank(b1f)));

    // --- phase 4: `Float.compare`, the comparator `restoreBoard:143` sorts with ---------------
    out.println("=== phase floatcompare ===");
    float negNaN = Float.intBitsToFloat(0xffc00000);
    float[][] pairs = {
      {1.0f, 2.0f},
      {2.0f, 1.0f},
      {1.0f, 1.0f},
      {0.0f, -0.0f},
      {-0.0f, 0.0f},
      {-0.0f, -0.0f},
      {Float.NaN, 1.0f},
      {1.0f, Float.NaN},
      {Float.NaN, Float.NaN},
      {Float.NaN, Float.POSITIVE_INFINITY},
      {negNaN, 1.0f},
      {1.0f, negNaN},
      {negNaN, Float.NEGATIVE_INFINITY},
      {Float.NEGATIVE_INFINITY, Float.POSITIVE_INFINITY},
    };
    for (float[] pair : pairs) {
      out.printf(
          "compare bits(%08x,%08x) = %d%n",
          Float.floatToRawIntBits(pair[0]),
          Float.floatToRawIntBits(pair[1]),
          Float.compare(pair[0], pair[1]));
    }

    // --- phase 5: `src/test/java/app/freerouting/autoroute/BoardHistoryTest.java` --------------
    javaTest(dsn.getParent());
  }

  // -------------------------------------------------------------------------------------------
  // `BoardHistoryTest.java`'s six methods, replayed against the jar (plan ruling 14)
  // -------------------------------------------------------------------------------------------

  /**
   * The six {@code BoardHistoryTest} methods, in declaration order, on that suite's own two
   * fixtures — {@code fixtures/empty_board.dsn} and {@code fixtures/Issue159-setonix_2hp-pcb.dsn}.
   * The suite's {@code @BeforeEach} reloads both boards before every method, so this does too:
   * {@code new BoardStatistics(board)} mutates the board it measures, and a shared board would
   * make the sixth method depend on the first five.
   */
  private static void javaTest(Path fixtures) throws Exception {
    out.println("=== phase javatest ===");
    callNo = 0;

    // `addAndRestoreBoard` (:53-66).
    Pair p = setUp(fixtures);
    BoardHistory h = new BoardHistory(scoring);
    h.add(p.board1);
    call(h, "addAndRestoreBoard: add(board1)", "-");
    RoutingBoard restored = h.restoreBestBoard();
    restore(h, "addAndRestoreBoard: restoreBestBoard", restored);
    out.printf("  notSameInstance=%b%n", restored != p.board1);
    out.printf("  sameHashAsBoard1=%b%n", restored.getHash().equals(p.board1.getHash()));

    // `restoreBestBoardFromMultiple` (:68-81).
    p = setUp(fixtures);
    h = new BoardHistory(scoring);
    h.add(p.board1);
    call(h, "restoreBestBoardFromMultiple: add(board1)", "-");
    h.add(p.board2);
    call(h, "restoreBestBoardFromMultiple: add(board2)", "-");
    RoutingBoard best = h.restoreBestBoard();
    restore(h, "restoreBestBoardFromMultiple: restoreBestBoard", best);
    out.printf("  sameHashAsBoard1=%b%n", best.getHash().equals(p.board1.getHash()));

    // `contains` (:83-90).
    p = setUp(fixtures);
    h = new BoardHistory(scoring);
    h.add(p.board1);
    call(h, "contains: add(board1)", "-");
    call(h, "contains: contains(board1)", String.valueOf(h.contains(p.board1)));
    call(h, "contains: contains(board2)", String.valueOf(h.contains(p.board2)));

    // `clear` (:92-101).
    p = setUp(fixtures);
    h = new BoardHistory(scoring);
    h.add(p.board1);
    h.add(p.board2);
    call(h, "clear: add(board1) add(board2)", String.valueOf(h.size()));
    h.clear();
    call(h, "clear: clear", String.valueOf(h.size()));

    // `sizeCapNeverExceedsMaxHistorySize` (:103-115).
    p = setUp(fixtures);
    h = new BoardHistory(scoring);
    h.add(p.board1);
    h.add(p.board2);
    h.add(p.board1);
    call(h, "sizeCapNeverExceedsMaxHistorySize", String.valueOf(h.size()));
    out.printf("  withinCap=%b%n", h.size() <= BoardHistory.MAX_HISTORY_SIZE);

    // `sizeCapEvictsWorstEntry` (:117-141) — the package-private cap-1 constructor.
    p = setUp(fixtures);
    Constructor<BoardHistory> ctor =
        BoardHistory.class.getDeclaredConstructor(ScoringSettings.class, int.class);
    ctor.setAccessible(true);
    h = ctor.newInstance(scoring, 1);
    h.add(p.board1);
    call(h, "sizeCapEvictsWorstEntry: add(board1)", String.valueOf(h.size()));
    h.add(p.board2);
    call(h, "sizeCapEvictsWorstEntry: add(board2)", String.valueOf(h.size()));
    RoutingBoard survivor = h.restoreBestBoard();
    restore(h, "sizeCapEvictsWorstEntry: restoreBestBoard", survivor);
    out.printf("  sameHashAsBoard1=%b%n", survivor.getHash().equals(p.board1.getHash()));
  }

  private record Pair(RoutingBoard board1, RoutingBoard board2) {}

  /** `BoardHistoryTest.setUp` (:27-50), minus the `SettingsMerger` (this probe's `scoring`). */
  private static Pair setUp(Path fixtures) throws Exception {
    RoutingBoard board1 = P6T1.loadBoard(fixtures.resolve("empty_board.dsn"), null);
    RoutingBoard board2 = P6T1.loadBoard(fixtures.resolve("Issue159-setonix_2hp-pcb.dsn"), null);
    describeBoard("board1", board1);
    describeBoard("board2", board2);
    return new Pair(board1, board2);
  }

  // -------------------------------------------------------------------------------------------
  // The board pool
  // -------------------------------------------------------------------------------------------

  /**
   * `Issue143-rpi_splitter.dsn` with its first {@code k} connections routed through exactly the
   * four choices `P6T1.java` documents. A fresh load and a fresh {@code ItemIdGenerator} per call,
   * so two builds with the same {@code k} are the same board down to the item ids.
   */
  private static RoutingBoard buildBoard(Path dsn, int k, int ripupPassNo) throws Exception {
    RoutingBoard board = P6T1.loadBoard(dsn, null);
    RouterSettings settings = new DefaultSettings().getSettings();
    settings.setLayerCount(board.getLayerCount());
    settings.applyBoardSpecificOptimizations(board);
    if (k == 0) {
      return board;
    }
    // `pickConnections` tests `result.size() >= maxItems` **after** appending, so `k = 0` would
    // still return one connection; the guard above is this probe's, as it is `P7T7`'s.
    for (P6T1.Connection connection : P6T1.pickConnections(board, k)) {
      Item item = board.getItem(connection.itemId());
      if (item == null) {
        continue;
      }
      board.startMarkingChangedArea();
      SortedSet<Item> rippedItemList = new TreeSet<>();
      Map<Item, Integer> ripupCosts = new LinkedHashMap<>();
      P6T1.route(
          board, settings, item, connection.netNo(), rippedItemList, ripupCosts, ripupPassNo);
    }
    return board;
  }

  /**
   * The board's identity line. The score is computed <b>first</b> and the hash second, and that
   * order is load-bearing: {@code getHash()} moves the first time anything asks a {@link Pin} for
   * its centre, because {@code Pin.getCenter} (Pin.java:92-140) lazily fills the
   * <b>non-transient</b> {@code DrillItem.center} (DrillItem.java:28) and {@code serialize(true)}
   * writes {@code itemList}. Measured on this fixture: an unrouted board's hash is
   * {@code c21982e8…} before the first {@code new BoardStatistics(board)} and {@code 0da46bc9…}
   * after, and stable from then on. {@code AutorouteBatchLoop:283-284} scores the board
   * immediately before {@code bh.add(board)}, so the pass loop only ever sees the settled value;
   * this probe takes the same order so the pool's labels are the ones {@code add} will store.
   * Quirk row, and a Task 3 (ruling AH) input: the port's {@code Board::structural_hash} covers
   * the item graph and must not learn to move on a lazily-populated cache.
   */
  private static void describeBoard(String name, RoutingBoard board) {
    String score = f(new BoardStatistics(board).getNormalizedScore(scoring));
    out.printf(
        "  %s hash=%s score=%s items=%d maxId=%d%n",
        name,
        label(board.getHash()),
        score,
        countItems(board),
        board.communication.idGenerator.maxGeneratedId());
  }

  // -------------------------------------------------------------------------------------------
  // Printing
  // -------------------------------------------------------------------------------------------

  /** Java's `Float.toString`; `fr_dsn::java_float_to_string` is the port's. */
  private static String f(float value) {
    return Float.toString(value);
  }

  /** The stable label for a hash value — ruling AH's equality pattern (never the value). */
  private static String label(String hash) {
    return HASH_LABELS.computeIfAbsent(hash, unused -> "H" + HASH_LABELS.size());
  }

  /** One call's line, then the whole list in list order. */
  private static void call(BoardHistory history, String op, String ret) throws Exception {
    out.printf("call=%d %s ret=%s%n", ++callNo, op, ret);
    out.printf("  size=%d%n", history.size());
    List<?> boards = entries(history);
    for (int i = 0; i < boards.size(); i++) {
      Object entry = boards.get(i);
      out.printf(
          "  entry=%d hash=%s score=%s restoreCount=%d%n",
          i,
          label((String) field(entry, "hash")),
          f((Float) field(entry, "score")),
          (Integer) field(entry, "restoreCount"));
    }
  }

  /** A call that answers a board: the call line, the list, then the restored board's identity. */
  private static void restore(BoardHistory history, String op, RoutingBoard restored)
      throws Exception {
    call(history, op, restored == null ? "null" : "board");
    if (restored == null) {
      out.println("  restored=null");
      return;
    }
    // Score first, hash second — `describeBoard`'s order, and for its reason.
    String score = f(new BoardStatistics(restored).getNormalizedScore(scoring));
    out.printf(
        "  restored hash=%s score=%s items=%d maxId=%d%n",
        label(restored.getHash()),
        score,
        countItems(restored),
        restored.communication.idGenerator.maxGeneratedId());
  }

  /** One line per item, in `getItems()` order (descending id, quirk #63). */
  private static void dumpItems(RoutingBoard board) {
    out.printf("  items maxId=%d%n", board.communication.idGenerator.maxGeneratedId());
    for (Item item : board.getItems()) {
      StringBuilder sb = new StringBuilder();
      sb.append("  item id=")
          .append(item.getId())
          .append(" type=")
          .append(item.getClass().getSimpleName())
          .append(" nets=")
          .append(nets(item.netNumbers))
          .append(" cl=")
          .append(item.clearanceClassIndex())
          .append(" fixed=")
          .append(item.getFixedState());
      if (item instanceof PolylineTrace trace) {
        sb.append(" layer=")
            .append(trace.getLayer())
            .append(" hw=")
            .append(trace.getHalfWidth())
            .append(" corners=")
            .append(corners(trace));
      } else if (item instanceof Via via) {
        sb.append(" center=").append(pointOf(via.getCenter()));
      } else if (item instanceof Pin pin) {
        sb.append(" center=").append(pointOf(pin.getCenter()));
      }
      out.println(sb);
    }
  }

  /**
   * The trace's corner list, in `P6T15aProbe.pt`'s rendering: an `IntPoint` corner prints exactly,
   * a rational one prints as `~(<Double.toString x>,<Double.toString y>)` of `cornerApprox`.
   */
  private static String corners(PolylineTrace trace) {
    Polyline p = trace.polyline();
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < p.cornerCount(); i++) {
      if (i > 0) {
        sb.append(",");
      }
      Point corner = p.corner(i);
      if (corner instanceof IntPoint ip) {
        sb.append("(").append(ip.x).append(",").append(ip.y).append(")");
      } else {
        FloatPoint f = p.cornerApprox(i);
        sb.append("~(").append(Double.toString(f.x)).append(",").append(Double.toString(f.y))
            .append(")");
      }
    }
    return sb.append("]").toString();
  }

  private static String pointOf(Point p) {
    FloatPoint f = p.toFloat().round().toFloat();
    return "(" + (int) f.x + "," + (int) f.y + ")";
  }

  private static String nets(int[] netNos) {
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < netNos.length; i++) {
      if (i > 0) {
        sb.append(",");
      }
      sb.append(netNos[i]);
    }
    return sb.append("]").toString();
  }

  private static int countItems(RoutingBoard board) {
    int count = 0;
    for (Item unused : board.getItems()) {
      count++;
    }
    return count;
  }

  // -------------------------------------------------------------------------------------------
  // Reflection into `BoardHistory`'s private state
  // -------------------------------------------------------------------------------------------

  private static List<?> entries(BoardHistory history) throws Exception {
    Field f = BoardHistory.class.getDeclaredField("boards");
    f.setAccessible(true);
    return new ArrayList<>((List<?>) f.get(history));
  }

  private static Object field(Object entry, String name) throws Exception {
    Field f = entry.getClass().getDeclaredField(name);
    f.setAccessible(true);
    return f.get(entry);
  }
}
