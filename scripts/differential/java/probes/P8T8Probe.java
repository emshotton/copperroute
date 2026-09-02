package app.freerouting.io.kicad;

import app.freerouting.board.actions.ItemIdGenerator;
import app.freerouting.board.facade.RoutingBoard;
import app.freerouting.board.model.items.Item;
import app.freerouting.board.model.structure.Layer;
import app.freerouting.board.state.Communication;
import app.freerouting.core.library.Padstack;
import app.freerouting.geometry.planar.IntBox;
import app.freerouting.geometry.planar.Point;
import app.freerouting.geometry.planar.PolygonShape;
import app.freerouting.geometry.planar.PolylineShape;
import app.freerouting.io.BoardMetadata;
import app.freerouting.io.BoardReadResult;
import app.freerouting.rules.ClearanceMatrix;
import app.freerouting.rules.DefaultItemClearanceClasses;
import app.freerouting.rules.Net;
import app.freerouting.rules.NetClass;
import app.freerouting.rules.ViaInfo;
import app.freerouting.rules.ViaRule;
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

// Plan 8 Task 8 ground-truth probe: `io/kicad/KiCadJsonReader.readBoard`
// (`KiCadJsonReader.java:61-755`), of which **sections 1-8 (`:63-497`)** are Task 8's port and
// sections 9-11 (`:498-755`) are Task 9's.
//
// It is not a differential driver: there is no Rust twin binary and `run.sh` does not know it —
// the `P7T15bProbe`/`P8T0Probe`/`P8T1Probe`/`P8T2Probe`/`P8T3Probe` pattern. Its stdout is
// committed verbatim as `crates/fr-dsn/tests/data/p8t8-kicad-read-a.txt` and replayed as literals
// by `crates/fr-dsn/tests/kicad_reader.rs`.
//
// ## Why every field a section-1-to-8 row prints survives sections 9-11
//
// `readBoard` is one method, so the probe can only observe the board it *returns* — after
// sections 9-11 have run. Those three sections are read-only on everything sections 1-8 built:
// `awk 'NR>=498 && NR<=720' KiCadJsonReader.java | grep -n 'boardRules\|rules\.\|netClasses\|
// clearanceMatrix\|viaRules\|viaInfos\|nets\.'` finds exactly four hits and all four are
// `boardRules.nets.get(<name>, 1)` lookups (`:639`, `:649`, `:668`, `:685`). What they *do* add is
// (a) padstacks after the section-8 ones, (b) packages, (c) components and (d) items. So the
// `[s8]` rows below are section-1-to-8 state and the `[s9]` rows are the Task 9 surface, printed
// for that task's benefit and **not** compared by Task 8's Rust test.
//
// ## The tables
//
//   HEADER  jar path/size/mtime — which jar produced the transcript
//   [case]  stem, and either `file=<path under the Java checkout>` or `json=<escaped literal>`
//   [s8] ...  one row per section-1-to-8 observable, listed under "the rows" below
//   [s9] ...  padstack/package/component/item counts, Task 9's surface
//
// Every floating-point value crosses as `Double.toString`. No clock, no identity hash, no
// `System.identityHashCode`: the one hash-ordered thing `readBoard` does is
// `readBoard:456-496`'s `HashSet<String> referencedNets`, whose iteration order is a function of
// `String.hashCode` alone and is therefore byte-stable across JVMs and across `-XX:hashCode`
// settings. That order is exactly what the `net` rows record, so the port has to reproduce
// `java.util.HashSet`'s bucket layout to match them.
//
//   JAR=../freerouting/build/libs/freerouting-current-executable.jar
//   JDK=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home
//   cd scripts/differential
//   "$JDK/bin/javac" -cp "$JAR" -d /tmp/p8t8 java/probes/P8T8Probe.java
//   "$JDK/bin/java" -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
//       -cp "/tmp/p8t8:$JAR" app.freerouting.io.kicad.P8T8Probe \
//     > ../../crates/fr-dsn/tests/data/p8t8-kicad-read-a.txt
public final class P8T8Probe {

  private P8T8Probe() {}

  /** One measured input: a stem, and either a file under the Java checkout or a JSON literal. */
  private record Row(String stem, String file, String json) {
    static Row file(String stem, String file) {
      return new Row(stem, file, null);
    }

    static Row json(String stem, String json) {
      return new Row(stem, null, json);
    }
  }

  /**
   * The corpus. The five `*_input_design`/`*_input_board` files and the two `*_output_session`
   * files are **real KiCad board JSON** shipped with the Java checkout — the fixture hunt the task
   * brief asked for found them under `fixtures/`, so no writer-generated stand-in was needed. The
   * synthetic rows after them reach the arms no fixture does: MIL and UM units, an unknown unit,
   * a missing layer list, a `plane` layer, custom clearance rules, an entirely empty board, and
   * the four malformed payloads.
   */
  private static final List<Row> CORPUS = new ArrayList<>();

  static {
    CORPUS.add(Row.file("ecc83-v1", "fixtures/Issue649-kicad_ecc83-pp_input_board_v1.json"));
    CORPUS.add(Row.file("ecc83-v2", "fixtures/Issue649-kicad_ecc83-pp_input_board_v2.json"));
    CORPUS.add(
        Row.file("complex-hierarchy", "fixtures/Issue733-kicad_complex_hierarchy_input_design.json"));
    CORPUS.add(Row.file("interf-u", "fixtures/Issue733-kicad_interf_u_input_design.json"));
    CORPUS.add(
        Row.file("corney-island", "fixtures/Issue368-CorneyIslandWireless_input_design.json"));
    CORPUS.add(
        Row.file("corney-island-session", "fixtures/Issue368-CorneyIslandWireless_output_session.json"));
    CORPUS.add(
        Row.file(
            "complex-hierarchy-session",
            "fixtures/Issue733-kicad_complex_hierarchy_output_session.json"));

    // --- synthetic: the arms the corpus does not reach ------------------------------------
    CORPUS.add(Row.json("empty-object", "{}"));
    CORPUS.add(Row.json("json-null", "null"));
    CORPUS.add(Row.json("json-empty", ""));
    CORPUS.add(Row.json("json-truncated", "{"));
    CORPUS.add(Row.json("layers-null", "{\"layers\": null}"));
    CORPUS.add(Row.json("netclasses-null", "{\"netClasses\": null}"));
    CORPUS.add(
        Row.json(
            "unit-mil",
            "{\"unit\":\"MIL\",\"resolution\":1.0,"
                + "\"layers\":[{\"index\":0,\"name\":\"F.Cu\",\"type\":\"signal\"},"
                + "{\"index\":1,\"name\":\"In1.Cu\",\"type\":\"plane\"},"
                + "{\"index\":2,\"name\":\"B.Cu\",\"type\":\"SIGNAL\"}],"
                + "\"netClasses\":[{\"name\":\"Default\",\"clearance\":10.0,\"traceWidth\":15.0,"
                + "\"viaDiameter\":30.0,\"viaDrill\":15.0},"
                + "{\"name\":\"HV\",\"clearance\":40.0,\"traceWidth\":25.0,"
                + "\"viaDiameter\":50.0,\"viaDrill\":25.0}],"
                + "\"nets\":[{\"id\":1,\"name\":\"GND\",\"className\":\"Default\","
                + "\"containsPlane\":true},"
                + "{\"id\":2,\"name\":\"HV1\",\"className\":\"hv\",\"containsPlane\":false},"
                + "{\"id\":3,\"name\":\"NC\",\"className\":\"nope\",\"containsPlane\":false}],"
                + "\"clearanceRules\":[{\"classA\":\"default\",\"classB\":\"HV\","
                + "\"clearance\":77.0},{\"classA\":\"ghost\",\"classB\":\"HV\","
                + "\"clearance\":99.0}],"
                + "\"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":100.0,\"y\":0.0},"
                + "{\"x\":100.0,\"y\":80.0},{\"x\":0.0,\"y\":80.0}],\"clearance\":0.0}}"));
    CORPUS.add(
        Row.json(
            "unit-um",
            "{\"unit\":\"UM\",\"resolution\":10.0,"
                + "\"layers\":[{\"index\":0,\"name\":\"top\",\"type\":\"plane\"},"
                + "{\"index\":1,\"name\":\"bot\",\"type\":null}],"
                + "\"netClasses\":[{\"name\":\"kicad_default\",\"clearance\":200.0,"
                + "\"traceWidth\":250.0}],"
                + "\"nets\":[{\"id\":1,\"name\":\"N1\",\"className\":\"kicad_default\","
                + "\"containsPlane\":false}],"
                + "\"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":5000.0,\"y\":0.0},"
                + "{\"x\":5000.0,\"y\":4000.0}]}}"));
    CORPUS.add(
        Row.json(
            "unit-unknown",
            "{\"unit\":\"FOO\",\"resolution\":1.0,"
                + "\"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":10.0,\"y\":0.0},"
                + "{\"x\":10.0,\"y\":10.0},{\"x\":0.0,\"y\":10.0}]}}"));
    CORPUS.add(
        Row.json(
            "unit-lowercase",
            "{\"unit\":\"mil\",\"resolution\":1.0,"
                + "\"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":10.0,\"y\":0.0},"
                + "{\"x\":10.0,\"y\":10.0}]}}"));
    CORPUS.add(
        Row.json(
            "resolution-fractional",
            "{\"unit\":\"MM\",\"resolution\":2.75,"
                + "\"hostCad\":\"  \",\"hostVersion\":\"9.9\","
                + "\"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":10.0,\"y\":0.0},"
                + "{\"x\":10.0,\"y\":10.0},{\"x\":0.0,\"y\":10.0}]}}"));
    CORPUS.add(
        Row.json(
            "resolution-zero",
            "{\"unit\":\"MM\",\"resolution\":0.0,\"hostCad\":\"Altium\","
                + "\"hostVersion\":\"v24\","
                + "\"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":10.0,\"y\":0.0},"
                + "{\"x\":10.0,\"y\":10.0},{\"x\":0.0,\"y\":10.0}]}}"));
    CORPUS.add(
        Row.json(
            "outline-two-corners",
            "{\"unit\":\"MM\",\"resolution\":1.0,"
                + "\"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":10.0,\"y\":10.0}]},"
                + "\"vias\":[{\"id\":1,\"netName\":\"V\",\"position\":{\"x\":3.0,\"y\":4.0},"
                + "\"diameter\":0.8,\"drill\":0.4,\"startLayerIndex\":0,\"endLayerIndex\":1}]}"));
    CORPUS.add(
        Row.json(
            "outline-unsorted",
            "{\"unit\":\"MM\",\"resolution\":1.0,"
                + "\"outline\":{\"corners\":[{\"x\":10.0,\"y\":10.0},{\"x\":0.0,\"y\":0.0},"
                + "{\"x\":0.0,\"y\":10.0},{\"x\":10.0,\"y\":0.0},{\"x\":5.0,\"y\":-3.0}]}}"));
    CORPUS.add(
        Row.json(
            "y-half-tie",
            "{\"unit\":\"MIL\",\"resolution\":1.0,"
                + "\"outline\":{\"corners\":[{\"x\":0.5,\"y\":0.5},{\"x\":10.5,\"y\":0.5},"
                + "{\"x\":10.5,\"y\":20.5}]}}"));
    CORPUS.add(
        Row.json(
            "netclass-duplicate-names",
            "{\"unit\":\"MM\",\"resolution\":1.0,"
                + "\"netClasses\":[{\"name\":\"A\",\"clearance\":0.2,\"traceWidth\":0.3},"
                + "{\"name\":\"A\",\"clearance\":0.4,\"traceWidth\":0.5}],"
                + "\"nets\":[{\"id\":1,\"name\":\"n1\",\"className\":\"A\"}],"
                + "\"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":10.0,\"y\":0.0},"
                + "{\"x\":10.0,\"y\":10.0},{\"x\":0.0,\"y\":10.0}]}}"));
    CORPUS.add(
        Row.json(
            "referenced-nets-only",
            "{\"unit\":\"MM\",\"resolution\":1.0,"
                + "\"outline\":{\"corners\":[{\"x\":0.0,\"y\":0.0},{\"x\":40.0,\"y\":0.0},"
                + "{\"x\":40.0,\"y\":30.0},{\"x\":0.0,\"y\":30.0}]},"
                + "\"traces\":[{\"id\":1,\"netName\":\"alpha\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":2,\"netName\":\"beta\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":3,\"netName\":\"gamma\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":4,\"netName\":\"delta\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":5,\"netName\":\"epsilon\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":6,\"netName\":\"zeta\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":7,\"netName\":\"eta\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":8,\"netName\":\"theta\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":9,\"netName\":\"iota\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":10,\"netName\":\"kappa\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":11,\"netName\":\"lambda\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":12,\"netName\":\"mu\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":13,\"netName\":\"nu\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":14,\"netName\":\"xi\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":15,\"netName\":\"omicron\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":16,\"netName\":\"pi\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":17,\"netName\":\"rho\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]},"
                + "{\"id\":18,\"netName\":\"\",\"width\":0.25,\"layerIndex\":0,"
                + "\"points\":[{\"x\":1.0,\"y\":1.0},{\"x\":2.0,\"y\":2.0}]}]}"));
  }

  public static void main(String[] argv) throws Exception {
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    // The jar chatters on stdout through `FRLogger`; keep the transcript clean.
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    String javaDir =
        System.getenv()
            .getOrDefault("FREEROUTING_JAVA_DIR", "/Users/em/Development/freerouting/freerouting");

    out.println("# P8T8Probe — KiCadJsonReader.readBoard, HEAD jar");
    out.println("# io/kicad/KiCadJsonReader.java:61-755; Task 8 ports sections 1-8 (:63-497)");
    out.println("# [s8] rows are section-1-to-8 state; [s9] rows are Task 9's surface");
    out.println("# no clock, no identity hash: byte-stable across runs and -XX:hashCode settings");
    out.println(header(javaDir));

    for (Row row : CORPUS) {
      String json;
      if (row.file() != null) {
        json = Files.readString(Path.of(javaDir, row.file()), StandardCharsets.UTF_8);
      } else {
        json = row.json();
      }

      out.println();
      out.print("[case] stem=" + row.stem() + " ");
      if (row.file() != null) {
        out.println("file=" + row.file() + " bytes=" + json.getBytes(StandardCharsets.UTF_8).length);
      } else {
        out.println("json=" + escape(json));
      }

      BoardReadResult result =
          KiCadJsonReader.readBoard(new StringReader(json), null, new ItemIdGenerator());
      emit(out, result);
    }
  }

  // ===============================================================================================
  // emission
  // ===============================================================================================

  private static void emit(PrintStream out, BoardReadResult result) {
    switch (result) {
      case BoardReadResult.ParseError e -> {
        out.println("[s8] result=ParseError location=" + escape(e.location()) + " detail="
            + escape(e.detail()));
        return;
      }
      case BoardReadResult.IoError e -> {
        out.println("[s8] result=IoError cause=" + escape(String.valueOf(e.cause())));
        return;
      }
      case BoardReadResult.OutlineMissing o -> {
        out.println("[s8] result=OutlineMissing");
        emitBoard(out, (RoutingBoard) o.board(), o.metadata(), o.warnings());
      }
      case BoardReadResult.Success s -> {
        out.println("[s8] result=Success");
        emitBoard(out, (RoutingBoard) s.board(), s.metadata(), s.warnings());
      }
    }
  }

  private static void emitBoard(
      PrintStream out, RoutingBoard board, BoardMetadata metadata, List<String> warnings) {
    // --- section 3: layer structure --------------------------------------------------------
    Layer[] layers = board.layerStructure.layers;
    out.println("[s8] layers count=" + layers.length);
    for (int i = 0; i < layers.length; i++) {
      out.println("[s8] layer " + i + " name=" + escape(layers[i].name) + " signal="
          + layers[i].isSignal);
    }

    // --- section 4: clearance matrix -------------------------------------------------------
    ClearanceMatrix matrix = board.rules.clearanceMatrix;
    out.println("[s8] clearance classes=" + matrix.getClassCount() + " layers="
        + matrix.getLayerCount());
    for (int i = 0; i < matrix.getClassCount(); i++) {
      out.println("[s8] clname " + i + " " + escape(matrix.getName(i)));
    }
    for (int layer = 0; layer < matrix.getLayerCount(); layer++) {
      for (int i = 0; i < matrix.getClassCount(); i++) {
        StringBuilder sb = new StringBuilder();
        sb.append("[s8] cl ").append(layer).append(' ').append(i);
        for (int j = 0; j < matrix.getClassCount(); j++) {
          sb.append(' ').append(matrix.getValue(i, j, layer, false));
        }
        out.println(sb);
      }
    }

    // --- section 5: outline + bounding box -------------------------------------------------
    IntBox bbox = board.boundingBox;
    out.println("[s8] bbox " + bbox.ll.x + " " + bbox.ll.y + " " + bbox.ur.x + " " + bbox.ur.y);
    var outline = board.getOutline();
    if (outline == null) {
      out.println("[s8] outline <null>");
    } else {
      out.println("[s8] outline shapes=" + outline.shapeCount() + " clearanceClass="
          + outline.clearanceClassIndex());
      for (int s = 0; s < outline.shapeCount(); s++) {
        PolylineShape shape = outline.getShape(s);
        if (shape instanceof PolygonShape poly) {
          out.println("[s8] outlineshape " + s + " PolygonShape corners=" + poly.corners.length);
          for (int c = 0; c < poly.corners.length; c++) {
            Point corner = poly.corners[c];
            out.println("[s8] corner " + s + " " + c + " " + corner.toFloat().x + " "
                + corner.toFloat().y);
          }
        } else {
          out.println("[s8] outlineshape " + s + " " + shape.getClass().getSimpleName()
              + " box=" + boxOf(shape));
        }
      }
    }

    // --- section 6: communication ----------------------------------------------------------
    Communication comm = board.communication;
    Communication.SpecctraParserInfo info = comm.specctraParserInfo;
    out.println("[s8] comm unit=" + comm.unit + " resolution=" + comm.resolution
        + " stringQuote=" + escape(info == null ? null : info.stringQuote)
        + " hostCad=" + escape(info == null ? null : info.hostCad)
        + " hostVersion=" + escape(info == null ? null : info.hostVersion)
        + " constants=" + (info == null || info.constants == null ? "<null>"
            : Integer.toString(info.constants.size()))
        + " writeResolution=" + (info == null ? "<null>" : String.valueOf(info.writeResolution))
        + " dsnGeneratedByHost=" + (info != null && info.dsnFileGeneratedByHost));
    out.println("[s8] transform scale=" + Double.toString(transformField(comm, "scaleFactor"))
        + " baseX=" + Double.toString(transformField(comm, "baseX"))
        + " baseY=" + Double.toString(transformField(comm, "baseY")));

    // --- section 7/8: rules ----------------------------------------------------------------
    out.println("[s8] netclasses count=" + board.rules.netClasses.count());
    for (int i = 0; i < board.rules.netClasses.count(); i++) {
      NetClass nc = board.rules.netClasses.get(i);
      StringBuilder widths = new StringBuilder();
      StringBuilder active = new StringBuilder();
      for (int layer = 0; layer < layers.length; layer++) {
        if (layer > 0) {
          widths.append(',');
          active.append(',');
        }
        widths.append(nc.getTraceHalfWidth(layer));
        active.append(nc.isActiveRoutingLayer(layer));
      }
      StringBuilder dicc = new StringBuilder();
      for (DefaultItemClearanceClasses.ItemClass ic :
          DefaultItemClearanceClasses.ItemClass.values()) {
        if (dicc.length() > 0) {
          dicc.append(',');
        }
        dicc.append(ic).append('=').append(nc.defaultItemClearanceClasses.get(ic));
      }
      ViaRule rule = nc.getViaRule();
      out.println("[s8] netclass " + i + " name=" + escape(nc.getName())
          + " traceClearanceClass=" + nc.getTraceClearanceClass()
          + " halfWidths=[" + widths + "]"
          + " activeLayers=[" + active + "]"
          + " viaRule=" + (rule == null ? "<null>" : escape(rule.name))
          + " shoveFixed=" + nc.isShoveFixed()
          + " pullTight=" + nc.getPullTight()
          + " ignoreCyclesWithAreas=" + nc.getIgnoreCyclesWithAreas()
          + " minTraceLength=" + Double.toString(nc.getMinimumTraceLength())
          + " maxTraceLength=" + Double.toString(nc.getMaximumTraceLength())
          + " dicc=[" + dicc + "]");
    }

    out.println("[s8] nets count=" + board.rules.nets.maxNetNumber());
    for (int no = 1; no <= board.rules.nets.maxNetNumber(); no++) {
      Net net = board.rules.nets.get(no);
      NetClass nc = net.getNetClass();
      out.println("[s8] net " + no + " name=" + escape(net.name)
          + " subnet=" + net.subnetNumber
          + " containsPlane=" + net.containsPlane()
          + " class=" + escape(nc == null ? null : nc.getName()));
    }

    out.println("[s8] viainfos count=" + board.rules.viaInfos.count());
    for (int i = 0; i < board.rules.viaInfos.count(); i++) {
      ViaInfo vi = board.rules.viaInfos.get(i);
      out.println("[s8] viainfo " + i + " name=" + escape(vi.getName())
          + " padstack=" + escape(vi.getPadstack() == null ? null : vi.getPadstack().name)
          + " clearanceClass=" + vi.getClearanceClassIndex()
          + " attachSmd=" + vi.attachSmdAllowed());
    }

    out.println("[s8] viarules count=" + board.rules.viaRules.size());
    for (int i = 0; i < board.rules.viaRules.size(); i++) {
      ViaRule rule = board.rules.viaRules.get(i);
      StringBuilder vias = new StringBuilder();
      for (int v = 0; v < rule.viaCount(); v++) {
        if (v > 0) {
          vias.append(',');
        }
        vias.append(rule.getVia(v).getName());
      }
      out.println("[s8] viarule " + i + " name=" + escape(rule.name) + " vias=[" + vias + "]");
    }

    out.println("[s8] viapadstacks count=" + board.library.getViaPadstacks().length);
    Padstack[] viaPadstacks = board.library.getViaPadstacks();
    for (int i = 0; i < viaPadstacks.length; i++) {
      out.println("[s8] viapadstack " + i + " name=" + escape(viaPadstacks[i].name)
          + " fromLayer=" + viaPadstacks[i].fromLayer()
          + " toLayer=" + viaPadstacks[i].toLayer()
          + " attachAllowed=" + viaPadstacks[i].attachAllowed
          + " placedAbsolute=" + viaPadstacks[i].placedAbsolute);
    }

    out.println("[s8] boardrules holeClearance=" + board.rules.getHoleClearance()
        + " minTraceHalfWidth=" + board.rules.getMinTraceHalfWidth()
        + " maxTraceHalfWidth=" + board.rules.getMaxTraceHalfWidth()
        + " traceAngleRestriction=" + board.rules.getTraceAngleRestriction()
        + " ignoreConduction=" + board.rules.getIgnoreConduction());

    // --- the tail: metadata + warnings -----------------------------------------------------
    out.println("[s8] metadata hostCad=" + escape(metadata == null ? null : metadata.hostCad())
        + " hostVersion=" + escape(metadata == null ? null : metadata.hostVersion())
        + " layerCount=" + (metadata == null ? "<null>" : Integer.toString(metadata.layerCount()))
        + " unit=" + (metadata == null ? "<null>" : String.valueOf(metadata.unit()))
        + " resolution=" + (metadata == null ? "<null>" : Integer.toString(metadata.resolution()))
        + " snapAngle=" + (metadata == null ? "<null>" : String.valueOf(metadata.snapAngle()))
        + " routerSettings=" + (metadata == null || metadata.routerSettings() == null
            ? "<null>" : "<present>"));
    out.println("[s8] warnings count=" + warnings.size());
    for (int i = 0; i < warnings.size(); i++) {
      out.println("[s8] warning " + i + " " + escape(warnings.get(i)));
    }

    // --- Task 9's surface ------------------------------------------------------------------
    out.println("[s9] padstacks count=" + board.library.padstacks.count());
    for (int i = 1; i <= board.library.padstacks.count(); i++) {
      Padstack p = board.library.padstacks.get(i);
      out.println("[s9] padstack " + i + " name=" + escape(p.name)
          + " fromLayer=" + p.fromLayer() + " toLayer=" + p.toLayer());
    }
    out.println("[s9] packages count=" + board.library.packages.count());
    out.println("[s9] components count=" + board.components.count());
    int items = 0;
    for (Item item : board.getItems()) {
      items++;
      if (item == null) {
        throw new IllegalStateException("unreachable");
      }
    }
    out.println("[s9] items count=" + items);
  }

  // ===============================================================================================
  // helpers
  // ===============================================================================================

  /** `CoordinateTransform`'s three fields are private (CoordinateTransform.java:22-24). */
  private static double transformField(Communication comm, String name) {
    try {
      java.lang.reflect.Field f =
          app.freerouting.io.CoordinateTransform.class.getDeclaredField(name);
      f.setAccessible(true);
      return f.getDouble(comm.coordinateTransform);
    } catch (ReflectiveOperationException e) {
      throw new IllegalStateException(e);
    }
  }

  private static String boxOf(PolylineShape shape) {
    IntBox b = shape.boundingBox();
    return b == null ? "<null>" : (b.ll.x + "," + b.ll.y + "," + b.ur.x + "," + b.ur.y);
  }

  /** `HEADER jar=<path> size=<bytes> mtime=<epoch ms>` — which jar produced this transcript. */
  private static String header(String javaDir) {
    try {
      Path jar =
          Path.of(
              KiCadJsonReader.class
                  .getProtectionDomain()
                  .getCodeSource()
                  .getLocation()
                  .toURI());
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
