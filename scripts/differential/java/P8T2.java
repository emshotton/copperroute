package app.freerouting.core.results;

import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.core.RoutingJob;
import app.freerouting.core.RoutingJobState;
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;

import java.io.FileDescriptor;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Plan 8 Task 4 differential driver: {@code core.results.RoutingResultManifest} — the thirteen
 * {@code @SerializedName} fields and their Gson key order ({@code :28-65}), the three nested DTOs
 * ({@code :68-95}), {@code fromJob} ({@code :98-135}), {@code write} ({@code :138-144}), {@code
 * resolveGitSha} ({@code :147-161}) and the private {@code sha256Hex} ({@code :163-171}) — plus
 * {@code core.RouterJobResourceUsage}, which {@code fromJob:114} copies wholesale.
 *
 * <p>Usage: {@code P8T2 [shape] <fixturesDir> <scratchDir>}, and the internal {@code
 * P8T2 gitsha-child} mode described below. The Rust twin is {@code
 * scripts/differential/rust/src/bin/p8t2.rs}; {@code scripts/differential/run.sh p8t2} compiles
 * this against the HEAD jar, runs both, and diffs their stdout, which must be empty.
 *
 * <h2>The two halves of this driver, and which one Task 4 lands</h2>
 *
 * <p>The plan's {@code p8t2} is the end-to-end manifest gate: run the jar's {@code -de <dsn> -do
 * <ses> --router.result_json=<f>}, run the port's equivalent, and compare the two manifests
 * byte-identically after {@link #normalizeManifest}. <strong>The port's binary does not grow
 * {@code --router.result_json} until Task 6</strong>, so Task 4 lands (a) this whole Java half,
 * (b) {@link #normalizeManifest} on both sides, pinned row for row by the {@code [norm]} table,
 * and (c) a <em>fixed-clock unit run</em> of every manifest shape the CLI can produce, which is
 * the {@code [man]} table. Task 6 adds the {@code e2e} mode that drives the two binaries. The
 * committed transcript of this driver's {@code shape} mode is {@code
 * crates/fr-core/tests/data/p8t2-manifest-shape.txt}, which {@code crates/fr-core/tests/manifest.rs}
 * replays row by row.
 *
 * <h2>What "fixed clock" means here</h2>
 *
 * <p>{@code fromJob:101} is {@code Instant.now().toString()} and {@code :103} reads the
 * environment, so two of the thirteen fields are not reproducible. Every {@code [man]} row therefore
 * overwrites {@code generatedAt} with {@code 1970-01-01T00:00:00Z} and {@code gitSha} with {@code
 * 0000000} <em>after</em> {@code fromJob} has run — the same two fields {@link #normalizeManifest}
 * strips, and the same two the Rust twin injects. Everything else, including {@code
 * phases.autorouter.duration_seconds} and the whole {@code resource_usage} object, is printed
 * <strong>raw</strong>, because those are exactly the values quirks #254 and #256 are about and
 * normalising them away here would delete the evidence.
 *
 * <h2>Why the git-sha rows re-exec a child JVM</h2>
 *
 * <p>{@code resolveGitSha} reads {@code System.getenv} and {@code System.getProperty}, and a JVM
 * cannot modify its own environment. Each {@code [gitsha]} row therefore spawns {@code java -cp
 * <this classpath> app.freerouting.core.results.P8T2 gitsha-child} with the environment and the
 * {@code -D} properties the row wants, and prints what the child answered. The Rust twin spawns
 * its own binary the same way, so neither side mutates its own environment.
 *
 * <p><strong>The port has no system properties.</strong> {@code resolveGitSha}'s two {@code
 * System.getProperty} arms become environment lookups of the <em>same names</em>, so a row labelled
 * {@code prop=freerouting.git.sha:"…"} sets a {@code -D} on this side and an environment variable
 * of that literal name on the port's side. The one row where that rename is observable is {@code
 * legacy_prop_beats_blank_env}, and it is printed as an {@code XDIFF} carrying both answers on both
 * sides — see {@code resolve_git_sha}'s {@code // not reachable:} marker in
 * {@code crates/fr-core/src/manifest.rs}.
 *
 * <h2>Reflection</h2>
 *
 * <p>{@code sha256Hex} is {@code private static} ({@code :163}) and is reached with {@code
 * setAccessible}. Nothing else here needs it: every field and every other method of
 * {@code RoutingResultManifest}, {@code FixtureInfo}, {@code PhaseMetrics}, {@code PhaseDetail} and
 * {@code RouterJobResourceUsage} is {@code public}.
 */
public final class P8T2 {

  /** The instant every {@code [man]} row's {@code generated_at} is forced to. */
  static final String FIXED_INSTANT = "1970-01-01T00:00:00Z";

  /** The sha every {@code [man]} row's {@code git_sha} is forced to. */
  static final String FIXED_GIT_SHA = "0000000";

  /** What {@link #normalizeManifest} puts in place of every value it strips. */
  static final String NORMALIZED = "\"<normalized>\"";

  private P8T2() {}

  public static void main(String[] args) throws Exception {
    // A private handle on the real stdout, then `System.out` is silenced: `DsnReader.readBoard`
    // routes `FRLogger` warnings there (the old-KiCad-version notice on some corpus boards), and
    // the port logs nothing, so they would be a diff. `P8T3Probe.main` does the same.
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    String mode = args.length > 0 ? args[0] : "shape";
    if (mode.equals("gitsha-child")) {
      // The whole child: one line, no header, so the parent can read it back verbatim.
      out.println(RoutingResultManifest.resolveGitSha());
      out.flush();
      return;
    }
    if (!mode.equals("shape")) {
      throw new IllegalArgumentException(
          "usage: P8T2 [shape] <fixturesDir> <scratchDir>  (the e2e mode is Task 6's)");
    }

    Path fixtures = Path.of(args[1]);
    Path scratch = Path.of(args[2]);
    deleteRecursively(scratch);
    Files.createDirectories(scratch);

    out.println("# p8t2 shape — RoutingResultManifest (core/results/RoutingResultManifest.java)");
    manifestTable(out, fixtures, scratch);
    durationTable(out);
    gitShaTable(out);
    sha256Table(out, fixtures, scratch);
    writeTable(out, scratch);
    normTable(out, fixtures, scratch);
    out.flush();
  }

  // ===============================================================================================
  // [man] — the whole manifest, one line of JSON per row
  // ===============================================================================================

  private static void manifestTable(PrintStream out, Path fixtures, Path scratch) throws Exception {
    out.println("[man]");

    emit(out, "default", new RoutingResultManifest());

    emit(out, "no_input", pinned(RoutingResultManifest.fromJob(freshJob(), null, false, 1)));

    Path dsn = fixtures.resolve("empty_board.dsn");
    emit(out, "input_present", pinned(fromJob(freshJob(), dsn.toString(), true, 0)));

    emit(
        out,
        "input_missing",
        pinned(fromJob(freshJob(), scratch.resolve("no-such-file.dsn").toString(), false, 1)));

    emit(out, "input_directory", pinned(fromJob(freshJob(), fixtures.toString(), false, 1)));

    // `fromJob:107` dereferences `Path.of("/").getFileName()`, which is null.
    RoutingResultManifest rootManifest = null;
    String rootFailure = null;
    try {
      rootManifest = pinned(fromJob(freshJob(), "/", false, 1));
    } catch (NullPointerException e) {
      rootFailure = "NullPointerException";
    }
    if (rootFailure != null) {
      out.println("MAN root_input 000 XDIFF java=NullPointerException rust=fixture.filename=\"\"");
    } else {
      emit(out, "root_input", rootManifest);
    }

    RoutingJob passes = freshJob();
    passes.setCurrentPass(7);
    emit(out, "passes_7", pinned(fromJob(passes, dsn.toString(), true, 0)));

    RoutingJob zeroPass = freshJob();
    zeroPass.setCurrentPass(0);
    emit(out, "passes_0", pinned(fromJob(zeroPass, dsn.toString(), true, 0)));

    RoutingJob timed = freshJob();
    timed.startedAt = Instant.ofEpochMilli(0);
    timed.finishedAt = Instant.ofEpochMilli(1234);
    timed.setCurrentPass(3);
    timed.state = RoutingJobState.COMPLETED;
    emit(out, "completed_run", pinned(fromJob(timed, dsn.toString(), true, 0)));

    RoutingJob halfClock = freshJob();
    halfClock.startedAt = Instant.ofEpochMilli(0);
    emit(out, "started_only", pinned(fromJob(halfClock, dsn.toString(), false, 1)));

    RoutingJob timedOut = freshJob();
    timedOut.state = RoutingJobState.TIMED_OUT;
    emit(out, "timed_out", pinned(fromJob(timedOut, dsn.toString(), true, 0)));


    RoutingJob withPath = freshJob();
    withPath.routerSettings.resultJsonPath = scratch.resolve("result.json").toString();
    emit(
        out,
        "result_json_path",
        stripScratch(pinned(fromJob(withPath, dsn.toString(), true, 0)), scratch));

    // ── the three rows with a real board ────────────────────────────────────────────────────
    //
    // `fromJob:117`'s computing constructor (two `DesignRulesChecker` runs) and `:119-120`'s
    // `getNormalizedScore`.
    //
    // **The weights are set explicitly, and that is not decoration.** `new ScoringSettings()`
    // leaves every weight `null` (`ScoringSettings.java:68-81` — they are boxed `Float`/`Double`/
    // `Integer` with no initialiser), so `new RouterSettings()` + a board is enough to make
    // `getMaximumScore:620` unbox a `null` and throw. Quirk #258 records that; the CLI never
    // reaches it because merge #1 replaces `routerSettings` with `DefaultSettings`' table, which
    // does fill them. Fixed literals rather than `DefaultSettings` keep the transcript
    // host-independent — `DefaultSettings` reads `availableProcessors()`.
    RoutingJob boarded = withWeights(freshJob());
    boarded.board = loadBoard(dsn);
    boarded.state = RoutingJobState.COMPLETED;
    boarded.startedAt = Instant.ofEpochMilli(0);
    boarded.finishedAt = Instant.ofEpochMilli(2500);
    boarded.setCurrentPass(1);
    emit(out, "empty_board", pinned(fromJob(boarded, dsn.toString(), true, 0)));

    // `:118`'s second guard: a board, but `routerSettings.scoring == null`. `board_statistics` is
    // written and `normalized_score` is not.
    RoutingJob noScoring = freshJob();
    noScoring.routerSettings.scoring = null;
    noScoring.board = loadBoard(dsn);
    emit(out, "board_without_scoring", pinned(fromJob(noScoring, dsn.toString(), true, 0)));

    // A board that actually has connections, so `getMaximumScore` is non-zero and
    // `getNormalizedScore:634` runs the division rather than the `<= 0f` guard.
    Path routed = fixtures.resolve("Issue103-Board-Routed.dsn");
    // A second weight set, deliberately: `withWeights`' trace cost of 1.0 per unit dwarfs a routed
    // board's `maximumScore`, so `calculateScore` goes negative and `Math.max(0, …)` clamps the
    // answer back to 0 — the same number the *guard* produces, which would prove nothing. Zeroing
    // the trace cost and shrinking the via cost leaves `getNormalizedScore:634`'s division and its
    // `float` narrowing as the only things deciding the value.
    RoutingJob connected = freshJob();
    connected.routerSettings.scoring.unroutedNetPenalty = 10.0f;
    connected.routerSettings.scoring.clearanceViolationPenalty = 5.0f;
    connected.routerSettings.scoring.bendPenalty = 0.5f;
    connected.routerSettings.scoring.defaultPreferredDirectionTraceCost = 0.0;
    connected.routerSettings.scoring.viaCosts = 1;
    connected.board = loadBoard(routed);
    connected.state = RoutingJobState.COMPLETED;
    connected.setCurrentPass(2);
    emit(out, "connected_board", pinned(fromJob(connected, routed.toString(), true, 0)));
  }

  /**
   * The five scoring weights {@code calculateScore}/{@code getMaximumScore} unbox, set to fixed
   * literals. See {@link #manifestTable}'s comment for why they are not {@code DefaultSettings}'.
   */
  private static RoutingJob withWeights(RoutingJob job) {
    job.routerSettings.scoring.unroutedNetPenalty = 10.0f;
    job.routerSettings.scoring.clearanceViolationPenalty = 5.0f;
    job.routerSettings.scoring.bendPenalty = 0.5f;
    job.routerSettings.scoring.defaultPreferredDirectionTraceCost = 1.0;
    job.routerSettings.scoring.viaCosts = 42;
    return job;
  }

  /** `fromJob` with the four-argument signature spelt out, so every call site reads the same. */
  private static RoutingResultManifest fromJob(
      RoutingJob job, String inputFilePath, boolean outputWritten, int exitCode) {
    return RoutingResultManifest.fromJob(job, inputFilePath, outputWritten, exitCode);
  }

  /** The clock and the environment, replaced by the two literals — see the class docs. */
  private static RoutingResultManifest pinned(RoutingResultManifest manifest) {
    manifest.generatedAt = FIXED_INSTANT;
    manifest.gitSha = FIXED_GIT_SHA;
    return manifest;
  }

  /** A manifest whose scratch-directory paths are replaced, so the transcript is portable. */
  private static RoutingResultManifest stripScratch(
      RoutingResultManifest manifest, Path scratch) {
    if (manifest.settingsSnapshot != null && manifest.settingsSnapshot.resultJsonPath != null) {
      manifest.settingsSnapshot.resultJsonPath =
          manifest.settingsSnapshot.resultJsonPath.replace(scratch.toString(), "<SCRATCH>");
    }
    return manifest;
  }

  private static void emit(PrintStream out, String label, RoutingResultManifest manifest) {
    String json = app.freerouting.util.gson.GsonProvider.GSON.toJson(manifest);
    String[] lines = json.split("\n", -1);
    for (int i = 0; i < lines.length; i++) {
      out.printf("MAN %s %03d %s%n", label, i, escape(lines[i]));
    }
    out.printf("MAN %s len %d%n", label, json.getBytes(StandardCharsets.UTF_8).length);
  }

  /**
   * A job in exactly the state {@code Freerouting.java:139-147} leaves one before routing: the
   * one-argument constructor, and {@code routerSettings} still the field initialiser's {@code new
   * RouterSettings()}.
   *
   * <p>The session id is a constant rather than {@code UUID.randomUUID()} so the transcript
   * reproduces; nothing the manifest carries is derived from it (the manifest has no {@code id},
   * {@code name} or {@code short_name} field), but a constant costs nothing and removes the
   * question.
   */
  private static RoutingJob freshJob() {
    return new RoutingJob(new java.util.UUID(0L, 0L));
  }

  private static RoutingBoard loadBoard(Path dsn) throws Exception {
    BoardReadResult result;
    String designName = dsn.getFileName().toString();
    try (FileInputStream in = new FileInputStream(dsn.toFile())) {
      result = DsnReader.readBoard(in, null, null, designName);
    }
    return switch (result) {
      case BoardReadResult.Success s -> (RoutingBoard) s.board();
      case BoardReadResult.OutlineMissing o -> (RoutingBoard) o.board();
      default -> throw new IllegalStateException(designName + " did not read: " + result);
    };
  }

  // ===============================================================================================
  // [dur] — `fromJob:128-132`'s float narrowing, isolated
  // ===============================================================================================

  private static void durationTable(PrintStream out) {
    out.println("[dur]");
    long[][] pairs = {
      {0, 0}, {0, 1}, {0, 3}, {0, 33}, {0, 100}, {0, 999}, {0, 1000}, {0, 1234}, {0, 3661500},
      {0, 86400000}, {0, 86399999}, {5, 1239},
    };
    for (long[] pair : pairs) {
      Instant started = Instant.ofEpochMilli(pair[0]);
      Instant finished = Instant.ofEpochMilli(pair[1]);
      float seconds =
          (float) (java.time.Duration.between(started, finished).toMillis() / 1000.0);
      out.printf("DUR %d->%d %s%n", pair[0], pair[1], Float.toString(seconds));
    }
    // `Duration.between` on two wall-clock instants can be negative; `std::time::Instant` is
    // monotonic, so the port cannot build the input at all. Both sides print this row verbatim.
    out.println("DUR 1234->0 XDIFF java=-1.234 rust=unreachable(monotonic clock)");
  }

  // ===============================================================================================
  // [gitsha] — `resolveGitSha:147-161`, one child JVM per row
  // ===============================================================================================

  private static void gitShaTable(PrintStream out) throws Exception {
    out.println("[gitsha]");
    gitSha(out, "none", null, null, null);
    gitSha(out, "env", "deadbeef", null, null);
    gitSha(out, "env_padded", "  deadbeef  ", null, null);
    gitSha(out, "env_tab_newline", "\tdeadbeef\n", null, null);
    gitSha(out, "env_empty", "", null, null);
    gitSha(out, "env_spaces", "   ", null, null);
    gitSha(out, "prop", null, "cafebabe", null);
    gitSha(out, "prop_padded", null, " cafebabe ", null);
    gitSha(out, "env_beats_prop", "deadbeef", "cafebabe", null);
    gitSha(out, "blank_env_falls_to_prop", "   ", "cafebabe", null);
    gitSha(out, "legacy_prop_only", null, null, "0badc0de");
    // `String.isBlank()` is `Character.isWhitespace`, which says yes to U+001C..U+001F and no to
    // the three non-breaking spaces; `String.trim()` strips only code units <= U+0020. Rust's
    // `char::is_whitespace`/`str::trim` disagree with both, which is why the port writes
    // `java_is_blank`/`java_trim` out by hand.
    gitSha(out, "file_separators_only", "\u001c\u001d\u001e\u001f", null, null);
    gitSha(out, "file_separators_around", "\u001cdeadbeef\u001f", null, null);
    gitSha(out, "nbsp_only", "\u00a0", null, null);
    // `U+0085` NEL — the fourth member of the "Rust calls it whitespace, Java does not" half.
    // `Character.isWhitespace(0x85)` is false and `trim()` strips only code units <= U+0020, so
    // the jar hands the NEL straight back. Task review SF1.
    gitSha(out, "nel_only", "\u0085", null, null);
    gitSha(out, "nel_around", "\u0085deadbeef\u0085", null, null);
    gitSha(out, "figure_space_only", "\u2007", null, null);
    gitSha(out, "narrow_nbsp_only", "\u202f", null, null);
    gitSha(out, "ideographic_space_only", "\u3000", null, null);
    gitSha(out, "nbsp_around", "\u00a0deadbeef\u00a0", null, null);
    // The one input where the two `FREEROUTING_GIT_SHA` sources can disagree: Java's third arm is
    // a *system property* of the same name as the first arm's environment variable, and the port
    // renames it to that same environment variable — so a blank environment value plus a
    // non-blank property is answerable in Java and not in the port.
    out.println(
        "GITSHA legacy_prop_beats_blank_env XDIFF java=0badc0de rust=unknown"
            + " (resolveGitSha:156-159 renames onto arm 1's variable)");
    // The rename's *second* consequence, and the sharper one: on this side the legacy property is
    // the LAST arm and loses to `freerouting.git.sha`; on the port it has become the FIRST arm's
    // environment variable and therefore wins. Same two inputs, opposite answers.
    out.println(
        "GITSHA prop_beats_legacy_prop XDIFF java=cafebabe rust=0badc0de"
            + " (arm 3 renames onto arm 1, which outranks arm 2)");
  }

  private static void gitSha(
      PrintStream out, String label, String env, String prop, String legacyProp) throws Exception {
    List<String> command = new ArrayList<>();
    command.add(Path.of(System.getProperty("java.home"), "bin", "java").toString());
    command.add("-Djava.awt.headless=true");
    if (prop != null) {
      command.add("-Dfreerouting.git.sha=" + prop);
    }
    if (legacyProp != null) {
      command.add("-DFREEROUTING_GIT_SHA=" + legacyProp);
    }
    command.add("-cp");
    command.add(System.getProperty("java.class.path"));
    command.add("app.freerouting.core.results.P8T2");
    command.add("gitsha-child");

    ProcessBuilder builder = new ProcessBuilder(command);
    builder.environment().remove("FREEROUTING_GIT_SHA");
    builder.environment().remove("freerouting.git.sha");
    if (env != null) {
      builder.environment().put("FREEROUTING_GIT_SHA", env);
    }
    builder.redirectErrorStream(false);
    Process process = builder.start();
    String answer = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
    process.waitFor();
    // `println` in the child, so exactly one trailing newline comes back.
    if (answer.endsWith("\n")) {
      answer = answer.substring(0, answer.length() - 1);
    }
    out.printf("GITSHA %s in=%s out=%s%n", label, describeSources(env, prop, legacyProp), escape(answer));
  }

  private static String describeSources(String env, String prop, String legacyProp) {
    Map<String, String> parts = new LinkedHashMap<>();
    if (env != null) {
      parts.put("env:FREEROUTING_GIT_SHA", escape(env));
    }
    if (prop != null) {
      parts.put("prop:freerouting.git.sha", escape(prop));
    }
    if (legacyProp != null) {
      parts.put("prop:FREEROUTING_GIT_SHA", escape(legacyProp));
    }
    if (parts.isEmpty()) {
      return "(nothing)";
    }
    StringBuilder text = new StringBuilder();
    for (Map.Entry<String, String> entry : parts.entrySet()) {
      if (text.length() > 0) {
        text.append('|');
      }
      text.append(entry.getKey()).append("=\"").append(entry.getValue()).append('"');
    }
    return text.toString();
  }

  // ===============================================================================================
  // [sha256] — the private `sha256Hex:163-171`, by reflection
  // ===============================================================================================

  private static void sha256Table(PrintStream out, Path fixtures, Path scratch) throws Exception {
    out.println("[sha256]");
    Method sha256Hex = RoutingResultManifest.class.getDeclaredMethod("sha256Hex", Path.class);
    sha256Hex.setAccessible(true);

    Path empty = scratch.resolve("sha-empty.bin");
    Files.write(empty, new byte[0]);
    sha256(out, sha256Hex, "empty_file", empty);

    Path abc = scratch.resolve("sha-abc.bin");
    Files.write(abc, "abc".getBytes(StandardCharsets.US_ASCII));
    sha256(out, sha256Hex, "nist_abc", abc);

    Path twoBlocks = scratch.resolve("sha-two-blocks.bin");
    Files.write(
        twoBlocks,
        "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"
            .getBytes(StandardCharsets.US_ASCII));
    sha256(out, sha256Hex, "nist_two_blocks", twoBlocks);

    byte[] million = new byte[1_000_000];
    java.util.Arrays.fill(million, (byte) 'a');
    Path millionPath = scratch.resolve("sha-million-a.bin");
    Files.write(millionPath, million);
    sha256(out, sha256Hex, "nist_million_a", millionPath);

    for (int length : new int[] {55, 56, 63, 64, 65}) {
      byte[] bytes = new byte[length];
      java.util.Arrays.fill(bytes, (byte) 'a');
      Path path = scratch.resolve("sha-" + length + "a.bin");
      Files.write(path, bytes);
      sha256(out, sha256Hex, "block_boundary_" + length, path);
    }

    sha256(out, sha256Hex, "fixture_empty_board", fixtures.resolve("empty_board.dsn"));
    sha256(out, sha256Hex, "fixture_issue026_ses", fixtures.resolve("Issue026-J2_reference.ses"));
    sha256(out, sha256Hex, "missing", scratch.resolve("no-such-file.bin"));
    sha256(out, sha256Hex, "directory", fixtures);
  }

  private static void sha256(PrintStream out, Method sha256Hex, String label, Path path)
      throws Exception {
    Object answer = sha256Hex.invoke(null, path);
    out.printf("SHA256 %s %s%n", label, answer == null ? "(null->key omitted)" : answer);
  }

  // ===============================================================================================
  // [write] — `write:137-144`
  // ===============================================================================================

  private static void writeTable(PrintStream out, Path scratch) throws Exception {
    out.println("[write]");
    RoutingResultManifest manifest = pinned(fromJob(freshJob(), null, false, 1));
    String json = app.freerouting.util.gson.GsonProvider.GSON.toJson(manifest);
    byte[] expected = json.getBytes(StandardCharsets.UTF_8);

    Path flat = scratch.resolve("manifest.json");
    RoutingResultManifest.write(flat, manifest);
    report(out, "existing_parent", flat, expected);

    Path nested = scratch.resolve("a/b/c/manifest.json");
    RoutingResultManifest.write(nested, manifest);
    report(out, "created_parents", nested, expected);

    // Written twice: `Files.writeString` truncates, so the size does not grow.
    RoutingResultManifest.write(flat, manifest);
    report(out, "overwritten", flat, expected);

    // `write:139` tests `getParent() != null`; for a bare filename it is null and no directory is
    // created. Reported rather than exercised, so the driver never writes outside the scratch
    // directory.
    out.printf("WRITE bare_filename parent=%s%n", Path.of("manifest.json").getParent());
    out.printf("WRITE nested_parent parent=%s%n", parentOf(nested, scratch));
    out.printf("WRITE root_parent parent=%s%n", Path.of("/").getParent());
  }

  private static String parentOf(Path path, Path scratch) {
    Path parent = path.getParent();
    return parent == null ? "null" : parent.toString().replace(scratch.toString(), "<SCRATCH>");
  }

  private static void report(PrintStream out, String label, Path path, byte[] expected)
      throws Exception {
    byte[] actual = Files.readAllBytes(path);
    out.printf(
        "WRITE %s exists=%s bytes=%d matches_toJson=%s last_byte=%s%n",
        label,
        Files.exists(path),
        actual.length,
        java.util.Arrays.equals(actual, expected),
        actual.length == 0 ? "(none)" : escape(String.valueOf((char) actual[actual.length - 1])));
  }

  // ===============================================================================================
  // [norm] — `normalize_manifest`, the comparison both halves of the e2e gate run through
  // ===============================================================================================

  private static void normTable(PrintStream out, Path fixtures, Path scratch) throws Exception {
    out.println("[norm]");
    // Deliberately NOT `pinned`: this manifest carries a real `Instant.now()`, a real
    // `resolveGitSha()`, a real duration and a real absolute path, and the normaliser is what
    // makes it reproducible. If the normaliser stopped stripping any one of them this table would
    // stop being byte-stable and the driver would fail on its own output.
    RoutingJob job = freshJob();
    job.startedAt = Instant.now();
    job.finishedAt = job.startedAt.plusMillis(777);
    job.setCurrentPass(4);
    job.state = RoutingJobState.COMPLETED;
    job.routerSettings.resultJsonPath = scratch.resolve("result.json").toString();
    job.resourceUsage.cpuTimeUsed = 1.5f;
    job.resourceUsage.peakMemoryUsed = 321.25f;
    RoutingResultManifest manifest =
        fromJob(job, fixtures.resolve("empty_board.dsn").toString(), true, 0);

    String json = app.freerouting.util.gson.GsonProvider.GSON.toJson(manifest);
    String normalized = normalizeManifest(json, scratch.toString());
    String[] lines = normalized.split("\n", -1);
    for (int i = 0; i < lines.length; i++) {
      out.printf("NORM live %03d %s%n", i, escape(lines[i]));
    }
  }

  /**
   * The comparison the {@code p8t2} end-to-end gate runs both manifests through before diffing
   * them: the fields that cannot reproduce across two implementations, two machines or two runs
   * are replaced or removed.
   *
   * <p>Five rules, applied line by line to Gson's pretty output (one key per line, two-space
   * indent), in this order:
   *
   * <ol>
   *   <li>the whole {@code "resource_usage"} object is <strong>removed</strong> — plan ruling 8 and
   *       quirk label J: Java fills it from a monitor thread this port does not have, and two of
   *       its five fields ({@code io_read}, {@code io_written}) are never written by anybody
   *       (quirk #256);
   *   <li>{@code "generated_at"}'s value becomes {@code "<normalized>"} — {@code fromJob:101}'s
   *       wall clock;
   *   <li>{@code "git_sha"}'s value becomes {@code "<normalized>"} — {@code :103}'s environment;
   *   <li>every {@code "duration_seconds"} value becomes {@code "<normalized>"} — {@code :131}'s
   *       measured wall-clock duration, which quirk #254 also shows is the wrong stage's;
   *   <li>every occurrence of {@code absolutePrefix} becomes {@code <DIR>}, so a manifest written
   *       under two different working directories still compares equal.
   * </ol>
   *
   * <p>A key's trailing comma is preserved, because the result must still be valid JSON: the
   * {@code resource_usage} object is never the last key of the manifest ({@code final_state},
   * {@code exit_code} and {@code output_written} follow it at {@code :58-65}, and the last two are
   * primitives that Gson always writes), so removing it never orphans a comma.
   */
  static String normalizeManifest(String json, String absolutePrefix) {
    List<String> kept = new ArrayList<>();
    int skipDepth = 0;
    boolean skipping = false;
    for (String line : json.split("\n", -1)) {
      if (skipping) {
        skipDepth += count(line, '{') - count(line, '}');
        if (skipDepth <= 0) {
          skipping = false;
        }
        continue;
      }
      String trimmed = line.trim();
      if (trimmed.startsWith("\"resource_usage\":")) {
        int depth = count(line, '{') - count(line, '}');
        if (depth > 0) {
          skipping = true;
          skipDepth = depth;
        }
        continue;
      }
      kept.add(normalizeLine(line, absolutePrefix));
    }
    return String.join("\n", kept);
  }

  private static String normalizeLine(String line, String absolutePrefix) {
    for (String key : new String[] {"generated_at", "git_sha", "duration_seconds"}) {
      String needle = "\"" + key + "\": ";
      int at = line.indexOf(needle);
      if (at >= 0) {
        String indent = line.substring(0, at);
        String comma = line.endsWith(",") ? "," : "";
        return indent + needle + NORMALIZED + comma;
      }
    }
    if (!absolutePrefix.isEmpty()) {
      return line.replace(absolutePrefix, "<DIR>");
    }
    return line;
  }

  private static int count(String text, char c) {
    int total = 0;
    for (int i = 0; i < text.length(); i++) {
      if (text.charAt(i) == c) {
        total++;
      }
    }
    return total;
  }

  // ===============================================================================================
  // shared helpers
  // ===============================================================================================

  /** Every byte outside printable ASCII as {@code \\uXXXX}, so the transcript is ASCII. */
  static String escape(String text) {
    StringBuilder out = new StringBuilder(text.length());
    for (int i = 0; i < text.length(); i++) {
      char c = text.charAt(i);
      if (c >= 0x20 && c < 0x7f) {
        out.append(c);
      } else {
        out.append(String.format("\\u%04x", (int) c));
      }
    }
    return out.toString();
  }

  private static void deleteRecursively(Path path) throws Exception {
    if (!Files.exists(path)) {
      return;
    }
    try (var walk = Files.walk(path)) {
      for (Path entry : walk.sorted(java.util.Comparator.reverseOrder()).toList()) {
        Files.deleteIfExists(entry);
      }
    }
  }
}
