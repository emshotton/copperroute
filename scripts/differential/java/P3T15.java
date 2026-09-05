package app.freerouting.io.specctra;

import app.freerouting.board.BasicBoard;
import app.freerouting.board.FixedState;
import app.freerouting.board.Item;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.io.BoardReadResult;
import java.io.FileDescriptor;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

/**
 * Plan 3 Task 15 differential driver: reads one Specctra DSN with the real reader and then dumps
 * either the board it built or one of the three writers' output verbatim.
 *
 * <p>Usage: {@code P3T15 <file.dsn> <mode>}, with
 *
 * <ul>
 *   <li>{@code 0} — one line per board item in ascending id order, then the warnings list.
 *   <li>{@code 1} — {@code DsnWriter.write(board, out, stem, false)}, byte for byte.
 *   <li>{@code 2} — {@code SesWriter.write(board, out, stem)}, byte for byte.
 *   <li>{@code 3} — {@code RulesWriter.write(board, out, stem)}, byte for byte.
 *   <li>{@code 4} — the {@code p3t3} token stream, delegated to {@link P3T3} so one driver can
 *       sweep the corpus.
 * </ul>
 *
 * <p>The read is exactly {@code scripts/gen-reference/RefWriter.java}'s:
 * {@code DsnReader.readBoard(in, null, null, designName)} with {@code designName} the input
 * file's name with a trailing {@code .dsn} stripped. That is also what
 * {@code crates/fr-dsn/tests/parity_dsn.rs} does, so a diff here and a parity failure there have
 * the same cause.
 *
 * <p>Unlike {@code p3t3}, this driver goes through the whole parser, so it exercises the
 * hand-rolled {@code nextString}/{@code nextStringList}/{@code nextDouble} bypass path that
 * {@code p3t3}'s pure {@code next_token} loop never reaches (Task 3 review).
 *
 * <p>Compiled against the pinned {@code tools/freerouting-2.3.0.jar} (plan ruling 10), whose
 * package layout predates the clone's renames — hence {@code app.freerouting.board.BasicBoard}
 * rather than {@code board.facade.BasicBoard}, exactly as {@code RefWriter.java} imports it.
 */
public final class P3T15 {

  private P3T15() {}

  public static void main(String[] args) throws Exception {
    if (args.length < 2) {
      System.err.println("usage: P3T15 <file.dsn> <mode 0-4>");
      System.exit(2);
    }
    String path = args[0];
    int mode = Integer.parseInt(args[1]);

    if (mode == 4) {
      P3T3.main(new String[] {path});
      return;
    }

    // `FRLogger` logs to `System.out`, and the reader warns freely; bind this driver's own
    // output to the real stdout and silence `System.out` before FRLogger is loaded, exactly as
    // `P3T3` and the `*Probe.java` golden generators do.
    //
    // `System.err` is deliberately left alone. Java can *throw* out of this driver — reading a
    // board with no `(library …)` scope and then calling `RulesWriter.write` NPEs, see
    // `docs/java-quirks.md` — and the uncaught-exception handler's stack trace on stderr, plus
    // the non-zero exit status, are the only things that tell a caller "Java crashed" apart from
    // "Java disagreed". `sweep-p3t15.sh` redirects stderr to a side file and records both exit
    // codes for exactly that reason. Nothing on stderr can reach the diffed stream.
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    OutputStream raw = new FileOutputStream(FileDescriptor.out);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    String designName = Path.of(path).getFileName().toString().replaceAll("\\.dsn$", "");

    BoardReadResult result;
    try (InputStream in = new FileInputStream(path)) {
      result = DsnReader.readBoard(in, null, null, designName);
    }

    BasicBoard board;
    List<String> warnings;
    switch (result) {
      case BoardReadResult.Success s -> {
        board = s.board();
        warnings = s.warnings();
      }
      case BoardReadResult.OutlineMissing o -> {
        board = o.board();
        warnings = o.warnings();
      }
      case BoardReadResult.ParseError e -> {
        out.println("RESULT ParseError " + e.location() + " | " + e.detail());
        return;
      }
      default -> {
        out.println("RESULT " + result.getClass().getSimpleName());
        return;
      }
    }
    if (board == null) {
      out.println("RESULT NoBoard");
      return;
    }

    switch (mode) {
      case 0 -> dumpItems(out, board, warnings);
      case 1 -> {
        DsnWriter.write(board, raw, designName, false);
        raw.flush();
      }
      case 2 -> {
        SesWriter.write(board, raw, designName);
        raw.flush();
      }
      case 3 -> {
        RulesWriter.write(board, raw, designName);
        raw.flush();
      }
      default -> {
        System.err.println("unknown mode: " + mode);
        System.exit(2);
      }
    }
  }

  /**
   * One line per item: {@code item <id> <kind> layers=<first>..<last> nets=[..] cl=<class>
   * fixed=<state> cmp=<component> bbox=(llx,lly,urx,ury) tiles=<count>}, in ascending id order,
   * then {@code itemcount} and the warnings.
   */
  private static void dumpItems(PrintStream out, BasicBoard board, List<String> warnings) {
    out.println("layers " + board.get_layer_count());
    List<Item> items = new ArrayList<>(board.get_items());
    items.sort(Comparator.comparingInt(Item::get_id_no));
    for (Item it : items) {
      out.println("item " + it.get_id_no() + " " + describe(it));
    }
    out.println("itemcount " + items.size());
    for (String w : warnings) {
      out.println("warning " + w);
    }
  }

  private static String describe(Item it) {
    StringBuilder sb = new StringBuilder(it.getClass().getSimpleName());
    sb.append(" layers=").append(it.first_layer()).append("..").append(it.last_layer());
    sb.append(" nets=[");
    for (int i = 0; i < it.net_count(); i++) {
      sb.append(it.get_net_no(i)).append(',');
    }
    sb.append(']');
    sb.append(" cl=").append(it.clearance_class_no());
    FixedState fixed = it.get_fixed_state();
    sb.append(" fixed=").append(fixed == null ? "null" : fixed.name());
    sb.append(" cmp=").append(it.get_component_no());
    IntBox box = it.bounding_box();
    sb.append(" bbox=(")
        .append(box.ll.x)
        .append(',')
        .append(box.ll.y)
        .append(',')
        .append(box.ur.x)
        .append(',')
        .append(box.ur.y)
        .append(')');
    sb.append(" tiles=").append(it.tile_shape_count());
    return sb.toString();
  }
}
