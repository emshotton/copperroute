package app.freerouting.autoroute.maze;

import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.state.Communication;
import app.freerouting.datastructures.IdGenerator;
import app.freerouting.settings.RouterSettings;
import app.freerouting.settings.sources.DefaultSettings;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;

/**
 * Plan 6 Task 17b probe: the per-allocation transcript of one connection's item ids.
 *
 * <p>Usage: {@code P6T17bProbe <dsn> <k> [ripupPassNo] [raw]}. Routes connections {@code 1..k} of
 * {@code dsn} exactly as {@link P6T1} does — same board, same settings, same connection list, same
 * {@code route} — and prints one line per {@code idGenerator.newId()} call made while connection
 * {@code k} is routed, followed by {@code CONNECTION} and that connection's own {@code P6T1} line.
 *
 * <p>This is the instrument Task 17's report asked for. The {@code router-dac2020-bm01} divergence
 * at {@code ripupPassNo >= 2} first showed as <b>six extra transient item ids</b> inside connection
 * 267 with byte-identical output geometry, and the ids alone cannot say which call site consumed
 * them. The probe answers that without patching a line of freerouting: it swaps a decorating {@link
 * IdGenerator} into {@code board.communication.idGenerator} — a {@code public final} instance
 * field, which {@code Field.setAccessible(true)} may still write — and walks the stack on every
 * {@code newId}. The id is allocated in {@code Item}'s constructor (Item.java:86-90), so the
 * interesting frame is the first one below the constructor chain.
 *
 * <p>Each line is {@code <ordinal> <id> <LABEL>}; with a fourth argument {@code raw} the compact
 * stack signature is appended, which is what makes the transcript alignable against the port's
 * (`Board::new_item_id` under `#[track_caller]`, or a `std::backtrace::Backtrace`). The labels are
 * the port's call-site vocabulary:
 *
 * <ul>
 *   <li>{@code SUB} — {@code ShapeTraceEntries.nextSubstituteTracePiece}
 *       (ShapeTraceEntries.java:272-281)
 *   <li>{@code FASTCUT} — {@code ShapeTraceEntries.fastCutoutTrace}
 *       (ShapeTraceEntries.java:112,130)
 *   <li>{@code INSTRACE} — {@code BasicBoard.insertTrace*} (BasicBoard.java:185-195)
 *   <li>{@code INSVIA} — {@code BasicBoard.insertVia} (BasicBoard.java:245-259)
 *   <li>{@code OTHER:<class>.<method>} — anything else, so a new site cannot hide in a bucket
 * </ul>
 *
 * <p>On {@code Issue508-DAC2020_bm01} at {@code k = 267}, {@code ripupPassNo = 2}, the jar prints
 * 1 483 lines (1 355 {@code SUB}, 70 {@code FASTCUT}, 58 {@code INSTRACE}); the transcript is
 * committed as {@code crates/fr-router/tests/data/p6t17b-dac2020-k267-pass2.txt}. Before Task 17b's
 * fix the port made six more {@code SUB} allocations; it now makes the same 1 483, in the same
 * order.
 *
 * <p>The probe deliberately stops there. Task 17b's bisect then went four levels deeper — the maze
 * queue, {@code expandToDoor}'s section segments, {@code completeExpansionRoom}'s candidates and
 * finally {@code MinAreaTree}'s pre-order shape — and each of those needs markers <i>inside</i>
 * Java. That was done by copying the five files concerned into a scratch directory, adding {@code
 * System.err.println}s, compiling them against the HEAD jar and putting the output directory
 * <b>in front of</b> the jar on the classpath, with the unmodified copies first checked to
 * reproduce {@code p6t1}'s output byte for byte. The Java tree itself is read-only and stays so;
 * see Task 17b's report for the exact markers.
 */
public final class P6T17bProbe {

  private P6T17bProbe() {}

  public static void main(String[] args) throws Exception {
    if (args.length < 2) {
      System.err.println("usage: P6T17bProbe <dsn> <k> [ripupPassNo] [raw]");
      System.exit(2);
    }
    // `FRLogger` writes to stdout on several corpus fixtures and the port has no logger to
    // reproduce them with; the same guard `P6T1.main` uses.
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    int k = Integer.parseInt(args[1]);
    int ripupPassNo = args.length > 2 ? Integer.parseInt(args[2]) : 1;
    boolean raw = args.length > 3 && "raw".equals(args[3]);

    RoutingBoard board = P6T1.loadBoard(dsn, null);
    RouterSettings settings = new DefaultSettings().getSettings();
    settings.setLayerCount(board.getLayerCount());
    settings.applyBoardSpecificOptimizations(board);

    List<P6T1.Connection> connections = P6T1.pickConnections(board, k);
    Tracer tracer = new Tracer(board.communication.idGenerator, out, raw);
    Field field = Communication.class.getDeclaredField("idGenerator");
    field.setAccessible(true);
    field.set(board.communication, tracer);

    // With `p6t17b-bisect.patch` applied and its classes ahead of the jar, the deeper markers
    // are switched on the same way: `MazeSearchEngine.P6T17B` for the connection under study and
    // `P6T17B_K` for every connection (the per-connection tree dump). Without the patch the two
    // fields do not exist, and the probe says so rather than failing obscurely.
    Field marker = null;
    Field markerK = null;
    Field markerChange = null;
    String deep = System.getenv("P6T17B_DEEP");
    boolean deepMarkers = deep != null;
    // `P6T17B_DEEP=all` widens the deep markers to every connection. Use it on a handful of
    // connections only: over a whole board its stderr runs to gigabytes.
    boolean deepAllConnections = "all".equals(deep);
    boolean treeDump = System.getenv("P6T17B_TREE") != null;
    // `PolylineTrace.change`'s CHANGE marker alone, for every connection — the task's pinning
    // measurement: 2358 `CHANGE` lines over the first 267 connections of
    // `Issue508-DAC2020_bm01` at `ripupPassNo = 2`, 1403 of them disagreeing between Java's
    // reference comparison and a value one. Separate from `P6T17B_DEEP=all`, whose other markers
    // cost gigabytes over a whole board.
    boolean changeMarker = System.getenv("P6T17B_CHANGE") != null;
    if (deepMarkers || treeDump || changeMarker) {
      try {
        Class<?> engine = Class.forName("app.freerouting.autoroute.maze.MazeSearchEngine");
        marker = engine.getDeclaredField("P6T17B");
        marker.setAccessible(true);
        markerK = engine.getDeclaredField("P6T17B_K");
        markerK.setAccessible(true);
        markerChange = engine.getDeclaredField("P6T17B_CHANGE");
        markerChange.setAccessible(true);
        markerChange.setBoolean(null, changeMarker);
      } catch (NoSuchFieldException e) {
        System.err.println(
            "P6T17B_DEEP/P6T17B_TREE/P6T17B_CHANGE need"
                + " scripts/differential/java/p6t17b-bisect.patch applied"
                + " and its classes ahead of the jar on the classpath; see that file's header.");
        System.exit(3);
      }
    }

    for (P6T1.Connection connection : connections) {
      tracer.active = connection.k() == k;
      if (marker != null) {
        // The tree dump runs for every connection; the rest only for the one under study.
        marker.setBoolean(null, deepMarkers && (deepAllConnections || tracer.active));
        markerK.setInt(null, connection.k());
      }
      String line = P6T1.routeOne(board, settings, connection, ripupPassNo);
      if (connection.k() == k) {
        out.println("CONNECTION " + line);
        out.flush();
      }
    }
  }

  /** A decorating {@link IdGenerator} that prints the call site of every allocation. */
  static final class Tracer implements IdGenerator {

    private final IdGenerator delegate;
    private final PrintStream out;
    private final boolean raw;
    private int ordinal;
    boolean active;

    Tracer(IdGenerator delegate, PrintStream out, boolean raw) {
      this.delegate = delegate;
      this.out = out;
      this.raw = raw;
    }

    @Override
    public int newId() {
      int id = delegate.newId();
      if (active) {
        List<StackWalker.StackFrame> frames =
            StackWalker.getInstance(StackWalker.Option.RETAIN_CLASS_REFERENCE)
                .walk(
                    stream ->
                        stream
                            .filter(f -> f.getClassName().startsWith("app.freerouting."))
                            .filter(f -> !f.getMethodName().equals("<init>"))
                            .limit(40)
                            .toList());
        out.print(++ordinal);
        out.print(' ');
        out.print(id);
        out.print(' ');
        out.print(label(frames));
        if (raw) {
          out.print(' ');
          out.print(signature(frames));
        }
        out.println();
      }
      return id;
    }

    @Override
    public int maxGeneratedId() {
      return delegate.maxGeneratedId();
    }

    /** The first frame below the {@code Item} constructor chain, in the port's vocabulary. */
    private static String label(List<StackWalker.StackFrame> frames) {
      for (StackWalker.StackFrame frame : frames) {
        String cls = simple(frame.getClassName());
        if (cls.startsWith("P6T17bProbe")) {
          continue;
        }
        return switch (cls + "." + frame.getMethodName()) {
          case "ShapeTraceEntries.nextSubstituteTracePiece" -> "SUB";
          case "ShapeTraceEntries.fastCutoutTrace" -> "FASTCUT";
          case "BasicBoard.insertTrace",
                  "BasicBoard.insertTraceWithoutCleaning",
                  "BoardItemRepository.insertTrace",
                  "BoardItemRepository.insertTraceWithoutCleaning" ->
              "INSTRACE";
          case "BasicBoard.insertVia", "BoardItemRepository.insertVia" -> "INSVIA";
          default -> "OTHER:" + cls + "." + frame.getMethodName();
        };
      }
      return "OTHER:none";
    }

    private static String signature(List<StackWalker.StackFrame> frames) {
      StringBuilder sb = new StringBuilder();
      for (StackWalker.StackFrame frame : frames) {
        if (sb.length() > 0) {
          sb.append('|');
        }
        sb.append(simple(frame.getClassName()))
            .append('.')
            .append(frame.getMethodName())
            .append(':')
            .append(frame.getLineNumber());
      }
      return sb.toString();
    }

    private static String simple(String className) {
      int dot = className.lastIndexOf('.');
      return dot < 0 ? className : className.substring(dot + 1);
    }
  }
}
