// Plan 6 Task 10 ground-truth probe: `board.actions.ForcedPadRouter.checkForcedPad` (with its
// private `inFrontOfPad` and its package-private `calcFromSide`) and the three public methods of
// `board.actions.ForcedViaInserter` (`checkLayer`, `check`, `insert`) plus its two private
// helpers `holeCheckShape` and `calculateFromSide`, on the clone's HEAD jar. It is not a
// differential driver — there is no Rust twin and `run.sh` does not know it — but every literal
// in `crates/fr-router/tests/forced_via.rs` is read off its stdout, so it is committed here to
// keep those numbers reproducible.
//
// It declares `package app.freerouting.board.actions` so it can call `ForcedPadRouter`'s
// package-private `calcFromSide` (ForcedPadRouter.java:471-492) directly; the three `private
// static` methods (`inFrontOfPad` :57-212, `ForcedViaInserter.holeCheckShape` :363-375 and
// `ForcedViaInserter.calculateFromSide` :377-461) are reached by reflection, exactly as
// `P6T9Probe` reflects into `RoutingBoard.autorouteEngine`.
//
// It compiles against the clone's HEAD jar exactly as `run.sh`'s jar mode does:
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential/java/probes
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p6t10 P6T10Probe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p6t10:$JAR" \
//       app.freerouting.board.actions.P6T10Probe <mode>
//
// Modes:
//   front  `ForcedPadRouter.inFrontOfPad` (:57-212) over all 8 `fromSide` values x 10 lines x
//          `withSides` in {false, true}, on one octagon — 160 rows. Includes the four rows that
//          pin **the `case 0` typo** at `:78` (`lineB.x + lineB.x`, twice `x`, where every
//          sibling disjunct reads `x + y`).
//   pad    `ForcedPadRouter.checkForcedPad` (:221-340) over a grid of pad shapes x nets x
//          `copperSharingAllowed` x `checkOnlyFront`, in both angle regimes — the method the
//          whole task exists for, and the one that closes Task 9's `DrillItemMover.check` cycle.
//   drill  the row Task 9 could not print: `DrillItemMover.check` on the **shovable** free via,
//          which is the arm that runs the full `checkForcedPad`.
//   side   `ForcedPadRouter.calcFromSide` (:471-492) and `ForcedViaInserter.calculateFromSide`
//          (:377-461), the two "which border do we shove from" calculators.
//   hole   `ForcedViaInserter.holeCheckShape` (:363-375) with the hole clearance on and off.
//   layer  `ForcedViaInserter.checkLayer` (:30-129), including the two-phase
//          `DRILLABLE_WITH_ATTACH_SMD` promotion at `:118-124` and the `NOT_DRILLABLE`
//          short-circuits at `:88-90` and `:117-119`.
//   check  `ForcedViaInserter.check` (:131-247) over a grid of via locations and nets, with the
//          `shoveFailingLayer` each refusal records.
//   rand   200 pseudo-random `(location, layer, net)` triples through `checkLayer` on a
//          **4-layer** board, which is the brief's `0 diffs` table.
package app.freerouting.board.actions;

import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Via;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.model.structure.Layer;
import app.freerouting.board.model.structure.LayerStructure;
import app.freerouting.board.model.structure.ShapeEntrySide;
import app.freerouting.board.state.Communication;
import app.freerouting.core.library.Package;
import app.freerouting.core.library.Padstack;
import app.freerouting.geometry.planar.Circle;
import app.freerouting.geometry.planar.ConvexShape;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.IntOctagon;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.IntVector;
import app.freerouting.geometry.planar.Line;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import app.freerouting.geometry.planar.PolylineShape;
import app.freerouting.geometry.planar.Shape;
import app.freerouting.geometry.planar.Simplex;
import app.freerouting.geometry.planar.TileShape;
import app.freerouting.geometry.planar.Vector;
import app.freerouting.rules.BoardRules;
import app.freerouting.rules.ClearanceMatrix;
import app.freerouting.rules.ViaInfo;
import java.lang.reflect.Method;
import java.util.Collection;
import java.util.LinkedList;

/** Task 10 ground truth: ForcedPadRouter.checkForcedPad and the ForcedViaInserter trio. */
public class P6T10Probe {

  static final IntBox BOUNDING_BOX = new IntBox(-10000, -10000, 10000, 10000);
  static RoutingBoard board;
  static Padstack thruPad;
  static Padstack smdPad;

  public static void main(String[] args) throws Exception {
    String mode = args.length > 0 ? args[0] : "front";
    switch (mode) {
      case "front" -> inFrontOfPad();
      case "pad" -> checkForcedPad();
      case "drill" -> drillItemMover();
      case "side" -> fromSides();
      case "hole" -> holeShape();
      case "layer" -> checkLayer();
      case "check" -> checkVia();
      case "rand" -> randomTable();
      default -> throw new IllegalArgumentException("unknown mode " + mode);
    }
  }

  // =============================================================================================
  // Boards
  // =============================================================================================

  /**
   * `P6T9Probe.build` verbatim — two layers, a 200-unit clearance matrix with a "wide" class, a
   * two-pin component (an SMD pad at (-500, 0) on layer 0 only and a through pad at (500, 0) on
   * both layers) and two traces, one on net 1 and one on net 2.
   */
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

  /** A four-layer board with a grid of traces on every layer, for mode `rand`. */
  static void buildFourLayer() {
    Layer[] layers = {
      new Layer("l0", true), new Layer("l1", true), new Layer("l2", true), new Layer("l3", true)
    };
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    cm.appendClass("wide");
    cm.setValue(2, 1, 600);
    cm.setValue(2, 2, 800);
    BoardRules rules = new BoardRules(ls, cm);
    rules.setTraceAngleRestriction(AngleRestriction.NONE);
    board = new RoutingBoard(BOUNDING_BOX, ls, new PolylineShape[0], 0, rules, new Communication());
    board.library.padstacks = new app.freerouting.core.library.Padstacks(ls);
    board.library.packages = new app.freerouting.core.library.Packages(board.library.padstacks);
    ConvexShape[] thru = {
      new IntOctagon(-70, -70, 70, 70, -140, 140, -140, 140),
      new IntOctagon(-70, -70, 70, 70, -140, 140, -140, 140),
      new IntOctagon(-70, -70, 70, 70, -140, 140, -140, 140),
      new IntOctagon(-70, -70, 70, 70, -140, 140, -140, 140)
    };
    thruPad = board.library.padstacks.add("thru", thru, true, false);
    ConvexShape[] bigPadShapes = {
      new IntOctagon(-200, -200, 200, 200, -400, 400, -400, 400),
      new IntOctagon(-200, -200, 200, 200, -400, 400, -400, 400),
      new IntOctagon(-200, -200, 200, 200, -400, 400, -400, 400),
      new IntOctagon(-200, -200, 200, 200, -400, 400, -400, 400)
    };
    Padstack bigPad = board.library.padstacks.add("big", bigPadShapes, true, false);
    Package.Pin[] pins = {
      new Package.Pin("P1", bigPad.id, new IntVector(0, 0), 0),
      new Package.Pin("P2", bigPad.id, new IntVector(1200, 0), 0),
      new Package.Pin("P3", bigPad.id, new IntVector(0, 1200), 0),
      new Package.Pin("P4", bigPad.id, new IntVector(1200, 1200), 0)
    };
    Package pkg =
        board.library.packages.add(
            "grid",
            pins,
            new Shape[0],
            new double[0],
            new boolean[0],
            new Package.Keepout[0],
            new Package.Keepout[0],
            new Package.Keepout[0],
            true);
    board.rules.nets.add("N1", 1, false);
    board.rules.nets.add("N2", 1, false);
    board.rules.nets.add("N3", 1, false);
    // Four components of four big through pins each — 16 unshovable obstacles spread over the
    // lattice, which is what makes the `rand` table discriminate at all.
    for (int cx = -1; cx <= 0; cx++) {
      for (int cy = -1; cy <= 0; cy++) {
        board.components.add(new IntPoint(cx * 2600 + 400, cy * 2600 + 400), 0, true, pkg);
      }
    }
    for (int pinNo = 1; pinNo <= 16; pinNo++) {
      board.insertPin(pinNo <= 4 ? 1 : (pinNo <= 8 ? 2 : (pinNo <= 12 ? 3 : 4)), (pinNo - 1) % 4,
          new int[] {((pinNo - 1) % 3) + 1}, 1, FixedState.UNFIXED);
    }
    // A 5 x 4 lattice of traces, net (layer % 3) + 1, so every layer has a different pattern.
    for (int layer = 0; layer < 4; layer++) {
      int net = (layer % 3) + 1;
      for (int i = -2; i <= 2; i++) {
        Polyline vertical =
            new Polyline(
                new Point[] {
                  new IntPoint(i * 1500 + layer * 100, -4000),
                  new IntPoint(i * 1500 + layer * 100, 4000)
                });
        board.insertTraceWithoutCleaning(vertical, layer, 40, new int[] {net}, 1, FixedState.UNFIXED);
      }
      for (int j = -1; j <= 2; j++) {
        Polyline horizontal =
            new Polyline(
                new Point[] {
                  new IntPoint(-4000, j * 1700 + layer * 90),
                  new IntPoint(4000, j * 1700 + layer * 90)
                });
        board.insertTraceWithoutCleaning(
            horizontal, layer, 40, new int[] {net}, 1, FixedState.UNFIXED);
      }
    }
  }

  // =============================================================================================
  // Reflection into the three private statics
  // =============================================================================================

  static Method inFrontOfPadMethod;
  static Method calculateFromSideMethod;
  static Method holeCheckShapeMethod;

  static boolean inFrontOfPad(Line line, TileShape padShape, int fromSide, int width, boolean sides)
      throws Exception {
    if (inFrontOfPadMethod == null) {
      inFrontOfPadMethod =
          ForcedPadRouter.class.getDeclaredMethod(
              "inFrontOfPad", Line.class, TileShape.class, int.class, int.class, boolean.class);
      inFrontOfPadMethod.setAccessible(true);
    }
    return (Boolean) inFrontOfPadMethod.invoke(null, line, padShape, fromSide, width, sides);
  }

  static ShapeEntrySide calculateFromSide(
      FloatPoint viaLocation, TileShape viaShape, Simplex roomShape, double dist, boolean is90)
      throws Exception {
    if (calculateFromSideMethod == null) {
      calculateFromSideMethod =
          ForcedViaInserter.class.getDeclaredMethod(
              "calculateFromSide",
              FloatPoint.class,
              TileShape.class,
              Simplex.class,
              double.class,
              boolean.class);
      calculateFromSideMethod.setAccessible(true);
    }
    return (ShapeEntrySide)
        calculateFromSideMethod.invoke(null, viaLocation, viaShape, roomShape, dist, is90);
  }

  static Shape holeCheckShape(Padstack padstack, Point location, RoutingBoard b) throws Exception {
    if (holeCheckShapeMethod == null) {
      holeCheckShapeMethod =
          ForcedViaInserter.class.getDeclaredMethod(
              "holeCheckShape", Padstack.class, Point.class, RoutingBoard.class);
      holeCheckShapeMethod.setAccessible(true);
    }
    return (Shape) holeCheckShapeMethod.invoke(null, padstack, location, b);
  }

  // =============================================================================================
  // Dumps
  // =============================================================================================

  static String shp(Shape s) {
    if (s == null) {
      return "null";
    }
    IntBox b = s.boundingBox();
    return s.getClass().getSimpleName()
        + "["
        + b.ll.x
        + ","
        + b.ll.y
        + ".."
        + b.ur.x
        + ","
        + b.ur.y
        + "]";
  }

  static String side(ShapeEntrySide s) {
    if (s == null) {
      return "null";
    }
    if (s.borderIntersection == null) {
      return "no=" + s.no + " border=null";
    }
    return "no="
        + s.no
        + " border=("
        + fmt(s.borderIntersection.x)
        + ","
        + fmt(s.borderIntersection.y)
        + ")";
  }

  static String fmt(double d) {
    return String.format(java.util.Locale.US, "%.4f", d);
  }

  // =============================================================================================
  // mode `front`
  // =============================================================================================

  /** The pad octagon every `inFrontOfPad` row is measured against. */
  static final IntOctagon FRONT_PAD = new IntOctagon(-100, -100, 100, 100, -200, 200, -200, 200);

  /** `(label, line)` — the probe lines, each an exact `Line` through two `IntPoint`s. */
  static Object[][] frontLines() {
    return new Object[][] {
      {"above", new Line(new IntPoint(-400, 500), new IntPoint(400, 500))},
      {"below", new Line(new IntPoint(-400, -500), new IntPoint(400, -500))},
      {"left", new Line(new IntPoint(-500, -400), new IntPoint(-500, 400))},
      {"right", new Line(new IntPoint(500, -400), new IntPoint(500, 400))},
      {"through", new Line(new IntPoint(-400, 0), new IntPoint(400, 0))},
      {"diagUp", new Line(new IntPoint(-400, -400), new IntPoint(400, 400))},
      {"diagDown", new Line(new IntPoint(-400, 400), new IntPoint(400, -400))},
      // The four rows below are where `case 0`'s `:78` typo bites: `lineB.x + lineB.x` is
      // `2 * lineB.x`, and these lines are chosen so `2 * b.x` and `b.x + b.y` fall on opposite
      // sides of `upperRightDiagonalX + diagWidth`.
      {"typoA", new Line(new IntPoint(0, 900), new IntPoint(900, 0))},
      {"typoB", new Line(new IntPoint(900, 0), new IntPoint(0, 900))},
      {"typoC", new Line(new IntPoint(-900, 900), new IntPoint(900, -900))}
    };
  }

  static void inFrontOfPad() throws Exception {
    System.out.println("mode=front pad=" + oct(FRONT_PAD));
    for (Object[] row : frontLines()) {
      String label = (String) row[0];
      Line line = (Line) row[1];
      for (int fromSide = 0; fromSide < 8; fromSide++) {
        for (boolean withSides : new boolean[] {false, true}) {
          for (int width : new int[] {30, 300}) {
            boolean answer = inFrontOfPad(line, FRONT_PAD, fromSide, width, withSides);
            System.out.println(
                "  line="
                    + label
                    + " fromSide="
                    + fromSide
                    + " width="
                    + width
                    + " withSides="
                    + withSides
                    + " -> "
                    + answer);
          }
        }
      }
    }
    // The out-of-range arm (`:205-208`): a warning and `true`.
    System.out.println(
        "  outOfRange fromSide=8 -> "
            + inFrontOfPad((Line) frontLines()[0][1], FRONT_PAD, 8, 30, true));
    System.out.println(
        "  outOfRange fromSide=-1 -> "
            + inFrontOfPad((Line) frontLines()[0][1], FRONT_PAD, -1, 30, true));
    // A randomised block: 600 (octagon, line, fromSide, width, withSides) rows, so the eight-case
    // switch is exercised on more than one pad. The generator is a plain LCG written out here, so
    // the Rust twin reproduces the sequence with no dependency at all.
    long seed = 20261010L;
    for (int i = 0; i < 600; i++) {
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int leftX = (int) ((seed >>> 33) % 800) - 400;
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int bottomY = (int) ((seed >>> 33) % 800) - 400;
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int w = (int) ((seed >>> 33) % 500) + 20;
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int h = (int) ((seed >>> 33) % 500) + 20;
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int diagSlack = (int) ((seed >>> 33) % 400);
      int rightX = leftX + w;
      int topY = bottomY + h;
      IntOctagon pad =
          new IntOctagon(
              leftX,
              bottomY,
              rightX,
              topY,
              leftX - topY - diagSlack,
              rightX - bottomY + diagSlack,
              leftX + bottomY - diagSlack,
              rightX + topY + diagSlack);
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int ax = (int) ((seed >>> 33) % 2400) - 1200;
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int ay = (int) ((seed >>> 33) % 2400) - 1200;
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int bx = (int) ((seed >>> 33) % 2400) - 1200;
      seed = seed * 6364136223846793005L + 1442695040888963407L;
      int by = (int) ((seed >>> 33) % 2400) - 1200;
      if (ax == bx && ay == by) {
        bx += 1;
      }
      Line line = new Line(new IntPoint(ax, ay), new IntPoint(bx, by));
      int fromSide = i % 8;
      int width = 10 + (i % 7) * 40;
      boolean withSides = (i % 2) == 0;
      System.out.println(
          "  rnd i="
              + i
              + " pad="
              + oct(pad)
              + " a=("
              + ax
              + ","
              + ay
              + ") b=("
              + bx
              + ","
              + by
              + ") fromSide="
              + fromSide
              + " width="
              + width
              + " withSides="
              + withSides
              + " -> "
              + inFrontOfPad(line, pad, fromSide, width, withSides));
    }
    // A triangle whose three sides are all multiples of 45 degrees: `Simplex.isIntOctagon`
    // (Simplex.java:365-379) says **yes**, so `:59-62` does not fire and the switch runs on the
    // triangle's bounding octagon. Kept because it is the trap: "not a box or an octagon" is not
    // the same predicate as "not an int octagon".
    TileShape fortyFiveTriangle =
        TileShape.getInstance(
            new IntPoint[] {new IntPoint(0, 0), new IntPoint(500, 0), new IntPoint(0, 500)});
    System.out.println(
        "  fortyFiveTriangle isIntOctagon="
            + fortyFiveTriangle.isIntOctagon()
            + " -> "
            + inFrontOfPad((Line) frontLines()[0][1], fortyFiveTriangle, 0, 30, true));
    // The non-octagon arm (`:59-62`): `true` without looking at the line.
    TileShape skewTriangle =
        TileShape.getInstance(
            new IntPoint[] {new IntPoint(0, 0), new IntPoint(500, 100), new IntPoint(0, 500)});
    for (int fromSide = 0; fromSide < 8; fromSide++) {
      System.out.println(
          "  nonOctagon isIntOctagon="
              + skewTriangle.isIntOctagon()
              + " fromSide="
              + fromSide
              + " -> "
              + inFrontOfPad((Line) frontLines()[0][1], skewTriangle, fromSide, 30, true));
    }
  }

  static String oct(IntOctagon o) {
    return "IntOctagon["
        + o.leftX
        + ","
        + o.bottomY
        + ","
        + o.rightX
        + ","
        + o.topY
        + ","
        + o.upperLeftDiagonalX
        + ","
        + o.lowerRightDiagonalX
        + ","
        + o.lowerLeftDiagonalX
        + ","
        + o.upperRightDiagonalX
        + "]";
  }

  // =============================================================================================
  // mode `pad`
  // =============================================================================================

  /** `(label, shape)` — the pad shapes `checkForcedPad` is asked about. */
  static Object[][] padShapes() {
    return new Object[][] {
      // Straddles the net-1 trace's vertical leg at x = 0.
      {"onNet1", new IntBox(-100, 100, 100, 300)},
      // Straddles the net-2 trace at y = 900.
      {"onNet2", new IntBox(-900, 800, -700, 1000)},
      // Over the SMD pin at (-500, 0).
      {"onSmdPin", new IntBox(-600, -100, -400, 100)},
      // Over the through pin at (500, 0).
      {"onThruPin", new IntBox(400, -100, 600, 100)},
      // Empty space.
      {"freeSpace", new IntBox(2000, 2000, 2200, 2200)},
      // Outside the bounding box (`:233-236`).
      {"offBoard", new IntBox(9900, 9900, 10100, 10100)},
      // An octagon over the net-1 trace, so `isOrthogonalMode` is false at `:302`.
      {"octagonOnNet1", new IntOctagon(-100, 100, 100, 300, -300, 300, -100, 500)}
    };
  }

  static void checkForcedPad() {
    for (AngleRestriction ar :
        new AngleRestriction[] {AngleRestriction.NONE, AngleRestriction.NINETY_DEGREE}) {
      build(ar);
      System.out.println("mode=pad angle=" + ar);
      ForcedPadRouter router = new ForcedPadRouter(board);
      for (Object[] row : padShapes()) {
        String label = (String) row[0];
        TileShape shape = (TileShape) row[1];
        ShapeEntrySide fromSide = new ShapeEntrySide(shape.centreOfGravity().round(), shape);
        for (int[] nets : new int[][] {new int[] {1}, new int[] {3}, new int[0]}) {
          for (boolean copperSharing : new boolean[] {false, true}) {
            for (boolean onlyFront : new boolean[] {false, true}) {
              board.setShoveFailingObstacle(null);
              ForcedPadRouter.CheckDrillResult result =
                  router.checkForcedPad(
                      shape,
                      fromSide,
                      0,
                      nets,
                      1,
                      copperSharing,
                      null,
                      20,
                      5,
                      onlyFront,
                      null);
              System.out.println(
                  "  shape="
                      + label
                      + " fromSide="
                      + fromSide.no
                      + " nets="
                      + java.util.Arrays.toString(nets)
                      + " copperSharing="
                      + copperSharing
                      + " onlyFront="
                      + onlyFront
                      + " -> "
                      + result);
            }
          }
        }
      }
      // The recursion budgets: `:255-258` (via budget) and `:293-296` (trace budget).
      TileShape onNet1 = (TileShape) padShapes()[0][1];
      ShapeEntrySide fs = new ShapeEntrySide(onNet1.centreOfGravity().round(), onNet1);
      for (int depth : new int[] {0, 1, 2, 20}) {
        for (int viaDepth : new int[] {0, 5}) {
          ForcedPadRouter.CheckDrillResult result =
              router.checkForcedPad(onNet1, fs, 0, new int[] {3}, 1, false, null, depth, viaDepth,
                  false, null);
          System.out.println(
              "  budget onNet1 depth=" + depth + " viaDepth=" + viaDepth + " -> " + result);
        }
      }
      // `ignoreItems` (`:243-245`): ignoring the net-1 trace makes `onNet1` drillable.
      Collection<Item> ignore = new LinkedList<>();
      ignore.add(board.getItem(4));
      ForcedPadRouter.CheckDrillResult ignored =
          router.checkForcedPad(onNet1, fs, 0, new int[] {3}, 1, false, ignore, 20, 5, false, null);
      System.out.println("  ignoringItem4 onNet1 -> " + ignored);

      // `:252-279`, the shove-via block: a free foreign-net via inside the checked pad shape, so
      // `shapeEntries.shoveViaList` is non-empty and `checkForcedPad` recurses through
      // `DrillItemMover.tryShoveViaPoints` + `DrillItemMover.check`. This is the arm Task 9 had
      // to leave as a panic, and it is the reason the two tasks are a cycle.
      Via freeVia =
          board.insertVia(
              thruPad, new IntPoint(2000, 2000), new int[] {3}, 1, FixedState.UNFIXED, false);
      Via fixedVia =
          board.insertVia(
              thruPad, new IntPoint(2600, 2000), new int[] {3}, 1, FixedState.SHOVE_FIXED, false);
      System.out.println("  freeViaId=" + freeVia.getId() + " fixedViaId=" + fixedVia.getId());
      Object[][] viaShapes = {
        {"overFreeVia", new IntBox(1900, 1900, 2100, 2100)},
        {"overFixedVia", new IntBox(2500, 1900, 2700, 2100)},
        {"overBothVias", new IntBox(1900, 1900, 2700, 2100)}
      };
      for (Object[] vrow : viaShapes) {
        TileShape vshape = (TileShape) vrow[1];
        ShapeEntrySide vside = new ShapeEntrySide(vshape.centreOfGravity().round(), vshape);
        for (int viaDepth : new int[] {0, 1, 5}) {
          for (int[] nets : new int[][] {new int[] {3}, new int[] {1}}) {
            ForcedPadRouter.CheckDrillResult result =
                router.checkForcedPad(
                    vshape, vside, 0, nets, 1, false, null, 20, viaDepth, false, null);
            System.out.println(
                "  via shape="
                    + vrow[0]
                    + " fromSide="
                    + vside.no
                    + " viaDepth="
                    + viaDepth
                    + " nets="
                    + java.util.Arrays.toString(nets)
                    + " -> "
                    + result);
          }
        }
      }
    }
  }

  // =============================================================================================
  // mode `drill` — the row Task 9 could not print
  // =============================================================================================

  static void drillItemMover() {
    for (AngleRestriction ar :
        new AngleRestriction[] {AngleRestriction.NONE, AngleRestriction.NINETY_DEGREE}) {
      build(ar);
      System.out.println("mode=drill angle=" + ar);
      Via free =
          board.insertVia(
              thruPad, new IntPoint(2000, 2000), new int[] {3}, 1, FixedState.UNFIXED, false);
      Via fixedVia =
          board.insertVia(
              thruPad, new IntPoint(3000, 2000), new int[] {3}, 1, FixedState.SHOVE_FIXED, false);
      Via onPin =
          board.insertVia(
              thruPad, new IntPoint(500, 0), new int[] {1}, 1, FixedState.UNFIXED, false);
      Vector delta = new IntVector(300, 0);
      for (Via via : new Via[] {free, fixedVia, onPin}) {
        Collection<Item> ignore = new LinkedList<>();
        boolean ok = DrillItemMover.check(via, delta, 20, 5, ignore, board, null);
        System.out.println(
            "  check viaId="
                + via.getId()
                + " delta=(300,0) result="
                + ok
                + " ignoreSize="
                + ignore.size());
      }
      // The free via, shoved far enough to land on the net-1 trace: the `checkForcedPad` arm now
      // has real obstacles to reason about.
      for (int[] d : new int[][] {{-1500, -2000}, {-2000, -1600}, {-1500, -1900}}) {
        Collection<Item> ignore = new LinkedList<>();
        boolean ok =
            DrillItemMover.check(free, new IntVector(d[0], d[1]), 20, 5, ignore, board, null);
        System.out.println(
            "  check viaId="
                + free.getId()
                + " delta=("
                + d[0]
                + ","
                + d[1]
                + ") result="
                + ok
                + " ignoreSize="
                + ignore.size());
      }
      // The via budget spent (`:255-258` inside `checkForcedPad`).
      Collection<Item> ignore = new LinkedList<>();
      System.out.println(
          "  check viaId="
              + free.getId()
              + " delta=(300,0) viaDepth=0 result="
              + DrillItemMover.check(free, delta, 20, 0, ignore, board, null));
    }
  }

  // =============================================================================================
  // mode `side`
  // =============================================================================================

  static void fromSides() throws Exception {
    for (AngleRestriction ar :
        new AngleRestriction[] {AngleRestriction.NONE, AngleRestriction.NINETY_DEGREE}) {
      build(ar);
      System.out.println("mode=side angle=" + ar);
      ForcedPadRouter router = new ForcedPadRouter(board);
      for (Object[] row : padShapes()) {
        String label = (String) row[0];
        TileShape shape = (TileShape) row[1];
        Point centre = shape.centreOfGravity().round();
        for (int offset : new int[] {0, 30, 300}) {
          ShapeEntrySide s = router.calcFromSide(shape, centre, 0, offset, 1);
          System.out.println(
              "  calcFromSide shape=" + label + " offset=" + offset + " -> " + side(s));
        }
      }
    }
    // `ForcedViaInserter.calculateFromSide` is board-free: a via shape, a room simplex and a
    // distance. Both the orthogonal (`:384-420`) and the diagonal (`:424-459`) sweeps.
    System.out.println("mode=side calculateFromSide");
    TileShape viaShape = new IntOctagon(-100, -100, 100, 100, -200, 200, -200, 200);
    Object[][] rooms = {
      {"wide", new IntBox(-2000, -2000, 2000, 2000).toSimplex()},
      {"tallNarrow", new IntBox(-150, -2000, 150, 2000).toSimplex()},
      {"flatWide", new IntBox(-2000, -150, 2000, 150).toSimplex()},
      {"upperRight", new IntBox(0, 0, 2000, 2000).toSimplex()},
      {"tiny", new IntBox(-120, -120, 120, 120).toSimplex()},
      {
        "diagonalOnly",
        TileShape.getInstance(
                new IntPoint[] {
                  new IntPoint(-1500, 0), new IntPoint(0, -1500),
                  new IntPoint(1500, 0), new IntPoint(0, 1500)
                })
            .toSimplex()
      }
    };
    for (Object[] room : rooms) {
      for (double dist : new double[] {50.0, 200.0, 900.0}) {
        for (boolean is90 : new boolean[] {false, true}) {
          ShapeEntrySide s =
              calculateFromSide(new FloatPoint(0, 0), viaShape, (Simplex) room[1], dist, is90);
          System.out.println(
              "  calculateFromSide room="
                  + room[0]
                  + " dist="
                  + fmt(dist)
                  + " is90="
                  + is90
                  + " -> "
                  + side(s));
        }
      }
    }
  }

  // =============================================================================================
  // mode `hole`
  // =============================================================================================

  static void holeShape() throws Exception {
    build(AngleRestriction.NONE);
    System.out.println("mode=hole");
    for (int holeClearance : new int[] {0, 100, 400}) {
      board.rules.setHoleClearance(holeClearance);
      for (Padstack ps : new Padstack[] {thruPad, smdPad}) {
        System.out.println(
            "  holeClearance="
                + holeClearance
                + " padstack="
                + ps.name
                + " drillRadius="
                + fmt(ps.getDrillRadius())
                + " -> "
                + shp(holeCheckShape(ps, new IntPoint(1000, 1000), board)));
      }
    }
    board.rules.setHoleClearance(0);
  }

  // =============================================================================================
  // mode `layer`
  // =============================================================================================

  static void checkLayer() {
    for (AngleRestriction ar :
        new AngleRestriction[] {AngleRestriction.NONE, AngleRestriction.NINETY_DEGREE}) {
      build(ar);
      System.out.println("mode=layer angle=" + ar);
      TileShape room = new IntBox(-3000, -3000, 3000, 3000).toSimplex();
      Object[][] spots = {
        {"onNet1", new IntPoint(0, 200)},
        {"onNet2", new IntPoint(-800, 900)},
        {"onSmdPin", new IntPoint(-500, 0)},
        {"onThruPin", new IntPoint(500, 0)},
        {"freeSpace", new IntPoint(2000, 2000)}
      };
      for (Object[] spot : spots) {
        for (double radius : new double[] {0.0, 60.0, 250.0}) {
          for (boolean attachSmd : new boolean[] {false, true}) {
            for (int traceHalfWidth : new int[] {0, 30, 400}) {
              for (int[] nets : new int[][] {new int[] {1}, new int[] {3}}) {
                ForcedPadRouter.CheckDrillResult result =
                    ForcedViaInserter.checkLayer(
                        radius, 1, attachSmd, room, (Point) spot[1], 0, nets, 20, 5, board,
                        traceHalfWidth, 1);
                System.out.println(
                    "  spot="
                        + spot[0]
                        + " radius="
                        + fmt(radius)
                        + " attachSmd="
                        + attachSmd
                        + " thw="
                        + traceHalfWidth
                        + " nets="
                        + java.util.Arrays.toString(nets)
                        + " -> "
                        + result);
              }
            }
          }
        }
      }
      // A room too small for `calculateFromSide` to find any side (`:70-72`).
      TileShape tinyRoom = new IntBox(-10, -10, 10, 10).toSimplex();
      System.out.println(
          "  tinyRoom -> "
              + ForcedViaInserter.checkLayer(
                  60.0, 1, false, tinyRoom, new IntPoint(2000, 2000), 0, new int[] {3}, 20, 5,
                  board, 30, 1));
    }
  }

  // =============================================================================================
  // mode `check`
  // =============================================================================================

  static void checkVia() {
    for (AngleRestriction ar :
        new AngleRestriction[] {AngleRestriction.NONE, AngleRestriction.NINETY_DEGREE}) {
      build(ar);
      System.out.println("mode=check angle=" + ar);
      ViaInfo viaInfo = new ViaInfo("v", thruPad, 1, false, board.rules);
      ViaInfo attachInfo = new ViaInfo("va", thruPad, 1, true, board.rules);
      Object[][] spots = {
        {"onNet1", new IntPoint(0, 200)},
        {"onNet2", new IntPoint(-800, 900)},
        {"onSmdPin", new IntPoint(-500, 0)},
        {"onThruPin", new IntPoint(500, 0)},
        {"freeSpace", new IntPoint(2000, 2000)},
        {"offBoard", new IntPoint(9990, 9990)}
      };
      for (int holeClearance : new int[] {0, 300}) {
        board.rules.setHoleClearance(holeClearance);
        for (Object[] spot : spots) {
          for (ViaInfo info : new ViaInfo[] {viaInfo, attachInfo}) {
            for (int[] nets : new int[][] {new int[] {1}, new int[] {3}}) {
              for (int[] pen : new int[][] {null, {0, 0}, {30, 30}, {400, 400}}) {
                board.setShoveFailingLayer(-1);
                boolean ok =
                    ForcedViaInserter.check(info, (Point) spot[1], nets, 20, 5, board, pen, 1);
                System.out.println(
                    "  hc="
                        + holeClearance
                        + " spot="
                        + spot[0]
                        + " attachSmd="
                        + info.attachSmdAllowed()
                        + " nets="
                        + java.util.Arrays.toString(nets)
                        + " pen="
                        + (pen == null ? "null" : java.util.Arrays.toString(pen))
                        + " -> "
                        + ok
                        + " failingLayer="
                        + board.getShoveFailingLayer());
              }
            }
          }
        }
      }
      board.rules.setHoleClearance(0);
    }
  }

  // =============================================================================================
  // mode `rand`
  // =============================================================================================

  /**
   * 200 pseudo-random `(location, layer, net)` triples through `checkLayer` on the four-layer
   * board. The generator is a plain LCG written out here rather than `java.util.Random`, so the
   * Rust twin reproduces the sequence with no dependency at all (plan-6 ruling 5's `JavaRandom`
   * exists, but this table does not need it and an explicit LCG is easier to audit).
   */
  static void randomTable() {
    buildFourLayer();
    System.out.println("mode=rand layers=4");
    TileShape room = new IntBox(-5000, -5000, 5000, 5000).toSimplex();
    long seed = 20261010L;
    for (int i = 0; i < 200; i++) {
      seed = (seed * 6364136223846793005L + 1442695040888963407L);
      int x = (int) ((seed >>> 33) % 8001) - 4000;
      seed = (seed * 6364136223846793005L + 1442695040888963407L);
      int y = (int) ((seed >>> 33) % 8001) - 4000;
      seed = (seed * 6364136223846793005L + 1442695040888963407L);
      int layer = (int) ((seed >>> 33) % 4);
      seed = (seed * 6364136223846793005L + 1442695040888963407L);
      int net = (int) ((seed >>> 33) % 3) + 1;
      seed = (seed * 6364136223846793005L + 1442695040888963407L);
      double radius = 40.0 + (seed >>> 33) % 400;
      seed = (seed * 6364136223846793005L + 1442695040888963407L);
      int traceHalfWidth = (int) ((seed >>> 33) % 300);
      ForcedPadRouter.CheckDrillResult result =
          ForcedViaInserter.checkLayer(
              radius,
              1,
              (i % 3) == 0,
              room,
              new IntPoint(x, y),
              layer,
              new int[] {net},
              20,
              5,
              board,
              traceHalfWidth,
              1);
      System.out.println(
          "  i="
              + i
              + " x="
              + x
              + " y="
              + y
              + " layer="
              + layer
              + " net="
              + net
              + " radius="
              + fmt(radius)
              + " thw="
              + traceHalfWidth
              + " -> "
              + result);
    }
  }
}
