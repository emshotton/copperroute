// Plan 5 Task 6 JVM probe: `DesignRulesChecker.calculateAllIncompletes` and the eight lazy
// accessors that hang off it, plus `BoardStatistics`' clearance-statistics block.
//
// The companion of `NetIncompletesProbe.java`, which dumps the *per net* `NetIncompletes` state.
// This one dumps the parts Task 6 owns and that probe does not print:
//
//   * `maxConnections` (DesignRulesChecker.java:567-577) — the public field
//     `calculateAllIncompletes` writes, read by `BoardStatistics.connections.maximumCount`;
//   * `getIncompleteCount()` (`:663-706`), `getAllAirlines().length` (`:780-798`) and the per-net
//     `getIncompleteCount(int)` (`:708-734`), whose sum the Java test asserts equals the total
//     (`RatsnestClearanceHeadlessTest.java:69-74`);
//   * `getLengthViolationCount()` (`:736-748`), `getLengthViolation(int)` (`:750-763`) and
//     `recalculateLengthViolations()` (`:765-778`);
//   * the clearance block of `BoardStatistics` (`BoardStatistics.java:338-367`) —
//     `clearanceViolations.{totalCount,minViolationUm,maxViolationUm,avgViolationUm}` — computed
//     here from `getAllClearanceViolations()` and `boardUnitToUmFactor` (`BoardStatistics.java:200`)
//     exactly as that block does, because `BoardStatistics` itself is Plan 8's (plan-5 ruling 5).
//
// All of these are **hash-independent** (plan-5 ruling 4): they are counts and lengths, not the
// airline endpoints. Verified by the `-XX:hashCode=0..4` sweep recorded in `README.md`.
//
// Transcript format (`<stem>.incompletes.txt`):
//
//   maxConnections <int>
//   incompleteCount <int>
//   airlines <int>
//   lengthViolationCount <int>
//   recalculateLengthViolations <true|false>
//   perNetIncompleteSum <int>
//   net=<n> incompleteCount=<int> lengthViolation=<Double.toString>      (one per net, ascending)
//   boardUnitToUmFactor <Double.toString>
//   clearanceViolations totalCount=<int> min=<D> max=<D> avg=<D>
//
// Usage: java -Djava.awt.headless=true -cp <jar>:. IncompletesProbe <board.dsn> <out-stem>
import app.freerouting.board.facade.BasicBoard;
import app.freerouting.board.model.structure.Unit;
import app.freerouting.drc.ClearanceViolation;
import app.freerouting.drc.DesignRulesChecker;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;
import java.io.FileInputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Collection;

public class IncompletesProbe {
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
    drc.calculateAllIncompletes();

    StringBuilder sb = new StringBuilder();
    sb.append("maxConnections ").append(drc.maxConnections).append('\n');
    sb.append("incompleteCount ").append(drc.getIncompleteCount()).append('\n');
    sb.append("airlines ").append(drc.getAllAirlines().length).append('\n');
    sb.append("lengthViolationCount ").append(drc.getLengthViolationCount()).append('\n');
    sb.append("recalculateLengthViolations ").append(drc.recalculateLengthViolations()).append('\n');

    int maxNetNo = board.rules.nets.maxNetNumber();
    int sumPerNet = 0;
    StringBuilder perNet = new StringBuilder();
    for (int netNumber = 1; netNumber <= maxNetNo; netNumber++) {
      int count = drc.getIncompleteCount(netNumber);
      sumPerNet += count;
      perNet
          .append("net=")
          .append(netNumber)
          .append(" incompleteCount=")
          .append(count)
          .append(" lengthViolation=")
          .append(Double.toString(drc.getLengthViolation(netNumber)))
          .append('\n');
    }
    sb.append("perNetIncompleteSum ").append(sumPerNet).append('\n');
    sb.append(perNet);

    // BoardStatistics.java:200-202 and :338-367, transcribed.
    double boardUnitToUmFactor =
        Unit.scale(1.0, board.communication.unit, Unit.UM)
            / (board.communication.resolution > 0 ? board.communication.resolution : 1);
    sb.append("boardUnitToUmFactor ").append(Double.toString(boardUnitToUmFactor)).append('\n');

    Collection<ClearanceViolation> violationsList =
        new DesignRulesChecker(board, null).getAllClearanceViolations();
    int totalCount = violationsList.size();
    double minViolation;
    double maxViolation;
    double avgViolation;
    if (!violationsList.isEmpty()) {
      minViolation = Double.MAX_VALUE;
      maxViolation = 0.0;
      double sumViolation = 0.0;
      for (ClearanceViolation cv : violationsList) {
        double shortfall = Math.max(0.0, cv.expectedClearance - cv.actualClearance);
        double shortfallUm = shortfall * boardUnitToUmFactor;
        minViolation = Math.min(minViolation, shortfallUm);
        maxViolation = Math.max(maxViolation, shortfallUm);
        sumViolation += shortfallUm;
      }
      avgViolation = sumViolation / violationsList.size();
    } else {
      minViolation = 0.0;
      maxViolation = 0.0;
      avgViolation = 0.0;
    }
    sb.append("clearanceViolations totalCount=")
        .append(totalCount)
        .append(" min=")
        .append(Double.toString(minViolation))
        .append(" max=")
        .append(Double.toString(maxViolation))
        .append(" avg=")
        .append(Double.toString(avgViolation))
        .append('\n');

    Files.writeString(Path.of(args[1] + ".incompletes.txt"), sb.toString(), StandardCharsets.UTF_8);
  }
}
