import app.freerouting.board.*;
import app.freerouting.datastructures.IdentificationNumberGenerator;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;
import app.freerouting.io.specctra.RulesReader;
import app.freerouting.io.specctra.RulesWriter;
import app.freerouting.rules.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

public final class RProbe {
  static final class Gen implements IdentificationNumberGenerator {
    private int last = 0;

    public int new_no() {
      return ++last;
    }

    public int max_generated_no() {
      return last;
    }
  }

  public static void main(String[] args) throws Exception {
    OutputStream rawOut = new FileOutputStream(FileDescriptor.out);
    PrintStream out = new PrintStream(rawOut, true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));
    System.setErr(new PrintStream(OutputStream.nullOutputStream()));

    String mode = args[0];
    BoardReadResult res;
    try (InputStream in = new FileInputStream(args[1])) {
      res = DsnReader.readBoard(in, new BoardObserverAdaptor(), new Gen());
    }
    if (!(res instanceof BoardReadResult.Success s)) {
      out.println("RESULT " + res);
      return;
    }
    BasicBoard board = s.board();

    boolean ok = true;
    if (!args[2].equals("-")) {
      try (InputStream rulesIn = new FileInputStream(args[2])) {
        ok = RulesReader.read(rulesIn, args[3], board);
      }
    }

    if (mode.equals("write")) {
      RulesWriter.write(board, rawOut, args[3]);
      rawOut.flush();
      return;
    }
    if (mode.equals("writesettings")) {
      app.freerouting.settings.RouterSettings settings = new app.freerouting.settings.RouterSettings();
      settings.setLayerCount(board.get_layer_count());
      settings.set_via_costs(75);
      settings.set_plane_via_costs(8);
      settings.set_start_ripup_costs(120);
      settings.set_layer_active(0, true);
      settings.set_layer_active(1, true);
      settings.set_preferred_direction_is_horizontal(0, true);
      settings.set_preferred_direction_is_horizontal(1, false);
      settings.set_preferred_direction_trace_costs(0, 1.2);
      settings.set_against_preferred_direction_trace_costs(0, 2.8);
      settings.set_preferred_direction_trace_costs(1, 1.0);
      settings.set_against_preferred_direction_trace_costs(1, 3.1);
      writeRulesWithSettings(board, settings, rawOut, args[3]);
      rawOut.flush();
      return;
    }

    if (mode.equals("divergence")) {
      for (ViaRule r : board.rules.via_rules) {
        for (int i = 0; i < r.via_count(); i++) {
          ViaInfo v = r.get_via(i);
          out.println(
              "rulevia "
                  + r.name
                  + " "
                  + v.get_name()
                  + " attach="
                  + v.attach_smd_allowed()
                  + " cl="
                  + v.get_clearance_class()
                  + " inList="
                  + (board.rules.via_infos.get(v.get_name()) == v)
                  + " id="
                  + System.identityHashCode(v));
        }
      }
      return;
    }

    out.println("read " + ok);
    out.println("snapangle " + board.rules.get_trace_angle_restriction());
    out.println("defaulthw " + halfWidthsOfDefault(board));

    app.freerouting.core.Padstack[] vps = board.library.get_via_padstacks();
    StringBuilder sb = new StringBuilder();
    for (app.freerouting.core.Padstack p : vps) sb.append(p.name).append(" ");
    out.println("viapadstacks[" + vps.length + "] " + sb.toString().trim());
    out.println("padstacks " + board.library.padstacks.count());

    for (int i = 0; i < board.rules.via_infos.count(); i++) {
      ViaInfo v = board.rules.via_infos.get(i);
      out.println(
          "viainfo "
              + i
              + " "
              + v.get_name()
              + " padstack="
              + v.get_padstack().name
              + " cl="
              + v.get_clearance_class()
              + " attach="
              + v.attach_smd_allowed());
    }

    int vr = 0;
    for (ViaRule r : board.rules.via_rules) {
      StringBuilder b2 = new StringBuilder();
      for (int i = 0; i < r.via_count(); i++) b2.append(r.get_via(i).get_name()).append(" ");
      out.println("viarule " + (vr++) + " " + r.name + " [" + b2.toString().trim() + "]");
    }

    for (int i = 0; i < board.rules.net_classes.count(); i++) {
      NetClass nc = board.rules.net_classes.get(i);
      out.println(
          "netclass "
              + i
              + " "
              + nc.get_name()
              + " traceCl="
              + nc.get_trace_clearance_class()
              + " viaRule="
              + (nc.get_via_rule() == null ? "null" : nc.get_via_rule().name)
              + " hw="
              + halfWidths(nc)
              + " pullTight="
              + nc.get_pull_tight()
              + " shoveFixed="
              + nc.is_shove_fixed()
              + " minLen="
              + nc.get_minimum_trace_length()
              + " maxLen="
              + nc.get_maximum_trace_length());
    }

    ClearanceMatrix cm = board.rules.clearance_matrix;
    StringBuilder cn = new StringBuilder();
    for (int i = 0; i < cm.get_class_count(); i++)
      cn.append(i).append(':').append(cm.get_name(i)).append(' ');
    out.println("clclasses[" + cm.get_class_count() + "] " + cn.toString().trim());
    for (int layer = 0; layer < board.get_layer_count(); layer++) {
      for (int i = 0; i < cm.get_class_count(); i++) {
        StringBuilder row = new StringBuilder();
        for (int j = 0; j < cm.get_class_count(); j++)
          row.append(cm.get_value(i, j, layer, false)).append(' ');
        out.println("clrow " + layer + " " + i + " " + row.toString().trim());
      }
    }
    out.println("pinedge " + board.rules.get_pin_edge_to_turn_dist());
  }

  static void writeRulesWithSettings(
      BasicBoard board,
      app.freerouting.settings.RouterSettings settings,
      OutputStream out,
      String designName)
      throws IOException {
    app.freerouting.datastructures.IndentFileWriter file =
        new app.freerouting.datastructures.IndentFileWriter(out);
    app.freerouting.io.specctra.parser.WriteScopeParameter p =
        new app.freerouting.io.specctra.parser.WriteScopeParameter(
            board,
            settings,
            file,
            board.communication.specctra_parser_info.string_quote,
            board.communication.coordinate_transform,
            false);
    file.start_scope();
    file.write("rules PCB ");
    file.write(designName);
    app.freerouting.io.specctra.parser.Structure.write_snap_angle(
        file, board.rules.get_trace_angle_restriction());
    app.freerouting.io.specctra.parser.AutorouteSettings.write_scope(
        file, settings, board.layer_structure, p.identifier_type);
    app.freerouting.io.specctra.parser.Rule.write_default_rule(p, 0);
    for (int i = 1; i <= board.library.padstacks.count(); i++) {
      app.freerouting.core.Padstack currentPadstack = board.library.padstacks.get(i);
      if (board.library.get_via_padstack(currentPadstack.name) != null) {
        app.freerouting.io.specctra.parser.Library.write_padstack_scope(p, currentPadstack);
      }
    }
    app.freerouting.io.specctra.parser.Network.write_via_infos(
        board.rules, file, p.identifier_type);
    app.freerouting.io.specctra.parser.Network.write_via_rules(board.rules, file, p.identifier_type);
    app.freerouting.io.specctra.parser.Network.write_net_classes(p);
    file.end_scope();
    file.flush();
  }

  static String halfWidthsOfDefault(BasicBoard board) {
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < board.get_layer_count(); i++) {
      if (i > 0) sb.append(',');
      sb.append(board.rules.get_default_trace_half_width(i));
    }
    return sb.append(']').toString();
  }

  static String halfWidths(NetClass nc) {
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < nc.layer_count(); i++) {
      if (i > 0) sb.append(',');
      sb.append(nc.get_trace_half_width(i));
    }
    return sb.append(']').toString();
  }
}
