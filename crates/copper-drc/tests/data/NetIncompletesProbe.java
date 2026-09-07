import app.freerouting.board.facade.BasicBoard;
import app.freerouting.board.model.items.Connectable;
import app.freerouting.board.model.items.Item;
import app.freerouting.drc.AirLine;
import app.freerouting.drc.DesignRulesChecker;
import app.freerouting.drc.NetIncompletes;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;
import java.io.FileInputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;

public class NetIncompletesProbe {
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

    int maxNetNo = board.rules.nets.maxNetNumber();

    int[] inputSizes = new int[maxNetNo];
    for (Item item : board.getItems()) {
      if (item instanceof Connectable) {
        for (int i = 0; i < item.netCount(); i++) {
          int netNumber = item.getNetNumber(i);
          if (netNumber >= 1 && netNumber <= maxNetNo) {
            inputSizes[netNumber - 1]++;
          }
        }
      }
    }

    DesignRulesChecker drc = new DesignRulesChecker(board, null);

    StringBuilder sb = new StringBuilder();
    sb.append("nets ").append(maxNetNo).append('\n');
    for (int netNumber = 1; netNumber <= maxNetNo; netNumber++) {
      NetIncompletes ni = drc.getNetIncompletes(netNumber);
      sb.append("net=")
          .append(netNumber)
          .append(" items=")
          .append(inputSizes[netNumber - 1])
          .append(" count=")
          .append(ni.count())
          .append(" groups=")
          .append(ni.getConnectedGroupCount())
          .append(" lengthViolation=")
          .append(Double.toString(ni.getLengthViolation()))
          .append(" markerRadius=")
          .append(Double.toString(ni.getMarkerRadius()))
          .append('\n');
    }
    AirLine[] airlines = drc.getAllAirlines();
    sb.append("airlines ").append(airlines.length).append('\n');
    sb.append("incompleteCount ").append(drc.getIncompleteCount()).append('\n');
    Files.writeString(Path.of(args[1] + ".netincompletes.txt"), sb.toString(), StandardCharsets.UTF_8);

    StringBuilder lines = new StringBuilder();
    for (AirLine airline : airlines) {
      lines
          .append("net=")
          .append(airline.net.netNumber)
          .append(" from=")
          .append(airline.fromItem.getId())
          .append(" to=")
          .append(airline.toItem.getId())
          .append(" fromCorner=")
          .append(Double.toString(airline.fromCorner.x))
          .append(',')
          .append(Double.toString(airline.fromCorner.y))
          .append(" toCorner=")
          .append(Double.toString(airline.toCorner.x))
          .append(',')
          .append(Double.toString(airline.toCorner.y))
          .append('\n');
    }
    Files.writeString(Path.of(args[1] + ".airlines.txt"), lines.toString(), StandardCharsets.UTF_8);
  }
}
