package app.freerouting.settings;

import app.freerouting.board.facade.BasicBoard;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.structure.Layer;
import app.freerouting.board.model.structure.LayerStructure;
import app.freerouting.board.state.Communication;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.PolylineShape;
import app.freerouting.geometry.planar.TileShape;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;
import app.freerouting.io.specctra.RulesReader;
import app.freerouting.rules.BoardRules;
import app.freerouting.rules.ClearanceMatrix;
import app.freerouting.settings.sources.ApiSettings;
import app.freerouting.settings.sources.CliSettings;
import app.freerouting.settings.sources.DefaultSettings;
import app.freerouting.settings.sources.DsnFileSettings;
import app.freerouting.settings.sources.EnvironmentVariablesSource;
import app.freerouting.settings.sources.JsonFileSettings;
import app.freerouting.settings.sources.RulesFileSettings;
import app.freerouting.util.gson.GsonProvider;
import java.io.ByteArrayInputStream;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Plan 4 Task 9 differential driver: Java's <em>real</em> headless settings composition, run over
 * the Plan 4 precedence matrix, with the resulting {@link RouterSettings} printed as a normalised
 * {@code path=value} dump that {@code scripts/differential/rust/src/bin/p4t1.rs} reproduces from
 * {@code fr_settings::resolve_headless}.
 *
 * <p>Usage: {@code P4T1 <cases.tsv> <case-index|all> <mode>} with
 *
 * <ul>
 *   <li>{@code 0} — the canonical dump (one {@code path=value} line per field, sorted by path);
 *       {@code <case-index|all>} picks one row or the whole table.
 *   <li>{@code 1} — the same object through {@code GsonProvider.GSON}. <b>Java-only</b> until Plan
 *       4 Task 10 gives the Rust side a Gson-shaped serialiser; {@code p4t1.rs} refuses mode 1
 *       rather than pretending to agree.
 *   <li>{@code 2} — every case, whatever {@code case-index} says ({@code all 0} and
 *       {@code <anything> 2} are the same run).
 * </ul>
 *
 * <p>Every case is preceded by a {@code CASE <id>} line, so a diff names the row that moved.
 *
 * <ul>
 * </ul>
 *
 * <h2>What this transcribes, line by line</h2>
 *
 * <p>Every merge-relevant class below is the real one out of the jar; only the plumbing that wires
 * them together is copied here, because it lives inside {@code Freerouting.main}'s process
 * lifecycle, a {@code RoutingJobScheduler} worker thread and a {@code HeadlessBoardManager} board
 * load. The Java line each statement stands for is quoted at the statement (source authority: the
 * clone's HEAD, plan ruling 7 — the same tree {@code $FREEROUTING_JAR} is built from). Re-checking
 * the transcription is a matter of opening these five ranges and reading down:
 *
 * <ol>
 *   <li>{@code Freerouting.java:1408-1413} — the prototype merger.
 *   <li>{@code Freerouting.java:125-146} — merge #1.
 *   <li>{@code HeadlessBoardManager.java:739-748} — the between-merges board pass, reached from
 *       {@code RoutingJobScheduler.java:93-96} → {@code loadFromSpecctraDsn} → {@code :733}.
 *   <li>{@code RoutingJobScheduler.java:103-170} — merge #2.
 *   <li>{@code RoutingJobScheduler.java:172-186} — the post-merge {@code RulesReader.read} and the
 *       final {@code applyBoardSpecificOptimizations}.
 * </ol>
 *
 * <p>Two deliberate substitutions, both of which remove a dependency on unrelated machinery
 * without changing a settings-visible input:
 *
 * <ul>
 *   <li>The board is built by hand ({@code BProbe.java}'s six-line recipe: 2 000 000 × 1 000 000,
 *       all-signal layers named {@code F.Cu}/{@code In1.Cu}/{@code In2.Cu}/{@code B.Cu}) instead
 *       of being read out of the DSN. {@code applyBoardSpecificOptimizations}
 *       ({@code RouterSettings.java:266-435}) reads exactly three things off a board — the
 *       bounding box, the layer count and each layer's {@code isSignal} — so a synthetic board
 *       pins all three and keeps the aspect-ratio penalties exact. The <em>settings</em> still
 *       come from real DSN fixtures through the real {@code DsnFileSettings}.
 *   <li>{@code job.name} is null in the CLI path, so {@code RoutingJobScheduler.java:175} passes
 *       {@code "board"} as the design name; the header mismatch that produces is non-fatal
 *       ({@code RulesReader.java:100-110} logs and continues), so it is passed verbatim.
 * </ul>
 *
 * <h2>The {@code JsonFileSettings} check</h2>
 *
 * <p>Spec §2 says there is no persistent config file, and the port drops the priority-10 tier
 * accordingly. Rather than assume it, this driver constructs {@code JsonFileSettings} on an empty
 * temporary directory and aborts with {@code JSON_SOURCE_NOT_EMPTY} unless every leaf field of its
 * {@code getSettings()} is null — which is what makes the tier a no-op. (A {@code freerouting.json}
 * in the real user-data directory would otherwise leak into every case, so the path is never
 * {@code GlobalSettings.getUserDataPath()}.)
 */
public final class P4T1 {

  private static final int BOARD_WIDTH = 2_000_000;
  private static final int BOARD_HEIGHT = 1_000_000;
  private static final String[] LAYER_NAMES = {"F.Cu", "In1.Cu", "In2.Cu", "B.Cu"};

  private P4T1() {}

  // -----------------------------------------------------------------------------------------
  // the case table
  // -----------------------------------------------------------------------------------------

  /** One row of {@code scripts/differential/matrix/p4t1-cases.tsv}. */
  private record Case(
      String id,
      String dsn,
      String cliRules,
      String schedulerRules,
      String env,
      String argv,
      String board) {}

  private static List<Case> readCases(Path tsv) throws Exception {
    List<Case> cases = new ArrayList<>();
    for (String line : Files.readAllLines(tsv, StandardCharsets.UTF_8)) {
      if (line.isBlank() || line.startsWith("#")) {
        continue;
      }
      String[] f = line.split("\t", -1);
      if (f.length != 7) {
        throw new IllegalArgumentException("expected 7 columns, got " + f.length + ": " + line);
      }
      cases.add(new Case(f[0], f[1], f[2], f[3], f[4], f[5], f[6]));
    }
    return cases;
  }

  private static byte[] readOptional(String spec, Path fixtures, Path data) throws Exception {
    if ("-".equals(spec)) {
      return null;
    }
    return Files.readAllBytes(resolve(spec, fixtures, data));
  }

  /** {@code D:<name>} is {@code crates/fr-settings/tests/data}; {@code F:<name>} is the corpus. */
  private static Path resolve(String spec, Path fixtures, Path data) {
    if (spec.startsWith("D:")) {
      return data.resolve(spec.substring(2));
    }
    if (spec.startsWith("F:")) {
      return fixtures.resolve(spec.substring(2));
    }
    return fixtures.resolve(spec);
  }

  private static String baseName(String spec) {
    String s = spec.startsWith("D:") || spec.startsWith("F:") ? spec.substring(2) : spec;
    int slash = s.lastIndexOf('/');
    return slash >= 0 ? s.substring(slash + 1) : s;
  }

  private static Map<String, String> parseEnv(String spec) {
    Map<String, String> env = new LinkedHashMap<>();
    if ("-".equals(spec)) {
      return env;
    }
    for (String pair : spec.split(";")) {
      int eq = pair.indexOf('=');
      env.put(pair.substring(0, eq), pair.substring(eq + 1));
    }
    return env;
  }

  private static String[] parseArgv(String spec) {
    return "-".equals(spec) ? new String[0] : spec.split(" ");
  }

  // -----------------------------------------------------------------------------------------
  // main
  // -----------------------------------------------------------------------------------------

  public static void main(String[] args) throws Exception {
    if (args.length < 3) {
      System.err.println("usage: P4T1 <cases.tsv> <case-index|all> <mode 0-2>");
      System.exit(2);
    }
    Path tsv = Paths.get(args[0]).toAbsolutePath().normalize();
    String select = args[1];
    int mode = Integer.parseInt(args[2]);

    // `FRLogger` writes to `System.out` and every source in this path logs freely; bind the
    // driver's own output to the real stdout and silence `System.out` before FRLogger loads,
    // exactly as `P3T15` and the `*Probe.java` golden generators do. `System.err` is left alone
    // so a thrown exception is still distinguishable from a disagreement.
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path fixtures = Paths.get(requireEnv("P4T1_FIXTURES")).toAbsolutePath().normalize();
    Path data = Paths.get(requireEnv("P4T1_DATA")).toAbsolutePath().normalize();

    List<Case> cases = readCases(tsv);

    // --- the header (plan ruling 7's check: the driver must be running against the jar the port
    // is a port of, and the machine-dependent defaults must be pinned to the same number) -----
    Path jar =
        Paths.get(RouterSettings.class.getProtectionDomain().getCodeSource().getLocation().toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d processors=%d cases=%s select=%s mode=%d%n",
        jar,
        Files.size(jar),
        Files.getLastModifiedTime(jar).toMillis(),
        Runtime.getRuntime().availableProcessors(),
        tsv.getFileName(),
        select,
        mode);

    // --- the priority-10 tier is provably empty (spec §2) ------------------------------------
    Path emptyUserData = Files.createTempDirectory("p4t1-user-data");
    emptyUserData.toFile().deleteOnExit();
    JsonFileSettings jsonProbe = new JsonFileSettings(emptyUserData.resolve("freerouting.json"));
    List<String> jsonNonNull = new ArrayList<>();
    dump("", jsonProbe.getSettings(), jsonNonNull, true);
    if (!jsonNonNull.isEmpty()) {
      out.println("JSON_SOURCE_NOT_EMPTY " + jsonNonNull);
      out.flush();
      System.exit(3);
    }
    out.printf("JSON_SOURCE_EMPTY dir=%s%n", "<temp>");

    List<Case> selected =
        (mode == 2 || "all".equals(select))
            ? cases
            : List.of(cases.get(Integer.parseInt(select)));
    for (Case c : selected) {
      out.println("CASE " + c.id());
      emit(out, resolveCase(c, emptyUserData, fixtures, data), mode == 2 ? 0 : mode);
    }
    out.flush();
  }

  private static String requireEnv(String name) {
    String value = System.getenv(name);
    if (value == null || value.isBlank()) {
      throw new IllegalStateException("environment variable " + name + " is not set");
    }
    return value;
  }

  private static void emit(PrintStream out, RouterSettings settings, int mode) {
    if (mode == 1) {
      // Task 10's comparison surface: only the fields Gson round-trips appear, which is itself
      // the thing that task compares. No Rust twin for it yet (see the class comment).
      out.println(GsonProvider.GSON.toJson(settings));
      return;
    }
    List<String> lines = new ArrayList<>();
    dump("", settings, lines, false);
    // `boardSpecificTraceCostsApplied` is `private transient`, so the reflective walk below
    // skips it exactly as `ReflectionUtil.copyFields` does (:226-228); it is observable through
    // its accessor and quirk #127 turns on it, so it is dumped by hand.
    lines.add("boardSpecificTraceCostsApplied=" + fmt(settings.areBoardSpecificTraceCostsApplied()));
    Collections.sort(lines);
    for (String line : lines) {
      out.println(line);
    }
  }

  // -----------------------------------------------------------------------------------------
  // the transcription
  // -----------------------------------------------------------------------------------------

  private static RouterSettings resolveCase(Case c, Path userData, Path fixtures, Path data)
      throws Exception {
    byte[] dsnBytes = readOptional(c.dsn(), fixtures, data);
    byte[] cliRulesBytes = readOptional(c.cliRules(), fixtures, data);
    byte[] schedulerRulesBytes = readOptional(c.schedulerRules(), fixtures, data);

    // `Freerouting.java:1408-1413`: the prototype built from the sources that do not change at
    // runtime. `EnvironmentVariablesSource(Map)` is the public testing constructor (:42) —
    // `System.getenv` cannot be set from inside the JVM.
    SettingsMerger prototype =
        new SettingsMerger(
            new DefaultSettings(), //                                          :1410
            new JsonFileSettings(userData.resolve("freerouting.json")), //      :1411
            new CliSettings(parseArgv(c.argv())), //                           :1412
            new EnvironmentVariablesSource(parseEnv(c.env()))); //             :1413

    // --- merge #1 (`Freerouting.java:125-146`) -----------------------------------------------
    SettingsMerger merger1 = prototype.clone(); //                             :125
    if (dsnBytes != null) {
      merger1.addOrReplaceSources( //                                          :126
          new DsnFileSettings(new ByteArrayInputStream(dsnBytes), baseName(c.dsn()))); //  :127
    }
    if (cliRulesBytes != null) { //                                            :129 (-dr / -de …rules)
      merger1.addOrReplaceSources( //                                          :133
          new RulesFileSettings( //                                            :134
              new ByteArrayInputStream(cliRulesBytes), baseName(c.cliRules()))); //         :135
    }
    RouterSettings settings = merger1.merge(); //                              :146

    // --- the between-merges board pass (`HeadlessBoardManager.java:739-748`) -----------------
    // `RoutingJobScheduler.java:93-96` builds a `HeadlessBoardManager` around the job whose
    // `routerSettings` merge #1 has just set and calls `loadFromSpecctraDsn`, which ends in
    // `applyParsedBoardResult` → `:733 applyRouterSettingsForLoadedBoard()`.
    RoutingBoard board = buildBoard(c, dsnBytes);
    int boardLayerCount = board.getLayerCount(); //                            :741
    if (settings.getLayerCount() != boardLayerCount) { //                      :742
      settings.setLayerCount(boardLayerCount); //                              :743
    }
    settings.applyBoardSpecificOptimizations(board); //                        :745
    // :746-747 `applyCopperToEdgeClearanceOverride` / `applyHoleClearanceOverride` read the
    // settings and write the *board's* rules; they change no setting, so there is nothing here.

    // --- merge #2 (`RoutingJobScheduler.java:103-170`) ----------------------------------------
    SettingsMerger merger2 = prototype.clone(); //                             :103
    if (dsnBytes != null) { //                                                 :105 (isDsn)
      merger2.addOrReplaceSources( //                                          :106
          new DsnFileSettings(new ByteArrayInputStream(dsnBytes), baseName(c.dsn()))); //  :107
    }
    // :113-152 resolve `job.rules ?? -dr ?? adjacent <design>.rules` into `rulesData`; the case
    // table supplies the answer directly, because which of the three branches produced it is
    // `resolve_scheduler_rules_path`'s subject (Task 8), not this driver's.
    if (schedulerRulesBytes != null) { //                                      :154
      merger2.addOrReplaceSources( //                                          :155
          new RulesFileSettings( //                                            :156
              new ByteArrayInputStream(schedulerRulesBytes), baseName(c.schedulerRules())));
    }
    merger2.addOrReplaceSources(new ApiSettings(settings)); //                 :163-166 (priority 70)
    settings = merger2.merge(); //                                             :170

    // --- the post-merge re-apply (`:172-181` → `RulesReader.java:153-157`) --------------------
    if (schedulerRulesBytes != null) { //                                      :172
      String designName = "board"; //                                          :175 (job.name is null)
      RulesReader.read( //                                                     :176
          new ByteArrayInputStream(schedulerRulesBytes), designName, board, settings); //    :177-180
    }

    settings.applyBoardSpecificOptimizations(board); //                        :186
    return settings;
  }

  /**
   * The case's board: either {@code BProbe.java}'s synthetic recipe with the named layer count, or
   * — for the rows whose {@code .rules} file carries {@code (padstack …)}/{@code (class …)} scopes
   * that need a real {@code library} — the fixture read back through the real {@code DsnReader},
   * exactly as {@code RoutingJobScheduler.java:95} reaches it through {@code loadFromSpecctraDsn}.
   */
  private static RoutingBoard buildBoard(Case c, byte[] dsnBytes) {
    if ("dsn".equals(c.board())) {
      String designName = baseName(c.dsn()).replaceAll("\\.dsn$", "");
      BoardReadResult result =
          DsnReader.readBoard(new ByteArrayInputStream(dsnBytes), null, null, designName);
      BasicBoard board;
      if (result instanceof BoardReadResult.Success s) {
        board = s.board();
      } else if (result instanceof BoardReadResult.OutlineMissing o) {
        board = o.board();
      } else {
        throw new IllegalStateException("cannot read " + c.dsn() + ": " + result);
      }
      return (RoutingBoard) board;
    }
    int layerCount = Integer.parseInt(c.board());
    Layer[] layers = new Layer[layerCount];
    for (int i = 0; i < layerCount; i++) {
      // 2 layers are the outer pair, so a `.rules` file naming `F.Cu`/`B.Cu` maps onto both
      // stacks; 4 layers are the whole list.
      String name = layerCount == 2 ? LAYER_NAMES[i == 0 ? 0 : 3] : LAYER_NAMES[i];
      layers[i] = new Layer(name, true);
    }
    LayerStructure layerStructure = new LayerStructure(layers);
    ClearanceMatrix clearanceMatrix = ClearanceMatrix.getDefaultInstance(layerStructure, 10);
    BoardRules boardRules = new BoardRules(layerStructure, clearanceMatrix);
    boardRules.createDefaultNetClass();
    return new RoutingBoard(
        new IntBox(0, 0, BOARD_WIDTH, BOARD_HEIGHT),
        layerStructure,
        new PolylineShape[] {TileShape.getInstance(0, 0, BOARD_WIDTH, BOARD_HEIGHT)},
        0,
        boardRules,
        new Communication());
  }

  // -----------------------------------------------------------------------------------------
  // the normalised dump
  // -----------------------------------------------------------------------------------------

  /**
   * Walks the public, non-static declared fields of {@code value} — the same set {@code
   * ReflectionUtil.copyFields} walks (:221-228), transients included — and appends one {@code
   * path=value} line per leaf. Arrays contribute a {@code <path>.length=N} line plus one line per
   * element, so "null" and "empty" stay distinguishable.
   *
   * @param nonNullOnly when true, only the non-null leaves are appended (the {@code
   *     JsonFileSettings} emptiness check)
   */
  private static void dump(String prefix, Object value, List<String> lines, boolean nonNullOnly) {
    if (value == null) {
      if (!nonNullOnly) {
        lines.add(prefix + "=null");
      }
      return;
    }
    Class<?> type = value.getClass();
    if (type.isArray()) {
      int length = java.lang.reflect.Array.getLength(value);
      if (!nonNullOnly) {
        lines.add(prefix + ".length=" + length);
      }
      for (int i = 0; i < length; i++) {
        dump(prefix + "[" + i + "]", java.lang.reflect.Array.get(value, i), lines, nonNullOnly);
      }
      return;
    }
    if (isLeaf(type)) {
      lines.add(prefix + "=" + fmt(value));
      return;
    }
    for (Field field : type.getDeclaredFields()) {
      if (Modifier.isStatic(field.getModifiers())) {
        continue; // ReflectionUtil.java:221-223
      }
      if (!Modifier.isPublic(field.getModifiers())) {
        continue; // ReflectionUtil.java:226-228 — `pcs` and the private applied flag
      }
      Object child;
      try {
        child = field.get(value);
      } catch (IllegalAccessException e) {
        throw new IllegalStateException(field.toString(), e);
      }
      String path = prefix.isEmpty() ? field.getName() : prefix + "." + field.getName();
      dump(path, child, lines, nonNullOnly);
    }
  }

  private static boolean isLeaf(Class<?> type) {
    return type.isEnum()
        || type == String.class
        || type == Boolean.class
        || type == Integer.class
        || type == Long.class
        || type == Double.class
        || type == Float.class;
  }

  /** {@code Double.toString}/{@code Float.toString} — Plan 3's {@code java_*_to_string} in Rust. */
  private static String fmt(Object value) {
    if (value == null) {
      return "null";
    }
    if (value instanceof Enum<?> e) {
      return e.name();
    }
    if (value instanceof Double d) {
      return Double.toString(d);
    }
    if (value instanceof Float f) {
      return Float.toString(f);
    }
    return String.valueOf(value);
  }
}
