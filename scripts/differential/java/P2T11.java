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
 * the conduction/net bookkeeping.
 */
public class P2T11 {

  static RoutingBoard board;
  static Padstack smdPad;
  static Padstack thruPad;

  public static void main(String[] args) {
    int mode = args.length > 0 ? Integer.parseInt(args[0]) : 0;
    build();
    switch (mode) {
      case 0 -> dumpInsertRemove();
      case 1 -> dumpConnectivity();
      case 2 -> dumpChecks();
      case 3 -> dumpChangedArea();
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
    Communication comm = new Communication();
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
