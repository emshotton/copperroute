package app.freerouting.datastructures;

import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.structure.*;
import app.freerouting.board.searchtree.SearchTreeObject;
import app.freerouting.board.state.Communication;
import app.freerouting.core.library.Package;
import app.freerouting.core.library.Padstack;
import app.freerouting.geometry.planar.*;
import app.freerouting.rules.*;
import java.util.*;

/**
 * Plan 2 Task 15: the ONE randomised board-level Java/Rust differential driver. Builds a real
 * `app.freerouting.board.facade.RoutingBoard` (two layers, two padstacks, one package, four nets,
 * two clearance classes) programmatically, then drives a shared xorshift stream through
 * `insertPin`/`insertVia`/`insertTraceWithoutCleaning`/`insertObstacle`/`insertConductionArea` `n`
 * times (plus three fixed obstacle areas and three fixed conduction areas), runs
 * `normalizeAllTraces`, and dumps:
 *
 * <ul>
 *   <li>every item, in board order (descending id, quirk #63): id, kind, layer range, nets,
 *       clearance class, bounding box, tile shape count and each tile's bounding box;
 *   <li>the default tree's `overlappingObjects` for 50 random query boxes on each of the two
 *       layers;
 *   <li>`overlappingTreeEntriesWithClearance` as `(id, shapeIndex)` lists for 50 random
 *       shapes/layers/clearance classes/ignore-net arrays;
 *   <li>`deepCopy`, then the same 150 queries replayed against the copy;
 *   <li>a `hashEqual` boolean (`getHash().equals(...)`, not the hash values themselves — Java's
 *       hash and the port's `structural_hash` are not byte-comparable, `docs/java-quirks.md`).
 * </ul>
 *
 * <p>No `toArray()`/tree-internals dump (unlike `P2T10`/`P2T11`, this driver stays in the
 * `app.freerouting.datastructures` package only so `run.sh` can reuse the same jar-based
 * compilation path; it never reaches into `ShapeTree`'s protected fields) — every query here goes
 * through public API, so quirk #77's tree-layout lines never appear and the driver is zero-diff.
 *
 * <p>args: seed n. `run.sh p2t15 <seed> <n>` diffs this against `p2t15.rs`.
 */
public class P2T15 {

  static long state;

  static long next() {
    state ^= state << 13;
    state ^= state >>> 7;
    state ^= state << 17;
    return state;
  }

  static int rnd(int bound) {
    return (int) Long.remainderUnsigned(next(), bound);
  }

  static final int RANGE = 12000;
  static final int QUERY_RANGE = 15000;

  static int randCoord(int range) {
    return rnd(2 * range + 1) - range;
  }

  static IntPoint randomPoint(int range) {
    return new IntPoint(randCoord(range), randCoord(range));
  }

  static IntBox randomBox(int range, int minSize, int maxSize) {
    int w = minSize + rnd(maxSize - minSize + 1);
    int h = minSize + rnd(maxSize - minSize + 1);
    int x = randCoord(range);
    int y = randCoord(range);
    return new IntBox(x, y, x + w, y + h);
  }

  static RoutingBoard board;
  static Padstack smdPad;
  static Padstack thruPad;
  static Package pkg;

  public static void main(String[] args) {
    long seed = args.length > 0 ? Long.parseLong(args[0]) : 42L;
    int n = args.length > 1 ? Integer.parseInt(args[1]) : 30;
    state = seed == 0 ? 0x9E3779B97F4A7C15L : seed;

    build();

    for (int i = 0; i < n; i++) {
      int roll = rnd(100);
      if (roll < 40) {
        insertRandomPin();
      } else if (roll < 70) {
        insertRandomVia();
      } else {
        insertRandomTrace();
      }
    }
    for (int i = 0; i < 3; i++) {
      insertRandomObstacle();
    }
    for (int i = 0; i < 3; i++) {
      insertRandomConduction();
    }

    boolean normalized = board.normalizeAllTraces();

    System.out.println("mode=p2t15 seed=" + seed + " n=" + n);
    System.out.println("normalizeAllTraces=" + normalized);
    System.out.println("itemCount=" + board.getItems().size());

    dumpItems(board);

    List<QuerySpec> overlapQueries = new ArrayList<>();
    for (int layer = 0; layer < 2; layer++) {
      for (int i = 0; i < 50; i++) {
        overlapQueries.add(new QuerySpec(randomBox(QUERY_RANGE, 50, 3000), layer));
      }
    }
    dumpOverlapQueries("overlap", board, overlapQueries);

    List<ClearanceQuerySpec> clearanceQueries = new ArrayList<>();
    for (int i = 0; i < 50; i++) {
      IntBox box = randomBox(QUERY_RANGE, 50, 3000);
      int layer = rnd(2);
      int cc = 1 + rnd(2);
      int[] ignore = rnd(4) == 0 ? new int[] {1 + rnd(4)} : new int[0];
      clearanceQueries.add(new ClearanceQuerySpec(box, layer, cc, ignore));
    }
    dumpClearanceQueries("clearance", board, clearanceQueries);

    System.out.println("--- deepCopy");
    RoutingBoard copy = board.deepCopy();
    dumpOverlapQueries("copyOverlap", copy, overlapQueries);
    dumpClearanceQueries("copyClearance", copy, clearanceQueries);
    System.out.println("hashEqual=" + board.getHash().equals(copy.getHash()));
  }

  // -------------------------------------------------------------------------------------------
  // The board
  // -------------------------------------------------------------------------------------------

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
    IntBox bbox = new IntBox(-20000, -20000, 20000, 20000);
    PolylineShape[] outline = {
      new PolygonShape(
          new Point[] {
            new IntPoint(-15000, -15000),
            new IntPoint(15000, -15000),
            new IntPoint(15000, 15000),
            new IntPoint(-15000, 15000)
          })
    };
    board = new RoutingBoard(bbox, ls, outline, 1, rules, comm);
    // `Nets.add` reads `netList.getBoard().rules` (Net.java:50), so the nets can only be created
    // once `BasicBoard`'s constructor has run `rules.nets.setBoard(this)` (BasicBoard.java:134).
    rules.nets.add("N1", 1, false);
    rules.nets.add("N2", 1, false);
    rules.nets.add("N3", 1, false);
    rules.nets.add("N4", 1, false);
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
    pkg =
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
  }

  // -------------------------------------------------------------------------------------------
  // Random item insertion — every draw below is mirrored draw-for-draw by `p2t15.rs`.
  // -------------------------------------------------------------------------------------------

  static void insertRandomPin() {
    IntPoint loc = randomPoint(RANGE);
    double rotation = 90.0 * rnd(4);
    boolean onFront = rnd(2) == 0;
    Component comp = board.components.add(loc, rotation, onFront, pkg);
    int compId = comp.id;
    int pinIndex = rnd(2);
    int netNo = 1 + rnd(4);
    int cc = 1 + rnd(2);
    board.insertPin(compId, pinIndex, new int[] {netNo}, cc, FixedState.UNFIXED);
  }

  static void insertRandomVia() {
    IntPoint center = randomPoint(RANGE);
    int netNo = 1 + rnd(4);
    int cc = 1 + rnd(2);
    boolean attach = rnd(2) == 0;
    board.insertVia(thruPad, center, new int[] {netNo}, cc, FixedState.UNFIXED, attach);
  }

  static void insertRandomTrace() {
    int layer = rnd(2);
    IntPoint p1 = randomPoint(RANGE);
    IntPoint p2;
    do {
      p2 = randomPoint(RANGE);
    } while (p2.equals(p1));
    int halfWidth = 5 + rnd(30);
    int netNo = 1 + rnd(4);
    int cc = 1 + rnd(2);
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {p1, p2}), layer, halfWidth, new int[] {netNo}, cc, FixedState.UNFIXED);
  }

  static void insertRandomObstacle() {
    IntBox box = randomBox(RANGE, 200, 2000);
    int layer = rnd(2);
    int cc = 1 + rnd(2);
    board.insertObstacle(box, layer, cc, FixedState.UNFIXED);
  }

  static void insertRandomConduction() {
    IntBox box = randomBox(RANGE, 200, 2000);
    int layer = rnd(2);
    int netNo = 1 + rnd(4);
    int cc = 1 + rnd(2);
    boolean isObstacle = rnd(2) == 0;
    board.insertConductionArea(box, layer, new int[] {netNo}, cc, isObstacle, FixedState.UNFIXED);
  }

  // -------------------------------------------------------------------------------------------
  // Dumping
  // -------------------------------------------------------------------------------------------

  /** Every item, in `getItems()` (board) order — descending id, quirk #63. */
  static void dumpItems(RoutingBoard b) {
    for (Item it : b.getItems()) {
      System.out.println(
          "item id="
              + it.getId()
              + " kind="
              + it.getClass().getSimpleName()
              + " layers="
              + it.firstLayer()
              + ".."
              + it.lastLayer()
              + " nets="
              + Arrays.toString(it.netNumbers)
              + " cl="
              + it.clearanceClassIndex()
              + " bbox="
              + box(it.boundingBox())
              + " tiles="
              + it.tileShapeCount());
      int n = it.tileShapeCount();
      for (int i = 0; i < n; i++) {
        TileShape shape = it.getTileShape(i);
        System.out.println("  tile[" + i + "]=" + (shape == null ? "null" : box(shape.boundingBox())));
      }
    }
  }

  static void dumpOverlapQueries(String label, RoutingBoard b, List<QuerySpec> specs) {
    for (QuerySpec q : specs) {
      Set<SearchTreeObject> result = b.overlappingObjects(q.box, q.layer);
      System.out.println(
          label + " box=" + box(q.box) + " layer=" + q.layer + " ids=" + objectIds(result));
    }
  }

  static void dumpClearanceQueries(String label, RoutingBoard b, List<ClearanceQuerySpec> specs) {
    for (ClearanceQuerySpec q : specs) {
      Collection<ShapeTree.TreeEntry> entries =
          b.searchTreeManager.getDefaultTree().overlappingTreeEntriesWithClearance(
              q.box, q.layer, q.ignore, q.cc);
      System.out.println(
          label
              + " box="
              + box(q.box)
              + " layer="
              + q.layer
              + " cc="
              + q.cc
              + " ignore="
              + Arrays.toString(q.ignore)
              + " entries="
              + pairText(entries));
    }
  }

  static class QuerySpec {
    final IntBox box;
    final int layer;

    QuerySpec(IntBox box, int layer) {
      this.box = box;
      this.layer = layer;
    }
  }

  static class ClearanceQuerySpec {
    final IntBox box;
    final int layer;
    final int cc;
    final int[] ignore;

    ClearanceQuerySpec(IntBox box, int layer, int cc, int[] ignore) {
      this.box = box;
      this.layer = layer;
      this.cc = cc;
      this.ignore = ignore;
    }
  }

  // -------------------------------------------------------------------------------------------
  // Formatting helpers — every one of these has an exact twin in the Rust driver.
  // -------------------------------------------------------------------------------------------

  static String box(IntBox b) {
    return "Box[" + b.ll.x + "," + b.ll.y + ".." + b.ur.x + "," + b.ur.y + "]";
  }

  static String objId(Object o) {
    return o instanceof Item item ? String.valueOf(item.getId()) : String.valueOf(o);
  }

  static String objectIds(Collection<SearchTreeObject> objects) {
    StringBuilder sb = new StringBuilder("[");
    boolean first = true;
    for (SearchTreeObject o : objects) {
      if (!first) {
        sb.append(" ");
      }
      first = false;
      sb.append(objId(o));
    }
    return sb.append("]").toString();
  }

  static String pairText(Collection<ShapeTree.TreeEntry> entries) {
    StringBuilder sb = new StringBuilder("[");
    boolean first = true;
    for (ShapeTree.TreeEntry e : entries) {
      if (!first) {
        sb.append(" ");
      }
      first = false;
      sb.append(objId(e.object)).append("/").append(e.shapeIndexInObject);
    }
    return sb.append("]").toString();
  }
}
