// Plan 6 Task 15a ground-truth probe (controller ruling AB): the pull-tight family —
// `board/optimize/TraceTightener.java` (547 lines), `TraceTightener90.java` (169),
// `TraceTightener45.java` (674), `TraceTightenerAnyAngle.java` (1004) and
// `board/trace/PolylineTrace.java`'s two `pullTight` overloads (`:809-863` and `:869-890`).
//
// It is not a differential driver — there is no Rust twin and `run.sh` does not know it — but
// every literal in `crates/fr-router/tests/tightener.rs` is read off its stdout, so it is
// committed here to keep those numbers reproducible.
//
// It declares `package app.freerouting.board.optimize` so it can name the three package-private
// subclasses and call the package-private `repositionLines` / `skipSegmentsOfLength0` and the
// protected `repositionLine`, none of which any public method exposes. It **compiles together
// with `P6T9Probe.java`**, which declares the same package, and reuses that probe's `build()` —
// the two-layer, two-pin, two-trace board `P6T3.build` / `P6T7Probe.build` / `P6T9Probe.build`
// all share — rather than declaring a second fixture.
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential/java/probes
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p6t15a P6T9Probe.java P6T15aProbe.java
//   for m in inst lineeq t90 t45 tany trace smooth pinedge rand90 rand45 randany; do
//     echo "######## $m"
//     "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//         -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p6t15a:$JAR" \
//         app.freerouting.board.optimize.P6T15aProbe "$m" 2>/dev/null \
//       | grep -Ev '^[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9:.]+ +(WARN|INFO|DEBUG|ERROR) '
//   done > crates/fr-router/tests/data/p6t15a-tightener.txt
//
// The `######## <mode>` banners of the committed transcript come from this loop, not from the
// probe; the `grep` strips `FRLogger`'s timestamped lines.
//
// Every polyline is printed as its **line array** — the lines are the polyline's only state, and
// they are `IntPoint`-based, so the dump is exact — plus the corner list, where an `IntPoint`
// corner prints as `(x,y)` and a `RationalPoint` corner prints as `~(<Double.toString x>,
// <Double.toString y>)` of `cornerApprox`. `fr_dsn::format::double::java_double_to_string`
// reproduces `Double.toString` on the Rust side.
//
// Modes:
//   inst     `TraceTightener.getInstance` (:87-114): which concrete class each of the three
//            `AngleRestriction`s builds, and the `Math.max(minTranslateDist, 100)` clamp of
//            `:112`.
//   lineeq   quirk #34's site, `TraceTightener.repositionLine:281` — `checkLines[1].equals(
//            translateLine)`. Prints, for a hand-built pair, Java's geometric `Line.equals`, the
//            structural end-point comparison the Rust port derives, and `Line.getId()`, on a case
//            where the two disagree; plus the `repositionLine` answer that depends on it.
//   t90      `TraceTightener90.pullTight` (:26-36) over a fixed polyline table on the 90-degree
//            board, plus `repositionLines` and `skipSegmentsOfLength0` called directly.
//   t45      the same table through `TraceTightener45.pullTight` (:35-45) on the 45-degree board.
//   tany     the same table through `TraceTightenerAnyAngle.pullTight` (:35-56) on the any-angle
//            board.
//   trace    `PolylineTrace.pullTight(TraceTightener)` (:809-863) and
//            `PolylineTrace.pullTight(boolean, int, Stoppable)` (:869-890) on the board's own two
//            traces in all three regimes: the return value, the trace's polyline afterwards and
//            the whole board item list (`getItems()` order, descending id — quirk #63) plus
//            `maxGeneratedId`.
//   smooth   `TraceTightener.smoothenEndCornersAtTrace` (:402-411) per regime, which is where the
//            90-degree regime's two `null` overrides (TraceTightener90.java:160-168) show.
//   pinedge  `PolylineTrace.pullTight:841-861`, the `pinEdgeToTurnDist > 0` branch that calls
//            `swapConnectionToPin` / `correctConnectionToPin` — ground truth for the two methods
//            `fr-board` still defers to Plan 7 (this task's port answers `false` there).
//   rand90   256 random polylines through `TraceTightener90.pullTight`, `java.util.Random(4242)`.
//   rand45   the same stream through `TraceTightener45.pullTight`.
//   randany  the same stream through `TraceTightenerAnyAngle.pullTight`.
package app.freerouting.board.optimize;

import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.trace.PolylineTrace;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.IntOctagon;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.Line;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import java.util.Collection;
import java.util.Random;
import java.util.Set;

/** Task 15a ground truth: the four `TraceTightener`s and `PolylineTrace.pullTight`. */
public class P6T15aProbe {

  public static void main(String[] args) throws Exception {
    String mode = args.length > 0 ? args[0] : "inst";
    switch (mode) {
      case "inst" -> instances();
      case "lineeq" -> lineEquality();
      case "t90" -> fixedTable(AngleRestriction.NINETY_DEGREE);
      case "t45" -> fixedTable(AngleRestriction.FORTYFIVE_DEGREE);
      case "tany" -> fixedTable(AngleRestriction.NONE);
      case "trace" -> tracePullTight();
      case "smooth" -> smoothen();
      case "pinedge" -> pinEdge();
      case "rand90" -> randomTable(AngleRestriction.NINETY_DEGREE);
      case "rand45" -> randomTable(AngleRestriction.FORTYFIVE_DEGREE);
      case "randany" -> randomTable(AngleRestriction.NONE);
      default -> throw new IllegalArgumentException("mode");
    }
  }

  // --- dumps ------------------------------------------------------------------------------------

  static String ln(Line l) {
    return "(" + ((IntPoint) l.a).x
        + ","
        + ((IntPoint) l.a).y
        + ")->("
        + ((IntPoint) l.b).x
        + ","
        + ((IntPoint) l.b).y
        + ")";
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
    sb.append("]");
    return sb.toString();
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
    System.out.println(
        "    maxId=" + P6T9Probe.board.communication.idGenerator.maxGeneratedId());
    for (Item item : P6T9Probe.board.getItems()) {
      StringBuilder sb = new StringBuilder();
      sb.append("    item id=")
          .append(item.getId())
          .append(" type=")
          .append(item.getClass().getSimpleName())
          .append(" nets=")
          .append(nets(item.netNumbers))
          .append(" cl=")
          .append(item.clearanceClassIndex());
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
      System.out.println(sb);
    }
  }

  // --- the fixed polyline table -----------------------------------------------------------------

  record Case(String name, Point[] corners, int layer, int halfWidth, int[] nets, int cl) {}

  static IntPoint p(int x, int y) {
    return new IntPoint(x, y);
  }

  /**
   * Eleven hand-placed polylines. The first six live in free space south-west of the two traces
   * and are pure detours the tightener can shorten; the rest run into the two traces, the SMD pad
   * and the through pad, so the `board.checkTraceShape` guard inside every tightening step has to
   * refuse at least one candidate.
   */
  static Case[] table() {
    return new Case[] {
      new Case("straight", new Point[] {p(-3000, -3000), p(-1000, -3000)}, 0, 30, new int[] {3}, 1),
      new Case(
          "elbow",
          new Point[] {p(-3000, -3000), p(-3000, -1000), p(-1000, -1000)},
          0,
          30,
          new int[] {3},
          1),
      new Case(
          "staircase",
          new Point[] {
            p(-3000, -3000),
            p(-3000, -2500),
            p(-2500, -2500),
            p(-2500, -2000),
            p(-2000, -2000),
            p(-2000, -1500),
            p(-1500, -1500)
          },
          0,
          30,
          new int[] {3},
          1),
      new Case(
          "detour",
          new Point[] {
            p(-3000, -3000), p(-3000, 0), p(-2000, 0), p(-2000, -3000), p(-1000, -3000)
          },
          0,
          30,
          new int[] {3},
          1),
      new Case(
          "diagonal",
          new Point[] {p(-3000, -3000), p(-2000, -2000), p(-2000, -1000), p(-1000, 0)},
          0,
          30,
          new int[] {3},
          1),
      new Case(
          "zerolen",
          new Point[] {
            p(-3000, -3000), p(-3000, -3000), p(-2000, -3000), p(-2000, -3000), p(-2000, -2000)
          },
          0,
          30,
          new int[] {3},
          1),
      new Case(
          "wide",
          new Point[] {
            p(-3000, -3000), p(-3000, -2000), p(-2000, -2000), p(-2000, -1000), p(-1000, -1000)
          },
          0,
          400,
          new int[] {3},
          2),
      new Case(
          "pastNet2",
          new Point[] {p(-1500, 400), p(-1500, 1400), p(-200, 1400), p(-200, 400)},
          0,
          30,
          new int[] {3},
          1),
      new Case(
          "roundPad",
          new Point[] {p(-500, -600), p(-1200, -600), p(-1200, 600), p(-500, 600)},
          0,
          30,
          new int[] {3},
          1),
      new Case(
          "layer1",
          new Point[] {p(200, -1500), p(200, -500), p(1200, -500), p(1200, -1500)},
          1,
          30,
          new int[] {3},
          1),
      new Case(
          "ownNet1",
          new Point[] {p(-400, -700), p(-400, -100), p(400, -100), p(400, -700)},
          0,
          30,
          new int[] {1},
          1),
    };
  }

  static TraceTightener algo(int minTranslateDist) {
    return TraceTightener.getInstance(
        P6T9Probe.board, new int[0], null, minTranslateDist, null, -1, null, -1);
  }

  static void fixedTable(AngleRestriction angleRestriction) {
    P6T9Probe.build(angleRestriction);
    System.out.println("regime=" + angleRestriction);
    for (Case c : table()) {
      Polyline before = new Polyline(c.corners());
      TraceTightener a = algo(500);
      Polyline after =
          a.pullTight(before, c.layer(), c.halfWidth(), c.nets(), c.cl(), (Set<Pin>) null);
      System.out.println("case=" + c.name() + " same=" + (after == before));
      System.out.println("  before " + poly(before));
      System.out.println("  after  " + poly(after));

      // `repositionLines` (:215-230) and `skipSegmentsOfLength0` (:338-399) on their own, so the
      // two shared helpers of the base class are pinned apart from the regime loops.
      TraceTightener b = algo(500);
      b.pullTight(before, c.layer(), c.halfWidth(), c.nets(), c.cl(), (Set<Pin>) null);
      Polyline repositioned = b.repositionLines(before);
      Polyline skipped = b.skipSegmentsOfLength0(before);
      System.out.println(
          "  reposition same=" + (repositioned == before) + " " + poly(repositioned));
      System.out.println("  skip0      same=" + (skipped == before) + " " + poly(skipped));
    }

    // The clip shape gate: the same table with a clip octagon that excludes everything.
    IntOctagon clip = new IntOctagon(0, 0, 100, 100, -200, 200, -200, 200);
    for (Case c : table()) {
      Polyline before = new Polyline(c.corners());
      TraceTightener a =
          TraceTightener.getInstance(
              P6T9Probe.board, new int[0], clip, 500, null, -1, null, -1);
      Polyline after =
          a.pullTight(before, c.layer(), c.halfWidth(), c.nets(), c.cl(), (Set<Pin>) null);
      System.out.println("clip=" + c.name() + " same=" + (after == before) + " " + poly(after));
    }

    // `minTranslateDist` below the `Math.max(.., 100)` floor, on the detour case.
    for (int mtd : new int[] {0, 1, 100, 5000}) {
      Polyline before = new Polyline(table()[3].corners());
      TraceTightener a = algo(mtd);
      Polyline after = a.pullTight(before, 0, 30, new int[] {3}, 1, (Set<Pin>) null);
      System.out.println("mtd=" + mtd + " same=" + (after == before) + " " + poly(after));
    }
  }

  // --- getInstance --------------------------------------------------------------------------------

  static void instances() throws Exception {
    for (AngleRestriction ar :
        new AngleRestriction[] {
          AngleRestriction.NINETY_DEGREE, AngleRestriction.FORTYFIVE_DEGREE, AngleRestriction.NONE
        }) {
      P6T9Probe.build(ar);
      for (int mtd : new int[] {-5, 0, 99, 100, 101, 500}) {
        TraceTightener a = algo(mtd);
        System.out.println(
            "regime="
                + ar
                + " mtd="
                + mtd
                + " class="
                + a.getClass().getSimpleName()
                + " minTranslateDist="
                + a.minTranslateDist
                + " onlyNetNoArrLen="
                + a.onlyNetNoArr.length
                + " clip="
                + a.currentClipShape);
      }
      // `splitTracesAtKeepPoint` (:476-491) with and without a keep point.
      TraceTightener noKeep = algo(500);
      System.out.println("regime=" + ar + " splitAtKeepPoint(null)=" + noKeep.splitTracesAtKeepPoint());
      TraceTightener keep =
          TraceTightener.getInstance(
              P6T9Probe.board, new int[0], null, 500, null, -1, new IntPoint(0, 200), 0);
      System.out.println("regime=" + ar + " splitAtKeepPoint(0,200)=" + keep.splitTracesAtKeepPoint());
      dumpBoard();
    }
  }

  // --- quirk #34: Line.equals at repositionLine:281 -----------------------------------------------

  static void lineEquality() {
    P6T9Probe.build(AngleRestriction.NONE);
    // A pair of lines that are the *same* geometric line described by different end points:
    // `Line.equals` (Line.java:57-79) answers true, a structural end-point comparison answers
    // false, and `getId()` — the hash Java never overrides — differs, which is the whole of
    // quirk #34.
    Line base = new Line(0, 0, 100, 0);
    Line sameGeometry = new Line(-50, 0, 250, 0);
    Line opposite = new Line(100, 0, 0, 0);
    Line parallel = new Line(0, 1, 100, 1);
    Line degenerate = new Line(7, 9, 7, 9);
    Line[][] pairs = {
      {base, base},
      {base, sameGeometry},
      {base, opposite},
      {base, parallel},
      {degenerate, degenerate},
      {degenerate, new Line(7, 9, 7, 9)},
    };
    for (Line[] pair : pairs) {
      Line x = pair[0];
      Line y = pair[1];
      boolean structural =
          ((IntPoint) x.a).x == ((IntPoint) y.a).x
              && ((IntPoint) x.a).y == ((IntPoint) y.a).y
              && ((IntPoint) x.b).x == ((IntPoint) y.b).x
              && ((IntPoint) x.b).y == ((IntPoint) y.b).y;
      System.out.println(
          "pair "
              + ln(x)
              + " vs "
              + ln(y)
              + " equals="
              + x.equals(y)
              + " structural="
              + structural
              + " sameRef="
              + (x == y)
              + " idEqual="
              + (x.getId() == y.getId()));
    }

    // `translate(dist)` with |dist| < 1 answers a line that is geometrically the same but has
    // different end points — exactly the case `repositionLine:281` guards against, and exactly
    // the case where the structural test would let the loop run on.
    Line diagonal = new Line(0, 0, 1000, 1000);
    for (double dist : new double[] {0.0, 0.2, 0.4, 0.6, 0.9, 1.0, 1.5, -0.4, -0.9}) {
      Line translated = diagonal.translate(dist);
      boolean structural =
          ((IntPoint) translated.a).x == ((IntPoint) diagonal.a).x
              && ((IntPoint) translated.a).y == ((IntPoint) diagonal.a).y
              && ((IntPoint) translated.b).x == ((IntPoint) diagonal.b).x
              && ((IntPoint) translated.b).y == ((IntPoint) diagonal.b).y;
      System.out.println(
          "translate dist="
              + Double.toString(dist)
              + " line="
              + ln(translated)
              + " equals="
              + translated.equals(diagonal)
              + " structural="
              + structural
              + " sameRef="
              + (translated == diagonal));
    }

    // The guard in situ. Four hand-built line arrays, each run through `repositionLine` in all
    // three regimes — the 90- and 45-degree instances take the base body
    // (TraceTightener.java:235-331, whose guard is `:281`), the any-angle instance takes its own
    // override (TraceTightenerAnyAngle.java:497-657, whose guards are `:568` and `:576`), and the
    // two count their index argument from different ends.
    //
    //   square    a closed square, where neither line can move
    //   onLine    both neighbouring corners lie **on** the line to translate, so
    //             `Line.getInstance(nearestPoint, translateLine.direction())` is the same line of
    //             the plane with different end points — `:281`'s geometric `equals` answers true
    //             and returns null where a structural `==` would let the loop run on
    //             (docs/java-quirks.md #34)
    //   subUnit   the nearest corner is 0.5 units from a near-horizontal translate line, so
    //             `translate(-0.5)` answers the same line — `:568`
    //   halfInt   the same, with the nearest corner a **fractional** intersection whose `round()`
    //             is farther away than the translation, so `:571`'s replacement is skipped and
    //             `:576` returns null
    Line[][] scripts = {
      {
        new Line(5000, 5000, 5000, 5100),
        new Line(5000, 5000, 6000, 5000),
        new Line(6000, 5000, 6000, 6000),
        new Line(6000, 6000, 5000, 6000),
        new Line(5000, 6000, 5000, 5000),
      },
      {
        new Line(-3000, -3000, -1000, -3000),
        new Line(-3000, -3100, -3000, -2900),
        new Line(-2500, -3000, -1500, -3000),
        new Line(-2000, -3100, -2000, -2900),
        new Line(-2000, -3000, -1000, -3000),
      },
      {
        new Line(5500, 5001, 5500, 5002),
        new Line(5500, 5001, 5600, 5001),
        new Line(5000, 5000, 7000, 5002),
        new Line(5900, 5003, 5900, 5004),
        new Line(5900, 5003, 6000, 5003),
      },
      {
        new Line(5000, 5000, 5002, 5001),
        new Line(5000, 5001, 5002, 5000),
        new Line(5000, 5000, 7000, 5002),
        new Line(5500, 5003, 5500, 5004),
        new Line(5500, 5003, 5600, 5003),
      },
    };
    String[] names = {"square", "onLine", "subUnit", "halfInt"};
    for (AngleRestriction ar :
        new AngleRestriction[] {
          AngleRestriction.NINETY_DEGREE, AngleRestriction.FORTYFIVE_DEGREE, AngleRestriction.NONE
        }) {
      P6T9Probe.build(ar);
      for (int k = 0; k < scripts.length; k++) {
        Line[] lines = scripts[k];
        TraceTightener a = algo(500);
        // Prime `currentLayer` / `currentHalfWidth` / `currentNetNumbers` /
        // `currentClearanceClassIndex` off a polyline of its own: the scripts below are raw
        // `Line[]` arrays that the normalising constructor would mangle.
        a.pullTight(
            new Polyline(new Point[] {p(-3000, -3000), p(-1000, -3000)}),
            0,
            30,
            new int[] {3},
            1,
            (Set<Pin>) null);
        for (int no = 0; no <= 2; no++) {
          Line result;
          try {
            result = a.repositionLine(lines, no);
          } catch (RuntimeException e) {
            System.out.println(
                "repositionLine regime=" + ar + " script=" + names[k] + " no=" + no + " -> threw "
                    + e.getClass().getSimpleName());
            continue;
          }
          System.out.println(
              "repositionLine regime="
                  + ar
                  + " script="
                  + names[k]
                  + " no="
                  + no
                  + " -> "
                  + (result == null ? "null" : ln(result)));
        }
      }
    }
  }

  // --- PolylineTrace.pullTight --------------------------------------------------------------------

  static void tracePullTight() {
    for (AngleRestriction ar :
        new AngleRestriction[] {
          AngleRestriction.NINETY_DEGREE, AngleRestriction.FORTYFIVE_DEGREE, AngleRestriction.NONE
        }) {
      // (a) `pullTight(TraceTightener)` on the two traces the board already carries.
      P6T9Probe.build(ar);
      System.out.println("regime=" + ar + " overload=algo");
      TraceTightener a = algo(500);
      for (Item item : P6T9Probe.board.getItems().toArray(new Item[0])) {
        if (item instanceof PolylineTrace trace) {
          System.out.println("  trace id=" + trace.getId() + " before " + poly(trace.polyline()));
          boolean changed = trace.pullTight(a);
          System.out.println(
              "  trace id=" + trace.getId() + " changed=" + changed + " after  " + poly(trace.polyline()));
        }
      }
      dumpBoard();

      // (b) the same on a freshly inserted detour trace, which the tightener can actually shorten.
      P6T9Probe.build(ar);
      System.out.println("regime=" + ar + " overload=algo-detour");
      Polyline detour =
          new Polyline(
              new Point[] {
                p(-3000, -3000), p(-3000, 0), p(-2000, 0), p(-2000, -3000), p(-1000, -3000)
              });
      P6T9Probe.board.insertTraceWithoutCleaning(
          detour, 0, 30, new int[] {3}, 1, FixedState.UNFIXED);
      TraceTightener b = algo(500);
      for (Item item : P6T9Probe.board.getItems().toArray(new Item[0])) {
        if (item instanceof PolylineTrace trace && trace.netNumbers[0] == 3) {
          boolean changed = trace.pullTight(b);
          System.out.println("  trace id=" + trace.getId() + " changed=" + changed);
        }
      }
      dumpBoard();

      // (c) the `(ownNetOnly, accuracy, Stoppable)` overload (:869-890).
      for (boolean ownNetOnly : new boolean[] {true, false}) {
        P6T9Probe.build(ar);
        System.out.println("regime=" + ar + " overload=flags ownNetOnly=" + ownNetOnly);
        P6T9Probe.board.insertTraceWithoutCleaning(
            new Polyline(
                new Point[] {
                  p(-3000, -3000), p(-3000, 0), p(-2000, 0), p(-2000, -3000), p(-1000, -3000)
                }),
            0,
            30,
            new int[] {3},
            1,
            FixedState.UNFIXED);
        for (Item item : P6T9Probe.board.getItems().toArray(new Item[0])) {
          if (item instanceof PolylineTrace trace) {
            boolean changed = trace.pullTight(ownNetOnly, 500, null);
            System.out.println("  trace id=" + trace.getId() + " changed=" + changed);
          }
        }
        dumpBoard();
      }

      // (d) the four refusals of `:811-828`: not on the board, shove-fixed, a non-normal net, and
      // an `onlyNetNoArr` that does not match.
      P6T9Probe.build(ar);
      System.out.println("regime=" + ar + " overload=refusals");
      PolylineTrace fixedTrace =
          (PolylineTrace)
              P6T9Probe.board.getItem(
                  P6T9Probe.board
                      .insertTraceWithoutCleaning(
                          new Polyline(
                              new Point[] {
                                p(-3000, -3000),
                                p(-3000, 0),
                                p(-2000, 0),
                                p(-2000, -3000),
                                p(-1000, -3000)
                              }),
                          0,
                          30,
                          new int[] {3},
                          1,
                          FixedState.SHOVE_FIXED)
                      .getId());
      System.out.println("  shoveFixed changed=" + fixedTrace.pullTight(algo(500)));
      PolylineTrace free =
          (PolylineTrace)
              P6T9Probe.board.getItem(
                  P6T9Probe.board
                      .insertTraceWithoutCleaning(
                          new Polyline(
                              new Point[] {
                                p(-3000, 3000),
                                p(-3000, 6000),
                                p(-2000, 6000),
                                p(-2000, 3000),
                                p(-1000, 3000)
                              }),
                          0,
                          30,
                          new int[] {3},
                          1,
                          FixedState.UNFIXED)
                      .getId());
      TraceTightener otherNet =
          TraceTightener.getInstance(
              P6T9Probe.board, new int[] {2}, null, 500, null, -1, null, -1);
      System.out.println("  otherNetFilter changed=" + free.pullTight(otherNet));
      P6T9Probe.board.removeItem(free);
      System.out.println("  removed changed=" + free.pullTight(algo(500)));
      dumpBoard();
    }
  }

  // --- smoothenEndCornersAtTrace ------------------------------------------------------------------

  static void smoothen() {
    for (AngleRestriction ar :
        new AngleRestriction[] {
          AngleRestriction.NINETY_DEGREE, AngleRestriction.FORTYFIVE_DEGREE, AngleRestriction.NONE
        }) {
      P6T9Probe.build(ar);
      System.out.println("regime=" + ar);
      // A second trace meeting the net-1 trace's start corner at an acute angle. Its start
      // corner is the SMD **pin**, so `smoothenStartCornerAtTrace`'s contact loop hits the
      // `else { return null; }` arm at TraceTightener45.java:517-519 — that is the row that
      // pins the "any non-trace contact refuses" rule.
      P6T9Probe.board.insertTraceWithoutCleaning(
          new Polyline(new Point[] {p(-500, 0), p(-1200, 700)}), 0, 30, new int[] {1}, 1,
          FixedState.UNFIXED);
      // Four net-3 pairs away from every pin, so the contact loop sees traces only:
      //   T1/T2 meet at (1000,1000) with an **acute** angle and an orthogonal other line, which
      //          is the `acuteAngle` arm in both the 45-degree and the any-angle regime;
      //   T3/T4 meet at (3000,1000) at a right angle with `cornerCount > 2`, which is the
      //          `bend` arm and therefore `repositionLine` reached from a smoothen method;
      //   T5/T6 meet at T5's **last** corner (6000,2000), which is `smoothenEndCornerAtTrace`.
      int[] net3 = {3};
      P6T9Probe.board.insertTraceWithoutCleaning(
          new Polyline(new Point[] {p(1000, 1000), p(2000, 2000)}), 0, 30, net3, 1,
          FixedState.UNFIXED);
      P6T9Probe.board.insertTraceWithoutCleaning(
          new Polyline(new Point[] {p(1000, 1000), p(1000, 2000)}), 0, 30, net3, 1,
          FixedState.UNFIXED);
      P6T9Probe.board.insertTraceWithoutCleaning(
          new Polyline(new Point[] {p(3000, 1000), p(4000, 1000), p(4000, 2000)}), 0, 30, net3, 1,
          FixedState.UNFIXED);
      P6T9Probe.board.insertTraceWithoutCleaning(
          new Polyline(new Point[] {p(3000, 1000), p(3000, 2000)}), 0, 30, net3, 1,
          FixedState.UNFIXED);
      P6T9Probe.board.insertTraceWithoutCleaning(
          new Polyline(new Point[] {p(5000, 1000), p(6000, 2000)}), 0, 30, net3, 1,
          FixedState.UNFIXED);
      P6T9Probe.board.insertTraceWithoutCleaning(
          new Polyline(new Point[] {p(6000, 2000), p(6000, 1000)}), 0, 30, net3, 1,
          FixedState.UNFIXED);
      //   T7/T8 meet at T7's last corner (8000,2000) at a right angle, with the projection of
      //          T7's *previous* line onto T8's direction POSITIVE. That is the only shape that
      //          separates `TraceTightenerAnyAngle.smoothenEndCornerAtTrace`'s `prevLineDirection`
      //          (`:908`, which reads `lines[endLineNo]` — the same line as `lineDirection`) from
      //          `TraceTightener45.smoothenEndCornerAtTrace`'s (`:586`, `lines[length - 3]`), and
      //          it is what makes the any-angle `bend` arm at `:988-1001` unreachable.
      P6T9Probe.board.insertTraceWithoutCleaning(
          new Polyline(new Point[] {p(9500, 5000), p(6000, 5000), p(6000, 6000)}), 0, 30, net3, 1,
          FixedState.UNFIXED);
      P6T9Probe.board.insertTraceWithoutCleaning(
          new Polyline(new Point[] {p(6000, 6000), p(9500, 6000), p(9500, 7000)}), 0, 30, net3, 1,
          FixedState.UNFIXED);
      TraceTightener a = algo(500);
      for (Item item : P6T9Probe.board.getItems().toArray(new Item[0])) {
        if (item instanceof PolylineTrace trace) {
          // `smoothenStartCornerAtTrace` reads `currentLayer`, `currentHalfWidth`,
          // `currentNetNumbers` and `currentClearanceClassIndex`, which only
          // `smoothenEndCornersAtTrace:406-409` and the six-argument `pullTight:182-188` ever
          // write; calling either override on a fresh instance dereferences a null
          // `currentNetNumbers` inside `checkTraceShape` (BasicBoard.java:1017). Prime the
          // instance with the trace's own parameters first, exactly as `:406-409` would.
          a.pullTight(
              trace.polyline(),
              trace.getLayer(),
              trace.getHalfWidth(),
              trace.netNumbers,
              trace.clearanceClassIndex(),
              (Set<Pin>) null);
          Polyline start = a.smoothenStartCornerAtTrace(trace);
          Polyline end = a.smoothenEndCornerAtTrace(trace);
          System.out.println("  trace id=" + trace.getId() + " start=" + poly(start));
          System.out.println("  trace id=" + trace.getId() + " end=  " + poly(end));
        }
      }
      for (Item item : P6T9Probe.board.getItems().toArray(new Item[0])) {
        if (item instanceof PolylineTrace trace) {
          System.out.println(
              "  trace id=" + trace.getId() + " smoothenEndCorners=" + a.smoothenEndCornersAtTrace(trace));
        }
      }
      dumpBoard();
    }
  }

  // --- the pin-edge branch of PolylineTrace.pullTight:841-861 -------------------------------------

  /**
   * `PolylineTrace.pullTight:841-861` — the `angleRestriction != NINETY_DEGREE &&
   * board.rules.getPinEdgeToTurnDist() > 0` branch, which calls `swapConnectionToPin` and
   * `correctConnectionToPin`.
   *
   * <p>Plan 6 never reaches it: `FoundConnectionInserter.insertTrace:140-141` sets
   * `pinEdgeToTurnDist` to `-1` for the whole insertion and restores it at `:447`. A board read
   * from a DSN file does have it set (`io/specctra/parser/Structure.java:668,714`), so this mode
   * records the JVM's answers for whoever ports the two `*ConnectionToPin` methods — they carry
   * `// added in Plan 7:` markers in `crates/fr-board/src/items/trace.rs`.
   */
  static void pinEdge() {
    for (AngleRestriction ar :
        new AngleRestriction[] {
          AngleRestriction.NINETY_DEGREE, AngleRestriction.FORTYFIVE_DEGREE, AngleRestriction.NONE
        }) {
      for (double edgeToTurnDist : new double[] {0.0, 500.0}) {
        P6T9Probe.build(ar);
        P6T9Probe.board.rules.setPinEdgeToTurnDist(edgeToTurnDist);
        System.out.println(
            "regime=" + ar + " pinEdgeToTurnDist=" + Double.toString(edgeToTurnDist));
        TraceTightener a = algo(500);
        for (Item item : P6T9Probe.board.getItems().toArray(new Item[0])) {
          if (item instanceof PolylineTrace trace) {
            System.out.println(
                "  trace id=" + trace.getId() + " changed=" + trace.pullTight(a));
          }
        }
        dumpBoard();
      }
    }
  }

  // --- the random block ---------------------------------------------------------------------------

  static final int RANDOM_COUNT = 256;
  static final long RANDOM_SEED = 4242L;

  static void randomTable(AngleRestriction angleRestriction) {
    P6T9Probe.build(angleRestriction);
    System.out.println("regime=" + angleRestriction + " n=" + RANDOM_COUNT + " seed=" + RANDOM_SEED);
    Random rnd = new Random(RANDOM_SEED);
    for (int i = 0; i < RANDOM_COUNT; i++) {
      int cornerCount = 2 + rnd.nextInt(7);
      Point[] corners = new Point[cornerCount];
      for (int j = 0; j < cornerCount; j++) {
        corners[j] = new IntPoint(rnd.nextInt(4001) - 2000, rnd.nextInt(4001) - 2000);
      }
      int halfWidth = 10 + rnd.nextInt(60);
      int layer = rnd.nextInt(2);
      int net = 1 + rnd.nextInt(3);
      int cl = 1 + rnd.nextInt(2);
      int mtd = 100 + rnd.nextInt(900);
      Polyline before = new Polyline(corners);
      if (before.lines.length < 3) {
        System.out.println("row " + i + " degenerate");
        continue;
      }
      TraceTightener a = algo(mtd);
      Polyline after = a.pullTight(before, layer, halfWidth, new int[] {net}, cl, (Set<Pin>) null);
      System.out.println(
          "row "
              + i
              + " layer="
              + layer
              + " hw="
              + halfWidth
              + " net="
              + net
              + " cl="
              + cl
              + " mtd="
              + mtd
              + " same="
              + (after == before));
      System.out.println("  in  " + poly(before));
      System.out.println("  out " + poly(after));
    }
  }

  /** Unused, but keeps the `Collection` import honest for future modes. */
  static int contactCount(Collection<Item> items) {
    return items.size();
  }
}
