package app.freerouting.autoroute.pipeline;

import static app.freerouting.autoroute.pipeline.BatchAutorouter.BOARD_RANK_LIMIT;
import static app.freerouting.autoroute.pipeline.BatchAutorouter.FANOUT_RECOVERY_STAGNATION_PASSES;
import static app.freerouting.autoroute.pipeline.BatchAutorouter.MAXIMUM_TRIES_ON_THE_SAME_BOARD;
import static app.freerouting.autoroute.pipeline.BatchAutorouter.STAGNATION_PASS_LIMIT;
import static app.freerouting.autoroute.pipeline.BatchAutorouter.STAGNATION_SCORE_THRESHOLD;
import static app.freerouting.autoroute.pipeline.BatchAutorouter.STOP_AT_PASS_MINIMUM;
import static app.freerouting.autoroute.pipeline.BatchAutorouter.STOP_AT_PASS_MODULO;

import app.freerouting.autoroute.BoardHistory;
import app.freerouting.autoroute.ItemRouteResult;
import app.freerouting.autoroute.events.TaskStateChangedEvent;
import app.freerouting.autoroute.events.TaskStateChangedEventListener;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.items.Pin;
import app.freerouting.board.model.items.Via;
import app.freerouting.board.trace.PolylineTrace;
import app.freerouting.core.RoutingJob;
import app.freerouting.core.scoring.BoardStatistics;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.IntPoint;
import app.freerouting.geometry.planar.Line;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.Polyline;
import app.freerouting.io.specctra.SesWriter;
import app.freerouting.board.actions.ItemIdGenerator;
import app.freerouting.management.HeadlessBoardManager;
import app.freerouting.settings.FanoutSettings;
import app.freerouting.settings.RouterSettings;
import app.freerouting.settings.SettingsMerger;
import app.freerouting.settings.sources.ApiSettings;
import app.freerouting.settings.sources.CliSettings;
import app.freerouting.settings.sources.DefaultSettings;
import app.freerouting.settings.sources.DsnFileSettings;
import app.freerouting.settings.sources.EnvironmentVariablesSource;
import app.freerouting.settings.sources.JsonFileSettings;
import java.io.ByteArrayInputStream;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.List;

/**
 * Plan 7 Task 10 differential driver, whole-board level: {@code AutorouteBatchLoop.run}
 * (AutorouteBatchLoop.java:37-588) — the pass loop, its best-board policy and its two stagnation
 * detectors — over a real DSN board, against the port's {@code AutorouteBatchLoop::run}.
 *
 * <p>Usage: {@code P7T9 <dsn> <maxPasses> <mode>}. {@code maxPasses} is an integer written straight
 * into {@code settings.maxPasses} ({@code 0} is Java's "unlimited", quirk #140); {@code mode} is
 * {@code router-only} ({@code fanout.enabled = false}, {@code runOptimizer = false}) or, from
 * Plan 7 Task 12, {@code router+fanout}, which turns the SMD fanout pre-pass ({@code :89-173})
 * on. In the second mode the per-pin fanout clock is disabled on <b>both</b> sides through
 * {@code settings.fanout.maxMillisecondsPerPin = Integer.MAX_VALUE} (ruling AI), which is where
 * {@code BatchFanout.fanoutPass:231-232} builds its {@code TimeLimit} from.
 *
 * <h2>The shape of the run, and why there are two halves</h2>
 *
 * <p>Same problem and same answer as {@link P7T2}: {@code run()} is a single 552-line method that
 * returns one {@code boolean} and writes its state into {@code router.board} and {@code job.board}.
 * Driving it directly gives a one-line transcript, which localises nothing when pass 6 of 8
 * diverges. So the driver does both:
 *
 * <ol>
 *   <li><b>{@code [transcript]}</b> — {@code run}'s body <b>transcribed</b> here, line for line
 *       against {@code :37-588}, calling the <i>real</i> {@code autoroutePass}, {@code removeTails},
 *       {@code calculateIncompleteCount}, {@code BoardStatistics} and {@code BoardHistory}, and
 *       printing the per-pass {@code PassRecord} tuple plus every decision the loop takes — the
 *       restore gate, the rank test, the two stagnation arms and the final best-board swap.
 *   <li><b>{@code [real]}</b> — a <b>freshly loaded board</b> with the same settings, and then the
 *       real {@code BatchAutorouter.runBatchLoop()} ({@code :479-481}), i.e. {@code run()} itself.
 *       Its return value and its board are printed, and the board is compared against the
 *       transcript's by {@code BasicBoard.getHash()}.
 * </ol>
 *
 * <p>The second half is what makes the first half evidence rather than a parallel implementation:
 * if the transcription ever drifted from the method, {@code equalsTranscript} would go false on the
 * Java side alone and the run would diff against a port that had not changed. The port's twin runs
 * the same two halves, comparing with {@code Board::structural_hash} — an <b>equality decision</b>,
 * never a hash value (controller ruling AH).
 *
 * <h2>Why {@code runBatchLoop()} and not {@code RoutingPipeline.createForHeadless(job).run()}</h2>
 *
 * <p>The plan's text names the pipeline entry point. Under this driver's settings the two are the
 * same board, and the pipeline adds one call the port's {@code AutorouteBatchLoop::run} does not
 * make — so driving the pipeline would compare the port's loop against a board Java mutated
 * afterwards. Read out of {@code RoutingPipeline.java:81-110}:
 *
 * <ul>
 *   <li>{@code runRoutingStage} computes {@code routerEnabled = getRunRouter() && (maxPasses ==
 *       null || maxPasses >= 0)} ({@code :87-90}), which is {@code true} here, and the stop flag is
 *       {@code NONE} at entry — so {@code :98} calls {@code this.autorouter.runBatchLoop()}, the
 *       one this driver calls;
 *   <li>{@code :106} then calls {@code this.job.board.finishAutoroute()}, whose whole body is
 *       {@code autorouteEngine.clear(); autorouteEngine = null;} (RoutingBoard.java:900-905) on a
 *       <b>transient</b> field (RoutingBoard.java:70) that {@code serialize(true)} does not write;
 *   <li>{@code runOptimizationStage} returns at {@code :113} because {@code this.optimizer} is
 *       {@code null} — {@code RoutingPipeline}'s constructor builds one only when
 *       {@code getRunOptimizer()} ({@code :35}), and this driver sets it {@code false}.
 * </ul>
 *
 * <p>The {@code FINISH-AUTOROUTE} line at the end of the {@code [real]} half is that argument
 * turned into a measurement: it calls {@code finishAutoroute()} and prints whether the hash moved.
 * It reads {@code moved=false} on every stem, which is what makes "the pipeline's routing stage is
 * this call" a fact rather than a reading of the source.
 *
 * <h2>The budget</h2>
 *
 * <p>Ruling AI asks parity runs to disable the wall clock on both sides. As {@code P7T8Probe}'s
 * class comment records, the Java side cannot: {@code TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP} is a
 * compile-time constant that {@code javac} inlines at both call sites. The port runs with
 * {@code RouterBudget::disabled()} against this side's live 1000 ms limit, so a MATCH proves the
 * limit never trips on the corpus rather than assuming it. The loop's own two clocks —
 * {@code sessionStartTime} ({@code :62}) and {@code FRLogger.traceEntry}/{@code traceExit}
 * ({@code :286-291}, {@code :345-351}) — are reporting only and are not printed by either side.
 */
public final class P7T9 {

  private P7T9() {}

  public static void main(String[] args) throws Exception {
    if (args.length < 1) {
      System.err.println(
          "usage: P7T9 <dsn> [maxPasses] [mode] [optPasses|all] [optItems|all]"
              + " [--fanout on|off] [--optimizer on|off] [--ses <path>] [--passes <path>]");
      System.exit(2);
    }
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    // Plan 7 Task 16's four trailing flags. They are parsed out first so the five positionals
    // keep the meanings Tasks 10/14/15 gave them and no existing invocation changes.
    List<String> positional = new ArrayList<>();
    Path sesPath = null;
    Path passesPath = null;
    Boolean fanoutFlag = null;
    Boolean optimizerFlag = null;
    for (int i = 0; i < args.length; i++) {
      switch (args[i]) {
        case "--ses" -> sesPath = Paths.get(args[++i]).toAbsolutePath();
        case "--passes" -> passesPath = Paths.get(args[++i]).toAbsolutePath();
        case "--fanout" -> fanoutFlag = onOff(args[++i]);
        case "--optimizer" -> optimizerFlag = onOff(args[++i]);
        default -> positional.add(args[i]);
      }
    }
    args = positional.toArray(new String[0]);

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    int maxPasses = args.length > 1 && !args[1].isBlank() ? Integer.parseInt(args[1]) : 1;
    String mode = args.length > 2 && !args[2].isBlank() ? args[2] : "router-only";
    if (!"router-only".equals(mode)
        && !"router+fanout".equals(mode)
        && !"optimizer".equals(mode)
        && !"optimizer+fanout".equals(mode)
        && !"optimizer-shared".equals(mode)
        && !"full".equals(mode)
        && !"batch".equals(mode)
        && !"batch-router".equals(mode)) {
      System.err.println(
          "P7T9: mode must be 'router-only', 'router+fanout', 'optimizer', 'optimizer+fanout',"
              + " 'optimizer-shared', 'full', 'batch' or 'batch-router', not: "
              + mode);
      System.exit(2);
    }
    // `null` is Java's "no limit" for both settings (`:167-170`, `:318-320`), and `all` is how the
    // command line spells it — a real arm of both gates, not a large number standing in for one.
    Integer optPasses = args.length > 3 && !args[3].isBlank() ? boxedLimit(args[3]) : Integer.valueOf(2);
    Integer optItems = args.length > 4 && !args[4].isBlank() ? boxedLimit(args[4]) : null;

    Path jar =
        Paths.get(RoutingBoard.class.getProtectionDomain().getCodeSource().getLocation().toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d fixture=%s maxPasses=%d mode=%s%s%n",
        jar,
        Files.size(jar),
        Files.getLastModifiedTime(jar).toMillis(),
        dsn.getFileName(),
        maxPasses,
        mode,
        isBatchMode(mode)
            ? " fanout="
                + onOffName(fanoutFlag == null || fanoutFlag)
                + " optimizer="
                + onOffName(optimizerFlag == null || optimizerFlag)
            : (isOptimizerMode(mode) || "full".equals(mode))
                ? " optPasses=" + limitName(optPasses) + " optItems=" + limitName(optItems)
                : "");
    System.err.println("java-version " + System.getProperty("java.version"));

    if (isBatchMode(mode)) {
      runBatchMode(
          out,
          dsn,
          maxPasses,
          mode,
          fanoutFlag == null || fanoutFlag,
          optimizerFlag == null || optimizerFlag,
          sesPath,
          passesPath);
      out.flush();
      return;
    }

    if (isOptimizerMode(mode)) {
      runOptimizerMode(out, dsn, maxPasses, mode, optPasses, optItems);
      out.flush();
      return;
    }

    if ("full".equals(mode)) {
      runFullMode(out, dsn, maxPasses, optPasses, optItems);
      out.flush();
      return;
    }

    // ---- half one: the transcription -----------------------------------------------------
    RoutingBoard board = P7T2.loadBoard(dsn);
    RouterSettings settings = buildSettings(board, maxPasses, mode);
    BatchAutorouter router = P7T2.newRouter(board, settings);
    out.println("[transcript]");
    boolean transcriptReturned = transcribeRun(out, router, settings);
    // `:552` — `job.board = router.board`. The board the transcript finished with is
    // `router.board`, which the restore arms may have replaced; read it back rather than reusing
    // the local, which is the very staleness quirk #209 records.
    RoutingBoard transcriptBoard = router.board;
    out.println(
        "TRANSCRIPT returned=" + transcriptReturned + " " + P7T2.boardShape(transcriptBoard));
    String transcriptHash = transcriptBoard.getHash();

    // ---- half two: the real method -------------------------------------------------------
    RoutingBoard realBoard = P7T2.loadBoard(dsn);
    RouterSettings realSettings = buildSettings(realBoard, maxPasses, mode);
    BatchAutorouter realRouter = P7T2.newRouter(realBoard, realSettings);
    out.println("[real]");
    boolean realReturned = realRouter.runBatchLoop();
    RoutingBoard realFinal = realRouter.board;
    String realHash = realFinal.getHash();
    out.println(
        "REAL returned="
            + realReturned
            + " "
            + P7T2.boardShape(realFinal)
            + " equalsTranscript="
            + realHash.equals(transcriptHash));
    // The `RoutingPipeline.java:106` call the class comment argues is a no-op here, measured.
    realFinal.finishAutoroute();
    out.println("FINISH-AUTOROUTE moved=" + !realFinal.getHash().equals(realHash));

    // ---- the final board, in `P6T15aProbe`'s polyline format ------------------------------
    out.println("[board]");
    dumpBoard(out, transcriptBoard);
    out.flush();
  }

  /**
   * {@code P7T2.buildSettings} plus this driver's three knobs: {@code maxPasses}, fanout off and
   * the optimizer off — the {@code router-only} mode.
   *
   * <p>{@code setFanoutEnabled} rather than a raw field write, because {@code
   * BatchAutorouter}'s constructor derives {@code removeUnconnectedVias = !isFanoutEnabled()}
   * (BatchAutorouter.java:115) and {@code isFanoutEnabled} answers {@code false} only when the
   * {@code FanoutSettings} object exists and its {@code enabled} is not {@code TRUE}
   * (RouterSettings.java:578-580). Both are set before the router is built.
   */
  static RouterSettings buildSettings(RoutingBoard board, int maxPasses, String mode) {
    RouterSettings settings = P7T2.buildSettings(board);
    settings.maxPasses = maxPasses;
    if (settings.fanout == null) {
      settings.fanout = new FanoutSettings();
    }
    settings.fanout.enabled =
        (mode.endsWith("+fanout")) ? Boolean.TRUE : Boolean.FALSE;
    // Ruling AI: `fanoutPass:231-232`'s per-pin `TimeLimit` is built from this setting, so this
    // is where the fanout stage's wall clock is disabled — on both sides, with the same number.
    settings.fanout.maxMillisecondsPerPin = (long) Integer.MAX_VALUE;
    settings.setRunRouter(true);
    // `RoutingPipeline`'s constructor (`RoutingPipeline.java:36`) builds an optimizer only when
    // this is set, which is what makes the optimizer modes a *pipeline* shape rather than a
    // driver invention.
    settings.setRunOptimizer(isOptimizerMode(mode));
    // `:408-410` — the snapshot event. Off, so `fireBoardSnapshotEvent` is not on this run's path;
    // the port's `RoutingEvent::BoardSnapshot` is pinned by a unit test instead.
    settings.saveIntermediateStages = Boolean.FALSE;
    return settings;
  }

  /** The three modes that drive {@code BatchOptimizer.runBatchLoop} (Plan 7 Task 14). */
  static boolean isOptimizerMode(String mode) {
    return mode.startsWith("optimizer");
  }

  // -----------------------------------------------------------------------------------------
  // Plan 7 Task 16 — the two batch modes, i.e. the jar's real `-de <dsn> -do <ses>` flow
  // -----------------------------------------------------------------------------------------

  /**
   * Modes {@code batch} and {@code batch-router}: the whole-board run <b>as the CLI runs it</b>.
   *
   * <p>Every other mode of this driver builds its board with {@code DsnReader.readBoard} and its
   * settings with {@code DefaultSettings} alone, which is the priority-0 ladder and exactly what
   * the per-connection drivers need. That is <b>not</b> what {@code java -jar … -de x.dsn -do
   * x.ses} does, and controller ruling AW makes the difference load-bearing for Task 16: the real
   * flow loads through {@link HeadlessBoardManager#loadFromSpecctraDsn}, whose {@code
   * applyRouterSettingsForLoadedBoard} ({@code HeadlessBoardManager.java:739-748}) runs
   * {@code applyCopperToEdgeClearanceOverride} and {@code applyHoleClearanceOverride} — which
   * <b>mutate the board</b> on 15 of the 16 corpus boards (Task 15b) — and resolves settings
   * through the two-merge ladder of {@code Freerouting.java:125-146} and
   * {@code RoutingJobScheduler.java:103-186}. A reference generated without those is not the
   * jar's output.
   *
   * <p>So these two modes are the only ones that go through the manager, and their settings come
   * from the <b>real</b> {@code SettingsMerger} with a <b>real</b> {@code CliSettings} built from
   * the same {@code argv} the bare jar is given ({@code -de}, {@code -do}, {@code -mp} and the
   * two {@code --router.*.enabled} switches). The port's twin calls
   * {@code fr_settings::resolve_headless} with the same argv and {@code
   * fr_router::pipeline::prepare_board}; the settings ladder itself is already byte-pinned by
   * Plan 5's {@code p4t1}, so what these modes add is the board mutation and the whole pipeline
   * on top of it.
   *
   * <ul>
   *   <li>{@code batch} runs the real {@code RoutingPipeline.createForHeadless(job).run()} — both
   *       stages — and, with {@code --ses}, writes the result through the real
   *       {@code SesWriter.write}, using {@code job.name} as the design name exactly as
   *       {@code RoutingJobSchedulerActionThread.setJobOutput:288} does. That file is
   *       {@code tests/reference/<stem>/batch.ses}.
   *   <li>{@code batch-router} runs the routing stage <b>transcribed</b>, on the same board and
   *       the same settings with the optimizer switched off, so that each completed pass prints
   *       its {@code PassRecord} tuple. With {@code --passes} those tuples are written as JSON
   *       lines — {@code tests/reference/<stem>/batch.passes.jsonl}, ruling 1(a)'s rung.
   * </ul>
   *
   * <h2>The budget, measured rather than assumed</h2>
   *
   * <p>The plan text asks this driver to reflect {@code TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP} to
   * {@code 0}. <b>It cannot be done.</b> All four declarations are {@code static final int = 1000}
   * with a constant initialiser, so {@code javac} inlines them: {@code javap -c} on the shipping
   * jar shows {@code sipush 1000} immediately before every {@code optChangedArea} call site, and
   * the fields are never read. Writing them reflectively changes nothing. The Java side therefore
   * runs with the live 1000 ms limit and the port with {@code RouterBudget::disabled()}; a
   * byte-identical SES is then <i>evidence</i> that the limit never changed the result, which is
   * strictly stronger than configuring both sides the same.
   *
   * <p>The trips that <i>did</i> happen are counted by the jar's own logging, not by this driver:
   * {@code TraceTightener.isStopRequested:202-211} calls
   * {@code FRLogger.debug("TraceTightener.is_stop_requested: time limit exceeded")} on every
   * exceeded check, and {@code Log4j2ConfigurationFactory} builds a root logger at
   * {@code Level.ALL} with a file appender whose level is
   * {@code -Dfreerouting.logging.file.level} (default {@code DEBUG}) and whose path is
   * {@code -Dfreerouting.logging.file.location}. So those properties plus {@code grep -c} are the
   * whole measurement <b>for this driver</b>, and
   * {@code scripts/gen-batch-reference.sh --verify-driver} is what runs it. A <b>bare-jar</b> run
   * needs the program arguments {@code --logging.file.location=…} instead, because
   * {@code Freerouting.main} ({@code Freerouting.java:1088-1098}) overwrites all five of those
   * system properties from its own argument/environment parse before logging initialises.
   * <b>A programmatic Log4j2 appender was tried first and did not work</b>: a counting
   * {@code AbstractAppender} added through {@code Configuration.addLogger} +
   * {@code updateLoggers} received <b>zero</b> events against this jar's configuration factory,
   * which is why the driver has no flag of its own.
   */
  static void runBatchMode(
      PrintStream out,
      Path dsn,
      int maxPasses,
      String mode,
      boolean fanout,
      boolean optimizer,
      Path sesPath,
      Path passesPath)
      throws Exception {
    boolean routerOnly = "batch-router".equals(mode);
    RoutingJob job = new RoutingJob();
    // `Freerouting.java:100` — `setInput(File)` reads the bytes, detects the format, sets
    // `job.input.filename` to the absolute path and `job.name` to the base name without the
    // extension. `job.name` is what the SES `(session …)`/`(base_design …)` header carries.
    job.setInput(dsn.toFile());
    byte[] dsnBytes = job.input.getData().readAllBytes();
    String dsnFilename = job.input.getFilename();

    String[] argv = batchArgv(dsn, maxPasses, fanout, optimizer && !routerOnly);
    out.println("ARGV " + String.join(" ", argv));

    // --- merge #1 (`Freerouting.java:125-146`), the P4T1 transcription ------------------------
    SettingsMerger prototype =
        new SettingsMerger(
            new DefaultSettings(), //                                          :1410
            new JsonFileSettings(), //                                         :1411
            new CliSettings(argv), //                                          :1412
            new EnvironmentVariablesSource()); //                              :1413
    SettingsMerger merger1 = prototype.clone(); //                             :125
    merger1.addOrReplaceSources( //                                            :126-127
        new DsnFileSettings(new ByteArrayInputStream(dsnBytes), dsnFilename));
    job.routerSettings = merger1.merge(); //                                   :146

    // --- the load, through the manager (`RoutingJobScheduler.java:93-101`) --------------------
    // This is the call that runs `applyRouterSettingsForLoadedBoard` (`:741-747`, i.e. the
    // between-merges board pass **and** the two clearance overrides) and
    // `applyImmediatePostLoadProcessing` (`:755-756`, `reduceNetsOfRouteItems` and the read-only
    // `validatePowerPlanes`). Ruling AW: without it the board is not the one the jar routes.
    HeadlessBoardManager manager = new HeadlessBoardManager(job);
    manager.loadFromSpecctraDsn(
        new ByteArrayInputStream(dsnBytes), null, new ItemIdGenerator()); //   :95-96
    RoutingBoard board = manager.getRoutingBoard(); //                         :101
    if (board == null) {
      throw new IllegalStateException("board did not load: " + dsn);
    }
    job.board = board;

    // --- merge #2 (`RoutingJobScheduler.java:103-170`) ----------------------------------------
    SettingsMerger merger2 = prototype.clone(); //                             :103
    merger2.addOrReplaceSources( //                                            :106-107
        new DsnFileSettings(new ByteArrayInputStream(dsnBytes), dsnFilename));
    // `:113-152` resolves `job.rules ?? -dr ?? adjacent <design>.rules`; no corpus stem has one,
    // and the generator passes none, so `rulesData` is null and `:154-160`/`:173-184` are skipped
    // — the same two steps `resolve_headless` skips for a `None` `scheduler_rules`.
    merger2.addOrReplaceSources(new ApiSettings(job.routerSettings)); //       :163-166
    job.routerSettings = merger2.merge(); //                                   :170
    job.routerSettings.applyBoardSpecificOptimizations(board); //              :186

    job.thread = new P7T2.NeverStarted();
    RouterSettings settings = job.routerSettings;
    out.println(
        "SETTINGS maxPasses="
            + settings.maxPasses
            + " maxItems="
            + settings.maxItems
            + " runRouter="
            + settings.getRunRouter()
            + " runFanout="
            + settings.isFanoutEnabled()
            + " runOptimizer="
            + settings.getRunOptimizer()
            + " optMaxPasses="
            + settings.optimizer.maxPasses
            + " optMaxItems="
            + settings.optimizer.maxItems
            + " fanoutMsPerPin="
            + settings.fanout.maxMillisecondsPerPin
            + " tracePullTightAccuracy="
            + settings.tracePullTightAccuracy);
    out.println(
        "BOARD-PREPARED classes="
            + board.rules.clearanceMatrix.getClassCount()
            + " outlineClass="
            + board.getOutline().clearanceClassIndex()
            + " holeClearance="
            + board.rules.getHoleClearance());

    if (routerOnly) {
      BatchAutorouter router = new BatchAutorouter(job);
      out.println("[transcript]");
      List<String> tuples = new ArrayList<>();
      passTuples = tuples;
      boolean returned;
      try {
        returned = transcribeRun(out, router, settings);
      } finally {
        passTuples = null;
      }
      RoutingBoard finalBoard = router.board;
      out.println("TRANSCRIPT returned=" + returned + " " + P7T2.boardShape(finalBoard));
      out.println("[board]");
      dumpBoard(out, finalBoard);
      if (passesPath != null) {
        writePasses(passesPath, tuples);
      }
      if (sesPath != null) {
        writeSes(sesPath, finalBoard, job.name);
      }
      return;
    }

    RoutingPipeline pipeline = RoutingPipeline.createForHeadless(job);
    StringBuilder events = new StringBuilder();
    int[] routerPassesRun = {0};
    pipeline.addTaskStateChangedEventListener(
        event -> {
          NamedAlgorithm source = (NamedAlgorithm) event.getSource();
          events
              .append("EVENT algorithm=")
              .append(source.getType())
              .append(" state=")
              .append(event.getTaskState())
              .append('\n');
          if (source.getType() == NamedAlgorithmType.ROUTER) {
            routerPassesRun[0] = event.getPassNumber();
          }
        });

    out.println("[pipeline]");
    pipeline.run();
    out.print(events);
    out.println(
        "RESULT passesRun="
            + routerPassesRun[0]
            + " optimizerPresent="
            + (pipeline.getOptimizer() != null)
            + " fanoutTimedOut="
            + pipeline.getAutorouter().isFanoutTimedOut()
            + " optimizerTimedOut="
            + (pipeline.getOptimizer() != null && pipeline.getOptimizer().isTimedOut())
            + " "
            + P7T2.boardShape(job.board));
    out.println("[board]");
    dumpBoard(out, job.board);
    if (sesPath != null) {
      writeSes(sesPath, job.board, job.name);
    }
  }

  /** {@code batch} and {@code batch-router} — the two modes that go through the manager. */
  static boolean isBatchMode(String mode) {
    return mode.startsWith("batch");
  }

  /**
   * The {@code argv} the bare jar is given for the same run, which is what makes
   * {@code gen-batch-reference.sh --verify-driver} an apples-to-apples comparison: the same four
   * switches reach the same {@code CliSettings} (priority 60) on both sides.
   */
  static String[] batchArgv(Path dsn, int maxPasses, boolean fanout, boolean optimizer) {
    return new String[] {
      "-de",
      dsn.toString(),
      "-do",
      dsn.toString().replaceAll("\\.dsn$", ".ses"),
      "-mp",
      Integer.toString(maxPasses),
      "--router.fanout.enabled=" + fanout,
      "--router.optimizer.enabled=" + optimizer,
    };
  }

  /** {@code on}/{@code off}, the two words the fixture table's columns 6 and 7 use. */
  static boolean onOff(String arg) {
    if ("on".equals(arg)) {
      return true;
    }
    if ("off".equals(arg)) {
      return false;
    }
    throw new IllegalArgumentException("expected 'on' or 'off', not: " + arg);
  }

  /** The inverse, for the {@code HEADER} line. */
  static String onOffName(boolean value) {
    return value ? "on" : "off";
  }

  /**
   * Set by {@link #runBatchMode} around {@link #transcribeRun} so each completed pass's tuple is
   * collected as well as printed. {@code null} — the value every other mode leaves it at — makes
   * the collection a no-op, so no existing transcript changes by one byte.
   */
  static List<String> passTuples;

  /**
   * {@code batch.passes.jsonl} — one JSON object per completed pass, the six fields of the port's
   * {@code PassRecord} in {@link #passRecord}'s order. Rendered by hand rather than through Gson:
   * every value is an {@code int} or a {@code Float.toString}, so there is nothing to escape and
   * nothing a serialiser could reorder.
   */
  static void writePasses(Path path, List<String> tuples) throws Exception {
    Files.createDirectories(path.getParent());
    StringBuilder sb = new StringBuilder();
    for (String tuple : tuples) {
      sb.append('{');
      String[] fields = tuple.split(" ");
      for (int i = 0; i < fields.length; i++) {
        String[] kv = fields[i].split("=", 2);
        if (i > 0) {
          sb.append(',');
        }
        sb.append('"').append(kv[0]).append("\":").append(kv[1]);
      }
      sb.append("}\n");
    }
    Files.writeString(path, sb.toString(), StandardCharsets.UTF_8);
  }

  /**
   * {@code batch.ses} — the real {@code SesWriter.write}, on the real final board, with
   * {@code job.name} as the design name. That is
   * {@code RoutingJobSchedulerActionThread.setJobOutput:283-290} minus the
   * {@code ByteArrayOutputStream} it buffers through; {@code HeadlessBoardManager
   * .saveAsSpecctraSessionSes:862-865} is a two-line delegate to the same call whose only extra
   * work is recomputing {@code originalBoardChecksum}, which nothing here reads.
   */
  static void writeSes(Path path, RoutingBoard board, String designName) throws Exception {
    Files.createDirectories(path.getParent());
    try (OutputStream sink = Files.newOutputStream(path)) {
      SesWriter.write(board, sink, designName);
    }
  }

  /** {@code all} is Java's {@code null}, i.e. "no limit"; anything else is an {@code Integer}. */
  static Integer boxedLimit(String arg) {
    return "all".equals(arg) ? null : Integer.valueOf(Integer.parseInt(arg));
  }

  /** The inverse, for the {@code HEADER} line. */
  static String limitName(Integer limit) {
    return limit == null ? "all" : limit.toString();
  }

  // -----------------------------------------------------------------------------------------
  // run() — transcribed, AutorouteBatchLoop.java:37-588
  // -----------------------------------------------------------------------------------------

  /**
   * {@code AutorouteBatchLoop.run}'s body, transcribed line for line, with one line printed per
   * decision the loop takes and one {@code PASS} tuple per completed pass.
   *
   * <p>Everything it calls is the real thing; only the control flow is here. The omissions are the
   * ones the port also omits and for the same reasons: the {@code PerformanceProfiler} block
   * ({@code :68-81}) and its two closing calls ({@code :568-569}), every {@code job.log*} payload,
   * the two {@code FRLogger.traceEntry}/{@code traceExit} wrappers ({@code :286-291},
   * {@code :345-351}), the per-net incomplete breakdown trace ({@code :378-406}), and the three
   * {@code fireTaskStateChangedEvent} calls, which fire into a listener list this driver leaves
   * empty exactly as the headless CLI does — the port's {@code ProgressSink} equivalents are
   * pinned by unit tests, not here.
   *
   * <p>The fanout pre-pass ({@code :89-218}) is transcribed from Plan 7 Task 12 on: in
   * {@code mode router-only} {@code :89}'s guard is false and the block is not on the run's path
   * in Java either, and in {@code mode router+fanout} it calls the real
   * {@code BatchFanout.fanoutBoard} and prints its summary. Everything else in the block is
   * {@code AutorouteRuntimeMetrics} and {@code job.log*}, which neither side carries.
   */
  static boolean transcribeRun(PrintStream out, BatchAutorouter router, RouterSettings settings) {
    RoutingBoard board = router.board;
    RoutingJob job = router.job;

    // :44-50.
    boolean anyRoutable = false;
    for (int i = 0; i < settings.getLayerCount(); i++) {
      if (settings.getLayerActive(i) && board.layerStructure.layers[i].isSignal) {
        anyRoutable = true;
        break;
      }
    }
    out.println("INIT anyRoutable=" + anyRoutable);
    if (!anyRoutable) {
      // :51-56 — TaskState.CANCELLED and then the throw. The port answers
      // `Err(RouterError::NoRoutableLayer)`; neither side prints past this line.
      out.println("THROW IllegalArgumentException state=CANCELLED");
      return false;
    }

    // :58-59 — TaskState.STARTED.
    out.println("STATE STARTED");

    // :62-63.
    router.sessionStartTime = java.time.Instant.now();
    router.initialUnroutedCount = router.calculateIncompleteCount(board);
    out.println("INITIAL-UNROUTED " + router.initialUnroutedCount);

    // :65.
    BoardHistory bh = new BoardHistory(job.routerSettings.scoring);

    // :89-218 — the fanout pre-pass; see the javadoc.
    out.println(
        "FANOUT enabled="
            + settings.isFanoutEnabled()
            + " smdPins="
            + router.board.getSmdPins().size());
    if (settings.isFanoutEnabled() && !router.board.getSmdPins().isEmpty()) {
      // :123-172.
      BatchFanout.FanoutRunSummary fanoutSummary =
          BatchFanout.fanoutBoard(router.board, router.settings, router.thread);
      // :173.
      router.fanoutTimedOut = fanoutSummary.isTimedOut();
      BatchFanout.EscapeStatistics finalEscape = fanoutSummary.escapeStatistics();
      out.println(
          "FANOUT-SUMMARY completedPassCount="
              + fanoutSummary.completedPassCount()
              + " isTimedOut="
              + fanoutSummary.isTimedOut()
              + " escapeTotalSmdPins="
              + finalEscape.totalSmdPins()
              + " escapedCount="
              + finalEscape.escapedCount()
              + " escapedPercentage="
              + Double.toString(finalEscape.escapedPercentage())
              + " fanoutTimedOut="
              + router.fanoutTimedOut
              + " "
              + P7T2.boardShape(router.board));
      board = router.board;
    }

    // :220.
    int currentUnrouted = router.calculateIncompleteCount(board);
    // :221-223.
    boolean isRouterEnabled =
        settings.getRunRouter() && (settings.maxPasses == null || settings.maxPasses >= 0);
    out.println("ROUTER-ENABLED " + isRouterEnabled + " unrouted=" + currentUnrouted);
    // :234.
    boolean continueAutorouting = isRouterEnabled;

    // :236-242.
    int currentPass = 1;
    int consecutiveNoImprovementPasses = 0;
    boolean fanoutRecoveryApplied = false;
    float lastBestScore = Float.NEGATIVE_INFINITY;
    float globalBestScore = Float.NEGATIVE_INFINITY;
    int passOfBestScore = 0;
    int incompleteCountAtBestScore = 0;
    // :249 — `alreadyRoutedBoardHashes`, whose two readers (`:259`, `:266`) are commented out.
    // Not transcribed and not ported (quirk #216).

    // :250.
    while (continueAutorouting && !router.thread.isStopAutoRouterRequested()) {
      // :251-253 — dead in production (quirk #203): the monitor thread already raised `ALL`.
      if (job != null && job.state == app.freerouting.core.RoutingJobState.TIMED_OUT) {
        router.thread.requestStopAutoRouter();
      }

      // :255 — `currentBoardHash`, a log payload only once `:257-266` is commented out.

      // :268-273.
      if (settings.maxPasses != null && settings.maxPasses > 0 && currentPass > settings.maxPasses) {
        out.println("MAXPASSES-BREAK pass=" + currentPass + " maxPasses=" + settings.maxPasses);
        router.thread.requestStopAutoRouter();
        break;
      }

      // :275-280 — `job.setCurrentPass` and TaskState.RUNNING.
      job.setCurrentPass(currentPass);
      out.println("STATE RUNNING pass=" + currentPass);

      // :282-283.
      float boardScoreBefore =
          new BoardStatistics(router.board).getNormalizedScore(job.routerSettings.scoring);
      // :284.
      bh.add(router.board);
      out.println(
          "HIST-ADD pass="
              + currentPass
              + " scoreBefore="
              + Float.toString(boardScoreBefore)
              + " size="
              + bh.size()
              + " maxScore="
              + Float.toString(bh.getMaxScore()));

      // :293.
      continueAutorouting = router.autoroutePass(currentPass);

      // :295-296.
      BoardStatistics boardStatisticsAfter = new BoardStatistics(router.board);
      float boardScoreAfter = boardStatisticsAfter.getNormalizedScore(job.routerSettings.scoring);
      out.println(
          "PASS-RET pass="
              + currentPass
              + " continueAutorouting="
              + continueAutorouting
              + " scoreAfter="
              + Float.toString(boardScoreAfter));

      // :298-344 — the best-board restore.
      boolean sizeGate =
          (bh.size() >= STOP_AT_PASS_MINIMUM) || router.thread.isStopAutoRouterRequested();
      boolean modGate =
          ((currentPass % STOP_AT_PASS_MODULO == 0) && (currentPass >= STOP_AT_PASS_MINIMUM))
              || router.thread.isStopAutoRouterRequested();
      out.println(
          "RESTORE-GATE pass="
              + currentPass
              + " sizeGate="
              + sizeGate
              + " modGate="
              + modGate
              + " maxScore="
              + Float.toString(bh.getMaxScore())
              + " fires="
              + (sizeGate && modGate && bh.getMaxScore() > boardScoreAfter));
      if (sizeGate) {
        if (modGate) {
          // :306 — a **strict** `>`.
          if (bh.getMaxScore() > boardScoreAfter) {
            // :307.
            var boardToRestore = bh.restoreBoard(MAXIMUM_TRIES_ON_THE_SAME_BOARD);
            if (boardToRestore == null) {
              // :308-313.
              out.println("RESTORE-NULL-BREAK pass=" + currentPass);
              router.thread.requestStopAutoRouter();
              break;
            }

            // :315.
            int boardToRestoreRank = bh.getRank(boardToRestore);
            out.println("RESTORE-RANK pass=" + currentPass + " rank=" + boardToRestoreRank);

            // :317-320.
            if (boardToRestoreRank > BOARD_RANK_LIMIT) {
              out.println("RANK-BREAK pass=" + currentPass + " rank=" + boardToRestoreRank);
              router.thread.requestStopAutoRouter();
              break;
            }

            // :322-334 — a fall-through, not an `else`.
            router.board = boardToRestore;
            board = router.board;
            var boardStatistics = router.board.getStatistics();
            consecutiveNoImprovementPasses = 0;
            boardStatisticsAfter = boardStatistics;
            boardScoreAfter = boardStatisticsAfter.getNormalizedScore(job.routerSettings.scoring);
            lastBestScore = boardScoreAfter;
            out.println(
                "RESTORED pass=" + currentPass + " scoreAfter=" + Float.toString(boardScoreAfter));
          }
        }
      }

      // :353-376 — the pass-completed report, and the port's `PassRecord` for this pass.
      String tuple = passRecord(currentPass, boardScoreAfter, boardStatisticsAfter);
      out.println("PASS " + tuple);
      if (passTuples != null) {
        passTuples.add(tuple);
      }

      // :408-410.
      if (Boolean.TRUE.equals(settings.saveIntermediateStages)) {
        out.println("SNAPSHOT pass=" + currentPass);
      }

      // :422 — the stagnation detector's guard.
      if (currentPass >= STOP_AT_PASS_MINIMUM && continueAutorouting) {

        // :425-427.
        if (boardScoreAfter > lastBestScore + STAGNATION_SCORE_THRESHOLD) {
          consecutiveNoImprovementPasses = 0;
          lastBestScore = boardScoreAfter;
        } else {
          // :429.
          consecutiveNoImprovementPasses++;

          // :435-454 — the one-shot fanout recovery. Never fires in `router-only`.
          if (settings.isFanoutEnabled()
              && !fanoutRecoveryApplied
              && boardStatisticsAfter.connections.incompleteCount > 0
              && consecutiveNoImprovementPasses >= FANOUT_RECOVERY_STAGNATION_PASSES) {
            out.println("FANOUT-RECOVERY pass=" + currentPass);
            router.removeTails(Item.StopConnectionOption.NONE);
            boardStatisticsAfter = new BoardStatistics(router.board);
            boardScoreAfter = boardStatisticsAfter.getNormalizedScore(job.routerSettings.scoring);
            lastBestScore = boardScoreAfter;
            consecutiveNoImprovementPasses = 0;
            fanoutRecoveryApplied = true;
          }

          // :456-476.
          if (consecutiveNoImprovementPasses >= STAGNATION_PASS_LIMIT) {
            out.println(
                "STAGNATION-LOCAL-BREAK pass="
                    + currentPass
                    + " counter="
                    + consecutiveNoImprovementPasses);
            router.buildUnroutedConnectionsReport();
            router.thread.requestStopAutoRouter();
            break;
          }
        }

        // :482-485.
        if (boardScoreAfter > globalBestScore + STAGNATION_SCORE_THRESHOLD) {
          globalBestScore = boardScoreAfter;
          passOfBestScore = currentPass;
          incompleteCountAtBestScore = boardStatisticsAfter.connections.incompleteCount;
        } else if ((currentPass - passOfBestScore) >= STAGNATION_PASS_LIMIT) {
          // :486-507.
          out.println(
              "STAGNATION-GLOBAL-BREAK pass="
                  + currentPass
                  + " passOfBestScore="
                  + passOfBestScore
                  + " incompleteAtBest="
                  + incompleteCountAtBestScore);
          router.buildUnroutedConnectionsReport();
          router.thread.requestStopAutoRouter();
          break;
        }

      } else if (boardStatisticsAfter.connections.incompleteCount == 0
          && boardScoreAfter > STAGNATION_SCORE_THRESHOLD) {
        // :509-517 — attached to the `currentPass >= 8` guard, which is the bug.
        out.println("ROUTED-RESET pass=" + currentPass);
        consecutiveNoImprovementPasses = 0;
        lastBestScore = boardScoreAfter;
      }

      out.println(
          "STAGNATION pass="
              + currentPass
              + " counter="
              + consecutiveNoImprovementPasses
              + " lastBest="
              + Float.toString(lastBestScore)
              + " globalBest="
              + Float.toString(globalBestScore)
              + " passOfBest="
              + passOfBestScore);

      // :520-522.
      if (continueAutorouting && !router.thread.isStopAutoRouterRequested()) {
        currentPass++;
      }
    }

    // :528-530.
    float currentFinalScore =
        new BoardStatistics(router.board).getNormalizedScore(job.routerSettings.scoring);
    float bestHistoryScore = bh.getMaxScore();
    boolean swapped = false;
    // :531-550.
    if (bestHistoryScore > currentFinalScore) {
      RoutingBoard bestBoard = bh.restoreBestBoard();
      if (bestBoard != null) {
        router.board = bestBoard;
        swapped = true;
      }
    }
    out.println(
        "FINAL-SWAP currentFinalScore="
            + Float.toString(currentFinalScore)
            + " bestHistoryScore="
            + Float.toString(bestHistoryScore)
            + " swapped="
            + swapped);

    // :552 — `job.board = router.board`.
    job.board = router.board;

    // :554-563.
    boolean wasRouterRun =
        settings.getRunRouter() && (settings.maxPasses == null || settings.maxPasses >= 0);
    boolean tailsFire =
        wasRouterRun
            && !(router.removeUnconnectedVias
                || continueAutorouting
                || router.thread.isStopAutoRouterRequested());
    out.println(
        "TAILS wasRouterRun="
            + wasRouterRun
            + " removeUnconnectedVias="
            + router.removeUnconnectedVias
            + " continueAutorouting="
            + continueAutorouting
            + " stopAutoRouterRequested="
            + router.thread.isStopAutoRouterRequested()
            + " fires="
            + tailsFire);
    if (tailsFire) {
      router.removeTails(Item.StopConnectionOption.NONE);
    }

    // :565.
    bh.clear();

    // :571-585.
    String state;
    if (!router.thread.isStopAutoRouterRequested()) {
      state = "FINISHED";
    } else {
      boolean isTimedOut =
          (job != null) && (job.state == app.freerouting.core.RoutingJobState.TIMED_OUT);
      state = isTimedOut ? "TIMED_OUT" : "CANCELLED";
    }
    out.println(
        "RESULT state="
            + state
            + " passesRun="
            + currentPass
            + " continueRouting="
            + !router.thread.isStopAutoRouterRequested()
            + " stopRequested="
            + router.thread.isStopRequested()
            + " stopAutoRouterRequested="
            + router.thread.isStopAutoRouterRequested());

    // :587.
    return !router.thread.isStopAutoRouterRequested();
  }

  // -----------------------------------------------------------------------------------------
  // Mode `full` — RoutingPipeline.run(), Plan 7 Task 15
  // -----------------------------------------------------------------------------------------

  /**
   * Mode {@code full}: the actual {@code RoutingPipeline.createForHeadless(job).run()}
   * ({@code RoutingPipeline.java:81-129}) — both stages, the fanout-only settings-clone contract
   * ({@code :99-108}, unreachable from this driver's settings, which always sets
   * {@code runRouter = true}), the one {@code finishAutoroute()} call ({@code :110}) and the
   * optimizer-stage skip ({@code :117-119}) — driven through the jar's own class, not a
   * transcription. Task 15's port collapses `RoutingPipeline` into one function
   * ({@code run_pipeline}), so there is nothing here for a hand-written transcript to add over
   * the real method; the two-half `[transcript]`/`[real]` shape the other modes use is Tasks
   * 10/14's own instrument for methods hundreds of lines long; `RoutingPipeline.run` is 5 lines
   * plus two 15-30 line private methods, and it delegates to {@code AutorouteBatchLoop.run} and
   * {@code BatchOptimizer.runBatchLoop} — both already pinned end to end by the other four modes.
   *
   * <p>{@code job.getCurrentPass()} ({@code core/RoutingJob.java:250-251}) is
   * {@code AutorouteBatchLoop.java:276}'s {@code job.setCurrentPass(currentPass)}, i.e. exactly
   * the number {@code PipelineResult::passes_run} carries — the one number from inside the pass
   * loop this driver can read back without a listener, because it is the only piece of per-pass
   * state Java routes through the job object rather than only through an event.
   *
   * <p>{@code [events]} is every {@code TaskStateChangedEvent} both stages fire, in order, tagged
   * by which {@code NamedAlgorithm} fired it (the source object's own {@code getType()}) — the
   * pipeline-level counterpart of the {@code ProgressSink} recorder test
   * {@code crates/fr-router/tests/pipeline.rs}'s
   * {@code a_recording_sink_sees_the_stage_events_in_javas_order} pins on the port side.
   */
  static void runFullMode(
      PrintStream out, Path dsn, int maxPasses, Integer optPasses, Integer optItems)
      throws Exception {
    RoutingBoard board = P7T2.loadBoard(dsn);
    RouterSettings settings = buildFullModeSettings(board, maxPasses, optPasses, optItems);

    RoutingJob job = new RoutingJob();
    job.board = board;
    job.routerSettings = settings;
    job.thread = new P7T2.NeverStarted();

    RoutingPipeline pipeline = RoutingPipeline.createForHeadless(job);
    StringBuilder events = new StringBuilder();
    // `AutorouteBatchLoop.java:270-274`'s cap check runs *before* `:276`'s `job.setCurrentPass`,
    // so on a `maxPasses`-capped exit the job's own `getCurrentPass()` is one **less** than the
    // loop's local `currentPass` at the point it fires its final event (`:572-584`) — the local
    // was already incremented past the cap by `:521` on the completed prior iteration, and the
    // aborted final iteration breaks before `job.setCurrentPass` runs again. `PipelineResult::
    // passes_run` is the port's transcription of that local, not of `job.getCurrentPass()`
    // (`crates/fr-router/src/pipeline/run.rs`'s doc corrects the plan text this drove); the
    // router's own last `TaskStateChangedEvent.getPassNumber()` carries the same local, so that
    // is what this driver reads back instead.
    int[] routerPassesRun = {0};
    TaskStateChangedEventListener listener =
        event -> {
          NamedAlgorithm source = (NamedAlgorithm) event.getSource();
          events
              .append("EVENT algorithm=")
              .append(source.getType())
              .append(" state=")
              .append(event.getTaskState())
              .append('\n');
          if (source.getType() == NamedAlgorithmType.ROUTER) {
            routerPassesRun[0] = event.getPassNumber();
          }
        };
    pipeline.addTaskStateChangedEventListener(listener);

    out.println("[pipeline]");
    pipeline.run();

    out.print(events);
    out.println(
        "RESULT passesRun="
            + routerPassesRun[0]
            + " optimizerPresent="
            + (pipeline.getOptimizer() != null)
            + " fanoutTimedOut="
            + pipeline.getAutorouter().isFanoutTimedOut()
            + " optimizerTimedOut="
            + (pipeline.getOptimizer() != null && pipeline.getOptimizer().isTimedOut())
            + " "
            + P7T2.boardShape(job.board));

    out.println("[board]");
    dumpBoard(out, job.board);
  }

  /**
   * {@code buildSettings} plus the optimizer knobs {@code applyOptimizerLimits} sets, both fanout
   * and the router always on — the shape {@code RoutingPipeline.java:36} needs to build both
   * stages ({@code job.routerSettings.getRunOptimizer()}).
   */
  static RouterSettings buildFullModeSettings(
      RoutingBoard board, int maxPasses, Integer optPasses, Integer optItems) {
    RouterSettings settings = buildSettings(board, maxPasses, "router-only");
    settings.setRunOptimizer(true);
    applyOptimizerLimits(settings, optPasses, optItems);
    return settings;
  }

  // -----------------------------------------------------------------------------------------
  // The optimizer stage — BatchOptimizer.java:125-272, :279-385 (Plan 7 Task 14)
  // -----------------------------------------------------------------------------------------

  /**
   * Modes {@code optimizer}, {@code optimizer+fanout} and {@code optimizer-shared}: the whole
   * {@code -dr}-equivalent run, router stage <b>and</b> optimizer stage, in this driver's two
   * halves.
   *
   * <p>The routing prologue is the <i>real</i> {@code BatchAutorouter.runBatchLoop()} — the call
   * the {@code router-only} mode already pins byte for byte on both sides — so a prologue
   * divergence shows up on the {@code ROUTED} line rather than inside the optimizer.
   * {@code router.board} is read back after it, because the best-board policy may have replaced it
   * (quirk #209).
   *
   * <h2>The stop flag, and why there are two optimizer modes</h2>
   *
   * <p>{@code RoutingPipeline.run} ({@code :81-85}) hands both stages the one {@code job.thread},
   * and <b>nothing in the tree ever lowers the flag</b>. Every ordinary exit from
   * {@code AutorouteBatchLoop.run} raises {@code AUTO_ROUTER_ONLY} (quirk #214), the optimizer
   * stage is gated on {@code isStopRequested()} — {@code ALL} — and so it runs; but
   * {@code BatchAutorouter.autoroutePassesForOptimizingItem}'s loop head ({@code :268}) is
   * {@code !isStopAutoRouterRequested()}, so every {@code optRouteItem} inside it routes
   * <b>zero</b> passes, measures a worse board and restores its snapshot. That is the production
   * shape, and mode {@code optimizer-shared} is it: the optimizer gets the prologue's own thread.
   *
   * <p>Modes {@code optimizer} and {@code optimizer+fanout} hand the stage a <b>fresh</b>
   * {@code NeverStarted} on both sides, which is what makes the rest of {@code runBatchLoop}
   * reachable at all. The {@code SEAM} line prints the flag the prologue left, so the difference
   * between the two is measured rather than asserted.
   *
   * <h2>The progress throttle</h2>
   *
   * <p>{@code optRoutePass:335-338} computes {@code board.getStatistics()} <i>inside</i> a
   * 1000 ms {@code ProgressThrottler} gate, so how often it runs depends on the wall clock — which
   * ruling AI forbids a parity run from depending on. The transcription below therefore computes
   * it <b>every</b> improved item, deterministically, and the port runs with
   * {@code RouterBudget::disabled()}, whose {@code progress_throttle_ms = 0} does the same. The
   * {@code [real]} half runs the jar's live gate, so {@code equalsTranscript} <b>measures</b> that
   * the extra {@code BoardStatistics} constructions do not move the board — on the Java side
   * alone, which is the only side that can settle it.
   */
  static void runOptimizerMode(
      PrintStream out, Path dsn, int maxPasses, String mode, Integer optPasses, Integer optItems)
      throws Exception {
    boolean shared = "optimizer-shared".equals(mode);

    // ---- half one: the transcription -----------------------------------------------------
    RoutingBoard loaded = P7T2.loadBoard(dsn);
    RouterSettings settings = buildSettings(loaded, maxPasses, mode);
    applyOptimizerLimits(settings, optPasses, optItems);
    BatchAutorouter router = P7T2.newRouter(loaded, settings);
    out.println("[route]");
    boolean routerReturned = router.runBatchLoop();
    // `:552` — the loop writes its answer into `router.board` (quirk #209).
    RoutingBoard board = router.board;
    out.println("ROUTED returned=" + routerReturned + " " + P7T2.boardShape(board));
    out.println(
        "SEAM shared="
            + shared
            + " stopRequested="
            + router.thread.isStopRequested()
            + " stopAutoRouterRequested="
            + router.thread.isStopAutoRouterRequested());

    BatchOptimizer optimizer = newOptimizer(board, settings, shared ? router.thread : null);
    out.println("[transcript]");
    transcribeOptimizer(out, optimizer, board, settings);
    String transcriptHash = board.getHash();

    // ---- half two: the real method -------------------------------------------------------
    RoutingBoard realLoaded = P7T2.loadBoard(dsn);
    RouterSettings realSettings = buildSettings(realLoaded, maxPasses, mode);
    applyOptimizerLimits(realSettings, optPasses, optItems);
    BatchAutorouter realRouter = P7T2.newRouter(realLoaded, realSettings);
    out.println("[real]");
    boolean realRouterReturned = realRouter.runBatchLoop();
    RoutingBoard realBoard = realRouter.board;
    out.println("REAL-ROUTED returned=" + realRouterReturned + " " + P7T2.boardShape(realBoard));
    BatchOptimizer realOptimizer =
        newOptimizer(realBoard, realSettings, shared ? realRouter.thread : null);
    realOptimizer.runBatchLoop();
    out.println(
        "OPT-REAL items="
            + realOptimizer.totalItemsOptimized
            + " timedOut="
            + realOptimizer.isTimedOut()
            + " useIncreasedRipupCosts="
            + realOptimizer.useIncreasedRipupCosts
            + " "
            + P7T2.boardShape(realBoard)
            + " equalsTranscript="
            + realBoard.getHash().equals(transcriptHash));

    // ---- the final board, in `P6T15aProbe`'s polyline format ------------------------------
    out.println("[board]");
    dumpBoard(out, board);
  }

  /**
   * {@code BatchOptimizer.createForHeadless} ({@code :51-53}), which is what
   * {@code RoutingPipeline}'s constructor calls for a headless job ({@code RoutingPipeline.java:46}).
   * A {@code null} thread means "a fresh {@code NeverStarted}" — see the class comment's stop-flag
   * section.
   */
  static BatchOptimizer newOptimizer(
      RoutingBoard board, RouterSettings settings, app.freerouting.core.StoppableThread thread) {
    RoutingJob job = new RoutingJob();
    job.board = board;
    job.routerSettings = settings;
    job.thread = thread != null ? thread : new P7T2.NeverStarted();
    return BatchOptimizer.createForHeadless(job);
  }

  /** The two {@code Integer} limits the optimizer modes drive, {@code null} being "no limit". */
  static void applyOptimizerLimits(RouterSettings settings, Integer optPasses, Integer optItems) {
    settings.optimizer.maxPasses = optPasses;
    settings.optimizer.maxItems = optItems;
    // Ruling AI: `:153-160` builds the stage's deadline from this string, and no corpus run sets
    // it. Cleared explicitly so a settings source cannot make the run wall-clock dependent.
    settings.optimizer.timeoutString = null;
  }

  /**
   * {@code BatchOptimizer.runBatchLoop}'s body ({@code :125-272}), transcribed line for line, with
   * one line per decision the loop takes and one {@code OPT-PASS} tuple per completed pass.
   *
   * <p>Everything it calls is the real thing — {@code BoardStatistics}, {@code getNormalizedScore}
   * and, through {@link #transcribeOptRoutePass}, the real {@code optRouteItem}. The omissions are
   * the ones the port also omits: every {@code job.log*} payload ({@code :126-130}, {@code :140-145},
   * {@code :174}, {@code :184-191}, {@code :222-228}, {@code :256-271}), the {@code board.getHash()}
   * reads those payloads and the event objects carry ({@code :142}, {@code :163}, {@code :195},
   * {@code :198}, {@code :234}), the three JMX samplers ({@code :94-122}, used at {@code :149-151},
   * {@code :202}, {@code :238-248}) and the three {@code fireTaskStateChangedEvent} calls, whose
   * listener list is empty on the headless path.
   */
  static void transcribeOptimizer(
      PrintStream out, BatchOptimizer optimizer, RoutingBoard board, RouterSettings settings) {
    RoutingJob job = optimizer.job;
    // :132.
    optimizer.useIncreasedRipupCosts = true;
    // :135-138.
    BoardStatistics initialStats = board.getStatistics();
    out.println(
        "OPT-START score="
            + Float.toString(initialStats.getNormalizedScore(settings.scoring))
            + " incompletes="
            + initialStats.connections.incompleteCount
            + " violations="
            + initialStats.clearanceViolations.totalCount
            + " maxPasses="
            + limitName(settings.optimizer.maxPasses)
            + " maxItems="
            + limitName(settings.optimizer.maxItems)
            + " threshold="
            + Float.toString(settings.optimizer.optimizationImprovementThreshold)
            + " maxConsecutiveFailures="
            + settings.optimizer.maxConsecutiveFailures);
    // :153-160 — `timeoutString` is null on every corpus run (`applyOptimizerLimits` clears it),
    // so `deadlineMs` stays null and neither `:172` nor `:308` can fire. Printed so that a
    // settings change cannot make this run wall-clock dependent in silence.
    out.println("OPT-DEADLINE timeoutString=" + settings.optimizer.timeoutString);
    // :162-163.
    out.println("OPT-STATE STARTED");

    int currentPass = 0;
    // :167-171 — `ALL` only.
    while ((settings.optimizer.maxPasses == null || currentPass < settings.optimizer.maxPasses)
        && (settings.optimizer.maxItems == null
            || optimizer.totalItemsOptimized < settings.optimizer.maxItems)
        && !job.thread.isStopRequested()) {
      // :172-176 — the per-stage deadline; unreachable here, see `OPT-DEADLINE`.
      // :177.
      ++currentPass;
      // :179.
      float scoreBeforePass = board.getStatistics().getNormalizedScore(settings.scoring);
      // :182-193.
      if (scoreBeforePass * (1 + settings.optimizer.optimizationImprovementThreshold) >= 1000.0f) {
        out.println(
            "OPT-STOP reason=near-perfect pass="
                + currentPass
                + " score="
                + Float.toString(scoreBeforePass));
        break;
      }
      // :195-198.
      out.println("OPT-STATE RUNNING pass=" + currentPass);
      // :200.
      boolean withPreferredDirections = currentPass % 2 != 0;
      // :201 — the return value is discarded by Java; printed here because it is what `:365-368`
      // decided.
      float routeImproved =
          transcribeOptRoutePass(
              out, optimizer, board, settings, currentPass, withPreferredDirections);
      // :204-206.
      if (optimizer.isTimedOut()) {
        out.println("OPT-STOP reason=timeout pass=" + currentPass);
        break;
      }
      // :208.
      BoardStatistics statisticsAfter = board.getStatistics();
      float scoreAfterPass = statisticsAfter.getNormalizedScore(settings.scoring);
      // :209-210.
      double passImprovement =
          scoreBeforePass > 0 ? (double) (scoreAfterPass - scoreBeforePass) / scoreBeforePass : 0;
      double scoreImprovement;
      // :212-218.
      if (optimizer.useIncreasedRipupCosts && scoreAfterPass <= scoreBeforePass) {
        optimizer.useIncreasedRipupCosts = false;
        scoreImprovement = -1;
      } else {
        scoreImprovement = passImprovement;
      }
      out.println(
          "OPT-PASS pass="
              + currentPass
              + " withPreferredDirections="
              + withPreferredDirections
              + " scoreBefore="
              + Float.toString(scoreBeforePass)
              + " scoreAfter="
              + Float.toString(scoreAfterPass)
              + " passImprovement="
              + Double.toString(passImprovement)
              + " scoreImprovement="
              + Double.toString(scoreImprovement)
              + " useIncreasedRipupCosts="
              + optimizer.useIncreasedRipupCosts
              + " routeImproved="
              + Float.toString(routeImproved)
              + " items="
              + optimizer.totalItemsOptimized
              + " "
              + passRecord(currentPass, scoreAfterPass, statisticsAfter));
      // :220-230.
      if (scoreImprovement != -1
          && scoreImprovement < settings.optimizer.optimizationImprovementThreshold) {
        out.println(
            "OPT-STOP reason=threshold pass="
                + currentPass
                + " scoreImprovement="
                + Double.toString(scoreImprovement));
        break;
      }
    }

    // :233-234 — unconditional, whatever ended the loop.
    out.println("OPT-STATE FINISHED pass=" + currentPass);
    // :250-251.
    BoardStatistics finalStats = new BoardStatistics(board);
    // :252-255 — `completionStatus`, the only place Java tells the three endings apart.
    String state =
        optimizer.isTimedOut()
            ? "TIMED_OUT"
            : (job.thread.isStopRequested() ? "CANCELLED" : "FINISHED");
    out.println(
        "OPT-RESULT state="
            + state
            + " passesRun="
            + currentPass
            + " items="
            + optimizer.totalItemsOptimized
            + " timedOut="
            + optimizer.isTimedOut()
            + " useIncreasedRipupCosts="
            + optimizer.useIncreasedRipupCosts
            + " finalScore="
            + Float.toString(finalStats.getNormalizedScore(settings.scoring))
            + " "
            + P7T2.boardShape(board));
  }

  /**
   * {@code BatchOptimizer.optRoutePass}'s body ({@code :279-385}), transcribed, calling the
   * <i>real</i> {@code optRouteItem} ({@code :395-514}) — the method {@code p7t8} already pins at
   * 10/10 MATCH — once per item, with one {@code OPT-ITEM} line beside each call.
   *
   * <p>{@code :335-338}'s throttled {@code board.getStatistics()} is computed <b>unconditionally</b>
   * here; see the class comment's progress-throttle section.
   */
  static float transcribeOptRoutePass(
      PrintStream out,
      BatchOptimizer optimizer,
      RoutingBoard board,
      RouterSettings settings,
      int passNo,
      boolean withPreferredDirections) {
    RoutingJob job = optimizer.job;
    // :281.
    BoardStatistics boardStatisticsBefore = board.getStatistics();
    // :284.
    optimizer.progressThrottler.reset();
    // :287.
    optimizer.sortedRouteItems = optimizer.new ReadSortedRouteItems();
    // :288.
    optimizer.minCumulativeTraceLength = boardStatisticsBefore.traces.totalWeightedLength;
    // :300-304.
    int consecutiveFailures = 0;
    int maxConsecutiveFailures =
        settings.optimizer.maxConsecutiveFailures != null
            ? settings.optimizer.maxConsecutiveFailures
            : 50;
    // :306.
    float routeImproved = 0.0F;
    int n = 0;
    // :307.
    while (true) {
      // :308-313 — no deadline on this run.
      // :314-317.
      if (job.thread.isStopRequested()) {
        out.println("OPT-PASS-STOP pass=" + passNo + " reason=stop n=" + n);
        return routeImproved;
      }
      // :318-326.
      if (settings.optimizer.maxItems != null
          && settings.optimizer.maxItems > 0
          && optimizer.totalItemsOptimized >= settings.optimizer.maxItems) {
        out.println("OPT-PASS-STOP pass=" + passNo + " reason=max-items n=" + n);
        break;
      }
      // :327-330.
      Item currentItem = optimizer.sortedRouteItems.next();
      if (currentItem == null) {
        out.println("OPT-PASS-STOP pass=" + passNo + " reason=exhausted n=" + n);
        break;
      }
      int itemId = currentItem.getId();
      String kind = currentItem.getClass().getSimpleName();
      // :331.
      ItemRouteResult result = optimizer.optRouteItem(currentItem, withPreferredDirections, false);
      // :332.
      optimizer.totalItemsOptimized++;
      boolean broke = false;
      // :333-348.
      if (result.improved()) {
        consecutiveFailures = 0;
        // :335-338, ungated — see the class comment.
        board.getStatistics();
        routeImproved =
            (float)
                (boardStatisticsBefore.items.viaCount != 0
                        && boardStatisticsBefore.traces.totalLength != 0
                    ? 1.0
                        - ((((float) result.viaCount() / boardStatisticsBefore.items.viaCount)
                                + (result.traceLength() / boardStatisticsBefore.traces.totalLength))
                            / 2)
                    : 0);
      } else {
        // :350-360.
        consecutiveFailures++;
        broke = consecutiveFailures >= maxConsecutiveFailures;
      }
      out.println(
          "OPT-ITEM pass="
              + passNo
              + " n="
              + n
              + " id="
              + itemId
              + " kind="
              + kind
              + " improved="
              + result.improved()
              + " viaCount="
              + result.viaCount()
              + " traceLength="
              + Double.toString(result.traceLength())
              + " incompleteBefore="
              + result.incompleteCountBefore()
              + " incompleteAfter="
              + result.incompleteCount()
              + " improvementPercentage="
              + Float.toString(result.improvementPercentage())
              + " routeImproved="
              + Float.toString(routeImproved)
              + " consecutiveFailures="
              + consecutiveFailures
              + " items="
              + optimizer.totalItemsOptimized
              + " minCumulativeTraceLength="
              + Double.toString(optimizer.minCumulativeTraceLength));
      out.flush();
      n++;
      if (broke) {
        out.println("OPT-PASS-STOP pass=" + passNo + " reason=consecutive-failures n=" + n);
        break;
      }
    }
    // :364.
    optimizer.sortedRouteItems = null;
    // :365-368.
    if (optimizer.useIncreasedRipupCosts && (routeImproved == 0)) {
      optimizer.useIncreasedRipupCosts = false;
      routeImproved = -1;
    }
    // :371-372.
    new BoardStatistics(board);
    // :384.
    return routeImproved;
  }

  /**
   * The port's {@code PassRecord} tuple, rendered. The six numbers are the ones Java scatters over
   * the {@code :353-363} pass-completed log and the {@code BoardStatistics} beside it.
   */
  static String passRecord(int pass, float score, BoardStatistics stats) {
    return "pass="
        + pass
        + " score="
        + Float.toString(score)
        + " incompletes="
        + stats.connections.incompleteCount
        + " violations="
        + stats.clearanceViolations.totalCount
        + " vias="
        + stats.items.viaCount
        + " traces="
        + stats.items.traceCount;
  }

  // -----------------------------------------------------------------------------------------
  // The final board — `P6T15aProbe`'s polyline format
  // -----------------------------------------------------------------------------------------

  /** {@code P6T15aProbe.ln}. */
  static String ln(Line l) {
    return "("
        + ((IntPoint) l.a).x
        + ","
        + ((IntPoint) l.a).y
        + ")->("
        + ((IntPoint) l.b).x
        + ","
        + ((IntPoint) l.b).y
        + ")";
  }

  /** {@code P6T15aProbe.pt}. */
  static String pt(Polyline p, int no) {
    Point corner = p.corner(no);
    if (corner instanceof IntPoint ip) {
      return "(" + ip.x + "," + ip.y + ")";
    }
    FloatPoint f = p.cornerApprox(no);
    return "~(" + Double.toString(f.x) + "," + Double.toString(f.y) + ")";
  }

  /** {@code P6T15aProbe.poly} — the line array and the corners. */
  static String poly(Polyline p) {
    if (p == null) {
      return "null";
    }
    StringBuilder sb = new StringBuilder();
    sb.append("n=").append(p.lines.length).append(" lines=[");
    for (int i = 0; i < p.lines.length; i++) {
      if (i > 0) {
        sb.append(",");
      }
      sb.append(ln(p.lines[i]));
    }
    sb.append("] corners=[");
    for (int i = 0; i < p.cornerCount(); i++) {
      if (i > 0) {
        sb.append(",");
      }
      sb.append(pt(p, i));
    }
    sb.append("]");
    return sb.toString();
  }

  /** {@code P6T15aProbe.pointOf}. */
  static String pointOf(Point p) {
    FloatPoint f = p.toFloat().round().toFloat();
    return "(" + (int) f.x + "," + (int) f.y + ")";
  }

  /** {@code P6T15aProbe.nets}. */
  static String nets(int[] netNos) {
    StringBuilder sb = new StringBuilder("[");
    for (int i = 0; i < netNos.length; i++) {
      if (i > 0) {
        sb.append(",");
      }
      sb.append(netNos[i]);
    }
    return sb.append("]").toString();
  }

  /**
   * {@code P6T15aProbe.dumpBoard}, widened by the two lines a SES needs and that probe did not
   * print: a {@code Via}'s centre, padstack and layer span. One line per item, in
   * {@code getItems()} order (descending id, quirk #63).
   */
  static void dumpBoard(PrintStream out, RoutingBoard board) {
    out.println("maxId=" + board.communication.idGenerator.maxGeneratedId());
    for (Item item : board.getItems()) {
      StringBuilder sb = new StringBuilder();
      sb.append("item id=")
          .append(item.getId())
          .append(" type=")
          .append(item.getClass().getSimpleName())
          .append(" nets=")
          .append(nets(item.netNumbers))
          .append(" cl=")
          .append(item.clearanceClassIndex());
      if (item instanceof PolylineTrace trace) {
        sb.append(" layer=")
            .append(trace.getLayer())
            .append(" hw=")
            .append(trace.getHalfWidth())
            .append(" ")
            .append(poly(trace.polyline()));
      } else if (item instanceof Via via) {
        sb.append(" center=")
            .append(pointOf(via.getCenter()))
            .append(" padstack=")
            .append(via.getPadstack().name)
            .append(" firstLayer=")
            .append(via.firstLayer())
            .append(" lastLayer=")
            .append(via.lastLayer());
      } else if (item instanceof Pin pin) {
        sb.append(" center=").append(pointOf(pin.getCenter()));
      }
      out.println(sb);
    }
  }

}
