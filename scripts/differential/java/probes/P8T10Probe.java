package app.freerouting.io.kicad;

import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.geometry.planar.FloatPoint;
import app.freerouting.geometry.planar.Point;
import app.freerouting.io.BoardReadResult;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.io.StringReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;

// Plan 8 Task 10 ground-truth probe: `io/kicad/KiCadJsonWriter.write`
// (`KiCadJsonWriter.java:27-226`) and `io/kicad/KiCadJsonReader.importSession`
// (`KiCadJsonReader.java:757-855`).
//
// It is not a differential driver: there is no Rust twin binary and `run.sh` does not know it —
// the `P7T15bProbe`/`P8T0Probe`/`P8T1Probe`/`P8T2Probe`/`P8T3Probe`/`P8T8Probe` pattern. Its
// stdout is committed verbatim as `crates/fr-dsn/tests/data/p8t10-kicad-writer.txt` and replayed
// row by row by `crates/fr-dsn/tests/kicad_writer.rs`.
//
// ## The three tables
//
//   HEADER  jar path/size — which jar produced the transcript
//   [wcase] stem, design name, and either `file=<path under the Java checkout>` or
//           `json=<escaped literal>`: one board handed to `KiCadJsonWriter.write`
//   [w]|    one line of that call's **exact** output. Reconstruct the string by joining every
//           `[w]|` line of the case with `\n`; there is no trailing newline, because
//           `Gson.toJson` writes none. A `[w]bytes=` row carries the UTF-8 length as a check.
//   [rt]    the round trip: `write` -> `readBoard` -> `write` again, and whether the two strings
//           are equal. `KiCadJsonWriter` emits no components, so the *second* write is a fixed
//           point even though the first is lossy.
//   [icase] stem: a base board plus the session document imported onto it
//   [is]    the board's item graph after `importSession` — the same shape `P8T8Probe`'s `[s9]`
//           item rows use — or `throw=<class>: <message>` when it threw.
//
// Every floating-point value crosses as `Double.toString`. No clock, no identity hash: the item
// order is `board.getItems()`, which is item id descending (quirk #63).
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential
//   # -encoding UTF-8 is explicit because the `non-ascii-net` stem carries a literal that is
//   # not ASCII; JDK 18+ defaults to it, older javac would mangle the source.
//   "$JDK/bin/javac" -encoding UTF-8 -cp "$JAR" -d /tmp/p8t10 java/probes/P8T10Probe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -cp "/tmp/p8t10:$JAR" app.freerouting.io.kicad.P8T10Probe \
//     > ../../crates/fr-dsn/tests/data/p8t10-kicad-writer.txt
//
// One invocation, one transcript — unlike `P8T8Probe`, which has two.
public final class P8T10Probe {

  private P8T10Probe() {}

  /** One writer input: a stem, a design name, and either a file or a JSON literal. */
  private record WRow(String stem, String designName, String file, String json) {
    static WRow file(String stem, String designName, String file) {
      return new WRow(stem, designName, file, null);
    }

    static WRow json(String stem, String designName, String json) {
      return new WRow(stem, designName, null, json);
    }
  }

  /** One `importSession` input: a stem, the base board's JSON, and the session document. */
  private record IRow(String stem, String base, String session) {}

  /**
   * The writer corpus.
   *
   * <p>Four real KiCad exports the checkout ships under {@code fixtures/}, including both boards
   * in the corpus that carry wiring, and five synthetic payloads for the arms no fixture reaches:
   * {@code mil-full} (the MIL scale factor, a {@code plane} layer, a net class whose via rule has
   * a padstack, two traces, two vias and a conduction area), {@code um-minimal} (the UM scale
   * factor and a three-corner outline), {@code empty-document} (no layers, no nets, and the
   * {@code new OutlineJson()} field initialiser written out as an empty corner list), the
   * {@code null} design name {@code :45} collapses to {@code "KiCad_Design"}, and the
   * one-argument {@code write(RoutingBoard)} overload {@code :27-31} that nothing in the Java
   * tree calls.
   */
  private static final List<WRow> WRITER = new ArrayList<>();

  static {
    WRITER.add(
        WRow.file(
            "ecc83-v1",
            "Issue649-kicad_ecc83-pp_input_board_v1",
            "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.json"));
    WRITER.add(
        WRow.file(
            "complex-hierarchy-session",
            "Issue733-kicad_complex_hierarchy_output_session",
            "fixtures/Issue733-kicad_complex_hierarchy_output_session.json"));
    WRITER.add(
        WRow.file(
            "corney-island-session",
            "Issue368-CorneyIslandWireless_output_session",
            "fixtures/Issue368-CorneyIslandWireless_output_session.json"));
    WRITER.add(
        WRow.file(
            "complex-hierarchy-design",
            "Issue733-kicad_complex_hierarchy_input_design",
            "fixtures/Issue733-kicad_complex_hierarchy_input_design.json"));
    // MIL, with a net class whose via rule has a padstack (so `:100-110` fires), two traces, two
    // vias and a conduction area. `resolution` 1000 so the numbers are legible.
    WRITER.add(
        WRow.json(
            "mil-full",
            "mil-full",
            "{\"unit\":\"MIL\",\"resolution\":1000.0,"
                + "\"layers\":[{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"},"
                + "{\"index\":1,\"name\":\"B.Cu\",\"type\":\"plane\"}],"
                + "\"netClasses\":[{\"name\":\"Default\",\"clearance\":0.2,\"traceWidth\":0.25,"
                + "\"viaDiameter\":0.8,\"viaDrill\":0.4},"
                + "{\"name\":\"power\",\"clearance\":0.4,\"traceWidth\":0.5,"
                + "\"viaDiameter\":1.2,\"viaDrill\":0.6}],"
                + "\"nets\":[{\"id\":1,\"name\":\"GND\",\"className\":\"Default\","
                + "\"containsPlane\":true},"
                + "{\"id\":2,\"name\":\"VCC\",\"className\":\"power\",\"containsPlane\":false}],"
                + "\"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":50.0,\"y\":0.0},"
                + "{\"x\":50.0,\"y\":40.0},{\"x\":0.0,\"y\":40.0}],\"clearance\":0.3},"
                + "\"traces\":[{\"id\":1,\"netName\":\"GND\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":5.0,\"y\":1.0},{\"x\":5.0,\"y\":6.0}]},"
                + "{\"id\":2,\"netName\":\"VCC\",\"width\":0.5,\"layerIndex\":1,"
                + "\"points\":[{\"x\":2.0,\"y\":8.0},{\"x\":9.0,\"y\":8.0}]}],"
                + "\"vias\":[{\"id\":1,\"netName\":\"GND\",\"position\":{\"x\":3.0,\"y\":4.0},"
                + "\"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":1},"
                + "{\"id\":2,\"netName\":\"VCC\",\"position\":{\"x\":7.0,\"y\":9.0},"
                + "\"diameter\":1.2,\"drill\":0.6,\"startLayerIndex\":0,\"endLayerIndex\":1}],"
                + "\"conductionAreas\":[{\"id\":1,\"netName\":\"GND\",\"layerIndex\":1,"
                + "\"isObstacle\":false,\"polygon\":[{\"x\":10.0,\"y\":10.0},"
                + "{\"x\":30.0,\"y\":10.0},{\"x\":30.0,\"y\":25.0},{\"x\":10.0,\"y\":25.0}]}]}"));
    // UM, so the third `scaleFactor` arm (`:37-38`, 10.0) is exercised by a synthetic row too.
    WRITER.add(
        WRow.json(
            "um-minimal",
            "um-minimal",
            "{\"unit\":\"UM\",\"resolution\":1.0,"
                + "\"layers\":[{\"index\":0,\"name\":\"top\",\"type\":\"signal\"}],"
                + "\"nets\":[{\"id\":1,\"name\":\"n1\",\"className\":\"default\"}],"
                + "\"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":1000.0,\"y\":0.0},"
                + "{\"x\":1000.0,\"y\":800.0}]}}"));
    // The empty document: no layers, no nets, no outline. `boardJson.outline` keeps its
    // `new OutlineJson()` initialiser, so the key is written with an **empty** corner list.
    WRITER.add(WRow.json("empty-document", "empty-document", "{}"));
    // A `null` design name: `:45`'s `designName != null ? designName : "KiCad_Design"`.
    WRITER.add(WRow.json("null-design-name", null, "{}"));
    // The one-argument overload `write(RoutingBoard)` (`:27-31`), which nothing in the Java tree
    // calls. Marked by a design name of `<one-arg>` in the `[wcase]` row.
    WRITER.add(WRow.json("one-arg-overload", "<one-arg>", "{}"));
  }

  /** The `importSession` corpus. */
  private static final List<IRow> SESSION = new ArrayList<>();

  /** A two-layer 50x40 mm board at resolution 1000 with two nets, and nothing on it. */
  private static String base(String body) {
    return "{\"unit\":\"MM\",\"resolution\":1000.0,"
        + "\"layers\":[{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"},"
        + "{\"index\":1,\"name\":\"B.Cu\",\"type\":\"signal\"}],"
        + "\"nets\":[{\"id\":1,\"name\":\"GND\",\"className\":\"default\"},"
        + "{\"id\":2,\"name\":\"VCC\",\"className\":\"default\"}],"
        + "\"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":50.0,\"y\":0.0},"
        + "{\"x\":50.0,\"y\":40.0},{\"x\":0.0,\"y\":40.0}]}"
        + body
        + "}";
  }

  private static final String BASE = base("");

  static {
    // The happy path: one zone, two traces, two vias, at the document's own resolution.
    SESSION.add(
        new IRow(
            "wires-and-vias",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"traces\":[{\"id\":1,\"netName\":\"GND\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":5.0,\"y\":1.0},{\"x\":5.0,\"y\":6.0}]},"
                + "{\"id\":2,\"netName\":\"VCC\",\"width\":0.5,\"layerIndex\":1,"
                + "\"points\":[{\"x\":2.0,\"y\":8.0},{\"x\":9.0,\"y\":8.0}]}],"
                + "\"vias\":[{\"id\":1,\"netName\":\"GND\",\"position\":{\"x\":3.0,\"y\":4.0},"
                + "\"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":1},"
                + "{\"id\":2,\"netName\":\"VCC\",\"position\":{\"x\":7.0,\"y\":9.0},"
                + "\"diameter\":1.2,\"drill\":0.6,\"startLayerIndex\":0,\"endLayerIndex\":1}],"
                + "\"conductionAreas\":[{\"id\":1,\"netName\":\"GND\",\"layerIndex\":1,"
                + "\"isObstacle\":true,\"polygon\":[{\"x\":10.0,\"y\":10.0},"
                + "{\"x\":30.0,\"y\":10.0},{\"x\":30.0,\"y\":25.0}]}]}"));
    // `:771-773` — `resolution == 1.0 && unit == MM` becomes 10000, not 1.
    SESSION.add(
        new IRow(
            "resolution-one-mm",
            BASE,
            "{\"unit\":\"MM\","
                + "\"traces\":[{\"id\":1,\"netName\":\"GND\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":1.0}]}]}"));
    // The same document in MIL, where the special case does **not** apply and `resolution` stays
    // `(int) Math.max(1.0, 1.0)` = 1.
    SESSION.add(
        new IRow(
            "resolution-one-mil",
            BASE,
            "{\"unit\":\"MIL\","
                + "\"traces\":[{\"id\":1,\"netName\":\"GND\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":1.0}]}]}"));
    // A fractional resolution: `(int) Math.max(1.0, 2.7)` truncates to 2.
    SESSION.add(
        new IRow(
            "resolution-fractional",
            BASE,
            "{\"unit\":\"UM\",\"resolution\":2.7,"
                + "\"traces\":[{\"id\":1,\"netName\":\"GND\",\"width\":100.0,\"layerIndex\":0,"
                + "\"points\":[{\"x\":100.0,\"y\":100.0},{\"x\":200.0,\"y\":100.0}]}]}"));
    // A resolution below 1: `Math.max(1.0, 0.25)` clamps to 1, and the MM special case then
    // does **not** fire because `boardJson.resolution` is `0.25`, not `1.0`.
    SESSION.add(
        new IRow(
            "resolution-below-one",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":0.25,"
                + "\"traces\":[{\"id\":1,\"netName\":\"GND\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":1.0}]}]}"));
    // A net name the board does not carry: `netNumber` 0, so `netNumbers` is empty.
    SESSION.add(
        new IRow(
            "unknown-net",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"traces\":[{\"id\":1,\"netName\":\"NOSUCH\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":1.0}]}]}"));
    // No `netName` at all — `Nets.get(null, 1)` is not a throw, because
    // `String.equalsIgnoreCase(null)` is `false`.
    SESSION.add(
        new IRow(
            "absent-net-name",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"traces\":[{\"id\":1,\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":1.0}]}]}"));
    // A non-ASCII net name that the base board carries — quirk label U's fixture. The document
    // reaches `importSession` already decoded here (the probe hands it a `StringReader`), so this
    // row measures the *matching*, not the charset; the charset itself is measured end to end by
    // `crates/freerouting/tests/cli_e2e.rs`.
    SESSION.add(
        new IRow(
            "non-ascii-net",
            base(",\"components\":[]").replace("\"GND\"", "\"GND_é中\""),
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"traces\":[{\"id\":1,\"netName\":\"GND_é中\",\"width\":0.25,"
                + "\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":1.0}]}]}"));
    // `:839-842` bounds-checks the shape store, where `readBoard:705-707` does not: an
    // out-of-range span is silently skipped here instead of throwing.
    SESSION.add(
        new IRow(
            "via-layer-out-of-range",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"vias\":[{\"id\":1,\"netName\":\"GND\",\"position\":{\"x\":3.0,\"y\":4.0},"
                + "\"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":-1,\"endLayerIndex\":5}]}"));
    // Every layer skipped, so the padstack is all-`null` and `DrillItem.tileShapeCount` is
    // negative — quirk #286's `NegativeArraySizeException`.
    SESSION.add(
        new IRow(
            "via-start-gt-end",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"vias\":[{\"id\":1,\"netName\":\"GND\",\"position\":{\"x\":3.0,\"y\":4.0},"
                + "\"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":1,\"endLayerIndex\":0}]}"));
    // A negative trace layer: `Trace`'s constructor clamps it to 0 (Trace.java:45).
    SESSION.add(
        new IRow(
            "trace-negative-layer",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"traces\":[{\"id\":1,\"netName\":\"GND\",\"width\":0.25,\"layerIndex\":-3,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":1.0}]}]}"));
    // A trace layer past the end: the second clamp, `Math.min(layer, layerCount - 1)`.
    SESSION.add(
        new IRow(
            "trace-layer-out-of-range",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"traces\":[{\"id\":1,\"netName\":\"GND\",\"width\":0.25,\"layerIndex\":9,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":1.0}]}]}"));
    // The three unguarded dereferences inside the guarded loops.
    SESSION.add(
        new IRow(
            "zone-null-polygon",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"conductionAreas\":[{\"id\":1,\"netName\":\"GND\",\"layerIndex\":0,"
                + "\"isObstacle\":false,\"polygon\":null}]}"));
    SESSION.add(
        new IRow(
            "zone-empty-polygon",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"conductionAreas\":[{\"id\":1,\"netName\":\"GND\",\"layerIndex\":0,"
                + "\"isObstacle\":false,\"polygon\":[]}]}"));
    SESSION.add(
        new IRow(
            "trace-null-points",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"traces\":[{\"id\":1,\"netName\":\"GND\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":null}]}"));
    SESSION.add(
        new IRow(
            "via-null-position",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"vias\":[{\"id\":1,\"netName\":\"GND\",\"position\":null,"
                + "\"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":1}]}"));
    // The three lists explicitly `null` — **guarded** here, unlike `readBoard`.
    SESSION.add(
        new IRow(
            "all-lists-null",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":1000.0,\"traces\":null,\"vias\":null,"
                + "\"conductionAreas\":null}"));
    // The empty document, and the two payloads Gson turns into a `null` DTO.
    SESSION.add(new IRow("empty-object", BASE, "{}"));
    SESSION.add(new IRow("json-null", BASE, "null"));
    SESSION.add(new IRow("json-empty-string", BASE, ""));
    SESSION.add(new IRow("json-truncated", BASE, "{\"unit\":"));
    // Two vias with the same generated padstack name: the second reuses the first's padstack.
    SESSION.add(
        new IRow(
            "via-padstack-reuse",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"vias\":[{\"id\":1,\"netName\":\"GND\",\"position\":{\"x\":3.0,\"y\":4.0},"
                + "\"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":1},"
                + "{\"id\":2,\"netName\":\"VCC\",\"position\":{\"x\":9.0,\"y\":4.0},"
                + "\"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":1}]}"));
    // The `%.0f` HALF_UP rounding in the generated padstack name (quirk #288): 0.0025 mm is
    // `3` to Java's `String.format` and `2` to Rust's half-to-even `{:.0}`.
    SESSION.add(
        new IRow(
            "via-name-half-up",
            BASE,
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"vias\":[{\"id\":1,\"netName\":\"GND\",\"position\":{\"x\":3.0,\"y\":4.0},"
                + "\"diameter\":0.0025,\"drill\":0.0035,\"startLayerIndex\":0,"
                + "\"endLayerIndex\":1}]}"));
    // A session imported onto a board that already carries wiring, which is the real `-di` shape:
    // the base board is the happy-path result, and the same document is imported a second time.
    SESSION.add(
        new IRow(
            "onto-existing-wiring",
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"layers\":[{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"},"
                + "{\"index\":1,\"name\":\"B.Cu\",\"type\":\"signal\"}],"
                + "\"nets\":[{\"id\":1,\"name\":\"GND\",\"className\":\"default\"},"
                + "{\"id\":2,\"name\":\"VCC\",\"className\":\"default\"}],"
                + "\"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":50.0,\"y\":0.0},"
                + "{\"x\":50.0,\"y\":40.0},{\"x\":0.0,\"y\":40.0}]},"
                + "\"traces\":[{\"id\":1,\"netName\":\"GND\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":9.0,\"y\":1.0}]}]}",
            "{\"unit\":\"MM\",\"resolution\":1000.0,"
                + "\"vias\":[{\"id\":1,\"netName\":\"GND\",\"position\":{\"x\":5.0,\"y\":1.0},"
                + "\"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":1}]}"));
  }

  public static void main(String[] argv) throws Exception {
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    // The jar chatters on stdout through `FRLogger`; keep the transcript clean.
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    String javaDir =
        System.getenv()
            .getOrDefault("FREEROUTING_JAVA_DIR", "/Users/em/Development/freerouting/freerouting");

    out.println("# P8T10Probe — KiCadJsonWriter.write and KiCadJsonReader.importSession, HEAD jar");
    out.println("# io/kicad/KiCadJsonWriter.java:27-226 and io/kicad/KiCadJsonReader.java:757-855");
    out.println("# [w]| rows are one line of write()'s exact output; join them with \\n");
    out.println("# [is] rows are the board's item graph after importSession");
    out.println("# no clock, no identity hash: byte-stable across runs and -XX:hashCode settings");
    out.println(header(javaDir));

    for (WRow row : WRITER) {
      String json = row.file() != null
          ? Files.readString(Path.of(javaDir, row.file()), StandardCharsets.UTF_8)
          : row.json();

      out.println();
      out.print("[wcase] stem=" + row.stem() + " designName=" + escape(row.designName()) + " ");
      if (row.file() != null) {
        out.println("file=" + row.file() + " bytes=" + json.getBytes(StandardCharsets.UTF_8).length);
      } else {
        out.println("json=" + escape(json));
      }

      RoutingBoard board = boardOf(json);
      if (board == null) {
        out.println("[w] result=<no board>");
        continue;
      }
      String written =
          "<one-arg>".equals(row.designName())
              ? KiCadJsonWriter.write(board)
              : KiCadJsonWriter.write(board, row.designName());
      out.println("[w]bytes=" + written.getBytes(StandardCharsets.UTF_8).length);
      for (String line : written.split("\n", -1)) {
        out.println("[w]|" + line);
      }

      // The round trip: read the written document back and write it again.
      RoutingBoard reread = boardOf(written);
      if (reread == null) {
        out.println("[rt] reread=<no board>");
      } else {
        String again = KiCadJsonWriter.write(reread, "roundtrip");
        String first = KiCadJsonWriter.write(board, "roundtrip");
        out.println("[rt] fixedPoint=" + again.equals(first) + " bytes=" + again.length());
      }
    }

    for (IRow row : SESSION) {
      out.println();
      out.println(
          "[icase] stem=" + row.stem() + " base=" + escape(row.base())
              + " session=" + escape(row.session()));
      RoutingBoard board = boardOf(row.base());
      if (board == null) {
        out.println("[is] base=<no board>");
        continue;
      }
      try {
        KiCadJsonReader.importSession(new StringReader(row.session()), board);
        out.println("[is] result=ok");
      } catch (Throwable e) {
        out.println(
            "[is] throw=" + e.getClass().getName() + ": " + escape(String.valueOf(e.getMessage())));
      }
      emitItems(out, board);
    }
  }

  /** `readBoard` on a document, or `null` if it did not produce a board. */
  private static RoutingBoard boardOf(String json) {
    BoardReadResult result =
        KiCadJsonReader.readBoard(
            new StringReader(json), null, new app.freerouting.board.actions.ItemIdGenerator());
    return switch (result) {
      case BoardReadResult.Success s -> (RoutingBoard) s.board();
      case BoardReadResult.OutlineMissing o -> (RoutingBoard) o.board();
      default -> null;
    };
  }

  /** The board's items, in `getItems()` order — the `P8T8Probe` `[s9]` item shape. */
  private static void emitItems(PrintStream out, RoutingBoard board) {
    List<Item> items = new ArrayList<>();
    for (Item item : board.getItems()) {
      items.add(item);
    }
    out.println("[is] items count=" + items.size());
    out.println("[is] padstacks count=" + board.library.padstacks.count());
    for (int i = 1; i <= board.library.padstacks.count(); i++) {
      out.println(
          "[is] padstack " + i + " name=" + escape(board.library.padstacks.get(i).name)
              + " fromLayer=" + board.library.padstacks.get(i).fromLayer()
              + " toLayer=" + board.library.padstacks.get(i).toLayer());
    }
    for (Item item : items) {
      StringBuilder nets = new StringBuilder();
      for (int n = 0; n < item.netCount(); n++) {
        if (n > 0) {
          nets.append(',');
        }
        nets.append(item.getNetNumber(n));
      }
      String head = "[is] item " + item.getId() + " " + item.getClass().getSimpleName()
          + " nets=[" + nets + "]"
          + " cl=" + item.clearanceClassIndex()
          + " fixed=" + item.getFixedState();
      if (item instanceof app.freerouting.board.trace.PolylineTrace trace) {
        StringBuilder corners = new StringBuilder();
        for (int c = 0; c < trace.polyline().cornerCount(); c++) {
          if (c > 0) {
            corners.append(';');
          }
          FloatPoint corner = trace.polyline().cornerApprox(c);
          corners.append(Double.toString(corner.x)).append(',').append(Double.toString(corner.y));
        }
        out.println(head + " layer=" + trace.getLayer()
            + " halfWidth=" + trace.getHalfWidth()
            + " corners=[" + corners + "]");
      } else if (item instanceof app.freerouting.board.model.items.Via via) {
        Point center = via.getCenter();
        out.println(head + " padstack="
            + (via.getPadstack() == null ? "<null>" : escape(via.getPadstack().name))
            + " center=" + Double.toString(center.toFloat().x)
            + "," + Double.toString(center.toFloat().y));
      } else if (item instanceof app.freerouting.board.model.items.ConductionArea zone) {
        StringBuilder sb = new StringBuilder();
        FloatPoint[] corners = zone.getRelativeArea().cornerApproxArr();
        for (int i = 0; i < corners.length; i++) {
          if (i > 0) {
            sb.append(';');
          }
          sb.append(Double.toString(corners[i].x)).append(',').append(Double.toString(corners[i].y));
        }
        out.println(head + " layer=" + zone.getLayer()
            + " isObstacle=" + zone.getIsObstacle()
            + " area=(" + sb + ")");
      } else {
        out.println(head);
      }
    }
  }

  private static String header(String javaDir) {
    try {
      Path jar =
          Path.of(
              KiCadJsonWriter.class.getProtectionDomain().getCodeSource().getLocation().toURI());
      return "HEADER jar="
          + jar.getFileName()
          + " size="
          + Files.size(jar)
          + " javaDir="
          + Path.of(javaDir).getFileName();
    } catch (Exception e) {
      return "HEADER jar=<unknown> (" + e + ")";
    }
  }

  /** One transcript row is one line: `\\`, `\t`, `\r`, `\n`; a `null` prints `<null>`. */
  private static String escape(String text) {
    if (text == null) {
      return "<null>";
    }
    StringBuilder sb = new StringBuilder(text.length() + 8);
    for (int i = 0; i < text.length(); i++) {
      char c = text.charAt(i);
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
}
