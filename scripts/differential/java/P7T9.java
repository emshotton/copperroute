package app.freerouting.autoroute.pipeline;

import static app.freerouting.autoroute.pipeline.BatchAutorouter.BOARD_RANK_LIMIT;
import static app.freerouting.autoroute.pipeline.BatchAutorouter.FANOUT_RECOVERY_STAGNATION_PASSES;
import static app.freerouting.autoroute.pipeline.BatchAutorouter.MAXIMUM_TRIES_ON_THE_SAME_BOARD;
import static app.freerouting.autoroute.pipeline.BatchAutorouter.STAGNATION_PASS_LIMIT;
import static app.freerouting.autoroute.pipeline.BatchAutorouter.STAGNATION_SCORE_THRESHOLD;
import static app.freerouting.autoroute.pipeline.BatchAutorouter.STOP_AT_PASS_MINIMUM;
import static app.freerouting.autoroute.pipeline.BatchAutorouter.STOP_AT_PASS_MODULO;

import app.freerouting.autoroute.BoardHistory;
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
import app.freerouting.settings.FanoutSettings;
import app.freerouting.settings.RouterSettings;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;

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
      System.err.println("usage: P7T9 <dsn> [maxPasses] [mode]");
      System.exit(2);
    }
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Path dsn = Paths.get(args[0]).toAbsolutePath().normalize();
    int maxPasses = args.length > 1 && !args[1].isBlank() ? Integer.parseInt(args[1]) : 1;
    String mode = args.length > 2 && !args[2].isBlank() ? args[2] : "router-only";
    if (!"router-only".equals(mode) && !"router+fanout".equals(mode)) {
      System.err.println("P7T9: mode must be 'router-only' or 'router+fanout', not: " + mode);
      System.exit(2);
    }

    Path jar =
        Paths.get(RoutingBoard.class.getProtectionDomain().getCodeSource().getLocation().toURI())
            .toRealPath();
    out.printf(
        "HEADER jar=%s bytes=%d mtime=%d fixture=%s maxPasses=%d mode=%s%n",
        jar,
        Files.size(jar),
        Files.getLastModifiedTime(jar).toMillis(),
        dsn.getFileName(),
        maxPasses,
        mode);
    System.err.println("java-version " + System.getProperty("java.version"));

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
    settings.fanout.enabled = "router+fanout".equals(mode) ? Boolean.TRUE : Boolean.FALSE;
    // Ruling AI: `fanoutPass:231-232`'s per-pin `TimeLimit` is built from this setting, so this
    // is where the fanout stage's wall clock is disabled — on both sides, with the same number.
    settings.fanout.maxMillisecondsPerPin = (long) Integer.MAX_VALUE;
    settings.setRunRouter(true);
    settings.setRunOptimizer(false);
    // `:408-410` — the snapshot event. Off, so `fireBoardSnapshotEvent` is not on this run's path;
    // the port's `RoutingEvent::BoardSnapshot` is pinned by a unit test instead.
    settings.saveIntermediateStages = Boolean.FALSE;
    return settings;
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
      out.println("PASS " + passRecord(currentPass, boardScoreAfter, boardStatisticsAfter));

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
