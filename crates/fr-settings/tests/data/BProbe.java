import app.freerouting.board.facade.BasicBoard;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.structure.Layer;
import app.freerouting.board.model.structure.LayerStructure;
import app.freerouting.board.state.Communication;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.PolylineShape;
import app.freerouting.geometry.planar.TileShape;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;
import app.freerouting.rules.BoardRules;
import app.freerouting.rules.ClearanceMatrix;
import app.freerouting.settings.RouterSettings;
import java.io.FileInputStream;
import java.io.InputStream;

/**
 * Plan 4 Task 5 golden driver: `RouterSettings.applyBoardSpecificOptimizations` over real DSN
 * fixtures and over the two synthetic boards the Java unit tests build.
 *
 * <p>Usage: {@code java -cp <jar>:. BProbe <fixture.dsn> [<fixture.dsn> ...]}. Every fixture is
 * read with {@code DsnReader.readBoard}, then tuned twice: once on a bare {@code new
 * RouterSettings()} and once after {@code setLayerCount(board.getLayerCount())}, which is the
 * shape {@code RoutingJobScheduler} reaches (`DsnFileSettings.java:46-48` then
 * `RoutingJobScheduler.java:186`).
 */
public final class BProbe {

  public static void main(String[] args) throws Exception {
    synthetic("synthetic-2sig-2000000x1000000", 2_000_000, 1_000_000, new boolean[] {true, true});
    synthetic("synthetic-2sig-2000000x2000000", 2_000_000, 2_000_000, new boolean[] {true, true});
    synthetic(
        "synthetic-4sig-2000000x1000000",
        2_000_000,
        1_000_000,
        new boolean[] {true, true, true, true});
    synthetic(
        "synthetic-sig-plane-sig-2000000x1000000",
        2_000_000,
        1_000_000,
        new boolean[] {true, false, true});

    for (String path : args) {
      fixture(path);
    }

    mergeDropsTheFlag();
  }

  /**
   * Quirk Q9 (docs/java-quirks.md #127): `ReflectionUtil.copyFields` skips non-public fields
   * (ReflectionUtil.java:226-228),
   * and `boardSpecificTraceCostsApplied` is `private transient`, so a merged `RouterSettings`
   * carries the source's tuned cost arrays with the flag reset to `null` — and the next
   * `applyBoardSpecificOptimizations` overwrites them.
   */
  private static void mergeDropsTheFlag() {
    Layer[] layers = {new Layer("L0", true), new Layer("L1", true)};
    LayerStructure layerStructure = new LayerStructure(layers);
    ClearanceMatrix clearanceMatrix = ClearanceMatrix.getDefaultInstance(layerStructure, 10);
    BoardRules boardRules = new BoardRules(layerStructure, clearanceMatrix);
    boardRules.createDefaultNetClass();
    RoutingBoard board =
        new RoutingBoard(
            new IntBox(0, 0, 2_000_000, 1_000_000),
            layerStructure,
            new PolylineShape[] {TileShape.getInstance(0, 0, 2_000_000, 1_000_000)},
            0,
            boardRules,
            new Communication());

    RouterSettings source = new RouterSettings();
    source.setLayerCount(2);
    source.applyBoardSpecificOptimizations(board);
    source.setAgainstPreferredDirectionTraceCosts(0, 4.5);
    System.out.printf(
        "Q9.source applied=%s und0=%s%n",
        source.areBoardSpecificTraceCostsApplied(),
        source.scoring.undesiredDirectionTraceCost[0]);

    RouterSettings target = new RouterSettings();
    target.applyNewValuesFrom(source);
    System.out.printf(
        "Q9.merged applied=%s layerCount=%d und0=%s%n",
        target.areBoardSpecificTraceCostsApplied(),
        target.getLayerCount(),
        target.scoring.undesiredDirectionTraceCost[0]);

    target.applyBoardSpecificOptimizations(board);
    System.out.printf(
        "Q9.retuned applied=%s und0=%s%n",
        target.areBoardSpecificTraceCostsApplied(),
        target.scoring.undesiredDirectionTraceCost[0]);
  }

  private static void synthetic(String name, int width, int height, boolean[] isSignal) {
    Layer[] layers = new Layer[isSignal.length];
    for (int i = 0; i < isSignal.length; i++) {
      layers[i] = new Layer("L" + i, isSignal[i]);
    }
    LayerStructure layerStructure = new LayerStructure(layers);
    ClearanceMatrix clearanceMatrix = ClearanceMatrix.getDefaultInstance(layerStructure, 10);
    BoardRules boardRules = new BoardRules(layerStructure, clearanceMatrix);
    boardRules.createDefaultNetClass();
    RoutingBoard board =
        new RoutingBoard(
            new IntBox(0, 0, width, height),
            layerStructure,
            new PolylineShape[] {TileShape.getInstance(0, 0, width, height)},
            0,
            boardRules,
            new Communication());
    report(name, board);
  }

  private static void fixture(String path) throws Exception {
    try (InputStream in = new FileInputStream(path)) {
      BoardReadResult result = DsnReader.readBoard(in, null, null);
      BasicBoard board;
      if (result instanceof BoardReadResult.Success s) {
        board = s.board();
      } else if (result instanceof BoardReadResult.OutlineMissing o) {
        board = o.board();
      } else {
        System.out.println(path + ": read failed -> " + result);
        return;
      }
      String name = path.substring(path.lastIndexOf('/') + 1);
      report(name, (RoutingBoard) board);
    }
  }

  private static void report(String name, RoutingBoard board) {
    int layerCount = board.getLayerCount();
    StringBuilder signal = new StringBuilder();
    for (int i = 0; i < layerCount; i++) {
      signal.append(board.layerStructure.layers[i].isSignal ? "S" : "-");
    }
    System.out.printf(
        "%s: bbox=[%d,%d,%d,%d] w=%d h=%d layers=%d signal=%d flags=%s%n",
        name,
        board.boundingBox.ll.x,
        board.boundingBox.ll.y,
        board.boundingBox.ur.x,
        board.boundingBox.ur.y,
        board.boundingBox.width(),
        board.boundingBox.height(),
        layerCount,
        board.layerStructure.signalLayerCount(),
        signal);

    RouterSettings bare = new RouterSettings();
    bare.applyBoardSpecificOptimizations(board);
    dump(name, "bare", bare, layerCount);

    RouterSettings sized = new RouterSettings();
    sized.setLayerCount(layerCount);
    sized.applyBoardSpecificOptimizations(board);
    dump(name, "sized", sized, layerCount);

    // Idempotence: the guarded second call must change nothing.
    sized.applyBoardSpecificOptimizations(board);
    dump(name, "sized2", sized, layerCount);
  }

  private static void dump(String name, String tag, RouterSettings s, int layerCount) {
    System.out.printf(
        "%s/%s applied=%s layerCount=%d%n",
        name, tag, s.areBoardSpecificTraceCostsApplied(), s.getLayerCount());
    for (int i = 0; i < layerCount; i++) {
      System.out.printf(
          "%s/%s L%d routable=%s prefHoriz=%s bendCost=%s pref=%s undesired=%s%n",
          name,
          tag,
          i,
          s.layers[i].routable,
          s.layers[i].preferredDirectionHorizontal,
          s.layers[i].bendCost,
          s.scoring.preferredDirectionTraceCost[i],
          s.scoring.undesiredDirectionTraceCost[i]);
    }
  }
}
