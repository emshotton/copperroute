// Plan 5 Task 2 JVM probe: `Item.clearanceViolations()` over a real board, item by item.
//
// Prints, for every item of `board.getItems()` (i.e. in `board.itemList` order — descending id,
// quirk #63), the violation list that `Item.clearanceViolations()` (Item.java:363-469) returns,
// field for field, plus the `smallestClearance` the call left on the item. Then it prints
// `ClearanceViolation.aggregateSortedBySeverity(board.getItems())` (:64-75) — a *second* pass
// over the same items, which is what makes the "smallestClearance is never reset" quirk visible —
// and `ClearanceViolation.smallestClearance(board.getItems())` (:86-94).
//
// Doubles are printed with `Double.toString`, so the port compares exact bits through
// `java_double_to_string`; the bisection is deterministic, so the values are stable run to run.
//
// Usage: java -Djava.awt.headless=true -cp <jar>:. DrcProbe <board.dsn>
import app.freerouting.board.facade.BasicBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.drc.ClearanceViolation;
import app.freerouting.io.BoardReadResult;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.TileShape;
import app.freerouting.io.specctra.DsnReader;
import java.io.FileInputStream;
import java.lang.reflect.Method;
import java.util.Collection;
import java.util.List;

public class DrcProbe {
  public static void main(String[] args) throws Exception {
    BoardReadResult result;
    try (FileInputStream in = new FileInputStream(args[0])) {
      result = DsnReader.readBoard(in, null, null, "test");
    }
    BasicBoard board =
        switch (result) {
          case BoardReadResult.Success s -> (BasicBoard) s.board();
          case BoardReadResult.OutlineMissing o -> (BasicBoard) o.board();
          default -> throw new IllegalStateException("Failed to read board: " + result);
        };

    System.out.println("dsn " + args[0]);
    System.out.println("items " + board.getItems().size());
    System.out.println("compensation " + board.searchTreeManager.isClearanceCompensationUsed());

    System.out.println("== A: per item, board.getItems() order ==");
    int totalViolations = 0;
    for (Item item : board.getItems()) {
      Collection<ClearanceViolation> violations = item.clearanceViolations();
      totalViolations += violations.size();
      StringBuilder sb = new StringBuilder();
      sb.append("item ").append(item.getId()).append(" n=").append(violations.size());
      for (ClearanceViolation v : violations) {
        sb.append(" | first=")
            .append(v.firstItem.getId())
            .append(" second=")
            .append(v.secondItem.getId())
            .append(" layer=")
            .append(v.layer)
            .append(" expected=")
            .append(Double.toString(v.expectedClearance))
            .append(" actual=")
            .append(Double.toString(v.actualClearance));
      }
      sb.append(" || smallest=").append(Double.toString(item.smallestClearance));
      System.out.println(sb);
    }
    System.out.println("total " + totalViolations);

    System.out.println("== B: aggregateSortedBySeverity(board.getItems()) ==");
    List<ClearanceViolation> aggregated =
        ClearanceViolation.aggregateSortedBySeverity(board.getItems());
    System.out.println("aggregated " + aggregated.size());
    for (ClearanceViolation v : aggregated) {
      System.out.println(
          "agg first="
              + v.firstItem.getId()
              + " second="
              + v.secondItem.getId()
              + " layer="
              + v.layer
              + " expected="
              + Double.toString(v.expectedClearance)
              + " actual="
              + Double.toString(v.actualClearance)
              + " shortfall="
              + Double.toString(v.expectedClearance - v.actualClearance));
    }

    System.out.println("== C: smallestClearance(board.getItems()) ==");
    System.out.println(
        "smallest " + Double.toString(ClearanceViolation.smallestClearance(board.getItems())));

    // Block D: the private `Item.calculateClearanceBetweenTwoShapes` (Item.java:471-493) on
    // synthetic boxes, reached by reflection. The method never touches `this`, so any item of the
    // board serves as the receiver. These are the goldens the port's bisection test asserts.
    System.out.println("== D: calculateClearanceBetweenTwoShapes on synthetic boxes ==");
    Method m =
        Item.class.getDeclaredMethod(
            "calculateClearanceBetweenTwoShapes",
            TileShape.class,
            TileShape.class,
            double.class,
            int.class,
            int.class);
    m.setAccessible(true);
    Item receiver = board.getItems().iterator().next();
    TileShape a = new IntBox(0, 0, 100, 100);
    bisect(m, receiver, "D1 gap=200 min=1000 comp=500/500", a, new IntBox(300, 0, 400, 100), 1000.0, 500, 500);
    bisect(m, receiver, "D2 gap=200 min=1000 comp=250/750", a, new IntBox(300, 0, 400, 100), 1000.0, 250, 750);
    bisect(m, receiver, "D3 overlap  min=1000 comp=500/500", a, new IntBox(50, 0, 150, 100), 1000.0, 500, 500);
    bisect(m, receiver, "D4 gap=2000 min=1000 comp=500/500", a, new IntBox(2100, 0, 2200, 100), 1000.0, 500, 500);
    bisect(m, receiver, "D5 touching min=1000 comp=500/500", a, new IntBox(100, 0, 200, 100), 1000.0, 500, 500);
    bisect(m, receiver, "D6 gap=200 min=1000 comp=0/0", a, new IntBox(300, 0, 400, 100), 1000.0, 0, 0);
    bisect(m, receiver, "D7 gap=120 min=200  comp=100/100", a, new IntBox(220, 0, 320, 100), 200.0, 100, 100);
  }

  private static void bisect(
      Method m,
      Item receiver,
      String label,
      TileShape s1,
      TileShape s2,
      double minimumClearance,
      int clComp1,
      int clComp2)
      throws Exception {
    double r = (Double) m.invoke(receiver, s1, s2, minimumClearance, clComp1, clComp2);
    System.out.println(label + " -> " + Double.toString(r));
  }
}
