package app.freerouting.core.scoring;

import app.freerouting.io.FileFormat;
import java.awt.geom.Rectangle2D;
import java.io.IOException;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

/**
 * Plan 8 Task 2's JVM-pinned evidence: the <b>text-scraping</b> {@code BoardStatistics(byte[],
 * FileFormat)} constructor (BoardStatistics.java:436-552), its private helper {@code
 * countOccurrences} (:578-586), and the Gson JSON surface — {@code toString} (:589-591) is
 * {@code GsonProvider.GSON.toJson(this)}.
 *
 * <p>Declares {@code package app.freerouting.core.scoring} because {@code countOccurrences} is
 * {@code private static} (:578) and is reached by reflection; the package declaration is
 * convention, matching {@code P8T1Probe}, and keeps the reflective lookup beside the class it
 * drives.
 *
 * <h2>Five tables</h2>
 *
 * <pre>
 *   HDR    &lt;50 field paths&gt;              the FLD column order, declaration order (:37-79)
 *   COUNT  &lt;idx&gt; &lt;haystack&gt; &lt;needle&gt; &lt;n&gt;  countOccurrences                       :578-586
 *   BS     &lt;idx&gt; &lt;label&gt; &lt;format&gt; &lt;src&gt;   the constructor's input                :436-552
 *   FLD    &lt;idx&gt; &lt;50 values&gt;              every DTO field of the result
 *   JSON   &lt;idx&gt; &lt;escaped toString()&gt;     the byte-exact Gson output            :589-591
 * </pre>
 *
 * <p>A {@code BS} row's {@code src} column is one of {@code null} (a null {@code byte[]}),
 * {@code java:&lt;relative path&gt;} (a file under the Java checkout), {@code ref:&lt;relative
 * path&gt;} (a file under this repository's {@code tests/reference/}), {@code hex:&lt;hex&gt;} (a
 * synthetic input the probe builds itself, so the Rust twin can rebuild exactly the same bytes) or
 * {@code synth:&lt;name&gt;} (a {@code BoardStatistics} assembled field by field rather than
 * scraped, which is how the Gson surface is pinned on the fields the scraper never writes).
 *
 * <h2>Escaping</h2>
 *
 * <p>Every value that can hold a tab, a newline or a backslash — the two string fields and the
 * whole pretty-printed JSON — is printed with {@code \\} for a backslash, {@code \t} for a tab,
 * {@code \r} for a carriage return and {@code \n} for a newline, so one transcript row is one
 * line. A {@code null} field prints as {@code <null>}; a {@code null} nested object prints
 * {@code <null>} in each of its own four columns.
 *
 * <h2>The two rows Java cannot answer the way the port does</h2>
 *
 * <ul>
 *   <li><b>{@code (parser (hostCad))}</b> — {@code :492} computes {@code parserScope.substring(
 *       hcIdx + 9, hcEnd)} with {@code hcIdx + 9 &gt; hcEnd}, so Java throws
 *       {@code StringIndexOutOfBoundsException} out of the constructor. The row prints
 *       {@code XDIFF java=StringIndexOutOfBoundsException rust=host=&lt;omitted&gt;}.
 *   <li><b>{@code (parser (hostCad  ))}</b> — the scrape succeeds with an <i>empty</i>
 *       {@code hostCad}, so Java's {@code host} is {@code ""} where a failed scrape leaves it
 *       {@code null}; Gson prints the first and omits the second. The port's
 *       {@code BoardStatistics.host} is a {@code String}, not an {@code Option&lt;String&gt;}
 *       (the type is {@code fr_router::score}'s, and plan 8 allows only additive changes to that
 *       crate), so it spells Java's {@code null} as {@code ""} and omits both. The {@code JSON}
 *       row prints {@code XDIFF java=… rust=…} with both answers.
 * </ul>
 *
 * <p>Both {@code rust=} halves are literals transcribed from the port, so the Rust twin printing
 * anything else is still a diff.
 *
 * <p>Run (this is exactly what {@code scripts/differential/run.sh p8t2probe} does):
 *
 * <pre>
 *   javac -cp &lt;HEAD jar&gt; -d &lt;out&gt; scripts/differential/java/probes/P8T2Probe.java
 *   java -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
 *        -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 \
 *        -cp &lt;out&gt;:&lt;HEAD jar&gt; app.freerouting.core.scoring.P8T2Probe &lt;java dir&gt; &lt;repo root&gt;
 * </pre>
 */
public final class P8T2Probe {

  /** The FLD column order: `BoardStatistics.java:37-79`, then each DTO's own declaration order. */
  private static final String[] FIELD_PATHS = {
    "host",
    "unit",
    "board.bounding_box.x",
    "board.bounding_box.y",
    "board.bounding_box.width",
    "board.bounding_box.height",
    "board.size.x",
    "board.size.y",
    "board.size.width",
    "board.size.height",
    "layers.total_count",
    "layers.signal_count",
    "items.total_count",
    "items.trace_count",
    "items.via_count",
    "items.conduction_area_count",
    "items.drill_item_count",
    "items.pin_count",
    "items.component_count",
    "items.other_count",
    "components.total_count",
    "pads.total_count",
    "nets.total_count",
    "nets.class_count",
    "connections.maximum_count",
    "connections.incomplete_count",
    "traces.total_count",
    "traces.total_segment_count",
    "traces.total_length",
    "traces.total_length_mm",
    "traces.total_weighted_length",
    "traces.average_length",
    "traces.total_vertical_length",
    "traces.total_horizontal_length",
    "traces.total_angled_length",
    "bends.total_count",
    "bends.90_degree_count",
    "bends.45_degree_count",
    "bends.other_angle_count",
    "vias.total_count",
    "vias.through_hole_count",
    "vias.blind_count",
    "vias.buried_count",
    "clearance_violations.total_count",
    "clearance_violations.min_violation_um",
    "clearance_violations.max_violation_um",
    "clearance_violations.avg_violation_um",
    "fanout.total_smd_pins",
    "fanout.pins_to_escape",
    "fanout.escaped_count",
  };

  /** The seven DSN/SES reference stems of `tests/reference/fixtures.txt`, in file order. */
  private static final String[][] STEMS = {
    {"tutorial_board", "examples/tutorial_board/tutorial_board.dsn"},
    {"Issue026-J2_reference", "fixtures/Issue026-J2_reference.dsn"},
    {"Issue103-Board-Unrouted", "fixtures/Issue103-Board-Unrouted.dsn"},
    {"Issue143-rpi_splitter", "fixtures/Issue143-rpi_splitter.dsn"},
    {"Issue413-test", "fixtures/Issue413-test.dsn"},
    {"Issue110-RelayModule", "fixtures/Issue110-RelayModule.dsn"},
    {"Issue753-CPU-85_r104", "fixtures/Issue753-CPU-85_r104.dsn"},
  };

  /** The eight batch stems of `tests/reference/router-fixtures.txt`, in file order. */
  private static final String[] BATCH_STEMS = {
    "router-rpi-splitter",
    "router-dac2020-bm01",
    "router-j2-reference",
    "router-tutorial-board",
    "router-ecc83-input",
    "router-fanout-bm11",
    "router-strict-drc-cnh",
    "router-empty-board",
  };

  private static final StringBuilder OUT = new StringBuilder();
  private static int index = 0;

  public static void main(String[] args) throws Exception {
    // The KiCad-JSON branch's `catch` at `:548-549` calls `FRLogger.error`, and log4j's Console
    // appender targets SYSTEM_OUT (quirk label AI) — which is the stream this transcript is the
    // whole of. `Log4j2ConfigurationFactory:46,68` reads these two properties before it builds
    // the appender list, so setting them here (log4j initialises lazily, on the first
    // `LogManager.getLogger`, which is inside the constructor under test) leaves only the
    // SYSTEM_ERR appender. This is probe plumbing; it changes nothing the probe measures.
    System.setProperty("freerouting.logging.console.enabled", "false");
    System.setProperty("freerouting.logging.file.enabled", "false");

    Path javaDir = Path.of(args[0]);
    Path repoRoot = Path.of(args[1]);

    line("HDR\t" + String.join("\t", FIELD_PATHS));

    // ---- countOccurrences (:578-586) --------------------------------------------------------
    Method count =
        BoardStatistics.class.getDeclaredMethod("countOccurrences", String.class, String.class);
    count.setAccessible(true);
    String[][] countCases = {
      {"", "(net"},
      {"(net", "(net"},
      {"(network", "(net"},
      {"(net_class", "(net"},
      {"(net (net (net", "(net"},
      {"(layer_rule (layer TOP)", "(layer"},
      {"(via_rule (via V1)", "(via"},
      {"(class_class (class C)", "(class"},
      {"aaaa", "aa"},
      {"(wire(wire(wire", "(wire"},
      {"(component", "(componentx"},
      {"é(net", "(net"},
    };
    int ci = 0;
    for (String[] c : countCases) {
      int n = (int) count.invoke(null, c[0], c[1]);
      line("COUNT\t" + ci + "\t" + esc(c[0]) + "\t" + esc(c[1]) + "\t" + n);
      ci++;
    }

    // ---- the scraper over the committed corpus ----------------------------------------------
    for (String[] stem : STEMS) {
      emitFile(stem[0] + "/source.dsn", FileFormat.DSN, "java:" + stem[1],
          javaDir.resolve(stem[1]));
      emitFile(stem[0] + "/roundtrip.dsn", FileFormat.DSN, "ref:" + stem[0] + "/roundtrip.dsn",
          repoRoot.resolve("tests/reference/" + stem[0] + "/roundtrip.dsn"));
      emitFile(stem[0] + "/unrouted.ses", FileFormat.SES, "ref:" + stem[0] + "/unrouted.ses",
          repoRoot.resolve("tests/reference/" + stem[0] + "/unrouted.ses"));
    }
    for (String stem : BATCH_STEMS) {
      emitFile(stem + "/batch.ses", FileFormat.SES, "ref:" + stem + "/batch.ses",
          repoRoot.resolve("tests/reference/" + stem + "/batch.ses"));
    }

    // A DSN read as SES and a SES read as DSN: the constructor trusts its `format` argument and
    // never re-sniffs, so both branches run over the wrong grammar and still answer.
    emitFile("Issue143-rpi_splitter/source.dsn AS SES", FileFormat.SES,
        "java:fixtures/Issue143-rpi_splitter.dsn",
        javaDir.resolve("fixtures/Issue143-rpi_splitter.dsn"));
    emitFile("Issue143-rpi_splitter/unrouted.ses AS DSN", FileFormat.DSN,
        "ref:Issue143-rpi_splitter/unrouted.ses",
        repoRoot.resolve("tests/reference/Issue143-rpi_splitter/unrouted.ses"));
    // The ONE shape in this repository where the host scrape succeeds on a real, jar-written
    // file: `Parser.writeScope(..., reduced = true)` (Parser.java:98-107) skips `(stringQuote ")`,
    // so the first `)` after `(parser` closes `(hostCad …)` instead of truncating in front of it,
    // and HEAD's own keyword IS the camelCase one the scrape looks for (Keyword.java:40-41).
    // Reachable only by handing a `.ses` to the DSN branch, which no Freerouting code path does.
    // The 2.3.0-written `unrouted.ses` above is snake_case and scrapes nothing.
    emitFile("router-dac2020-bm01/batch.ses AS DSN", FileFormat.DSN,
        "ref:router-dac2020-bm01/batch.ses",
        repoRoot.resolve("tests/reference/router-dac2020-bm01/batch.ses"));

    // ---- the guards and the formats with no branch -------------------------------------------
    emitNull("null data", FileFormat.DSN);
    emitBytes("null format", "s:(pcb x", null);
    for (FileFormat f :
        new FileFormat[] {
          FileFormat.UNKNOWN,
          FileFormat.FRB,
          FileFormat.RULES,
          FileFormat.SCR,
          FileFormat.DRC_JSON,
          FileFormat.KICAD_SESSION_JSON,
        }) {
      emitBytes("no branch " + f.name(), "s:(pcb x", f);
    }
    emitBytes("empty SES", "s:", FileFormat.SES);
    emitBytes("empty DSN", "s:", FileFormat.DSN);
    emitBytes("empty KiCad JSON", "s:", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("binary junk SES", "h:00ff01fe0210(net", FileFormat.SES);
    emitBytes("binary junk DSN", "h:00ff01fe0210(layer", FileFormat.DSN);

    // ---- quirk F, the substring counting ------------------------------------------------------
    emitBytes(
        "quirk F DSN keywords",
        "s:(layer_rule)(layer TOP)(network X)(net_class Y)(net N)(via_rule R)(via V)"
            + "(class_class Z)(class C)(component U1)(wire)",
        FileFormat.DSN);
    emitBytes(
        "quirk F SES keywords",
        "s:(component U1)(net N)(network X)(net_class Y)(wire)(via V)(via_rule R)",
        FileFormat.SES);

    // ---- the SES layer scrape: split on `(path `, words[0] of every chunk after the first ------
    emitBytes("SES no path", "s:(session x)", FileFormat.SES);
    emitBytes("SES one path", "s:(wire (path F.Cu 250 1 2 3 4))", FileFormat.SES);
    emitBytes("SES two layers", "s:(path F.Cu 250 1 2)(path B.Cu 250 3 4)", FileFormat.SES);
    emitBytes("SES repeated layer", "s:(path F.Cu 250 1 2)(path F.Cu 250 3 4)", FileFormat.SES);
    emitBytes("SES leading path", "s:(path F.Cu 250 1 2)", FileFormat.SES);
    emitBytes("SES one-word chunk", "s:a(path F.Cu", FileFormat.SES);
    emitBytes("SES empty chunk", "s:a(path (path B.Cu 1 2", FileFormat.SES);
    emitBytes("SES trailing path", "s:a(path ", FileFormat.SES);

    // ---- quirk G, the DSN host scrape ----------------------------------------------------------
    emitBytes(
        "real DSN parser scope",
        "s:(pcb x\n  (parser\n    (string_quote \")\n    (host_cad \"KiCad's Pcbnew\")\n"
            + "    (host_version \"8.0.4\")\n  )\n)",
        FileFormat.DSN);
    emitBytes(
        "camelCase, both", "s:(parser (hostCad \"KiCad\" (hostVersion \"8.0\" ))", FileFormat.DSN);
    emitBytes("camelCase, cad only", "s:(parser (hostCad \"KiCad\" ))", FileFormat.DSN);
    emitBytes("camelCase, version only", "s:(parser (hostVersion \"8.0\" ))", FileFormat.DSN);
    emitBytes("camelCase, two spaces", "s:(parser (hostCad  \"KiCad\" ))", FileFormat.DSN);
    emitBytes("camelCase, no space", "s:(parser (hostCad\"KiCad\" ))", FileFormat.DSN);
    emitBytes("trim keeps NBSP", "s:(parser (hostCad K\u00a0 ))", FileFormat.DSN);
    emitBytes("trim drops the control char", "s:(parser (hostCad K\u0001 ))", FileFormat.DSN);
    emitBytes(
        "camelCase, unquoted", "s:(parser (hostCad KiCad (hostVersion 8.0 ))", FileFormat.DSN);
    emitBytes("no parser scope", "s:(pcb x (structure))", FileFormat.DSN);
    emitBytes("parser, no close", "s:(parser (hostCad \"K\" (hostVersion \"8\" ", FileFormat.DSN);
    emitBytes(
        "parser, no close, >1000",
        "s:(parser (hostCad \"K\" " + "x".repeat(1200) + " (hostVersion \"8\" ",
        FileFormat.DSN);
    emitBytes(
        "host is not unescaped",
        "s:(parser (hostCad \"K\\u0041D\" (hostVersion \"8\" ))",
        FileFormat.DSN);
    // The empty-hostCad arm: Java's `host` is "" and the port's is "" too, but Gson prints the
    // first and the port omits it. XDIFF on the JSON row only.
    emitBytes("empty hostCad", "s:(parser (hostCad  ))", FileFormat.DSN);
    // The inverted substring: `hcIdx + 9 > hcEnd`, so Java throws out of the constructor.
    emitBytes("inverted substring", "s:(parser (hostCad))", FileFormat.DSN);

    // ---- the KiCad design JSON branch ----------------------------------------------------------
    emitBytes(
        "kicad full",
        "s:{\"layers\":[1,2,3,4],\"components\":[{},{}],\"netClasses\":[{}],\"nets\":[1,2,3],"
            + "\"traces\":[],\"vias\":[{},{},{}],\"designName\":\"board\"}",
        FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad empty object", "s:{}", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad array", "s:[1,2]", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad null literal", "s:null", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad malformed", "s:{\"layers\":", FileFormat.KICAD_DESIGN_JSON);
    emitBytes(
        "kicad wrong type mid-way",
        "s:{\"layers\":[1,2],\"components\":7,\"nets\":[1]}",
        FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad numeric designName", "s:{\"designName\":42}", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad designName 1e5", "s:{\"designName\":1e5}", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad designName 1.50", "s:{\"designName\":1.50}", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad designName big integer",
        "s:{\"designName\":123456789012345678901234567890}", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad designName one-element array",
        "s:{\"designName\":[\"foo\"]}", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad designName nested one-element array",
        "s:{\"designName\":[[\"deep\"]]}", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad designName one-element numeric array",
        "s:{\"designName\":[1e5]}", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad designName two-element array",
        "s:{\"designName\":[\"a\",\"b\"]}", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad designName empty array",
        "s:{\"designName\":[]}", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad designName boolean",
        "s:{\"designName\":true}", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad designName null",
        "s:{\"designName\":null}", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad designName object",
        "s:{\"designName\":{\"a\":1}}", FileFormat.KICAD_DESIGN_JSON);
    emitBytes("kicad designName duplicated",
        "s:{\"designName\":\"first\",\"designName\":\"last\"}", FileFormat.KICAD_DESIGN_JSON);

    // ---- the Gson surface on the fields the scraper never writes --------------------------------
    emitSynth("empty", new BoardStatistics());
    emitSynth("populated", populated());
    emitSynth("fanout only", fanoutOnly());

    System.out.print(OUT);
    System.out.flush();
  }

  // ==============================================================================================
  // the emitters
  // ==============================================================================================

  private static void emitFile(String label, FileFormat format, String src, Path path)
      throws IOException {
    emit(label, format, src, Files.readAllBytes(path));
  }

  /** `spec` is `s:&lt;text&gt;` or `h:&lt;hex&gt;`; a null `format` drives the `:437-439` guard. */
  private static void emitBytes(String label, String spec, FileFormat format) {
    byte[] data = decode(spec);
    emit(label, format, src(data), data);
  }

  /**
   * `txt:&lt;escaped&gt;` when the bytes survive a UTF-8 round trip and hold no control character
   * other than tab/CR/LF, `hex:&lt;hex&gt;` otherwise. Both are exactly reversible, and the first
   * keeps the synthetic DSN and SES rows readable in the committed transcript.
   */
  private static String src(byte[] data) {
    String text = new String(data, StandardCharsets.UTF_8);
    if (!java.util.Arrays.equals(data, text.getBytes(StandardCharsets.UTF_8))) {
      return "hex:" + hex(data);
    }
    for (int i = 0; i < text.length(); i++) {
      char c = text.charAt(i);
      if (c < 0x20 && c != '\t' && c != '\r' && c != '\n') {
        return "hex:" + hex(data);
      }
    }
    return "txt:" + esc(text);
  }

  /** The null-`byte[]` arm of the `:437-439` guard. */
  private static void emitNull(String label, FileFormat format) {
    emit(label, format, "null", null);
  }

  private static void emit(String label, FileFormat format, String src, byte[] data) {
    int i = index++;
    line(
        "BS\t" + i + "\t" + label + "\t" + (format == null ? "<null>" : format.name()) + "\t" + src);
    BoardStatistics s;
    try {
      s = new BoardStatistics(data, format);
    } catch (RuntimeException ex) {
      // The port totalises the inverted `substring` range to an empty value, so `hostCad` is `""`
      // and `host` is `""` — which the port then omits from the JSON, exactly as it omits a
      // failed scrape's `null`.
      line("XDIFF\t" + i + "\tjava=" + ex.getClass().getSimpleName() + "\trust=host=<omitted>");
      return;
    }
    line("FLD\t" + i + "\t" + String.join("\t", fields(s)));
    if ("".equals(s.host)) {
      // Gson prints an empty string and omits a null; the port cannot tell the two apart, so it
      // answers `null` in the FLD column and omits the key in the JSON. Both halves of both rows
      // are printed on both sides, exactly as `P8T1Probe` prints its hang and its NPE.
      line("XDIFF\t" + i + "\tjava=host=\"\"\trust=host=<null>");
      line(
          "JSON\t" + i + "\tXDIFF\tjava=" + esc(s.toString()) + "\trust="
              + esc(withoutHost(s.toString())));
    } else {
      line("JSON\t" + i + "\t" + esc(s.toString()));
    }
  }

  private static void emitSynth(String label, BoardStatistics s) {
    int i = index++;
    line("BS\t" + i + "\t" + label + "\t<none>\tsynth:" + label.replace(' ', '-'));
    line("FLD\t" + i + "\t" + String.join("\t", fields(s)));
    line("JSON\t" + i + "\t" + esc(s.toString()));
  }

  /** Drops the `"host": "",` line Gson emits, which is what the port's omission produces. */
  private static String withoutHost(String json) {
    List<String> kept = new ArrayList<>();
    for (String l : json.split("\n", -1)) {
      if (!l.startsWith("  \"host\": ")) {
        kept.add(l);
      }
    }
    return String.join("\n", kept);
  }

  // ==============================================================================================
  // the field printer
  // ==============================================================================================

  private static String[] fields(BoardStatistics s) {
    List<String> v = new ArrayList<>();
    v.add(str(s.host));
    v.add(str(s.unit));
    rect(v, s.board == null ? null : s.board.boundingBox);
    rect(v, s.board == null ? null : s.board.size);
    v.add(num(s.layers.totalCount));
    v.add(num(s.layers.signalCount));
    v.add(num(s.items.totalCount));
    v.add(num(s.items.traceCount));
    v.add(num(s.items.viaCount));
    v.add(num(s.items.conductionAreaCount));
    v.add(num(s.items.drillItemCount));
    v.add(num(s.items.pinCount));
    v.add(num(s.items.componentOutlineCount));
    v.add(num(s.items.otherCount));
    v.add(num(s.components.totalCount));
    v.add(num(s.pads.totalCount));
    v.add(num(s.nets.totalCount));
    v.add(num(s.nets.classCount));
    v.add(num(s.connections.maximumCount));
    v.add(num(s.connections.incompleteCount));
    v.add(num(s.traces.totalCount));
    v.add(num(s.traces.totalSegmentCount));
    v.add(num(s.traces.totalLength));
    v.add(num(s.traces.totalLengthMm));
    v.add(num(s.traces.totalWeightedLength));
    v.add(num(s.traces.averageLength));
    v.add(num(s.traces.totalVerticalLength));
    v.add(num(s.traces.totalHorizontalLength));
    v.add(num(s.traces.totalAngledLength));
    v.add(num(s.bends.totalCount));
    v.add(num(s.bends.ninetyDegreeCount));
    v.add(num(s.bends.fortyFiveDegreeCount));
    v.add(num(s.bends.otherAngleCount));
    v.add(num(s.vias.totalCount));
    v.add(num(s.vias.throughHoleCount));
    v.add(num(s.vias.blindCount));
    v.add(num(s.vias.buriedCount));
    v.add(num(s.clearanceViolations.totalCount));
    v.add(num(s.clearanceViolations.minViolationUm));
    v.add(num(s.clearanceViolations.maxViolationUm));
    v.add(num(s.clearanceViolations.avgViolationUm));
    v.add(Integer.toString(s.fanout.totalSmdPins));
    v.add(Integer.toString(s.fanout.pinsToEscape));
    v.add(Integer.toString(s.fanout.escapedCount));
    if (v.size() != FIELD_PATHS.length) {
      throw new IllegalStateException("field count " + v.size() + " != " + FIELD_PATHS.length);
    }
    return v.toArray(new String[0]);
  }

  private static void rect(List<String> v, Rectangle2D.Float r) {
    if (r == null) {
      v.add("<null>");
      v.add("<null>");
      v.add("<null>");
      v.add("<null>");
    } else {
      v.add(Float.toString(r.x));
      v.add(Float.toString(r.y));
      v.add(Float.toString(r.width));
      v.add(Float.toString(r.height));
    }
  }

  private static String num(Number n) {
    return n == null ? "<null>" : n.toString();
  }

  private static String str(String s) {
    return s == null ? "<null>" : "\"" + esc(s) + "\"";
  }

  // ==============================================================================================
  // the two synthetic statistics
  // ==============================================================================================

  /** Every field non-null and distinct, so the JSON pins all fifty in declaration order. */
  private static BoardStatistics populated() {
    BoardStatistics s = new BoardStatistics();
    s.host = "KiCad's \"Pcbnew\",8.0.4 é";
    s.unit = "um";
    s.board.boundingBox = new Rectangle2D.Float(1.5f, -2.25f, -1000000.5f, 0.1f);
    s.board.size = new Rectangle2D.Float(0f, 0f, 1.0E8f, 3.0E-4f);
    s.layers.totalCount = 4;
    s.layers.signalCount = 2;
    s.items.totalCount = 11;
    s.items.traceCount = 12;
    s.items.viaCount = 13;
    s.items.conductionAreaCount = 14;
    s.items.drillItemCount = 15;
    s.items.pinCount = 16;
    s.items.componentOutlineCount = 17;
    s.items.otherCount = 18;
    s.components.totalCount = 19;
    s.pads.totalCount = 20;
    s.nets.totalCount = 21;
    s.nets.classCount = 22;
    s.connections.maximumCount = 23;
    s.connections.incompleteCount = 24;
    s.traces.totalCount = 25;
    s.traces.totalSegmentCount = 26;
    s.traces.totalLength = 0.1f;
    s.traces.totalLengthMm = 1.0E7f;
    s.traces.totalWeightedLength = 9.999999E-4f;
    s.traces.averageLength = -0.0f;
    s.traces.totalVerticalLength = 1234567.9f;
    s.traces.totalHorizontalLength = 3.4028235E38f;
    s.traces.totalAngledLength = 1.4E-45f;
    s.bends.totalCount = 27;
    s.bends.ninetyDegreeCount = 28;
    s.bends.fortyFiveDegreeCount = 29;
    s.bends.otherAngleCount = 30;
    s.vias.totalCount = 31;
    s.vias.throughHoleCount = 32;
    s.vias.blindCount = 33;
    s.vias.buriedCount = 34;
    s.clearanceViolations.totalCount = 35;
    s.clearanceViolations.minViolationUm = 0.1;
    s.clearanceViolations.maxViolationUm = 1.0E7;
    s.clearanceViolations.avgViolationUm = -9.999999999999999E-4;
    s.fanout.totalSmdPins = 36;
    s.fanout.pinsToEscape = 37;
    s.fanout.escapedCount = 38;
    return s;
  }

  /** The one DTO whose fields are primitive `int`: Gson emits `0`, it does not omit them. */
  private static BoardStatistics fanoutOnly() {
    BoardStatistics s = new BoardStatistics();
    s.fanout.totalSmdPins = 7;
    return s;
  }

  // ==============================================================================================
  // plumbing
  // ==============================================================================================

  private static void line(String s) {
    OUT.append(s).append('\n');
  }

  private static byte[] decode(String spec) {
    if (spec.startsWith("s:")) {
      return spec.substring(2).getBytes(StandardCharsets.UTF_8);
    }
    String h = spec.substring(2);
    // `h:` is hex up to the first non-hex character, then literal UTF-8 text — so a row can mix an
    // invalid byte sequence with a keyword without needing a second escape vocabulary.
    int n = 0;
    while (n < h.length() && Character.digit(h.charAt(n), 16) >= 0) {
      n++;
    }
    n -= n % 2;
    byte[] head = new byte[n / 2];
    for (int i = 0; i < head.length; i++) {
      head[i] = (byte) Integer.parseInt(h.substring(2 * i, 2 * i + 2), 16);
    }
    byte[] tail = h.substring(n).getBytes(StandardCharsets.UTF_8);
    byte[] all = new byte[head.length + tail.length];
    System.arraycopy(head, 0, all, 0, head.length);
    System.arraycopy(tail, 0, all, head.length, tail.length);
    return all;
  }

  private static String hex(byte[] data) {
    StringBuilder sb = new StringBuilder();
    for (byte b : data) {
      sb.append(String.format("%02x", b));
    }
    return sb.toString();
  }

  private static String esc(String s) {
    StringBuilder sb = new StringBuilder();
    for (int i = 0; i < s.length(); i++) {
      char c = s.charAt(i);
      switch (c) {
        case '\\' -> sb.append("\\\\");
        case '\t' -> sb.append("\\t");
        case '\r' -> sb.append("\\r");
        case '\n' -> sb.append("\\n");
        default -> sb.append(c);
      }
    }
    return sb.toString();
  }

  private P8T2Probe() {}
}
