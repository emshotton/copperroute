import app.freerouting.board.actions.ItemIdGenerator;
import app.freerouting.board.facade.BasicBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.structure.Component;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;
import app.freerouting.io.specctra.SesWriter;
import java.io.BufferedReader;
import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.FileDescriptor;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicBoolean;

// Plan 8 Task 13 ground-truth probe for the four zero-coverage Plan 3 paths
// (docs/plan-3-handoff.md's "register row", docs/java-quirks.md:464).
//
// It is not a differential driver: there is no Rust twin binary and `run.sh` does not know it —
// the `P7T15bProbe`/`P8T0Probe`/`P8T3Probe`/`P8T8Probe` pattern. Its stdout is committed verbatim,
// one section per path, as `crates/fr-dsn/tests/data/p8t13-directed-<path>.txt`, and replayed as
// literals by the four directed tests in `crates/fr-dsn/tests/`.
//
// The four paths and what this probe measures for each:
//
//   was-is           `SesWriter.writeWasIs`'s swap body (SesWriter.java:188-215). Unreachable
//                    from a `.dsn` file alone — `Pin.changedTo` is only ever moved by
//                    `Pin.swap(Pin)` (Pin.java:437-461), which has **no** live caller anywhere in
//                    the Java tree (`grep -rn 'swapWith\|\.swap(' src/main/java` finds none). The
//                    probe therefore reads the fixture, calls `Pin.swap` on two pins by hand, and
//                    prints `SesWriter.write`'s whole output — the only way the swap body has ever
//                    been executed.
//   lock-type        `Component.readLockType`'s `(lock_type position)` arm (Component.java:352-364)
//                    end to end: the fixture's placement scope, the resulting components'
//                    `isPositionFixed`, each pin's `FixedState`, and `SesWriter.write`'s
//                    `(lock_type position)` line back out (SesWriter.java:201-204).
//   conduction-area  `SesWriter.writeConductionArea` (SesWriter.java:536-553) — quirk #110's mixed
//                    integer boundary / `Double.toString` hole output in one SES scope.
//   via-net-numbers  quirk #105: `Wiring.readViaScope`'s net-number loop that never increments
//                    (Wiring.java:684-687), beside `readWireScope`'s identical loop that does
//                    (:441-445). The `[item]` rows print every item's net array, so the via's
//                    `[<n>, 0]` and the wire's `[<n1>, <n2>]` stand next to each other.
//
// No clock, no identity hash, no `System.identityHashCode`: `board.getItems()` and
// `board.getPins()` are both views of one insertion-ordered item list, so every row is byte-stable
// across runs and across `-XX:hashCode` settings. `[item]` rows are re-sorted by id ascending —
// HEAD hands them back descending, and that order is not what any of the four paths is about. The
// `[pinorder]` row **is** raw `getPins()` order, because `writeWasIs` iterates it directly and the
// `(pins …)` lines come out in it.
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p8t13 java/probes/P8T13Probe.java
//   cd ..                                    # the repo root: the probe resolves paths from it
//   for p in was-is lock-type conduction-area via-net-numbers; do \
//     "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//         -cp "/tmp/p8t13:$JAR" P8T13Probe "$p" \
//       > crates/fr-dsn/tests/data/p8t13-directed-"$p".txt; done
//
// It takes about a minute and a half: three fixtures route in seconds, `via-net-numbers` burns the
// whole CLI_TIMEOUT_SECONDS deadline.
//
// The jar-acceptance half of each transcript's evidence — the task brief's `java -jar <HEAD jar>
// -de <fixture> -do <out.ses>` — is run by the probe itself, as a subprocess, and transcribed as
// the `[jar-cli]` rows: the exit status, the `.ses` it produced and the distinct throwables and
// `app.freerouting` frames its log carried. Nothing timestamped, job-id-tagged or network-derived
// is transcribed, so those rows are byte-stable too. `via-net-numbers` never terminates (quirk
// #105's padded zero net number NPEs the autorouter's incomplete count on every pass), so the run
// is killed after CLI_TIMEOUT_SECONDS and the transcript says so.
public final class P8T13Probe {

  private static final String DATA = "crates/fr-dsn/tests/data";

  /**
   * How long the acceptance run gets before the probe kills it. Three of the four fixtures finish
   * in about three seconds; `via-net-numbers` never finishes at all.
   */
  private static final int CLI_TIMEOUT_SECONDS = 60;

  public static void main(String[] args) throws Exception {
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    // The reader logs through `FRLogger`, which writes to stdout/stderr; silence both so the
    // transcript is only the rows below.
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));
    System.setErr(new PrintStream(OutputStream.nullOutputStream()));

    String path = args.length > 0 ? args[0] : "was-is";
    File repoRoot = new File(".").getCanonicalFile();
    File jar =
        new File(repoRoot, "../freerouting/build/libs/freerouting-current-executable.jar")
            .getCanonicalFile();
    File dsn = new File(repoRoot, DATA + "/p8t13-" + path + ".dsn").getCanonicalFile();

    out.println("# P8T13Probe " + path + " — Plan 8 Task 13, HEAD jar");
    out.println("# fixture " + DATA + "/p8t13-" + path + ".dsn (" + dsn.length() + " bytes)");
    out.println("HEADER jar=" + jar.getName() + " size=" + jar.length());
    out.println();

    jarCli(jar, dsn, out);

    BasicBoard board = read(dsn, out);
    if (board == null) {
      return;
    }

    switch (path) {
      case "was-is" -> wasIs(board, dsn, out);
      case "lock-type" -> lockType(board, dsn, out);
      case "conduction-area" -> conductionArea(board, dsn, out);
      case "via-net-numbers" -> viaNetNumbers(board, out);
      default -> out.println("UNKNOWN PATH " + path);
    }
  }

  // --------------------------------------------------------------------------------- the paths

  /**
   * `SesWriter.writeWasIs` (SesWriter.java:183-218). Prints the empty `(was_is)` the CLI produces,
   * then swaps two pins with `Pin.swap` and prints the whole SES again.
   */
  private static void wasIs(BasicBoard board, File dsn, PrintStream out) throws Exception {
    // `writeWasIs` iterates `board.getPins()` raw, not by id — the `(pins …)` line order below is
    // that order, so the port has to reproduce it to match the bytes.
    StringBuilder order = new StringBuilder();
    for (Pin pin : board.getPins()) {
      order.append(pin.getId()).append(' ');
    }
    out.println("[pinorder] getPins() " + order.toString().trim());
    out.println();
    out.println("[pins] before the swap");
    for (Pin pin : sortedPins(board)) {
      out.println(
          "[pin] "
              + pin.getId()
              + " "
              + componentName(board, pin)
              + "-"
              + pin.name()
              + " pinIndex="
              + pin.getPinIndex()
              + " nets="
              + nets(pin)
              + " changedTo="
              + pin.getChangedTo().getId());
    }
    emitSes(board, dsn, "before", out);

    // `Pin.swap` has no live caller in the Java tree; this is the call the swap body needs.
    List<Pin> pins = sortedPins(board);
    Pin first = pins.get(0); // U1-A
    Pin last = pins.get(pins.size() - 1); // U2-B
    out.println();
    out.println(
        "[swap] "
            + componentName(board, first)
            + "-"
            + first.name()
            + " <-> "
            + componentName(board, last)
            + "-"
            + last.name()
            + " returned="
            + first.swap(last));
    out.println();

    out.println("[pins] after the swap");
    for (Pin pin : sortedPins(board)) {
      out.println(
          "[pin] "
              + pin.getId()
              + " "
              + componentName(board, pin)
              + "-"
              + pin.name()
              + " pinIndex="
              + pin.getPinIndex()
              + " nets="
              + nets(pin)
              + " changedTo="
              + pin.getChangedTo().getId());
    }
    emitSes(board, dsn, "after", out);
  }

  /** `Component.readLockType` (Component.java:352-364) end to end. */
  private static void lockType(BasicBoard board, File dsn, PrintStream out) throws Exception {
    for (int i = 1; i <= board.components.count(); i++) {
      Component c = board.components.get(i);
      out.println(
          "[component] "
              + i
              + " "
              + c.name
              + " positionFixed="
              + c.positionFixed
              + " placed="
              + c.isPlaced()
              + " front="
              + c.placedOnFront());
    }
    for (Pin pin : sortedPins(board)) {
      out.println(
          "[pin] "
              + pin.getId()
              + " "
              + componentName(board, pin)
              + "-"
              + pin.name()
              + " fixed="
              + pin.getFixedState()
              + " nets="
              + nets(pin));
    }
    emitSes(board, dsn, "ses", out);
  }

  /** `SesWriter.writeConductionArea` (SesWriter.java:536-553) — quirk #110. */
  private static void conductionArea(BasicBoard board, File dsn, PrintStream out) throws Exception {
    items(board, out);
    emitSes(board, dsn, "ses", out);
  }

  /** Quirk #105: `Wiring.readViaScope`'s net-number loop (Wiring.java:684-687). */
  private static void viaNetNumbers(BasicBoard board, PrintStream out) {
    for (int i = 1; i <= board.rules.nets.maxNetNumber(); i++) {
      app.freerouting.rules.Net net = board.rules.nets.get(i);
      out.println(
          "[net] "
              + i
              + " "
              + net.name
              + " subnet="
              + net.subnetNumber
              + " class="
              + net.getNetClass().getName()
              + " plane="
              + net.containsPlane());
    }
    items(board, out);
  }

  // ------------------------------------------------------------------------------- the helpers

  /**
   * The task brief's acceptance run: `java -jar <HEAD jar> -de <fixture> -do <out.ses>`, headless,
   * `en`/`US`. Killed after {@link #CLI_TIMEOUT_SECONDS} — `via-net-numbers` never terminates (see
   * that transcript's `[jar-cli]` rows), so the deadline is part of the evidence, not a shortcut.
   *
   * <p>Only three things are printed, all of them byte-stable: the exit status, the `.ses` the run
   * produced, and the **distinct** throwable and `app.freerouting` stack frames the log carried, in
   * first-seen order. Everything else the CLI logs is timestamped or carries a random job id, and
   * the "New version available" line depends on the network, so none of it is transcribed.
   */
  private static void jarCli(File jar, File dsn, PrintStream out) throws Exception {
    File ses = File.createTempFile("p8t13-", ".ses");
    if (!ses.delete()) {
      out.println("[jar-cli] could not clear " + ses);
    }
    ProcessBuilder pb =
        new ProcessBuilder(
            javaBinary(),
            "-Djava.awt.headless=true",
            "-Duser.language=en",
            "-Duser.country=US",
            "-jar",
            jar.getPath(),
            "-de",
            dsn.getPath(),
            "-do",
            ses.getPath());
    pb.redirectErrorStream(true);
    out.println(
        "[jar-cli] cmd=java -Djava.awt.headless=true -Duser.language=en -Duser.country=US"
            + " -jar <HEAD jar> -de "
            + DATA
            + "/"
            + dsn.getName()
            + " -do <out.ses>");
    Process proc = pb.start();
    AtomicBoolean killed = new AtomicBoolean(false);
    Thread watchdog =
        new Thread(
            () -> {
              try {
                if (!proc.waitFor(CLI_TIMEOUT_SECONDS, TimeUnit.SECONDS)) {
                  killed.set(true);
                  proc.destroyForcibly();
                }
              } catch (InterruptedException e) {
                Thread.currentThread().interrupt();
              }
            });
    watchdog.setDaemon(true);
    watchdog.start();

    LinkedHashSet<String> throwables = new LinkedHashSet<>();
    LinkedHashSet<String> frames = new LinkedHashSet<>();
    try (BufferedReader log =
        new BufferedReader(new InputStreamReader(proc.getInputStream(), StandardCharsets.UTF_8))) {
      String line;
      while ((line = log.readLine()) != null) {
        String trimmed = line.strip();
        if (trimmed.startsWith("java.") && trimmed.contains("Exception")) {
          throwables.add(trimmed);
        } else if (trimmed.startsWith("at app.freerouting.")) {
          frames.add(trimmed);
        }
      }
    }
    proc.waitFor();
    out.println(
        killed.get()
            ? "[jar-cli] exit=<none: still running after "
                + CLI_TIMEOUT_SECONDS
                + "s, killed by the probe>"
            : "[jar-cli] exit=" + proc.exitValue());
    for (String t : throwables) {
      out.println("[jar-cli] throwable " + t);
    }
    for (String f : frames) {
      out.println("[jar-cli] frame " + f);
    }
    if (ses.isFile()) {
      String text = Files.readString(ses.toPath(), StandardCharsets.UTF_8);
      out.println("[jar-cli] ses bytes=" + text.getBytes(StandardCharsets.UTF_8).length);
      for (String line : text.split("\n", -1)) {
        out.println("[jar-cli] ses|" + line);
      }
      if (!ses.delete()) {
        out.println("[jar-cli] could not remove " + ses);
      }
    } else {
      out.println("[jar-cli] ses <not written>");
    }
    out.println();
  }

  /** The JVM this probe itself runs on — the same JDK 25 the recipe in the header names. */
  private static String javaBinary() {
    return new File(new File(System.getProperty("java.home"), "bin"), "java").getPath();
  }

  private static BasicBoard read(File dsn, PrintStream out) throws Exception {
    BoardReadResult res;
    try (InputStream in = new FileInputStream(dsn)) {
      res = DsnReader.readBoard(in, null, new ItemIdGenerator(), dsn.getName());
    }
    if (res instanceof BoardReadResult.Success s) {
      out.println("[read] Success warnings=" + s.warnings().size());
      for (String w : s.warnings()) {
        out.println("[warning] " + w);
      }
      out.println();
      return s.board();
    }
    out.println("[read] " + res);
    return null;
  }

  /** Every item, **sorted by id ascending** — `board.getItems()` is descending at HEAD. */
  private static void items(BasicBoard board, PrintStream out) {
    List<Item> all = new ArrayList<>(board.getItems());
    all.sort(Comparator.comparingInt(Item::getId));
    for (Item item : all) {
      StringBuilder sb = new StringBuilder("[item] ");
      sb.append(item.getId())
          .append(' ')
          .append(item.getClass().getSimpleName())
          .append(" comp=")
          .append(item.getComponentId())
          .append(" cl=")
          .append(item.clearanceClassIndex())
          .append(" nets=")
          .append(nets(item))
          .append(" fixed=")
          .append(item.getFixedState());
      out.println(sb);
    }
    out.println("[itemcount] " + board.getItems().size());
    out.println();
  }

  private static void emitSes(BasicBoard board, File dsn, String label, PrintStream out)
      throws Exception {
    String designName = dsn.getName().replace(".dsn", "");
    ByteArrayOutputStream sink = new ByteArrayOutputStream();
    SesWriter.write(board, sink, designName);
    String ses = sink.toString(StandardCharsets.UTF_8);
    out.println();
    out.println("[ses " + label + "] bytes=" + ses.getBytes(StandardCharsets.UTF_8).length);
    for (String line : ses.split("\n", -1)) {
      out.println("[ses " + label + "]|" + line);
    }
  }

  private static List<Pin> sortedPins(BasicBoard board) {
    List<Pin> pins = new ArrayList<>(board.getPins());
    pins.sort(Comparator.comparingInt(Item::getId));
    return pins;
  }

  private static String componentName(BasicBoard board, Pin pin) {
    Component c = board.components.get(pin.getComponentId());
    return c == null ? "<null>" : c.name;
  }

  private static String nets(Item item) {
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < item.netCount(); i++) {
      if (i > 0) {
        sb.append(',');
      }
      sb.append(item.getNetNumber(i));
    }
    return sb.append(']').toString();
  }
}
