// Plan 5 Task 4 JVM probe: `DesignRulesChecker.getAllUnconnectedItems()`, normalised.
//
// The list Java returns is **not** reproducible run to run: `connectedSets` is a
// `HashSet<Item>` (DesignRulesChecker.java:118) over a class with no `hashCode` override, so
// `allItems`' order and `findRepresentativeItem`'s choice among equal-kind candidates
// (`:188-197`) are identity-hash ordered, and `itemsByNet` is a `HashMap` (`:93`) whose
// iteration order is table-size dependent. Plan-5 ruling 3 verified all of that on the JVM with
// `-XX:hashCode=0..4`.
//
// So this probe prints the **hash-independent projection** of the result — the part the port can
// be held to — and nothing else:
//
//   unconnectedItems <n>
//   net=<netNumber> first=<Pin|Trace|other> second=<Pin|Trace|other> items=<id,id,…>
//   track_dangling_candidates <n>
//   first=<id>
//   via_dangling <n>
//   first=<id>
//
// with the `unconnectedItems` lines sorted by net number (at most one entry per net, `:131-147`),
// each `items` list sorted ascending, and each representative reduced to its **kind class**: a
// set holding a `Pin` always yields a `Pin` and a set holding no `Pin` but a `Trace` always
// yields a `Trace` (`:188-195`), so the class is hash-independent even though the item is not.
// The two dangling blocks are printed in full, in Java's own order — `board.getItems()`,
// descending id (BasicBoard.java:603-605).
//
// `track_dangling_candidates` is the trace phase **before its dedup** (`:154-160`, without
// `:162`): every trace whose start or end contact set is empty. The emitted `track_dangling`
// count is *not* hash-independent, because the dedup drops a trace that some net entry's
// `firstItem` happens to be (`:162`) and `findRepresentativeItem` picks that item out of a
// `HashSet` — measured on this jar, the Natural Tone Preamp fixture emits 111 under
// `-XX:hashCode=0,1,2,4`, 109 under `-XX:hashCode=3` and 110 under the default. The candidate
// set is what both sides can be held to; the port's own emitted count is asserted separately, in
// `crates/fr-drc/tests/unconnected.rs`, as a regression guard on its ascending-id divergence
// (plan-5 ruling 3). `via_dangling` has no dedup at all (`:168-174`), so it is printed as
// emitted.
//
// The constructor's second argument is `null`, as every headless caller passes it; the port
// drops the parameter (plan-5 ruling 12). `UnconnectedItemsReproductionTest.java:104` passes a
// fresh `DesignRulesCheckerSettings()` instead, which changes nothing: the field is never read.
//
// The transcript is written to the file named by the **second** argument, not to stdout: on the
// Natural Tone Preamp fixture `FRLogger` writes `DSN file 'test' was loaded with 152 warning(s).`
// to stdout ahead of the first line, and the port has no logger to reproduce it with.
//
// Usage: java -Djava.awt.headless=true -cp <jar>:. UnconnectedProbe <board.dsn> <out.txt>
import app.freerouting.board.facade.BasicBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.items.Trace;
import app.freerouting.drc.DesignRulesChecker;
import app.freerouting.drc.UnconnectedItems;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;
import java.io.FileInputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.Comparator;
import java.util.List;

public class UnconnectedProbe {
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
    Collection<UnconnectedItems> all = drc.getAllUnconnectedItems();

    List<UnconnectedItems> nets = new ArrayList<>();
    List<UnconnectedItems> tracks = new ArrayList<>();
    List<UnconnectedItems> vias = new ArrayList<>();
    for (UnconnectedItems ui : all) {
      switch (ui.type) {
        case "track_dangling" -> tracks.add(ui);
        case "via_dangling" -> vias.add(ui);
        default -> nets.add(ui);
      }
    }
    nets.sort(Comparator.comparingInt(ui -> netOf(ui.firstItem)));

    StringBuilder sb = new StringBuilder();
    sb.append("unconnectedItems ").append(nets.size()).append('\n');
    for (UnconnectedItems ui : nets) {
      List<Integer> ids = new ArrayList<>();
      for (Item item : ui.allItems) {
        ids.add(item.getId());
      }
      ids.sort(Comparator.naturalOrder());
      sb.append("net=")
          .append(netOf(ui.firstItem))
          .append(" first=")
          .append(kind(ui.firstItem))
          .append(" second=")
          .append(kind(ui.secondItem))
          .append(" items=");
      for (int i = 0; i < ids.size(); i++) {
        if (i > 0) {
          sb.append(',');
        }
        sb.append(ids.get(i));
      }
      sb.append('\n');
    }
    List<Integer> candidates = new ArrayList<>();
    for (Item item : board.getItems()) {
      if (item instanceof Trace trace
          && (trace.getStartContacts().isEmpty() || trace.getEndContacts().isEmpty())) {
        candidates.add(trace.getId());
      }
    }
    sb.append("track_dangling_candidates ").append(candidates.size()).append('\n');
    for (int id : candidates) {
      sb.append("first=").append(id).append('\n');
    }
    sb.append("via_dangling ").append(vias.size()).append('\n');
    for (UnconnectedItems ui : vias) {
      sb.append("first=").append(ui.firstItem.getId()).append('\n');
    }
    Files.writeString(Path.of(args[1]), sb.toString(), StandardCharsets.UTF_8);
    // Informational, on stderr so it stays out of the transcript: the hash-dependent count.
    System.err.println("info: emitted track_dangling=" + tracks.size());
  }

  private static int netOf(Item item) {
    return item.netCount() > 0 ? item.getNetNumber(0) : 0;
  }

  private static String kind(Item item) {
    if (item == null) {
      return "none";
    }
    if (item instanceof Pin) {
      return "Pin";
    }
    if (item instanceof Trace) {
      return "Trace";
    }
    return "other";
  }
}
