// Plan 6 Task 8 ground-truth probe: `DestinationDistance`, `AutorouteControl` and the ruling-H
// via-info re-pointing mechanism, on the clone's HEAD jar. It is not a differential driver —
// there is no Rust twin and `run.sh` does not know it — but every literal in
// `crates/fr-router/tests/{destination_distance,control}.rs` is read off its stdout, so it is
// committed here to keep those numbers reproducible.
//
// It declares `package app.freerouting.autoroute.maze` so it can read `DestinationDistance`'s
// nine package-private cost fields, which the constructor derives and no public method exposes.
// It compiles against the clone's HEAD jar exactly as `run.sh`'s jar mode does:
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential/java/probes
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p6t8 P6T8Probe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p6t8:$JAR" \
//       app.freerouting.autoroute.maze.P6T8Probe <mode> [args]
//
// Modes:
//   dd                     `DestinationDistance` over six configurations x a point/layer grid.
//   ctrl <dsn> [netNo]     `AutorouteControl` over a real board, every field printed.
//   nan                    what `Math.min`/`Math.max` do to a NaN trace cost on the way through
//                          `DestinationDistance` — Rust's `f64::min`/`f64::max` absorb it,
//                          `Math.min`/`Math.max` propagate it.
//   viadiv <dsn> <rules>   the ruling-H mechanism: what a `ViaRule` reaches after
//                          `RulesReader.applyViaInfo` has replaced the via info it names.
//
// The exact invocations behind the three committed outputs:
//
//   F=../../../../../freerouting/fixtures; D=../../../../crates/fr-router/tests/data
//   java ... P6T8Probe dd                                   > $D/p6t8-destination-distance.txt
//   java ... P6T8Probe ctrl $F/Issue593-BBD_Mars-64.dsn      > (the Issue593 half of
//   java ... P6T8Probe ctrl $F/Issue508-DAC2020_bm01.dsn        $D/p6t8-autoroute-control.txt)
//   java ... P6T8Probe viadiv $F/Issue593-BBD_Mars-64.dsn \
//       $D/ruling-h-redeclare.rules                          > $D/p6t8-ruling-h-viadiv.txt
//
// Each committed file carries a `#` header with the same commands; the `ctrl` file is the two
// runs concatenated under `######## <stem>` separators, and `INFO`/`WARN` log lines are filtered
// out with `grep -v "^20[0-9][0-9]-"`.
//
// The routing half of the ruling-H probe is not this program: it is the HEAD jar itself,
//
//   java -Djava.awt.headless=true -jar build/libs/freerouting-current-executable.jar \
//       -de fixtures/Issue593-BBD_Mars-64.dsn -do /tmp/h-without.ses -mp 1 -mt 1 -oit 0
//   java ... -de fixtures/Issue593-BBD_Mars-64.dsn \
//       -dr <freerouting-rs>/crates/fr-router/tests/data/ruling-h-redeclare.rules \
//       -do /tmp/h-with.ses -mp 1 -mt 1 -oit 0
//
// which emits 123 `(via ...)` without the rules file and 45 with it (2192 diff lines between the
// two .ses files). See `docs/java-quirks.md`'s re-pointing row.
package app.freerouting.autoroute.maze;

import app.freerouting.autoroute.maze.AutorouteControl.ExpansionCostFactor;
import app.freerouting.board.actions.ItemIdGenerator;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.core.RoutingJob;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.io.specctra.RulesReader;
import app.freerouting.management.HeadlessBoardManager;
import app.freerouting.rules.ViaInfo;
import app.freerouting.rules.ViaRule;
import app.freerouting.settings.RouterSettings;
import java.io.File;
import java.io.FileInputStream;
import java.util.Collection;
import java.util.UUID;

/** Task 8 ground truth. */
public class P6T8Probe {

  public static void main(String[] args) throws Exception {
    String mode = args.length > 0 ? args[0] : "dd";
    switch (mode) {
      case "dd" -> destinationDistance();
      case "ctrl" -> control(args[1], args.length > 2 ? Integer.parseInt(args[2]) : -1);
      case "nan" -> nanPropagation();
      case "viadiv" -> viaDivergence(args[1], args[2]);
      default -> throw new IllegalArgumentException("mode " + mode);
    }
  }

  // =============================================================================================
  // DestinationDistance
  // =============================================================================================

  static String f(double v) {
    if (v == Integer.MAX_VALUE) {
      return "INT_MAX";
    }
    return String.format(java.util.Locale.ROOT, "%.6f", v);
  }

  /** The nine constructor-derived cost fields, in declaration order. */
  static void dumpCosts(DestinationDistance d) {
    System.out.println(
        "  costs minComp="
            + f(d.minComponentSideTraceCost)
            + " maxComp="
            + f(d.maxComponentSideTraceCost)
            + " minSold="
            + f(d.minSolderSideTraceCost)
            + " maxSold="
            + f(d.maxSolderSideTraceCost)
            + " maxInner="
            + f(d.maxInnerSideTraceCost)
            + " minCompInner="
            + f(d.minComponentInnerTraceCost)
            + " minSoldInner="
            + f(d.minSolderInnerTraceCost)
            + " minCompSoldInner="
            + f(d.minComponentSolderInnerTraceCost));
  }

  static final ExpansionCostFactor[] COSTS_4 = {
    new ExpansionCostFactor(1.0, 2.0),
    new ExpansionCostFactor(0.5, 1.2),
    new ExpansionCostFactor(2.5, 1.5),
    new ExpansionCostFactor(3.0, 5.0),
  };

  /** The probe grid: (x, y) points fed through `calculate(FloatPoint, layer)`. */
  static final double[][] POINTS = {
    {50.0, 50.0}, {-400.0, 250.5}, {1200.0, 1200.0}, {250.0, 550.0}, {0.0, 0.0}, {-1000.0, -1000.0}
  };

  static void grid(String tag, DestinationDistance d, int layerCount) {
    dumpCosts(d);
    for (int layer = 0; layer < layerCount; layer++) {
      for (double[] p : POINTS) {
        FloatPoint fp = new FloatPoint(p[0], p[1]);
        System.out.println(
            "  "
                + tag
                + " point("
                + f(p[0])
                + ","
                + f(p[1])
                + ") layer="
                + layer
                + " => "
                + f(d.calculate(fp, layer)));
      }
    }
    // The IntBox overload, which is the one `calculateCheapDistance` takes.
    IntBox[] boxes = {
      new IntBox(0, 0, 100, 100), new IntBox(-500, -500, -400, -400), new IntBox(700, 200, 900, 400)
    };
    for (int layer = 0; layer < layerCount; layer++) {
      for (IntBox b : boxes) {
        System.out.println(
            "  "
                + tag
                + " box["
                + b.ll.x
                + ","
                + b.ll.y
                + ".."
                + b.ur.x
                + ","
                + b.ur.y
                + "] layer="
                + layer
                + " => "
                + f(d.calculate(b, layer))
                + " cheap="
                + f(d.calculateCheapDistance(b, layer))
                + " again="
                + f(d.calculate(b, layer)));
      }
    }
  }

  static void destinationDistance() {
    // (1) four active layers, three joined boxes.
    System.out.println("case allActive4");
    DestinationDistance d =
        new DestinationDistance(COSTS_4, new boolean[] {true, true, true, true}, 50.0, 40.0);
    d.join(new IntBox(0, 0, 100, 100), 0);
    d.join(new IntBox(500, 500, 600, 600), 3);
    d.join(new IntBox(200, 200, 300, 300), 1);
    grid("allActive4", d, 4);

    // (2) nothing joined at all — the `boxIsEmpty` short circuit.
    System.out.println("case empty4");
    DestinationDistance e =
        new DestinationDistance(COSTS_4, new boolean[] {true, true, true, true}, 50.0, 40.0);
    grid("empty4", e, 4);

    // (3) three active layers (the inner layer 2 is off).
    System.out.println("case active3");
    DestinationDistance a3 =
        new DestinationDistance(COSTS_4, new boolean[] {true, true, false, true}, 50.0, 40.0);
    a3.join(new IntBox(0, 0, 100, 100), 0);
    a3.join(new IntBox(500, 500, 600, 600), 3);
    a3.join(new IntBox(200, 200, 300, 300), 1);
    grid("active3", a3, 4);

    // (4) two active layers (both inner layers off).
    System.out.println("case active2");
    DestinationDistance a2 =
        new DestinationDistance(COSTS_4, new boolean[] {true, false, false, true}, 50.0, 40.0);
    a2.join(new IntBox(0, 0, 100, 100), 0);
    a2.join(new IntBox(500, 500, 600, 600), 3);
    grid("active2", a2, 4);

    // (5) one active layer — the `activeLayerCount <= 1` return, and the layer-0-inactive arm
    // that leaves minComponentSideTraceCost at its 0.0 default.
    System.out.println("case active1solder");
    DestinationDistance a1 =
        new DestinationDistance(COSTS_4, new boolean[] {false, false, false, true}, 50.0, 40.0);
    a1.join(new IntBox(500, 500, 600, 600), 3);
    grid("active1solder", a1, 4);

    // (5b) only the component-side box is joined, so `solderSideBoxIsEmpty` and
    // `innerSideBoxIsEmpty` stay true and the layer-1..3 arms fall through to the two/three/four
    // layer estimates against boxes that were never joined (IntBox.EMPTY).
    System.out.println("case compOnly4");
    DestinationDistance c =
        new DestinationDistance(COSTS_4, new boolean[] {true, true, true, true}, 50.0, 40.0);
    c.join(new IntBox(0, 0, 100, 100), 0);
    grid("compOnly4", c, 4);

    // (6) a two-layer board: layer 1 is *both* `layerCount - 1` and the solder side, so the inner
    // arm of `calculate` is unreachable and the `join` else-branch never fires.
    System.out.println("case twoLayer");
    ExpansionCostFactor[] costs2 = {
      new ExpansionCostFactor(1.0, 4.0), new ExpansionCostFactor(3.0, 1.0)
    };
    DestinationDistance t =
        new DestinationDistance(costs2, new boolean[] {true, true}, 25.0, 20.0);
    t.join(new IntBox(0, 0, 100, 100), 0);
    t.join(new IntBox(500, 500, 600, 600), 1);
    grid("twoLayer", t, 2);
  }

  /**
   * A NaN horizontal trace cost on layer 0. Java's `Math.min`/`Math.max` propagate the NaN
   * (`if (a != a) return a;`), so it reaches `calculate`'s answer; Rust's `f64::min`/`f64::max`
   * absorb it, which would make quirk #170 unreachable from this direction.
   */
  static void nanPropagation() {
    ExpansionCostFactor[] costs = {
      new ExpansionCostFactor(Double.NaN, 2.0),
      new ExpansionCostFactor(0.5, 1.2),
      new ExpansionCostFactor(2.5, 1.5),
      new ExpansionCostFactor(3.0, 5.0),
    };
    DestinationDistance d =
        new DestinationDistance(costs, new boolean[] {true, true, true, true}, 50.0, 40.0);
    d.join(new IntBox(0, 0, 100, 100), 0);
    d.join(new IntBox(500, 500, 600, 600), 3);
    d.join(new IntBox(200, 200, 300, 300), 1);
    System.out.println("case nanComponentHorizontal");
    dumpCosts(d);
    IntBox[] boxes = {new IntBox(0, 0, 100, 100), new IntBox(700, 200, 900, 400)};
    for (int layer = 0; layer < 4; layer++) {
      for (IntBox b : boxes) {
        double v = d.calculate(b, layer);
        System.out.println(
            "  nan box["
                + b.ll.x
                + ","
                + b.ll.y
                + ".."
                + b.ur.x
                + ","
                + b.ur.y
                + "] layer="
                + layer
                + " => "
                + (Double.isNaN(v) ? "NaN" : f(v))
                + " isNaN="
                + Double.isNaN(v));
      }
    }
    // And what the two languages' library calls do, in isolation.
    System.out.println("  mathMin(NaN, 1.0)=" + Math.min(Double.NaN, 1.0));
    System.out.println("  mathMin(1.0, NaN)=" + Math.min(1.0, Double.NaN));
    System.out.println("  mathMax(NaN, 1.0)=" + Math.max(Double.NaN, 1.0));
    System.out.println("  mathMax(1.0, NaN)=" + Math.max(1.0, Double.NaN));
  }

  // =============================================================================================
  // AutorouteControl
  // =============================================================================================

  static RoutingBoard loadBoard(String dsn) throws Exception {
    RoutingJob job = new RoutingJob(UUID.randomUUID());
    job.setInput(new File(dsn));
    HeadlessBoardManager manager = new HeadlessBoardManager(job);
    manager.loadFromSpecctraDsn(job.input.getData(), null, new ItemIdGenerator());
    return manager.getRoutingBoard();
  }

  static boolean isPureSmd(RoutingBoard board, int netNo) {
    Collection<Item> items = board.getConnectableItems(netNo);
    if (items.isEmpty()) {
      return false;
    }
    for (Item item : items) {
      if (!(item instanceof Pin pin) || pin.firstLayer() != pin.lastLayer()) {
        return false;
      }
    }
    return true;
  }

  static void dumpControl(String tag, RoutingBoard board, int netNo, RouterSettings settings) {
    AutorouteControl ctrl =
        new AutorouteControl(board, netNo, settings, settings.getViaCosts(), settings.getTraceCosts());
    StringBuilder sb = new StringBuilder();
    sb.append(tag).append(" net=").append(netNo);
    sb.append(" pureSmd=").append(isPureSmd(board, netNo));
    sb.append(" layerCount=").append(ctrl.layerCount);
    sb.append(" layerActive=");
    for (boolean b : ctrl.layerActive) {
      sb.append(b ? '1' : '0');
    }
    sb.append(" traceHalfWidth=");
    for (int v : ctrl.traceHalfWidth) {
      sb.append(v).append(',');
    }
    sb.append(" compensatedTraceHalfWidth=");
    for (int v : ctrl.compensatedTraceHalfWidth) {
      sb.append(v).append(',');
    }
    sb.append(" viaRadii=");
    for (double v : ctrl.viaRadii) {
      sb.append(f(v)).append(',');
    }
    sb.append(" bendCosts=");
    for (double v : ctrl.bendCosts) {
      sb.append(f(v)).append(',');
    }
    sb.append(" traceClearanceClassIndex=").append(ctrl.traceClearanceClassIndex);
    sb.append(" viaClearanceClass=").append(ctrl.viaClearanceClass);
    sb.append(" viaRule=").append(ctrl.viaRule == null ? "null" : ctrl.viaRule.name);
    sb.append(" viaInfos=");
    for (AutorouteControl.ViaMask m : ctrl.viaInfos) {
      sb.append('[')
          .append(m.fromLayer)
          .append("..")
          .append(m.toLayer)
          .append(" attach=")
          .append(m.attachSmdAllowed)
          .append(']');
    }
    sb.append(" attachSmdAllowed=").append(ctrl.attachSmdAllowed);
    sb.append(" maxViaRadius=").append(f(ctrl.maxViaRadius));
    sb.append(" minNormalViaCost=").append(f(ctrl.minNormalViaCost));
    sb.append(" minCheapViaCost=").append(f(ctrl.minCheapViaCost));
    sb.append(" viasAllowed=").append(ctrl.viasAllowed);
    sb.append(" withNeckdown=").append(ctrl.withNeckdown);
    sb.append(" viaLowerBound=").append(ctrl.viaLowerBound);
    sb.append(" viaUpperBound=").append(ctrl.viaUpperBound);
    sb.append(" tidyRegionWidth=").append(ctrl.tidyRegionWidth);
    sb.append(" pullTightAccuracy=").append(ctrl.pullTightAccuracy);
    sb.append(" maxShoveTraceRecursionDepth=").append(ctrl.maxShoveTraceRecursionDepth);
    sb.append(" maxShoveViaRecursionDepth=").append(ctrl.maxShoveViaRecursionDepth);
    sb.append(" maxSpringOverRecursionDepth=").append(ctrl.maxSpringOverRecursionDepth);
    sb.append(" ripupAllowed=").append(ctrl.ripupAllowed);
    sb.append(" ripupCosts=").append(ctrl.ripupCosts);
    sb.append(" ripupPassNo=").append(ctrl.ripupPassNo);
    sb.append(" isFanout=").append(ctrl.isFanout);
    sb.append(" fanoutStartPinLayer=").append(ctrl.fanoutStartPinLayer);
    sb.append(" removeUnconnectedVias=").append(ctrl.removeUnconnectedVias);
    sb.append(" addViaCostsAllZero=").append(allZero(ctrl));
    System.out.println(sb);
  }

  static boolean allZero(AutorouteControl ctrl) {
    for (int i = 0; i < ctrl.layerCount; i++) {
      for (int j = 0; j < ctrl.layerCount; j++) {
        if (ctrl.addViaCosts[i].toLayer[j] != 0) {
          return false;
        }
      }
    }
    return true;
  }

  static void control(String dsn, int netNo) throws Exception {
    RoutingBoard board = loadBoard(dsn);
    RouterSettings settings = new RouterSettings(board);
    System.out.println(
        "board layerCount="
            + board.getLayerCount()
            + " maxNetNo="
            + board.rules.nets.maxNetNumber()
            + " viaRules="
            + board.rules.viaRules.size()
            + " viaInfos="
            + board.rules.viaInfos.count());
    for (int i = 0; i < board.getLayerCount(); i++) {
      System.out.println(
          "layer "
              + i
              + " name="
              + board.layerStructure.layers[i].name
              + " isSignal="
              + board.layerStructure.layers[i].isSignal
              + " settingActive="
              + settings.getLayerActive(i));
    }
    System.out.println(
        "settings viaCosts=" + settings.getViaCosts() + " viasAllowed=" + settings.getViasAllowed());
    if (netNo >= 0) {
      dumpControl("ctrl", board, netNo, settings);
      return;
    }
    // Scan for the first pure-SMD net and the first mixed one, and dump both.
    int pure = -1;
    int mixed = -1;
    for (int n = 1; n <= board.rules.nets.maxNetNumber(); n++) {
      if (board.getConnectableItems(n).isEmpty()) {
        continue;
      }
      if (isPureSmd(board, n)) {
        if (pure < 0) {
          pure = n;
        }
      } else if (mixed < 0) {
        mixed = n;
      }
    }
    System.out.println("firstPureSmdNet=" + pure + " firstMixedNet=" + mixed);
    if (pure > 0) {
      dumpControl("ctrl", board, pure, settings);
    }
    if (mixed > 0) {
      dumpControl("ctrl", board, mixed, settings);
    }
    // net 0 (the fall back to net 1's half widths) and a net number the board does not have
    // (the null-net arm: clearance class 1 and viaRules.firstElement()).
    dumpControl("ctrl", board, 0, settings);
    // A *positive* net number the board does not have: `initNet`'s null-net arm (`:212-216`)
    // runs, and then `:219`'s `board.rules.getTraceHalfWidth(netNumber, i)` dereferences the same
    // null. The arm is only observable for netNumber <= 0.
    try {
      dumpControl("ctrl", board, board.rules.nets.maxNetNumber() + 1000, settings);
    } catch (RuntimeException e) {
      System.out.println("ctrl net=" + (board.rules.nets.maxNetNumber() + 1000) + " threw " + e);
    }
  }

  // =============================================================================================
  // ruling H: what a via rule reaches after its via info has been re-declared
  // =============================================================================================

  static void viaDivergence(String dsn, String rules) throws Exception {
    RoutingBoard board = loadBoard(dsn);
    report("before", board);
    try (FileInputStream in = new FileInputStream(rules)) {
      boolean ok = RulesReader.read(in, "x", board);
      System.out.println("rulesRead " + ok);
    }
    report("after", board);
  }

  static void report(String tag, RoutingBoard board) {
    for (int i = 0; i < board.rules.viaInfos.count(); i++) {
      ViaInfo v = board.rules.viaInfos.get(i);
      System.out.println(
          tag
              + " viainfo "
              + i
              + " "
              + v.getName()
              + " attach="
              + v.attachSmdAllowed()
              + " cl="
              + v.getClearanceClassIndex()
              + " id="
              + System.identityHashCode(v));
    }
    for (ViaRule r : board.rules.viaRules) {
      for (int i = 0; i < r.viaCount(); i++) {
        ViaInfo v = r.getVia(i);
        System.out.println(
            tag
                + " rulevia "
                + r.name
                + "["
                + i
                + "] "
                + v.getName()
                + " attach="
                + v.attachSmdAllowed()
                + " cl="
                + v.getClearanceClassIndex()
                + " inList="
                + (board.rules.viaInfos.get(v.getName()) == v)
                + " id="
                + System.identityHashCode(v));
      }
    }
  }
}
