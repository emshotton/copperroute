package app.freerouting.datastructures;

import app.freerouting.geometry.planar.FortyfiveDegreeBoundingDirections;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.IntOctagon;
import app.freerouting.geometry.planar.OrthogonalBoundingDirections;
import app.freerouting.geometry.planar.RegularTileShape;
import app.freerouting.geometry.planar.ShapeBoundingDirections;
import app.freerouting.geometry.planar.TileShape;
import java.util.ArrayList;
import java.util.List;
import java.util.Set;

/**
 * Randomised Java ground truth for MinAreaTree/ShapeTree. Twin: `p2t3r`.
 *
 * <p>args: ops seed mode(0=orthogonal,1=45-degree) dumpEvery insertPct
 *
 * <p>Drives `insert(Storable)` (so the tree itself applies boundingDirections),
 * `remove(Leaf[])` with deliberate `null` holes, in-place re-keying of live leaves the way
 * `ShapeSearchTree` does, `overlaps`, `toArray` and `distanceToRoot`. Every random draw is
 * mirrored draw-for-draw by `rust/src/bin/p2t3r.rs`.
 */
public class P2T3R {

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

  static class Obj implements ShapeTree.Storable {
    final int id;
    final TileShape[] shapes;
    ShapeTree.Leaf[] entries;

    Obj(int id, TileShape... shapes) {
      this.id = id;
      this.shapes = shapes;
    }

    public int treeShapeCount(ShapeTree tree) {
      return shapes.length;
    }

    public TileShape getTreeShape(ShapeTree tree, int index) {
      return shapes[index];
    }

    public void setSearchTreeEntries(ShapeTree.Leaf[] entries, ShapeTree tree) {
      this.entries = entries;
    }

    public int compareTo(Object other) {
      return Integer.compare(this.id, ((Obj) other).id);
    }

    public String toString() {
      return "#" + id;
    }
  }

  static String boundsStr(RegularTileShape s) {
    if (s instanceof IntBox b) {
      return "B(" + b.ll.x + "," + b.ll.y + "," + b.ur.x + "," + b.ur.y + ")";
    }
    IntOctagon o = (IntOctagon) s;
    return "O("
        + o.leftX + "," + o.bottomY + "," + o.rightX + "," + o.topY + ","
        + o.upperLeftDiagonalX + "," + o.lowerRightDiagonalX + ","
        + o.lowerLeftDiagonalX + "," + o.upperRightDiagonalX + ")";
  }

  static void dump(ShapeTree.TreeNode node, String prefix, StringBuilder sb) {
    if (node instanceof ShapeTree.Leaf leaf) {
      sb.append(prefix).append("L ").append(leaf.object).append("/")
          .append(leaf.shapeIndexInObject).append(" ").append(boundsStr(leaf.boundingShape))
          .append("\n");
    } else {
      ShapeTree.InnerNode inner = (ShapeTree.InnerNode) node;
      sb.append(prefix).append("I ").append(boundsStr(inner.boundingShape)).append("\n");
      dump(inner.firstChild, prefix + " ", sb);
      dump(inner.secondChild, prefix + " ", sb);
    }
  }

  static void printTree(MinAreaTree tree, String label) {
    System.out.println("== " + label + " n=" + tree.size() + " rootNull=" + (tree.root == null));
    if (tree.root == null) {
      return;
    }
    StringBuilder sb = new StringBuilder();
    dump(tree.root, "", sb);
    System.out.print(sb);
  }

  static String key(ShapeTree.Leaf leaf) {
    return leaf.object + "/" + leaf.shapeIndexInObject;
  }

  public static void main(String[] args) {
    int ops = args.length > 0 ? Integer.parseInt(args[0]) : 200;
    long seed = args.length > 1 ? Long.parseLong(args[1]) : 1L;
    int mode = args.length > 2 ? Integer.parseInt(args[2]) : 0;
    int dumpEvery = args.length > 3 ? Integer.parseInt(args[3]) : 1;
    int insertPct = args.length > 4 ? Integer.parseInt(args[4]) : 62;
    state = seed == 0 ? 0x9E3779B97F4A7C15L : seed;

    ShapeBoundingDirections dirs =
        mode == 0
            ? OrthogonalBoundingDirections.INSTANCE
            : FortyfiveDegreeBoundingDirections.INSTANCE;
    MinAreaTree tree = new MinAreaTree(dirs);

    List<ShapeTree.Leaf> live = new ArrayList<>();
    int nextId = 1;

    for (int step = 0; step < ops; step++) {
      int roll = rnd(100);
      if (live.isEmpty() || roll < insertPct) {
        // `ShapeTree.insert(Storable)`: the TREE applies boundingDirections to each tree shape.
        int shapeCount = 1 + rnd(3);
        TileShape[] shapes = new TileShape[shapeCount];
        for (int k = 0; k < shapeCount; k++) {
          int x = rnd(2000) - 500;
          int y = rnd(2000) - 500;
          int w = 1 + rnd(150);
          int h = 1 + rnd(150);
          shapes[k] = new IntBox(x, y, x + w, y + h);
        }
        Obj o = new Obj(nextId++, shapes);
        tree.insert(o);
        StringBuilder ib = new StringBuilder("op" + step + " insert " + o + " x" + shapeCount);
        for (ShapeTree.Leaf lf : o.entries) {
          if (lf == null) {
            ib.append(" -");
          } else {
            live.add(lf);
            ib.append(" ").append(key(lf));
          }
        }
        System.out.println(ib);
      } else if (roll < insertPct + 12) {
        // In-place re-key of a live leaf: `ShapeSearchTree.java:150,214-215,221,294-295,325-326,
        // 342-343` assign leaf.object / leaf.shapeIndexInObject without touching bounds or links.
        int idx = rnd(live.size());
        int newIndex = rnd(4);
        ShapeTree.Leaf lf = live.get(idx);
        Obj newOwner = new Obj(nextId++);
        System.out.println(
            "op" + step + " rekey " + key(lf) + " -> " + newOwner + "/" + newIndex);
        lf.object = newOwner;
        lf.shapeIndexInObject = newIndex;
      } else {
        // `remove(Leaf[])` on an array with `null` holes, as `reuseEntriesAfterCutout` leaves
        // behind (ShapeSearchTree.java:327,344) and `SearchTreeManager.remove` passes on.
        int batch = 1 + rnd(4);
        ShapeTree.Leaf[] batchArr = new ShapeTree.Leaf[batch];
        StringBuilder rb = new StringBuilder("op" + step + " remove");
        for (int k = 0; k < batch; k++) {
          boolean hole = rnd(5) == 0;
          if (hole || live.isEmpty()) {
            batchArr[k] = null;
            rb.append(" -");
          } else {
            int idx = rnd(live.size());
            ShapeTree.Leaf lf = live.remove(idx);
            batchArr[k] = lf;
            rb.append(" ").append(key(lf));
          }
        }
        System.out.println(rb);
        tree.remove(batchArr);
      }

      if (step % dumpEvery == 0 || step == ops - 1) {
        printTree(tree, "op" + step);
      }

      for (int q = 0; q < 2; q++) {
        int x = rnd(2000) - 500;
        int y = rnd(2000) - 500;
        int w = 1 + rnd(600);
        int h = 1 + rnd(600);
        RegularTileShape query = new IntBox(x, y, x + w, y + h).boundingShape(dirs);
        Set<ShapeTree.Leaf> res = tree.overlaps(query);
        StringBuilder sb = new StringBuilder("q" + q + " " + boundsStr(query) + " ->");
        for (ShapeTree.Leaf l : res) {
          sb.append(" ").append(key(l));
        }
        System.out.println(sb);
      }

      ShapeTree.Leaf[] arr = tree.toArray();
      StringBuilder ta = new StringBuilder("toArray");
      for (ShapeTree.Leaf l : arr) {
        ta.append(" ").append(key(l));
      }
      System.out.println(ta);

      // distanceToRoot for every live leaf; -1 stands in for the one-element NPE case.
      StringBuilder dr = new StringBuilder("depths");
      for (ShapeTree.Leaf l : arr) {
        dr.append(" ").append(tree.size() == 1 ? -1 : l.distanceToRoot());
      }
      System.out.println(dr);
    }
  }
}
