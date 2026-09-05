package app.freerouting.autoroute.pipeline;

import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.settings.RouterSettings;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;

/**
 * Plan 7 Task 9 differential driver, item-selection level: {@code
 * BatchAutorouter.getAutorouteItems} (BatchAutorouter.java:345-409) over a real DSN board, against
 * the port's {@code BatchAutorouter::autoroute_items}.
 *
 * <p>Usage: {@code P7T1 <dsn> [passNo]}. {@code passNo} defaults to {@code 1}.
 *
 * <h2>Why {@code passNo} is an argument when {@code getAutorouteItems} does not take one</h2>
 *
 * <p>The method's answer depends on the <b>board</b>, not on the pass number — but the board a
 * pass sees is the board the previous passes left. So this driver runs {@code passNo - 1} real
 * autoroute passes through {@code AutoroutePassRunner.runSingleThread} first, and prints the item
 * selection the <i>next</i> pass would compute. {@code passNo = 1} is therefore the freshly loaded
 * board and needs no routing at all. The port's twin drives its own pass runner the same way, so
 * a divergence at pass 3 that is really a pass-2 routing divergence still shows up here — which is
 * the point of running the driver at 1, 2 and 3 rather than only at 1.
 *
 * <h2>What is printed</h2>
 *
 * <p>Line 1 is {@code HEADER …}, the {@code p6t1} convention: the jar the port is a port of, its
 * size and mtime, the fixture and the argument. Then, <b>before any routing of the pass being
 * reported</b>:
 *
 * <ul>
 *   <li>{@code COUNT <n>} — the length of {@code autorouteItemList};
 *   <li>one {@code ITEM <ordinal> <itemId> <class> nets=<n> [<net>,…]} line per entry <b>in list
 *       order</b>. {@code Java's list is a List<Item>}, so an item that qualified on two nets
 *       appears twice, at two ordinals, with the same id and the same net list — which is exactly
 *       plan-7 ruling 10's bug and the thing this driver exists to pin. The nets are the ones
 *       {@code AutoroutePassRunner:207} will loop over, i.e. {@code item.getNetNumber(0
 *       .. netCount()-1)} read off the item, <b>not</b> off the list;
 *   <li>{@code HANDLED <ordinal> <size> [<id>,…]} — the {@code handledItems} set after each net
 *       index of each <i>examined</i> item, which is the state that decides whether a later item
 *       is skipped at {@code :359}. It is printed with an ordinal that counts examined
 *       {@code (item, net index)} steps, not appended entries, because the set grows at
 *       {@code :366-370} whether or not the item is appended.
 * </ul>
 *
 * <p>The {@code handledItems} trace is what makes the driver diagnostic rather than merely a
 * verdict: {@code getAutorouteItems} has exactly two pieces of hidden state, the set and the
 * append, and printing only the second would localise nothing.
 *
 * <h2>Reflection, and why it is needed</h2>
 *
 * <p>{@code getAutorouteItems} is package-private, which this driver's package reaches directly.
 * {@code handledItems} is a private field ({@code reusableHandledItems}, BatchAutorouter.java:74)
 * that the method <b>reuses</b> across calls (it clears it at {@code :348}), so its post-call
 * contents are exactly the set the last call built — no reflection into the method's internals is
 * needed, only {@code Field.setAccessible(true)} on the field. That reuse is the one place where
 * Java's allocation optimisation is observable from outside, and the driver uses it rather than
 * instrumenting the method.
 *
 * <p>Per-step {@code HANDLED} lines cannot be read off that field, because it only holds the
 * <i>final</i> set. They are produced by re-running the method's own set-building rule beside it
 * — {@code getConnectedSet(net)} then {@code netCount() <= 1} — which is a transcription, not an
 * observation, and the {@code HANDLED-FINAL} line at the end checks the transcription against the
 * real field. If the two ever disagreed, the driver would be lying and the final line would say
 * so on both sides.
 *
 * <h2>The budget</h2>
 *
 * <p>Ruling AI asks parity runs to disable the wall clock on both sides. As {@code P7T8Probe}'s
 * class comment records, the Java side cannot: {@code TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP} is a
 * compile-time constant that {@code javac} inlines. The port runs with {@code
 * RouterBudget::disabled()} against this side's live 1000 ms limit, so a MATCH proves the limit
 * never trips on the corpus.
 */
public final class P7T1 {

  private P7T1() {}

  public static void main(String[] args) throws Exception {
    if (args.length < 1) {
      System.err.println("usage: P7T1 <dsn> [passNo]");
      System.exit(2);
    }
    // `FRLogger` writes to stdout on several corpus fixtures; same guard as `P6T1.java`.
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    int passNo = args.length > 1 ? Integer.parseInt(args[1]) : 1;

    Path jar =
        Paths.get(RoutingBoard.class.getProtectionDomain().getCodeSource().getLocation().toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d fixture=%s passNo=%d%n",
        jar,
        Files.size(jar),
        Files.getLastModifiedTime(jar).toMillis(),
        dsn.getFileName(),
        passNo);
    System.err.println("java-version " + System.getProperty("java.version"));

    RoutingBoard board = P7T2.loadBoard(dsn);
    RouterSettings settings = P7T2.buildSettings(board);
    BatchAutorouter router = P7T2.newRouter(board, settings);

    // The board this pass sees is the board the previous passes left; see the class comment.
    for (int previous = 1; previous < passNo; previous++) {
      P7T2.runPass(router, previous);
    }

    P7T2.dumpAutorouteItems(out, router, board);
  }

  /** {@code P7T2.dumpAutorouteItems}'s answer, so the two drivers cannot drift apart. */
  static List<Item> autorouteItems(BatchAutorouter router, RoutingBoard board) {
    return router.getAutorouteItems(board);
  }
}
