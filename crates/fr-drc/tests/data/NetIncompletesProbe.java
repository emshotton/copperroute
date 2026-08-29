// Plan 5 Task 5 JVM probe: `drc.NetIncompletes`, per net.
//
// Plan-5 ruling 4 splits this class's output in two:
//
//   * `count()`, `getConnectedGroupCount()` and `getLengthViolation()` are **hash-independent**
//     — they are graph invariants of the net plus its net class's length limits — and are the
//     parity surface. They go into `<stem>.netincompletes.txt`, which
//     `the_three_fixtures_match_the_jvm` (`crates/fr-drc/tests/net_incompletes.rs`) compares byte
//     for byte.
//   * the airlines themselves are **not**. `NetIncompletes.calculateNetItems` seeds its outer
//     loop from a `HashSet<Item>` (NetIncompletes.java:295, :299) over a class with no
//     `hashCode` override, so the Delaunay corner insertion order — and therefore which of
//     several equal-length edges the spanning tree accepts — is identity-hash ordered. Ruling 4
//     measured five different airline lists under `-XX:hashCode=0..4` on the dev board with an
//     identical `incompleteCount`. They go into `<stem>.airlines.txt`, which is committed for
//     **one** run and read by no assertion; the port's own list is diffed against it by hand and
//     the differences classified (see `tests/data/README.md`).
//
// Transcript format (`.netincompletes.txt`):
//
//   nets <maxNetNumber>
//   net=<n> items=<filtered-input-size> count=<airlines> groups=<connectedGroupCount> \
//       lengthViolation=<Double.toString> markerRadius=<Double.toString>
//   airlines <getAllAirlines().length>
//   incompleteCount <getIncompleteCount()>
//
// with one `net=` line per net number 1..maxNetNumber, in ascending order. `items=` is the size
// of the collection `calculateAllIncompletes` passes the constructor (`:550-563`,
// `:617-621`) — reproduced here rather than read out of the object, which does not keep it — so
// that a port whose *input* differs is distinguishable from one whose ratsnest differs.
//
// The transcript is written to files, not stdout: `FRLogger` prints a warning line to stdout on
// one of the fixtures and the port has no logger to reproduce it with.
//
// Usage: java -Djava.awt.headless=true -cp <jar>:. NetIncompletesProbe <board.dsn> <out-stem>
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

    // The per-net input sizes, exactly as `calculateAllIncompletes` builds them
    // (DesignRulesChecker.java:544-563): every `Connectable` item, once per net it carries.
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

    // Informational only — hash-dependent, asserted by nothing.
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
