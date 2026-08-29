package app.freerouting.drc;

import app.freerouting.board.facade.BasicBoard;
import app.freerouting.constants.Constants;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.kicad.KiCadDrcReport;
import app.freerouting.io.kicad.KiCadDrcViolation;
import app.freerouting.io.kicad.KiCadDrcViolationItem;
import app.freerouting.io.specctra.DsnReader;
import app.freerouting.io.specctra.RulesReader;
import app.freerouting.io.specctra.SesReader;
import app.freerouting.util.gson.GsonProvider;
import com.google.gson.JsonObject;
import java.io.FileDescriptor;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Comparator;

/**
 * Plan 5 Task 10 differential driver, report level: Java's real {@code
 * DesignRulesChecker.generateReport} (DesignRulesChecker.java:210-290) serialised by the real
 * {@code GsonProvider.GSON} (DesignRulesChecker.java:817-820), against the port's {@code
 * DesignRulesChecker::report_to_json(DrcJsonFlavor::FreeroutingHead)}.
 *
 * <p>Usage: {@code P5T1 <dsn> [rules|-] [ses|-]}. The two optional slots take {@code -} or an empty
 * string for "absent", so the argv is positional and the Rust twin can be handed the same one.
 *
 * <h2>What is compared, and what is normalised away</h2>
 *
 * <p>The comparison surface is <b>Gson's own bytes</b>: key order, two-space indentation, {@code
 * Number.toString()} number rendering and the string escaping {@code disableHtmlEscaping()} leaves
 * on (Task 8's target, pinned by {@code crates/fr-drc/tests/report_json.rs}). Five rules — the same
 * five on both sides — take the three <em>injected</em> values (plan-5 ruling 5) and the one
 * hash-ordered list (ruling 3) out of the comparison, and touch nothing else:
 *
 * <ol>
 *   <li><b>{@code date}</b> is replaced by {@link #FIXED_DATE}. It is {@code
 *       ZonedDateTime.now().format(ISO_OFFSET_DATE_TIME)} in a {@code final} field
 *       (KiCadDrcReport.java:70), so it cannot be set on the object and this is a one-line edit of
 *       the serialised tree. <em>Replaced</em> rather than removed, deliberately: the port's
 *       serialiser always emits the key, so removing it here would make "the port dropped {@code
 *       date}" undetectable, where replacing it still compares the key's presence and position.
 *   <li><b>{@code freeroutingVersion}</b> is replaced by {@link #FIXED_VERSION}. It is {@code
 *       "Freerouting " + Constants.FREEROUTING_VERSION} (DesignRulesChecker.java:212-213), also
 *       {@code final}; on the port's side it is a caller-supplied string (ruling 5), so comparing
 *       it would compare which literal this driver pair agreed on, and would make the whole sweep
 *       fail on the day the jar's version is bumped.
 *   <li><b>{@code qualityScore}</b> is set to {@code -1.0} on the object. Only the CLI computes it,
 *       from {@code BoardStatistics.getNormalizedScore} (Freerouting.java:349), which is Plan 8's;
 *       {@code generateReport} leaves it {@code null} and Gson would drop the key. Pinning it to a
 *       non-null number keeps the key — and its {@code Double.toString} rendering — in the
 *       comparison without needing {@code BoardStatistics} here.
 *   <li><b>Every {@code unconnectedItems} entry's {@code items} list is sorted by numeric uuid.</b>
 *       Those lists are {@code connectedSets.get(0)} followed by {@code connectedSets.get(1)}
 *       (DesignRulesChecker.java:143-145), two {@code HashSet<Item>}s over a class with no {@code
 *       hashCode} override — identity-hash order, i.e. nothing to port (quirk #144, ruling 3). The
 *       sort is numeric because the uuids are {@code String.valueOf(item.getId())} (`:320-321`), so
 *       {@code "1000"} must sort after {@code "99"}. Same rule, same comparator as {@code
 *       crates/fr-drc/tests/data/JsonProbe.java} and {@code parity::normalize_drc_json}.
 *   <li><b>Nothing else is touched.</b> In particular {@code violations} is left completely alone —
 *       neither the array nor any entry's {@code items}: the array order is hash-independent
 *       (ruling 3's probe) and a clearance entry's two items are {@code [firstItem, secondItem]} in
 *       {@code ClearanceViolation}'s own deterministic order (`:319-324`), so sorting them would
 *       hide a real ordering regression.
 * </ol>
 *
 * <p>Rules 3 and 4 are applied to the report object, before Gson sees it; rules 1 and 2 have to be
 * applied to the serialised tree, because the two fields are {@code final}. Re-rendering a {@code
 * JsonObject} through the same {@code GsonProvider.GSON} is byte-preserving: {@code JsonObject}
 * keeps insertion order, and a {@code JsonPrimitive} holding a {@code Double} is written by {@code
 * JsonWriter.value(Number)}, i.e. through the same {@code Double.toString} the reflective adapter
 * uses.
 *
 * <h2>Why this driver and not the {@code -drc} CLI</h2>
 *
 * <p>{@code scripts/gen-drc-reference.sh} drives the real CLI, because a reference wants a real
 * {@code qualityScore}. A differential wants the opposite: no clock, no scoring stack, no file
 * output, and a board built by the exact three calls the port's test harness makes. So this driver
 * transcribes the four lines of {@code Freerouting.initializeDrc} that matter
 * (Freerouting.java:246-372) — read the DSN, apply the {@code .rules} file, apply the session file,
 * hard-code {@code "mm"} (`:335`) and the input file's base name (`:339`) — and calls {@code
 * DesignRulesChecker(board, null)} directly. Ten of the fifteen constructions in the Java tree pass
 * {@code null} for the settings, including both {@code BoardStatistics} call sites (plan-5 ruling
 * 12).
 *
 * <p>The board is loaded in the CLI's order: DSN, then {@code .rules}, then the session — the rules
 * can change the clearances the session's wires are then checked against.
 */
public final class P5T1 {

  /** Rule 1. Any fixed ISO-8601 string; the Rust twin injects the identical one. */
  static final String FIXED_DATE = "1970-01-01T00:00:00Z";

  /** Rule 2. The whole field value, prefix included. */
  static final String FIXED_VERSION = "Freerouting p5t1";

  /** Rule 3. */
  static final double FIXED_QUALITY_SCORE = -1.0;

  private P5T1() {}

  public static void main(String[] args) throws Exception {
    if (args.length < 1) {
      System.err.println("usage: P5T1 <dsn> [rules|-] [ses|-]");
      System.exit(2);
    }
    // `FRLogger` writes to stdout on at least one corpus fixture (a degenerate-wire warning), and
    // the port has no logger to reproduce it with. Same guard as `P4T1.java`: the driver's own
    // output goes to a private stream on the real stdout, and `System.out` is redirected into the
    // void so nothing else can reach the diff.
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    Path rules = optionalPath(args, 1);
    Path ses = optionalPath(args, 2);

    // The header (the `p4t1` convention): the driver must be running against the jar the port is a
    // port of, and both sides must have been handed the same three files.
    Path jar =
        Paths.get(
                DesignRulesChecker.class
                    .getProtectionDomain()
                    .getCodeSource()
                    .getLocation()
                    .toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d fixture=%s rules=%s ses=%s%n",
        jar,
        Files.size(jar),
        Files.getLastModifiedTime(jar).toMillis(),
        dsn.getFileName(),
        name(rules),
        name(ses));
    // Provenance for the jar's own version, on stderr: rule 2 takes it out of stdout, and a sweep
    // that silently changed jars would otherwise leave no trace.
    System.err.println("jar-version Freerouting " + Constants.FREEROUTING_VERSION);

    BasicBoard board = loadBoard(dsn, rules, ses);

    // `Freerouting.java:335-336` hard-codes the unit; `:339` takes the base name of the input file.
    KiCadDrcReport report =
        new DesignRulesChecker(board, null)
            .generateReport(dsn.getFileName().toString(), "mm");

    // Rule 3.
    report.qualityScore = FIXED_QUALITY_SCORE;
    // Rule 4 — `unconnectedItems` only (see the class comment).
    Comparator<KiCadDrcViolationItem> byUuid = Comparator.comparingLong(i -> Long.parseLong(i.uuid));
    for (KiCadDrcViolation violation : report.unconnectedItems) {
      violation.items.sort(byUuid);
    }

    JsonObject tree = GsonProvider.GSON.toJsonTree(report).getAsJsonObject();
    // Rules 1 and 2 — the two `final` fields, edited on the tree.
    tree.addProperty("date", FIXED_DATE);
    tree.addProperty("freeroutingVersion", FIXED_VERSION);

    out.println(GsonProvider.GSON.toJson(tree));
    out.flush();
  }

  /**
   * The board {@code Freerouting.initializeDrc} hands to {@code DesignRulesChecker}: the DSN, then
   * the {@code .rules} file (Freerouting.java:277-292), then the session file (`:297-329`).
   */
  static BasicBoard loadBoard(Path dsn, Path rules, Path ses) throws Exception {
    BoardReadResult result;
    // The design name is a log-message hint only (DsnReader.java:56-57); the base name is what the
    // port's harness passes, so both sides pass it.
    String designName = dsn.getFileName().toString();
    try (FileInputStream in = new FileInputStream(dsn.toFile())) {
      result = DsnReader.readBoard(in, null, null, designName);
    }
    BasicBoard board =
        switch (result) {
          case BoardReadResult.Success s -> s.board();
          case BoardReadResult.OutlineMissing o -> o.board();
          default -> throw new IllegalStateException("board did not read: " + result);
        };

    if (rules != null) {
      try (FileInputStream in = new FileInputStream(rules.toFile())) {
        // `designName` here is `drcJob.name` (Freerouting.java:283), which `RoutingJob.setInput`
        // fills from `input.getFilenameWithoutExtension()` (RoutingJob.java:457) — the base name
        // **without** `.dsn`. `RulesReader` compares it against the `(rules PCB <name>` header and
        // warns on a mismatch (RulesReader.java:100-110); the extension-ful name would take the
        // other branch. `crates/fr-drc/tests/reference_parity.rs` passes the same string.
        //
        // The CLI passes `drcJob.routerSettings` as the fourth argument (Freerouting.java:284-285);
        // that only receives the file's `(autoroute_settings …)`, which never reaches the board, so
        // the DRC path can use the three-argument overload (RulesReader.java:47-49).
        String rulesDesignName =
            designName.endsWith(".dsn")
                ? designName.substring(0, designName.length() - ".dsn".length())
                : designName;
        if (!RulesReader.read(in, rulesDesignName, board)) {
          throw new IllegalStateException("rules were rejected: " + rules);
        }
      }
    }
    if (ses != null) {
      try (FileInputStream in = new FileInputStream(ses.toFile())) {
        var summary = SesReader.read(in, board);
        if (summary.errorsEncountered() != 0) {
          throw new IllegalStateException(
              "session imported with " + summary.errorsEncountered() + " errors: " + ses);
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

  static String name(Path path) {
    return path == null ? "-" : path.getFileName().toString();
  }
}
