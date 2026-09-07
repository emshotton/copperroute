import app.freerouting.board.facade.BasicBoard;
import app.freerouting.drc.ClearanceViolation;
import app.freerouting.drc.DesignRulesChecker;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;
import java.io.FileInputStream;
import java.util.Collection;

public class DrcListProbe {
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
    Collection<ClearanceViolation> violations = drc.getAllClearanceViolations();

    StringBuilder sb = new StringBuilder();
    sb.append("count ").append(violations.size()).append('\n');
    for (ClearanceViolation v : violations) {
      sb.append("first=")
          .append(v.firstItem.getId())
          .append(" second=")
          .append(v.secondItem.getId())
          .append(" layer=")
          .append(v.layer)
          .append(" expected=")
          .append(Double.toString(v.expectedClearance))
          .append(" actual=")
          .append(Double.toString(v.actualClearance))
          .append('\n');
    }
    System.out.print(sb);
  }
}
