// Plan 6 Task 13 ground-truth probe: `autoroute/maze/MazeExpansionEngine.java:31-414`,
// `autoroute/maze/MazeRipupResolver.java:35-268` and `autoroute/path/Connection.java:39-154`,
// plus the first end-to-end `MazeSearchEngine.findConnection` (`:300-312`) now that no expander
// is a stub.
//
// It is not a differential driver — there is no Rust twin and `run.sh` does not know it — but
// every literal in `crates/fr-router/tests/{maze_drills,ripup}.rs` is read off its stdout, so it
// is committed here to keep those numbers reproducible.
//
// It declares `package app.freerouting.autoroute.maze` so it can construct the package-private
// `MazeExpansionEngine` / `MazeRipupResolver` and call their package-private methods directly,
// read `MazeSearchEngine`'s package-private `mazeExpansionList` / `ctrl` and `MazeListElement`'s
// fields, and reflect into the two private members under test
// (`MazeExpansionEngine.checkLayerWithAnyMatchingVia`, `MazeRipupResolver.enterThroughSmallDoor`).
// It compiles together with `P6T11Probe.java`, whose `fp()`/`fl()`/`describe()`/`dumpQueue()`/
// `Counter` it reuses, against the clone's HEAD jar exactly as `run.sh`'s jar mode does:
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential/java/probes
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p6t13 P6T11Probe.java P6T13Probe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p6t13:$JAR" \
//       app.freerouting.autoroute.maze.P6T13Probe <mode> 2>/dev/null \
//     | grep -Ev '^[0-9]{4}-[0-9]{2}-[0-9]{2} [0-9:.]+ +(WARN|INFO|DEBUG|ERROR) '
//
// The `grep` strips `FRLogger`'s timestamped lines; `MazeRipupResolver:173-195` emits one per
// `checkRipup` on stdout, which would otherwise make the transcript non-reproducible.
//
// Unlike Tasks 11 and 12 this probe builds **its own** board: their board carries an *empty*
// `ViaRule`, so `ctrl.viaInfos` is zero-length and every via mask of
// `MazeExpansionEngine.java:331-345` would be dead. This one declares a real via padstack, a
// `ViaInfo` over it and a one-via `ViaRule` on the default net class, plus a foreign-net trace to
// rip up and a free via to expand a drill from.
//
// Modes:
//   items      the board's item list in `getItems()` order, so the Rust twin names the same ids
//   ctrl       the `AutorouteControl` fields the drill/ripup half reads
//   page       `expandToDrillPage` (:115-144): the queue element one drill page produces
//   pagedrills `expandToDrillsOfPage` (:145-236): the page's drills and the elements they make,
//              including the three `continue`s of :169-232
//   drill      `expandToDrill` (:31-114): the thin-room refusal, the `DrillPage`-door branch of
//              :78-85 and the `minNormalViaCost` the other branch adds
//   layers     `expandToOtherLayers` (:237-375) on a free-space drill and on a via obstacle room
//   checklayer `checkLayerWithAnyMatchingVia` (:377-414) per layer
//   fanoutfac  `calcFanoutViaRipupCostFactor` (:35-66) over five traces
//   ripup      `checkRipup` (:72-197): the refusals, `ALREADY_RIPPED_COSTS`, the trace and via
//              cost factors, the detour divide and the `Integer.MAX_VALUE / 100` clamp
//   random     the `Random(ripupCosts)` draws of :158-163 for three seeds
//   smalldoor  `enterThroughSmallDoor` (:219-268) and `checkLeavingRippedItem` (:200-217)
//   conn       `Connection.get` / `traceLength` / `getDetour` (Connection.java:39-154)
//   find       `findConnection` end to end: the pop sequence and the `Result`
package app.freerouting.autoroute.maze;

import app.freerouting.autoroute.drill.DrillPage;
import app.freerouting.autoroute.drill.ExpansionDrill;
import app.freerouting.autoroute.expansion.CompleteExpansionRoom;
import app.freerouting.autoroute.expansion.ExpandableObject;
import app.freerouting.autoroute.expansion.ExpansionDoor;
import app.freerouting.autoroute.expansion.ObstacleExpansionRoom;
import app.freerouting.autoroute.path.Connection;
import app.freerouting.board.actions.ForcedPadRouter;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Trace;
import app.freerouting.board.model.items.Via;
import app.freerouting.board.model.structure.AngleRestriction;
import app.freerouting.board.model.structure.FixedState;
import app.freerouting.board.model.structure.Layer;
import app.freerouting.board.model.structure.LayerStructure;
import app.freerouting.board.state.Communication;
import app.freerouting.core.library.Package;
import app.freerouting.core.library.Packages;
import app.freerouting.core.library.Padstack;
import app.freerouting.core.library.Padstacks;
import app.freerouting.geometry.planar.ConvexShape;
import app.freerouting.geometry.planar.FloatLine;
import app.freerouting.geometry.planar.FloatPoint;
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
import app.freerouting.rules.ViaInfo;
import app.freerouting.rules.ViaRule;
import app.freerouting.settings.RouterSettings;
import java.lang.reflect.Method;
import java.util.Collection;
import java.util.Set;
import java.util.TreeSet;

/** Task 13 ground truth: the drill/layer expansion, the ripup cost model and `Connection`. */
public class P6T13Probe {

  static final IntBox BOUNDING_BOX = new IntBox(-4000, -4000, 4000, 4000);
  static RoutingBoard board;
  static Padstack smdPad;
  static Padstack thruPad;
  static Padstack viaPad;

  public static void main(String[] args) throws Exception {
    String mode = args.length > 0 ? args[0] : "items";
    switch (mode) {
      case "items" -> items();
      case "ctrl" -> ctrl();
      case "page" -> page();
      case "pagedrills" -> pageDrills();
      case "drill" -> drill();
      case "layers" -> layers();
      case "checklayer" -> checkLayer();
      case "fanoutfac" -> fanoutFactor();
      case "ripup" -> ripup();
      case "random" -> random();
      case "smalldoor" -> smallDoor();
      case "conn" -> conn();
      case "find" -> find();
      default -> throw new IllegalArgumentException("mode " + mode);
    }
  }

  // =============================================================================================
  // The board
  // =============================================================================================

  /**
   * Two layers, a 200-unit clearance matrix, a two-pin component on net 1 (an SMD pad at (-500,0)
   * on layer 0 and a through pad at (500,0) on both layers), a **real** one-via `ViaRule` on the
   * default net class, a foreign-net trace across the channel between the pins and a free via on
   * net 3.
   */
  static void build() {
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    BoardRules rules = new BoardRules(ls, cm);
    rules.setTraceAngleRestriction(AngleRestriction.NONE);
    // The default half width is 1500, which on a 8000-unit board leaves no channel at all: every
    // room would be thin and `checkLayerWithAnyMatchingVia` would answer NOT_DRILLABLE everywhere.
    // 30 matches the traces inserted below.
    rules.setDefaultTraceHalfWidths(30);
    Communication comm = new Communication();
    board = new RoutingBoard(BOUNDING_BOX, ls, new PolylineShape[0], 0, rules, comm);
    board.library.padstacks = new Padstacks(ls);
    board.library.packages = new Packages(board.library.padstacks);

    ConvexShape[] smd = {new IntBox(-50, -50, 50, 50), null};
    smdPad = board.library.padstacks.add("smd", smd, false, false);
    IntOctagon thruShape = new IntOctagon(-70, -70, 70, 70, -140, 140, -140, 140);
    ConvexShape[] thru = {thruShape, thruShape};
    thruPad = board.library.padstacks.add("thru", thru, true, false);
    IntOctagon viaShape = new IntOctagon(-100, -100, 100, 100, -200, 200, -200, 200);
    ConvexShape[] via = {viaShape, viaShape};
    viaPad = board.library.padstacks.add("via", via, true, false);

    ViaInfo viaInfo = new ViaInfo("v", viaPad, 1, false, board.rules);
    board.rules.viaInfos.add(viaInfo);
    ViaRule viaRule = new ViaRule("rule");
    viaRule.appendVia(viaInfo);
    board.rules.viaRules.add(viaRule);
    board.rules.getDefaultNetClass().setViaRule(viaRule);

    board.rules.nets.add("N1", 1, false);
    board.rules.nets.add("N2", 1, false);
    board.rules.nets.add("N3", 1, false);

    Package.Pin[] pins1 = {
      new Package.Pin("P1", smdPad.id, new IntVector(-2000, 0), 0),
      new Package.Pin("P2", thruPad.id, new IntVector(2000, 0), 0)
    };
    Package pkg1 = addPackage("pkg1", pins1);
    Package.Pin[] pins2 = {
      new Package.Pin("P3", smdPad.id, new IntVector(0, -2000), 0),
      new Package.Pin("P4", smdPad.id, new IntVector(0, 2000), 0)
    };
    Package pkg2 = addPackage("pkg2", pins2);
    board.components.add(new IntPoint(0, 0), 0, true, pkg1);
    board.components.add(new IntPoint(0, 0), 0, true, pkg2);

    board.insertPin(1, 0, new int[] {1}, 1, FixedState.UNFIXED); // id 2, the start pin
    board.insertPin(1, 1, new int[] {1}, 1, FixedState.UNFIXED); // id 3, the destination pin
    board.insertPin(2, 0, new int[] {2}, 1, FixedState.UNFIXED); // id 4
    board.insertPin(2, 1, new int[] {2}, 1, FixedState.UNFIXED); // id 5
    // The foreign-net obstacle the ripup model prices: a bent net-2 trace between the two net-2
    // pins, right across the channel between the net-1 pins. The bend makes its `getDetour`
    // strictly above 1.
    Polyline blocker =
        new Polyline(
            new Point[] {new IntPoint(0, -2000), new IntPoint(400, 0), new IntPoint(0, 2000)});
    board.insertTraceWithoutCleaning(blocker, 0, 30, new int[] {2}, 1, FixedState.UNFIXED); // 6
    // A free via on net 3 with exactly one trace contact: `checkRipup`'s via branch with
    // `contactCount == 1`, whose `0.5 * (contactCount - 1)` zeroes the cost factor.
    board.insertVia(viaPad, new IntPoint(2500, 2500), new int[] {3}, 1, FixedState.UNFIXED, false); // 7
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(2500, 2500), new IntPoint(2500, 3500)}),
        0, 30, new int[] {3}, 1, FixedState.UNFIXED); // 8
    // A second via with two trace contacts, so the same branch has a non-zero cost factor.
    board.insertVia(viaPad, new IntPoint(2500, -2500), new int[] {3}, 1, FixedState.UNFIXED, false); // 9
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(2500, -2500), new IntPoint(2500, -3500)}),
        0, 30, new int[] {3}, 1, FixedState.UNFIXED); // 10
    board.insertTraceWithoutCleaning(
        new Polyline(new Point[] {new IntPoint(2500, -2500), new IntPoint(3500, -2500)}),
        0, 30, new int[] {3}, 1, FixedState.UNFIXED); // 11
  }

  /**
   * The `find` mode's board: the same rules and library, one two-pin component and nothing else,
   * on a 2000-unit square, so `findConnection` terminates in a handful of pops.
   */
  static void buildSimple() {
    build();
    board = null;
    Layer[] layers = {new Layer("front", true), new Layer("back", true)};
    LayerStructure ls = new LayerStructure(layers);
    ClearanceMatrix cm = ClearanceMatrix.getDefaultInstance(ls, 200);
    BoardRules rules = new BoardRules(ls, cm);
    rules.setTraceAngleRestriction(AngleRestriction.NONE);
    rules.setDefaultTraceHalfWidths(30);
    board =
        new RoutingBoard(
            new IntBox(-1000, -1000, 1000, 1000), ls, new PolylineShape[0], 0, rules,
            new Communication());
    board.library.padstacks = new Padstacks(ls);
    board.library.packages = new Packages(board.library.padstacks);
    ConvexShape[] smd = {new IntBox(-50, -50, 50, 50), null};
    smdPad = board.library.padstacks.add("smd", smd, false, false);
    IntOctagon thruShape = new IntOctagon(-70, -70, 70, 70, -140, 140, -140, 140);
    ConvexShape[] thru = {thruShape, thruShape};
    thruPad = board.library.padstacks.add("thru", thru, true, false);
    IntOctagon viaShape = new IntOctagon(-100, -100, 100, 100, -200, 200, -200, 200);
    ConvexShape[] via = {viaShape, viaShape};
    viaPad = board.library.padstacks.add("via", via, true, false);
    ViaInfo viaInfo = new ViaInfo("v", viaPad, 1, false, board.rules);
    board.rules.viaInfos.add(viaInfo);
    ViaRule viaRule = new ViaRule("rule");
    viaRule.appendVia(viaInfo);
    board.rules.viaRules.add(viaRule);
    board.rules.getDefaultNetClass().setViaRule(viaRule);
    board.rules.nets.add("N1", 1, false);
    Package.Pin[] pins = {
      new Package.Pin("P1", smdPad.id, new IntVector(-400, 0), 0),
      new Package.Pin("P2", thruPad.id, new IntVector(400, 0), 0)
    };
    Package pkg = addPackage("pkg", pins);
    board.components.add(new IntPoint(0, 0), 0, true, pkg);
    board.insertPin(1, 0, new int[] {1}, 1, FixedState.UNFIXED);
    board.insertPin(1, 1, new int[] {1}, 1, FixedState.UNFIXED);
  }

  static Package addPackage(String name, Package.Pin[] pins) {
    return board.library.packages.add(
        name,
        pins,
        new Shape[0],
        new double[0],
        new boolean[0],
        new Package.Keepout[0],
        new Package.Keepout[0],
        new Package.Keepout[0],
        true);
  }

  // =============================================================================================
  // Helpers
  // =============================================================================================

  static Item itemById(int id) {
    for (Item current : board.getItems()) {
      if (current.getId() == id) {
        return current;
      }
    }
    throw new IllegalStateException("no item " + id);
  }

  static Set<Item> setOf(int... ids) {
    Set<Item> result = new TreeSet<>();
    for (int id : ids) {
      result.add(itemById(id));
    }
    return result;
  }

  static RouterSettings settings() {
    return new RouterSettings(board);
  }

  static AutorouteControl control(int netNo) {
    RouterSettings settings = settings();
    return new AutorouteControl(
        board, netNo, settings, settings.getViaCosts(), settings.getTraceCosts());
  }

  static AutorouteEngine engine(int netNo) {
    AutorouteEngine result = new AutorouteEngine(board, 1, false);
    result.initConnection(netNo, new P6T11Probe.Counter(0), null);
    return result;
  }

  static String fp(FloatPoint p) {
    return P6T11Probe.fp(p);
  }

  static String fl(FloatLine l) {
    return P6T11Probe.fl(l);
  }

  static String describe(ExpandableObject door) {
    return P6T11Probe.describe(door);
  }

  static void dumpQueue(MazeSearchEngine maze) {
    P6T11Probe.dumpQueue(maze);
  }

  static void drain(MazeSearchEngine maze) {
    maze.mazeExpansionList.clear();
  }

  static String box(IntBox b) {
    return "[" + b.ll.x + "," + b.ll.y + ".." + b.ur.x + "," + b.ur.y + "]";
  }

  static String tile(TileShape s) {
    return s == null ? "null" : box(s.boundingBox());
  }

  static Method priv(Class<?> owner, String name, Class<?>... params) throws Exception {
    Method m = owner.getDeclaredMethod(name, params);
    m.setAccessible(true);
    return m;
  }

  // =============================================================================================
  // mode `items`
  // =============================================================================================

  static void items() {
    build();
    for (Item current : board.getItems()) {
      System.out.println(
          "item id="
              + current.getId()
              + " class="
              + current.getClass().getSimpleName()
              + " nets="
              + netsOf(current)
              + " layers="
              + current.firstLayer()
              + ".."
              + current.lastLayer()
              + " routable="
              + current.isRoutable()
              + " bounds="
              + box(current.boundingBox()));
    }
  }

  static String netsOf(Item item) {
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < item.netCount(); i++) {
      if (i > 0) {
        sb.append(",");
      }
      sb.append(item.getNetNumber(i));
    }
    return sb.append("]").toString();
  }

  // =============================================================================================
  // mode `ctrl`
  // =============================================================================================

  static void ctrl() {
    build();
    AutorouteControl c = control(1);
    System.out.println("netNumber=" + c.netNumber);
    System.out.println("layerCount=" + c.layerCount);
    System.out.println("layerActive=" + java.util.Arrays.toString(c.layerActive));
    System.out.println("traceHalfWidth=" + java.util.Arrays.toString(c.traceHalfWidth));
    System.out.println(
        "compensatedTraceHalfWidth=" + java.util.Arrays.toString(c.compensatedTraceHalfWidth));
    System.out.println("viaRadii=" + java.util.Arrays.toString(c.viaRadii));
    System.out.println("maxViaRadius=" + c.maxViaRadius);
    System.out.println("minNormalViaCost=" + c.minNormalViaCost);
    System.out.println("minCheapViaCost=" + c.minCheapViaCost);
    for (int i = 0; i < c.layerCount; i++) {
      System.out.println(
          "addViaCosts[" + i + "]=" + java.util.Arrays.toString(c.addViaCosts[i].toLayer));
      System.out.println(
          "traceCosts["
              + i
              + "] horizontal="
              + c.traceCosts[i].horizontal()
              + " vertical="
              + c.traceCosts[i].vertical());
    }
    System.out.println("viaLowerBound=" + c.viaLowerBound + " viaUpperBound=" + c.viaUpperBound);
    System.out.println("viaClearanceClass=" + c.viaClearanceClass);
    System.out.println("viaInfos.length=" + c.viaInfos.length);
    for (int i = 0; i < c.viaInfos.length; i++) {
      System.out.println(
          "viaInfos["
              + i
              + "] fromLayer="
              + c.viaInfos[i].fromLayer
              + " toLayer="
              + c.viaInfos[i].toLayer
              + " attachSmdAllowed="
              + c.viaInfos[i].attachSmdAllowed);
    }
    System.out.println("attachSmdAllowed=" + c.attachSmdAllowed);
    System.out.println("viasAllowed=" + c.viasAllowed);
    System.out.println("ripupAllowed=" + c.ripupAllowed);
    System.out.println("ripupCosts=" + c.ripupCosts);
    System.out.println("ripupPassNo=" + c.ripupPassNo);
    System.out.println("removeUnconnectedVias=" + c.removeUnconnectedVias);
    System.out.println("isFanout=" + c.isFanout);
    System.out.println("startRipupCosts=" + c.settings.getStartRipupCosts());
    System.out.println("traceClearanceClassIndex=" + c.traceClearanceClassIndex);
    System.out.println("maxShoveTraceRecursionDepth=" + c.maxShoveTraceRecursionDepth);
    System.out.println("bendCosts=" + java.util.Arrays.toString(c.bendCosts));
    System.out.println("withNeckdown=" + c.withNeckdown);
  }

  // =============================================================================================
  // The shared start point for the expander modes: one seeded element on the start pin's room
  // =============================================================================================

  /** `getInstance(start = {SMD pin 2}, destination = {through pin 3})` on the shared board. */
  static MazeSearchEngine maze(AutorouteEngine autorouteEngine, AutorouteControl control) {
    return MazeSearchEngine.getInstance(setOf(2), setOf(3), autorouteEngine, control);
  }

  static MazeListElement firstElement(MazeSearchEngine maze) {
    return maze.mazeExpansionList.iterator().next();
  }

  static void dumpPages(AutorouteEngine autorouteEngine, TileShape shape) {
    Collection<DrillPage> pages = autorouteEngine.drillPageArray.overlappingPages(shape);
    System.out.println("overlappingPages n=" + pages.size());
    for (DrillPage p : pages) {
      System.out.println("  page " + box(p.shape) + " getId=" + p.getId());
    }
  }

  // =============================================================================================
  // mode `page`
  // =============================================================================================

  static void page() throws Exception {
    build();
    AutorouteEngine autorouteEngine = engine(1);
    AutorouteControl control = control(1);
    MazeSearchEngine maze = maze(autorouteEngine, control);
    System.out.println("instance=" + (maze == null ? "null" : "ok"));
    MazeListElement from = firstElement(maze);
    System.out.println("from door=" + describe(from.door) + " section=" + from.sectionNoOfDoor);
    System.out.println(
        "from expansion="
            + String.format("%.9f", from.expansionValue)
            + " sorting="
            + String.format("%.9f", from.sortingValue)
            + " shapeEntry="
            + fl(from.shapeEntry)
            + " nextRoom="
            + (from.nextRoom == null ? "null" : from.nextRoom.getId())
            + " layer="
            + (from.nextRoom == null ? "-" : from.nextRoom.getLayer()));
    dumpPages(autorouteEngine, from.nextRoom.getShape());
    MazeExpansionEngine expander = new MazeExpansionEngine(maze);
    for (DrillPage p : autorouteEngine.drillPageArray.overlappingPages(from.nextRoom.getShape())) {
      drain(maze);
      expander.expandToDrillPage(p, from);
      System.out.println("--- page " + box(p.shape));
      dumpQueue(maze);
    }
  }

  // =============================================================================================
  // mode `pagedrills`
  // =============================================================================================

  static void pageDrills() throws Exception {
    build();
    AutorouteEngine autorouteEngine = engine(1);
    AutorouteControl control = control(1);
    MazeSearchEngine maze = maze(autorouteEngine, control);
    MazeListElement from = firstElement(maze);
    MazeExpansionEngine expander = new MazeExpansionEngine(maze);
    DrillPage page =
        autorouteEngine.drillPageArray.overlappingPages(from.nextRoom.getShape()).iterator().next();
    System.out.println("page " + box(page.shape));
    // Java: `expandToDrillPage` first, so the element that reaches `expandToDrillsOfPage` is the
    // one the pop loop would hand it.
    drain(maze);
    expander.expandToDrillPage(page, from);
    MazeListElement pageElement = firstElement(maze);
    System.out.println(
        "pageElement door="
            + describe(pageElement.door)
            + " section="
            + pageElement.sectionNoOfDoor
            + String.format(
                " expansion=%.9f sorting=%.9f",
                pageElement.expansionValue, pageElement.sortingValue));
    Collection<ExpansionDrill> drills = page.getDrills(autorouteEngine, control.attachSmdAllowed);
    System.out.println("drills n=" + drills.size());
    int i = 0;
    for (ExpansionDrill d : drills) {
      System.out.println(
          "  drill["
              + i
              + "] location="
              + d.location
              + " firstLayer="
              + d.firstLayer
              + " lastLayer="
              + d.lastLayer
              + " getId="
              + d.getId()
              + " shape="
              + tile(d.getShape())
              + " room0="
              + (d.roomArr[0] == null ? "null" : d.roomArr[0].getId())
              + " room1="
              + (d.roomArr[1] == null ? "null" : d.roomArr[1].getId()));
      i++;
    }
    drain(maze);
    expander.expandToDrillsOfPage(pageElement);
    dumpQueue(maze);
  }

  // =============================================================================================
  // mode `drill`
  // =============================================================================================

  static void drill() throws Exception {
    build();
    AutorouteEngine autorouteEngine = engine(1);
    AutorouteControl control = control(1);
    MazeSearchEngine maze = maze(autorouteEngine, control);
    MazeListElement from = firstElement(maze);
    MazeExpansionEngine expander = new MazeExpansionEngine(maze);
    DrillPage page =
        autorouteEngine.drillPageArray.overlappingPages(from.nextRoom.getShape()).iterator().next();
    drain(maze);
    expander.expandToDrillPage(page, from);
    MazeListElement pageElement = firstElement(maze);
    Collection<ExpansionDrill> drills = page.getDrills(autorouteEngine, control.attachSmdAllowed);
    ExpansionDrill first = drills.iterator().next();

    System.out.println(
        "room minWidth="
            + String.format("%.9f", from.nextRoom.getShape().minWidth())
            + " 2*traceHalfWidth="
            + (2 * control.compensatedTraceHalfWidth[0]));
    for (int addCosts : new int[] {0, 250}) {
      // (a) through the drill page's own element: `:78-80`, no `minNormalViaCost`.
      drain(maze);
      expander.expandToDrill(first, pageElement, addCosts);
      System.out.println("--- fromPage addCosts=" + addCosts);
      dumpQueue(maze);
      // (b) through the room's own door: `:81-85`, which adds `minNormalViaCost`.
      drain(maze);
      expander.expandToDrill(first, from, addCosts);
      System.out.println("--- fromDoor addCosts=" + addCosts);
      dumpQueue(maze);
    }

    // `:35-51`, the thin-room refusal. The room is 2134 wide, so the only way to make it thin is
    // to widen the trace; the pass runner does the same thing through the net class.
    int saved = control.compensatedTraceHalfWidth[0];
    control.compensatedTraceHalfWidth[0] = 2000;
    System.out.println(
        "thin: minWidth="
            + String.format("%.9f", from.nextRoom.getShape().minWidth())
            + " 2*traceHalfWidth="
            + (2 * control.compensatedTraceHalfWidth[0]));
    drain(maze);
    expander.expandToDrill(first, from, 0);
    System.out.println("--- thin, backtrackDoor=null");
    dumpQueue(maze);
    // The same element with a backtrack door whose shape the drill does **not** intersect.
    MazeListElement withBacktrack =
        new MazeListElement(
            from.door,
            0,
            pageElement.door,
            0,
            from.expansionValue,
            from.sortingValue,
            from.nextRoom,
            from.shapeEntry,
            false,
            MazeSearchElement.Adjustment.NONE,
            false);
    System.out.println(
        "drill.shape="
            + tile(first.getShape())
            + " backtrack.shape="
            + tile(pageElement.door.getShape())
            + " intersects="
            + first.getShape().intersects(pageElement.door.getShape()));
    drain(maze);
    expander.expandToDrill(first, withBacktrack, 0);
    System.out.println("--- thin, backtrackDoor=the whole page");
    dumpQueue(maze);
    control.compensatedTraceHalfWidth[0] = saved;
  }

  // =============================================================================================
  // mode `layers`
  // =============================================================================================

  static void layers() throws Exception {
    build();
    AutorouteEngine autorouteEngine = engine(1);
    AutorouteControl control = control(1);
    MazeSearchEngine maze = maze(autorouteEngine, control);
    MazeListElement from = firstElement(maze);
    MazeExpansionEngine expander = new MazeExpansionEngine(maze);
    DrillPage page =
        autorouteEngine.drillPageArray.overlappingPages(from.nextRoom.getShape()).iterator().next();
    drain(maze);
    expander.expandToDrillPage(page, from);
    MazeListElement pageElement = firstElement(maze);
    Collection<ExpansionDrill> drills = page.getDrills(autorouteEngine, control.attachSmdAllowed);
    drain(maze);
    expander.expandToDrillsOfPage(pageElement);
    System.out.println("drillElements n=" + maze.mazeExpansionList.size());
    MazeListElement drillElement = firstElement(maze);
    System.out.println(
        "drillElement door="
            + describe(drillElement.door)
            + " section="
            + drillElement.sectionNoOfDoor
            + String.format(
                " expansion=%.9f sorting=%.9f",
                drillElement.expansionValue, drillElement.sortingValue));
    drain(maze);
    expander.expandToOtherLayers(drillElement);
    System.out.println("--- free space");
    dumpQueue(maze);

    // `addViaCosts` is all-zero out of the constructor (`AutorouteControl.java:174-178`), so the
    // `:355` term would be invisible. The pass runner is what fills it in production.
    control.addViaCosts[0].toLayer[1] = 700;
    control.addViaCosts[1].toLayer[0] = 900;
    drain(maze);
    expander.expandToOtherLayers(drillElement);
    System.out.println("--- free space addViaCosts[0][1]=700");
    dumpQueue(maze);

    // The `ObstacleExpansionRoom` arm of `:246-261`: the free via on net 3, whose padstack is the
    // control's own and whose clearance class matches.
    Via freeVia = (Via) itemById(7);
    ExpansionDrill viaDrill = freeVia.getAutorouteDrillInfo(autorouteEngine.autorouteSearchTree);
    System.out.println(
        "viaDrill location="
            + viaDrill.location
            + " firstLayer="
            + viaDrill.firstLayer
            + " lastLayer="
            + viaDrill.lastLayer
            + " getId="
            + viaDrill.getId()
            + " room0="
            + describeRoom(viaDrill.roomArr[0])
            + " room1="
            + describeRoom(viaDrill.roomArr[1]));
    MazeListElement viaElement =
        new MazeListElement(
            viaDrill,
            0,
            drillElement.door,
            0,
            1000.0,
            2000.0,
            viaDrill.roomArr[0],
            new FloatLine(new FloatPoint(2000, 2000), new FloatPoint(2000, 2000)),
            false,
            MazeSearchElement.Adjustment.NONE,
            false);
    for (boolean ripupAllowed : new boolean[] {false, true}) {
      control.ripupAllowed = ripupAllowed;
      drain(maze);
      expander.expandToOtherLayers(viaElement);
      System.out.println("--- obstacle via ripupAllowed=" + ripupAllowed);
      dumpQueue(maze);
    }
    control.ripupAllowed = false;
  }

  static String describeRoom(CompleteExpansionRoom room) {
    if (room == null) {
      return "null";
    }
    return room.getClass().getSimpleName()
        + " id="
        + room.getId()
        + " layer="
        + room.getLayer()
        + " shape="
        + tile(room.getShape());
  }

  // =============================================================================================
  // mode `checklayer`
  // =============================================================================================

  static void checkLayer() throws Exception {
    build();
    AutorouteEngine autorouteEngine = engine(1);
    AutorouteControl control = control(1);
    MazeSearchEngine maze = maze(autorouteEngine, control);
    MazeExpansionEngine expander = new MazeExpansionEngine(maze);
    Method m =
        priv(
            MazeExpansionEngine.class,
            "checkLayerWithAnyMatchingVia",
            ExpansionDrill.class,
            int.class,
            TileShape.class,
            int[].class);
    Object[][] spots = {
      {"freeSpace", new IntPoint(1000, 1000)},
      {"onBlocker", new IntPoint(400, 0)},
      {"onSmdPin", new IntPoint(-2000, 0)},
      {"onThruPin", new IntPoint(2000, 0)},
      {"onFreeVia", new IntPoint(2500, 2500)}
    };
    for (int halfSize : new int[] {60, 400}) {
      for (Object[] spot : spots) {
        IntPoint location = (IntPoint) spot[1];
        TileShape roomShape =
            new IntBox(
                    location.x - halfSize,
                    location.y - halfSize,
                    location.x + halfSize,
                    location.y + halfSize)
                .toSimplex();
        ExpansionDrill d = new ExpansionDrill(TileShape.getInstance(location), location, 0, 1);
        for (int layer = 0; layer < 2; layer++) {
          Object result = m.invoke(expander, d, layer, roomShape, new int[] {1});
          System.out.println(
              "  room=" + (2 * halfSize) + " spot=" + spot[0] + " layer=" + layer + " -> " + result);
        }
      }
    }
  }

  // =============================================================================================
  // mode `fanoutfac`
  // =============================================================================================

  static void fanoutFactor() {
    build();
    for (int id : new int[] {6, 8, 10, 11}) {
      Trace t = (Trace) itemById(id);
      System.out.println(
          "trace id="
              + id
              + " halfWidth="
              + t.getHalfWidth()
              + " length="
              + String.format("%.9f", t.getLength())
              + " startContacts="
              + t.getStartContacts().size()
              + " endContacts="
              + t.getEndContacts().size()
              + " factor="
              + String.format(
                  "%.9f", MazeRipupResolver.calcFanoutViaRipupCostFactor(t)));
    }
    // A short SHOVE_FIXED two-corner trace contacting the free via at one end: the second
    // `protectFanoutVia` arm of `:51-56`.
    Polyline stub = new Polyline(new Point[] {new IntPoint(2500, 2500), new IntPoint(2500, 2900)});
    board.insertTraceWithoutCleaning(stub, 1, 30, new int[] {3}, 1, FixedState.SHOVE_FIXED);
    Polyline attached =
        new Polyline(new Point[] {new IntPoint(2500, 2900), new IntPoint(3100, 2900)});
    Trace attachedTrace =
        board.insertTraceWithoutCleaning(attached, 1, 30, new int[] {3}, 1, FixedState.UNFIXED);
    System.out.println(
        "attached halfWidth="
            + attachedTrace.getHalfWidth()
            + " length="
            + String.format("%.9f", attachedTrace.getLength())
            + " startContacts="
            + attachedTrace.getStartContacts().size()
            + " endContacts="
            + attachedTrace.getEndContacts().size()
            + " factor="
            + String.format(
                "%.9f", MazeRipupResolver.calcFanoutViaRipupCostFactor(attachedTrace)));
  }

  // =============================================================================================
  // mode `ripup`
  // =============================================================================================

  static void ripup() throws Exception {
    build();
    AutorouteEngine autorouteEngine = engine(1);
    AutorouteControl control = control(1);
    MazeSearchEngine maze = maze(autorouteEngine, control);
    MazeRipupResolver resolver = new MazeRipupResolver(maze);
    MazeListElement from = firstElement(maze);
    Item blocker = itemById(6);
    Item freeVia = itemById(7);
    Item twoContactVia = itemById(9);
    Item smdPin = itemById(2);

    System.out.println("--- not routable");
    System.out.println("smdPin -> " + resolver.checkRipup(from, smdPin, false));

    for (int ripupCosts : new int[] {1000, 100000, 2000000000}) {
      control.ripupCosts = ripupCosts;
      System.out.println("--- ripupCosts=" + ripupCosts);
      System.out.println("blocker -> " + resolver.checkRipup(from, blocker, false));
      System.out.println("freeVia -> " + resolver.checkRipup(from, freeVia, false));
      System.out.println("twoContactVia -> " + resolver.checkRipup(from, twoContactVia, false));
    }
    control.ripupCosts = 1000;

    System.out.println("--- removeUnconnectedVias=false (fanout protection on)");
    control.removeUnconnectedVias = false;
    System.out.println("blocker -> " + resolver.checkRipup(from, blocker, false));
    System.out.println("freeVia -> " + resolver.checkRipup(from, freeVia, false));
    System.out.println("twoContactVia -> " + resolver.checkRipup(from, twoContactVia, false));
    control.removeUnconnectedVias = true;

    System.out.println("--- isFanout=true");
    control.isFanout = true;
    System.out.println("blocker -> " + resolver.checkRipup(from, blocker, false));
    control.isFanout = false;

    System.out.println("--- randomize");
    for (int passNo : new int[] {3, 4, 5, 6, 7}) {
      control.ripupPassNo = passNo;
      System.out.println(
          "passNo=" + passNo + " blocker -> " + resolver.checkRipup(from, blocker, false));
    }
    control.ripupPassNo = 1;

    System.out.println("--- doorIsSmall");
    System.out.println("blocker small -> " + resolver.checkRipup(from, blocker, true));
  }

  // =============================================================================================
  // mode `random`
  // =============================================================================================

  static void random() {
    for (long seed : new long[] {1000L, 5000L, 17L}) {
      java.util.Random r = new java.util.Random(seed);
      StringBuilder sb = new StringBuilder("seed=" + seed);
      for (int i = 0; i < 3; i++) {
        double d = r.nextDouble();
        sb.append(String.format(" %.17f", d));
      }
      System.out.println(sb);
    }
  }

  // =============================================================================================
  // mode `smalldoor`
  // =============================================================================================

  static void smallDoor() throws Exception {
    build();
    AutorouteEngine autorouteEngine = engine(1);
    AutorouteControl control = control(1);
    MazeSearchEngine maze = maze(autorouteEngine, control);
    MazeRipupResolver resolver = new MazeRipupResolver(maze);
    Method m =
        priv(
            MazeRipupResolver.class,
            "enterThroughSmallDoor",
            MazeListElement.class,
            Item.class);
    MazeListElement from = firstElement(maze);
    System.out.println(
        "from door="
            + describe(from.door)
            + " dimension="
            + from.door.getDimension()
            + " -> "
            + m.invoke(resolver, from, itemById(6)));
    System.out.println("checkLeavingRippedItem(from) -> " + resolver.checkLeavingRippedItem(from));

    // Every door of every complete room, against the blocker trace (net 2) and against the free
    // via (net 3): a door whose neighbourhood holds an item of neither net answers false.
    for (MazeListElement seeded : new java.util.ArrayList<>(maze.mazeExpansionList)) {
      autorouteEngine.completeNeighbourRooms(seeded.nextRoom);
    }
    java.lang.reflect.Field f = AutorouteEngine.class.getDeclaredField("completeExpansionRooms");
    f.setAccessible(true);
    @SuppressWarnings("unchecked")
    Collection<app.freerouting.autoroute.expansion.CompleteFreeSpaceExpansionRoom> rooms =
        (Collection<app.freerouting.autoroute.expansion.CompleteFreeSpaceExpansionRoom>)
            f.get(autorouteEngine);
    for (app.freerouting.autoroute.expansion.CompleteFreeSpaceExpansionRoom room : rooms) {
      System.out.println("room " + room.getId() + " layer=" + room.getLayer());
      for (ExpansionDoor door : room.getDoors()) {
        CompleteExpansionRoom other = door.otherRoom(room);
        MazeListElement element =
            new MazeListElement(
                door,
                0,
                from.door,
                0,
                0.0,
                0.0,
                room,
                from.shapeEntry,
                false,
                MazeSearchElement.Adjustment.NONE,
                false);
        System.out.println(
            "  door id="
                + door.getId()
                + " dim="
                + door.getDimension()
                + " shape="
                + tile(door.getShape())
                + " other="
                + (other == null ? "null" : other.getClass().getSimpleName())
                + " blocker="
                + m.invoke(resolver, element, itemById(6))
                + " via="
                + m.invoke(resolver, element, itemById(7))
                + " leaving="
                + resolver.checkLeavingRippedItem(element));
      }
    }
  }

  // =============================================================================================
  // mode `conn`
  // =============================================================================================

  static void conn() {
    build();
    for (int id : new int[] {2, 3, 4, 5, 6, 7, 8, 9, 10, 11}) {
      Item item = itemById(id);
      Connection c = Connection.get(item);
      if (c == null) {
        System.out.println("item " + id + " -> null");
        continue;
      }
      StringBuilder sb = new StringBuilder("[");
      for (Item connectionItem : c.itemList) {
        if (sb.length() > 1) {
          sb.append(",");
        }
        sb.append(connectionItem.getId());
      }
      sb.append("]");
      System.out.println(
          "item "
              + id
              + " -> items="
              + sb
              + " start="
              + (c.startPoint == null ? "null" : c.startPoint.toString())
              + " startLayer="
              + c.startLayer
              + " end="
              + (c.endPoint == null ? "null" : c.endPoint.toString())
              + " endLayer="
              + c.endLayer
              + String.format(" traceLength=%.9f detour=%.9f", c.traceLength(), c.getDetour()));
      System.out.println(
          "  memoised=" + (item.getAutorouteInfo().getPrecalculatedConnection() == c));
    }
  }

  // =============================================================================================
  // mode `find`
  // =============================================================================================

  static void find() throws Exception {
    buildSimple();
    AutorouteEngine autorouteEngine = engine(1);
    AutorouteControl control = control(1);
    MazeSearchEngine maze = maze(autorouteEngine, control);
    System.out.println("instance=" + (maze == null ? "null" : "ok"));
    System.out.println("--- seeded");
    dumpQueue(maze);
    int pops = 0;
    while (true) {
      MazeListElement head =
          maze.mazeExpansionList.isEmpty() ? null : maze.mazeExpansionList.iterator().next();
      String headText =
          head == null
              ? "empty"
              : describe(head.door)
                  + " section="
                  + head.sectionNoOfDoor
                  + String.format(
                      " expansion=%.9f sorting=%.9f", head.expansionValue, head.sortingValue);
      boolean more = maze.occupyNextElement();
      System.out.println(
          "pop["
              + pops
              + "] head="
              + headText
              + " -> more="
              + more
              + " queue="
              + maze.mazeExpansionList.size());
      if (!more) {
        break;
      }
      pops++;
      if (pops > 200) {
        System.out.println("ABORT: more than 200 pops");
        break;
      }
    }
    ExpandableObject destination = P6T11Probe.destinationDoor(maze);
    System.out.println(
        "destinationDoor="
            + describe(destination)
            + " section="
            + P6T11Probe.sectionNoOfDestinationDoor(maze));
    System.out.println("--- final");
    dumpQueue(maze);
  }
}
