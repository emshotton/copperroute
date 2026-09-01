package app.freerouting.management;

import app.freerouting.board.actions.ItemIdGenerator;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.ObstacleArea;
import app.freerouting.board.model.structure.Unit;
import app.freerouting.core.RoutingJob;
import app.freerouting.geometry.planar.Circle;
import app.freerouting.rules.ClearanceMatrix;
import app.freerouting.rules.DefaultItemClearanceClasses;
import app.freerouting.settings.RouterSettings;
import app.freerouting.settings.SettingsMerger;
import app.freerouting.settings.sources.CliSettings;
import app.freerouting.settings.sources.DefaultSettings;
import app.freerouting.settings.sources.DsnFileSettings;
import app.freerouting.settings.sources.EnvironmentVariablesSource;
import app.freerouting.settings.sources.JsonFileSettings;
import app.freerouting.settings.sources.RulesFileSettings;

import java.io.File;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.TreeMap;
import java.util.UUID;

// Plan 7 Task 15b ground-truth probe: `management/HeadlessBoardManager.java`'s three
// board-mutating clearance overrides —
//   `applyCopperToEdgeClearanceOverride`  (:466-552)
//   `applyHoleClearanceOverride`          (:346-396)
//   `assignHoleKeepoutClearanceClass`     (:404-464)
// — measured on the sixteen-board Plan 7/8 parity corpus.
//
// It is not a differential driver: there is no Rust twin binary and `run.sh` does not know it,
// the `P7T2Probe`/`P7T4Probe`/`P7T9Probe` pattern. Its stdout is committed verbatim as
// `crates/fr-router/tests/data/p7t15b-clearance-overrides.txt` and replayed by
// `crates/fr-router/tests/clearance_override.rs`, which loads the same DSN through
// `fr_dsn::read_board`, calls `Board::apply_*` / `pipeline::prepare_board`, and re-derives every
// field below.
//
// ## Why the load is repeated once per variant
//
// All three methods are **private**, and Java invokes them from inside the load — at
// `createBoard:342-343` on the freshly built, still itemless board and again at
// `applyRouterSettingsForLoadedBoard:746-747` on the fully loaded one. So the only way to
// observe both call sites without instrumenting bytecode is to load the same DSN several times
// with the two settings knobs (`router.copper_to_edge_clearance_um`,
// `router.hole_clearance_um`) set differently, and diff the resulting board state. `null`
// disables an override at *both* call sites (`:470-471`, `:350-351`), which is what makes
// variant A the pristine board.
//
// The nine variants, and what each pins:
//
//   A  copper=null  hole=null    the pristine board — the state `fr_dsn::read_board` answers
//   B  copper=MERGED hole=null   the copper override alone, at the real merged value (500 um)
//   C  copper=null  hole=MERGED  the hole override alone, at the real merged value (0 um)
//   D  copper=MERGED hole=MERGED exactly what a plain `-de <dsn> -do <ses>` run produces
//   E  copper=null  hole=100.0   the non-default hole path; the um -> board-unit conversion
//   F  copper=null  hole=500.0   the same, where the `Math.max` floor starts to bite
//   G  copper=null  hole=-1.0    the negative early return (:355-359)
//   H  copper=-1.0  hole=null    the negative early return (:474-480)
//   I  copper=0.0   hole=null    a NON-default copper value: the `:501-507` guard cannot fire,
//                                so this mutates even the one board that early-returns at the
//                                default (router-rpi-splitter)
//
// ## Byte stability
//
// No clock, no wall-time budget, no `HashSet<Item>` iteration: every collection printed is a
// `TreeMap` or an `itemList` walk, and every floating-point value crosses as `Double.toString`.
// The transcript is reproducible across runs and machines given the same jar and corpus.
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p7t15b java/probes/P7T15bProbe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p7t15b:$JAR" \
//       app.freerouting.management.P7T15bProbe \
//     > ../../crates/fr-router/tests/data/p7t15b-clearance-overrides.txt
public final class P7T15bProbe {

  private P7T15bProbe() {}

  /** One corpus row: the parity stem, the DSN under the Java checkout, an optional `.rules`. */
  private record Row(String stem, String dsn, String rules) {}

  /** One variant's observable board state. */
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
    long matrixSum;
    long[] layerSum;
    long[][] rowSum;
    int[][] layer0;
  }

  /** The Plan 7/8 parity corpus, in the order `plan8-evidence/job1-summary.md` lists it. */
  private static final List<Row> CORPUS =
      List.of(
          new Row("router-rpi-splitter", "fixtures/Issue143-rpi_splitter.dsn", null),
          new Row("router-dac2020-bm01", "fixtures/Issue508-DAC2020_bm01.dsn", null),
          new Row("router-j2-reference", "fixtures/Issue026-J2_reference.dsn", null),
          new Row("router-tutorial-board", "examples/tutorial_board/tutorial_board.dsn", null),
          new Row(
              "router-ecc83-input",
              "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.dsn",
              null),
          new Row(
              "drc-dev-board",
              "fixtures/Issue575-drc_dev-board_4_hole_clearance_violations.dsn",
              null),
          new Row(
              "drc-bbd-mars-64",
              "fixtures/Issue575-drc_BBD_Mars-64_6_track_1_hole_clearance_violations.dsn",
              null),
          new Row(
              "drc-natural-tone-preamp",
              "fixtures/Issue575-drc_Natural_Tone_Preamp_7_unconnected_items.dsn",
              null),
          new Row(
              "drc-issue593-rules",
              "fixtures/Issue593-BBD_Mars-64.dsn",
              "fixtures/Issue593-BBD_Mars-64.rules"),
          new Row("drc-issue593-ses", "fixtures/Issue593-BBD_Mars-64.dsn", null),
          new Row("drc-issue753-cpu85", "fixtures/Issue753-CPU-85_r104.dsn", null),
          new Row("drc-issue110-relay", "fixtures/Issue110-RelayModule.dsn", null),
          new Row("drc-tutorial-board", "examples/tutorial_board/tutorial_board.dsn", null),
          new Row("batch-fanout-bm11", "fixtures/Issue730-DAC2020_bm11.dsn", null),
          new Row(
              "batch-strict-drc-cnh", "fixtures/Issue555-CNH_Functional_Tester_1.dsn", null),
          new Row("batch-empty-board", "fixtures/empty_board.dsn", null));

  public static void main(String[] argv) throws Exception {
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    String javaDir =
        System.getenv()
            .getOrDefault("FREEROUTING_JAVA_DIR", "/Users/em/Development/freerouting/freerouting");

    out.println("# P7T15bProbe — HeadlessBoardManager's three clearance overrides, HEAD jar");
    out.println(
        "# management/HeadlessBoardManager.java:346-396, :404-464, :466-552; the sixteen-board"
            + " parity corpus");
    out.println("# no clock, no HashSet iteration: byte-stable across runs");
    out.println(
        "# DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM = "
            + Double.toString(DefaultSettings.DEFAULT_COPPER_TO_EDGE_CLEARANCE_UM));
    out.println(
        "# DEFAULT_HOLE_CLEARANCE_UM = "
            + Double.toString(DefaultSettings.DEFAULT_HOLE_CLEARANCE_UM));

    for (Row row : CORPUS) {
      File dsn = new File(javaDir, row.dsn());
      File rules = row.rules() == null ? null : new File(javaDir, row.rules());
      RouterSettings merged = mergeLikeCli(dsn, rules);
      Double copper = merged.copperToEdgeClearanceUm;
      Double hole = merged.holeClearanceUm;

      out.println();
      out.println(
          "[board] stem="
              + row.stem()
              + " dsn="
              + row.dsn()
              + " rules="
              + (row.rules() == null ? "-" : row.rules()));
      out.println(
          "merged copper=" + str(copper) + " hole=" + str(hole));

      emit(out, "A", dsn, rules, null, null);
      emit(out, "B", dsn, rules, copper, null);
      emit(out, "C", dsn, rules, null, hole);
      emit(out, "D", dsn, rules, copper, hole);
      emit(out, "E", dsn, rules, null, 100.0);
      emit(out, "F", dsn, rules, null, 500.0);
      emit(out, "G", dsn, rules, null, -1.0);
      emit(out, "H", dsn, rules, -1.0, null);
      emit(out, "I", dsn, rules, 0.0, null);
    }
  }

  private static String str(Double d) {
    return d == null ? "-" : Double.toString(d);
  }

  private static void emit(
      PrintStream out, String variant, File dsn, File rules, Double copper, Double hole)
      throws Exception {
    RoutingBoard board = load(dsn, rules, copper, hole);
    State s = snapshot(board);
    out.println(
        "[variant] name="
            + variant
            + " copper="
            + str(copper)
            + " hole="
            + str(hole)
            + " copper_units="
            + (copper == null ? "-" : Integer.toString(boardUnits(board, copper)))
            + " hole_units="
            + (hole == null ? "-" : Integer.toString(boardUnits(board, hole))));
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
    out.println(
        "  keepouts=" + s.keepoutCount + " keepout_classes=" + histogram);
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
    for (int i = 0; i < s.classCount; i++) {
      StringBuilder values = new StringBuilder();
      for (int j = 0; j < s.classCount; j++) {
        if (j > 0) {
          values.append(' ');
        }
        values.append(s.layer0[i][j]);
      }
      out.println("  mrow L0 " + i + " " + s.classNames.get(i) + " = " + values);
    }
  }

  /**
   * `HeadlessBoardManager.java:365-372` / `:508-516` — the shared um -> board-unit conversion,
   * transcribed so the transcript pins it independently of what the override then does with it.
   */
  private static int boardUnits(RoutingBoard board, double clearanceUm) {
    int boardResolution = Math.max(1, board.communication.resolution);
    return (int)
        Math.round(
            Unit.scale(clearanceUm * boardResolution, Unit.UM, board.communication.unit));
  }

  /** Reproduces `Freerouting.initializeCli:125-146` (merge #1) exactly. */
  private static RouterSettings mergeLikeCli(File dsn, File rules) throws Exception {
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
    if (rules != null && rules.exists()) {
      job.setRules(rules.getAbsolutePath());
      if (job.rules != null && job.rules.getData() != null) {
        merger.addOrReplaceSources(
            new RulesFileSettings(job.rules.getData(), job.rules.getFilename()));
      }
    }
    return merger.merge();
  }

  /** One full `loadFromSpecctraDsn` with the two override knobs forced to `copper` / `hole`. */
  private static RoutingBoard load(File dsn, File rules, Double copper, Double hole)
      throws Exception {
    RouterSettings settings = mergeLikeCli(dsn, rules);
    settings.copperToEdgeClearanceUm = copper;
    settings.holeClearanceUm = hole;
    RoutingJob job = new RoutingJob(UUID.randomUUID());
    job.setInput(dsn.getAbsolutePath());
    job.routerSettings = settings;
    HeadlessBoardManager manager = new HeadlessBoardManager(job);
    manager.loadFromSpecctraDsn(job.input.getData(), null, new ItemIdGenerator());
    RoutingBoard board = manager.getRoutingBoard();
    if (board == null) {
      throw new IllegalStateException(
          String.format(Locale.ROOT, "%s produced no board", dsn.getName()));
    }
    return board;
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

    s.layerSum = new long[s.layerCount];
    s.rowSum = new long[s.layerCount][s.classCount];
    s.layer0 = new int[s.classCount][s.classCount];
    for (int layer = 0; layer < s.layerCount; layer++) {
      for (int i = 0; i < s.classCount; i++) {
        for (int j = 0; j < s.classCount; j++) {
          int value = matrix.getValue(i, j, layer, false);
          s.matrixSum += value;
          s.layerSum[layer] += value;
          s.rowSum[layer][i] += value;
          if (layer == 0) {
            s.layer0[i][j] = value;
          }
        }
      }
    }

    for (Item item : board.getItems()) {
      s.itemCount++;
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
}
