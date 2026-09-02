package app.freerouting.settings;

import app.freerouting.logger.FRLogger;
import app.freerouting.logger.LogEntry;
import java.io.BufferedReader;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * {@code p8t5} — the legacy command line, Java half.
 *
 * <p>Drives {@link GlobalSettings#applyCommandLineArguments(String[])} ({@code
 * GlobalSettings.java:521-838}) over the argv table in {@code
 * scripts/differential/matrix/p8t5-argv.tsv} and prints, per row:
 *
 * <ul>
 *   <li>the four filename slots ({@code initialInputFile}, {@code initialOutputFile}, {@code
 *       initialRulesFile}, {@code designSessionFilename}) plus {@code
 *       drcReportFile.getFilename()} and {@code showHelpOption};
 *   <li>{@code logging.console.level}, the one non-router field {@code -ll} writes;
 *   <li>every field of the {@code @Deprecated routerSettings} bridge that this method can write —
 *       plan-4 ruling 8's {@code LegacyBridge}, plus {@code drcSettings.enabled};
 *   <li>every {@code FRLogger} line the parse emitted, in order, read back out of {@code
 *       FRLogger.getLogEntries()}.
 * </ul>
 *
 * <p><b>Why the log entries and not the console.</b> {@code FRLogger}'s console appender targets
 * {@code SYSTEM_OUT} ({@code Log4j2ConfigurationFactory.java:58}), which is this driver's own
 * output stream. {@code System.out} and {@code System.err} are therefore redirected to a null
 * stream on the very first line of {@code main}, before any freerouting class is touched, and the
 * transcript is written to the saved original. The in-memory ring {@code FRLogger.getLogEntries()}
 * is what the comparison reads, and it holds exactly the {@code info}/{@code warn}/{@code error}
 * calls, which end in {@code logEntries.add(...)} ({@code FRLogger.java:243}, {@code :273},
 * {@code :338}); {@code debug} has no such call in its body and simply returns {@code null}
 * ({@code FRLogger.java:303}).
 *
 * <p><b>What is deliberately not compared.</b> The rewritten native command line, because Java has
 * no counterpart — it never builds a second command line. The port's {@code legacy::rewrite} is a
 * function of the slots printed here, and {@code crates/freerouting/tests/legacy_cli.rs} pins it
 * against those slots directly.
 *
 * <p>Usage: {@code P8T5 [argv-table.tsv]}. Declares {@code package app.freerouting.settings} so it
 * can read the package-private slot fields, and runs against the clone's HEAD jar.
 */
public class P8T5 {

  public static void main(String[] args) throws Exception {
    // FIRST: take the real stdout away from log4j, which grabs SYSTEM_OUT at configuration time.
    PrintStream real = System.out;
    System.setOut(new PrintStream(OutputStream.nullOutputStream(), true, StandardCharsets.UTF_8));
    System.setErr(new PrintStream(OutputStream.nullOutputStream(), true, StandardCharsets.UTF_8));

    Path table =
        Path.of(
            args.length > 0
                ? args[0]
                : "scripts/differential/matrix/p8t5-argv.tsv");

    StringBuilder out = new StringBuilder();
    out.append("# p8t5 legacy command line — Java (applyCommandLineArguments)\n");

    int rows = 0;
    try (BufferedReader reader = Files.newBufferedReader(table, StandardCharsets.UTF_8)) {
      String line;
      while ((line = reader.readLine()) != null) {
        if (line.isEmpty() || line.charAt(0) == '#') {
          continue;
        }
        String[] fields = line.split("\t", -1);
        String label = fields[0];
        String[] argv = new String[fields.length - 1];
        System.arraycopy(fields, 1, argv, 0, argv.length);
        emit(out, label, argv);
        rows++;
      }
    }
    out.append("# rows ").append(rows).append('\n');

    real.print(out);
    real.flush();
  }

  private static void emit(StringBuilder out, String label, String[] argv) {
    FRLogger.getLogEntries().clear();

    GlobalSettings settings = new GlobalSettings();
    settings.applyCommandLineArguments(argv);

    out.append("[row] ").append(label).append('\n');
    out.append("argv ").append(argv.length).append('\n');
    for (int i = 0; i < argv.length; i++) {
      out.append("argv[").append(i).append("] = ").append(quote(argv[i])).append('\n');
    }

    out.append("input = ").append(quote(settings.initialInputFile)).append('\n');
    out.append("output = ").append(quote(settings.initialOutputFile)).append('\n');
    out.append("rules = ").append(quote(settings.initialRulesFile)).append('\n');
    out.append("session = ").append(quote(settings.designSessionFilename)).append('\n');
    out.append("drc = ")
        .append(settings.drcReportFile == null ? "null" : quote(settings.drcReportFile.getFilename()))
        .append('\n');
    out.append("help = ").append(settings.showHelpOption).append('\n');
    out.append("console_level = ").append(quote(settings.logging.console.level)).append('\n');

    RouterSettings router = settings.routerSettings;
    out.append("router.enabled = ").append(router.enabled).append('\n');
    out.append("router.max_passes = ").append(router.maxPasses).append('\n');
    out.append("router.ignore_net_classes = ")
        .append(joinOrNull(router.ignoreNetClasses))
        .append('\n');
    out.append("optimizer.max_threads = ").append(router.optimizer.maxThreads).append('\n');
    out.append("optimizer.optimization_improvement_threshold = ")
        .append(
            router.optimizer.optimizationImprovementThreshold == null
                ? "null"
                : Float.toString(router.optimizer.optimizationImprovementThreshold))
        .append('\n');
    out.append("optimizer.board_update_strategy = ")
        .append(
            router.optimizer.boardUpdateStrategy == null
                ? "null"
                : router.optimizer.boardUpdateStrategy.name())
        .append('\n');
    out.append("optimizer.item_selection_strategy = ")
        .append(
            router.optimizer.itemSelectionStrategy == null
                ? "null"
                : router.optimizer.itemSelectionStrategy.name())
        .append('\n');
    out.append("optimizer.hybrid_ratio = ").append(quote(router.optimizer.hybridRatio)).append('\n');
    out.append("drc.enabled = ").append(settings.drcSettings.enabled).append('\n');

    List<String> messages = new ArrayList<>();
    for (LogEntry entry : FRLogger.getLogEntries().getEntries(null, null)) {
      messages.add(entry.getType().name().toUpperCase() + " " + entry.getMessage());
    }
    out.append("log ").append(messages.size()).append('\n');
    for (int i = 0; i < messages.size(); i++) {
      out.append("log[").append(i).append("] = ").append(quote(messages.get(i))).append('\n');
    }
    out.append('\n');
  }

  /** {@code null} as the four letters, anything else inside angle brackets so spaces show. */
  private static String quote(String value) {
    return value == null ? "null" : "<" + value + ">";
  }

  /** A {@code String[]} as {@code <a|b>}, so an untrimmed entry is visible. */
  private static String joinOrNull(String[] values) {
    if (values == null) {
      return "null";
    }
    return "<" + String.join("|", values) + ">";
  }
}
