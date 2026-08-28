package app.freerouting.datastructures;

import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.ConductionArea;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.ObstacleArea;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.items.Trace;
import app.freerouting.board.model.items.Via;
import app.freerouting.board.model.structure.*;
import app.freerouting.board.searchtree.SearchTreeObject;
import app.freerouting.board.searchtree.ShapeSearchTree;
import app.freerouting.board.searchtree.ShapeTraceEntries;
import app.freerouting.board.model.structure.ShapeAndEntrySide;
import app.freerouting.board.model.structure.ShapeEntrySide;
import app.freerouting.board.state.Communication;
import app.freerouting.board.trace.PolylineTrace;
import app.freerouting.core.library.Package;
import app.freerouting.core.library.Padstack;
import app.freerouting.geometry.planar.*;
import app.freerouting.rules.*;
import java.util.*;

/**
 * Plan 2 Task 11: `Board` differential driver — the insert/remove protocol, the item-list and
 * search queries, the connectivity family, `checkTraceSegment` and the changed area, all driven
 * through the real `app.freerouting.board.facade.RoutingBoard`.
 *
 * <p>Modes: 0 insert/remove + queries, 1 connectivity, 2 the check queries, 3 the changed area and
 * the conduction/net bookkeeping, 4 the compensated 90-degree board, 5 `ShapeTraceEntries`, 6
 * cycles and the last inserters, 7 `PolylineTrace.combine` (Task 9), 8 `PolylineTrace.split` and
 * `normalize` (Task 9), 9 `BasicBoard`'s four normalisation loops and the five Task-11 methods
 * whose bodies end in normalisation (Task 9).
 */
public class P2T11 {

  static RoutingBoard board;
  /** Mode 4 only: a host CAD name and a resolution, which lower `maxTreeShapeWidth`. */
  static boolean hostCad;
  static Padstack smdPad;
  static Padstack thruPad;

  public static void main(String[] args) {
    int mode = args.length > 0 ? Integer.parseInt(args[0]) : 0;
    hostCad = mode == 4;
    build();
    switch (mode) {
      case 0 -> dumpInsertRemove();
      case 1 -> dumpConnectivity();
      case 2 -> dumpChecks();
      case 3 -> dumpChangedArea();
      case 4 -> dumpCompensated();
      case 5 -> dumpShapeTraceEntries();
      case 6 -> dumpCyclesAndInserters();
      case 7 -> dumpCombine();
      case 8 -> dumpSplitAndNormalize();
      case 9 -> dumpBoardNormalizationLoops();
      case 10 -> dumpCombineStackOverflow(args.length > 1 ? Integer.parseInt(args[1]) : 4000);
      default -> throw new IllegalArgumentException("mode " + mode);
    }
  }

  // -------------------------------------------------------------------------------------------
  // The board
  // -------------------------------------------------------------------------------------------

  /**
   * A two-layer board with a real outline, one two-pin component (an SMD pad on layer 0 and a
   * through pad on both), two traces, a via, an obstacle area and a conduction area.
   *
   * <p>The ids `BasicBoard` hands out, in order: 1 outline, 2 pin P1, 3 pin P2, 4 trace (pin P1 to
   * the via), 5 trace (the via up), 6 via, 7 obstacle area, 8 conduction area.
   */
  static void build() {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    cm.appendClass("wide");
    cm.setValue(2, 1, 600);
    cm.setValue(2, 2, 800);
    BoardRules rules = new BoardRules(ls, cm);
    rules.createDefaultNetClass();
    if (hostCad) {
      rules.setTraceAngleRestriction(AngleRestriction.NINETY_DEGREE);
    }
    // Mode 4 gives the board a host CAD name and a resolution of 10, which lowers
    // `ShapeSearchTree.calculateTreeShapes(ObstacleArea)`'s section width from 50000 to
    // `min(500 * 10, 50000) = 5000` (ShapeSearchTree.java:916-920).
    Communication comm =
        hostCad
            ? new Communication(
                Unit.MIL,
                10,
                new Communication.SpecctraParserInfo("\"", "KiCad", "7.0", null, null, false),
                new app.freerouting.io.CoordinateTransform(1, 0, 0),
                new app.freerouting.board.actions.ItemIdGenerator(),
                new app.freerouting.board.state.BoardObserverAdaptor())
            : new Communication();
    IntBox bbox = new IntBox(-10000, -10000, 10000, 10000);
    PolylineShape[] outline = {
      new PolygonShape(
          new Point[] {
            new IntPoint(-5000, -5000),
            new IntPoint(5000, -5000),
            new IntPoint(5000, 5000),
            new IntPoint(-5000, 5000)
          })
    };
    board = new RoutingBoard(bbox, ls, outline, 1, rules, comm);
    // `Nets.add` reads `netList.getBoard().rules` (Net.java:50), so the nets can only be created
    // once `BasicBoard`'s constructor has run `rules.nets.setBoard(this)` (BasicBoard.java:134).
    rules.nets.add("N1", 1, false);
    rules.nets.add("N2", 1, false);
    board.library.padstacks = new app.freerouting.core.library.Padstacks(ls);
    board.library.packages = new app.freerouting.core.library.Packages(board.library.padstacks);

    ConvexShape[] smd = {new IntBox(-50, -50, 50, 50), null};
    smdPad = board.library.padstacks.add("smd", smd, false, false);
    ConvexShape[] thru = {new IntBox(-70, -70, 70, 70), new IntBox(-70, -70, 70, 70)};
    thruPad = board.library.padstacks.add("thru", thru, true, false);

    Package.Pin[] pins = {
      new Package.Pin("P1", smdPad.id, new IntVector(-1000, 0), 0),
      new Package.Pin("P2", thruPad.id, new IntVector(1000, 1000), 0)
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

    // 2, 3: the two pins, both on net 1.
    board.insertPin(1, 0, new int[] {1}, 1, FixedState.UNFIXED);
    board.insertPin(1, 1, new int[] {1}, 1, FixedState.UNFIXED);
    // 4: SMD pin (-1000, 0) -> (0, 0), where the via sits.
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(-1000, 0), new IntPoint(0, 0)}),
        0,
        30,
        new int[] {1},
        1,
        FixedState.UNFIXED);
    // 5: the via -> the through pin, on layer 1.
    board.insertTraceWithoutCleaning(
        new Polyline(
            new Point[] {new IntPoint(0, 0), new IntPoint(1000, 0), new IntPoint(1000, 1000)}),
        1,
        30,
        new int[] {1},
        1,
        FixedState.UNFIXED);
    // 6: the via at (0, 0), joining layers 0 and 1.
    board.insertVia(thruPad, new IntPoint(0, 0), new int[] {1}, 1, FixedState.UNFIXED, true);
    // 7: an obstacle area on layer 0, far from the traces.
    board.insertObstacle(new IntBox(2000, 2000, 3000, 3000), 0, 1, FixedState.UNFIXED);
    // 8: a conduction area on net 2, layer 0.
    board.insertConductionArea(
        new IntBox(-3000, -3000, -2000, -2000), 0, new int[] {2}, 1, true, FixedState.UNFIXED);
    if (hostCad) {
      // 9: an obstacle area wider than the lowered section width, so `divideIntoSections`
      // actually splits it.
      board.insertObstacle(new IntBox(-9000, -9000, 9000, -8000), 0, 1, FixedState.UNFIXED);
    }
  }

  // -------------------------------------------------------------------------------------------
  // Mode 4: a 90-degree, clearance-compensated board whose communication names a host CAD
  // -------------------------------------------------------------------------------------------

  static void dumpCompensated() {
    System.out.println("mode=4");
    System.out.println(
        "hostCadExists=" + board.communication.hostCadExists()
            + " resolution(MIL)=" + board.communication.getResolution(Unit.MIL)
            + " hostIsOldKicad=" + board.communication.hostIsOldKicad()
            + " hostCadIsEagle=" + board.communication.hostCadIsEagle());
    ShapeSearchTree def = board.searchTreeManager.getDefaultTree();
    System.out.println("defaultTree=" + def + " compensationUsed=" + def.isClearanceCompensationUsed());
    System.out.println("wideArea treeShapes=" + board.getItem(9).treeShapeCount(def));
    for (int i = 0; i < board.getItem(9).treeShapeCount(def); i++) {
      System.out.println("  [" + i + "]=" + box(board.getItem(9).getTreeShape(def, i).boundingBox()));
    }

    System.out.println("--- setClearanceCompensationUsed(true)");
    board.searchTreeManager.setClearanceCompensationUsed(true);
    def = board.searchTreeManager.getDefaultTree();
    System.out.println("defaultTree=" + def + " compensationUsed=" + def.isClearanceCompensationUsed());
    System.out.println("compensation(1, 0)=" + def.clearanceCompensationValue(1, 0));
    System.out.println("trace 4 treeShapes=" + board.getItem(4).treeShapeCount(def));
    for (int i = 0; i < board.getItem(4).treeShapeCount(def); i++) {
      System.out.println("  [" + i + "]=" + box(board.getItem(4).getTreeShape(def, i).boundingBox()));
    }
    System.out.println("wideArea treeShapes=" + board.getItem(9).treeShapeCount(def));

    // The compensated + 90-degree paths of the check queries: `checkPolylineTrace` builds a
    // temporary trace whose tile shapes come from this tree, so they are boxes carrying the
    // compensation (BasicBoard.java:1067-1071).
    System.out.println(
        "checkPolylineTrace(free)="
            + board.checkPolylineTrace(
                new Polyline(new Point[] {new IntPoint(-4000, 4000), new IntPoint(-3000, 4000)}),
                0,
                30,
                new int[] {1},
                1));
    System.out.println(
        "checkPolylineTrace(obstacle)="
            + board.checkPolylineTrace(
                new Polyline(new Point[] {new IntPoint(1500, 2500), new IntPoint(2500, 2500)}),
                0,
                30,
                new int[] {1},
                1));
    System.out.println(
        "checkPolylineTrace(nearArea)="
            + board.checkPolylineTrace(
                new Polyline(new Point[] {new IntPoint(-4000, -7800), new IntPoint(-3000, -7800)}),
                0,
                30,
                new int[] {1},
                1));
    System.out.println(
        "checkTraceShape(free)="
            + board.checkTraceShape(new IntBox(-4500, 4000, -4000, 4500), 0, new int[] {1}, 1, null));
    seg("free", new IntPoint(-4000, 4000), new IntPoint(-3000, 4000), 0, new int[] {1}, 30, 1, false);
    seg("blocked", new IntPoint(1500, 2500), new IntPoint(2500, 2500), 0, new int[] {1}, 30, 1, false);
  }

  // -------------------------------------------------------------------------------------------
  // Mode 0: the insert/remove protocol and the item-list queries
  // -------------------------------------------------------------------------------------------

  static void dumpInsertRemove() {
    System.out.println("mode=0 revision=" + board.getRevision());
    System.out.println("layerCount=" + board.getLayerCount());
    System.out.println(
        "minTraceHalfWidth=" + board.getMinTraceHalfWidth()
            + " maxTraceHalfWidth=" + board.getMaxTraceHalfWidth());
    System.out.println("boundingBox=" + box(board.getBoundingBox()));
    for (Item it : board.getItems()) {
      System.out.println(
          "item id="
              + it.getId()
              + " class="
              + it.getClass().getSimpleName()
              + " nets="
              + Arrays.toString(it.netNumbers)
              + " cl="
              + it.clearanceClassIndex()
              + " layers="
              + it.firstLayer()
              + ".."
              + it.lastLayer()
              + " onBoard="
              + it.isOnTheBoard()
              + " tiles="
              + it.tileShapeCount()
              + " bbox="
              + box(it.boundingBox()));
    }
    System.out.println("outline=" + board.getOutline().getId());
    System.out.println("pins=" + ids(board.getPins()));
    System.out.println("smdPins=" + ids(board.getSmdPins()));
    System.out.println("vias=" + ids(board.getVias()));
    System.out.println("traces=" + ids(board.getTraces()));
    System.out.println("conductionAreas=" + ids(board.getConductionAreas()));
    System.out.println("connectableItems(1)=" + ids(board.getConnectableItems(1)));
    System.out.println("connectableItemCount(1)=" + board.connectableItemCount(1));
    System.out.println("connectableItems(2)=" + ids(board.getConnectableItems(2)));
    System.out.println("componentItems(1)=" + ids(board.getComponentItems(1)));
    System.out.println("componentPins(1)=" + ids(board.getComponentPins(1)));
    System.out.println("pin(1,0)=" + board.getPin(1, 0).getId());
    System.out.println("pin(1,1)=" + board.getPin(1, 1).getId());
    System.out.println("pin(1,7)=" + (board.getPin(1, 7) == null ? "null" : "?"));
    System.out.printf(Locale.ROOT, "cumulativeTraceLength=%.6f%n", board.cumulativeTraceLength());
    System.out.println("non45=" + board.getNon45DegreeTraceCount());
    System.out.println("clearance(1,1,0)=" + board.clearanceValue(1, 1, 0));
    System.out.println("clearance(2,1,0)=" + board.clearanceValue(2, 1, 0));
    System.out.println("clearance(2,2,1)=" + board.clearanceValue(2, 2, 1));
    System.out.println("contains(0,0)=" + board.contains(new IntPoint(0, 0)));
    System.out.println("contains(99999,0)=" + board.contains(new IntPoint(99999, 0)));
    System.out.println("componentName(2)=" + board.getItem(2).componentName());
    System.out.println("componentName(4)=" + board.getItem(4).componentName());
    System.out.println("allNetNames(4)=" + board.getItem(4).getAllNetNames());
    System.out.println("allNetNames(7)=" + board.getItem(7).getAllNetNames());
    System.out.println(
        "boundingBoxOf(4,5)=" + box(board.getBoundingBox(List.of(board.getItem(4), board.getItem(5)))));

    // The plain overlap query around the via, before and after removing the via.
    TileShape probe = new IntBox(-100, -100, 100, 100);
    System.out.println("overlappingObjects(probe, 0)=" + objectIds(board.overlappingObjects(probe, 0)));
    System.out.println("overlappingObjects(probe, 1)=" + objectIds(board.overlappingObjects(probe, 1)));
    System.out.println("overlappingObjects(probe, -1)=" + objectIds(board.overlappingObjects(probe, -1)));
    System.out.println(
        "overlappingItemsWithClearance(probe, 0, [], 1)="
            + ids(board.overlappingItemsWithClearance(probe, 0, new int[0], 1)));
    System.out.println(
        "overlappingItemsWithClearance(probe, 0, [1], 1)="
            + ids(board.overlappingItemsWithClearance(probe, 0, new int[] {1}, 1)));
    System.out.println("overlappingItems(box, 0)=" + ids(board.overlappingItems(new IntBox(-1100, -100, 100, 100), 0)));
    System.out.println("pickItems((0,0), 0)=" + ids(board.pickItems(new IntPoint(0, 0), 0, null)));

    System.out.println("--- removeItem(outline) refuses (SYSTEM_FIXED)");
    Item outline = board.getOutline();
    System.out.println("isDeletionForbidden(outline)=" + outline.isDeletionForbidden());
    board.removeItem(outline);
    System.out.println("revision=" + board.getRevision() + " outline=" + board.getOutline().getId());

    System.out.println("--- removeItem(6) removes the via");
    board.removeItem(board.getItem(6));
    System.out.println("revision=" + board.getRevision());
    System.out.println("getItem(6)=" + (board.getItem(6) == null ? "null" : "?"));
    System.out.println("items=" + ids(board.getItems()));
    System.out.println("overlappingObjects(probe, 0)=" + objectIds(board.overlappingObjects(probe, 0)));
    System.out.println("overlappingObjects(probe, 1)=" + objectIds(board.overlappingObjects(probe, 1)));

    System.out.println("--- removeItems([4, 7])");
    System.out.println(
        "removeItems=" + board.removeItems(List.of(board.getItem(4), board.getItem(7))));
    System.out.println("items=" + ids(board.getItems()) + " revision=" + board.getRevision());

    System.out.println("--- incrementRevision");
    board.incrementRevision();
    System.out.println("revision=" + board.getRevision());
  }

  // -------------------------------------------------------------------------------------------
  // Mode 1: connectivity
  // -------------------------------------------------------------------------------------------

  static void dumpConnectivity() {
    System.out.println("mode=1");
    for (Item it : board.getItems()) {
      System.out.println(
          "id="
              + it.getId()
              + " normalContacts="
              + ids(it.getNormalContacts())
              + " allContacts="
              + ids(it.getAllContacts())
              + " connected="
              + it.isConnected()
              + " tail="
              + it.isTail()
              + " overlap="
              + it.isOverlap()
              + " ratsnest="
              + points(it.getRatsnestCorners()));
    }
    for (int layer = 0; layer < board.getLayerCount(); layer++) {
      for (Item it : board.getItems()) {
        System.out.println(
            "id=" + it.getId() + " layer=" + layer + " contactsOnLayer=" + ids(it.getAllContacts(layer))
                + " connectedOnLayer=" + it.isConnectedOnLayer(layer));
      }
    }
    System.out.println("connectedSet(2, 1)=" + ids(board.getItem(2).getConnectedSet(1)));
    System.out.println("connectedSet(2, -1)=" + ids(board.getItem(2).getConnectedSet(-1)));
    System.out.println("connectedSet(3, 1)=" + ids(board.getItem(3).getConnectedSet(1)));
    System.out.println("connectedSet(8, 2)=" + ids(board.getItem(8).getConnectedSet(2)));
    System.out.println("connectedSet(2, 2)=" + ids(board.getItem(2).getConnectedSet(2)));
    System.out.println(
        "connectedSetStopAtPlane(2, 1)=" + ids(board.getItem(2).getConnectedSet(1, true)));
    System.out.println("unconnectedSet(2, 1)=" + ids(board.getItem(2).getUnconnectedSet(1)));
    System.out.println("unconnectedSet(8, 2)=" + ids(board.getItem(8).getUnconnectedSet(2)));
    System.out.println("unconnectedSet(2, 0)=" + ids(board.getItem(2).getUnconnectedSet(0)));
    System.out.println("connectionItems(4)=" + ids(board.getItem(4).getConnectionItems()));
    System.out.println("connectionItems(5)=" + ids(board.getItem(5).getConnectionItems()));
    System.out.println(
        "connectionItemsVia(4)="
            + ids(board.getItem(4).getConnectionItems(Item.StopConnectionOption.VIA)));
    System.out.println(
        "connectionItemsFanout(4)="
            + ids(board.getItem(4).getConnectionItems(Item.StopConnectionOption.FANOUT_VIA)));
    for (int a : new int[] {2, 4, 5, 6}) {
      for (int b : new int[] {2, 4, 5, 6}) {
        System.out.println(
            "normalContactPoint("
                + a
                + ","
                + b
                + ")="
                + point(board.getItem(a).normalContactPoint(board.getItem(b)))
                + " firstCommonLayer="
                + board.getItem(a).firstCommonLayer(board.getItem(b))
                + " lastCommonLayer="
                + board.getItem(a).lastCommonLayer(board.getItem(b)));
      }
    }
    List<String> sets = new ArrayList<>();
    for (Collection<Item> set : board.getConnectedSets(1)) {
      sets.add(ids(set));
    }
    System.out.println("connectedSets(1)=" + sets);
    System.out.println("isFanoutVia(6)=" + board.getItem(6).isFanoutVia(null));
    System.out.println("hasIgnoredNets(4)=" + board.getItem(4).hasIgnoredNets());
    System.out.println("isCycle(4)=" + ((Trace) board.getItem(4)).isCycle());
    System.out.println("startContacts(4)=" + ids(((Trace) board.getItem(4)).getStartContacts()));
    System.out.println("endContacts(4)=" + ids(((Trace) board.getItem(4)).getEndContacts()));
    System.out.println("startContacts(5)=" + ids(((Trace) board.getItem(5)).getStartContacts()));
    System.out.println("endContacts(5)=" + ids(((Trace) board.getItem(5)).getEndContacts()));
    System.out.println(
        "touchingPins(4)=" + ids(((Trace) board.getItem(4)).touchingPinsAtEndCorners()));
    System.out.println("validate(4)=" + board.getItem(4).validate());
    System.out.println("validate(6)=" + board.getItem(6).validate());
    System.out.println("swappablePins(2)=" + ids(((Pin) board.getItem(2)).getSwappablePins()));
  }

  // -------------------------------------------------------------------------------------------
  // Mode 2: the check queries
  // -------------------------------------------------------------------------------------------

  static void dumpChecks() {
    System.out.println("mode=2");
    // A free run on layer 0 well away from everything, and one straight into the obstacle area.
    seg("free", new IntPoint(-4000, 4000), new IntPoint(-3000, 4000), 0, new int[] {1}, 30, 1, false);
    seg("blocked", new IntPoint(1500, 2500), new IntPoint(2500, 2500), 0, new int[] {1}, 30, 1, false);
    seg("blocked_own_net", new IntPoint(-2000, 0), new IntPoint(-500, 0), 0, new int[] {1}, 30, 1, false);
    seg("blocked_foreign", new IntPoint(-2000, 0), new IntPoint(-500, 0), 0, new int[] {9}, 30, 1, false);
    seg("degenerate", new IntPoint(0, 0), new IntPoint(0, 0), 0, new int[] {1}, 30, 1, false);
    seg("shovable_only", new IntPoint(-2000, 0), new IntPoint(-500, 0), 0, new int[] {9}, 30, 1, true);
    seg("wide_class", new IntPoint(1500, 2500), new IntPoint(2500, 2500), 0, new int[] {1}, 30, 2, false);

    System.out.println(
        "checkShape(free)=" + board.checkShape(new IntBox(-4500, 4000, -4000, 4500), 0, new int[] {1}, 1));
    System.out.println(
        "checkShape(obstacle)=" + board.checkShape(new IntBox(2200, 2200, 2400, 2400), 0, new int[] {1}, 1));
    System.out.println(
        "checkShape(outside)=" + board.checkShape(new IntBox(-20000, 0, -19000, 100), 0, new int[] {1}, 1));
    System.out.println(
        "checkTraceShape(free)="
            + board.checkTraceShape(new IntBox(-4500, 4000, -4000, 4500), 0, new int[] {1}, 1, null));
    System.out.println(
        "checkTraceShape(obstacle)="
            + board.checkTraceShape(new IntBox(2200, 2200, 2400, 2400), 0, new int[] {1}, 1, null));
    Set<Pin> contactPins = new TreeSet<>();
    contactPins.add((Pin) board.getItem(2));
    System.out.println(
        "checkTraceShape(atPin, contactPins)="
            + board.checkTraceShape(new IntBox(-1050, -50, -950, 50), 0, new int[] {1}, 1, contactPins));
    System.out.println(
        "checkTraceShape(atPin, emptyContactPins)="
            + board.checkTraceShape(
                new IntBox(-1050, -50, -950, 50), 0, new int[] {1}, 1, new TreeSet<Pin>()));
    System.out.println(
        "checkPolylineTrace(free)="
            + board.checkPolylineTrace(
                new Polyline(new Point[] {new IntPoint(-4000, 4000), new IntPoint(-3000, 4000)}),
                0,
                30,
                new int[] {1},
                1));
    System.out.println(
        "checkPolylineTrace(obstacle)="
            + board.checkPolylineTrace(
                new Polyline(new Point[] {new IntPoint(1500, 2500), new IntPoint(2500, 2500)}),
                0,
                30,
                new int[] {1},
                1));
    System.out.println(
        "checkMoveItem(7, (10,10))="
            + board.checkMoveItem(board.getItem(7), new IntVector(10, 10), null));
    System.out.println(
        "checkMoveItem(4, (10,10))="
            + board.checkMoveItem(board.getItem(4), new IntVector(10, 10), null));
    System.out.println("checkChangeNet(7, 3)=" + board.checkChangeNet(board.getItem(7), 3));
    System.out.println("checkChangeNet(4, 3)=" + board.checkChangeNet(board.getItem(4), 3));
    System.out.println(
        "pickNearestRoutingItem((0,0), 0)=" + itemId(board.pickNearestRoutingItem(new IntPoint(0, 0), 0, null)));
    System.out.println(
        "pickNearestRoutingItem((-1000,0), 0)="
            + itemId(board.pickNearestRoutingItem(new IntPoint(-1000, 0), 0, null)));
    System.out.println(
        "pickNearestRoutingItem((4000,4000), 0)="
            + itemId(board.pickNearestRoutingItem(new IntPoint(4000, 4000), 0, null)));
    System.out.println(
        "traceTail((1000,1000), 1, [1])="
            + itemId(board.getTraceTail(new IntPoint(1000, 1000), 1, new int[] {1})));
    System.out.println(
        "traceTail((0,0), 0, [1])=" + itemId(board.getTraceTail(new IntPoint(0, 0), 0, new int[] {1})));
    System.out.println(
        "containsTraceTails([4,5], [])="
            + board.containsTraceTails(List.of(board.getItem(4), board.getItem(5)), new int[0]));

    // `checkPolylineTrace` builds a temporary `PolylineTrace` (BasicBoard.java:1055-1065) whose
    // `Item` constructor draws an id from the generator (Item.java:85-90), so the next real
    // insert is *not* the id you would get from the item count.
    System.out.println("idBefore=" + board.communication.idGenerator.maxGeneratedId());
    board.checkPolylineTrace(
        new Polyline(new Point[] {new IntPoint(-4000, 4000), new IntPoint(-3000, 4000)}),
        0,
        30,
        new int[] {1},
        1);
    System.out.println("idAfterOneCheck=" + board.communication.idGenerator.maxGeneratedId());
    PolylineTrace inserted =
        board.insertTraceWithoutCleaning(
            new Polyline(new Point[] {new IntPoint(-4000, 3000), new IntPoint(-3000, 3000)}),
            0,
            30,
            new int[] {1},
            1,
            FixedState.UNFIXED);
    System.out.println("insertedId=" + inserted.getId() + " items=" + ids(board.getItems()));
  }

  // -------------------------------------------------------------------------------------------
  // Mode 5: ShapeTraceEntries, ShapeEntrySide and ShapeAndEntrySide
  // -------------------------------------------------------------------------------------------

  /**
   * A two-layer board with three traces crossing a square at the origin: one of the own net
   * (1) and two of a foreign net (2), so `storeItems` has something to sort and
   * `nextSubstituteTracePiece` something to build.
   */
  static void buildShoveBoard() {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    BoardRules rules = new BoardRules(ls, cm);
    rules.createDefaultNetClass();
    Communication comm = new Communication();
    board =
        new RoutingBoard(
            new IntBox(-10000, -10000, 10000, 10000),
            ls,
            new PolylineShape[0],
            0,
            rules,
            comm);
    board.library.padstacks = new app.freerouting.core.library.Padstacks(ls);
    board.library.packages = new app.freerouting.core.library.Packages(board.library.padstacks);
    rules.nets.add("N1", 1, false);
    rules.nets.add("N2", 1, false);
    // 2: the own-net trace, straight through the square.
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(-3000, 0), new IntPoint(3000, 0)}),
        0,
        30,
        new int[] {1},
        1,
        FixedState.UNFIXED);
    // 3, 4: two foreign-net traces crossing it.
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(0, -3000), new IntPoint(0, 3000)}),
        0,
        30,
        new int[] {2},
        1,
        FixedState.UNFIXED);
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(-3000, 200), new IntPoint(3000, 200)}),
        0,
        30,
        new int[] {2},
        1,
        FixedState.UNFIXED);
  }

  static void dumpShapeTraceEntries() {
    buildShoveBoard();
    System.out.println("mode=5");
    TileShape shape = new IntBox(-500, -500, 500, 500);
    int[] ownNetNos = {1};
    System.out.println("items=" + ids(board.getItems()));
    Set<Item> overlaps = board.overlappingItemsWithClearance(shape, 0, ownNetNos, 1);
    System.out.println("overlaps=" + ids(overlaps));

    System.out.println("--- ShapeEntrySide");
    PolylineTrace crossing = (PolylineTrace) board.getItem(3);
    ShapeEntrySide fromPolyline = new ShapeEntrySide(crossing.polyline(), 1, shape);
    System.out.println("fromPolyline no=" + fromPolyline.no + " is=" + fp(fromPolyline.borderIntersection));
    ShapeEntrySide fromPoint = new ShapeEntrySide(new IntPoint(-2000, 0), shape);
    System.out.println("fromPoint no=" + fromPoint.no + " is=" + fp(fromPoint.borderIntersection));
    LineSegment seg = new LineSegment(crossing.polyline(), 1);
    ShapeEntrySide left = new ShapeEntrySide(seg, shape, true);
    System.out.println("fromSegment(left) no=" + left.no + " is=" + fp(left.borderIntersection));
    ShapeEntrySide right = new ShapeEntrySide(seg, shape, false);
    System.out.println("fromSegment(right) no=" + right.no + " is=" + fp(right.borderIntersection));
    System.out.println(
        "NOT_CALCULATED no=" + ShapeEntrySide.NOT_CALCULATED.no
            + " is=" + fp(ShapeEntrySide.NOT_CALCULATED.borderIntersection));

    System.out.println("--- ShapeAndEntrySide");
    for (boolean orthogonal : new boolean[] {false, true}) {
      for (boolean inShoveCheck : new boolean[] {false, true}) {
        ShapeAndEntrySide sae = new ShapeAndEntrySide(crossing, 0, orthogonal, inShoveCheck);
        System.out.println(
            "orthogonal=" + orthogonal
                + " inShoveCheck=" + inShoveCheck
                + " shape=" + sae.shape.getClass().getSimpleName()
                + " bbox=" + box(sae.shape.boundingBox())
                + " fromSide=" + (sae.fromSide == null
                    ? "null"
                    : sae.fromSide.no + "@" + fp(sae.fromSide.borderIntersection)));
      }
    }

    System.out.println("--- storeItems");
    ShapeTraceEntries entries =
        new ShapeTraceEntries(shape, 0, ownNetNos, 1, null, board);
    boolean stored = entries.storeItems(overlaps, false, false);
    System.out.println(
        "stored=" + stored
            + " stackDepth=" + entries.stackDepth()
            + " substituteTraceCount=" + entries.substituteTraceCount()
            + " traceTailsInShape=" + entries.traceTailsInShape()
            + " foundObstacle=" + itemId(entries.getFoundObstacle())
            + " shoveVias=" + entries.shoveViaList.size());
    for (; ; ) {
      PolylineTrace piece = entries.nextSubstituteTracePiece();
      if (piece == null) {
        break;
      }
      StringBuilder sb = new StringBuilder("piece net=" + Arrays.toString(piece.netNumbers)
          + " halfWidth=" + piece.getHalfWidth() + " corners=");
      for (int i = 0; i < piece.cornerCount(); i++) {
        sb.append(' ').append(fp(piece.polyline().cornerApprox(i)));
      }
      System.out.println(sb);
    }
    System.out.println("after: substituteTraceCount=" + entries.substituteTraceCount());

    System.out.println("--- cutoutTrace");
    buildShoveBoard();
    ShapeTraceEntries.cutoutTrace((PolylineTrace) board.getItem(3), shape, 1);
    System.out.println("items=" + ids(board.getItems()));
    for (Item it : board.getItems()) {
      if (it instanceof PolylineTrace t) {
        System.out.println(
            "trace " + it.getId() + " corners=" + cornerList(t) + " onBoard=" + it.isOnTheBoard());
      }
    }

    System.out.println("--- cutoutTraces");
    buildShoveBoard();
    ShapeTraceEntries entries2 =
        new ShapeTraceEntries(shape, 0, ownNetNos, 1, null, board);
    entries2.cutoutTraces(new java.util.ArrayList<>(board.getItems()));
    System.out.println("items=" + ids(board.getItems()));
    for (Item it : board.getItems()) {
      if (it instanceof PolylineTrace t) {
        System.out.println("trace " + it.getId() + " corners=" + cornerList(t));
      }
    }
  }

  // -------------------------------------------------------------------------------------------
  // Mode 6: cycles, overlaps, the remaining inserters and the trace-geometry adapter
  // -------------------------------------------------------------------------------------------

  /**
   * A board with a genuine cycle (two traces between the same pair of vias) and a trace whose two
   * ends both land inside one conduction area, which is what `isOverlap` looks for.
   *
   * <p>Ids: 1 outline, 2 via A, 3 via B, 4 the direct trace, 5 the detour trace, 6 the conduction
   * area, 7 the trace inside it.
   */
  static void buildCycleBoard() {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    BoardRules rules = new BoardRules(ls, cm);
    rules.createDefaultNetClass();
    Communication comm = new Communication();
    board =
        new RoutingBoard(
            new IntBox(-10000, -10000, 10000, 10000),
            ls,
            new PolylineShape[0],
            0,
            rules,
            comm);
    board.library.padstacks = new app.freerouting.core.library.Padstacks(ls);
    board.library.packages = new app.freerouting.core.library.Packages(board.library.padstacks);
    rules.nets.add("N1", 1, false);
    rules.nets.add("N2", 1, false);
    rules.nets.add("N3", 1, false);
    ConvexShape[] thru = {new IntBox(-70, -70, 70, 70), new IntBox(-70, -70, 70, 70)};
    thruPad = board.library.padstacks.add("thru", thru, true, false);
    board.insertVia(thruPad, new IntPoint(0, 0), new int[] {1}, 1, FixedState.UNFIXED, true);
    board.insertVia(thruPad, new IntPoint(2000, 0), new int[] {1}, 1, FixedState.UNFIXED, true);
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(0, 0), new IntPoint(2000, 0)}),
        0,
        30,
        new int[] {1},
        1,
        FixedState.UNFIXED);
    board.insertTraceWithoutCleaning(
        new Polyline(
            new Point[] {
              new IntPoint(0, 0),
              new IntPoint(0, 1000),
              new IntPoint(2000, 1000),
              new IntPoint(2000, 0)
            }),
        0,
        30,
        new int[] {1},
        1,
        FixedState.UNFIXED);
    board.insertConductionArea(
        new IntBox(4000, 0, 6000, 2000), 0, new int[] {3}, 1, true, FixedState.UNFIXED);
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(4200, 200), new IntPoint(5800, 1800)}),
        0,
        30,
        new int[] {3},
        1,
        FixedState.UNFIXED);
    // Two pinless components, one on each side, so `ComponentObstacleArea.isFront` has a real
    // component to read (with `componentId == 0` it throws — quirk #49).
    Package pkg =
        board.library.packages.add(
            "pkg",
            new Package.Pin[0],
            new Shape[0],
            new double[0],
            new boolean[0],
            new Package.Keepout[0],
            new Package.Keepout[0],
            new Package.Keepout[0],
            true);
    board.components.add(new IntPoint(0, 0), 0, true, pkg);
    board.components.add(new IntPoint(0, 0), 0, false, pkg);
  }

  static void dumpCyclesAndInserters() {
    buildCycleBoard();
    System.out.println("mode=6");
    System.out.println("items=" + ids(board.getItems()));
    for (int id : new int[] {4, 5, 7}) {
      Trace t = (Trace) board.getItem(id);
      System.out.println(
          "trace " + id
              + " startContacts=" + ids(t.getStartContacts())
              + " endContacts=" + ids(t.getEndContacts())
              + " isOverlap=" + t.isOverlap()
              + " isCycle=" + t.isCycle()
              + " isTail=" + t.isTail());
    }
    System.out.println("connectionItems(4)=" + ids(board.getItem(4).getConnectionItems()));

    // `PolylineTraceSearchTreeAdapter` is package-private, so this driver cannot reach it; its
    // five methods are `Board::trace_has_default_entries`, `replace_trace_geometry`,
    // `merge_trace_entries_in_front`/`_at_end` and `change_trace_entries`, and the
    // `SearchTreeManager` bodies they forward to are covered by `p2t10` modes 3, 5 and 8.

    System.out.println("--- removeIfCycle(4)");
    buildCycleBoard();
    System.out.println("removed=" + board.removeIfCycle((Trace) board.getItem(4)));
    System.out.println("items=" + ids(board.getItems()));

    System.out.println("--- reduceNetsOfRouteItems");
    buildCycleBoard();
    board.getItem(4).netNumbers = new int[] {1, 2};
    System.out.println("result=" + board.reduceNetsOfRouteItems());
    System.out.println("nets(4)=" + Arrays.toString(board.getItem(4).netNumbers));

    System.out.println("--- deleteAllTracksAndVias");
    buildCycleBoard();
    board.deleteAllTracksAndVias();
    System.out.println("items=" + ids(board.getItems()));

    System.out.println("--- clearAllItemTemporaryAutorouteData");
    buildCycleBoard();
    board.clearAllItemTemporaryAutorouteData();
    System.out.println("items=" + ids(board.getItems()));

    System.out.println("--- the remaining inserters");
    buildCycleBoard();
    Via escape =
        board.insertEscapeVia(thruPad, new IntPoint(-2000, 0), new int[] {1}, 1, FixedState.UNFIXED, 0);
    System.out.println(
        "escapeVia id=" + escape.getId()
            + " isEscapeVia=" + escape.isEscapeVia
            + " smdLayer=" + escape.escapeViaSmdLayer
            + " attachAllowed=" + escape.attachAllowed);
    Item viaObstacle =
        board.insertViaObstacle(new IntBox(-4000, 0, -3000, 1000), 0, 1, FixedState.UNFIXED);
    System.out.println("viaObstacle id=" + viaObstacle.getId() + " class=" + viaObstacle.getClass().getSimpleName());
    for (int componentId : new int[] {1, 2}) {
      Item componentObstacle =
          board.insertComponentObstacle(
              new IntBox(-6000, 1200 * componentId, -5000, 1200 * componentId + 1000),
              0,
              new IntVector(0, 0),
              0,
              false,
              1,
              componentId,
              "ko" + componentId,
              FixedState.UNFIXED);
      System.out.println(
          "componentObstacle id=" + componentObstacle.getId()
              + " component=" + componentId
              + " class=" + componentObstacle.getClass().getSimpleName()
              + " isFront=" + ((app.freerouting.board.model.items.ComponentObstacleArea) componentObstacle).isFront()
              + " name=" + componentObstacle.componentName());
    }
    Item obstacleOfComponent =
        board.insertObstacle(
            new IntBox(-8000, 0, -7000, 1000),
            0,
            new IntVector(10, 20),
            0,
            false,
            1,
            0,
            "keepout",
            FixedState.UNFIXED);
    System.out.println(
        "obstacleOfComponent id=" + obstacleOfComponent.getId()
            + " bbox=" + box(obstacleOfComponent.boundingBox()));
    Item outline =
        board.insertComponentOutline(
            new IntBox(-9000, 3000, -8000, 4000),
            true,
            new IntVector(0, 0),
            0,
            0,
            true,
            false,
            true,
            FixedState.UNFIXED);
    System.out.println(
        "componentOutline id=" + outline.getId()
            + " class=" + outline.getClass().getSimpleName()
            + " tiles=" + outline.tileShapeCount());
    System.out.println("items=" + ids(board.getItems()) + " revision=" + board.getRevision());
  }

  static String cornerList(PolylineTrace t) {
    StringBuilder sb = new StringBuilder();
    for (int i = 0; i < t.cornerCount(); i++) {
      sb.append(' ').append(fp(t.polyline().cornerApprox(i)));
    }
    return sb.toString().trim();
  }

  static String fp(FloatPoint p) {
    if (p == null) {
      return "null";
    }
    return String.format(Locale.ROOT, "(%.4f,%.4f)", p.x, p.y);
  }

  static void seg(
      String name,
      Point from,
      Point to,
      int layer,
      int[] netNos,
      int halfWidth,
      int clClass,
      boolean onlyNotShovable) {
    double result = board.checkTraceSegment(from, to, layer, netNos, halfWidth, clClass, onlyNotShovable);
    System.out.printf(Locale.ROOT, "checkTraceSegment(%s)=%.6f%n", name, result);
  }

  // -------------------------------------------------------------------------------------------
  // Mode 3: the changed area and the board-level bookkeeping
  // -------------------------------------------------------------------------------------------

  static void dumpChangedArea() {
    System.out.println("mode=3");
    System.out.println("changedArea=" + (board.changedArea == null ? "null" : "?"));
    board.startMarkingChangedArea();
    System.out.println("after start: " + changedArea());
    board.joinChangedArea(new IntPoint(100, 100).toFloat(), 0);
    System.out.println("after join(100,100,0): " + changedArea());
    board.changedArea.join(board.getItem(7).getTileShape(0), 0);
    System.out.println("after join(shape of 7, 0): " + changedArea());
    System.out.println("surroundingBox=" + box(board.changedArea.surroundingBox()));
    board.changedArea.setEmpty(0);
    System.out.println("after setEmpty(0): " + changedArea());
    board.markAllChangedArea();
    System.out.println("after markAll: " + changedArea());
    System.out.println("surroundingBox=" + box(board.changedArea.surroundingBox()));

    System.out.println("--- ignoreConduction / changeConductionIsObstacle (quirk 50)");
    System.out.println("ignoreConduction=" + board.rules.getIgnoreConduction());
    System.out.println("isObstacle(8)=" + ((ConductionArea) board.getItem(8)).getIsObstacle());
    board.changeConductionIsObstacle(false);
    System.out.println(
        "after change(false): ignoreConduction="
            + board.rules.getIgnoreConduction()
            + " isObstacle(8)="
            + ((ConductionArea) board.getItem(8)).getIsObstacle());
    board.changeConductionIsObstacle(false);
    System.out.println(
        "after change(false) again: ignoreConduction="
            + board.rules.getIgnoreConduction()
            + " isObstacle(8)="
            + ((ConductionArea) board.getItem(8)).getIsObstacle());
    board.changeConductionIsObstacle(true);
    System.out.println(
        "after change(true): ignoreConduction="
            + board.rules.getIgnoreConduction()
            + " isObstacle(8)="
            + ((ConductionArea) board.getItem(8)).getIsObstacle());

    System.out.println("--- unfillConductionAreas");
    board.unfillConductionAreas();
    System.out.println(
        "ignoreConduction="
            + board.rules.getIgnoreConduction()
            + " isObstacle(8)="
            + ((ConductionArea) board.getItem(8)).getIsObstacle()
            + " isFilled(8)="
            + ((ConductionArea) board.getItem(8)).getIsFilled());

    System.out.println("--- removeTraceTails(1, NONE)");
    System.out.println("removed=" + board.removeTraceTails(1, Item.StopConnectionOption.NONE));
    System.out.println("items=" + ids(board.getItems()));

    System.out.println("--- moveBy(item 7, (10, 20))");
    board.getItem(7).moveBy(new IntVector(10, 20));
    System.out.println("items=" + ids(board.getItems()));
    System.out.println("item 7 bbox=" + box(board.getItem(7).boundingBox()));
    System.out.println(
        "overlappingObjects at the moved area="
            + objectIds(board.overlappingObjects(new IntBox(2500, 2500, 2600, 2600), 0)));

    System.out.println("--- changeClearanceClassIndex(4, 2)");
    board.getItem(4).changeClearanceClassIndex(2);
    System.out.println("cl(4)=" + board.getItem(4).clearanceClassIndex());
    // Item.changeClearanceClassIndex clears the derived data and, with clearance compensation
    // off, does *not* re-insert (Item.java:944-949) — so the item keeps its tree leaves with an
    // empty shape cache, and the next query has to recompute (Item.java:212-238). No `validate`
    // in between, so nothing else warms the cache first.
    System.out.println(
        "overlappingObjects after change="
            + objectIds(board.overlappingObjects(new IntBox(-600, -100, -400, 100), 0)));
    System.out.println(
        "overlappingItemsWithClearance after change="
            + ids(board.overlappingItemsWithClearance(
                new IntBox(-600, -100, -400, 100), 0, new int[0], 1)));
    System.out.println("validate(4)=" + board.getItem(4).validate());

    System.out.println("--- makeConductive(7, 3)");
    Item newItem = (Item) board.makeConductive((ObstacleArea) board.getItem(7), 3);
    System.out.println(
        "newId=" + newItem.getId() + " nets=" + Arrays.toString(newItem.netNumbers) + " items=" + ids(board.getItems()));

    System.out.println("--- generateKeepoutOutside(true)");
    System.out.println(
        "outline tiles before=" + board.getOutline().tileShapeCount()
            + " treeShapes=" + board.getOutline().treeShapeCount(board.searchTreeManager.getDefaultTree()));
    board.getOutline().generateKeepoutOutside(true);
    System.out.println(
        "outline tiles after=" + board.getOutline().tileShapeCount()
            + " treeShapes=" + board.getOutline().treeShapeCount(board.searchTreeManager.getDefaultTree()));
    System.out.println("keepoutGenerated=" + board.getOutline().keepoutOutsideOutlineGenerated());

    System.out.println("--- net queries");
    Net n1 = board.rules.nets.get(1);
    System.out.println("net1 terminalItems=" + ids(n1.getTerminalItems()));
    System.out.println("net1 pins=" + ids(n1.getPins()));
    System.out.println("net1 items=" + ids(n1.getItems()));
    System.out.printf(Locale.ROOT, "net1 traceLength=%.6f%n", n1.getTraceLength());
    System.out.println("net1 viaCount=" + n1.getViaCount());
  }

  static String changedArea() {
    StringBuilder sb = new StringBuilder();
    for (int layer = 0; layer < board.getLayerCount(); layer++) {
      if (layer > 0) {
        sb.append(" ");
      }
      sb.append(layer).append(":").append(oct(board.changedArea.getArea(layer)));
    }
    return sb.toString();
  }


  // -------------------------------------------------------------------------------------------
  // Mode 7: `PolylineTrace.combine` — `combineAtStart` (:201-332), `combineAtEnd` (:341-456)
  // -------------------------------------------------------------------------------------------

  /** A bare board with no components, the shape `PolylineTraceSplitTest.createTestBoard` builds. */
  static RoutingBoard traceBoard(int layerCount) {
    Layer[] layers = new Layer[layerCount];
    for (int i = 0; i < layerCount; i++) {
      layers[i] = new Layer("l" + i, true);
    }
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 10);
    BoardRules rules = new BoardRules(ls, cm);
    rules.createDefaultNetClass();
    RoutingBoard b =
        new RoutingBoard(
            new IntBox(-2000000, -2000000, 2000000, 2000000),
            ls,
            new PolylineShape[] {
              new PolygonShape(
                  new Point[] {
                    new IntPoint(-1000000, -1000000),
                    new IntPoint(1000000, -1000000),
                    new IntPoint(1000000, 1000000),
                    new IntPoint(-1000000, 1000000)
                  })
            },
            0,
            rules,
            new Communication());
    b.rules.nets.add("N1", 1, false);
    b.rules.nets.add("N2", 1, false);
    b.library.padstacks = new app.freerouting.core.library.Padstacks(ls);
    ConvexShape[] viaShapes = new ConvexShape[layerCount];
    for (int i = 0; i < layerCount; i++) {
      viaShapes[i] = new IntBox(-300, -300, 300, 300);
    }
    tracePad = b.library.padstacks.add("via", viaShapes, true, false);
    return b;
  }

  static Point[] pts(int... xy) {
    Point[] result = new Point[xy.length / 2];
    for (int i = 0; i < result.length; i++) {
      result[i] = new IntPoint(xy[2 * i], xy[2 * i + 1]);
    }
    return result;
  }

  static PolylineTrace tr(int layer, int halfWidth, int net, FixedState fixed, int... xy) {
    return board.insertTraceWithoutCleaning(
        new Polyline(pts(xy)), layer, halfWidth, new int[] {net}, 1, fixed);
  }

  static String entryCount(Item item) {
    ShapeTree.Leaf[] entries = item.getSearchTreeEntries(board.searchTreeManager.getDefaultTree());
    return entries == null ? "null" : Integer.toString(entries.length);
  }

  static String corners(PolylineTrace t) {
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < t.cornerCount(); i++) {
      if (i > 0) {
        sb.append(" ");
      }
      sb.append(point(t.polyline().corner(i)));
    }
    return sb.append("]").toString();
  }

  static String traceLine(PolylineTrace t) {
    return "#"
        + t.getId()
        + " onBoard="
        + t.isOnTheBoard()
        + " layer="
        + t.getLayer()
        + " hw="
        + t.getHalfWidth()
        + " lines="
        + t.polyline().lines.length
        + " tiles="
        + t.tileShapeCount()
        + " entries="
        + entryCount(t)
        + " corners="
        + corners(t);
  }

  /** Every trace still in the item list, in the item list's own (descending id) order. */
  static String traces() {
    StringBuilder sb = new StringBuilder();
    boolean first = true;
    for (Item it : board.getItems()) {
      if (it instanceof PolylineTrace t) {
        if (!first) {
          sb.append(" | ");
        }
        first = false;
        sb.append(traceLine(t));
      }
    }
    return first ? "(none)" : sb.toString();
  }

  static void dumpCombine() {
    System.out.println("mode=7");

    // A: combineAtStart, straight order, two collinear segments (skipLine == true).
    board = traceBoard(1);
    PolylineTrace a2 = tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    System.out.println("A before: " + traces());
    System.out.println("A combine(#" + a2.getId() + ")=" + a2.combine());
    System.out.println("A after:  " + traces());
    System.out.println("A items=" + ids(board.getItems()) + " revision=" + board.getRevision());

    // B: combineAtStart, reverse order — the other trace starts at this trace's start corner.
    board = traceBoard(1);
    PolylineTrace b2 = tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 0, 0);
    System.out.println("B combine(#" + b2.getId() + ")=" + b2.combine());
    System.out.println("B after:  " + traces());

    // C: combineAtEnd, straight order, with the changed area being marked.
    board = traceBoard(1);
    board.startMarkingChangedArea();
    PolylineTrace c1 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    System.out.println("C combine(#" + c1.getId() + ")=" + c1.combine());
    System.out.println("C after:  " + traces());
    System.out.println("C changedArea=" + changedArea());

    // D: combineAtEnd, reverse order.
    board = traceBoard(1);
    PolylineTrace d1 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 20000, 0, 10000, 0);
    System.out.println("D combine(#" + d1.getId() + ")=" + d1.combine());
    System.out.println("D after:  " + traces());

    // E: combineAtEnd on a corner (skipLine == false — the join lines are not parallel).
    board = traceBoard(1);
    PolylineTrace e1 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 10000, 10000);
    System.out.println("E combine(#" + e1.getId() + ")=" + e1.combine());
    System.out.println("E after:  " + traces());

    // F: three traces meeting at one point — `contacts.size() != 1` at both ends.
    board = traceBoard(1);
    PolylineTrace f1 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 10000, 10000);
    System.out.println("F combine(#" + f1.getId() + ")=" + f1.combine());
    System.out.println("F after:  " + traces());

    // G: a different half width refuses.
    board = traceBoard(1);
    PolylineTrace g1 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    tr(0, 500, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    System.out.println("G combine(#" + g1.getId() + ")=" + g1.combine());
    System.out.println("G after:  " + traces());

    // H: a different fixed state refuses.
    board = traceBoard(1);
    PolylineTrace h1 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    tr(0, 1000, 1, FixedState.SHOVE_FIXED, 10000, 0, 20000, 0);
    System.out.println("H combine(#" + h1.getId() + ")=" + h1.combine());
    System.out.println("H after:  " + traces());

    // I: a different net refuses (`netsEqual`).
    board = traceBoard(1);
    PolylineTrace i1 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    tr(0, 1000, 2, FixedState.UNFIXED, 10000, 0, 20000, 0);
    System.out.println("I combine(#" + i1.getId() + ")=" + i1.combine());
    System.out.println("I after:  " + traces());

    // J: a chain of five collinear segments, combined from the middle — the iterative loop
    // (PolylineTrace.java:175-192) absorbs at the start first, then at the end, until neither
    // half can grow.
    board = traceBoard(1);
    tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    PolylineTrace j3 = tr(0, 1000, 1, FixedState.UNFIXED, 20000, 0, 30000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 30000, 0, 40000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 40000, 0, 50000, 0);
    System.out.println("J before: " + traces());
    System.out.println("J combine(#" + j3.getId() + ")=" + j3.combine());
    System.out.println("J after:  " + traces());

    // K: `PolylineTraceSplitTest.testCombineAtEndRecoversMissingDefaultTreeEntries` (:353-379) —
    // the trace has lost its default-tree entries, so `hasDefaultEntries` sends the join down the
    // `replaceGeometry` fallback instead of `mergeEntriesAtEnd`.
    board = traceBoard(1);
    PolylineTrace k1 = tr(0, 1000, 1, FixedState.UNFIXED, 10000, 10000, 20000, 10000);
    PolylineTrace k2 = tr(0, 1000, 1, FixedState.UNFIXED, 20000, 10000, 30000, 10000);
    board.searchTreeManager.remove(k1);
    k1.setOnTheBoard(true);
    System.out.println("K entriesBefore=" + entryCount(k1));
    System.out.println("K combine(#" + k1.getId() + ")=" + k1.combine());
    System.out.println("K firstOnBoard=" + k1.isOnTheBoard() + " secondOnBoard=" + k2.isOnTheBoard());
    System.out.println("K entriesAfter=" + entryCount(k1));
    System.out.println("K after:  " + traces());

    // L: an L-shaped three-corner trace absorbed at its start by a straight one.
    board = traceBoard(1);
    PolylineTrace l1 = tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 10000, 10000, 20000, 10000);
    tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    System.out.println("L combine(#" + l1.getId() + ")=" + l1.combine());
    System.out.println("L after:  " + traces());

    // M: a conduction area at the join is dropped by `ignoreAreas` (PolylineTrace.java:206-209),
    // so the two traces still see exactly one contact and combine.
    board = traceBoard(1);
    board.insertConductionArea(
        new IntBox(9000, -1000, 11000, 1000), 0, new int[] {1}, 1, true, FixedState.UNFIXED);
    PolylineTrace m1 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    System.out.println("M combine(#" + m1.getId() + ")=" + m1.combine());
    System.out.println("M after:  " + traces());
    System.out.println("M items=" + ids(board.getItems()));
  }


  // -------------------------------------------------------------------------------------------
  // Mode 8: `PolylineTrace.split(IntOctagon)` (:464-691) and `normalize` (:801-803)
  // -------------------------------------------------------------------------------------------

  /** The `Padstack` mode 8/9's vias use; `traceBoard` installs it into every board. */
  static Padstack tracePad;

  static String splitResult(Collection<PolylineTrace> pieces) {
    StringBuilder sb = new StringBuilder("[");
    boolean first = true;
    for (PolylineTrace t : pieces) {
      if (!first) {
        sb.append(" ");
      }
      first = false;
      sb.append("#").append(t.getId()).append(t.isOnTheBoard() ? "" : "(off)");
    }
    return sb.append("]").toString();
  }

  static void dumpSplitAndNormalize() {
    System.out.println("mode=8");

    // S1: `PolylineTraceSplitTest.testSplitPreservesNonOverlappingSegments` (:220-293) — a
    // four-corner trace and a second trace lying on its middle segment.
    board = traceBoard(1);
    PolylineTrace s1 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0, 20000, 0, 30000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    System.out.println("S1 before: " + traces());
    System.out.println("S1 split=" + splitResult(s1.split((IntOctagon) null)));
    System.out.println("S1 after:  " + traces());

    // S2: `PolylineTraceSplitTest.testSplitDoesNotRemoveValidSegments` (:61-216) — the bug-report
    // geometry: two traces combined, then an overlapping third inserted, then a split.
    board = traceBoard(1);
    tr(
        0, 1000, 98, FixedState.UNFIXED,
        1291423, -987076, 1270000, -975000, 1250000, -970000, 1243227, -964893);
    PolylineTrace s2b = tr(0, 1000, 98, FixedState.UNFIXED, 1243227, -964893, 1241414, -964893);
    System.out.println("S2 combine=" + s2b.combine());
    System.out.println("S2 combined: " + traces());
    PolylineTrace s2combined = null;
    for (Item it : board.getItems()) {
      if (it instanceof PolylineTrace t && t.containsNet(98) && t.isOnTheBoard()) {
        s2combined = t;
        break;
      }
    }
    System.out.println("S2 pick=#" + s2combined.getId() + " first=" + point(s2combined.firstCorner())
        + " last=" + point(s2combined.lastCorner()));
    tr(0, 1000, 98, FixedState.UNFIXED, 1243227, -964893, 1242000, -960000, 1241171, -952775);
    System.out.println("S2 split=" + splitResult(s2combined.split((IntOctagon) null)));
    System.out.println("S2 after:  " + traces());

    // S3: the same S1 board, but a `clipShape` that misses the overlap entirely
    // (PolylineTrace.java:475-479).
    board = traceBoard(1);
    PolylineTrace s3 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0, 20000, 0, 30000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    IntOctagon clipAway = new IntBox(100000, 100000, 110000, 110000).boundingOctagon();
    System.out.println("S3 split(clip away)=" + splitResult(s3.split(clipAway)));
    System.out.println("S3 after:  " + traces());
    IntOctagon clipOver = new IntBox(-1000, -1000, 31000, 1000).boundingOctagon();
    System.out.println("S3 split(clip over)=" + splitResult(s3.split(clipOver)));
    System.out.println("S3 after2: " + traces());

    // S4: the `DrillItem` branch (PolylineTrace.java:649-661) — a via sitting in the middle of a
    // one-segment trace. `split(i + 1, splitLine)`'s result is discarded, so `ownTraceSplit`
    // stays false and the collection Java returns is *not* the two pieces.
    board = traceBoard(1);
    board.insertVia(tracePad, new IntPoint(10000, 0), new int[] {1}, 1, FixedState.UNFIXED, true);
    PolylineTrace s4 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 20000, 0);
    System.out.println("S4 before: " + traces());
    System.out.println("S4 split=" + splitResult(s4.split((IntOctagon) null)));
    System.out.println("S4 after:  " + traces());
    System.out.println("S4 items=" + ids(board.getItems()));

    // S5: `normalize(null)` over the S1 geometry — split, then combine each piece.
    board = traceBoard(1);
    PolylineTrace s5 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0, 20000, 0, 30000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    System.out.println("S5 normalize=" + s5.normalize(null));
    System.out.println("S5 after:  " + traces());

    // S6: normalize on a board where nothing overlaps — no split, no combine.
    board = traceBoard(1);
    PolylineTrace s6 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    System.out.println("S6 normalize=" + s6.normalize(null));
    System.out.println("S6 after:  " + traces());

    // S7: normalize where the only change is a combine (PolylineTraceNormalization.java:122-125).
    board = traceBoard(1);
    PolylineTrace s7 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    System.out.println("S7 normalize=" + s7.normalize(null));
    System.out.println("S7 after:  " + traces());

    // S8: the conduction-area cycle branch (PolylineTrace.java:662-681) — both end corners of the
    // trace are contacts of the same-net area, so the trace is removed and the result is empty.
    board = traceBoard(1);
    board.insertConductionArea(
        new IntBox(-1000, -1000, 21000, 1000), 0, new int[] {1}, 1, true, FixedState.UNFIXED);
    PolylineTrace s8 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 20000, 0);
    System.out.println("S8 split=" + splitResult(s8.split((IntOctagon) null)));
    System.out.println("S8 onBoard=" + s8.isOnTheBoard() + " items=" + ids(board.getItems()));

    // S9: a trace of a non-normal net is never split (PolylineTrace.java:466-470).
    board = traceBoard(1);
    PolylineTrace s9 = tr(0, 1000, 0, FixedState.UNFIXED, 0, 0, 10000, 0, 20000, 0, 30000, 0);
    tr(0, 1000, 0, FixedState.UNFIXED, 10000, 0, 20000, 0);
    System.out.println("S9 split=" + splitResult(s9.split((IntOctagon) null)));
    System.out.println("S9 after:  " + traces());

    // S10: two traces crossing at right angles — both get split at the crossing point.
    board = traceBoard(1);
    PolylineTrace s10 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 20000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, -10000, 10000, 10000);
    System.out.println("S10 before: " + traces());
    System.out.println("S10 split=" + splitResult(s10.split((IntOctagon) null)));
    System.out.println("S10 after:  " + traces());

    // S11: a USER_FIXED trace refuses to split (`isDeletionForbidden`, PolylineTrace.java:727).
    board = traceBoard(1);
    PolylineTrace s11 =
        tr(0, 1000, 1, FixedState.USER_FIXED, 0, 0, 10000, 0, 20000, 0, 30000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    System.out.println("S11 split=" + splitResult(s11.split((IntOctagon) null)));
    System.out.println("S11 after:  " + traces());
    System.out.println("S11 normalize=" + s11.normalize(null));
    System.out.println("S11 after2: " + traces());

    // S12: `PolylineTrace.change` (PolylineTrace.java:936-1005) on a live trace. The new
    // polyline's very first line differs, so both `indexOfFirstDifferentLine` and the port's
    // structural comparison answer 0 and the whole entry array is rebuilt.
    board = traceBoard(1);
    PolylineTrace s12 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0, 20000, 0);
    System.out.println("S12 before: " + traces());
    s12.change(new Polyline(pts(0, 0, 10000, 5000, 20000, 0)));
    System.out.println("S12 after:  " + traces());

    // S13: `change` on a trace that is not on the board just swaps the polyline (:937-941).
    board = traceBoard(1);
    PolylineTrace s13 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    board.searchTreeManager.remove(s13);
    s13.change(new Polyline(pts(0, 0, 30000, 0)));
    System.out.println(
        "S13 onBoard=" + s13.isOnTheBoard() + " corners=" + corners(s13)
            + " entries=" + entryCount(s13));
    System.out.println("S13 items=" + ids(board.getItems()));

    // S14: quirk #22 reached through `combineAtStart`. The joined line array is
    // `[D, C, B, C, D, X, Y]`, on which `Polyline.removeOverlaps` cancels its way down to
    // `newLength == 0` and reads `tmpArr[-1]` (Polyline.java:148). Java throws out of
    // `combineAtStart`; the port answers `BoardError::Normalization`. Neither touches the board.
    board = traceBoard(1);
    Line lineA = new Line(new IntPoint(0, 0), new IntPoint(1000, 0));
    Line lineB = new Line(new IntPoint(0, 0), new IntPoint(0, 1000));
    Line lineC = new Line(new IntPoint(0, 0), new IntPoint(1000, 1000));
    Line lineD = new Line(new IntPoint(2000, 2000), new IntPoint(3000, 2000));
    Line lineX = new Line(new IntPoint(4000, 2000), new IntPoint(4000, 3000));
    Line lineY = new Line(new IntPoint(4000, 5000), new IntPoint(5000, 5000));
    PolylineTrace s14 =
        board.insertTraceWithoutCleaning(
            new Polyline(new Line[] {lineA, lineB, lineC, lineD, lineX, lineY}),
            0,
            100,
            new int[] {1},
            1,
            FixedState.UNFIXED);
    board.insertTraceWithoutCleaning(
        new Polyline(new Line[] {lineD, lineC, lineB}),
        0,
        100,
        new int[] {1},
        1,
        FixedState.UNFIXED);
    System.out.println("S14 before: " + traces());
    try {
      System.out.println("S14 combine=" + s14.combine());
    } catch (RuntimeException e) {
      System.out.println("S14 combine=threw " + e.getClass().getSimpleName());
    }
    System.out.println("S14 after:  " + traces());

    // S15: the same board through `normalize`, `normalizeTraces` and `normalizeAllTraces` — the
    // outer loops have no catch of their own (BasicBoard.java:709-795,798-885), so whatever
    // `split`/`combine` does to the geometry first decides whether the exception still escapes.
    board = traceBoard(1);
    PolylineTrace s15 =
        board.insertTraceWithoutCleaning(
            new Polyline(new Line[] {lineA, lineB, lineC, lineD, lineX, lineY}),
            0,
            100,
            new int[] {1},
            1,
            FixedState.UNFIXED);
    board.insertTraceWithoutCleaning(
        new Polyline(new Line[] {lineD, lineC, lineB}),
        0,
        100,
        new int[] {1},
        1,
        FixedState.UNFIXED);
    try {
      System.out.println("S15 normalize=" + s15.normalize(null));
    } catch (RuntimeException e) {
      System.out.println("S15 normalize=threw " + e.getClass().getSimpleName());
    }
    System.out.println("S15 after:  " + traces());
    try {
      System.out.println("S15 normalizeTraces(1)=" + board.normalizeTraces(1));
    } catch (RuntimeException e) {
      System.out.println("S15 normalizeTraces(1)=threw " + e.getClass().getSimpleName());
    }
    System.out.println("S15 after2: " + traces());

    // S16: `insertTrace`'s own `catch (Exception)` (BasicBoard.java:230-241) — the trace is
    // inserted and kept even though its normalisation blew up.
    board = traceBoard(1);
    board.insertTraceWithoutCleaning(
        new Polyline(new Line[] {lineA, lineB, lineC, lineD, lineX, lineY}),
        0,
        100,
        new int[] {1},
        1,
        FixedState.UNFIXED);
    board.insertTrace(
        new Polyline(new Line[] {lineD, lineC, lineB}), 0, 100, new int[] {1}, 1,
        FixedState.UNFIXED);
    System.out.println("S16 after:  " + traces());
    System.out.println("S16 items=" + ids(board.getItems()));
  }

  // -------------------------------------------------------------------------------------------
  // Mode 9: `BasicBoard`'s four normalisation loops, plus the five methods whose bodies end in
  // one of them
  // -------------------------------------------------------------------------------------------

  static void dumpBoardNormalizationLoops() {
    System.out.println("mode=9");

    // N1: `insertTrace(Polyline, …)` (BasicBoard.java:209-242) — the `normalize` tail runs.
    board = traceBoard(1);
    tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    board.insertTrace(
        new Polyline(pts(10000, 0, 20000, 0)), 0, 1000, new int[] {1}, 1, FixedState.UNFIXED);
    System.out.println("N1 after:  " + traces());

    // N2: the same through `insertTrace(Point[], …)` (:248-262), with the changed area on so the
    // `clipShape` branch (:224-229) is taken.
    board = traceBoard(1);
    board.startMarkingChangedArea();
    board.markAllChangedArea();
    tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    board.insertTrace(pts(10000, 0, 20000, 0), 0, 1000, new int[] {1}, 1, FixedState.UNFIXED);
    System.out.println("N2 after:  " + traces());

    // N3: `combineTraces(netNumber)` (:683-706) over a five-segment chain, and over all nets.
    board = traceBoard(1);
    for (int i = 0; i < 5; i++) {
      tr(0, 1000, 1, FixedState.UNFIXED, i * 10000, 0, (i + 1) * 10000, 0);
    }
    tr(0, 1000, 2, FixedState.UNFIXED, 0, 50000, 10000, 50000);
    tr(0, 1000, 2, FixedState.UNFIXED, 10000, 50000, 20000, 50000);
    System.out.println("N3 before: " + traces());
    System.out.println("N3 combineTraces(1)=" + board.combineTraces(1));
    System.out.println("N3 after:  " + traces());
    System.out.println("N3 combineTraces(-1)=" + board.combineTraces(-1));
    System.out.println("N3 after2: " + traces());
    System.out.println("N3 combineTraces(-1) again=" + board.combineTraces(-1));

    // N4: `normalizeTraces(netNumber)` (:709-795).
    board = traceBoard(1);
    tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0, 20000, 0, 30000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    System.out.println("N4 before: " + traces());
    System.out.println("N4 normalizeTraces(1)=" + board.normalizeTraces(1));
    System.out.println("N4 after:  " + traces());
    System.out.println("N4 normalizeTraces(1) again=" + board.normalizeTraces(1));
    System.out.println("N4 after2: " + traces());

    // N5: `normalizeAllTraces()` (:798-885) over two nets at once.
    board = traceBoard(1);
    tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0, 20000, 0, 30000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    tr(0, 1000, 2, FixedState.UNFIXED, 0, 50000, 10000, 50000);
    tr(0, 1000, 2, FixedState.UNFIXED, 10000, 50000, 20000, 50000);
    System.out.println("N5 before: " + traces());
    System.out.println("N5 normalizeAllTraces=" + board.normalizeAllTraces());
    System.out.println("N5 after:  " + traces());
    System.out.println("N5 normalizeAllTraces again=" + board.normalizeAllTraces());

    // N6: `splitTraces(location, layer, netNumber)` (:891-907).
    board = traceBoard(1);
    tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 20000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, -10000, 10000, 10000);
    System.out.println("N6 before: " + traces());
    System.out.println("N6 splitTraces(hit)=" + board.splitTraces(new IntPoint(10000, 0), 0, 1));
    System.out.println("N6 after:  " + traces());
    System.out.println("N6 splitTraces(miss)=" + board.splitTraces(new IntPoint(90000, 0), 0, 1));

    // N7: `insertVia`'s `splitTraces` loop (:287-293) — the via lands on an existing trace.
    board = traceBoard(2);
    tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 20000, 0);
    System.out.println("N7 before: " + traces());
    Via n7via =
        board.insertVia(tracePad, new IntPoint(10000, 0), new int[] {1}, 1, FixedState.UNFIXED, true);
    System.out.println("N7 via=#" + n7via.getId());
    System.out.println("N7 after:  " + traces());
    System.out.println("N7 items=" + ids(board.getItems()));

    // N8: `RoutingBoard.connectToTrace` (:1116-1170) — its `insertTrace` tail normalises.
    board = traceBoard(1);
    PolylineTrace n8 = tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 20000, 0);
    System.out.println(
        "N8 connectToTrace=" + board.connectToTrace(new IntPoint(10000, 5000), n8, 1000, 1));
    System.out.println("N8 after:  " + traces());
    System.out.println("N8 items=" + ids(board.getItems()));

    // N9: `RoutingBoard.removeTraceTails` (:1193-1238) — its `combineTraces(netNumber)` tail
    // (:1236) runs once the stub is gone, joining the two traces the stub used to fork.
    board = traceBoard(1);
    tr(0, 1000, 1, FixedState.UNFIXED, 0, 0, 10000, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 10000, 10000);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 10000, 0, 0);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    System.out.println("N9 before: " + traces());
    System.out.println(
        "N9 removeTraceTails=" + board.removeTraceTails(1, Item.StopConnectionOption.NONE));
    System.out.println("N9 after:  " + traces());

    // N10: `DrillItem.moveBy`'s `insertTrace` tail (DrillItem.java:137-143) — a via with a
    // contacting trace, moved.
    board = traceBoard(1);
    Via n10via =
        board.insertVia(tracePad, new IntPoint(10000, 0), new int[] {1}, 1, FixedState.UNFIXED, true);
    tr(0, 1000, 1, FixedState.UNFIXED, 10000, 0, 20000, 0);
    System.out.println("N10 before: " + traces());
    n10via.moveBy(new IntVector(0, 10000));
    System.out.println("N10 after:  " + traces());
    System.out.println("N10 items=" + ids(board.getItems()));
  }


  // -------------------------------------------------------------------------------------------
  // Mode 10: the `CombineStackOverflowTest` fixture, rebuilt by hand
  // -------------------------------------------------------------------------------------------

  /**
   * The wiring of `fixtures/Issue723-CombineStackOverflow.dsn`, generated rather than parsed: a
   * single GND net drawn as a boustrophedon of 200-unit collinear segments, 280 horizontal per
   * row plus one vertical connector, starting at (130000, -107000) — exactly the 4000
   * `(wire (path F.Cu 152.4 …))` entries of the fixture, in the same order.
   *
   * <p>`src/test/java/app/freerouting/fixtures/CombineStackOverflowTest.java` drives this through
   * `DsnReader.readBoard`, which is Plan 3; the two board calls it ends in are
   * `insertTraceWithoutCleaning` per wire (Wiring.java:530-535) and one `normalizeAllTraces`
   * (Wiring.java:347).
   */
  static void dumpCombineStackOverflow(int segmentCount) {
    System.out.println("mode=10 segments=" + segmentCount);
    board = traceBoard(1);
    int x = 130000;
    int y = -107000;
    int dx = 200;
    int emitted = 0;
    int inRow = 0;
    while (emitted < segmentCount) {
      int nextX = x;
      int nextY = y;
      if (inRow < 280) {
        nextX = x + dx;
        inRow++;
      } else {
        nextY = y + 200;
        inRow = 0;
        dx = -dx;
      }
      board.insertTraceWithoutCleaning(
          new Polyline(new Point[] {new IntPoint(x, y), new IntPoint(nextX, nextY)}),
          0,
          76,
          new int[] {1},
          1,
          FixedState.UNFIXED);
      x = nextX;
      y = nextY;
      emitted++;
    }
    System.out.println("inserted=" + board.getTraces().size() + " lastCorner=" + point(new IntPoint(x, y)));
    boolean changed = board.normalizeAllTraces();
    System.out.println("normalizeAllTraces=" + changed);
    System.out.println("traces=" + board.getTraces().size());
    System.out.println("after: " + traces());
  }

  // -------------------------------------------------------------------------------------------
  // Formatting helpers — every one of these has an exact twin in the Rust driver.
  // -------------------------------------------------------------------------------------------

  static String box(IntBox b) {
    return "Box[" + b.ll.x + "," + b.ll.y + ".." + b.ur.x + "," + b.ur.y + "]";
  }

  static String oct(IntOctagon o) {
    return "Oct["
        + o.leftX + "," + o.bottomY + "," + o.rightX + "," + o.topY + ","
        + o.upperLeftDiagonalX + "," + o.lowerRightDiagonalX + ","
        + o.lowerLeftDiagonalX + "," + o.upperRightDiagonalX + "]";
  }

  static String point(Point p) {
    if (p == null) {
      return "null";
    }
    IntPoint ip = (IntPoint) p;
    return "(" + ip.x + "," + ip.y + ")";
  }

  static String points(Point[] arr) {
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < arr.length; i++) {
      if (i > 0) {
        sb.append(" ");
      }
      sb.append(point(arr[i]));
    }
    return sb.append("]").toString();
  }

  static String itemId(Item item) {
    return item == null ? "null" : Integer.toString(item.getId());
  }

  static String ids(Collection<? extends Item> items) {
    StringBuilder sb = new StringBuilder("[");
    boolean first = true;
    for (Item it : items) {
      if (!first) {
        sb.append(" ");
      }
      first = false;
      sb.append(it.getId());
    }
    return sb.append("]").toString();
  }

  static String objectIds(Collection<SearchTreeObject> objects) {
    StringBuilder sb = new StringBuilder("[");
    boolean first = true;
    for (SearchTreeObject o : objects) {
      if (!first) {
        sb.append(" ");
      }
      first = false;
      sb.append(((Item) o).getId());
    }
    return sb.append("]").toString();
  }
}
