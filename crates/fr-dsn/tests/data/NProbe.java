import app.freerouting.board.*;
import app.freerouting.datastructures.IdentificationNumberGenerator;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;
import app.freerouting.rules.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

public final class NProbe {
  static final class Gen implements IdentificationNumberGenerator {
    private int last = 0;
    public int new_no() { return ++last; }
    public int max_generated_no() { return last; }
  }

  public static void main(String[] args) throws Exception {
    PrintStream out = new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));
    System.setErr(new PrintStream(OutputStream.nullOutputStream()));
    int limit = args.length > 1 ? Integer.parseInt(args[1]) : Integer.MAX_VALUE;
    BoardReadResult res;
    try (InputStream in = new FileInputStream(args[0])) {
      res = DsnReader.readBoard(in, new BoardObserverAdaptor(), new Gen());
    }
    if (!(res instanceof BoardReadResult.Success s)) {
      out.println("RESULT " + res);
      return;
    }
    BasicBoard board = s.board();
    out.println("layers " + board.get_layer_count());
    List<Item> items = new ArrayList<>(board.get_items());
    items.sort(Comparator.comparingInt(Item::get_id_no));
    int n = 0;
    for (Item it : items) {
      if (n++ >= limit) break;
      out.println("item " + it.get_id_no() + " " + describe(it, board));
    }
    out.println("itemcount " + items.size());
    for (int i = 1; i <= board.rules.nets.max_net_no(); i++) {
      app.freerouting.rules.Net net = board.rules.nets.get(i);
      out.println("net " + i + " " + net.name + " subnet=" + net.subnet_number
          + " class=" + net.get_class().get_name() + " plane=" + net.contains_plane());
    }
    app.freerouting.core.Padstack[] vps = board.library.get_via_padstacks();
    StringBuilder sb = new StringBuilder();
    for (app.freerouting.core.Padstack p : vps) sb.append(p.name).append(" ");
    out.println("viapadstacks[" + vps.length + "] " + sb.toString().trim());
    for (int i = 0; i < board.rules.via_infos.count(); i++) {
      ViaInfo v = board.rules.via_infos.get(i);
      out.println("viainfo " + i + " " + v.get_name() + " padstack=" + v.get_padstack().name
          + " cl=" + v.get_clearance_class() + " attach=" + v.attach_smd_allowed());
    }
    int vr = 0;
    for (ViaRule r : board.rules.via_rules) {
      StringBuilder b2 = new StringBuilder();
      for (int i = 0; i < r.via_count(); i++) b2.append(r.get_via(i).get_name()).append(" ");
      out.println("viarule " + (vr++) + " " + r.name + " [" + b2.toString().trim() + "]");
    }
    for (int i = 0; i < board.rules.net_classes.count(); i++) {
      NetClass nc = board.rules.net_classes.get(i);
      out.println("netclass " + i + " " + nc.get_name() + " traceCl=" + nc.get_trace_clearance_class()
          + " viaRule=" + (nc.get_via_rule() == null ? "null" : nc.get_via_rule().name)
          + " hw=" + halfWidths(nc)
          + " pullTight=" + nc.get_pull_tight() + " shoveFixed=" + nc.is_shove_fixed()
          + " minLen=" + nc.get_minimum_trace_length() + " maxLen=" + nc.get_maximum_trace_length());
    }
    ClearanceMatrix cm = board.rules.clearance_matrix;
    StringBuilder cn = new StringBuilder();
    for (int i = 0; i < cm.get_class_count(); i++) cn.append(i).append(':').append(cm.get_name(i)).append(' ');
    out.println("clclasses[" + cm.get_class_count() + "] " + cn.toString().trim());
    for (int i = 0; i < cm.get_class_count(); i++) {
      StringBuilder row = new StringBuilder();
      for (int j = 0; j < cm.get_class_count(); j++) row.append(cm.get_value(i, j, 0, false)).append(' ');
      out.println("clrow " + i + " " + row.toString().trim());
    }
    for (int i = 1; i <= board.components.count(); i++) {
      Component c = board.components.get(i);
      out.println("component " + i + " " + c.name + " pkg=" + c.get_package().name
          + " placed=" + c.is_placed() + " front=" + c.placed_on_front()
          + " lp=" + (c.get_logical_part() == null ? "null" : c.get_logical_part().name));
    }
    for (int i = 0; i < board.library.logical_parts.count(); i++) {
      out.println("logicalpart " + i + " " + board.library.logical_parts.get(i).name);
    }
    for (String w : s.warnings()) out.println("warning " + w);
  }

  static String halfWidths(NetClass nc) {
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < nc.layer_count(); i++) {
      if (i > 0) sb.append(',');
      sb.append(nc.get_trace_half_width(i));
    }
    return sb.append(']').toString();
  }

  static String describe(Item it, BasicBoard board) {
    String kind = it.getClass().getSimpleName();
    StringBuilder sb = new StringBuilder(kind);
    if (it instanceof ObstacleArea a) {
      sb.append(" layer=").append(a.get_layer()).append(" name=").append(a.name);
    } else if (it instanceof ComponentOutline co) {
      sb.append(" layer=").append(co.get_layer()).append(" courtyard=").append(co.is_courtyard());
    } else if (it instanceof Pin p) {
      sb.append(" pin=").append(p.name());
    } else if (it instanceof ConductionArea ca) {
      sb.append(" layer=").append(ca.get_layer());
    } else if (it instanceof PolylineTrace t) {
      sb.append(" layer=").append(t.get_layer())
        .append(" hw=").append(t.get_half_width())
        .append(" corners=").append(t.corner_count())
        .append(" first=").append(t.first_corner())
        .append(" last=").append(t.last_corner())
        .append(" fixed=").append(t.get_fixed_state());
    } else if (it instanceof Via v) {
      sb.append(" padstack=").append(v.get_padstack().name)
        .append(" at=").append(v.get_center())
        .append(" layers=").append(v.first_layer()).append("..").append(v.last_layer())
        .append(" attach=").append(v.attach_allowed)
        .append(" fixed=").append(v.get_fixed_state());
    }
    sb.append(" cmp=").append(it.get_component_no());
    sb.append(" cl=").append(it.clearance_class_no());
    StringBuilder nets = new StringBuilder();
    for (int i = 0; i < it.net_count(); i++) nets.append(it.get_net_no(i)).append(',');
    sb.append(" nets=[").append(nets).append(']');
    return sb.toString();
  }
}
