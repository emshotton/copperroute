package app.freerouting.core;

import app.freerouting.autoroute.pipeline.NamedAlgorithmType;
import app.freerouting.autoroute.pipeline.TaskState;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Modifier;

// Plan 7 Task 4 ground-truth probe (controller ruling AI): the three-state stop flag
// (`core/StopRequestState.java`, `core/StoppableThread.java:8, 20-42`), the two small pipeline
// enums (`autoroute/pipeline/{NamedAlgorithmType,TaskState}.java`) and the field list of
// `core/RouterCounters.java`.
//
// It is not a differential driver — there is no Rust twin and `run.sh` does not know it, the
// `P7T2Probe` pattern — but every literal in `crates/fr-router/tests/stop_and_progress.rs` that
// this probe can reach comes from its stdout, committed verbatim as
// `crates/fr-router/tests/data/p7t4-stop-and-counters.txt` and replayed by that test.
//
// ## The probe deliberately carries no clock
//
// Ruling AI's deadline is asserted against Java's *code* — the per-job monitor thread at
// `management/jobs/RoutingJobSchedulerActionThread.java:55-90`, quoted in
// `crates/fr-router/src/pipeline/stop.rs` — and not against a timing measurement, because a
// timing measurement is not reproducible and ruling AI's whole point is that the parity runs
// never reach the deadline. So there is no `Thread.sleep` and no `System.currentTimeMillis()`
// anywhere below, and the transcript is byte-stable across runs and machines.
//
// ## What is reachable and what is not
//
// `[constants]` reflects the four `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` static finals, which are
// the port's `RouterBudget::opt_changed_area_ms` default. The other three budget literals are
// **not** reflectable and are therefore cited by source line in the port's test messages instead
// (the controller's "a small probe or direct source citation per field"):
//
//   * `250` is an inline literal in a method body, `BatchAutorouter.shouldFireBoardUpdate`
//     (`autoroute/pipeline/BatchAutorouter.java:337-338`);
//   * `10000L` is an inline `?:` fallback in `BatchFanout.fanoutPass`
//     (`autoroute/pipeline/BatchFanout.java:175-178`);
//   * `core.ProgressThrottler`'s interval is a **constructor argument**, not a constant: all
//     three construction sites pass `1000` (`autoroute/pipeline/BatchOptimizer.java:29`,
//     `autoroute/pipeline/BatchAutorouterThread.java:43`, `autoroute/pipeline/BatchFanout.java:28`)
//     and each is an instance-field initialiser of a class this probe cannot instantiate without
//     a board, a job and a settings tree.
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p7t4 java/probes/P7T4Probe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 -cp "/tmp/p7t4:$JAR" \
//       app.freerouting.core.P7T4Probe \
//     > ../../crates/fr-router/tests/data/p7t4-stop-and-counters.txt
//
// The package is `app.freerouting.core` (the brief's), which is what reaches
// `StoppableThread`'s package-private surface and `BatchAutorouter`'s package-private
// `TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP` without a module opening.
public final class P7T4Probe {

  /** The minimal concrete `StoppableThread`: `threadAction` is abstract and never run here. */
  private static final class Probe extends StoppableThread {
    @Override
    protected void threadAction() {
      // The probe never calls `run()`; the flag is driven directly, on this thread.
    }
  }

  private static PrintStream out;

  public static void main(String[] args) throws Exception {
    // `FRLogger` writes to stdout when the jar's classes initialise; print through our own
    // stream on the raw file descriptor so no `grep` filter is needed (the `P6T1`/`P7T2Probe`
    // guard).
    out = new PrintStream(new FileOutputStream(FileDescriptor.out), true);

    out.println("# P7T4Probe — plan 7 task 4 ground truth, HEAD jar");
    out.println("# core/StopRequestState.java, core/StoppableThread.java:8,20-42,");
    out.println("# autoroute/pipeline/{NamedAlgorithmType,TaskState}.java, core/RouterCounters.java");
    out.println("# no clock: this transcript is byte-stable across runs (ruling AI)");

    printEnums();
    printQueriesAtRest();
    printTransitions();
    printSequences();
    printConstants();
    printRouterCounters();

    out.flush();
  }

  // -------------------------------------------------------------------------------------------
  // The three enums, in declaration order, with their ordinals.
  // -------------------------------------------------------------------------------------------
  private static void printEnums() {
    out.println("[enums]");
    StringBuilder sb = new StringBuilder("StopRequestState =");
    for (StopRequestState s : StopRequestState.values()) {
      sb.append(' ').append(s.name()).append('(').append(s.ordinal()).append(')');
    }
    out.println(sb);

    sb = new StringBuilder("TaskState =");
    for (TaskState s : TaskState.values()) {
      sb.append(' ').append(s.name()).append('(').append(s.ordinal()).append(')');
    }
    out.println(sb);

    sb = new StringBuilder("NamedAlgorithmType =");
    for (NamedAlgorithmType s : NamedAlgorithmType.values()) {
      sb.append(' ').append(s.name()).append('(').append(s.ordinal()).append(')');
    }
    out.println(sb);
  }

  // -------------------------------------------------------------------------------------------
  // The two queries, read in every state without requesting anything: `isStopRequested()` is
  // `== ALL` (:28-30) and `isStopAutoRouterRequested()` is `!= NONE` (:40-42). The two are
  // adjacent and easy to swap, which is why both are read in all three states.
  // -------------------------------------------------------------------------------------------
  private static void printQueriesAtRest() throws Exception {
    out.println("[queries-at-rest]");
    for (StopRequestState from : StopRequestState.values()) {
      Probe p = withState(from);
      out.printf(
          "state=%s isStopRequested=%s isStopAutoRouterRequested=%s%n",
          from.name(), p.isStopRequested(), p.isStopAutoRouterRequested());
    }
  }

  // -------------------------------------------------------------------------------------------
  // The full transition table: 3 starting states x 2 requests, each followed by both queries.
  // -------------------------------------------------------------------------------------------
  private static void printTransitions() throws Exception {
    out.println("[transitions]");
    for (StopRequestState from : StopRequestState.values()) {
      for (String request : new String[] {"requestStop", "requestStopAutoRouter"}) {
        Probe p = withState(from);
        if (request.equals("requestStop")) {
          p.requestStop();
        } else {
          p.requestStopAutoRouter();
        }
        out.printf(
            "from=%s request=%s -> state=%s isStopRequested=%s isStopAutoRouterRequested=%s%n",
            from.name(),
            request,
            readState(p).name(),
            p.isStopRequested(),
            p.isStopAutoRouterRequested());
      }
    }
  }

  // -------------------------------------------------------------------------------------------
  // The two orderings that the pipeline actually performs, spelled out because the difference
  // between them is what quirk candidates C and R ride on:
  //
  //   * `maxItems` -> `AutoroutePassRunner.java:219` requestStop()      -> ALL
  //   * `maxPasses` -> `AutorouteBatchLoop.java:271` requestStopAutoRouter() -> AUTO_ROUTER_ONLY
  //   * `RoutingPipeline.java:117` gates the optimizer stage on isStopRequested(), i.e. on ALL.
  //
  // and the monitor-thread order, `requestStop()` then (30 s later) `job.state = TIMED_OUT`
  // then `AutorouteBatchLoop.java:251-253`'s `requestStopAutoRouter()`, which is a no-op.
  // -------------------------------------------------------------------------------------------
  private static void printSequences() throws Exception {
    out.println("[sequences]");

    Probe maxItems = new Probe();
    maxItems.requestStop();
    out.printf(
        "seq=maxItems calls=requestStop state=%s optimizerStageWouldRun=%s%n",
        readState(maxItems).name(), !maxItems.isStopRequested());

    Probe maxPasses = new Probe();
    maxPasses.requestStopAutoRouter();
    out.printf(
        "seq=maxPasses calls=requestStopAutoRouter state=%s optimizerStageWouldRun=%s%n",
        readState(maxPasses).name(), !maxPasses.isStopRequested());

    Probe monitor = new Probe();
    monitor.requestStop();
    StopRequestState afterMonitor = readState(monitor);
    monitor.requestStopAutoRouter();
    out.printf(
        "seq=monitorTimeout calls=requestStop,requestStopAutoRouter stateAfterFirst=%s "
            + "stateAfterSecond=%s secondWasNoOp=%s%n",
        afterMonitor.name(),
        readState(monitor).name(),
        afterMonitor == readState(monitor));

    Probe twice = new Probe();
    twice.requestStopAutoRouter();
    StopRequestState afterFirst = readState(twice);
    twice.requestStopAutoRouter();
    out.printf(
        "seq=autoRouterTwice stateAfterFirst=%s stateAfterSecond=%s%n",
        afterFirst.name(), readState(twice).name());
  }

  // -------------------------------------------------------------------------------------------
  // `RouterBudget::opt_changed_area_ms`'s default, reflected from the four declarations.
  // -------------------------------------------------------------------------------------------
  private static void printConstants() throws Exception {
    out.println("[constants]");
    String[][] sites = {
      {"app.freerouting.autoroute.pipeline.AutorouteConnectionRouter", "22"},
      {"app.freerouting.autoroute.pipeline.BatchAutorouter", "43"},
      {"app.freerouting.autoroute.pipeline.BatchAutorouterThread", "38"},
      {"app.freerouting.autoroute.pipeline.AutoroutePassRunner", "32"},
    };
    for (String[] site : sites) {
      Class<?> cls = Class.forName(site[0]);
      Field f = cls.getDeclaredField("TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP");
      f.setAccessible(true);
      out.printf(
          "%s.TIME_LIMIT_TO_PREVENT_ENDLESS_LOOP:%s = %s%n",
          cls.getSimpleName(), site[1], f.get(null));
    }
  }

  // -------------------------------------------------------------------------------------------
  // `RouterCounters`' declared instance fields, in declaration order, with each field's type and
  // the value a freshly constructed instance carries. Nine fields; `phase` and
  // `fanoutExtraViasCount` are the two an eyeball transcription misses.
  // -------------------------------------------------------------------------------------------
  private static void printRouterCounters() throws Exception {
    out.println("[RouterCounters]");
    RouterCounters rc = new RouterCounters();
    Field[] fields = RouterCounters.class.getDeclaredFields();
    int index = 0;
    for (Field f : fields) {
      if (f.isSynthetic() || Modifier.isStatic(f.getModifiers())) {
        continue;
      }
      f.setAccessible(true);
      out.printf(
          "field %d name=%s type=%s default=%s%n",
          index++, f.getName(), f.getType().getName(), f.get(rc));
    }
    out.printf("fieldCount = %d%n", index);
  }

  // -------------------------------------------------------------------------------------------
  private static Probe withState(StopRequestState state) throws Exception {
    Probe p = new Probe();
    Field f = StoppableThread.class.getDeclaredField("stopRequestState");
    f.setAccessible(true);
    f.set(p, state);
    return p;
  }

  private static StopRequestState readState(Probe p) throws Exception {
    Field f = StoppableThread.class.getDeclaredField("stopRequestState");
    f.setAccessible(true);
    return (StopRequestState) f.get(p);
  }

  private P7T4Probe() {}
}
