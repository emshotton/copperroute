// Plan 6 Task 6 ground-truth probe: `AutorouteEngine`'s expansion-room lifecycle on hand-built
// boards. It is not a differential driver — there is no Rust twin and `run.sh` does not know it —
// but every literal in `crates/fr-router/tests/engine_rooms.rs` is read off its stdout, so it is
// committed here to keep those numbers reproducible.
//
// It declares `package app.freerouting.autoroute.maze` so it can reflect into `AutorouteEngine`'s
// private `completeExpansionRooms` / `incompleteExpansionRooms` / `expansionRoomInstanceCount`
// fields, which are the state the tests assert on and which no public method exposes. It compiles
// against the clone's HEAD jar exactly as `run.sh`'s jar mode does:
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential/java/probes
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p6t6 P6T6Probe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p6t6:$JAR" \
//       app.freerouting.autoroute.maze.P6T6Probe <mode>
//
// Modes: 0 an empty board, 1 one obstacle, 2 `completeNeighbourRooms`, 3 the net-dependent
// invalidation, 4 `completeShape`'s raw candidates for mode 1's seed, 5 the door-by-door trace of
// `removeCompleteExpansionRoom` (which is how the `otherRoom` overload of quirk #164 was found).
package app.freerouting.autoroute.maze;

import app.freerouting.autoroute.expansion.CompleteExpansionRoom;
import app.freerouting.autoroute.expansion.CompleteFreeSpaceExpansionRoom;
import app.freerouting.autoroute.expansion.ExpansionDoor;
import app.freerouting.autoroute.expansion.ExpansionRoom;
import app.freerouting.autoroute.expansion.IncompleteFreeSpaceExpansionRoom;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.model.structure.Layer;
import app.freerouting.board.model.structure.LayerStructure;
import app.freerouting.board.searchtree.ShapeSearchTree;
import app.freerouting.board.state.Communication;
import app.freerouting.core.library.Package;
import app.freerouting.core.library.Padstack;
import app.freerouting.geometry.planar.Area;
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
import app.freerouting.rules.BoardRules;
import app.freerouting.rules.ClearanceMatrix;
import java.lang.reflect.Field;
import java.util.Collection;
import java.util.List;

/** Task 6 ground truth: AutorouteEngine's room lifecycle on hand-built boards. */
public class P6T6Probe {

  static final IntBox BOUNDING_BOX = new IntBox(-10000, -10000, 10000, 10000);
  static RoutingBoard board;

  public static void main(String[] args) throws Exception {
    int mode = args.length > 0 ? Integer.parseInt(args[0]) : 0;
    switch (mode) {
      case 0 -> emptyBoard();
      case 1 -> oneObstacle();
      case 2 -> neighbourRestart();
      case 3 -> netDependent();
      case 4 -> rawCandidates();
      case 5 -> netDependentDoors();
      default -> throw new IllegalArgumentException("mode");
    }
  }

  // --- boards -------------------------------------------------------------------------------

  /** A board with nothing on it but its outline-less bounding box. */
  static void buildBare() {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    BoardRules rules = new BoardRules(ls, cm);
    rules.setTraceAngleRestriction(AngleRestriction.NONE);
    board = new RoutingBoard(BOUNDING_BOX, ls, new PolylineShape[0], 0, rules, new Communication());
    board.library.padstacks = new app.freerouting.core.library.Padstacks(ls);
    board.library.packages = new app.freerouting.core.library.Packages(board.library.padstacks);
  }

  /** The bare board plus one trace on net 1, so `isNetDependent` has something to bite on. */
  static void addTrace() {
    Polyline traceLine =
        new Polyline(new Point[] {new IntPoint(-500, 0), new IntPoint(0, 0), new IntPoint(0, 400)});
    board.insertTraceWithoutCleaning(traceLine, 0, 30, new int[] {1}, 1, FixedState.UNFIXED);
  }

  static ShapeSearchTree treeOf(AutorouteEngine engine) {
    return engine.autorouteSearchTree;
  }

  // --- dumps --------------------------------------------------------------------------------

  static String shp(TileShape s) {
    if (s == null) {
      return "null";
    }
    if (s.isEmpty()) {
      return "empty";
    }
    IntBox b = s.boundingBox();
    StringBuilder sb = new StringBuilder();
    sb.append(s.getClass().getSimpleName())
        .append("[")
        .append(b.ll.x)
        .append(",")
        .append(b.ll.y)
        .append("..")
        .append(b.ur.x)
        .append(",")
        .append(b.ur.y)
        .append("]dim=")
        .append(s.dimension())
        .append(" corners=");
    for (int i = 0; i < s.borderLineCount(); i++) {
      sb.append("(")
          .append(Math.round(s.cornerApprox(i).x))
          .append(",")
          .append(Math.round(s.cornerApprox(i).y))
          .append(")");
    }
    return sb.toString();
  }

  @SuppressWarnings("unchecked")
  static void dumpEngine(AutorouteEngine engine, String tag) throws Exception {
    Field fc = AutorouteEngine.class.getDeclaredField("completeExpansionRooms");
    fc.setAccessible(true);
    Field fi = AutorouteEngine.class.getDeclaredField("incompleteExpansionRooms");
    fi.setAccessible(true);
    Field fn = AutorouteEngine.class.getDeclaredField("expansionRoomInstanceCount");
    fn.setAccessible(true);
    List<CompleteFreeSpaceExpansionRoom> complete =
        (List<CompleteFreeSpaceExpansionRoom>) fc.get(engine);
    List<IncompleteFreeSpaceExpansionRoom> incomplete =
        (List<IncompleteFreeSpaceExpansionRoom>) fi.get(engine);
    System.out.println(
        tag
            + " counter="
            + fn.get(engine)
            + " complete="
            + (complete == null ? "null" : complete.size())
            + " incomplete="
            + (incomplete == null ? "null" : incomplete.size())
            + " treeSize="
            + engine.autorouteSearchTree.size());
    if (complete != null) {
      for (CompleteFreeSpaceExpansionRoom r : complete) {
        System.out.println(
            "    complete id=" + r.getId() + " layer=" + r.getLayer() + " shape=" + shp(r.getShape())
                + " netDependent=" + r.isNetDependent() + " doors=" + r.getDoors().size()
                + " targetDoors=" + r.getTargetDoors().size());
      }
    }
    if (incomplete != null) {
      for (IncompleteFreeSpaceExpansionRoom r : incomplete) {
        System.out.println(
            "    incomplete layer=" + r.getLayer() + " shape=" + shp(r.getShape())
                + " contained=" + shp(r.getContainedShape()) + " doors=" + r.getDoors().size());
      }
    }
  }

  static void dumpResult(Collection<CompleteFreeSpaceExpansionRoom> result) {
    System.out.println("  result n=" + result.size());
    for (CompleteFreeSpaceExpansionRoom r : result) {
      System.out.println(
          "    room id=" + r.getId() + " layer=" + r.getLayer() + " shape=" + shp(r.getShape()));
    }
  }

  @SuppressWarnings("unchecked")
  static void netDependentDoors() throws Exception {
    buildBare();
    addTrace();
    AutorouteEngine engine = new AutorouteEngine(board, 1, true);
    engine.initConnection(1, null, null);
    IncompleteFreeSpaceExpansionRoom seed =
        engine.addIncompleteExpansionRoom(null, 0, new IntBox(-600, -100, -400, 100));
    engine.completeExpansionRoom(seed);
    Field fc = AutorouteEngine.class.getDeclaredField("completeExpansionRooms");
    fc.setAccessible(true);
    List<CompleteFreeSpaceExpansionRoom> complete =
        (List<CompleteFreeSpaceExpansionRoom>) fc.get(engine);
    for (CompleteFreeSpaceExpansionRoom r : complete) {
      System.out.println("room id=" + r.getId() + " shape=" + shp(r.getShape()));
      for (ExpansionDoor d : r.getDoors()) {
        ExpansionRoom other = d.otherRoom((ExpansionRoom) r);
        String kind = other == null ? "null" : other.getClass().getSimpleName();
        TileShape os = other == null ? null : other.getShape();
        TileShape inter = os == null ? null : r.getShape().intersection(os);
        System.out.println(
            "    door dim=" + d.dimension + " other=" + kind + " oshape=" + shp(os)
                + " interDim=" + (inter == null ? "-" : inter.dimension()));
      }
    }
    // Now remove them one by one, printing the same for each.
    for (CompleteFreeSpaceExpansionRoom r : new java.util.ArrayList<>(complete)) {
      System.out.println("removing id=" + r.getId() + " doors=" + r.getDoors().size());
      for (ExpansionDoor d : new java.util.ArrayList<>(r.getDoors())) {
        ExpansionRoom other = d.otherRoom((ExpansionRoom) r);
        String kind = other == null ? "null" : other.getClass().getSimpleName();
        TileShape os = other == null ? null : other.getShape();
        TileShape inter = os == null ? null : r.getShape().intersection(os);
        String ts = "-";
        if (inter != null && inter.dimension() == 1) {
          int[] t = r.getShape().touchingSides(os);
          ts = t.length == 0 ? "EMPTY" : (t[0] + "/" + t[1]);
        }
        System.out.println(
            "    door dim=" + d.dimension + " other=" + kind + " oshape=" + shp(os)
                + " interDim=" + (inter == null ? "-" : inter.dimension())
                + " oLines=" + (os == null ? -1 : os.borderLineCount())
                + " touchingSides=" + ts);
      }
      try {
        engine.removeCompleteExpansionRoom(r);
        System.out.println("    removed ok");
        dumpEngine(engine, "  afterRemoving" + r.getId());
      } catch (RuntimeException e) {
        System.out.println("    THREW " + e);
        for (StackTraceElement el : e.getStackTrace()) {
          System.out.println("      at " + el);
          break;
        }
      }
    }
    dumpEngine(engine, "afterRemovals");
  }

  static void rawCandidates() throws Exception {
    buildBare();
    board.insertObstacle(new IntBox(0, 0, 1000, 1000), 0, 1, FixedState.UNFIXED);
    AutorouteEngine engine = new AutorouteEngine(board, 1, false);
    engine.initConnection(1, null, null);
    IncompleteFreeSpaceExpansionRoom seed =
        engine.addIncompleteExpansionRoom(null, 0, new IntBox(-3000, -3000, 3000, 3000));
    Collection<IncompleteFreeSpaceExpansionRoom> completed =
        engine.autorouteSearchTree.completeShape(seed, 1, null, null);
    System.out.println("mode=4 rawCandidates n=" + completed.size());
    int i = 0;
    for (IncompleteFreeSpaceExpansionRoom r : completed) {
      System.out.println("    [" + (i++) + "] layer=" + r.getLayer() + " shape=" + shp(r.getShape()));
    }
  }

  // --- modes --------------------------------------------------------------------------------

  static void emptyBoard() throws Exception {
    buildBare();
    AutorouteEngine engine = new AutorouteEngine(board, 1, false);
    System.out.println("mode=0 emptyBoard treeSize=" + engine.autorouteSearchTree.size());
    engine.initConnection(1, null, null);
    IncompleteFreeSpaceExpansionRoom seed =
        engine.addIncompleteExpansionRoom(null, 0, new IntBox(2000, 2000, 2100, 2100));
    dumpEngine(engine, "before");
    Collection<CompleteFreeSpaceExpansionRoom> result = engine.completeExpansionRoom(seed);
    dumpResult(result);
    dumpEngine(engine, "after");
  }

  static void oneObstacle() throws Exception {
    buildBare();
    board.insertObstacle(
        new IntBox(0, 0, 1000, 1000), 0, 1, FixedState.UNFIXED);
    AutorouteEngine engine = new AutorouteEngine(board, 1, false);
    System.out.println("mode=1 oneObstacle treeSize=" + engine.autorouteSearchTree.size());
    engine.initConnection(1, null, null);
    IncompleteFreeSpaceExpansionRoom seed =
        engine.addIncompleteExpansionRoom(null, 0, new IntBox(-3000, -3000, 3000, 3000));
    dumpEngine(engine, "before");
    Collection<CompleteFreeSpaceExpansionRoom> result = engine.completeExpansionRoom(seed);
    dumpResult(result);
    dumpEngine(engine, "after");
  }

  static void neighbourRestart() throws Exception {
    buildBare();
    board.insertObstacle(new IntBox(0, 0, 1000, 1000), 0, 1, FixedState.UNFIXED);
    board.insertObstacle(new IntBox(-4000, -4000, -3000, -3000), 0, 1, FixedState.UNFIXED);
    AutorouteEngine engine = new AutorouteEngine(board, 1, false);
    System.out.println("mode=2 neighbourRestart treeSize=" + engine.autorouteSearchTree.size());
    engine.initConnection(1, null, null);
    IncompleteFreeSpaceExpansionRoom seed =
        engine.addIncompleteExpansionRoom(null, 0, new IntBox(-100, -100, -50, -50));
    Collection<CompleteFreeSpaceExpansionRoom> result = engine.completeExpansionRoom(seed);
    dumpResult(result);
    dumpEngine(engine, "afterComplete");
    for (CompleteFreeSpaceExpansionRoom r : result) {
      engine.completeNeighbourRooms(r);
    }
    dumpEngine(engine, "afterNeighbours");
  }

  static void netDependent() throws Exception {
    buildBare();
    addTrace();
    AutorouteEngine engine = new AutorouteEngine(board, 1, true);
    System.out.println("mode=3 netDependent maintainDatabase=true");
    engine.initConnection(1, null, null);
    IncompleteFreeSpaceExpansionRoom seed =
        engine.addIncompleteExpansionRoom(null, 0, new IntBox(-600, -100, -400, 100));
    Collection<CompleteFreeSpaceExpansionRoom> result = engine.completeExpansionRoom(seed);
    dumpResult(result);
    dumpEngine(engine, "afterComplete");
    engine.initConnection(2, null, null);
    dumpEngine(engine, "afterInitNet2");
  }
}
