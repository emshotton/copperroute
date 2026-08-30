// Plan 6 Task 15b ground-truth probe (controller ruling AB): `RoutingBoard.insertForcedTracePolyline`
// (`board/facade/RoutingBoard.java:456-876`), `RoutingBoard.insertForcedTraceSegment` (`:361-402`)
// and `board/optimize/TraceShover.springOverObstacles` (`:827-874`).
//
// It is not a differential driver — there is no Rust twin and `run.sh` does not know it — but
// every literal in `crates/fr-router/tests/board_ext.rs`'s Task 15b tests is read off its stdout,
// so it is committed here to keep those numbers reproducible.
//
// It declares `package app.freerouting.board.optimize` so it can reflect into `TraceShover`'s
// private `springOver` (`:611-818`) for the recursion-depth ladder, and it **compiles together
// with `P6T9Probe.java`**, which declares the same package, reusing that probe's
// `build(AngleRestriction)` — the two-layer, two-pin, two-trace board `P6T3.build` /
// `P6T7Probe.build` / `P6T9Probe.build` / `P6T15aProbe` all share — rather than declaring a
// second fixture.
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential/java/probes
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p6t15b P6T9Probe.java P6T15bProbe.java
//   for m in spring ladder poly tail seg neck side rand; do
//     echo "######## $m"
//     "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//         -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p6t15b:$JAR" \
//         app.freerouting.board.optimize.P6T15bProbe "$m" 2>/dev/null \
//       | grep -Ev '^[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9:.]+ +(WARN|INFO|DEBUG|ERROR|TRACE) '
//   done > crates/fr-router/tests/data/p6t15b-insert-forced.txt
//
// The `######## <mode>` banners of the committed transcript come from this loop, not from the
// probe; the `grep` strips `FRLogger`'s timestamped lines (both `insertForcedTracePolyline` and
// `TraceShover.insert` log heavily, and a wall-clock timestamp cannot be committed).
//
// Dump format, identical to `P6T15aProbe`'s so the two transcripts read the same way: a polyline
// prints as its **line array** plus its corner list (an `IntPoint` corner as `(x,y)`, any other
// as `~(<Double.toString x>,<Double.toString y>)` of `cornerApprox`), and every mutating row is
// followed by `maxId=`, `failing=` (the `shoveFailingObstacle` and `shoveFailingLayer` left
// behind) and one line per board item in `getItems()` order (descending id, quirk #63).
//
// **Every row rebuilds its board from scratch**, because every row mutates it.
//
// Modes:
//   spring  `TraceShover.springOverObstacles` (`:827-874`) called directly, over eight polylines
//           x two half widths x three net arrays x `contactPins` null / non-null, in all three
//           angle regimes. The `throughPin` / `throughVia` rows are where it succeeds: a pin and
//           a user-fixed via are both `!isRoutable()`, so `springOver:666-684` finds them and
//           `:713-722`'s `DrillItem` arm hands it a shape to wrap around.
//   ladder  the recursion limit. `springOver`'s `recursionDepth <= 0` arm (`:695-700`) is reached
//           two ways: by reflection at depth 0/1/2/3 on a board with one obstacle, and through
//           the public `springOverObstacles` on a board carrying a **row of foreign-net pins**,
//           where the hard-coded `maxSpringOverRecursionDepth = 20` of `:834` runs out.
//   poly    `insertForcedTracePolyline` over the fourteen cases of `cases()` x three regimes x
//           `maxRecursionDepth` 0/20 x `withCheck` x `tidyWidth` 0/MAX_VALUE (336 rows).
//   tail    the `tidyWidth > 0` pull-tight tail at `:860-862`: the same insertion run with
//           `tidyWidth = 0` and with `tidyWidth = Integer.MAX_VALUE`, so the rows where the tail
//           changes the inserted polyline are visible side by side. Also `pullTightAccuracy`
//           0/100/500/5000 and the `maxRecursionDepth <= 0` arm of `:777-782`, which is the only
//           way `optNetNoArr` is non-empty and `PolylineTrace.pullTight:821`'s filter fires.
//   seg     `insertForcedTraceSegment` (`:361-402`) over the same endpoint pairs, which is where
//           `:393-400`'s three-way identity test on the returned corner shows.
//   neck    the neck-down shape `FoundConnectionInserter.tryNeckDown:473-491` and
//           `insertFanoutMicroNeckdown:596-675` drive: the same segment at a ladder of half
//           widths below the base one, next to a pin, with `tidyWidth = Integer.MAX_VALUE` and
//           `withCheck = true` exactly as those five call sites pass them.
//   side    the `ShapeEntrySide` index `insertForcedTracePolyline:567-571` computes for shove shape
//           `i`, beside the one `checkForcedTracePolyline:429` computes for the same shape, and
//           the `ShapeEntrySide.no` each of the two produces. They disagree by one.
//   rand    128 randomised `insertForcedTracePolyline` calls and 128 randomised
//           `insertForcedTraceSegment` calls from one `java.util.Random(4242)` stream, replayed
//           on the Rust side with `fr_geometry::JavaRandom`. Each row prints its answer, `maxId`,
//           the item count and `String.hashCode` of the whole board dump, so a board compares as
//           one integer.
package app.freerouting.board.optimize;

import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.model.structure.ShapeEntrySide;
import app.freerouting.board.trace.PolylineTrace;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.Line;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import app.freerouting.geometry.planar.TileShape;
import java.lang.reflect.Method;
import java.util.Random;
import java.util.Set;

/** Task 15b ground truth: insertForcedTracePolyline / insertForcedTraceSegment / springOverObstacles. */
public class P6T15bProbe {

  static final int MAXV = Integer.MAX_VALUE;
  static final long RANDOM_SEED = 4242L;

  public static void main(String[] args) throws Exception {
    String mode = args.length > 0 ? args[0] : "poly";
    switch (mode) {
      case "spring" -> springTable();
      case "ladder" -> ladderTable();
      case "poly" -> polyTable();
      case "tail" -> tailTable();
      case "seg" -> segTable();
      case "neck" -> neckTable();
      case "side" -> sideTable();
      case "rand" -> randomTable();
      default -> throw new IllegalArgumentException("mode");
    }
  }

  // --- dumps (identical to P6T15aProbe's) -------------------------------------------------------

  static String ln(Line l) {
    return "("
        + ((IntPoint) l.a).x
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

  /** A `Point` answer, exactly: `null`, an `IntPoint`, or a rational one through `cornerApprox`. */
  static String ptOf(Point p) {
    if (p == null) {
      return "null";
    }
    if (p instanceof IntPoint ip) {
      return "(" + ip.x + "," + ip.y + ")";
    }
    FloatPoint f = p.toFloat();
    return "~(" + Double.toString(f.x) + "," + Double.toString(f.y) + ")";
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

  /** `shoveFailingObstacle` + `shoveFailingLayer`, exactly as the board left them. */
  static String failing(RoutingBoard board) {
    Item obstacle = board.getShoveFailingObstacle();
    String id =
        obstacle == null
            ? "null"
            : obstacle.getClass().getSimpleName() + "#" + obstacle.getId();
    return "failing=" + id + " failingLayer=" + board.getShoveFailingLayer();
  }

  /** One line per item, in `getItems()` order (descending id, quirk #63). */
  static String boardDump(RoutingBoard board) {
    StringBuilder out = new StringBuilder();
    out.append("    maxId=").append(board.communication.idGenerator.maxGeneratedId());
    for (Item item : board.getItems()) {
      StringBuilder sb = new StringBuilder();
      sb.append("\n    item id=")
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
      out.append(sb);
    }
    return out.toString();
  }

  static void dump(RoutingBoard board) {
    System.out.println(boardDump(board));
  }

  /**
   * Whether `result` carries the same **value** as `original` — the port answers `Option<Polyline>`
   * and cannot express Java's reference identity, so every `same=` column is printed beside a
   * `sameVal=` one and the two are asserted to agree on every row.
   */
  static boolean sameValue(Polyline result, Polyline original) {
    return result != null && poly(result).equals(poly(original));
  }

  static String regimeName(AngleRestriction r) {
    return switch (r) {
      case NINETY_DEGREE -> "90";
      case FORTYFIVE_DEGREE -> "45";
      default -> "any";
    };
  }

  // --- the boards -------------------------------------------------------------------------------

  static IntPoint p(int x, int y) {
    return new IntPoint(x, y);
  }

  /** `P6T9Probe.build`, verbatim. */
  static RoutingBoard base(AngleRestriction restriction) {
    P6T9Probe.build(restriction);
    return P6T9Probe.board;
  }

  /**
   * `P6T9Probe.build` plus a **user-fixed** net-3 via at (2400, 2000) and a shove-fixed net-2
   * trace at y = -900. Both are things `springOver` treats as obstacles rather than as shovable
   * copper: a user-fixed via is `!isRoutable()` (Via.java:145-148) and a shove-fixed trace with
   * no own-net contact is the `:645-655` arm.
   */
  static RoutingBoard obstacles(AngleRestriction restriction) {
    RoutingBoard board = base(restriction);
    board.insertVia(P6T9Probe.thruPad, p(2400, 2000), new int[] {3}, 1, FixedState.USER_FIXED, false);
    Polyline fixedTrace =
        new Polyline(new Point[] {p(-1500, -900), p(1500, -900)});
    board.insertTraceWithoutCleaning(fixedTrace, 0, 30, new int[] {2}, 1, FixedState.SHOVE_FIXED);
    return board;
  }

  /** The empty lane the recursion ladder is drawn along: nothing else on this board is near it. */
  static final int LADDER_Y = -3000;

  /**
   * A row of `count` **user-fixed** net-1 vias along `LADDER_Y`, spaced 400 apart from x = -2000.
   * Each is `!isRoutable()` (Via.java:145-148), and 400 apart their bounding boxes (140 wide) do
   * not meet, so `springOver:670-682`'s "two overlapping obstacles" arm is out of the way and the
   * only budget that can run out is `springOverObstacles:834`'s 20.
   */
  static RoutingBoard ladderBoard(int count) {
    RoutingBoard board = base(AngleRestriction.NONE);
    for (int i = 0; i < count; i++) {
      board.insertVia(
          P6T9Probe.thruPad,
          p(-2000 + 400 * i, LADDER_Y),
          new int[] {1},
          1,
          FixedState.USER_FIXED,
          false);
    }
    return board;
  }

  /**
   * Two user-fixed net-1 vias `gap` apart on `LADDER_Y`. At `gap = 100` their 140-wide bounding
   * boxes intersect and neither contains the other, which is `springOver:679-681`'s bare
   * `return null` — **the one refusal that leaves `shoveFailingObstacle` untouched**.
   */
  static RoutingBoard overlapBoard(int gap) {
    RoutingBoard board = base(AngleRestriction.NONE);
    board.insertVia(P6T9Probe.thruPad, p(0, LADDER_Y), new int[] {1}, 1, FixedState.USER_FIXED, false);
    board.insertVia(
        P6T9Probe.thruPad, p(gap, LADDER_Y), new int[] {1}, 1, FixedState.USER_FIXED, false);
    return board;
  }

  // --- the polyline table -----------------------------------------------------------------------

  record Case(String name, Point[] corners, int layer, int halfWidth, int[] nets, int cl) {}

  /**
   * Nine insertion cases. `free` never meets anything; `acrossNet1` / `intoNet1` / `acrossNet2`
   * cross a shovable foreign-net trace; `throughPin` runs straight through the through-pin, which
   * is where `springOverObstacles` has to work; `ownNetEnd` starts exactly at the net-1 trace's
   * **last** corner, which is the only spot on this board where `pickItems` answers a single
   * trace and `:508-517`'s combine path runs; `twoSegments` is a three-corner polyline, so
   * `traceShapes.length` is 2 and the shove loop runs twice; `offBoard` leaves the bounding box
   * and `degenerate` has equal end corners.
   */
  static Case[] cases() {
    return new Case[] {
      new Case("free", new Point[] {p(2000, 2000), p(3000, 2000)}, 0, 30, new int[] {3}, 1),
      new Case("acrossNet1", new Point[] {p(-300, -600), p(-300, 600)}, 0, 30, new int[] {3}, 1),
      new Case("intoNet1", new Point[] {p(-300, -600), p(-300, 0)}, 0, 30, new int[] {3}, 1),
      new Case("acrossNet2", new Point[] {p(-1200, 600), p(-400, 600)}, 0, 30, new int[] {3}, 1),
      new Case("throughPin", new Point[] {p(500, -600), p(500, 600)}, 0, 30, new int[] {3}, 1),
      new Case(
          "twoSegments",
          new Point[] {p(-1200, 600), p(-300, 600), p(-300, -600)},
          0,
          30,
          new int[] {3},
          1),
      new Case("ownNetEnd", new Point[] {p(500, 400), p(900, 400)}, 0, 30, new int[] {1}, 1),
      new Case("offBoard", new Point[] {p(9800, 9800), p(12000, 9800)}, 0, 30, new int[] {3}, 1),
      new Case("degenerate", new Point[] {p(2000, 2000), p(2000, 2000)}, 0, 30, new int[] {3}, 1),
      // The three cases whose **end corner** lands on another trace, so
      // `insertForcedTracePolyline:826-833`'s `pickItems(newCorner, layer).iterator().next()`
      // has more than one candidate to choose between. Java's `Set<Item>` there is a `TreeSet`
      // ordered by `Item.compareTo` (Item.java:95-103, `other.id - this.id`), i.e. **descending**
      // id, so `next()` is the item with the highest id.
      new Case("ownNetTee", new Point[] {p(300, -600), p(300, 400)}, 0, 30, new int[] {1}, 1),
      new Case("ownNetCross", new Point[] {p(300, -600), p(300, 600)}, 0, 30, new int[] {1}, 1),
      new Case("foreignTee", new Point[] {p(-1400, 600), p(-800, 600)}, 0, 30, new int[] {3}, 1),
      // An acute turn in free space, where the segment **before** the shove line runs back into
      // the shove shape — the shape of polyline `:567-571`'s index-below-the-line can answer a
      // different entry side from `checkForcedTracePolyline:429`'s.
      new Case(
          "sharpTurn", new Point[] {p(-1000, -800), p(200, -800), p(0, -700)}, 0, 30,
          new int[] {3}, 1),
      // The one case that reaches `:806-834` with **more than one** trace at `newCorner`: a net-2
      // segment whose end corner is in the middle of the existing net-2 trace, so `normalize`
      // splits that trace in two and `pickItems(newCorner, layer)` then answers three traces.
      // `:829`'s `iterator().next()` picks the highest id of the three (`Item.compareTo`,
      // Item.java:95-103), and `:860-862` pull-tightens exactly that one.
      new Case(
          "net2Tee", new Point[] {p(-1400, 600), p(-800, 600)}, 0, 40, new int[] {2}, 2),
    };
  }

  static final AngleRestriction[] REGIMES = {
    AngleRestriction.NINETY_DEGREE, AngleRestriction.FORTYFIVE_DEGREE, AngleRestriction.NONE
  };

  // --- mode `spring` ----------------------------------------------------------------------------

  /** The polylines `springOverObstacles` is probed with, on the `obstacles` board. */
  static Case[] springCases() {
    return new Case[] {
      new Case("free", new Point[] {p(4000, 4000), p(5000, 4000)}, 0, 30, new int[] {3}, 1),
      new Case("throughPin", new Point[] {p(500, -600), p(500, 600)}, 0, 30, new int[] {3}, 1),
      new Case("throughSmd", new Point[] {p(-500, -600), p(-500, 600)}, 0, 30, new int[] {3}, 1),
      new Case("throughVia", new Point[] {p(2400, 1400), p(2400, 2600)}, 0, 30, new int[] {1}, 1),
      new Case("acrossFixed", new Point[] {p(0, -1400), p(0, -400)}, 0, 30, new int[] {3}, 1),
      new Case("acrossNet1", new Point[] {p(-300, -600), p(-300, 600)}, 0, 30, new int[] {3}, 1),
      new Case(
          "twoObstacles", new Point[] {p(-900, -600), p(900, -600), p(900, 600)}, 0, 30,
          new int[] {3}, 1),
      new Case("offBoard", new Point[] {p(9800, 9800), p(12000, 9800)}, 0, 30, new int[] {3}, 1),
    };
  }

  static void springTable() {
    for (AngleRestriction restriction : REGIMES) {
      for (Case c : springCases()) {
        for (int halfWidth : new int[] {c.halfWidth(), 100}) {
          for (int[] netArr : new int[][] {c.nets(), new int[] {1}, new int[0]}) {
            for (boolean withContactPins : new boolean[] {false, true}) {
              RoutingBoard board = obstacles(restriction);
              Set<Pin> contactPins = null;
              if (withContactPins) {
                contactPins = new java.util.TreeSet<>();
                for (Item item : board.getItems()) {
                  if (item instanceof Pin pin) {
                    contactPins.add(pin);
                  }
                }
              }
              Polyline polyline = new Polyline(c.corners());
              TraceShover algo = new TraceShover(board);
              Polyline result =
                  algo.springOverObstacles(
                      polyline, halfWidth, c.layer(), netArr, c.cl(), contactPins);
              System.out.println(
                  "  regime="
                      + regimeName(restriction)
                      + " case="
                      + c.name()
                      + " hw="
                      + halfWidth
                      + " nets="
                      + nets(netArr)
                      + " contactPins="
                      + (contactPins == null ? "null" : Integer.toString(contactPins.size()))
                      + " same="
                      + (result == polyline)
                      + " sameVal="
                      + sameValue(result, polyline)
                      + " -> "
                      + poly(result)
                      + " "
                      + failing(board));
            }
          }
        }
      }
    }
  }

  // --- mode `ladder` ----------------------------------------------------------------------------

  static void ladderTable() throws Exception {
    // The private `springOver` at an explicit recursion depth: `:695-700`'s refusal arm.
    Method springOver =
        TraceShover.class.getDeclaredMethod(
            "springOver",
            Polyline.class,
            int.class,
            int.class,
            int[].class,
            int.class,
            boolean.class,
            int.class,
            Set.class);
    springOver.setAccessible(true);
    Polyline throughPin = new Polyline(new Point[] {p(500, -600), p(500, 600)});
    for (int depth : new int[] {0, 1, 2, 3, 20}) {
      for (boolean overConnectedPins : new boolean[] {false, true}) {
        RoutingBoard board = obstacles(AngleRestriction.NONE);
        TraceShover algo = new TraceShover(board);
        Polyline result =
            (Polyline)
                springOver.invoke(
                    algo, throughPin, 30, 0, new int[] {3}, 1, overConnectedPins, depth, null);
        System.out.println(
            "  springOver depth="
                + depth
                + " overConnectedPins="
                + overConnectedPins
                + " same="
                + (result == throughPin)
                + " sameVal="
                + sameValue(result, throughPin)
                + " -> "
                + poly(result)
                + " "
                + failing(board));
      }
    }
    // And through the public entry point, on a board with a row of vias in the way: the budget of
    // `:834` is 20, so somewhere along this ladder `springOverObstacles` stops answering.
    for (int viaCount : new int[] {1, 2, 3, 4, 5, 6, 8, 10, 15, 18, 19, 20, 21, 22, 25}) {
      RoutingBoard board = ladderBoard(viaCount);
      Polyline along =
          new Polyline(
              new Point[] {p(-2600, LADDER_Y), p(-2000 + 400 * viaCount + 600, LADDER_Y)});
      TraceShover algo = new TraceShover(board);
      Polyline result = algo.springOverObstacles(along, 30, 0, new int[] {3}, 1, null);
      System.out.println(
          "  ladder vias="
              + viaCount
              + " same="
              + (result == along)
              + " sameVal="
              + sameValue(result, along)
              + " lines="
              + (result == null ? -1 : result.lines.length)
              + " len="
              + (result == null ? "null" : Long.toString(Math.round(result.lengthApprox())))
              + " "
              + failing(board));
    }
    // `springOver:679-681` — the bare `return null` that leaves `shoveFailingObstacle` alone.
    for (int gap : new int[] {100, 140, 200, 400}) {
      RoutingBoard board = overlapBoard(gap);
      Polyline across =
          new Polyline(new Point[] {p(-800, LADDER_Y), p(800, LADDER_Y)});
      TraceShover algo = new TraceShover(board);
      Polyline result = algo.springOverObstacles(across, 30, 0, new int[] {3}, 1, null);
      System.out.println(
          "  overlap gap="
              + gap
              + " same="
              + (result == across)
              + " sameVal="
              + sameValue(result, across)
              + " lines="
              + (result == null ? -1 : result.lines.length)
              + " "
              + failing(board));
    }
  }

  // --- mode `poly` ------------------------------------------------------------------------------

  static void polyTable() {
    for (AngleRestriction restriction : REGIMES) {
      for (Case c : cases()) {
        for (int maxRec : new int[] {0, 20}) {
          for (boolean withCheck : new boolean[] {false, true}) {
            for (int tidyWidth : new int[] {0, MAXV}) {
              RoutingBoard board = base(restriction);
              Polyline polyline = new Polyline(c.corners());
              Point result =
                  board.insertForcedTracePolyline(
                      polyline,
                      c.halfWidth(),
                      c.layer(),
                      c.nets(),
                      c.cl(),
                      maxRec,
                      maxRec,
                      maxRec,
                      tidyWidth,
                      500,
                      withCheck,
                      null);
              System.out.println(
                  "  regime="
                      + regimeName(restriction)
                      + " case="
                      + c.name()
                      + " maxRec="
                      + maxRec
                      + " withCheck="
                      + withCheck
                      + " tidy="
                      + (tidyWidth == MAXV ? "MAX" : Integer.toString(tidyWidth))
                      + " -> "
                      + ptOf(result)
                      + " first="
                      + (result != null && result == polyline.firstCorner())
                      + " last="
                      + (result != null && result == polyline.lastCorner())
                      + " "
                      + failing(board));
              dump(board);
            }
          }
        }
      }
    }
  }

  // --- mode `tail` ------------------------------------------------------------------------------

  static void tailTable() {
    for (AngleRestriction restriction : REGIMES) {
      for (Case c : cases()) {
        for (int tidyWidth : new int[] {0, 1, 400, MAXV}) {
          for (int accuracy : new int[] {0, 100, 500, 5000}) {
            RoutingBoard board = base(restriction);
            Polyline polyline = new Polyline(c.corners());
            Point result =
                board.insertForcedTracePolyline(
                    polyline, c.halfWidth(), c.layer(), c.nets(), c.cl(), 20, 20, 20, tidyWidth,
                    accuracy, true, null);
            System.out.println(
                "  regime="
                    + regimeName(restriction)
                    + " case="
                    + c.name()
                    + " tidy="
                    + (tidyWidth == MAXV ? "MAX" : Integer.toString(tidyWidth))
                    + " acc="
                    + accuracy
                    + " -> "
                    + ptOf(result)
                    + " "
                    + failing(board));
            dump(board);
          }
        }
        // The `maxRecursionDepth <= 0` arm of `:777-782`, the only one that hands
        // `TraceTightener.getInstance` a **non-empty** `onlyNetNoArr`, so
        // `PolylineTrace.pullTight:821-823`'s own-net filter can refuse. It never refuses the
        // trace just inserted (its nets *are* `optNetNoArr`), but it does refuse the foreign
        // trace `splitTracesAtKeepPoint` + `pickItems(newCorner)` can leave behind at `:826-833`.
        // Both net arrays name a net that exists: `Trace.isShoveFixed` (Trace.java:244-250)
        // dereferences `rules.nets.get(netNumbers[0])` and throws for one that does not.
        for (int[] netArr : new int[][] {c.nets(), new int[] {2}}) {
          RoutingBoard board = base(restriction);
          Polyline polyline = new Polyline(c.corners());
          Point result =
              board.insertForcedTracePolyline(
                  polyline, c.halfWidth(), c.layer(), netArr, c.cl(), 0, 0, 0, MAXV, 500, true,
                  null);
          System.out.println(
              "  regime="
                  + regimeName(restriction)
                  + " case="
                  + c.name()
                  + " optNet="
                  + nets(netArr)
                  + " -> "
                  + ptOf(result)
                  + " "
                  + failing(board));
          dump(board);
        }
      }
    }
  }

  // --- mode `seg` -------------------------------------------------------------------------------

  static void segTable() {
    for (AngleRestriction restriction : REGIMES) {
      for (Case c : cases()) {
        Point from = c.corners()[0];
        Point to = c.corners()[c.corners().length - 1];
        for (int maxRec : new int[] {0, 20}) {
          for (int tidyWidth : new int[] {0, MAXV}) {
            RoutingBoard board = base(restriction);
            Point result =
                board.insertForcedTraceSegment(
                    from, to, c.halfWidth(), c.layer(), c.nets(), c.cl(), maxRec, maxRec, maxRec,
                    tidyWidth, 500, true, null);
            System.out.println(
                "  regime="
                    + regimeName(restriction)
                    + " case="
                    + c.name()
                    + " maxRec="
                    + maxRec
                    + " tidy="
                    + (tidyWidth == MAXV ? "MAX" : Integer.toString(tidyWidth))
                    + " -> "
                    + ptOf(result)
                    + " isFrom="
                    + (result == from)
                    + " isTo="
                    + (result == to)
                    + " "
                    + failing(board));
            dump(board);
          }
        }
      }
    }
  }

  // --- mode `neck` ------------------------------------------------------------------------------

  /**
   * `FoundConnectionInserter.tryNeckDown:473-491` and `insertFanoutMicroNeckdown:596-675` both
   * pass `tidyWidth = Integer.MAX_VALUE`, `withCheck = true` and `timeLimit = null`, and vary only
   * the half width; the interesting rows are the ones where a **narrower** trace fits where the
   * base one does not.
   */
  static void neckTable() {
    Point[][] pairs = {
      {p(500, -600), p(500, 0)}, // straight into the centre of the through-pin
      {p(500, -600), p(500, 600)}, // straight through it
      {p(-500, -600), p(-500, 0)}, // into the SMD pad
      {p(-300, -600), p(-300, 600)}, // across the net-1 trace
      {p(-800, 1400), p(-800, 600)}, // into the net-2 trace, which is 40 wide
    };
    String[] names = {"intoPin", "throughPin", "intoSmd", "acrossNet1", "intoNet2"};
    for (AngleRestriction restriction : REGIMES) {
      for (int i = 0; i < pairs.length; i++) {
        for (int halfWidth : new int[] {1, 5, 10, 20, 30, 60, 100}) {
          RoutingBoard board = base(restriction);
          Point result =
              board.insertForcedTraceSegment(
                  pairs[i][0], pairs[i][1], halfWidth, 0, new int[] {3}, 1, 20, 20, 20, MAXV, 500,
                  true, null);
          System.out.println(
              "  regime="
                  + regimeName(restriction)
                  + " case="
                  + names[i]
                  + " hw="
                  + halfWidth
                  + " -> "
                  + ptOf(result)
                  + " isTo="
                  + (result == pairs[i][1])
                  + " "
                  + failing(board));
          dump(board);
        }
      }
    }
  }

  // --- mode `side` ------------------------------------------------------------------------------

  /**
   * The `ShapeEntrySide` index `insertForcedTracePolyline:567-571` computes for shove shape `i`,
   * beside the one `checkForcedTracePolyline:429` computes for the **same** shape, and the
   * `ShapeEntrySide.no` each produces. The two expressions disagree by one; this mode is the
   * ground truth for that.
   */
  static void sideTable() {
    for (AngleRestriction restriction : REGIMES) {
      for (Case c : cases()) {
        RoutingBoard board = base(restriction);
        Polyline polyline = new Polyline(c.corners());
        if (polyline.lines.length < 3) {
          System.out.println(
              "  regime=" + regimeName(restriction) + " case=" + c.name() + " empty=true");
          continue;
        }
        int compensatedHalfWidth =
            c.halfWidth()
                + board
                    .searchTreeManager
                    .getDefaultTree()
                    .clearanceCompensationValue(c.cl(), c.layer());
        // No picked trace on this board unless the polyline starts on one, so `startShapeNo` is
        // whatever `insertForcedTracePolyline:554` computes for `combinedPolyline == newPolyline`.
        Polyline combined = polyline;
        int startShapeNo = 0;
        TileShape[] traceShapes =
            combined.offsetShapes(compensatedHalfWidth, startShapeNo, combined.lines.length - 1);
        boolean orthogonalMode =
            board.rules.getTraceAngleRestriction() == AngleRestriction.NINETY_DEGREE;
        for (int i = 0; i < traceShapes.length; i++) {
          TileShape shape = traceShapes[i];
          if (orthogonalMode) {
            shape = shape.boundingBox();
          }
          int insertIndex = combined.cornerCount() - traceShapes.length - 1 + i;
          int checkIndex = i + 1;
          ShapeEntrySide insertSide = new ShapeEntrySide(combined, insertIndex, shape);
          ShapeEntrySide checkSide = new ShapeEntrySide(combined, checkIndex, shape);
          System.out.println(
              "  regime="
                  + regimeName(restriction)
                  + " case="
                  + c.name()
                  + " shape="
                  + i
                  + " insertIndex="
                  + insertIndex
                  + " insertNo="
                  + insertSide.no
                  + " checkIndex="
                  + checkIndex
                  + " checkNo="
                  + checkSide.no
                  + " agree="
                  + (insertSide.no == checkSide.no));
        }
      }
    }
  }

  // --- mode `rand` ------------------------------------------------------------------------------

  static void randomTable() {
    Random rnd = new Random(RANDOM_SEED);
    System.out.println("  block=polyline");
    for (int row = 0; row < 128; row++) {
      int regimeNo = rnd.nextInt(3);
      int cornerCount = 2 + rnd.nextInt(3);
      Point[] corners = new Point[cornerCount];
      for (int i = 0; i < cornerCount; i++) {
        corners[i] = p(rnd.nextInt(3001) - 1500, rnd.nextInt(3001) - 1500);
      }
      int halfWidth = 10 + rnd.nextInt(60);
      int layer = rnd.nextInt(2);
      int net = 1 + rnd.nextInt(3);
      int cl = 1 + rnd.nextInt(2);
      int maxRec = rnd.nextInt(3) * 10;
      boolean withCheck = rnd.nextInt(2) == 0;
      int tidyWidth = rnd.nextInt(2) == 0 ? 0 : MAXV;
      int accuracy = rnd.nextInt(1000);
      RoutingBoard board = base(REGIMES[regimeNo]);
      Polyline polyline = new Polyline(corners);
      String answer;
      try {
        Point result =
            board.insertForcedTracePolyline(
                polyline, halfWidth, layer, new int[] {net}, cl, maxRec, maxRec, maxRec, tidyWidth,
                accuracy, withCheck, null);
        answer = ptOf(result);
      } catch (Exception | StackOverflowError e) {
        answer = "throw:" + e.getClass().getSimpleName();
      }
      String dump = boardDump(board);
      System.out.println(
          "  row="
              + row
              + " regime="
              + regimeName(REGIMES[regimeNo])
              + " n="
              + cornerCount
              + " hw="
              + halfWidth
              + " layer="
              + layer
              + " net="
              + net
              + " cl="
              + cl
              + " maxRec="
              + maxRec
              + " check="
              + withCheck
              + " tidy="
              + (tidyWidth == MAXV ? "MAX" : "0")
              + " acc="
              + accuracy
              + " -> "
              + answer
              + " maxId="
              + board.communication.idGenerator.maxGeneratedId()
              + " items="
              + board.getItems().size()
              + " hash="
              + dump.hashCode()
              + " "
              + failing(board));
    }
    System.out.println("  block=segment");
    for (int row = 0; row < 128; row++) {
      int regimeNo = rnd.nextInt(3);
      Point from = p(rnd.nextInt(3001) - 1500, rnd.nextInt(3001) - 1500);
      Point to = p(rnd.nextInt(3001) - 1500, rnd.nextInt(3001) - 1500);
      int halfWidth = 10 + rnd.nextInt(60);
      int layer = rnd.nextInt(2);
      int net = 1 + rnd.nextInt(3);
      int cl = 1 + rnd.nextInt(2);
      int maxRec = rnd.nextInt(3) * 10;
      boolean withCheck = rnd.nextInt(2) == 0;
      int tidyWidth = rnd.nextInt(2) == 0 ? 0 : MAXV;
      int accuracy = rnd.nextInt(1000);
      RoutingBoard board = base(REGIMES[regimeNo]);
      String answer;
      boolean isFrom = false;
      boolean isTo = false;
      try {
        Point result =
            board.insertForcedTraceSegment(
                from, to, halfWidth, layer, new int[] {net}, cl, maxRec, maxRec, maxRec, tidyWidth,
                accuracy, withCheck, null);
        answer = ptOf(result);
        isFrom = result == from;
        isTo = result == to;
      } catch (Exception | StackOverflowError e) {
        answer = "throw:" + e.getClass().getSimpleName();
      }
      String dump = boardDump(board);
      System.out.println(
          "  row="
              + row
              + " regime="
              + regimeName(REGIMES[regimeNo])
              + " from="
              + ptOf(from)
              + " to="
              + ptOf(to)
              + " hw="
              + halfWidth
              + " layer="
              + layer
              + " net="
              + net
              + " cl="
              + cl
              + " maxRec="
              + maxRec
              + " check="
              + withCheck
              + " tidy="
              + (tidyWidth == MAXV ? "MAX" : "0")
              + " acc="
              + accuracy
              + " -> "
              + answer
              + " isFrom="
              + isFrom
              + " isTo="
              + isTo
              + " maxId="
              + board.communication.idGenerator.maxGeneratedId()
              + " items="
              + board.getItems().size()
              + " hash="
              + dump.hashCode()
              + " "
              + failing(board));
    }
  }
}
