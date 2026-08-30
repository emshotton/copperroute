package app.freerouting.autoroute.maze;

import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.structure.Unit;
import app.freerouting.core.scoring.BoardStatistics;
import app.freerouting.settings.RouterSettings;
import app.freerouting.settings.ScoringSettings;
import app.freerouting.settings.sources.DefaultSettings;
import java.io.FileDescriptor;
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
import java.util.SortedSet;
import java.util.TreeSet;
import java.util.stream.DoubleStream;

/**
 * Plan 7 Task 1 differential driver: {@code core.scoring.BoardStatistics}' computing constructor,
 * {@code isPinEscaped}, {@code calculateScore}, {@code getMaximumScore} and {@code
 * getNormalizedScore}, against the port's {@code fr_router::score}.
 *
 * <p>Usage: {@code P7T7 <dsn> [routeK] [ripupPassNo]}. Defaults: {@code routeK = 0} (score the
 * board as the DSN reader left it), {@code ripupPassNo = 1}.
 *
 * <h2>Why this driver lives in {@code app.freerouting.autoroute.maze}</h2>
 *
 * <p>The task brief asked for {@code package app.freerouting.core.scoring;}. Nothing in that
 * package is package-private — {@code BoardStatistics}, all thirteen DTOs, every field and every
 * method this driver touches are {@code public} — so the brief's package buys no access, while
 * <em>this</em> package does: {@link P6T1}'s {@code loadBoard}, {@code pickConnections} and {@code
 * route} are package-private statics, and {@code routeK > 0} has to route a board <b>exactly</b>
 * the way {@code p6t1} routes one or the two harnesses would describe different boards. Same
 * reasoning, and the same mechanism, as {@code P5T2.java} loading its board through {@code
 * P5T1.loadBoard}: {@code P6T1.java} is compiled alongside this file (see {@code run.sh}'s
 * {@code extra_jar_sources}), so the routed board under the statistics cannot drift from the one
 * {@code p6t1} pins connection by connection.
 *
 * <h2>What is printed</h2>
 *
 * <p>Line 1 is {@code HEADER …} — the jar, its size and mtime, the fixture and the arguments; the
 * {@code p4t1}/{@code p5t1}/{@code p6t1} convention, so a run against the wrong jar is a diff
 * rather than a silent pass.
 *
 * <p>Then, per <b>variant</b> — the four constructor argument combinations that have live callers:
 *
 * <ul>
 *   <li>{@code A} = {@code new BoardStatistics(board)} (`:84-86`), the one every score reader uses;
 *   <li>{@code B} = {@code new BoardStatistics(board, null, false)} (`:100-102`), which is what
 *       {@code BatchFanout.java:158-162} builds — clearance violations skipped;
 *   <li>{@code C} = {@code new BoardStatistics(board, Unit.MIL, true, true)}, the only combination
 *       that reaches the {@code unit != board.communication.unit} conversion block (`:377-405`);
 *   <li>{@code D} = {@code new BoardStatistics(board, null, true, false)} — {@code
 *       includeConnections = false}, so {@code connections.maximumCount} and {@code
 *       incompleteCount} stay <b>null</b>. No score is printed for {@code D}: {@code
 *       getMaximumScore} would unbox a null {@code Integer} and throw.
 * </ul>
 *
 * <p>every field of every DTO, one {@code key=value} line each, rendered by {@code
 * Integer.toString} / {@code Float.toString} / {@code Double.toString} — never a re-parsed number,
 * so the exact rendering is the comparison surface (the {@code p6t1} convention). A null boxed
 * field prints the token {@code null}.
 *
 * <p>Then, for A, B and C, {@code calculateScore}, {@code getMaximumScore} and {@code
 * getNormalizedScore} under <b>four</b> {@code ScoringSettings} presets: the default one {@code
 * DefaultSettings} produces, and one each with {@code unroutedNetPenalty}, {@code viaCosts} and
 * {@code bendPenalty} moved off their defaults.
 *
 * <p>Finally {@code isPinEscaped} for every SMD pin of the board, in ascending item id.
 */
public final class P7T7 {

  private P7T7() {}

  public static void main(String[] args) throws Exception {
    if (args.length < 1) {
      System.err.println("usage: P7T7 <dsn> [routeK] [ripupPassNo]");
      System.exit(2);
    }
    // `FRLogger` writes to stdout on several corpus fixtures; the port has no logger to reproduce
    // them with. Same guard as `P5T1.java`/`P6T1.java`.
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    int routeK = args.length > 1 ? Integer.parseInt(args[1]) : 0;
    int ripupPassNo = args.length > 2 ? Integer.parseInt(args[2]) : 1;

    Path jar =
        Paths.get(
                RoutingBoard.class.getProtectionDomain().getCodeSource().getLocation().toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d fixture=%s routeK=%d ripupPassNo=%d%n",
        jar, Files.size(jar), Files.getLastModifiedTime(jar).toMillis(),
        dsn.getFileName(), routeK, ripupPassNo);
    System.err.println("java-version " + System.getProperty("java.version"));

    RoutingBoard board = P6T1.loadBoard(dsn, null);
    RouterSettings settings = new DefaultSettings().getSettings();
    settings.setLayerCount(board.getLayerCount());
    settings.applyBoardSpecificOptimizations(board);

    // `routeK > 0`: route the first `routeK` connections through exactly `p6t1`'s machinery, so
    // that the trace / via / bend / weighted-length blocks are exercised on a board that really is
    // routed rather than on one the DSN reader just built.
    // `pickConnections` tests `result.size() >= maxItems` **after** appending, so a `maxItems` of
    // 0 would still return one connection; the guard is this driver's, not Java's.
    List<P6T1.Connection> connections =
        routeK > 0 ? P6T1.pickConnections(board, routeK) : List.of();
    for (P6T1.Connection connection : connections) {
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

    emit(out, "A", new BoardStatistics(board), settings, true);
    emit(out, "B", new BoardStatistics(board, null, false), settings, true);
    emit(out, "C", new BoardStatistics(board, Unit.MIL, true, true), settings, true);
    emit(out, "D", new BoardStatistics(board, null, true, false), settings, false);

    emitSynthetic(out);
    emitKahan(out);

    List<Pin> smdPins = new ArrayList<>(board.getSmdPins());
    smdPins.sort(java.util.Comparator.comparingInt(Item::getId));
    for (Pin pin : smdPins) {
      out.printf("escaped %d=%b%n", pin.getId(), BoardStatistics.isPinEscaped(pin));
    }
  }

  // -----------------------------------------------------------------------------------------
  // The synthetic cases: `float` narrowing, and the two arithmetic edges
  // -----------------------------------------------------------------------------------------

  /**
   * One hand-built {@code BoardStatistics} — the public no-argument constructor plus field
   * assignment, which is what {@code BoardStatistics()} (`:81-82`) exists for.
   */
  private record Synth(
      String tag,
      int maximumCount,
      int incompleteCount,
      int violationCount,
      int bendCount,
      float totalLengthMm,
      int viaCount,
      float unroutedNetPenalty,
      float clearanceViolationPenalty,
      float bendPenalty,
      double traceCost,
      int viaCosts) {}

  /**
   * Four cases the corpus cannot produce, each pinning one width decision that a plausible
   * mis-transcription would lose:
   *
   * <ul>
   *   <li>{@code S0} — {@code maximumCount = 2^24 + 1}, the first integer a {@code float} cannot
   *       hold, against a unit penalty: {@code getMaximumScore} rounds it and an {@code f64}
   *       intermediate would not;
   *   <li>{@code S1} — three penalty terms whose {@code float} running sum differs from the same
   *       sum computed in {@code double} and narrowed once;
   *   <li>{@code S2} — {@code maximumCount = 0}, i.e. {@code getNormalizedScore}'s
   *       {@code maximumScore <= 0f} guard (`:626-633`), which returns {@code 0f} and never
   *       reaches the division;
   *   <li>{@code S3} — {@code vias.totalCount * viaCosts} = 3e9, an {@code Integer * Integer}
   *       product that <b>overflows int and wraps negative</b> before it is widened to
   *       {@code double} at `:613`, so the "cost" term *raises* the score;
   *   <li>{@code S4} — a tiny negative {@code calculateScore} over a huge {@code maximumScore},
   *       so the quotient at `:634` <b>underflows to {@code -0.0f}</b>. {@code Math.max}'s
   *       signed-zero clause turns it back into {@code +0.0f}, where an {@code f32::max} would
   *       be free to return either and {@code Float.toString} renders the two differently;
   *   <li>{@code S5} — {@code maximumScore} and {@code penalties} both overflow {@code float} to
   *       {@code Infinity}, so {@code calculateScore} is {@code Inf - Inf = NaN} and
   *       {@code Math.max(0, NaN)} is <b>NaN</b>, not 0 — the guard at `:626` does not fire
   *       because {@code Infinity <= 0f} is false.
   * </ul>
   */
  private static final List<Synth> SYNTHETIC =
      List.of(
          new Synth("S0", 16_777_217, 1, 3, 7, 1.1f, 5, 1.0f, 0.1f, 0.1f, 0.1, 3),
          new Synth("S1", 3, 3, 11, 129, 12345.678f, 17, 1.0E7f, 1.5f, 0.75f, 3.3, 42),
          new Synth("S2", 0, 4, 0, 0, 2.5f, 1, 5.0E6f, 1.0f, 1.0f, 1.0, 50),
          new Synth("S3", 7, 0, 0, 0, 0.0f, 3000, 1.0f, 1.0f, 1.0f, 1.0, 1_000_000),
          new Synth("S4", 1, 1, 0, 0, 1.0E-40f, 0, 3.4E38f, 1.0f, 1.0f, 1.0, 1),
          new Synth("S5", 2, 2, 0, 0, 0.0f, 0, 3.4E38f, 1.0f, 1.0f, 1.0, 1));

  private static void emitSynthetic(PrintStream out) {
    for (Synth synth : SYNTHETIC) {
      BoardStatistics s = new BoardStatistics();
      s.connections.maximumCount = synth.maximumCount();
      s.connections.incompleteCount = synth.incompleteCount();
      s.clearanceViolations.totalCount = synth.violationCount();
      s.bends.totalCount = synth.bendCount();
      s.traces.totalLengthMm = synth.totalLengthMm();
      s.vias.totalCount = synth.viaCount();

      ScoringSettings sc = new ScoringSettings();
      sc.unroutedNetPenalty = synth.unroutedNetPenalty();
      sc.clearanceViolationPenalty = synth.clearanceViolationPenalty();
      sc.bendPenalty = synth.bendPenalty();
      sc.defaultPreferredDirectionTraceCost = synth.traceCost();
      sc.viaCosts = synth.viaCosts();

      p(out, synth.tag(), "calculateScore", f(s.calculateScore(sc)));
      p(out, synth.tag(), "getMaximumScore", f(s.getMaximumScore(sc)));
      p(out, synth.tag(), "getNormalizedScore", f(s.getNormalizedScore(sc)));
    }
  }

  // -----------------------------------------------------------------------------------------
  // The Kahan cases: `DoubleStream.sum()` itself
  // -----------------------------------------------------------------------------------------

  /**
   * {@code BoardStatistics.java:188-189} sums the trace lengths with {@code
   * mapToDouble(...).sum()}, which is <b>not</b> a naive fold: {@code DoublePipeline.sum()}
   * collects through {@code Collectors.sumWithCompensation} (Kahan/Neumaier) and finishes with
   * {@code Collectors.computeFinalSum}, whose first line is {@code summands[0] - summands[1]} —
   * a <b>subtraction</b>, because the compensation slot holds the negated low-order bits.
   *
   * <p>The corpus cannot pin that sign: the sum feeds a {@code (float)} cast one line later
   * (`:189`) and the difference is invisible at {@code float} width on every board. These five
   * vectors pin it at {@code double} width instead. The first three were found by search and each
   * has a compensation of exactly half an ulp of the sum, so {@code sum - c} and {@code sum + c}
   * round to <em>different</em> doubles; {@code K3} drives the {@code isNaN(tmp) &&
   * isInfinite(simpleSum)} arm, which returns the <em>simple</em> sum; {@code K4} is the empty
   * stream.
   */
  private static final double[][] KAHAN = {
    {44646902.244757555, 15114766.05020856, 134419886.7378119},
    {153162863.29820704, 22943764.53161407, 51720560.6157495, 294156676.54173774},
    {29004725.81742059, 21933330.804348517, 86551149.06402807},
    {Double.MAX_VALUE, Double.MAX_VALUE, -Double.MAX_VALUE},
    {},
  };

  private static void emitKahan(PrintStream out) {
    for (int k = 0; k < KAHAN.length; k++) {
      p(out, "K" + k, "sum", Double.toString(DoubleStream.of(KAHAN[k]).sum()));
    }
  }

  // -----------------------------------------------------------------------------------------
  // Rendering
  // -----------------------------------------------------------------------------------------

  private static void emit(
      PrintStream out, String tag, BoardStatistics s, RouterSettings settings, boolean withScore) {
    p(out, tag, "host", s.host);
    p(out, tag, "unit", s.unit);
    p(out, tag, "board.boundingBox.x", f(s.board.boundingBox.x));
    p(out, tag, "board.boundingBox.y", f(s.board.boundingBox.y));
    p(out, tag, "board.boundingBox.width", f(s.board.boundingBox.width));
    p(out, tag, "board.boundingBox.height", f(s.board.boundingBox.height));
    p(out, tag, "board.size.x", f(s.board.size.x));
    p(out, tag, "board.size.y", f(s.board.size.y));
    p(out, tag, "board.size.width", f(s.board.size.width));
    p(out, tag, "board.size.height", f(s.board.size.height));
    p(out, tag, "layers.totalCount", i(s.layers.totalCount));
    p(out, tag, "layers.signalCount", i(s.layers.signalCount));
    p(out, tag, "items.totalCount", i(s.items.totalCount));
    p(out, tag, "items.traceCount", i(s.items.traceCount));
    p(out, tag, "items.viaCount", i(s.items.viaCount));
    p(out, tag, "items.conductionAreaCount", i(s.items.conductionAreaCount));
    p(out, tag, "items.drillItemCount", i(s.items.drillItemCount));
    p(out, tag, "items.pinCount", i(s.items.pinCount));
    p(out, tag, "items.componentOutlineCount", i(s.items.componentOutlineCount));
    p(out, tag, "items.otherCount", i(s.items.otherCount));
    p(out, tag, "components.totalCount", i(s.components.totalCount));
    p(out, tag, "pads.totalCount", i(s.pads.totalCount));
    p(out, tag, "nets.totalCount", i(s.nets.totalCount));
    p(out, tag, "nets.classCount", i(s.nets.classCount));
    p(out, tag, "connections.maximumCount", i(s.connections.maximumCount));
    p(out, tag, "connections.incompleteCount", i(s.connections.incompleteCount));
    p(out, tag, "traces.totalCount", i(s.traces.totalCount));
    p(out, tag, "traces.totalSegmentCount", i(s.traces.totalSegmentCount));
    p(out, tag, "traces.totalLength", f(s.traces.totalLength));
    p(out, tag, "traces.totalLengthMm", f(s.traces.totalLengthMm));
    p(out, tag, "traces.totalWeightedLength", f(s.traces.totalWeightedLength));
    p(out, tag, "traces.averageLength", f(s.traces.averageLength));
    p(out, tag, "traces.totalVerticalLength", f(s.traces.totalVerticalLength));
    p(out, tag, "traces.totalHorizontalLength", f(s.traces.totalHorizontalLength));
    p(out, tag, "traces.totalAngledLength", f(s.traces.totalAngledLength));
    p(out, tag, "bends.totalCount", i(s.bends.totalCount));
    p(out, tag, "bends.ninetyDegreeCount", i(s.bends.ninetyDegreeCount));
    p(out, tag, "bends.fortyFiveDegreeCount", i(s.bends.fortyFiveDegreeCount));
    p(out, tag, "bends.otherAngleCount", i(s.bends.otherAngleCount));
    p(out, tag, "vias.totalCount", i(s.vias.totalCount));
    p(out, tag, "vias.throughHoleCount", i(s.vias.throughHoleCount));
    p(out, tag, "vias.blindCount", i(s.vias.blindCount));
    p(out, tag, "vias.buriedCount", i(s.vias.buriedCount));
    p(out, tag, "clearanceViolations.totalCount", i(s.clearanceViolations.totalCount));
    p(out, tag, "clearanceViolations.minViolationUm", d(s.clearanceViolations.minViolationUm));
    p(out, tag, "clearanceViolations.maxViolationUm", d(s.clearanceViolations.maxViolationUm));
    p(out, tag, "clearanceViolations.avgViolationUm", d(s.clearanceViolations.avgViolationUm));
    p(out, tag, "fanout.totalSmdPins", Integer.toString(s.fanout.totalSmdPins));
    p(out, tag, "fanout.pinsToEscape", Integer.toString(s.fanout.pinsToEscape));
    p(out, tag, "fanout.escapedCount", Integer.toString(s.fanout.escapedCount));

    if (!withScore) {
      return;
    }
    for (int preset = 0; preset < 4; preset++) {
      ScoringSettings sc = preset(settings, preset);
      p(out, tag, "score" + preset + ".calculateScore", f(s.calculateScore(sc)));
      p(out, tag, "score" + preset + ".getMaximumScore", f(s.getMaximumScore(sc)));
      p(out, tag, "score" + preset + ".getNormalizedScore", f(s.getNormalizedScore(sc)));
    }
  }

  /**
   * The four scoring presets: the default weights {@code DefaultSettings} writes, and one each with
   * {@code unroutedNetPenalty}, {@code viaCosts} and {@code bendPenalty} moved off default. The
   * three moved values are deliberately non-round so that a lost {@code float} narrowing shows in
   * the rendered digits.
   */
  private static ScoringSettings preset(RouterSettings settings, int preset) {
    ScoringSettings sc = settings.scoring.clone();
    switch (preset) {
      case 1 -> sc.unroutedNetPenalty = 17.3f;
      case 2 -> sc.viaCosts = 77;
      case 3 -> sc.bendPenalty = 3.25f;
      default -> {}
    }
    return sc;
  }

  private static void p(PrintStream out, String tag, String key, String value) {
    out.printf("%s %s=%s%n", tag, key, value);
  }

  private static String i(Integer value) {
    return value == null ? "null" : Integer.toString(value);
  }

  private static String f(Float value) {
    return value == null ? "null" : Float.toString(value);
  }

  private static String f(float value) {
    return Float.toString(value);
  }

  private static String d(Double value) {
    return value == null ? "null" : Double.toString(value);
  }
}
