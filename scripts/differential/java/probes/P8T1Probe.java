package app.freerouting.core;

import app.freerouting.io.FileFormat;
import java.io.File;
import java.io.FileInputStream;
import java.io.InputStream;
import java.lang.reflect.Method;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.UUID;
import java.util.zip.CRC32;

/**
 * Plan 8 Task 1's JVM-pinned evidence: the whole job model — {@code RoutingJob.getFileFormat(byte[])}
 * and {@code getFileFormat(Path)}, {@code changeFileExtension}, {@code tryToSetInput},
 * {@code tryToSetOutputFile}, {@code setInputFromFile}'s default-output derivation, and
 * {@code BoardFileDetails.setFilename} / {@code calculateCrc32}.
 *
 * <p>Declares {@code package app.freerouting.core} because it needs package access twice:
 * {@code BoardFileDetails.filename} and {@code BoardFileDetails.directoryPath} are
 * {@code protected} (BoardFileDetails.java:47, :51). {@code changeFileExtension} is
 * {@code private} (RoutingJob.java:352), so that one still needs reflection;
 * {@code tryToSetInput} and {@code setInputFromFile} are driven through their public
 * {@code setInput} overloads rather than reflectively, because that is how the CLI reaches them.
 *
 * <h2>Eight tables</h2>
 *
 * <pre>
 *   SNIFF  &lt;idx&gt; &lt;hex&gt;            getFileFormat(byte[])   :151-227
 *   EXT    &lt;idx&gt; &lt;quoted path&gt;    getFileFormat(Path)    :230-247
 *   CFE    &lt;idx&gt; &lt;path&gt; &lt;ext&gt;     changeFileExtension    :352-374   (reflective)
 *   SETFN  &lt;idx&gt; &lt;preset&gt; &lt;name&gt;  BoardFileDetails.setFilename       :149-197
 *   CRC    &lt;idx&gt; &lt;source&gt;         BoardFileDetails.calculateCrc32    :75-87
 *   TSI    &lt;idx&gt; &lt;hex&gt;            setInput(byte[]) -&gt; tryToSetInput  :271-275, :335-349
 *   TSOF   &lt;idx&gt; &lt;path&gt;           tryToSetOutputFile     :377-397
 *   SIF    &lt;idx&gt; &lt;file&gt;           setInput(File) -&gt; setInputFromFile :425-461
 * </pre>
 *
 * <h2>The two rows Java cannot answer, and how they are printed</h2>
 *
 * <p>Two of the transcribed behaviours crash or hang in Java, so the port totalises them and the
 * row is an {@code XDIFF} carrying BOTH answers:
 *
 * <ul>
 *   <li><b>{@code SNIFF}, the shift loop</b> ({@code RoutingJob.java:181-187}, plan quirk label
 *       O). The loop never refills {@code buffer[5]}, so once the first six bytes are all
 *       {@code 0x0A}/{@code 0x0D} it spins forever. Every {@code SNIFF} row therefore runs on a
 *       <b>daemon thread with a 5 s join</b>; a row that does not finish prints
 *       {@code XDIFF java=HANG(&gt;5000ms) rust=UNKNOWN}. The {@code rust=} half is a literal
 *       transcribed here from the port, so the Rust twin printing anything else is a diff.
 *   <li><b>{@code CFE} and {@code SETFN}, the null parent</b> ({@code RoutingJob.java:356},
 *       {@code BoardFileDetails.java:160}, plan quirk labels Q and R).
 *       {@code Path.of("out.ses").getParent()} is {@code null}, so a bare filename NPEs. The row
 *       prints {@code XDIFF java=NullPointerException rust=&lt;the port's answer&gt;}.
 * </ul>
 *
 * <p>Every other row prints the value itself and must MATCH byte for byte.
 *
 * <h2>Path normalisation</h2>
 *
 * <p>Three absolute prefixes are replaced before printing, so the transcript is portable and the
 * Rust twin (which runs from the same working directory) and the Rust test (which does not) both
 * reproduce it: the scratch directory becomes {@code <SCRATCH>}, the Java clone's
 * {@code fixtures/} becomes {@code <FIXTURES>}, and the process working directory becomes
 * {@code <CWD>}. In that order — the scratch directory lives underneath the working directory.
 * The one value that is neither is the working directory's own final component, which
 * {@code setFilename("")} produces; it prints as {@code <CWD-BASENAME>}.
 *
 * <p>Run (this is exactly what {@code scripts/differential/run.sh p8t1probe} does):
 *
 * <pre>
 *   javac -cp &lt;HEAD jar&gt; -d &lt;out&gt; scripts/differential/java/probes/P8T1Probe.java
 *   java -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
 *        -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 \
 *        -cp &lt;out&gt;:&lt;HEAD jar&gt; app.freerouting.core.P8T1Probe &lt;fixtures dir&gt; &lt;scratch dir&gt;
 * </pre>
 *
 * <p>The driver name is {@code p8t1probe} rather than {@code p8t1}: the plan reserves
 * {@code p8t1} for Task 6's end-to-end SES-byte gate.
 */
public final class P8T1Probe {

  /** How long a {@code SNIFF} row is given before it is declared a hang. */
  private static final long SNIFF_TIMEOUT_MS = 5000;

  /**
   * The port's answer on the rows Java hangs on. Transcribed from
   * {@code fr_core::FileFormat::sniff_bytes}, whose {@code // totalized:} marker bounds the shift
   * loop at five iterations — the most that can move a byte the buffer actually holds into
   * {@code buffer[0]}. After five shifts every slot holds the original {@code buffer[5]}, which on
   * a hanging input is CR or LF, so no magic matches and the answer is {@code UNKNOWN}.
   */
  private static final String RUST_ANSWER_ON_HANG = "UNKNOWN";

  private P8T1Probe() {}

  // ── Table 1: getFileFormat(byte[]) ─────────────────────────────────────────────────────────
  //
  // Each entry is `s:<ascii>` or `h:<hex>`. What each probes is on the line.
  private static final String[] SNIFF_INPUTS = {
    "h:", //                                     empty: read() answers -1, bytesRead != 6
    "s:(", //                                    1 byte
    "s:(pcb", //                                 4 bytes — "(pcb" alone is NOT enough
    "s:(pcb ", //                                5 bytes — still not enough
    "s:(pcb x", //                               DSN
    "s:(PCB x", //                               DSN, the upper-case branch
    "s:(Pcb x", //                               neither branch is case-insensitive per character
    "s:(ses x", //                               SES
    "s:(SES x", //                               SES, the upper-case branch
    "s:(Ses x", //                               again, no mixed case
    "s:(rules", //                               RULES
    "s:(RULES", //                               RULES
    "s:(RuLes", //                               RULES — this branch IS per-character case-folded
    "s:(rUlEs", //                               RULES, the other mixture
    "s:(rulx  ", //                              RULES: only four bytes are ever compared
    "h:aced000500 00", //                        (spaces are stripped) 6 bytes, the FRB magic
    "h:aced0005", //                             4 bytes of FRB magic — not enough
    "h:aced000600 00", //                        wrong 4th byte
    "h:aced00057b7b", //                         FRB magic with '{' at index 4: FRB still wins
    "s:{", //                                    the JSON pre-check fires on ONE byte
    "s:   {", //                                 leading spaces are skipped by the pre-check
    "h:09 0d0a 20 7b", //                        tab CR LF space '{'
    "s:\n\n\n\n\n\n{", //                        six newlines then '{' — the PRE-CHECK wins, no hang
    "s:\n\n\n\n\n\n   {", //                     six newlines, spaces, '{' — pre-check again
    "s:[1,2]", //                                '[' breaks the pre-check
    "h:efbbbf7b", //                             UTF-8 BOM then '{' — the BOM defeats the pre-check
    "h:efbbbf28706362 20", //                    UTF-8 BOM then "(pcb " — the BOM defeats the sniff
    "h:fffe2800700 0", //                        UTF-16LE BOM
    "s:\n(pcb ", //                              one leading LF: one shift, then DSN
    "s:\r\n(pcb", //                             CR LF: two shifts, then DSN
    "s:\n\n(pcb  ", //                           two leading LFs: DSN
    "s:\n\n\n(pcb ", //                          three: buffer[5] floods slot 3 — NOT DSN
    "s:\n\n\n\n(pcb", //                         four: floods slots 2..5
    "s:\n\n\n\n\n(pcb", //                       five: every slot becomes '('
    "s:\n\n\n\n\n", //                           five bytes of LF — bytesRead != 6
    "s:\n\n\n\n\n\n(pcb", //                     SIX leading LFs, then content: HANGS
    "h:0d0a0d0a0d0a", //                         exactly six CR/LF bytes, nothing else: HANGS
    "h:0d0d0d0d0d0d58", //                       six CRs then 'X': HANGS
    "s: (pcb ", //                               a leading SPACE is not stripped by the shift loop
    "h:09 28706362 20", //                       a leading TAB likewise
    "s:(pcb\n\n", //                             trailing newlines are irrelevant
    "s:(sesx ", //                               SES: only four bytes are compared
    "s:(net (", //                               nothing matches
    "h:000000000000", //                         six zero bytes
    "s:(Rules", //                               RULES
    "s:(RuLeS", //                               RULES
  };

  // ── Table 2: getFileFormat(Path) ───────────────────────────────────────────────────────────
  private static final String[] EXT_INPUTS = {
    "board.dsn",
    "board.DSN",
    "board.frb",
    "board.ses",
    "board.rules",
    "board.scr",
    "board.json",
    "board.JSON",
    "board.txt",
    "board.drc", //     DRC_JSON has no extension of its own in the switch
    "board", //         no dot at all
    "board.", //        String.split drops the trailing empty run
    ".dsn", //          a leading empty part IS kept
    "a.b.dsn",
    "board.ses.", //    ditto — this is SES, not UNKNOWN
    "board..dsn",
    "dir.dsn/board", // the whole PATH is split, not the file name
    "dir.dsn/board.ses",
    "/tmp/x.DsN",
    "board.scr ", //    the trailing space is part of the extension
    "", //              Path.of("") is the empty path
    "..", //            every part is empty: the array is length 0
    "x.SCR",
    "x.Rules",
  };

  // ── Table 3: changeFileExtension ───────────────────────────────────────────────────────────
  private static final String[][] CFE_INPUTS = {
    {"out.ses", "ses"}, //          bare name, extension already matches -> NPE anyway (:356 first)
    {"out.dsn", "ses"}, //          bare name -> NPE
    {"out", "ses"}, //              bare name, no extension -> NPE
    {"", "ses"}, //                 the empty path -> NPE
    {"/", "ses"}, //                the root -> NPE
    {"dir/out.dsn", "ses"}, //      relative: the answer is ABSOLUTE
    {"dir/out.ses", "ses"}, //      relative + matching: the answer stays RELATIVE (the asymmetry)
    {"./out.dsn", "ses"}, //        "." is not normalised away
    {"/tmp/out.dsn", "ses"},
    {"/tmp/out.SES", "ses"}, //     the extension is lower-cased before comparison...
    {"/tmp/out.DSN", "ses"}, //     ...but its ORIGINAL length is what gets cut
    {"/tmp/out.ses", "SES"}, //     the new extension is NOT lower-cased
    {"/tmp/out.dsn", "dsn"}, //     absolute + matching
    {"/tmp/archive.tar.gz", "ses"},
    {"/tmp/out.", "ses"}, //        "out." splits to one part -> the no-extension arm
    {"/tmp/.hidden", "ses"}, //     ".hidden" splits to two parts -> ".ses"
    {"/a.b/out", "ses"}, //         the dot is in the DIRECTORY
    {"/tmp/dir.d/", "ses"}, //      Path.of drops the trailing separator
    {"/board.dsn", "ses"}, //       the parent IS the root
  };

  // ── Table 4: BoardFileDetails.setFilename ──────────────────────────────────────────────────
  //
  // `preset` is the value of `this.format` before the call ("-" for the default UNKNOWN), because
  // :174 only re-sniffs when the format is still UNKNOWN and :180-194 only appends an extension
  // when it is not.
  private static final String[][] SETFN_INPUTS = {
    {"-", "<null>"}, //                     the early return at :150-154
    {"-", "board.dsn"}, //                  no separator -> directoryPath is dropped entirely
    {"-", "board"}, //                      no separator, no extension
    {"-", "/tmp/board.dsn"},
    {"-", "/tmp/board"},
    {"-", "/tmp/board.DSN"}, //             getFileFormat lower-cases
    {"-", "/tmp/"}, //                      the trailing-slash strip eats the root
    {"-", "/board.dsn"}, //                 a file AT the root loses its directory entirely
    {"-", "/"}, //                          getParent() is null -> NPE
    {"-", "dir/board.ses"}, //              relative WITH a separator -> absolutised
    {"-", "/tmp/dir\\x/board.dsn"}, //      quirk R: `\\.$` eats "\x"
    {"-", "/tmp/a\\.\\b/board.dsn"}, //     quirk R: both Windows rewrites fire
    {"-", "/tmp/dir\\/board.dsn"}, //       the trailing "\" is stripped by the [/\\]+$ rule first
    {"-", "/tmp/b.dsn/x"}, //               the dot is in the directory
    {"-", ""}, //                           Path.of("").toAbsolutePath() is the working directory
    {"SES", "/tmp/board"}, //               :180-194 appends the default extension
    {"DSN", "/tmp/board"},
    {"FRB", "/tmp/board"},
    {"RULES", "/tmp/board"},
    {"SCR", "/tmp/board"},
    {"DRC_JSON", "/tmp/board"}, //          the switch default is "" -> nothing is appended
    {"KICAD_SESSION_JSON", "/tmp/board"}, // likewise
    {"DSN", "/tmp/board.txt"}, //           already contains "." -> nothing is appended
    {"SES", "board"}, //                    no separator AND a default extension
  };

  // ── Table 5: calculateCrc32 ────────────────────────────────────────────────────────────────
  private static final String[] CRC_LITERALS = {
    "h:", //                     the empty stream
    "s:a",
    "s:123456789", //            the standard CRC-32 check vector
    "s:The quick brown fox jumps over the lazy dog",
    "z:8191", //                 one byte short of the 8192-byte read buffer
    "z:8192", //                 exactly the buffer
    "z:8193", //                 one byte past it: two reads
  };

  /** Corpus files the CRC table covers, resolved under the fixtures directory given in argv[0]. */
  private static final String[] CRC_FILES = {
    "empty_board.dsn",
    "Issue026-J2_reference.dsn",
    "Issue026-J2_reference.ses",
    "Issue029-hw48na.rules",
    "Issue034-Green14SegLED.dsn",
    "Issue107-freq_teiler_200kHz_kicad.dsn",
  };

  // ── Table 6: setInput(byte[]) -> tryToSetInput ─────────────────────────────────────────────
  private static final String[] TSI_INPUTS = {
    "n:", //             null — the :336-338 guard
    "h:", //             empty
    "s:(pcb X)",
    "s:(ses X)",
    "s:(rules X)",
    "h:aced000500 05",
    "s:{\"a\":1}",
    "s:hello!", //       UNKNOWN -> false, and setData is NEVER called
  };

  // ── Table 7: tryToSetOutputFile ────────────────────────────────────────────────────────────
  //
  // Relative to the scratch directory unless the entry starts with "!", which means "the literal
  // string, unresolved" (so `null` and a bare name are reachable).
  private static final String[] TSOF_INPUTS = {
    "!<null>",
    "out.ses",
    "out.dsn",
    "out.frb",
    "out.scr",
    "out.json", //       :391 rewrites KICAD_DESIGN_JSON to KICAD_SESSION_JSON
    "out.rules", //      RULES is NOT in the accepted set -> false
    "out.txt", //        quirk label L's `-do out.txt`
    "out", //            no extension -> false
    "a.dsn", //          a file that EXISTS: the ctor reads it and re-sniffs the bytes
    "!out.ses", //       a bare relative name
  };

  // ── Table 8: setInput(File) -> setInputFromFile ────────────────────────────────────────────
  //
  // {scratch file name, content spec}. The content spec is written before the call.
  private static final String[][] SIF_FILES = {
    {"a.dsn", "s:(pcb A)\n"}, //     content and extension agree
    {"b.frb", "h:aced000500 05"}, // the FRB branch
    {"c.json", "s:{\"a\":1}\n"}, //  the KiCad branch — whose output name EQUALS the input name
    {"d.txt", "s:(pcb D)\n"}, //     content wins: a .txt input still derives a .ses output
    {"e.dsn", "s:hello!\n"}, //      extension wins, and size/crc32 stay 0 (setData never ran)
    {"f.txt", "s:hello!\n"}, //      neither: no output, and `name` keeps its id-derived default
    {"g.ses", "s:(ses G)\n"}, //     SES: `name` is set but NO output is derived
    {"h.missing", "!"}, //           the file is not created: setInput throws
  };

  private static String cwd;
  private static String cwdBasename;
  private static String fixturesDir;
  private static String scratchDir;

  /** Prints the eight tables. */
  public static void main(String[] args) throws Exception {
    fixturesDir = new File(args[0]).getAbsolutePath();
    scratchDir = new File(args[1]).getAbsolutePath();
    cwd = System.getProperty("user.dir");
    cwdBasename = Path.of(cwd).getFileName().toString();
    Files.createDirectories(Path.of(scratchDir));

    System.out.println("SNIFF-ROWS\t" + SNIFF_INPUTS.length);
    System.out.println("EXT-ROWS\t" + EXT_INPUTS.length);
    System.out.println("CFE-ROWS\t" + CFE_INPUTS.length);
    System.out.println("SETFN-ROWS\t" + SETFN_INPUTS.length);
    System.out.println("CRC-ROWS\t" + (CRC_LITERALS.length + CRC_FILES.length));
    System.out.println("TSI-ROWS\t" + TSI_INPUTS.length);
    System.out.println("TSOF-ROWS\t" + TSOF_INPUTS.length);
    System.out.println("SIF-ROWS\t" + SIF_FILES.length);

    sniffTable();
    extTable();
    cfeTable();
    setFilenameTable();
    crcTable();
    tsiTable();
    tsofTable();
    sifTable();
  }

  // ───────────────────────────────────────────────────────────────────────────────────────────

  private static void sniffTable() {
    for (int i = 0; i < SNIFF_INPUTS.length; i++) {
      byte[] content = decode(SNIFF_INPUTS[i]);
      String answer = sniffWithWatchdog(content);
      System.out.println("SNIFF\t" + i + "\t" + hex(content) + "\t" + answer);
    }
  }

  /**
   * Runs {@code getFileFormat(byte[])} on a daemon thread and gives it {@link #SNIFF_TIMEOUT_MS}.
   * A row that does not finish is the shift loop of {@code :181-187} spinning forever.
   */
  private static String sniffWithWatchdog(byte[] content) {
    final FileFormat[] slot = new FileFormat[1];
    final Throwable[] thrown = new Throwable[1];
    Thread t =
        new Thread(
            () -> {
              try {
                slot[0] = RoutingJob.getFileFormat(content);
              } catch (Throwable ex) {
                thrown[0] = ex;
              }
            });
    t.setDaemon(true);
    t.start();
    try {
      t.join(SNIFF_TIMEOUT_MS);
    } catch (InterruptedException _) {
      Thread.currentThread().interrupt();
    }
    if (t.isAlive()) {
      return "XDIFF\tjava=HANG(>" + SNIFF_TIMEOUT_MS + "ms)\trust=" + RUST_ANSWER_ON_HANG;
    }
    if (thrown[0] != null) {
      return "<" + thrown[0].getClass().getSimpleName() + ">";
    }
    return slot[0].name();
  }

  private static void extTable() {
    for (int i = 0; i < EXT_INPUTS.length; i++) {
      String raw = EXT_INPUTS[i];
      String answer;
      try {
        answer = RoutingJob.getFileFormat(Path.of(raw)).name();
      } catch (Throwable ex) {
        answer = "<" + ex.getClass().getSimpleName() + ">";
      }
      System.out.println("EXT\t" + i + "\t" + quote(raw) + "\t" + answer);
    }
  }

  private static void cfeTable() throws Exception {
    Method m = RoutingJob.class.getDeclaredMethod("changeFileExtension", String.class, String.class);
    m.setAccessible(true);
    RoutingJob job = new RoutingJob();
    for (int i = 0; i < CFE_INPUTS.length; i++) {
      String path = CFE_INPUTS[i][0];
      String ext = CFE_INPUTS[i][1];
      String answer;
      try {
        answer = norm((String) m.invoke(job, path, ext));
      } catch (java.lang.reflect.InvocationTargetException ite) {
        answer =
            "XDIFF\tjava="
                + ite.getCause().getClass().getSimpleName()
                + "\trust="
                + rustChangeFileExtension(path, ext);
      }
      System.out.println("CFE\t" + i + "\t" + quote(path) + "\t" + quote(ext) + "\t" + answer);
    }
  }

  /**
   * The port's totalisation of {@code changeFileExtension} on the inputs Java NPEs on, transcribed
   * from {@code fr_core::RoutingJob::change_file_extension}'s {@code // totalized:} marker: a
   * {@code null} parent (Java) is an empty parent (Rust), and {@code Path.of("", name)} is
   * {@code name}, so the answer is the bare name with the new extension — or, when the extension
   * already matches, the input unchanged.
   */
  private static String rustChangeFileExtension(String filename, String ext) {
    Path p = Path.of(filename);
    Path nameOnly = p.getFileName();
    // `Path.of("/")` has no file name; the port totalises that null to the empty string too and
    // carries on, so the answer is `"" + "." + ext`.
    String originalFilename = nameOnly == null ? "" : nameOnly.toString();
    String[] parts = originalFilename.split("\\.");
    if (parts.length > 1) {
      String extension = parts[parts.length - 1].toLowerCase();
      if (extension.equals(ext)) {
        return norm(p.toString());
      }
      return originalFilename.substring(0, originalFilename.length() - extension.length() - 1)
          + "."
          + ext;
    }
    return originalFilename + "." + ext;
  }

  private static void setFilenameTable() {
    for (int i = 0; i < SETFN_INPUTS.length; i++) {
      String preset = SETFN_INPUTS[i][0];
      String raw = SETFN_INPUTS[i][1];
      String arg = "<null>".equals(raw) ? null : raw;
      BoardFileDetails d = new BoardFileDetails();
      if (!"-".equals(preset)) {
        d.format = FileFormat.valueOf(preset);
      }
      String answer;
      try {
        d.setFilename(arg);
        answer =
            "dir="
                + quote(norm(d.getDirectoryPath()))
                + "\tname="
                + quote(norm(d.getFilename()))
                + "\tformat="
                + d.format.name()
                + "\tabs="
                + quote(norm(d.getAbsolutePath()))
                + "\tstem="
                + quote(norm(d.getFilenameWithoutExtension()));
      } catch (Throwable ex) {
        // `Path.of("/")` has neither a parent nor a file name; the port answers with two empty
        // strings, which is what `<null> -> ""` gives at both sites.
        answer =
            "XDIFF\tjava="
                + ex.getClass().getSimpleName()
                + "\trust=dir=\"\"\tname=\"\"\tformat=UNKNOWN\tabs=\"\"\tstem=\"\"";
      }
      System.out.println("SETFN\t" + i + "\t" + preset + "\t" + quote(raw) + "\t" + answer);
    }
  }

  private static void crcTable() throws Exception {
    int idx = 0;
    for (String spec : CRC_LITERALS) {
      byte[] data = decode(spec);
      CRC32 crc;
      try (InputStream in = new java.io.ByteArrayInputStream(data)) {
        crc = BoardFileDetails.calculateCrc32(in);
      }
      System.out.println(
          "CRC\t" + idx + "\tbytes\t" + hex(data) + "\tlen=" + data.length + "\t" + crc.getValue());
      idx++;
    }
    for (String name : CRC_FILES) {
      File f = new File(fixturesDir, name);
      CRC32 crc;
      try (InputStream in = new FileInputStream(f)) {
        crc = BoardFileDetails.calculateCrc32(in);
      }
      System.out.println(
          "CRC\t" + idx + "\tfile\t" + name + "\tlen=" + f.length() + "\t" + crc.getValue());
      idx++;
    }
  }

  private static void tsiTable() {
    for (int i = 0; i < TSI_INPUTS.length; i++) {
      byte[] content = TSI_INPUTS[i].startsWith("n:") ? null : decode(TSI_INPUTS[i]);
      RoutingJob job = new RoutingJob();
      boolean ok = job.setInput(content);
      System.out.println(
          "TSI\t"
              + i
              + "\t"
              + (content == null ? "<null>" : hex(content))
              + "\tok="
              + ok
              + "\tformat="
              + job.input.format.name()
              + "\tsize="
              + job.input.size
              + "\tcrc32="
              + job.input.crc32);
    }
  }

  private static void tsofTable() throws Exception {
    writeScratchFiles();
    for (int i = 0; i < TSOF_INPUTS.length; i++) {
      String spec = TSOF_INPUTS[i];
      File f;
      if ("!<null>".equals(spec)) {
        f = null;
      } else if (spec.startsWith("!")) {
        f = new File(spec.substring(1));
      } else {
        f = new File(scratchDir, spec);
      }
      RoutingJob job = new RoutingJob();
      boolean ok = job.tryToSetOutputFile(f);
      String out =
          job.output == null
              ? "output=<null>"
              : "format="
                  + job.output.format.name()
                  + "\tdir="
                  + quote(norm(job.output.getDirectoryPath()))
                  + "\tname="
                  + quote(norm(job.output.getFilename()))
                  + "\tsize="
                  + job.output.size
                  + "\tcrc32="
                  + job.output.crc32;
      System.out.println("TSOF\t" + i + "\t" + quote(spec) + "\tok=" + ok + "\t" + out);
    }
  }

  private static void sifTable() throws Exception {
    writeScratchFiles();
    for (int i = 0; i < SIF_FILES.length; i++) {
      String name = SIF_FILES[i][0];
      File f = new File(scratchDir, name);
      RoutingJob job = new RoutingJob();
      String idDerivedName = "J-" + job.id.toString().substring(0, 6).toUpperCase();
      String answer;
      try {
        job.setInput(f);
        answer =
            "in.format="
                + job.input.format.name()
                + "\tin.dir="
                + quote(norm(job.input.getDirectoryPath()))
                + "\tin.name="
                + quote(norm(job.input.getFilename()))
                + "\tin.size="
                + job.input.size
                + "\tin.crc32="
                + job.input.crc32
                + "\t"
                + (job.output == null
                    ? "out=<null>"
                    : "out.format="
                        + job.output.format.name()
                        + "\tout.dir="
                        + quote(norm(job.output.getDirectoryPath()))
                        + "\tout.name="
                        + quote(norm(job.output.getFilename())))
                + "\tjob.name="
                + quote(job.name.equals(idDerivedName) ? "<ID-DERIVED>" : norm(job.name));
      } catch (Throwable ex) {
        answer =
            "throws="
                + ex.getClass().getSimpleName()
                + "\tinput="
                + (job.input == null ? "<null>" : "<set>");
      }
      System.out.println("SIF\t" + i + "\t" + name + "\t" + answer);
    }
  }

  /** Writes the {@link #SIF_FILES} content into the scratch directory (idempotently). */
  private static void writeScratchFiles() throws Exception {
    for (String[] spec : SIF_FILES) {
      if ("!".equals(spec[1])) {
        Files.deleteIfExists(Path.of(scratchDir, spec[0]));
        continue;
      }
      Files.write(Path.of(scratchDir, spec[0]), decode(spec[1]));
    }
  }

  // ── helpers ────────────────────────────────────────────────────────────────────────────────

  /** `s:<ascii>` (with `\n`/`\r`/`\t` escapes), `h:<hex, spaces ignored>` or `z:<count>` zeros. */
  private static byte[] decode(String spec) {
    String body = spec.substring(2);
    switch (spec.charAt(0)) {
      case 's':
        {
          StringBuilder sb = new StringBuilder();
          for (int i = 0; i < body.length(); i++) {
            char c = body.charAt(i);
            if (c == '\\' && i + 1 < body.length()) {
              char n = body.charAt(++i);
              sb.append(n == 'n' ? '\n' : n == 'r' ? '\r' : n == 't' ? '\t' : n);
            } else {
              sb.append(c);
            }
          }
          return sb.toString().getBytes(java.nio.charset.StandardCharsets.ISO_8859_1);
        }
      case 'h':
        {
          String clean = body.replace(" ", "");
          byte[] out = new byte[clean.length() / 2];
          for (int i = 0; i < out.length; i++) {
            out[i] = (byte) Integer.parseInt(clean.substring(i * 2, i * 2 + 2), 16);
          }
          return out;
        }
      case 'z':
        return new byte[Integer.parseInt(body)];
      default:
        throw new IllegalArgumentException(spec);
    }
  }

  /** Lower-case hex, or `-` for an empty array. Truncated past 64 bytes so the rows stay short. */
  private static String hex(byte[] b) {
    if (b.length == 0) {
      return "-";
    }
    StringBuilder sb = new StringBuilder();
    int n = Math.min(b.length, 64);
    for (int i = 0; i < n; i++) {
      sb.append(String.format("%02x", b[i]));
    }
    if (b.length > n) {
      sb.append("+").append(b.length - n);
    }
    return sb.toString();
  }

  /** Replaces the three absolute prefixes, longest-rooted first. */
  private static String norm(String s) {
    if (s == null) {
      return "<null>";
    }
    s = s.replace(scratchDir, "<SCRATCH>");
    s = s.replace(fixturesDir, "<FIXTURES>");
    s = s.replace(cwd, "<CWD>");
    if (s.equals(cwdBasename)) {
      s = "<CWD-BASENAME>";
    }
    return s;
  }

  /** Renders a string unambiguously, so a trailing space or a backslash is visible. */
  private static String quote(String s) {
    if (s == null) {
      return "<null>";
    }
    StringBuilder sb = new StringBuilder("\"");
    for (int i = 0; i < s.length(); i++) {
      char c = s.charAt(i);
      if (c == '"' || c == '\\') {
        sb.append('\\').append(c);
      } else if (c < 0x20 || c > 0x7e) {
        sb.append(String.format("\\u%04x", (int) c));
      } else {
        sb.append(c);
      }
    }
    return sb.append('"').toString();
  }
}
