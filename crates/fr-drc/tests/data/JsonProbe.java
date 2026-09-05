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
    String separators = "a" + (char) 0x2028 + "b" + (char) 0x2029 + "c";
    sb.append("source-separators\t").append(sourceLine(separators)).append('\n');
    Files.writeString(Path.of(out), sb.toString(), StandardCharsets.UTF_8);
  }

  private static String scoreLine(double score) {
    KiCadDrcReport report = new KiCadDrcReport("mm", "probe", "Freerouting probe");
    report.qualityScore = score;
    return lineContaining(GsonProvider.GSON.toJson(report), "\"qualityScore\"");
  }

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
