package app.freerouting.datastructures;

import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.OrthogonalBoundingDirections;
import app.freerouting.geometry.planar.RegularTileShape;
import java.util.Set;

/**
 * Plan 2 / Task 3 ground-truth dump for {@link MinAreaTree} and {@link ShapeTree}.
 *
 * <p>Unlike the Task 14-17 drivers this one has no Rust twin binary: the Rust side is the
 * `crates/fr-board/tests/min_area_tree.rs` integration test, whose expected values were copied
 * from this program's stdout. Run it with `scripts/differential/run.sh p2t3` and diff against
 * `build/p2t3.j.out` (checked in) after any change to the Java sources.
 *
 * <p>Everything it prints is deterministic: no randomness, no timing.
 */
public class P2T3 {

  /** A minimal {@link ShapeTree.Storable}: an integer id plus its tree shapes. */
  static class Obj implements ShapeTree.Storable {

    final int id;
    final IntBox[] shapes;

    Obj(int id, IntBox... shapes) {
      this.id = id;
      this.shapes = shapes;
    }

    @Override
    public int treeShapeCount(ShapeTree tree) {
      return shapes.length;
    }

    @Override
    public app.freerouting.geometry.planar.TileShape getTreeShape(ShapeTree tree, int index) {
      return shapes[index];
    }

    @Override
    public void setSearchTreeEntries(ShapeTree.Leaf[] entries, ShapeTree tree) {}

    /** Java `Item.compareTo` reduced to its id comparison — what `Leaf.compareTo` delegates to. */
    @Override
    public int compareTo(Object other) {
      return this.id - ((Obj) other).id;
    }

    @Override
    public String toString() {
      return "#" + id;
    }
  }

  static String boundsStr(RegularTileShape s) {
    IntBox b = (IntBox) s;
    return "(" + b.ll.x + "," + b.ll.y + "," + b.ur.x + "," + b.ur.y + ")";
  }

  static void dump(ShapeTree.TreeNode node, String prefix, StringBuilder sb) {
    if (node instanceof ShapeTree.Leaf leaf) {
      sb.append(prefix)
          .append("Leaf ")
          .append(leaf.object)
          .append("/")
          .append(leaf.shapeIndexInObject)
          .append(" ")
          .append(boundsStr(leaf.boundingShape))
          .append("\n");
    } else {
      ShapeTree.InnerNode inner = (ShapeTree.InnerNode) node;
      sb.append(prefix).append("Inner ").append(boundsStr(inner.boundingShape)).append("\n");
      dump(inner.firstChild, prefix + "  ", sb);
      dump(inner.secondChild, prefix + "  ", sb);
    }
  }

  static void printTree(MinAreaTree tree, String label) {
    System.out.println("--- " + label + " leafCount=" + tree.size());
    if (tree.root == null) {
      System.out.println("(empty)");
      return;
    }
    StringBuilder sb = new StringBuilder();
    dump(tree.root, "", sb);
    System.out.print(sb);
  }

  static void printOverlaps(MinAreaTree tree, IntBox query, String label) {
    RegularTileShape q = query.boundingShape(OrthogonalBoundingDirections.INSTANCE);
    Set<ShapeTree.Leaf> res = tree.overlaps(q);
    StringBuilder sb = new StringBuilder(label + " -> ");
    for (ShapeTree.Leaf l : res) {
      sb.append(l.object).append("/").append(l.shapeIndexInObject).append(" ");
    }
    System.out.println(sb.toString().stripTrailing());
  }

  public static void main(String[] args) {
    eightBoxes();
    tieGoesToFirstChild();
    edgeCases();
  }

  /** The 8-box fixture the Rust integration test mirrors. */
  static void eightBoxes() {
    MinAreaTree tree = new MinAreaTree(OrthogonalBoundingDirections.INSTANCE);

    int[][] boxes = {
      {0, 0, 10, 10}, // #1 A
      {20, 0, 30, 10}, // #2 B
      {0, 20, 10, 30}, // #3 C
      {20, 20, 30, 30}, // #4 D
      {40, 0, 50, 10}, // #5 E
      {40, 20, 50, 30}, // #6 F
      {5, 5, 15, 15}, // #7 G
      {60, 60, 70, 70}, // #8 H
    };

    ShapeTree.Leaf[] leaves = new ShapeTree.Leaf[boxes.length];
    for (int i = 0; i < boxes.length; i++) {
      int[] c = boxes[i];
      leaves[i] = tree.insert(new Obj(i + 1, new IntBox(c[0], c[1], c[2], c[3])), 0);
      printTree(tree, "after inserting #" + (i + 1));
    }

    printOverlaps(tree, new IntBox(0, 0, 10, 10), "Q1 (0,0,10,10)");
    printOverlaps(tree, new IntBox(15, 15, 45, 45), "Q2 (15,15,45,45)");
    printOverlaps(tree, new IntBox(55, 55, 65, 65), "Q3 (55,55,65,65)");
    printOverlaps(tree, new IntBox(200, 200, 210, 210), "Q4 (200,200,210,210)");
    printOverlaps(tree, new IntBox(-5, -5, 100, 100), "Q5 all");

    for (int i = 0; i < leaves.length; i++) {
      System.out.println("distanceToRoot #" + (i + 1) + " = " + leaves[i].distanceToRoot());
    }

    tree.removeLeaf(leaves[6]);
    printTree(tree, "after removing #7");
    printOverlaps(tree, new IntBox(0, 0, 10, 10), "Q1' (0,0,10,10)");
    printOverlaps(tree, new IntBox(-5, -5, 100, 100), "Q5' all");

    tree.removeLeaf(leaves[7]);
    printTree(tree, "after removing #8");

    ShapeTree.Leaf[] arr = tree.toArray();
    StringBuilder ta = new StringBuilder("toArray:");
    for (ShapeTree.Leaf l : arr) {
      ta.append(" ").append(l.object).append("/").append(l.shapeIndexInObject);
    }
    System.out.println(ta);

    for (int i = 0; i < 6; i++) {
      tree.removeLeaf(leaves[i]);
      System.out.println(
          "after removing #"
              + (i + 1)
              + " leafCount="
              + tree.size()
              + " rootNull="
              + (tree.root == null));
    }

    // Multi-shape object: `insert(Storable)` makes one leaf per shape index; the TreeSet then
    // orders (object, shapeIndexInObject).
    MinAreaTree t2 = new MinAreaTree(OrthogonalBoundingDirections.INSTANCE);
    t2.insert(new Obj(2, new IntBox(0, 0, 10, 10), new IntBox(100, 100, 110, 110)));
    t2.insert(new Obj(1, new IntBox(5, 5, 15, 15)));
    printOverlaps(t2, new IntBox(-5, -5, 200, 200), "Q6 multi-shape");
  }

  /**
   * `firstAreaIncrease <= secondAreaIncrease` (MinAreaTree.java:109) sends an exact tie to the
   * FIRST child.
   */
  static void tieGoesToFirstChild() {
    MinAreaTree tree = new MinAreaTree(OrthogonalBoundingDirections.INSTANCE);
    tree.insert(new Obj(1, new IntBox(0, 0, 10, 10)), 0);
    tree.insert(new Obj(2, new IntBox(20, 20, 30, 30)), 0);
    // increase(#1) = area(0,0,20,20) - 100 = 300; increase(#2) = area(10,10,30,30) - 100 = 300.
    tree.insert(new Obj(3, new IntBox(10, 10, 20, 20)), 0);
    printTree(tree, "tie tree");
  }

  static void edgeCases() {
    MinAreaTree t2 = new MinAreaTree(OrthogonalBoundingDirections.INSTANCE);
    t2.insert(new Obj(1));
    System.out.println("empty insert leafCount=" + t2.size() + " rootNull=" + (t2.root == null));

    System.out.println(
        "touching intersects = " + new IntBox(0, 0, 10, 10).intersects(new IntBox(10, 0, 20, 10)));

    MinAreaTree t3 = new MinAreaTree(OrthogonalBoundingDirections.INSTANCE);
    ShapeTree.Leaf only = t3.insert(new Obj(9, new IntBox(0, 0, 1, 1)), 0);
    try {
      System.out.println("root leaf distanceToRoot = " + only.distanceToRoot());
    } catch (Throwable t) {
      System.out.println("root leaf distanceToRoot threw " + t.getClass().getName());
    }
    t3.removeLeaf(only);
    System.out.println(
        "single removeLeaf leafCount=" + t3.size() + " rootNull=" + (t3.root == null));
    t3.remove(null);
    System.out.println("remove(null) ok, leafCount=" + t3.size());
  }
}
