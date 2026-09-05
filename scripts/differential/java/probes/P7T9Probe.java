package app.freerouting.autoroute;

import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Random;

// Plan 7 Task 9 ground-truth probe: `autoroute/ItemRouteResult.java` (145 lines) in full — the
// improvement ladder the **constructor** runs (`:39-57`), the `improvementPercentage` expression
// (`:59-65`) with its integer-division bug (quirk #212), `compareTo` (`:69-89`), `improvedOver`
// (`:92-94`), `viaCountReduced` (`:127-129`), `lengthReduced` (`:132-134`), `updateImproved`
// (`:137-139`) and the one-argument constructor (`:17-20`).
//
// It is not a differential driver — there is no Rust twin and `run.sh` does not know it, the
// `P7T2Probe`/`P7T4Probe` pattern. Its stdout is committed verbatim as
// `crates/fr-router/tests/data/p7t9-item-route-result.txt` and replayed by
// `crates/fr-router/tests/item_route_result.rs`.
//
// ## Why 500 scripted tuples and not a corpus
//
// `ItemRouteResult` has no live caller a corpus run could observe it through:
// `BatchOptimizer.optRouteItem` builds one per optimized item and reads `improved()`, but it
// recomputes `improvementPercentage`'s expression itself with a `(float)` cast
// (`BatchOptimizer.java:340-348`) rather than reading the field, and `compareTo` is reached only
// from the GUI-only `BatchAutorouterThread`'s `PriorityQueue`. So the class is pinned the way a
// pure function is pinned: over a scripted domain that reaches every arm of the ladder and both
// sides of every comparison. The tuples come from `new Random(4919)` over deliberately small
// integer ranges (0..5) and a fixed length table that includes `0.0`, so ties on each rung are
// frequent — a tie is what selects the *next* rung, and an untied domain would test one
// comparison instead of three.
//
// ## The transcript carries the inputs as well as the answers
//
// The Rust twin does not reproduce `java.util.Random`; it reads the tuples out of the `[tuples]`
// block. Floating point crosses the boundary as `Float.toString`/`Double.toString` text and the
// Rust side **parses** it rather than formatting its own: both languages' printers emit the
// shortest round-tripping form and both parsers are correctly rounded, so the comparison is
// exact and the test needs no `java_float_to_string`.
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p7t9 java/probes/P7T9Probe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p7t9:$JAR" \
//       app.freerouting.autoroute.P7T9Probe \
//     > ../../crates/fr-router/tests/data/p7t9-item-route-result.txt
public final class P7T9Probe {

  private P7T9Probe() {}

  /** The trace-length table. `0.0` is in it because `:61` guards on `traceLengthBefore != 0`. */
  private static final double[] LENGTHS = {
    0.0, 1.0, 2.0, 3.5, 10.0, 100.0, 1234.5678, 0.125, 7.0, 999999.5
  };

  public static void main(String[] args) {
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    out.println("# P7T9Probe — plan 7 task 9 ground truth, HEAD jar");
    out.println("# autoroute/ItemRouteResult.java:1-145, the whole class");
    out.println("# no clock, no board: this transcript is byte-stable across runs and machines");

    // The one-argument constructor, `:17-20`: `this(itemId, 0, 0, 0, 0, 0, 1)` and then an
    // explicit `improved = false`. `incompleteCountAfter = 1 > incompleteCountBefore = 0` already
    // makes the ladder answer `false`, so the explicit assignment is redundant — the probe prints
    // the observable state either way.
    out.println("[unimproved-ctor]");
    ItemRouteResult bare = new ItemRouteResult(4919);
    out.println(
        "itemId="
            + bare.itemId()
            + " improved="
            + bare.improved()
            + " improvementPercentage="
            + Float.toString(bare.improvementPercentage())
            + " viaCount="
            + bare.viaCount()
            + " traceLength="
            + Double.toString(bare.traceLength())
            + " incompleteCount="
            + bare.incompleteCount()
            + " incompleteCountBefore="
            + bare.incompleteCountBefore()
            + " viaCountReduced="
            + bare.viaCountReduced()
            + " lengthReduced="
            + Double.toString(bare.lengthReduced()));

    // `updateImproved(boolean)` (`:137-139`) — the only mutator on the class.
    out.println("[update-improved]");
    ItemRouteResult mutable = new ItemRouteResult(1);
    out.println("initial=" + mutable.improved());
    mutable.updateImproved(true);
    out.println("afterTrue=" + mutable.improved());
    mutable.updateImproved(false);
    out.println("afterFalse=" + mutable.improved());

    Random random = new Random(4919);
    List<ItemRouteResult> results = new ArrayList<>();

    out.println("[tuples]");
    out.println(
        "# k itemId vcBefore vcAfter tlBefore tlAfter icBefore icAfter"
            + " -> improved improvementPercentage viaCountReduced lengthReduced");
    for (int k = 0; k < 500; k++) {
      int itemId = 1 + random.nextInt(400);
      int viaCountBefore = random.nextInt(6);
      int viaCountAfter = random.nextInt(6);
      double traceLengthBefore = LENGTHS[random.nextInt(LENGTHS.length)];
      double traceLengthAfter = LENGTHS[random.nextInt(LENGTHS.length)];
      int incompleteCountBefore = random.nextInt(4);
      int incompleteCountAfter = random.nextInt(4);

      ItemRouteResult r =
          new ItemRouteResult(
              itemId,
              viaCountBefore,
              viaCountAfter,
              traceLengthBefore,
              traceLengthAfter,
              incompleteCountBefore,
              incompleteCountAfter);
      results.add(r);

      out.println(
          k
              + " "
              + itemId
              + " "
              + viaCountBefore
              + " "
              + viaCountAfter
              + " "
              + Double.toString(traceLengthBefore)
              + " "
              + Double.toString(traceLengthAfter)
              + " "
              + incompleteCountBefore
              + " "
              + incompleteCountAfter
              + " -> "
              + r.improved()
              + " "
              + Float.toString(r.improvementPercentage())
              + " "
              + r.viaCountReduced()
              + " "
              + Double.toString(r.lengthReduced()));
    }

    // `improvedOver` (`:92-94`) is `compareTo(...) < 0`; printed for consecutive pairs, both ways
    // round, plus the sign of `compareTo` itself, so the relation is pinned as well as the total
    // order it is derived from.
    out.println("[improved-over]");
    for (int k = 0; k + 1 < results.size(); k++) {
      out.println(
          k
              + " "
              + (k + 1)
              + " "
              + results.get(k).improvedOver(results.get(k + 1))
              + " "
              + results.get(k + 1).improvedOver(results.get(k))
              + " "
              + Integer.signum(results.get(k).compareTo(results.get(k + 1))));
    }

    // `List.sort` is **stable** by contract, so ties keep their input order and the printed key
    // sequence is a total function of `compareTo`. The Rust twin sorts the same tuples with
    // `slice::sort_by`, which is stable too.
    out.println("[sorted]");
    List<Integer> order = new ArrayList<>();
    for (int k = 0; k < results.size(); k++) {
      order.add(k);
    }
    order.sort((a, b) -> results.get(a).compareTo(results.get(b)));
    StringBuilder sb = new StringBuilder();
    for (int i = 0; i < order.size(); i++) {
      if (i > 0) {
        sb.append(' ');
      }
      sb.append(order.get(i));
    }
    out.println(sb);
  }
}
