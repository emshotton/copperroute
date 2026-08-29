// Plan 5 Task 3 JVM probe: `DesignRulesChecker.getAllClearanceViolations()`, in order.
//
// Task 2's `crates/fr-board/tests/data/DrcProbe.java` dumps the *per item* violation lists
// (`Item.clearanceViolations()`, before deduplication). This one dumps what
// `DesignRulesChecker.getAllClearanceViolations()` (DesignRulesChecker.java:52-81) returns —
// the deduplicated list, in the order the `board.getItems()` walk produced it — so the port's
// walk order, its dedup key and every field of every surviving violation can be compared
// field for field rather than only by count.
//
// The constructor's second argument is `null`, exactly as every one of Java's headless callers
// passes it (`RatsnestClearanceHeadlessTest.java:83`, `BoardStatistics.java:268`, `:339`) — the
// field is stored and never read, which is why the port drops the parameter (plan-5 ruling 12).
//
// Output format, one line per violation after a `count` line, which is what
// `crates/fr-drc/tests/data/*.list.txt` holds and what `the_ordered_list_matches_the_jvm`
// asserts:
//
//   count <n>
//   first=<id> second=<id> layer=<n> expected=<double> actual=<double>
//
// Doubles are printed with `Double.toString`, so the port compares exact bits through
// `fr_dsn::format::double::java_double_to_string`.
//
// Usage: java -Djava.awt.headless=true -cp <jar>:. DrcListProbe <board.dsn>
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
