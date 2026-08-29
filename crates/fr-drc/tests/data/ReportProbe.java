// Plan 5 Task 7 JVM probe: `DesignRulesChecker.generateReport` (DesignRulesChecker.java:210-290)
// and the four `io/kicad/KiCadDrc*.java` DTOs it fills.
//
// Dumps the report as **normalised text** rather than as Gson JSON, because Task 7 ports the DTOs
// and the builder, not the serialiser (that is Task 8's, with ruling 2's two key flavors). The
// normalisation is plan-5 ruling 3's:
//
//   * `date` is dropped entirely — it is `ZonedDateTime.now()` in Java (KiCadDrcReport.java:70)
//     and an injected string in the port (ruling 5);
//   * the `items` array of each entry is sorted by **numeric uuid**, because it comes out of a
//     `HashSet<Item>` on the Java side (quirk #144);
//   * the entry order is **kept** — `violations` is hash-independent (ruling 3's probe) and the
//     `unconnectedItems` entry order is ascending net number on both sides.
//
// Doubles are printed with `Double.toString`, so the Rust side compares exact bits through
// `fr_dsn::format::double::java_double_to_string`. `qualityScore` is always `null` here: the CLI
// fills it from `BoardStatistics` (Freerouting.java:343-352), which is Plan 8's.
//
// Transcript format (`<stem>.report.txt`):
//
//   schema <$schema>
//   coordinateUnits <unit>
//   kicadVersion <string>
//   freeroutingVersion <string>
//   source <string>
//   qualityScore <null|Double.toString>
//   counts violations=<int> unconnectedItems=<int> schematicParity=<int>
//   V type=<t> severity=<s> desc=<description>          (one per `violations` entry, in order)
//   I uuid=<u> x=<Double.toString> y=<Double.toString> desc=<description>
//   U type=<t> severity=<s> desc=<description>          (one per `unconnectedItems` entry)
//   I uuid=<u> x=<...> y=<...> desc=<...>
//
// Usage: java -Djava.awt.headless=true -cp <jar>:. ReportProbe <board.dsn> <source> <unit> <stem>
import app.freerouting.board.facade.BasicBoard;
import app.freerouting.drc.DesignRulesChecker;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.kicad.KiCadDrcReport;
import app.freerouting.io.kicad.KiCadDrcViolation;
import app.freerouting.io.kicad.KiCadDrcViolationItem;
import app.freerouting.io.specctra.DsnReader;
import java.io.FileInputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

public class ReportProbe {
  public static void main(String[] args) throws Exception {
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

    StringBuilder sb = new StringBuilder();
    sb.append("schema ").append(report.jsonSchema).append('\n');
    sb.append("coordinateUnits ").append(report.coordinateUnits).append('\n');
    sb.append("kicadVersion ").append(report.kicadVersion).append('\n');
    sb.append("freeroutingVersion ").append(report.freeroutingVersion).append('\n');
    sb.append("source ").append(report.source).append('\n');
    sb.append("qualityScore ")
        .append(report.qualityScore == null ? "null" : Double.toString(report.qualityScore))
        .append('\n');
    sb.append("counts violations=")
        .append(report.violations.size())
        .append(" unconnectedItems=")
        .append(report.unconnectedItems.size())
        .append(" schematicParity=")
        .append(report.schematicParity.size())
        .append('\n');
    for (KiCadDrcViolation v : report.violations) {
      appendEntry(sb, "V", v);
    }
    for (KiCadDrcViolation v : report.unconnectedItems) {
      appendEntry(sb, "U", v);
    }

    Files.writeString(Path.of(args[3] + ".report.txt"), sb.toString(), StandardCharsets.UTF_8);
  }

  private static void appendEntry(StringBuilder sb, String tag, KiCadDrcViolation v) {
    sb.append(tag)
        .append(" type=")
        .append(v.type)
        .append(" severity=")
        .append(v.severity)
        .append(" desc=")
        .append(v.description)
        .append('\n');
    List<KiCadDrcViolationItem> items = new ArrayList<>(v.items);
    items.sort(Comparator.comparingLong(i -> Long.parseLong(i.uuid)));
    for (KiCadDrcViolationItem i : items) {
      sb.append("I uuid=")
          .append(i.uuid)
          .append(" x=")
          .append(Double.toString(i.pos.coordX))
          .append(" y=")
          .append(Double.toString(i.pos.coordY))
          .append(" desc=")
          .append(i.description)
          .append('\n');
    }
  }
}
