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
