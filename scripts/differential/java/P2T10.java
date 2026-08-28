package app.freerouting.datastructures;

import app.freerouting.board.facade.BasicBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.structure.*;
import app.freerouting.board.searchtree.SearchTreeObject;
import app.freerouting.board.searchtree.ShapeSearchTree;
import app.freerouting.board.state.Communication;
import app.freerouting.board.trace.PolylineTrace;
import app.freerouting.core.library.Package;
import app.freerouting.core.library.Padstack;
import app.freerouting.geometry.planar.*;
import app.freerouting.rules.*;
import java.util.*;

/** Plan 2 Task 10: ShapeSearchTree / SearchTreeManager differential driver. */
public class P2T10 {

  static BasicBoard board;

  public static void main(String[] args) {
    int mode = args.length > 0 ? Integer.parseInt(args[0]) : 0;
    build(mode);
    if (mode == 3) {
      mutate();
    } else {
      dump(mode);
    }
  }

  static void build(int mode) {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    // A second, wider clearance class so maxValue() and the sort-by-clearance path are not
    // degenerate.
    cm.appendClass("wide");
    cm.setValue(2, 1, 600);
    cm.setValue(2, 2, 800);
    BoardRules rules = new BoardRules(ls, cm);
    if (mode == 1) {
      rules.setTraceAngleRestriction(AngleRestriction.NINETY_DEGREE);
    } else if (mode == 2) {
      rules.setTraceAngleRestriction(AngleRestriction.NONE);
    }
    Communication comm = new Communication();
    IntBox bbox = new IntBox(-10000, -10000, 10000, 10000);
    board = new BasicBoard(bbox, ls, new PolylineShape[0], 0, rules, comm);
    board.library.padstacks = new app.freerouting.core.library.Padstacks(ls);
    board.library.packages = new app.freerouting.core.library.Packages(board.library.padstacks);

    // Two padstacks: a square SMD pad on layer 0 only, and a through octagon on both layers.
    ConvexShape[] smd = {new IntBox(-50, -50, 50, 50), null};
    Padstack smdPad = board.library.padstacks.add("smd", smd, false, false);
    ConvexShape[] thru = {
      new IntOctagon(-70, -70, 70, 70, -140, 140, -140, 140),
      new IntOctagon(-70, -70, 70, 70, -140, 140, -140, 140)
    };
    Padstack thruPad = board.library.padstacks.add("thru", thru, true, false);

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

    // Two pins on net 1, clearance class 1 (default).
    board.insertPin(1, 0, new int[] {1}, 1, FixedState.UNFIXED);
    board.insertPin(1, 1, new int[] {1}, 1, FixedState.UNFIXED);
    // A trace on net 1, layer 0, half width 30, clearance class 1.
    Polyline traceLine =
        new Polyline(
            new Point[] {
              new IntPoint(-500, 0), new IntPoint(0, 0), new IntPoint(0, 400), new IntPoint(500, 400)
            });
    board.insertTraceWithoutCleaning(traceLine, 0, 30, new int[] {1}, 1, FixedState.UNFIXED);
    // A trace on net 2, layer 0, clearance class 2 (the wide class).
    Polyline other =
        new Polyline(
            new Point[] {new IntPoint(-800, 300), new IntPoint(-800, 900), new IntPoint(300, 900)});
    board.insertTraceWithoutCleaning(other, 0, 40, new int[] {2}, 2, FixedState.UNFIXED);
  }

  static void dump(int mode) {
    System.out.println("mode=" + mode + " angle=" + board.rules.getTraceAngleRestriction());
    for (Item it : board.getItems()) {
      System.out.println(
          "item id="
              + it.getId()
              + " class="
              + it.getClass().getSimpleName()
              + " nets="
              + Arrays.toString(it.netNumbers)
              + " cc="
              + it.clearanceClassIndex()
              + " layers="
              + it.firstLayer()
              + ".."
              + it.lastLayer()
              + " tileShapeCount="
              + it.tileShapeCount()
              + " bbox="
              + b(it.boundingBox()));
    }
    ShapeSearchTree def = board.searchTreeManager.getDefaultTree();
    dumpTree("default", def);
    ShapeSearchTree auto = board.searchTreeManager.getAutorouteTree(1);
    dumpTree("autoroute_cc1", auto);
    ShapeSearchTree auto2 = board.searchTreeManager.getAutorouteTree(2);
    dumpTree("autoroute_cc2", auto2);

    // Queries against the default tree.
    TileShape probe = new IntBox(-600, -100, 600, 500);
    query("default", def, probe, 0, new int[0], 1);
    query("default", def, probe, 0, new int[] {1}, 1);
    query("default", def, probe, -1, new int[0], 1);
    query("default", def, probe, 1, new int[0], 1);
    query("default", def, probe, 0, new int[0], 2);
    query("autoroute_cc1", auto, probe, 0, new int[0], 1);

    TileShape smallProbe = new IntBox(-520, -20, -480, 20);
    query("default", def, smallProbe, 0, new int[0], 1);
    query("default", def, smallProbe, 0, new int[] {1}, 1);
  }

  /** Mode 3: the in-place mutation methods and the clearance-compensation switch. */
  static void mutate() {
    ClearanceMatrix cm = board.rules.clearanceMatrix;
    System.out.println("matrix classCount=" + cm.getClassCount());
    for (int i = 0; i < cm.getClassCount(); i++) {
      StringBuilder sb = new StringBuilder("  getValue(" + i + ",j,0,false):");
      for (int j = 0; j < cm.getClassCount(); j++) {
        sb.append(' ').append(cm.getValue(i, j, 0, false));
      }
      sb.append(" | withMargin:");
      for (int j = 0; j < cm.getClassCount(); j++) {
        sb.append(' ').append(cm.getValue(i, j, 0, true));
      }
      sb.append(" | maxValue=").append(cm.maxValue(i, 0));
      sb.append(" | compensation=").append(cm.clearanceCompensationValue(i, 0));
      System.out.println(sb);
    }

    ShapeSearchTree def = board.searchTreeManager.getDefaultTree();
    for (int cc = 0; cc <= 2; cc++) {
      System.out.println(
          "clearanceCompensationValue tree=" + def + " cc=" + cc + " layer0="
              + def.clearanceCompensationValue(cc, 0));
    }
    ShapeSearchTree a1 = board.searchTreeManager.getAutorouteTree(1);
    for (int cc = 0; cc <= 2; cc++) {
      System.out.println(
          "clearanceCompensationValue tree=" + a1 + " cc=" + cc + " layer0="
              + a1.clearanceCompensationValue(cc, 0));
    }
    ShapeSearchTree a2 = board.searchTreeManager.getAutorouteTree(2);
    for (int cc = 0; cc <= 2; cc++) {
      System.out.println(
          "clearanceCompensationValue tree=" + a2 + " cc=" + cc + " layer0="
              + a2.clearanceCompensationValue(cc, 0));
    }

    PolylineTrace trace = (PolylineTrace) board.getItem(4);
    System.out.println("validateEntries(4)=" + def.validateEntries(trace));
    System.out.println("compensatedHalfWidth(4, default)=" + trace.getCompensatedHalfWidth(def));
    System.out.println("compensatedHalfWidth(4, auto1)=" + trace.getCompensatedHalfWidth(a1));

    System.out.println("--- changeItemShape(trace 4, 1, Box[-20,-20..20,420])");
    def.changeItemShape(trace, 1, new IntBox(-20, -20, 20, 420));
    dumpTree("default_after_changeItemShape", def);

    System.out.println("--- changeEntries(trace 4, shifted polyline, keepStart=1, keepEnd=1)");
    Polyline shifted =
        new Polyline(
            new Point[] {
              new IntPoint(-500, 0), new IntPoint(0, 0), new IntPoint(0, 600), new IntPoint(500, 600)
            });
    def.changeEntries(trace, shifted, 1, 1);
    dumpTree("default_after_changeEntries", def);
    System.out.println("validateEntries(4)=" + def.validateEntries(trace));

    System.out.println("--- setClearanceCompensationUsed(true)");
    board.searchTreeManager.setClearanceCompensationUsed(true);
    System.out.println("isClearanceCompensationUsed=" + board.searchTreeManager.isClearanceCompensationUsed());
    dumpTree("default_compensated", board.searchTreeManager.getDefaultTree());
    query("default_compensated", board.searchTreeManager.getDefaultTree(),
        new IntBox(-600, -100, 600, 500), 0, new int[0], 1);
  }

  static void dumpTree(String label, ShapeSearchTree t) {
    System.out.println("tree " + label + " key=" + t + " size=" + t.size());
    ShapeTree.Leaf[] leaves = t.toArray();
    for (ShapeTree.Leaf leaf : leaves) {
      System.out.println(
          "  leaf obj="
              + objId(leaf.object)
              + " idx="
              + leaf.shapeIndexInObject
              + " boundsClass="
              + leaf.boundingShape.getClass().getSimpleName()
              + " bounds="
              + shp(leaf.boundingShape));
    }
    for (Item it : board.getItems()) {
      int n = it.treeShapeCount(t);
      StringBuilder sb = new StringBuilder();
      for (int i = 0; i < n; i++) {
        TileShape s = it.getTreeShape(t, i);
        sb.append(" [").append(i).append("]=").append(s == null ? "null" : shp(s));
      }
      System.out.println("  shapes id=" + it.getId() + " n=" + n + sb);
    }
  }

  static void query(String label, ShapeSearchTree t, TileShape shape, int layer, int[] ignore, int cc) {
    StringBuilder head =
        new StringBuilder(
            "query tree=" + label + " shape=" + shp(shape) + " layer=" + layer + " ignore="
                + Arrays.toString(ignore) + " cc=" + cc);
    System.out.println(head);
    Set<SearchTreeObject> objs = new TreeSet<>();
    t.overlappingObjects(shape, layer, ignore, objs);
    StringBuilder sb = new StringBuilder("  overlappingObjects:");
    for (SearchTreeObject o : objs) {
      sb.append(' ').append(objId(o));
    }
    System.out.println(sb);

    Collection<ShapeTree.TreeEntry> entries = new LinkedList<>();
    t.overlappingTreeEntries(shape, layer, ignore, entries);
    sb = new StringBuilder("  overlappingTreeEntries:");
    for (ShapeTree.TreeEntry e : entries) {
      sb.append(' ').append(objId(e.object)).append('/').append(e.shapeIndexInObject);
    }
    System.out.println(sb);

    Collection<ShapeTree.TreeEntry> cl = new LinkedList<>();
    t.overlappingTreeEntriesWithClearance(shape, layer, ignore, cc, cl);
    sb = new StringBuilder("  overlappingTreeEntriesWithClearance:");
    for (ShapeTree.TreeEntry e : cl) {
      sb.append(' ').append(objId(e.object)).append('/').append(e.shapeIndexInObject);
    }
    System.out.println(sb);

    Set<Item> items = t.overlappingItemsWithClearance(shape, layer, ignore, cc);
    sb = new StringBuilder("  overlappingItemsWithClearance:");
    for (Item i : items) {
      sb.append(' ').append(i.getId());
    }
    System.out.println(sb);
  }

  static String objId(Object o) {
    return o instanceof Item item ? String.valueOf(item.getId()) : String.valueOf(o);
  }

  static String b(IntBox x) {
    return "[" + x.ll.x + "," + x.ll.y + ".." + x.ur.x + "," + x.ur.y + "]";
  }

  static String shp(Shape s) {
    if (s instanceof IntBox x) {
      return "Box" + b(x);
    }
    if (s instanceof IntOctagon o) {
      return "Oct["
          + o.leftX + "," + o.bottomY + "," + o.rightX + "," + o.topY + ","
          + o.upperLeftDiagonalX + "," + o.lowerRightDiagonalX + ","
          + o.lowerLeftDiagonalX + "," + o.upperRightDiagonalX + "]";
    }
    if (s instanceof Simplex sx) {
      StringBuilder sb = new StringBuilder("Simplex" + b(sx.boundingBox()) + "{");
      for (int i = 0; i < sx.borderLineCount(); i++) {
        Line l = sx.borderLine(i);
        sb.append("(").append(l.a.toString()).append("->").append(l.b.toString()).append(")");
      }
      return sb.append("}").toString();
    }
    return s.getClass().getSimpleName() + b(s.boundingBox());
  }
}
