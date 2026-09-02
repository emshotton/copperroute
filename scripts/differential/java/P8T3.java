package app.freerouting.settings;

import app.freerouting.constants.Constants;
import app.freerouting.core.RoutingJob;
import app.freerouting.core.scoring.BoardStatistics;
import app.freerouting.io.specctra.RulesReader;
import app.freerouting.io.specctra.SesReader;
import app.freerouting.management.BoardLoader;
import app.freerouting.settings.sources.CliSettings;
import app.freerouting.settings.sources.DefaultSettings;
import app.freerouting.settings.sources.DsnFileSettings;
import app.freerouting.settings.sources.EnvironmentVariablesSource;
import app.freerouting.settings.sources.JsonFileSettings;
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
import java.util.List;
import java.util.UUID;

/**
 * Plan 8 Task 7 differential driver, <b>merge level</b>: {@code Freerouting.initializeDrc}'s
 * quality-score block (Freerouting.java:342-352) against {@code
 * freerouting::commands::drc::{quality_score_settings, quality_score}}.
 *
 * <p>Usage: {@code P8T3 merge <fixture-table> <java-dir>}, where the table is {@code
 * tests/reference/drc-fixtures.txt} — the same eight rows the committed {@code
 * tests/reference/drc-*} documents were generated from — and every path column is relative to the
 * Java checkout. The Rust twin is handed the identical two arguments.
 *
 * <h2>Why there is a Java half at all, when {@code p8t1} has none</h2>
 *
 * <p>{@code p8t1} compares two <em>whole programs</em>, so a Java class could only re-implement
 * {@code parity::normalize_log} a second time; {@code p8t3}'s {@code e2e} mode is the same shape
 * and is Rust-only for the same reason ({@code run.sh} switches this driver to {@code rust_only}
 * for it). This mode is different: what it drives is <b>quirk #272</b>, a Java <em>method
 * composition</em> — {@code globalSettings.settingsMergerProtype.clone()} plus one {@code
 * DsnFileSettings}, and nothing else — whose answer is not observable in the report beyond a single
 * {@code qualityScore} number. Comparing the number alone would say "the two agree" without saying
 * <em>which</em> of the seven scoring weights and six board counters produced it, so a divergence
 * would be one float with no diagnosis. This driver prints the weights, the counters and the score,
 * so a diff names the field.
 *
 * <h2>The transcription, line for line</h2>
 *
 * <pre>{@code
 *   var settingsMerger = globalSettings.settingsMergerProtype.clone();                  // :344
 *   settingsMerger.addOrReplaceSources(
 *       new DsnFileSettings(drcJob.input.getData(), drcJob.input.getFilename()));       // :345-346
 *   var routerSettings = settingsMerger.merge();                                        // :347
 *   var finalStats = drcJob.board.getStatistics();                                      // :348
 *   report.qualityScore = (double) finalStats.getNormalizedScore(routerSettings.scoring); // :349
 * }</pre>
 *
 * <p>The prototype itself is {@code new SettingsMerger(new DefaultSettings(), new
 * JsonFileSettings(), new CliSettings(args), new EnvironmentVariablesSource())}
 * (Freerouting.java:1408-1413), built here rather than cloned because {@code main} is what builds
 * it and this driver is not {@code main}. {@code args} is the empty array: every stem's command
 * line is {@code -de <dsn> [-dr <rules>] -drc <report>}, none of which {@code CliSettings} reads
 * (it is the {@code --section.field=value} / {@code -mp} parser).
 *
 * <p>The board is the one {@code initializeDrc} scores: {@code BoardLoader.loadBoardIfNeeded}
 * (`:271`), then the {@code .rules} file (`:277-294`), then the session (`:296-329`) — in that
 * order, which is quirk #273 — and then {@code generateReport} (`:340`) <b>before</b> the
 * statistics, because {@code generateReport} mutates the board (plan-5 ruling 8) and {@code :348}
 * reads it afterwards. Skipping the report here would score a different board.
 *
 * <h2>The one environmental asymmetry, printed rather than hidden</h2>
 *
 * <p>{@code :1410}'s {@code new JsonFileSettings()} resolves {@code
 * GlobalSettings.getUserDataPath().resolve("freerouting.json")} — on macOS {@code ~/Library/
 * Application Support/freerouting/freerouting.json}. Controller ruling BG measured that the port
 * has <b>no</b> default file at all (the tier is reachable only through {@code --settings <file>}
 * on the native form), so the Rust twin's priority-10 slot is empty on this argv. That is a
 * deliberate divergence and it is invisible here only because the file the jar writes carries an
 * <em>empty</em> {@code "router"} block. This driver therefore prints the resolved path and
 * whether it exists <b>on stderr</b>, so a machine where that file did carry a {@code
 * router.scoring} value would show the divergence as a DIFF on the {@code MERGE} line with the
 * provenance beside it, instead of as an unexplained one.
 *
 * <h2>Float printing</h2>
 *
 * <p>Every float and double is printed twice: once as {@code Float.toString}/{@code
 * Double.toString} (readable, and the port renders it through {@code
 * fr_dsn::format::double::java_float_to_string}, which {@code p3t2} already pins against the JVM)
 * and once as raw IEEE bits. The bits are the assertion — they cannot agree by rounding — and the
 * text is what a reader of a diff needs.
 */
public final class P8T3 {

  private P8T3() {}

  public static void main(String[] args) throws Exception {
    if (args.length < 3 || !"merge".equals(args[0])) {
      // `e2e` never reaches a JVM: `run.sh` switches the driver to `rust_only` for it.
      System.err.println("usage: P8T3 merge <drc-fixtures.txt> <java-dir>");
      System.exit(2);
    }
    // `FRLogger`'s console appender targets SYSTEM_OUT (quirk #261), and the board load emits
    // INFO/WARN lines on several corpus fixtures. Same guard as `P4T1`/`P5T1`: the transcript goes
    // to a private stream on the real stdout and `System.out` is redirected into the void.
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path table = Paths.get(args[1]).toAbsolutePath().normalize();
    Path javaDir = Paths.get(args[2]).toAbsolutePath().normalize();

    List<Row> rows = readTable(table);
    out.printf("HEADER driver=p8t3 mode=merge stems=%d%n", rows.size());
    System.err.println("jar-version Freerouting " + Constants.FREEROUTING_VERSION);
    Path userData = GlobalSettings.getUserDataPath().resolve("freerouting.json");
    System.err.printf("jsonfile path=%s exists=%b%n", userData, Files.exists(userData));

    for (Row row : rows) {
      run(out, javaDir, row);
    }
    out.flush();
  }

  private static void run(PrintStream out, Path javaDir, Row row) throws Exception {
    Path dsn = javaDir.resolve(row.dsn).normalize();

    // `Freerouting.java:260-264` — the job, and its input.
    RoutingJob job = new RoutingJob(UUID.nameUUIDFromBytes(new byte[0]));
    job.setInput(dsn.toString());

    // `:271` — the whole loader, with `job.routerSettings` still `new RouterSettings()`.
    if (!BoardLoader.loadBoardIfNeeded(job)) {
      out.printf("LOAD %s failed%n", row.stem);
      return;
    }

    // `:277-294` — the `.rules` file, before the session (quirk #273).
    if (row.rules != null) {
      Path rules = javaDir.resolve(row.rules).normalize();
      try (FileInputStream in = new FileInputStream(rules.toFile())) {
        // `:283` — `drcJob.name`, i.e. the base name **without** the extension.
        RulesReader.read(in, job.name, job.board, job.routerSettings);
      }
    }
    // `:296-329` — the session, after the rules.
    if (row.ses != null) {
      Path ses = javaDir.resolve(row.ses).normalize();
      try (FileInputStream in = new FileInputStream(ses.toFile())) {
        SesReader.read(in, job.board);
      }
    }

    // `:340` — the report runs **before** `:348`'s statistics and mutates the board.
    new app.freerouting.drc.DesignRulesChecker(job.board, null)
        .generateReport(dsn.getFileName().toString(), "mm");

    // `:344` — the prototype of `Freerouting.java:1408-1413`.
    List<SettingsSource> prototype = new ArrayList<>();
    prototype.add(new DefaultSettings());
    prototype.add(new JsonFileSettings());
    prototype.add(new CliSettings(new String[0]));
    prototype.add(new EnvironmentVariablesSource());
    SettingsMerger merger = new SettingsMerger(prototype);

    // `:345-346` — the DSN at priority 20, and nothing else.
    merger.addOrReplaceSources(
        new DsnFileSettings(job.input.getData(), job.input.getFilename()));

    // `:347`.
    RouterSettings routerSettings = merger.merge();
    ScoringSettings scoring = routerSettings.scoring;
    out.printf(
        "MERGE %s viaCosts=%s planeViaCosts=%s startRipupCosts=%s unroutedNetPenalty=%s"
            + " clearanceViolationPenalty=%s bendPenalty=%s defaultPreferredDirectionTraceCost=%s"
            + " defaultUndesiredDirectionTraceCost=%s defaultBendCost=%s%n",
        row.stem,
        scoring.viaCosts,
        scoring.planeViaCosts,
        scoring.startRipupCosts,
        f(scoring.unroutedNetPenalty),
        f(scoring.clearanceViolationPenalty),
        f(scoring.bendPenalty),
        d(scoring.defaultPreferredDirectionTraceCost),
        d(scoring.defaultUndesiredDirectionTraceCost),
        d(scoring.defaultBendCost));

    // `:348`.
    BoardStatistics stats = job.board.getStatistics();
    out.printf(
        "STATS %s maximumCount=%s incompleteCount=%s clearanceViolations=%s bends=%s vias=%s"
            + " traceLengthMm=%s%n",
        row.stem,
        stats.connections.maximumCount,
        stats.connections.incompleteCount,
        stats.clearanceViolations.totalCount,
        stats.bends.totalCount,
        stats.vias.totalCount,
        f(stats.traces.totalLengthMm));

    // `:349` — the `float`, then the `(double)` widening that puts it in the report.
    float score = stats.getNormalizedScore(scoring);
    out.printf(
        "SCORE %s float=%s bits=%d widened=%s%n",
        row.stem,
        Float.toString(score),
        Float.floatToRawIntBits(score),
        Double.toString((double) score));
  }

  /** {@code Float.toString}, with Java's own {@code null} spelling for a boxed null. */
  private static String f(Float value) {
    return value == null ? "null" : Float.toString(value);
  }

  /** {@code Double.toString}, with Java's own {@code null} spelling for a boxed null. */
  private static String d(Double value) {
    return value == null ? "null" : Double.toString(value);
  }

  /** One row of {@code tests/reference/drc-fixtures.txt}: {@code stem|dsn|rules|ses}. */
  private record Row(String stem, String dsn, String rules, String ses) {}

  private static List<Row> readTable(Path table) throws Exception {
    List<Row> rows = new ArrayList<>();
    for (String line : Files.readAllLines(table, StandardCharsets.UTF_8)) {
      String trimmed = line.trim();
      if (trimmed.isEmpty() || trimmed.startsWith("#")) {
        continue;
      }
      String[] fields = trimmed.split("\\|", -1);
      rows.add(
          new Row(
              fields[0].trim(),
              fields[1].trim(),
              blankToNull(fields.length > 2 ? fields[2] : ""),
              blankToNull(fields.length > 3 ? fields[3] : "")));
    }
    return rows;
  }

  private static String blankToNull(String value) {
    String trimmed = value.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }
}
