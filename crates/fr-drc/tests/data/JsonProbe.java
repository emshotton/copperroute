// Plan 5 Task 8 JVM probe: `DesignRulesChecker.generateReportJson`
// (DesignRulesChecker.java:817-820) — i.e. `GsonProvider.GSON.toJson(report)` over the four
// `io/kicad/KiCadDrc*.java` DTOs, which is the byte-parity target of `KiCadDrcReport::to_json`.
//
// Task 7's `ReportProbe.java` dumps the same report as normalised *text*, so it pins the report's
// content; this probe pins the **bytes Gson writes** for it: the `@SerializedName` key spellings
// (plan-5 ruling 1 — HEAD's camelCase), the two-space pretty printing, `Number.toString()` number
// rendering, the omission of a null `qualityScore` (Gson's default `serializeNulls = false`) and
// the string escaping `disableHtmlEscaping()` leaves on.
//
// Two modes:
//
//   JsonProbe <board.dsn> <source> <unit> <stem>
//     Writes `<stem>.head.json`: the report of `generateReport(source, unit)`, with each entry's
//     `items` list sorted by **numeric uuid** in place before serialising — `unconnectedItems`
//     entries only (plan-5 ruling 3 — those lists come out of two `HashSet<Item>`s and are
//     hash-ordered on the JVM, quirk #144; a clearance entry's two items are not), rendered
//     through the same `GsonProvider.GSON` `generateReportJson` uses. Everything else — key
//     order, entry order, indentation, number text — is Gson's, untouched. `date` is the real
//     `ZonedDateTime.now()` of the run (KiCadDrcReport.java:70); the Rust side reads it back out
//     of this file and injects it (plan-5 ruling 5).
//
//   JsonProbe --escapes <out.txt>
//     Writes the four one-line facts about `GsonProvider.GSON` that no fixture exercises, each as
//     `<name>\t<the JSON Gson wrote>`: a `qualityScore` of 902.078369140625 and of 1.0E7
//     (`Double.toString`, not `%f`), a `source` holding `<'&=>"` (HTML escaping is **off**,
//     GsonProvider.java:15) and a `source` holding U+2028/U+2029 (escaped anyway — they are legal
//     in JSON and illegal in JavaScript).
//
// Usage: java -Djava.awt.headless=true -cp <jar>:. JsonProbe ...
import app.freerouting.board.facade.BasicBoard;
import app.freerouting.drc.DesignRulesChecker;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.kicad.KiCadDrcReport;
import app.freerouting.io.kicad.KiCadDrcViolation;
import app.freerouting.io.kicad.KiCadDrcViolationItem;
import app.freerouting.io.specctra.DsnReader;
import app.freerouting.util.gson.GsonProvider;
import java.io.FileInputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;

public class JsonProbe {
  public static void main(String[] args) throws Exception {
    if ("--escapes".equals(args[0])) {
      escapes(args[1]);
      return;
    }

    BoardReadResult result;
    try (FileInputStream in = new FileInputStream(args[0])) {
      result = DsnReader.readBoard(in, null, null, "test");
    }
    BasicBoard board =
        switch (result) {
          case BoardReadResult.Success s -> (BasicBoard) s.board();
          case BoardReadResult.OutlineMissing o -> (BasicBoard) o.board();
          default -> throw new IllegalStateException("Failed to read board: " + result);
        };

    DesignRulesChecker drc = new DesignRulesChecker(board, null);
    KiCadDrcReport report = drc.generateReport(args[1], args[2]);

    // plan-5 ruling 3: the only normalisation, and it happens before Gson sees the object so the
    // bytes on either side of the comparison are Gson's own.
    //
    // **`unconnectedItems` only.** Those entries' `items` are `connectedSets.get(0)` followed by
    // `connectedSets.get(1)` (DesignRulesChecker.java:143-145), two `HashSet<Item>`s, so their
    // order is identity-hash order and nothing to copy. A `violations` entry is a different case:
    // a clearance entry's two items are `[firstItem, secondItem]` in `ClearanceViolation`'s own
    // order (`:319-324`), which **is** deterministic and which the port reproduces — sorting them
    // here would hide a real ordering divergence. The dangling entries carry one item each.
    Comparator<KiCadDrcViolationItem> byUuid = Comparator.comparingLong(i -> Long.parseLong(i.uuid));
    for (KiCadDrcViolation v : report.unconnectedItems) {
      v.items.sort(byUuid);
    }

    Files.writeString(
        Path.of(args[3] + ".head.json"),
        GsonProvider.GSON.toJson(report),
        StandardCharsets.UTF_8);
  }

  private static void escapes(String out) throws Exception {
    StringBuilder sb = new StringBuilder();
    sb.append("qualityScore-902\t").append(scoreLine(902.078369140625)).append('\n');
    sb.append("qualityScore-1e7\t").append(scoreLine(1.0e7)).append('\n');
    sb.append("source-html\t").append(sourceLine("<'&=>\"")).append('\n');
    // Built from char values rather than written as literals, which would put a U+2028 into
    // this file for no reason.
    String separators = "a" + (char) 0x2028 + "b" + (char) 0x2029 + "c";
    sb.append("source-separators\t").append(sourceLine(separators)).append('\n');
    Files.writeString(Path.of(out), sb.toString(), StandardCharsets.UTF_8);
  }

  /** The `"qualityScore": …` line Gson writes for `score`. */
  private static String scoreLine(double score) {
    KiCadDrcReport report = new KiCadDrcReport("mm", "probe", "Freerouting probe");
    report.qualityScore = score;
    return lineContaining(GsonProvider.GSON.toJson(report), "\"qualityScore\"");
  }

  /** The `"source": …` line Gson writes for `source`. */
  private static String sourceLine(String source) {
    KiCadDrcReport report = new KiCadDrcReport("mm", source, "Freerouting probe");
    return lineContaining(GsonProvider.GSON.toJson(report), "\"source\"");
  }

  private static String lineContaining(String json, String key) {
    for (String line : json.split("\n", -1)) {
      if (line.contains(key)) {
        return line.strip();
      }
    }
    throw new IllegalStateException("no " + key + " in " + json);
  }
}
