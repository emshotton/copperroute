package app.freerouting.board.trace;

import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.model.structure.Layer;
import app.freerouting.board.model.structure.LayerStructure;
import app.freerouting.core.library.Package;
import app.freerouting.core.library.Padstack;
import app.freerouting.geometry.planar.ConvexShape;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.IntVector;
import app.freerouting.geometry.planar.Line;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import app.freerouting.geometry.planar.PolylineShape;
import app.freerouting.geometry.planar.Shape;
import app.freerouting.board.state.Communication;
import app.freerouting.rules.BoardRules;
import app.freerouting.rules.ClearanceMatrix;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.util.Random;

/**
 * Plan 7 Task 5 differential driver: the {@code ConnectionToPin} trio of {@code
 * board/trace/PolylineTrace.java} — {@code checkConnectionToPin(boolean)} ({@code :1013-1076}),
 * {@code correctConnectionToPin(boolean, AngleRestriction)} ({@code :1082-1245}) and {@code
 * swapConnectionToPin(boolean)} ({@code :1252-1313}) — plus the four call sites of {@code
 * pullTight:841-861} that drive them.
 *
 * <p>Usage: {@code P7T6 <mode>}. Modes:
 *
 * <ul>
 *   <li>{@code check} — {@code checkConnectionToPin} over the directed table, at both ends, for
 *       every {@code pinEdgeToTurnDist} x angle regime. This is the <b>regression oracle</b>: the
 *       port's own {@code check_connection_to_pin} is new code in Plan 7 Task 5 (the plan's scan
 *       ruling 5 recorded it as landed in Plan 6; it had not), so its first evidence is this mode.
 *   <li>{@code correct} — {@code correctConnectionToPin} over the same table, printing the return
 *       value and the whole board afterwards (item ids included, so the {@code SHOVE_FIXED} exit
 *       stub {@code :1239-1244} inserts is visible and its id is pinned).
 *   <li>{@code swap} — {@code swapConnectionToPin} over the swap table: a {@code SHOVE_FIXED}
 *       stub out of the pin plus a main trace meeting it at a sharp angle, which is the only
 *       shape that reaches {@code :1312-1313}. The reordering the method performs — the contact
 *       trace losing its fixed state and then being swallowed by {@code combine()} — is pinned by
 *       the board dump.
 *   <li>{@code rand} — 128 random traces out of the pin, {@code java.util.Random(70605)}, through
 *       all three methods on a fresh board each, at every regime.
 *   <li>{@code edge} — the two skips of {@code pullTight:841-842}: the 90-degree regime and a
 *       non-positive {@code pinEdgeToTurnDist}, driven through {@code pullTight} itself so the
 *       branch and not just the methods is compared.
 * </ul>
 *
 * <p>Every polyline is printed as its line array plus its corner list, the {@code P6T15aProbe}
 * format: an {@code IntPoint} corner as {@code (x,y)} and a rational one as {@code
 * ~(<Double.toString x>,<Double.toString y>)} of {@code cornerApprox}.
 *
 * <p><b>The budget is not involved.</b> None of the three methods reads a {@code Stoppable} or a
 * {@code TimeLimit}; {@code TraceTightener.TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP} is {@code p7t3}'s
 * concern, not this driver's. The {@code edge} mode builds its tightener with {@code timeLimit =
 * -1}, which is Java's own "no limit" ({@code TraceTightener.java:73-77}).
 */
public final class P7T6 {

  private P7T6() {}

  static final IntBox BOUNDING_BOX = new IntBox(new IntPoint(-10_000, -10_000), new IntPoint(10_000, 10_000));

  static RoutingBoard board;
  static Padstack longPad;

  /**
   * The driver's own stdout. `FRLogger` writes its `WARN` lines to `System.out` — and the `rand`
   * mode's degenerate polylines produce them — so the `P6T1`/`P5T1` guard applies here too: the
   * driver prints to a private stream on the real file descriptor and `System.out` goes into the
   * void.
   */
  static PrintStream out;

  public static void main(String[] args) throws Exception {
    out = new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(java.io.OutputStream.nullOutputStream()));
    String mode = args.length > 0 ? args[0] : "check";
    switch (mode) {
      case "check" -> checkMode();
      case "correct" -> correctMode();
      case "swap" -> swapMode();
      case "rand" -> randMode();
      case "edge" -> edgeMode();
      default -> {
        System.err.println("usage: P7T6 <check|correct|swap|rand|edge>");
        System.exit(2);
      }
    }
  }

  // --- the board ---------------------------------------------------------------------------------

  /**
   * A four-pin component with a 400 x 100 SMD pad per pin, so {@code Pin.getTraceExitRestrictions}
   * keeps {@code padXyFactor} at 1.5 ({@code Pin.java:274-276} doubles it only for a package of
   * three pins or fewer) and {@code Padstack.getTraceExitDirections:182-193} answers the long
   * side's two directions — RIGHT and LEFT, each with a minimal length of 200 — rather than all
   * four. That non-empty, *restrictive* set is what {@code checkConnectionToPin:1038-1040} needs;
   * {@code P6T9Probe}'s square 100 x 100 pad on a two-pin package answers all four directions and
   * therefore never refuses anything, which is why the `p6t15a` `pinedge` transcript shows the
   * whole branch answering `false`.
   */
  static void build(AngleRestriction angleRestriction, double pinEdgeToTurnDist) {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    cm.appendClass("wide");
    cm.setValue(2, 1, 600);
    cm.setValue(2, 2, 800);
    BoardRules rules = new BoardRules(ls, cm);
    rules.setTraceAngleRestriction(angleRestriction);
    rules.setPinEdgeToTurnDist(pinEdgeToTurnDist);
    Communication comm = new Communication();
    board = new RoutingBoard(BOUNDING_BOX, ls, new PolylineShape[0], 0, rules, comm);
    board.library.padstacks = new app.freerouting.core.library.Padstacks(ls);
    board.library.packages = new app.freerouting.core.library.Packages(board.library.padstacks);

    ConvexShape[] longShapes = {new IntBox(-200, -50, 200, 50), null};
    longPad = board.library.padstacks.add("long", longShapes, false, false);

    Package.Pin[] pins = {
      new Package.Pin("P1", longPad.id, new IntVector(-500, 0), 0),
      new Package.Pin("P2", longPad.id, new IntVector(500, 0), 0),
      new Package.Pin("P3", longPad.id, new IntVector(-500, 2000), 0),
      new Package.Pin("P4", longPad.id, new IntVector(500, 2000), 0)
    };
    Package pkg =
        board.library.packages.add(
            "pkg",
            pins,
            new Shape[0],
            new double[0],
            new boolean[0],
            new Package.Keepout[0],
            new Package.Keepout[0],
            new Package.Keepout[0],
            true);
    board.components.add(new IntPoint(0, 0), 0, true, pkg);

    board.rules.nets.add("N1", 1, false);
    board.rules.nets.add("N2", 1, false);

    board.insertPin(1, 0, new int[] {1}, 1, FixedState.UNFIXED);
    board.insertPin(1, 1, new int[] {1}, 1, FixedState.UNFIXED);
  }

  // --- dumps -------------------------------------------------------------------------------------

  static String ln(Line l) {
    return "(" + ((IntPoint) l.a).x + "," + ((IntPoint) l.a).y + ")->(" + ((IntPoint) l.b).x + "," + ((IntPoint) l.b).y + ")";
  }

  static String pt(Polyline p, int no) {
    Point corner = p.corner(no);
    if (corner instanceof IntPoint ip) {
      return "(" + ip.x + "," + ip.y + ")";
    }
    FloatPoint f = p.cornerApprox(no);
    return "~(" + Double.toString(f.x) + "," + Double.toString(f.y) + ")";
  }

  static String poly(Polyline p) {
    if (p == null) {
      return "null";
    }
    StringBuilder sb = new StringBuilder();
    sb.append("n=").append(p.lines.length).append(" lines=[");
    for (int i = 0; i < p.lines.length; i++) {
      if (i > 0) {
        sb.append(",");
      }
      sb.append(ln(p.lines[i]));
    }
    sb.append("] corners=[");
    for (int i = 0; i < p.cornerCount(); i++) {
      if (i > 0) {
        sb.append(",");
      }
      sb.append(pt(p, i));
    }
    return sb.append("]").toString();
  }

  static String pointOf(Point p) {
    FloatPoint f = p.toFloat().round().toFloat();
    return "(" + (int) f.x + "," + (int) f.y + ")";
  }

  static String nets(int[] netNos) {
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < netNos.length; i++) {
      if (i > 0) {
        sb.append(",");
      }
      sb.append(netNos[i]);
    }
    return sb.append("]").toString();
  }

  /** One line per item, in `getItems()` order (descending id, quirk #63). */
  static void dumpBoard() {
    out.println("    maxId=" + board.communication.idGenerator.maxGeneratedId());
    for (Item item : board.getItems()) {
      StringBuilder sb = new StringBuilder();
      sb.append("    item id=")
          .append(item.getId())
          .append(" type=")
          .append(item.getClass().getSimpleName())
          .append(" nets=")
          .append(nets(item.netNumbers))
          .append(" cl=")
          .append(item.clearanceClassIndex())
          .append(" fix=")
          .append(item.getFixedState());
      if (item instanceof PolylineTrace trace) {
        sb.append(" layer=")
            .append(trace.getLayer())
            .append(" hw=")
            .append(trace.getHalfWidth())
            .append(" ")
            .append(poly(trace.polyline()));
      } else if (item instanceof Pin pin) {
        sb.append(" center=").append(pointOf(pin.getCenter()));
      }
      out.println(sb);
    }
  }

  // --- the tables --------------------------------------------------------------------------------

  static IntPoint p(int x, int y) {
    return new IntPoint(x, y);
  }

  record Case(String name, Point[] corners, int halfWidth, int cl) {}

  static final AngleRestriction[] REGIMES = {
    AngleRestriction.NINETY_DEGREE, AngleRestriction.FORTYFIVE_DEGREE, AngleRestriction.NONE
  };

  static final double[] EDGE_DISTS = {-1.0, 0.0, 100.0, 500.0};

  /**
   * Ten hand-placed traces, all of them touching pin P1's centre (-500, 0) at one end and pin P2's
   * centre (500, 0) at the other where the name says so, so that both {@code atStart} and {@code
   * atEnd} have a pin to check against.
   *
   * <p>The pad's exit restrictions are RIGHT and LEFT with {@code minLength = 200}, the clearance
   * of class 1 against class 1 is 200 and the trace half width is 30, so {@code
   * checkConnectionToPin:1074-1076} preserves {@code 200 + 30 + max(edgeToTurnDist, 201)} — 431 at
   * {@code edgeToTurnDist <= 201} and 730 at 500. The first segment lengths below (300, 600, 1000)
   * therefore straddle both thresholds, and the {@code up*} rows leave the pad in a direction no
   * restriction names, which refuses at {@code :1063-1065} whatever the length.
   */
  static Case[] table() {
    return new Case[] {
      new Case("short-right", new Point[] {p(-500, 0), p(-200, 0), p(-200, 800)}, 30, 1),
      new Case("mid-right", new Point[] {p(-500, 0), p(100, 0), p(100, 800)}, 30, 1),
      new Case("long-right", new Point[] {p(-500, 0), p(500, 0), p(500, 800)}, 30, 1),
      new Case("up-out", new Point[] {p(-500, 0), p(-500, 800), p(300, 800)}, 30, 1),
      new Case("diag-out", new Point[] {p(-500, 0), p(-100, 400), p(600, 400)}, 30, 1),
      new Case("pin-to-pin-short", new Point[] {p(-500, 0), p(-200, 300), p(200, 300), p(500, 0)}, 30, 1),
      new Case("pin-to-pin-long", new Point[] {p(-500, 0), p(-500, 900), p(500, 900), p(500, 0)}, 30, 1),
      new Case("wide-class", new Point[] {p(-500, 0), p(-200, 0), p(-200, 800)}, 30, 2),
      new Case("fat", new Point[] {p(-500, 0), p(100, 0), p(100, 800)}, 90, 1),
      new Case("two-corner", new Point[] {p(-500, 0), p(0, 0)}, 30, 1),
    };
  }

  static PolylineTrace insert(Case c) {
    Polyline polyline = new Polyline(c.corners());
    board.insertTraceWithoutCleaning(polyline, 0, c.halfWidth(), new int[] {1}, c.cl(), FixedState.UNFIXED);
    return lastTrace();
  }

  /** The most recently inserted trace — `getItems()` is descending id, so it is the first one. */
  static PolylineTrace lastTrace() {
    for (Item item : board.getItems()) {
      if (item instanceof PolylineTrace trace) {
        return trace;
      }
    }
    return null;
  }

  // --- mode `check` ------------------------------------------------------------------------------

  static void checkMode() {
    for (AngleRestriction ar : REGIMES) {
      for (double edge : EDGE_DISTS) {
        for (Case c : table()) {
          build(ar, edge);
          PolylineTrace trace = insert(c);
          out.println(
              "regime="
                  + ar
                  + " edge="
                  + Double.toString(edge)
                  + " case="
                  + c.name()
                  + " atStart="
                  + trace.checkConnectionToPin(true)
                  + " atEnd="
                  + trace.checkConnectionToPin(false));
        }
      }
    }
  }

  // --- mode `correct` ----------------------------------------------------------------------------

  static void correctMode() {
    for (AngleRestriction ar : REGIMES) {
      for (double edge : EDGE_DISTS) {
        for (Case c : table()) {
          for (boolean atStart : new boolean[] {true, false}) {
            build(ar, edge);
            PolylineTrace trace = insert(c);
            boolean changed = trace.correctConnectionToPin(atStart, ar);
            out.println(
                "regime="
                    + ar
                    + " edge="
                    + Double.toString(edge)
                    + " case="
                    + c.name()
                    + " atStart="
                    + atStart
                    + " changed="
                    + changed);
            dumpBoard();
          }
        }
      }
    }
  }

  // --- mode `swap` -------------------------------------------------------------------------------

  /**
   * The swap fixture: a {@code SHOVE_FIXED} stub leaving pin P1 at (-500, 0) into {@code stubDir}
   * and a main trace continuing from the stub's far end into {@code mainCorners}. {@code
   * swapConnectionToPin:1263-1265} needs the main trace's end to have <b>exactly one</b> contact,
   * which is the stub, and {@code :1275-1289} needs the two to meet at a sharp angle.
   */
  /**
   * {@code stubHalfWidth} is the stub's own half width, which is normally the main trace's.
   * {@code wide-stub} makes them differ on purpose: {@code swapConnectionToPin} never compares
   * widths, but {@code combineAtStart} does ({@code PolylineTrace.java:239-244}), so that row is
   * a {@code swap} that succeeds and whose {@code combine()} then merges <b>nothing</b> — the
   * shape that separates Java's per-merge {@code additionalUpdateAfterChange} at {@code :188}
   * from a single unconditional call. The board dump proves the premise (the stub survives as a
   * separate item); the call count itself is not observable from outside the JVM, and
   * {@code crates/fr-router/tests/connection_to_pin.rs} pins it against a live engine.
   */
  record SwapCase(String name, Point stubEnd, Point[] mainCorners, int halfWidth, int stubHalfWidth) {}

  static SwapCase[] swapTable() {
    return new SwapCase[] {
      new SwapCase("left-stub-sharp", p(-800, 0), new Point[] {p(-800, 0), p(-200, 300)}, 30, 30),
      new SwapCase("left-stub-blunt", p(-800, 0), new Point[] {p(-800, 0), p(-1400, 300)}, 30, 30),
      new SwapCase("right-stub-sharp", p(-200, 0), new Point[] {p(-200, 0), p(-800, 300)}, 30, 30),
      new SwapCase("left-stub-long", p(-1200, 0), new Point[] {p(-1200, 0), p(-300, 700)}, 30, 30),
      new SwapCase("left-stub-kink", p(-800, 0), new Point[] {p(-800, 0), p(-780, 20), p(-200, 400)}, 30, 30),
      new SwapCase("left-stub-fat", p(-900, 0), new Point[] {p(-900, 0), p(-200, 500)}, 90, 90),
      new SwapCase("wide-stub", p(-800, 0), new Point[] {p(-800, 0), p(-200, 300)}, 30, 90),
    };
  }

  static void swapMode() {
    for (AngleRestriction ar : REGIMES) {
      for (double edge : EDGE_DISTS) {
        for (SwapCase c : swapTable()) {
          for (boolean atStart : new boolean[] {true, false}) {
            build(ar, edge);
            Polyline stub = new Polyline(new Point[] {p(-500, 0), c.stubEnd()});
            board.insertTraceWithoutCleaning(
                stub, 0, c.stubHalfWidth(), new int[] {1}, 1, FixedState.SHOVE_FIXED);
            Point[] mainCorners = c.mainCorners();
            if (!atStart) {
              Point[] reversed = new Point[mainCorners.length];
              for (int i = 0; i < mainCorners.length; i++) {
                reversed[i] = mainCorners[mainCorners.length - 1 - i];
              }
              mainCorners = reversed;
            }
            board.insertTraceWithoutCleaning(
                new Polyline(mainCorners), 0, c.halfWidth(), new int[] {1}, 1, FixedState.UNFIXED);
            PolylineTrace main = lastTrace();
            boolean changed = main.swapConnectionToPin(atStart);
            out.println(
                "regime="
                    + ar
                    + " edge="
                    + Double.toString(edge)
                    + " case="
                    + c.name()
                    + " atStart="
                    + atStart
                    + " changed="
                    + changed);
            dumpBoard();
          }
        }
      }
    }
  }

  // --- mode `rand` -------------------------------------------------------------------------------

  static final int RANDOM_COUNT = 128;
  static final long RANDOM_SEED = 70605L;

  static void randMode() {
    for (AngleRestriction ar : REGIMES) {
      out.println("regime=" + ar + " n=" + RANDOM_COUNT + " seed=" + RANDOM_SEED);
      Random rnd = new Random(RANDOM_SEED);
      for (int i = 0; i < RANDOM_COUNT; i++) {
        int cornerCount = 2 + rnd.nextInt(4);
        Point[] corners = new Point[cornerCount];
        corners[0] = p(-500, 0);
        for (int j = 1; j < cornerCount; j++) {
          corners[j] = p(rnd.nextInt(2401) - 1200, rnd.nextInt(1601) - 400);
        }
        int halfWidth = 10 + rnd.nextInt(60);
        double edge = EDGE_DISTS[rnd.nextInt(EDGE_DISTS.length)];
        // `rnd.nextInt(2) == 0` rather than `rnd.nextBoolean()`: the port's `JavaRandom`
        // reproduces `nextInt(int)` and `nextDouble()` and has no `nextBoolean`, and the two
        // consume the underlying stream differently, so the driver picks the one both can express.
        boolean atStart = rnd.nextInt(2) == 0;
        // `check` on its own board.
        build(ar, edge);
        Polyline polyline = new Polyline(corners);
        if (polyline.lines.length < 3) {
          out.println("row " + i + " degenerate");
          continue;
        }
        board.insertTraceWithoutCleaning(polyline, 0, halfWidth, new int[] {1}, 1, FixedState.UNFIXED);
        PolylineTrace trace = lastTrace();
        boolean checked = trace == null || trace.checkConnectionToPin(atStart);
        out.println(
            "row " + i + " hw=" + halfWidth + " edge=" + Double.toString(edge) + " atStart=" + atStart + " check=" + checked);
        out.println("  in  " + poly(polyline));
        // `correct` on a fresh board.
        build(ar, edge);
        board.insertTraceWithoutCleaning(new Polyline(corners), 0, halfWidth, new int[] {1}, 1, FixedState.UNFIXED);
        PolylineTrace correctTrace = lastTrace();
        boolean corrected = correctTrace != null && correctTrace.correctConnectionToPin(atStart, ar);
        out.println("  correct=" + corrected);
        dumpBoard();
        // `swap` on a fresh board with a SHOVE_FIXED stub from the pin to the trace's first corner.
        build(ar, edge);
        Polyline stub = new Polyline(new Point[] {p(-500, 0), corners[1]});
        if (stub.lines.length >= 3) {
          board.insertTraceWithoutCleaning(stub, 0, halfWidth, new int[] {1}, 1, FixedState.SHOVE_FIXED);
          Point[] tail = new Point[cornerCount - 1];
          System.arraycopy(corners, 1, tail, 0, cornerCount - 1);
          Polyline tailPolyline = new Polyline(tail);
          if (tailPolyline.lines.length >= 3) {
            board.insertTraceWithoutCleaning(
                tailPolyline, 0, halfWidth, new int[] {1}, 1, FixedState.UNFIXED);
            PolylineTrace swapTrace = lastTrace();
            out.println("  swap=" + (swapTrace != null && swapTrace.swapConnectionToPin(atStart)));
            dumpBoard();
          } else {
            out.println("  swap=degenerate-tail");
          }
        } else {
          out.println("  swap=degenerate-stub");
        }
      }
    }
  }

  // --- mode `edge` -------------------------------------------------------------------------------

  /**
   * {@code PolylineTrace.pullTight:841-861} itself: the branch is entered only when the angle
   * restriction is not 90 degrees <b>and</b> {@code getPinEdgeToTurnDist() > 0}. Driving it here
   * rather than the three methods directly is what makes the two skips observable — a port that
   * wired the four calls unconditionally would still match `check`/`correct`/`swap` and diff here.
   */
  static void edgeMode() {
    for (AngleRestriction ar : REGIMES) {
      for (double edge : EDGE_DISTS) {
        for (Case c : table()) {
          build(ar, edge);
          PolylineTrace trace = insert(c);
          app.freerouting.board.optimize.TraceTightener algo =
              app.freerouting.board.optimize.TraceTightener.getInstance(
                  board, new int[0], null, 500, null, -1, null, -1);
          boolean changed = trace.pullTight(algo);
          out.println(
              "regime="
                  + ar
                  + " edge="
                  + Double.toString(edge)
                  + " case="
                  + c.name()
                  + " pullTight="
                  + changed);
          dumpBoard();
        }
      }
    }
  }
}
