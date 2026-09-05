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
