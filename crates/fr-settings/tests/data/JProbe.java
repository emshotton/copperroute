import app.freerouting.settings.FanoutSettings;
import app.freerouting.settings.LayerSettings;
import app.freerouting.settings.OptimizerSettings;
import app.freerouting.settings.RouterSettings;
import app.freerouting.settings.ScoringSettings;
import app.freerouting.autoroute.BoardUpdateStrategy;
import app.freerouting.autoroute.ItemSelectionStrategy;
import app.freerouting.util.gson.GsonProvider;

/**
 * Plan 4 Task 10 probe: what `GsonProvider.GSON` writes for a fully-populated `RouterSettings`,
 * and what it reads back from the fixtures `RouterSettingsSerializationTest` and
 * `JsonFileSettingsTest` use. Every expected string in
 * `crates/fr-settings/tests/json.rs` comes from this transcript.
 *
 * Block A pins the emitted key set and key order; block B pins Java's number formatting
 * (`Number.toString()` through `JsonWriter.value(Number)`) at the four places where it differs
 * from Rust's shortest-round-trip formatter: the 1e-3 and 1e7 scientific-notation thresholds,
 * `-0.0`, and `Float.toString` vs `Double.toString` for the same decimal. Block C is the read
 * side. Block D is Gson's lenient reader on inputs `serde_json` rejects.
 */
public final class JProbe {

  private JProbe() {}

  private static void h(String s) {
    System.out.println();
    System.out.println("== " + s + " ==");
  }

  public static void main(String[] args) {
    // ------------------------------------------------------------------ A
    h("A: fully populated, every non-transient field distinct");
    RouterSettings a = new RouterSettings();
    a.setLayerCount(2);
    a.enabled = true;
    a.algorithm = "freerouting-router-v19";
    a.copperToEdgeClearanceUm = 501.5;
    a.holeClearanceUm = 2.0;
    a.neckWidthUm = 3.0;
    a.strictDrc = true;
    a.jobTimeoutString = "12:00:00";
    a.maxPasses = 11;
    a.tracePullTightAccuracy = 12;
    a.viasAllowed = false;
    a.automaticNeckdown = false;
    a.maxThreads = 13;
    a.resultJsonPath = "/tmp/result.json";
    // transient — must NOT appear
    a.maxItems = 14;
    a.saveIntermediateStages = true;
    a.ignoreNetClasses = new String[] {"x", "y"};
    a.layers[0] = new LayerSettings(false, true);
    a.layers[1] = new LayerSettings(true, false);

    FanoutSettings f = a.fanout;
    f.enabled = false;
    f.maxPasses = 21;
    f.maxItems = 22;
    f.maxMillisecondsPerPin = 23L;
    f.ripupAllowed = false;
    f.minEscapeLengthMm = 2.25;
    f.maxEscapeLengthMm = 4.75;
    f.startViaDiameterMm = 0.5;
    f.endViaDiameterMm = 0.75;
    f.pinSortingOrder = "inner_first";
    f.fallbackToBoardVias = false;
    f.timeoutString = "00:01:00";

    OptimizerSettings o = a.optimizer;
    o.enabled = false;
    o.algorithm = "freerouting-optimizer-v19";
    o.maxPasses = 31;
    o.maxItems = 32;
    o.maxThreads = 33;
    o.optimizationImprovementThreshold = 0.02f;
    o.maxConsecutiveFailures = 34;
    o.additionalRipupCostFactorAtStart = 35;
    o.traceRipupCostFactor = 0.7f;
    o.maxAutoroutePasses = 36;
    o.timeoutString = "00:02:00";
    // transient — must NOT appear
    o.boardUpdateStrategy = BoardUpdateStrategy.HYBRID;
    o.hybridRatio = "1:1";
    o.itemSelectionStrategy = ItemSelectionStrategy.PRIORITIZED;

    ScoringSettings s = a.scoring;
    s.defaultPreferredDirectionTraceCost = 1.5;
    s.defaultUndesiredDirectionTraceCost = 2.5;
    s.viaCosts = 41;
    s.planeViaCosts = 42;
    s.startRipupCosts = 43;
    s.unroutedNetPenalty = 44.5f;
    s.clearanceViolationPenalty = 45.5f;
    s.bendPenalty = 46.5f;
    s.defaultBendCost = 3.5;
    // transient — must NOT appear
    s.preferredDirectionTraceCost = new double[] {7.0, 8.0};
    s.undesiredDirectionTraceCost = new double[] {9.0, 10.0};

    System.out.println(GsonProvider.GSON.toJson(a));

    // ------------------------------------------------------------------ B
    h("B: number formatting — Double.toString / Float.toString thresholds");
    RouterSettings b = new RouterSettings();
    b.copperToEdgeClearanceUm = 1.0e-3; // exactly the lower threshold
    b.holeClearanceUm = 9.999e-4; // just below it
    b.neckWidthUm = 1.0e7; // exactly the upper threshold
    b.scoring.defaultPreferredDirectionTraceCost = 9999999.0; // just below it
    b.scoring.defaultUndesiredDirectionTraceCost = -0.0;
    b.scoring.defaultBendCost = 1.0e300;
    b.scoring.unroutedNetPenalty = 5000000.0f;
    b.scoring.clearanceViolationPenalty = 1.0e7f;
    b.scoring.bendPenalty = 1.0e-3f;
    b.optimizer.optimizationImprovementThreshold = 9.999e-4f;
    b.optimizer.traceRipupCostFactor = 3.4028235e38f;
    b.fanout.minEscapeLengthMm = 0.1;
    b.fanout.maxEscapeLengthMm = 1.0 / 3.0;
    b.fanout.startViaDiameterMm = 123456789012345680.0;
    System.out.println(GsonProvider.GSON.toJson(b));

    h("B2: raw Number.toString for the same values");
    System.out.println("Double.toString(1.0e-3)=" + Double.toString(1.0e-3));
    System.out.println("Double.toString(9.999e-4)=" + Double.toString(9.999e-4));
    System.out.println("Double.toString(1.0e7)=" + Double.toString(1.0e7));
    System.out.println("Double.toString(9999999.0)=" + Double.toString(9999999.0));
    System.out.println("Double.toString(-0.0)=" + Double.toString(-0.0));
    System.out.println("Float.toString(5000000.0f)=" + Float.toString(5000000.0f));
    System.out.println("Float.toString(1.0e7f)=" + Float.toString(1.0e7f));
    System.out.println("Float.toString(1.0e-3f)=" + Float.toString(1.0e-3f));

    h("B3: the default settings' floats, as the differential's case 0 emits them");
    System.out.println("Float.toString(5.0e6f)=" + Float.toString(5.0e6f));
    System.out.println("Float.toString(1.0e6f)=" + Float.toString(1.0e6f));
    System.out.println("Float.toString(0.01f)=" + Float.toString(0.01f));
    System.out.println("Float.toString(0.6f)=" + Float.toString(0.6f));

    h("B4: non-finite floats under Strictness.LENIENT");
    RouterSettings nf = new RouterSettings();
    nf.holeClearanceUm = Double.NaN;
    nf.neckWidthUm = Double.POSITIVE_INFINITY;
    nf.copperToEdgeClearanceUm = Double.NEGATIVE_INFINITY;
    nf.scoring.bendPenalty = Float.NaN;
    try {
      System.out.println(GsonProvider.GSON.toJson(nf));
    } catch (RuntimeException e) {
      System.out.println("THREW " + e.getClass().getName() + ": " + e.getMessage());
    }

    // ------------------------------------------------------------------ C
    h("C: read side — RouterSettingsSerializationTest's API payload");
    String apiJson =
        """
        {
          "max_passes": 42,
          "layers": [
            {"routable": false, "preferred_direction_horizontal": true},
            {"routable": true, "preferred_direction_horizontal": false}
          ]
        }
        """;
    RouterSettings c = GsonProvider.GSON.fromJson(apiJson, RouterSettings.class);
    System.out.println("maxPasses=" + c.maxPasses);
    System.out.println("layers=" + (c.layers == null ? "null" : Integer.toString(c.layers.length)));
    if (c.layers != null) {
      for (int i = 0; i < c.layers.length; i++) {
        System.out.println(
            "layers[" + i + "].routable=" + c.layers[i].routable
                + " preferredDirectionHorizontal=" + c.layers[i].preferredDirectionHorizontal
                + " bendCost=" + c.layers[i].bendCost);
      }
    }
    System.out.println("fanout=" + c.fanout + " optimizer=" + c.optimizer + " scoring=" + c.scoring);
    System.out.println("enabled=" + c.enabled + " algorithm=" + c.algorithm);

    h("C2: the four transient keys present in the input");
    RouterSettings c2 =
        GsonProvider.GSON.fromJson(
            "{\"max_items\":99,\"layers\":[{\"routable\":false}],"
                + "\"save_intermediate_stages\":true,\"ignore_net_classes\":[\"x\",\"y\"]}",
            RouterSettings.class);
    System.out.println(
        "maxItems=" + c2.maxItems
            + " saveIntermediateStages=" + c2.saveIntermediateStages
            + " ignoreNetClasses=" + java.util.Arrays.toString(c2.ignoreNetClasses)
            + " layers=" + (c2.layers == null ? "null" : Integer.toString(c2.layers.length)));

    h("C3: the @SerializedName alternates");
    RouterSettings c3 =
        GsonProvider.GSON.fromJson(
            "{\"tracePullTightAccuracy\":7,\"automaticNeckdown\":true,"
                + "\"scoring\":{\"viaCosts\":7,\"startRipupCosts\":8},"
                + "\"fanout\":{\"ripupAllowed\":false}}",
            RouterSettings.class);
    System.out.println(
        "tracePullTightAccuracy=" + c3.tracePullTightAccuracy
            + " automaticNeckdown=" + c3.automaticNeckdown
            + " scoring.viaCosts=" + c3.scoring.viaCosts
            + " scoring.startRipupCosts=" + c3.scoring.startRipupCosts
            + " fanout.ripupAllowed=" + c3.fanout.ripupAllowed);

    h("C4: unknown keys, and an empty object");
    RouterSettings c4 =
        GsonProvider.GSON.fromJson(
            "{\"max_passes\":42,\"no_such_field\":1,\"scoring\":{\"nope\":2}}",
            RouterSettings.class);
    System.out.println(
        "maxPasses=" + c4.maxPasses + " scoring=" + (c4.scoring == null ? "null" : "present"));
    RouterSettings c5 = GsonProvider.GSON.fromJson("{}", RouterSettings.class);
    System.out.println(
        "empty: maxPasses=" + c5.maxPasses + " fanout=" + c5.fanout + " enabled=" + c5.enabled);

    // ------------------------------------------------------------------ D
    h("D: Strictness.LENIENT reader — inputs strict JSON rejects");
    for (String bad :
        new String[] {
          "{max_passes: 42}",
          "{'max_passes': 42}",
          "{\"max_passes\": \"42\"}",
          "{\"max_passes\": 42,}",
          "// c\n{\"max_passes\": 42}",
          "{\"strict_drc\": \"true\"}",
        }) {
      try {
        RouterSettings d = GsonProvider.GSON.fromJson(bad, RouterSettings.class);
        System.out.println(
            "OK   " + bad.replace("\n", "\\n") + "  -> maxPasses=" + d.maxPasses
                + " strictDrc=" + d.strictDrc);
      } catch (RuntimeException e) {
        System.out.println(
            "FAIL " + bad.replace("\n", "\\n") + "  -> " + e.getClass().getSimpleName());
      }
    }


    h("F: what the no-arg constructor leaves behind on an explicit null");
    RouterSettings f1 =
        GsonProvider.GSON.fromJson(
            "{\"fanout\":null,\"optimizer\":null,\"scoring\":null}", RouterSettings.class);
    System.out.println(
        "explicit nulls: fanout=" + f1.fanout + " optimizer=" + f1.optimizer
            + " scoring=" + f1.scoring);
    RouterSettings f2 = GsonProvider.GSON.fromJson("{}", RouterSettings.class);
    System.out.println(
        "empty object: fanout.enabled=" + f2.fanout.enabled
            + " fanout.maxPasses=" + f2.fanout.maxPasses
            + " optimizer.enabled=" + f2.optimizer.enabled
            + " optimizer.boardUpdateStrategy=" + f2.optimizer.boardUpdateStrategy
            + " scoring.viaCosts=" + f2.scoring.viaCosts
            + " scoring.preferredDirectionTraceCost="
            + java.util.Arrays.toString(f2.scoring.preferredDirectionTraceCost)
            + " layers=" + f2.layers
            + " maxItems=" + f2.maxItems);
    System.out.println("empty object re-serialised: " + GsonProvider.GSON.toJson(f2));
    RouterSettings f3 =
        GsonProvider.GSON.fromJson("{\"scoring\":{\"via_costs\":7}}", RouterSettings.class);
    System.out.println(
        "nested partial: scoring.viaCosts=" + f3.scoring.viaCosts
            + " scoring.planeViaCosts=" + f3.scoring.planeViaCosts
            + " fanout=" + (f3.fanout == null ? "null" : "present"));
    RouterSettings f4 =
        GsonProvider.GSON.fromJson(
            "{\"optimizer\":{\"board_update_strategy\":\"HYBRID\","
                + "\"hybrid_ratio\":\"1:1\",\"item_selection_strategy\":\"PRIORITIZED\"}}",
            RouterSettings.class);
    System.out.println(
        "transient optimizer keys: boardUpdateStrategy=" + f4.optimizer.boardUpdateStrategy
            + " hybridRatio=" + f4.optimizer.hybridRatio
            + " itemSelectionStrategy=" + f4.optimizer.itemSelectionStrategy);

    h("G: JsonFileSettingsTest's fixtures, through the same Gson");
    System.out.println(
        "valid router section -> maxPasses="
            + GsonProvider.GSON
                .fromJson("{\"max_passes\": 42}", RouterSettings.class)
                .maxPasses);

    h("E: round trip of A's output");
    String json = GsonProvider.GSON.toJson(a);
    RouterSettings e = GsonProvider.GSON.fromJson(json, RouterSettings.class);
    System.out.println("re-serialised equal: " + json.equals(GsonProvider.GSON.toJson(e)));
    System.out.println("layers after round trip: " + (e.layers == null ? "null" : "present"));
  }
}
