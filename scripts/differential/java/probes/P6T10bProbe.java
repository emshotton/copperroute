// Plan 6 Task 10b ground-truth probe: the **via-insertion chain** —
// `board.actions.ForcedPadRouter.forcedPad` (:346-465), `board.optimize.TraceShover.insert`
// (:417-591), `board.actions.DrillItemMover.insert` (:110-167) / `.shoveVias` (:173-249) and
// `board.actions.ForcedViaInserter.insert` (:249-356) — on the clone's HEAD jar. It is not a
// differential driver (there is no Rust twin and `run.sh` does not know it), but every literal
// in the Task-10b half of `crates/fr-router/tests/forced_via.rs` is read off its stdout, so it
// is committed here to keep those numbers reproducible.
//
// Unlike `P6T10Probe`, every row here **mutates the board**, so each row rebuilds its board from
// scratch and then dumps the whole item list in `getItems()` order (descending id, quirk #63)
// together with `communication.idGenerator.maxGeneratedId()` — which is what pins the item-id
// burn order and the board-state parity plan-6 Task 10b's controller ruling asks for.
//
// It declares `package app.freerouting.board.actions` so it can call `ForcedPadRouter`'s
// package-private `forcedPad` (:346) and `calcFromSide` (:471) directly, exactly as `P6T10Probe`
// does. Nothing here needs reflection: `TraceShover.insert`, `DrillItemMover.insert`,
// `DrillItemMover.shoveVias` and `ForcedViaInserter.insert` are all public.
//
// It compiles against the clone's HEAD jar exactly as `run.sh`'s jar mode does:
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential/java/probes
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p6t10b P6T10bProbe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p6t10b:$JAR" \
//       app.freerouting.board.actions.P6T10bProbe <mode>
//
// Modes:
//   pad    `ForcedPadRouter.forcedPad` (:346-465) over a grid of pad shapes x nets x
//          copperSharing x recursion depths x `changedArea` on/off, in both angle regimes. The
//          `changedArea` axis is load-bearing: `forcedPad:437-441` guards the null where
//          `TraceShover.insert:572` does not (see mode `trace`).
//   trace  `TraceShover.insert` (:417-591) over the same shape grid plus the spring-over budget.
//          The `changedArea=false` rows are the ones that pin the **swallowed
//          NullPointerException** at `:571-575`: with `board.changedArea == null`,
//          `board.changedArea.getArea(layer)` throws, the `catch (Exception)` eats it, and the
//          substitute trace is inserted **un-normalized**.
//   shove  `DrillItemMover.shoveVias` (:173-249) and `DrillItemMover.insert` (:110-167) on a
//          board carrying a foreign-net via inside the obstacle shape, over the via-recursion
//          budget and both angle regimes.
//   via    `ForcedViaInserter.insert` (:249-356): the mode-`check` grid of `P6T10Probe`, but
//          inserting for real — including the ruling-F row where the new via **splits the traces
//          it crosses** (`BasicBoard.insertVia:287-293` -> `splitTraces` -> `PolylineTrace.split`)
//          and the rows that refuse and leave the board damaged.
//   rand   >= 100 pseudo-random rows for each of the four methods, each printed as
//          `-> <answer> maxId=<n> items=<n> hash=<String.hashCode of the board dump>`, so the
//          Rust twin can compare a whole board state in one integer.
package app.freerouting.board.actions;

import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.DrillItem;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.items.Via;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.model.structure.Layer;
import app.freerouting.board.model.structure.LayerStructure;
import app.freerouting.board.model.structure.ShapeEntrySide;
import app.freerouting.board.optimize.TraceShover;
import app.freerouting.board.state.Communication;
import app.freerouting.board.trace.PolylineTrace;
import app.freerouting.core.library.Package;
import app.freerouting.core.library.Padstack;
import app.freerouting.geometry.planar.ConvexShape;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.IntOctagon;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.IntVector;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import app.freerouting.geometry.planar.PolylineShape;
import app.freerouting.geometry.planar.Shape;
import app.freerouting.geometry.planar.TileShape;
import app.freerouting.geometry.planar.Vector;
import app.freerouting.rules.BoardRules;
import app.freerouting.rules.ClearanceMatrix;
import app.freerouting.rules.ViaInfo;

/** Task 10b ground truth: the via-insertion chain. */
public class P6T10bProbe {

  static final IntBox BOUNDING_BOX = new IntBox(-10000, -10000, 10000, 10000);
  static RoutingBoard board;
  static Padstack thruPad;
  static Padstack smdPad;

  public static void main(String[] args) {
    String mode = args.length > 0 ? args[0] : "pad";
    switch (mode) {
      case "pad" -> forcedPadMode();
      case "trace" -> traceShoverMode();
      case "shove" -> drillItemMoverMode();
      case "via" -> forcedViaMode();
      case "rand" -> randomMode();
      default -> throw new IllegalArgumentException("unknown mode " + mode);
    }
  }

  // =============================================================================================
  // The board — `P6T10Probe.build` verbatim, plus an optional foreign-net via
  // =============================================================================================

  static void build(AngleRestriction angleRestriction) {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    cm.appendClass("wide");
    cm.setValue(2, 1, 600);
    cm.setValue(2, 2, 800);
    BoardRules rules = new BoardRules(ls, cm);
    rules.setTraceAngleRestriction(angleRestriction);
    Communication comm = new Communication();
    board = new RoutingBoard(BOUNDING_BOX, ls, new PolylineShape[0], 0, rules, comm);
    board.library.padstacks = new app.freerouting.core.library.Padstacks(ls);
    board.library.packages = new app.freerouting.core.library.Packages(board.library.padstacks);

    ConvexShape[] smd = {new IntBox(-50, -50, 50, 50), null};
    smdPad = board.library.padstacks.add("smd", smd, false, false);
    ConvexShape[] thru = {
      new IntOctagon(-70, -70, 70, 70, -140, 140, -140, 140),
      new IntOctagon(-70, -70, 70, 70, -140, 140, -140, 140)
    };
    thruPad = board.library.padstacks.add("thru", thru, true, false);

    Package.Pin[] pins = {
      new Package.Pin("P1", smdPad.id, new IntVector(-500, 0), 0),
      new Package.Pin("P2", thruPad.id, new IntVector(500, 0), 0)
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
    board.rules.nets.add("N3", 1, false);

    board.insertPin(1, 0, new int[] {1}, 1, FixedState.UNFIXED);
    board.insertPin(1, 1, new int[] {1}, 1, FixedState.UNFIXED);
    Polyline traceLine =
        new Polyline(
            new Point[] {
              new IntPoint(-500, 0),
              new IntPoint(0, 0),
              new IntPoint(0, 400),
              new IntPoint(500, 400)
            });
    board.insertTraceWithoutCleaning(traceLine, 0, 30, new int[] {1}, 1, FixedState.UNFIXED);
    Polyline other =
        new Polyline(
            new Point[] {new IntPoint(-800, 300), new IntPoint(-800, 900), new IntPoint(300, 900)});
    board.insertTraceWithoutCleaning(other, 0, 40, new int[] {2}, 2, FixedState.UNFIXED);
  }

  /**
   * `build` plus a free, unfixed net-3 via at `viaCenter` — the item every shove path in this
   * probe is trying to push out of the way.
   */
  static Via buildWithVia(AngleRestriction angleRestriction, IntPoint viaCenter) {
    build(angleRestriction);
    return board.insertVia(thruPad, viaCenter, new int[] {3}, 1, FixedState.UNFIXED, false);
  }

  /**
   * A ladder board: quirk #76's minimal repro from Plan 3, four rungs between two rails, on which
   * `PolylineTrace.split`'s entry re-walk and `Item.getConnectionItems`' contact walk both fail to
   * terminate in the port. Used by the Rust `insert_stops_when_the_stop_check_trips` test; the JVM
   * side of it is not printed here because the JVM has no stop check at all.
   */
  static void buildLadder(AngleRestriction angleRestriction) {
    build(angleRestriction);
    for (int i = 0; i < 4; i++) {
      int x = 1000 + i * 200;
      board.insertTraceWithoutCleaning(
          new Polyline(new Point[] {new IntPoint(x, 1000), new IntPoint(x, 1600)}),
          0,
          30,
          new int[] {3},
          1,
          FixedState.UNFIXED);
    }
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(1000, 1000), new IntPoint(1600, 1000)}),
        0,
        30,
        new int[] {3},
        1,
        FixedState.UNFIXED);
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(1000, 1600), new IntPoint(1600, 1600)}),
        0,
        30,
        new int[] {3},
        1,
        FixedState.UNFIXED);
  }

  // =============================================================================================
  // Dumps
  // =============================================================================================

  static String pt(Point p) {
    if (p == null) {
      return "null";
    }
    return "(" + p.toFloat().round().x + "," + p.toFloat().round().y + ")";
  }

  static String nets(int[] arr) {
    if (arr == null) {
      return "null";
    }
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < arr.length; i++) {
      if (i > 0) {
        sb.append(",");
      }
      sb.append(arr[i]);
    }
    return sb.append("]").toString();
  }

  /** One line per item, in `getItems()` order — descending id (quirk #63). */
  static String dump() {
    StringBuilder sb = new StringBuilder();
    for (Item item : board.getItems()) {
      sb.append("    item id=").append(item.getId());
      sb.append(" type=").append(item.getClass().getSimpleName());
      sb.append(" nets=").append(nets(item.netNumbers));
      sb.append(" cl=").append(item.clearanceClassIndex());
      if (item instanceof PolylineTrace trace) {
        sb.append(" layer=").append(trace.getLayer());
        sb.append(" hw=").append(trace.getHalfWidth());
        sb.append(" corners=[");
        for (int i = 0; i < trace.cornerCount(); i++) {
          if (i > 0) {
            sb.append(",");
          }
          sb.append(pt(trace.polyline().corner(i)));
        }
        sb.append("]");
      } else if (item instanceof Via via) {
        sb.append(" padstack=").append(via.getPadstack().name);
        sb.append(" center=").append(pt(via.getCenter()));
        sb.append(" attach=").append(via.attachAllowed);
      } else if (item instanceof Pin pin) {
        sb.append(" padstack=").append(pin.getPadstack().name);
        sb.append(" center=").append(pt(pin.getCenter()));
      }
      sb.append("\n");
    }
    return sb.toString();
  }

  static int itemCount() {
    int n = 0;
    for (Item ignored : board.getItems()) {
      n++;
    }
    return n;
  }

  static int maxId() {
    return board.communication.idGenerator.maxGeneratedId();
  }

  /** The board dump plus its `maxId`, printed under the row that produced it. */
  static void printBoard() {
    System.out.println("    maxId=" + maxId() + " items=" + itemCount());
    System.out.print(dump());
  }

  /** `String.hashCode()` of `maxId|items|dump` — the compact form mode `rand` prints. */
  static int fingerprint() {
    return (maxId() + "|" + itemCount() + "|" + dump()).hashCode();
  }

  static String failing() {
    Item obstacle = board.getShoveFailingObstacle();
    return "failing=" + (obstacle == null ? "null" : String.valueOf(obstacle.getId()));
  }

  // =============================================================================================
  // The shared shape grid
  // =============================================================================================

  /** `(label, centre)` — where each probed pad / trace shape sits. */
  static Object[][] spots() {
    return new Object[][] {
      {"onNet1Trace", new IntPoint(0, 200)},
      {"onNet2Trace", new IntPoint(-800, 900)},
      {"onSmdPin", new IntPoint(-500, 0)},
      {"freeSpace", new IntPoint(2000, 2000)},
      {"offBoard", new IntPoint(9990, 9990)}
    };
  }

  /** A pad shape of `radius` around `centre`, in the angle regime's own family. */
  static TileShape padShape(IntPoint centre, int radius, boolean is90) {
    if (is90) {
      return new IntBox(centre.x - radius, centre.y - radius, centre.x + radius, centre.y + radius);
    }
    return new IntOctagon(
        centre.x - radius,
        centre.y - radius,
        centre.x + radius,
        centre.y + radius,
        centre.x - centre.y - 2 * radius,
        centre.x - centre.y + 2 * radius,
        centre.x + centre.y - 2 * radius,
        centre.x + centre.y + 2 * radius);
  }

  // =============================================================================================
  // mode `pad` — ForcedPadRouter.forcedPad
  // =============================================================================================

  static void forcedPadMode() {
    for (AngleRestriction ar :
        new AngleRestriction[] {AngleRestriction.NONE, AngleRestriction.NINETY_DEGREE}) {
      boolean is90 = ar == AngleRestriction.NINETY_DEGREE;
      System.out.println("mode=pad angle=" + ar);
      for (Object[] spot : spots()) {
        for (int radius : new int[] {60, 250}) {
          for (int[] netArr : new int[][] {new int[] {1}, new int[] {3}}) {
            for (boolean copperSharing : new boolean[] {false, true}) {
              for (int maxRecursionDepth : new int[] {0, 20}) {
                for (boolean withChangedArea : new boolean[] {false, true}) {
                  build(ar);
                  if (withChangedArea) {
                    board.startMarkingChangedArea();
                  }
                  board.setShoveFailingObstacle(null);
                  TileShape shape = padShape((IntPoint) spot[1], radius, is90);
                  ShapeEntrySide fromSide = new ShapeEntrySide((IntPoint) spot[1], shape);
                  boolean ok =
                      new ForcedPadRouter(board)
                          .forcedPad(
                              shape,
                              fromSide,
                              0,
                              netArr,
                              1,
                              copperSharing,
                              null,
                              maxRecursionDepth,
                              5);
                  System.out.println(
                      "  spot="
                          + spot[0]
                          + " r="
                          + radius
                          + " nets="
                          + nets(netArr)
                          + " share="
                          + copperSharing
                          + " maxRec="
                          + maxRecursionDepth
                          + " changedArea="
                          + withChangedArea
                          + " -> "
                          + ok
                          + " "
                          + failing());
                  printBoard();
                }
              }
            }
          }
        }
      }
    }
    // The two early arms: an empty pad shape (`:355-358` -> true, board untouched) and a pad that
    // is not contained in the bounding box (`:359-362` -> false, `shoveFailingObstacle` = the
    // outline, which is null on a board built without one).
    build(AngleRestriction.NONE);
    board.setShoveFailingObstacle(null);
    TileShape empty = new IntBox(100, 100, 0, 0);
    System.out.println(
        "  emptyShape -> "
            + new ForcedPadRouter(board)
                .forcedPad(empty, ShapeEntrySide.NOT_CALCULATED, 0, new int[] {1}, 1, false, null, 20, 5)
            + " "
            + failing());
    printBoard();
    build(AngleRestriction.NONE);
    board.setShoveFailingObstacle(null);
    TileShape huge = new IntBox(-20000, -20000, 20000, 20000);
    System.out.println(
        "  outsideBoundingBox -> "
            + new ForcedPadRouter(board)
                .forcedPad(huge, ShapeEntrySide.NOT_CALCULATED, 0, new int[] {1}, 1, false, null, 20, 5)
            + " "
            + failing());
    printBoard();
  }

  // =============================================================================================
  // mode `trace` — TraceShover.insert
  // =============================================================================================

  static void traceShoverMode() {
    for (AngleRestriction ar :
        new AngleRestriction[] {AngleRestriction.NONE, AngleRestriction.NINETY_DEGREE}) {
      boolean is90 = ar == AngleRestriction.NINETY_DEGREE;
      System.out.println("mode=trace angle=" + ar);
      for (Object[] spot : spots()) {
        for (int radius : new int[] {60, 250}) {
          for (int[] netArr : new int[][] {new int[] {1}, new int[] {3}}) {
            for (int maxRecursionDepth : new int[] {0, 20}) {
              for (int springOver : new int[] {0, 3}) {
                for (boolean withChangedArea : new boolean[] {false, true}) {
                  build(ar);
                  if (withChangedArea) {
                    board.startMarkingChangedArea();
                  }
                  board.setShoveFailingObstacle(null);
                  TileShape shape = padShape((IntPoint) spot[1], radius, is90);
                  ShapeEntrySide fromSide = new ShapeEntrySide((IntPoint) spot[1], shape);
                  boolean ok =
                      new TraceShover(board)
                          .insert(
                              shape, fromSide, 0, netArr, 1, null, maxRecursionDepth, 5, springOver);
                  System.out.println(
                      "  spot="
                          + spot[0]
                          + " r="
                          + radius
                          + " nets="
                          + nets(netArr)
                          + " maxRec="
                          + maxRecursionDepth
                          + " spring="
                          + springOver
                          + " changedArea="
                          + withChangedArea
                          + " -> "
                          + ok
                          + " "
                          + failing());
                  printBoard();
                }
              }
            }
          }
        }
      }
    }
    // The two early arms, as in mode `pad`.
    build(AngleRestriction.NONE);
    board.setShoveFailingObstacle(null);
    System.out.println(
        "  emptyShape -> "
            + new TraceShover(board)
                .insert(
                    new IntBox(100, 100, 0, 0),
                    ShapeEntrySide.NOT_CALCULATED,
                    0,
                    new int[] {1},
                    1,
                    null,
                    20,
                    5,
                    0)
            + " "
            + failing());
    printBoard();
    build(AngleRestriction.NONE);
    board.setShoveFailingObstacle(null);
    System.out.println(
        "  outsideBoundingBox -> "
            + new TraceShover(board)
                .insert(
                    new IntBox(-20000, -20000, 20000, 20000),
                    ShapeEntrySide.NOT_CALCULATED,
                    0,
                    new int[] {1},
                    1,
                    null,
                    20,
                    5,
                    0)
            + " "
            + failing());
    printBoard();
  }

  // =============================================================================================
  // mode `shove` — DrillItemMover.shoveVias and DrillItemMover.insert
  // =============================================================================================

  static void drillItemMoverMode() {
    for (AngleRestriction ar :
        new AngleRestriction[] {AngleRestriction.NONE, AngleRestriction.NINETY_DEGREE}) {
      boolean is90 = ar == AngleRestriction.NINETY_DEGREE;
      System.out.println("mode=shove angle=" + ar);
      Object[][] viaSpots = {
        {"freeSpace", new IntPoint(2000, 2000)},
        {"nearNet1Trace", new IntPoint(0, 250)},
        {"nearNet2Trace", new IntPoint(-500, 900)}
      };
      for (Object[] viaSpot : viaSpots) {
        IntPoint viaCentre = (IntPoint) viaSpot[1];
        for (int radius : new int[] {80, 300}) {
          for (int[] netArr : new int[][] {new int[] {1}, new int[] {3}}) {
            for (int maxViaRecursionDepth : new int[] {0, 1, 5}) {
              for (boolean copperSharing : new boolean[] {false, true}) {
                buildWithVia(ar, viaCentre);
                board.startMarkingChangedArea();
                board.setShoveFailingObstacle(null);
                TileShape shape = padShape(viaCentre, radius, is90);
                ShapeEntrySide fromSide = new ShapeEntrySide(viaCentre, shape);
                boolean ok =
                    DrillItemMover.shoveVias(
                        shape,
                        fromSide,
                        0,
                        netArr,
                        1,
                        null,
                        20,
                        maxViaRecursionDepth,
                        copperSharing,
                        board);
                System.out.println(
                    "  shoveVias via="
                        + viaSpot[0]
                        + " r="
                        + radius
                        + " nets="
                        + nets(netArr)
                        + " maxViaRec="
                        + maxViaRecursionDepth
                        + " share="
                        + copperSharing
                        + " -> "
                        + ok
                        + " "
                        + failing());
                printBoard();
              }
            }
          }
        }
      }
      // `DrillItemMover.insert` on its own: translate the free via by a handful of vectors.
      int[][] deltas = {{0, 0}, {300, 0}, {-300, 0}, {0, 700}, {4000, 4000}, {20000, 0}};
      for (Object[] viaSpot : viaSpots) {
        for (int[] delta : deltas) {
          for (int maxRecursionDepth : new int[] {0, 20}) {
            Via via = buildWithVia(ar, (IntPoint) viaSpot[1]);
            board.startMarkingChangedArea();
            board.setShoveFailingObstacle(null);
            boolean ok =
                DrillItemMover.insert(
                    via, new IntVector(delta[0], delta[1]), maxRecursionDepth, 5, null, board);
            System.out.println(
                "  insert via="
                    + viaSpot[0]
                    + " delta=("
                    + delta[0]
                    + ","
                    + delta[1]
                    + ") maxRec="
                    + maxRecursionDepth
                    + " -> "
                    + ok
                    + " "
                    + failing());
            printBoard();
          }
        }
      }
      // `:117-119` — a shove-fixed drill item refuses without touching the board.
      Via fixedVia = buildWithVia(ar, new IntPoint(2000, 2000));
      board.setShoveFailingObstacle(null);
      Via userFixed =
          board.insertVia(thruPad, new IntPoint(3000, 3000), new int[] {3}, 1, FixedState.USER_FIXED, false);
      System.out.println(
          "  insert shoveFixed -> "
              + DrillItemMover.insert(userFixed, new IntVector(300, 0), 20, 5, null, board)
              + " "
              + failing()
              + " unfixedControl="
              + DrillItemMover.insert(fixedVia, new IntVector(0, 0), 20, 5, null, board));
      printBoard();
    }
  }

  // =============================================================================================
  // mode `via` — ForcedViaInserter.insert
  // =============================================================================================

  static void forcedViaMode() {
    for (AngleRestriction ar :
        new AngleRestriction[] {AngleRestriction.NONE, AngleRestriction.NINETY_DEGREE}) {
      System.out.println("mode=via angle=" + ar);
      Object[][] spots = {
        {"onNet1Trace", new IntPoint(0, 200)},
        {"crossesNet1Trace", new IntPoint(0, 400)},
        {"onNet2Trace", new IntPoint(-800, 900)},
        {"onSmdPin", new IntPoint(-500, 0)},
        {"onThruPin", new IntPoint(500, 0)},
        {"freeSpace", new IntPoint(2000, 2000)},
        {"offBoard", new IntPoint(9990, 9990)}
      };
      for (int holeClearance : new int[] {0, 300}) {
        for (Object[] spot : spots) {
          for (boolean attachSmd : new boolean[] {false, true}) {
            for (int[] netArr : new int[][] {new int[] {1}, new int[] {3}}) {
              for (int[] pen : new int[][] {{0, 0}, {30, 30}, {400, 400}}) {
                build(ar);
                board.rules.setHoleClearance(holeClearance);
                board.startMarkingChangedArea();
                board.setShoveFailingLayer(-1);
                board.setShoveFailingObstacle(null);
                ViaInfo info = new ViaInfo("v", thruPad, 1, attachSmd, board.rules);
                boolean ok =
                    ForcedViaInserter.insert(
                        info, (Point) spot[1], netArr, 1, pen, 20, 5, board);
                System.out.println(
                    "  hc="
                        + holeClearance
                        + " spot="
                        + spot[0]
                        + " attachSmd="
                        + attachSmd
                        + " nets="
                        + nets(netArr)
                        + " pen="
                        + nets(pen)
                        + " -> "
                        + ok
                        + " failingLayer="
                        + board.getShoveFailingLayer()
                        + " "
                        + failing());
                printBoard();
              }
            }
          }
        }
      }
    }
  }

  // =============================================================================================
  // mode `rand`
  // =============================================================================================

  static long seed;

  static long next() {
    seed = seed * 6364136223846793005L + 1442695040888963407L;
    return seed >>> 33;
  }

  static int nextInt(int bound) {
    return (int) (next() % bound);
  }

  static void randomMode() {
    randomBlock("forcedPad", 0);
    randomBlock("traceShoverInsert", 1);
    randomBlock("shoveVias", 2);
    randomBlock("drillItemMoverInsert", 3);
    randomBlock("forcedViaInsert", 4);
  }

  static void randomBlock(String label, int which) {
    seed = 20261111L + which * 7919L;
    System.out.println("mode=rand block=" + label);
    for (int i = 0; i < 120; i++) {
      AngleRestriction ar =
          nextInt(2) == 0 ? AngleRestriction.NONE : AngleRestriction.NINETY_DEGREE;
      boolean is90 = ar == AngleRestriction.NINETY_DEGREE;
      int x = nextInt(4000) - 2000;
      int y = nextInt(4000) - 2000;
      int radius = nextInt(400) + 40;
      int netNo = nextInt(3) + 1;
      int maxRecursionDepth = nextInt(4) * 7;
      int maxViaRecursionDepth = nextInt(4);
      int springOver = nextInt(3);
      boolean copperSharing = nextInt(2) == 0;
      boolean withChangedArea = nextInt(2) == 0;
      // The free via is placed **near the probed shape**, not independently of it: the first
      // draft drew it uniformly over the board and the `shoveVias` block then answered `true`
      // with an untouched board on all 120 rows, because a via that does not overlap the shape
      // is never a candidate. A 0-diffs table over a degenerate board is not a test.
      int viaX = x + nextInt(700) - 350;
      int viaY = y + nextInt(700) - 350;
      // Block 2 (`shoveVias`) narrows three of the draws, because a row that skips the shove is
      // a no-op row and 120 of those prove nothing: the via goes **inside** the shape, never on
      // the shape's own net (`:203-205` would `continue`) and always with a via budget (`:206-208`
      // would return early). All three skip arms are covered by the deterministic `shove` mode
      // instead, where they are the point of the row rather than an accident of the draw.
      if (which == 2) {
        viaX = x + (viaX - x) / 3;
        viaY = y + (viaY - y) / 3;
        netNo = (netNo % 2) + 1;
        maxViaRecursionDepth = maxViaRecursionDepth + 1;
      }
      IntPoint centre = new IntPoint(x, y);
      IntPoint viaCentre = new IntPoint(viaX, viaY);
      Via via = buildWithVia(ar, viaCentre);
      if (withChangedArea) {
        board.startMarkingChangedArea();
      }
      board.setShoveFailingObstacle(null);
      board.setShoveFailingLayer(-1);
      TileShape shape = padShape(centre, radius, is90);
      ShapeEntrySide fromSide = new ShapeEntrySide(centre, shape);
      int[] netArr = {netNo};
      String answer;
      switch (which) {
        case 0 ->
            answer =
                String.valueOf(
                    new ForcedPadRouter(board)
                        .forcedPad(
                            shape,
                            fromSide,
                            0,
                            netArr,
                            1,
                            copperSharing,
                            null,
                            maxRecursionDepth,
                            maxViaRecursionDepth));
        case 1 ->
            answer =
                String.valueOf(
                    new TraceShover(board)
                        .insert(
                            shape,
                            fromSide,
                            0,
                            netArr,
                            1,
                            null,
                            maxRecursionDepth,
                            maxViaRecursionDepth,
                            springOver));
        case 2 ->
            answer =
                String.valueOf(
                    DrillItemMover.shoveVias(
                        shape,
                        fromSide,
                        0,
                        netArr,
                        1,
                        null,
                        maxRecursionDepth,
                        maxViaRecursionDepth,
                        copperSharing,
                        board));
        case 3 ->
            answer =
                String.valueOf(
                    DrillItemMover.insert(
                        via,
                        new IntVector(x / 8, y / 8),
                        maxRecursionDepth,
                        maxViaRecursionDepth,
                        null,
                        board));
        default -> {
          ViaInfo info = new ViaInfo("v", thruPad, 1, copperSharing, board.rules);
          answer =
              String.valueOf(
                  ForcedViaInserter.insert(
                      info,
                      centre,
                      netArr,
                      1,
                      new int[] {radius / 4, radius / 4},
                      maxRecursionDepth,
                      maxViaRecursionDepth,
                      board));
        }
      }
      System.out.println(
          "  i="
              + i
              + " angle="
              + (is90 ? "90" : "none")
              + " centre=("
              + x
              + ","
              + y
              + ") r="
              + radius
              + " viaCentre=("
              + viaX
              + ","
              + viaY
              + ") net="
              + netNo
              + " maxRec="
              + maxRecursionDepth
              + " maxViaRec="
              + maxViaRecursionDepth
              + " spring="
              + springOver
              + " share="
              + copperSharing
              + " changedArea="
              + withChangedArea
              + " -> "
              + answer
              + " maxId="
              + maxId()
              + " items="
              + itemCount()
              + " hash="
              + fingerprint()
              + " "
              + failing());
    }
  }
}
