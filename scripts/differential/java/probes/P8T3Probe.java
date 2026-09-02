package app.freerouting.management;

import app.freerouting.board.actions.ItemIdGenerator;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.ObstacleArea;
import app.freerouting.board.model.structure.LayerStructure;
import app.freerouting.board.state.Communication;
import app.freerouting.board.model.structure.Unit;
import app.freerouting.core.RoutingJob;
import app.freerouting.geometry.planar.Circle;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.PolylineShape;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;
import app.freerouting.rules.BoardRules;
import app.freerouting.rules.ClearanceMatrix;
import app.freerouting.rules.DefaultItemClearanceClasses;
import app.freerouting.settings.RouterSettings;
import app.freerouting.settings.SettingsMerger;
import app.freerouting.settings.sources.CliSettings;
import app.freerouting.settings.sources.DefaultSettings;
import app.freerouting.settings.sources.DsnFileSettings;
import app.freerouting.settings.sources.EnvironmentVariablesSource;
import app.freerouting.settings.sources.JsonFileSettings;

import java.io.ByteArrayInputStream;
import java.io.File;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.UUID;

// Plan 8 Task 3 ground-truth probe: the **board load sequence** of
// `management/HeadlessBoardManager.java` —
//   `createBoard`                        (:310-344)
//   `loadFromSpecctraDsn`                (:673-705)
//   `applyParsedBoardResult`             (:711-737)
//   `applyRouterSettingsForLoadedBoard`  (:739-749)
//   `applyImmediatePostLoadProcessing`   (:751-757)
// — measured at four points on three fixtures times three `router.hole_clearance_um` values.
//
// It is not a differential driver: there is no Rust twin binary and `run.sh` does not know it,
// the `P7T15bProbe`/`P8T0Probe`/`P8T1Probe`/`P8T2Probe` pattern. Its stdout is committed verbatim
// as `crates/fr-core/tests/data/p8t3-clearance-overrides.txt` and replayed by
// `crates/fr-core/tests/overrides.rs` and `crates/fr-core/tests/load.rs`.
//
// ## The finding this probe exists to settle: `createBoard` is NOT on the load path
//
// The Plan 8 survey (ruling AD) and quirk register row #232 both assert that the two clearance
// overrides run **twice** per DSN load — once from `HeadlessBoardManager.createBoard:342-343`,
// which "the parser invokes at `io/specctra/parser/Structure.java:1268`", and once from
// `applyRouterSettingsForLoadedBoard:746-747`. **That is false at the pinned jar.**
// `Structure.java:1268` calls `scopeParameter.boardHandling.createBoard(...)`, and
// `ReadScopeParameter` has exactly one constructor, which assigns its `final BoardParserCallback
// boardHandling` field `new MinimalBoardManager()` (`ReadScopeParameter.java:103`).
// `MinimalBoardManager.createBoard` (`:139-166`) constructs the `RoutingBoard` and returns; it
// calls neither override, and its `getCurrentRoutingJob()` returns `null`.
// `HeadlessBoardManager.createBoard` has **no caller at all** outside `GuiBoardManager:411`.
//
// The `[createboard]` block below measures that at runtime, with a counting subclass: a full
// `loadFromSpecctraDsn` invokes `HeadlessBoardManager.createBoard` **zero** times. The
// `[stage] name=counterfactual_create_board` rows measure what the two overrides *would* have
// done at that point, so the correction is backed by the numbers rather than by an argument.
//
// ## The four points
//
//   after_create_board   the itemless board the structure scope hands back. Not separately
//                        observable inside a full parse, so it is measured by re-parsing the
//                        DSN truncated to `(pcb <name> (parser ...) (resolution ...)
//                        (structure ...))` — the same code path, stopped one scope later than
//                        `createBoard`. `crates/fr-core/tests/overrides.rs` truncates the same
//                        bytes the same way.
//   after_parser         `DsnReader.readBoard`'s board: every item inserted, no override run.
//   after_router_settings   after `applyRouterSettingsForLoadedBoard` (:739-749), i.e. after
//                        `setLayerCount` / `applyBoardSpecificOptimizations` /
//                        `applyCopperToEdgeClearanceOverride` / `applyHoleClearanceOverride`.
//   after_post_load      after `applyImmediatePostLoadProcessing` (:751-757), i.e. after
//                        `reduceNetsOfRouteItems()`.
//
// plus, on the two fixtures with **zero** circular component keepouts,
//
//   after_second_hole_override   `applyHoleClearanceOverride` invoked a second time on the
//                        loaded board — the quirk-#232 boundary the Plan 8 obligation at
//                        `crates/fr-board/src/board/clearance_override.rs:60` asks for. With
//                        `changed == false` and `holeKeepouts == 0` the second run reaches no
//                        `reinsertTreeItems`, so the tree leaf count and the entry id do not
//                        move.
//
// ## Byte stability
//
// No clock, no wall-time budget, no `HashSet` iteration: every map printed is a `TreeMap` and
// every floating-point value crosses as `Double.toString`. `ShapeSearchTree.lastGeneratedEntryId`
// is a `private static` that accumulates over a JVM, so it is reset to 0 before every measured
// load; without that the transcript would depend on the order the cases run in.
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p8t3 java/probes/P8T3Probe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -cp "/tmp/p8t3:$JAR" app.freerouting.management.P8T3Probe \
//     > ../../crates/fr-core/tests/data/p8t3-clearance-overrides.txt
public final class P8T3Probe {

  private P8T3Probe() {}

  /** One fixture row: the parity stem and the DSN under the Java checkout. */
  private record Row(String stem, String dsn) {}

  /**
   * The three fixtures the task brief names: a KiCad-exported board, `Issue593-BBD_Mars-64.dsn`
   * (the one with circular component keepouts, so the reclassification pass is reached) and the
   * tutorial board.
   */
  private static final List<Row> CORPUS =
      List.of(
          new Row("kicad-ecc83-input", "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn"),
          new Row("issue593-bbd-mars-64", "fixtures/Issue593-BBD_Mars-64.dsn"),
          new Row("tutorial-board", "examples/tutorial_board/tutorial_board.dsn"));

  /**
   * `DEFAULT_HOLE_CLEARANCE_UM` is 0.0, which makes the hole override a no-op on every corpus
   * board, so a probe that only ran the defaults would prove nothing. 100 and 500 µm are the two
   * non-default values Plan 7 Task 15b's variants E and F used.
   */
  private static final double[] HOLE_VALUES = {0.0, 100.0, 500.0};

  /** One measured board state. */
  private static final class State {
    int layerCount;
    int classCount;
    final List<String> classNames = new ArrayList<>();
    int outlineClassIdx = -1;
    int defaultAreaClassNo = -1;
    int holeClearance;
    int itemCount;
    int keepoutCount;
    final Map<Integer, Integer> keepoutClasses = new TreeMap<>();
    /** item id -> clearance class index, for every item on the board. */
    final Map<Integer, Integer> itemClasses = new TreeMap<>();
    int treeLeaves;
    int entryId;
    /** `(objectId, shapeIndexInObject)` for every leaf, in `ShapeTree.toArray()` order. */
    final List<String> treeOrder = new ArrayList<>();
    long matrixSum;
    long[] layerSum;
    long[][] rowSum;
    int[][][] matrix;
  }

  /**
   * A `HeadlessBoardManager` that counts how often the DSN load reaches
   * `HeadlessBoardManager.createBoard` (:310-344). The answer is 0, which is the whole point.
   */
  private static final class CountingManager extends HeadlessBoardManager {
    int createBoardCalls;

    CountingManager(RoutingJob routingJob) {
      super(routingJob);
    }

    @Override
    public void createBoard(
        IntBox boundingBox,
        LayerStructure layerStructure,
        PolylineShape[] outlineShapes,
        String outlineClearanceClassName,
        BoardRules rules,
        Communication boardCommunication) {
      createBoardCalls++;
      super.createBoard(
          boundingBox,
          layerStructure,
          outlineShapes,
          outlineClearanceClassName,
          rules,
          boardCommunication);
    }
  }

  public static void main(String[] argv) throws Exception {
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    String javaDir =
        System.getenv()
            .getOrDefault("FREEROUTING_JAVA_DIR", "/Users/em/Development/freerouting/freerouting");

    out.println("# P8T3Probe — the HeadlessBoardManager board load sequence, HEAD jar");
    out.println(
        "# management/HeadlessBoardManager.java:310-344, :673-705, :711-737, :739-749, :751-757");
    out.println("# no clock, no HashSet iteration: byte-stable across runs");
    out.println(
        "# DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM = "
            + Double.toString(DefaultSettings.DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM));
    out.println(
        "# DEFAULT_HOLE_CLEARANCE_UM = "
            + Double.toString(DefaultSettings.DEFAULT_HOLE_CLEARANCE_UM));
    out.println(
        "# BoardParserCallback implementation used by DsnReader.readBoard = "
            + parserCallbackImplementation());

    for (Row row : CORPUS) {
      File dsn = new File(javaDir, row.dsn());
      byte[] full = Files.readAllBytes(dsn.toPath());
      byte[] truncated = truncateAfterStructureScope(full);
      RouterSettings merged = mergeLikeCli(dsn);

      out.println();
      out.println("[board] stem=" + row.stem() + " dsn=" + row.dsn());
      out.println(
          "merged copper="
              + str(merged.copperToEdgeClearanceUm)
              + " hole="
              + str(merged.holeClearanceUm)
              + " truncated_bytes="
              + truncated.length);

      // ---- the `createBoard` reachability measurement (quirk label AD) --------------------
      CountingManager counting = new CountingManager(newJob(dsn, merged.copperToEdgeClearanceUm, 500.0));
      resetEntryId();
      counting.loadFromSpecctraDsn(
          new ByteArrayInputStream(full), null, new ItemIdGenerator());
      out.println(
          "[createboard] stem="
              + row.stem()
              + " headless_create_board_calls="
              + counting.createBoardCalls
              + " board_loaded="
              + (counting.getRoutingBoard() != null));

      for (double hole : HOLE_VALUES) {
        Double copper = merged.copperToEdgeClearanceUm;
        String caseName = row.stem() + "@hole=" + Double.toString(hole);

        // ---- point 1: after createBoard, i.e. the itemless board -------------------------
        resetEntryId();
        RoutingBoard created = readBoard(truncated, dsn.getName());
        out.println();
        out.println(
            "[case] stem="
                + row.stem()
                + " copper="
                + str(copper)
                + " hole="
                + Double.toString(hole)
                + " copper_units="
                + (copper == null ? "-" : Integer.toString(boardUnits(created, copper)))
                + " hole_units="
                + Integer.toString(boardUnits(created, hole)));
        State createdState = snapshot(created);
        emit(out, "after_create_board", createdState, null);

        // ---- the counterfactual: what `createBoard:342-343` WOULD have done --------------
        RoutingBoard counterfactualBoard = readBoard(truncated, dsn.getName());
        HeadlessBoardManager counterfactual =
            new HeadlessBoardManager(newJob(dsn, copper, hole));
        counterfactual.replaceRoutingBoard(counterfactualBoard);
        invoke(counterfactual, "applyCopperToEdgeClearanceOverride");
        invoke(counterfactual, "applyHoleClearanceOverride");
        emit(
            out,
            "counterfactual_create_board",
            snapshot(counterfactualBoard),
            createdState);

        // ---- points 2-4: the real load, stopped between the steps ------------------------
        resetEntryId();
        HeadlessBoardManager manager = new HeadlessBoardManager(newJob(dsn, copper, hole));
        BoardReadResult result =
            DsnReader.readBoard(
                new ByteArrayInputStream(full), null, new ItemIdGenerator(), dsn.getName());
        RoutingBoard loaded = boardOf(result, dsn.getName());
        manager.replaceRoutingBoard(loaded);
        State parsed = snapshot(loaded);
        emit(out, "after_parser", parsed, null);

        invoke(manager, "applyRouterSettingsForLoadedBoard");
        State settled = snapshot(loaded);
        emit(out, "after_router_settings", settled, parsed);

        invoke(manager, "applyImmediatePostLoadProcessing");
        State post = snapshot(loaded);
        emit(out, "after_post_load", post, settled);

        // ---- the quirk-#232 boundary (scan ruling R8) -------------------------------------
        invoke(manager, "applyHoleClearanceOverride");
        emit(out, "after_second_hole_override", snapshot(loaded), post);
        if (caseName.isEmpty()) {
          throw new IllegalStateException("unreachable");
        }
      }
    }
  }

  // ===============================================================================================
  // measurement
  // ===============================================================================================

  private static String str(Double d) {
    return d == null ? "-" : Double.toString(d);
  }

  /**
   * FNV-1a over the UTF-8 bytes, printed as 16 lowercase hex digits. Chosen because it is four
   * lines in both languages and has no library dependency on either side; it is a fingerprint of
   * the tree order, not a security primitive.
   */
  private static String fnv1a64(String text) {
    long hash = 0xcbf29ce484222325L;
    for (byte b : text.getBytes(StandardCharsets.UTF_8)) {
      hash ^= (b & 0xff);
      hash *= 0x100000001b3L;
    }
    return String.format("%016x", hash);
  }

  /**
   * `HeadlessBoardManager.java:365-372` / `:508-516` — the shared um -> board-unit conversion,
   * transcribed so the transcript pins it independently of what the override then does with it.
   */
  private static int boardUnits(RoutingBoard board, double clearanceUm) {
    int boardResolution = Math.max(1, board.communication.resolution);
    return (int)
        Math.round(Unit.scale(clearanceUm * boardResolution, Unit.UM, board.communication.unit));
  }

  private static State snapshot(RoutingBoard board) {
    State s = new State();
    ClearanceMatrix matrix = board.rules.clearanceMatrix;
    s.layerCount = matrix.getLayerCount();
    s.classCount = matrix.getClassCount();
    for (int i = 0; i < s.classCount; i++) {
      s.classNames.add(matrix.getName(i));
    }
    s.defaultAreaClassNo =
        board
            .rules
            .getDefaultNetClass()
            .defaultItemClearanceClasses
            .get(DefaultItemClearanceClasses.ItemClass.AREA);
    var outline = board.getOutline();
    s.outlineClassIdx = outline == null ? -1 : outline.clearanceClassIndex();
    s.holeClearance = board.rules.getHoleClearance();
    s.treeLeaves = board.searchTreeManager.getDefaultTree().size();
    s.entryId = readEntryId();
    // The internal tree order — the observable quirk #232 claimed a second
    // `reinsertTreeItems` would shift. `ShapeTree.toArray` (ShapeTree.java:66-94) is a
    // left-to-right in-order walk, so the sequence below IS the tree's layout.
    for (app.freerouting.datastructures.ShapeTree.Leaf leaf :
        board.searchTreeManager.getDefaultTree().toArray()) {
      String objectId =
          leaf.object instanceof Item item
              ? Integer.toString(item.getId())
              : leaf.object.getClass().getSimpleName();
      s.treeOrder.add(objectId + ":" + leaf.shapeIndexInObject);
    }

    s.layerSum = new long[s.layerCount];
    s.rowSum = new long[s.layerCount][s.classCount];
    s.matrix = new int[s.layerCount][s.classCount][s.classCount];
    for (int layer = 0; layer < s.layerCount; layer++) {
      for (int i = 0; i < s.classCount; i++) {
        for (int j = 0; j < s.classCount; j++) {
          int value = matrix.getValue(i, j, layer, false);
          s.matrixSum += value;
          s.layerSum[layer] += value;
          s.rowSum[layer][i] += value;
          s.matrix[layer][i][j] = value;
        }
      }
    }

    for (Item item : board.getItems()) {
      s.itemCount++;
      s.itemClasses.put(item.getId(), item.clearanceClassIndex());
      // HeadlessBoardManager.java:412-422: the EXACT class, and a circular area on a component.
      if (item.getClass() != ObstacleArea.class) {
        continue;
      }
      ObstacleArea keepout = (ObstacleArea) item;
      if (keepout.getComponentId() > 0 && keepout.getArea() instanceof Circle) {
        s.keepoutCount++;
        s.keepoutClasses.merge(keepout.clearanceClassIndex(), 1, Integer::sum);
      }
    }
    return s;
  }

  /**
   * One `[stage]` block. `previous` is the state the stage started from; the `reclass=` line lists
   * every item whose clearance class index the stage changed, as `id:old-&gt;new`, ascending by id
   * — which is what `assignHoleKeepoutClearanceClass:455-462` and
   * `applyCopperToEdgeClearanceOverride:545` between them produce.
   */
  private static void emit(PrintStream out, String name, State s, State previous) {
    out.println("[stage] name=" + name);
    out.println(
        "  layers="
            + s.layerCount
            + " classes="
            + s.classCount
            + " names="
            + String.join(",", s.classNames));
    out.println(
        "  outline_class="
            + s.outlineClassIdx
            + " default_area_class="
            + s.defaultAreaClassNo
            + " hole_clearance="
            + s.holeClearance
            + " items="
            + s.itemCount);
    StringBuilder histogram = new StringBuilder();
    for (Map.Entry<Integer, Integer> e : s.keepoutClasses.entrySet()) {
      if (histogram.length() > 0) {
        histogram.append(';');
      }
      histogram.append(e.getKey()).append(':').append(e.getValue());
    }
    out.println("  keepouts=" + s.keepoutCount + " keepout_classes=" + histogram);
    StringBuilder reclass = new StringBuilder();
    if (previous != null) {
      for (Map.Entry<Integer, Integer> e : s.itemClasses.entrySet()) {
        Integer before = previous.itemClasses.get(e.getKey());
        if (before != null && !before.equals(e.getValue())) {
          if (reclass.length() > 0) {
            reclass.append(';');
          }
          reclass.append(e.getKey()).append(':').append(before).append("->").append(e.getValue());
        }
      }
    }
    out.println("  reclassified=" + reclass);
    out.println("  tree_leaves=" + s.treeLeaves + " entry_id=" + s.entryId);
    out.println(
        "  tree_order digest="
            + fnv1a64(String.join(",", s.treeOrder))
            + " head="
            + String.join(",", s.treeOrder.subList(0, Math.min(12, s.treeOrder.size())))
            + " tail="
            + String.join(
                ",", s.treeOrder.subList(Math.max(0, s.treeOrder.size() - 12), s.treeOrder.size())));
    out.println("  matrix_sum=" + s.matrixSum);
    for (int layer = 0; layer < s.layerCount; layer++) {
      StringBuilder rows = new StringBuilder();
      for (int i = 0; i < s.classCount; i++) {
        if (i > 0) {
          rows.append(',');
        }
        rows.append(s.rowSum[layer][i]);
      }
      out.println("  msum L" + layer + " = " + s.layerSum[layer] + " rows=" + rows);
    }
    for (int layer = 0; layer < s.layerCount; layer++) {
      for (int i = 0; i < s.classCount; i++) {
        StringBuilder values = new StringBuilder();
        for (int j = 0; j < s.classCount; j++) {
          if (j > 0) {
            values.append(' ');
          }
          values.append(s.matrix[layer][i][j]);
        }
        out.println("  mrow L" + layer + " " + i + " " + s.classNames.get(i) + " = " + values);
      }
    }
  }

  // ===============================================================================================
  // plumbing
  // ===============================================================================================

  /** Reproduces `Freerouting.initializeCli:125-146` (merge #1) exactly, as `P7T15bProbe` does. */
  private static RouterSettings mergeLikeCli(File dsn) throws Exception {
    SettingsMerger prototype =
        new SettingsMerger(
            new DefaultSettings(),
            new JsonFileSettings(),
            new CliSettings(new String[0]),
            new EnvironmentVariablesSource());
    RoutingJob job = new RoutingJob(UUID.randomUUID());
    job.setInput(dsn.getAbsolutePath());
    SettingsMerger merger = prototype.clone();
    merger.addOrReplaceSources(new DsnFileSettings(job.input.getData(), job.input.getFilename()));
    return merger.merge();
  }

  private static RoutingJob newJob(File dsn, Double copper, Double hole) throws Exception {
    RouterSettings settings = mergeLikeCli(dsn);
    settings.copperToEdgeClearanceUm = copper;
    settings.holeClearanceUm = hole;
    RoutingJob job = new RoutingJob(UUID.randomUUID());
    job.setInput(dsn.getAbsolutePath());
    job.routerSettings = settings;
    return job;
  }

  private static RoutingBoard readBoard(byte[] bytes, String designName) {
    BoardReadResult result =
        DsnReader.readBoard(
            new ByteArrayInputStream(bytes), null, new ItemIdGenerator(), designName);
    return boardOf(result, designName);
  }

  private static RoutingBoard boardOf(BoardReadResult result, String designName) {
    if (result instanceof BoardReadResult.Success success) {
      return (RoutingBoard) success.board();
    }
    if (result instanceof BoardReadResult.OutlineMissing outlineMissing) {
      return (RoutingBoard) outlineMissing.board();
    }
    throw new IllegalStateException(designName + " did not read: " + result);
  }

  /**
   * The DSN cut down to `(pcb &lt;name&gt; ... (structure ...))`: everything up to and including the
   * end of the `(structure ...)` scope, plus the `)` that closes `(pcb`. The structure scope is
   * where `Structure.java:1035` calls `createBoard`, so the board this parses is exactly the
   * board `createBoard` produced — the only way to observe that point without instrumenting the
   * parser.
   *
   * <p>Parenthesis counting skips anything inside a `"`-quoted string, which is what
   * `SpecctraDsnStreamReader` does with the default `(string_quote ")`.
   */
  private static byte[] truncateAfterStructureScope(byte[] dsn) {
    String text = new String(dsn, StandardCharsets.UTF_8);
    int start = text.indexOf("(structure");
    if (start < 0) {
      throw new IllegalStateException("no (structure scope");
    }
    int depth = 0;
    boolean inQuote = false;
    int end = -1;
    for (int i = start; i < text.length(); i++) {
      char c = text.charAt(i);
      if (c == '"') {
        inQuote = !inQuote;
      } else if (!inQuote) {
        if (c == '(') {
          depth++;
        } else if (c == ')') {
          depth--;
          if (depth == 0) {
            end = i + 1;
            break;
          }
        }
      }
    }
    if (end < 0) {
      throw new IllegalStateException("unterminated (structure scope");
    }
    return (text.substring(0, end) + "\n)\n").getBytes(StandardCharsets.UTF_8);
  }

  /** The concrete `BoardParserCallback` `DsnReader.readBoard` always uses. */
  private static String parserCallbackImplementation() throws Exception {
    Class<?> readScopeParameter =
        Class.forName("app.freerouting.io.specctra.parser.ReadScopeParameter");
    Field field = readScopeParameter.getDeclaredField("boardHandling");
    field.setAccessible(true);
    Class<?> minimal =
        Class.forName("app.freerouting.io.specctra.parser.ReadScopeParameter$MinimalBoardManager");
    return minimal.getName()
        + " (field "
        + field.getType().getName()
        + ", assigned once at ReadScopeParameter.java:103)";
  }

  private static void invoke(HeadlessBoardManager manager, String method) throws Exception {
    Method m = HeadlessBoardManager.class.getDeclaredMethod(method);
    m.setAccessible(true);
    m.invoke(manager);
  }

  /**
   * `ShapeSearchTree.lastGeneratedEntryId` (`ShapeSearchTree.java:55`) is a `private static int`
   * that accumulates over the whole JVM. The port's counter is per-`SearchTreeManager`, so the two
   * are only comparable when this one starts each measured load at 0.
   */
  private static void resetEntryId() throws Exception {
    entryIdField().setInt(null, 0);
  }

  private static int readEntryId() {
    try {
      return entryIdField().getInt(null);
    } catch (Exception e) {
      throw new IllegalStateException("cannot read lastGeneratedEntryId", e);
    }
  }

  private static Field entryIdField() throws Exception {
    Field field =
        Class.forName("app.freerouting.board.searchtree.ShapeSearchTree")
            .getDeclaredField("lastGeneratedEntryId");
    field.setAccessible(true);
    return field;
  }
}
