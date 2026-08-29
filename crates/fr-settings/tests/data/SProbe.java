import app.freerouting.settings.RouterSettings;
import app.freerouting.settings.SettingsMerger;
import app.freerouting.settings.SettingsSource;
import app.freerouting.settings.sources.ApiSettings;
import app.freerouting.settings.sources.DefaultSettings;
import app.freerouting.settings.sources.DsnFileSettings;
import app.freerouting.settings.sources.RulesFileSettings;
import app.freerouting.settings.sources.SesFileSettings;
import java.io.File;
import java.io.FileInputStream;
import java.io.InputStream;
import java.lang.reflect.Field;
import java.util.Arrays;
import java.util.List;

/**
 * Plan 4 Task 6 probe: `SettingsMerger`, `SettingsSource` and the five in-scope
 * `settings/sources/**` classes, as the clone-HEAD jar really runs them.
 *
 * <p>Run with `-XX:ActiveProcessorCount=4` so every `availableProcessors()`-derived default
 * matches `HostEnvironment::with_processors(4)`. See `crates/fr-settings/tests/data/README.md`
 * for the recorded command.
 */
public class SProbe {

  static String s(Object o) {
    if (o == null) {
      return "null";
    }
    if (o instanceof String[] a) {
      return Arrays.toString(a);
    }
    if (o instanceof double[] a) {
      return Arrays.toString(a);
    }
    return o.toString();
  }

  static void p(String key, Object value) {
    System.out.println(key + " = " + s(value));
  }

  /** Every field of a RouterSettings, in RouterSettings.java declaration order. */
  static void dump(String tag, RouterSettings r) {
    if (r == null) {
      p(tag, null);
      return;
    }
    p(tag + ".enabled", r.enabled);
    p(tag + ".algorithm", r.algorithm);
    p(tag + ".copperToEdgeClearanceUm", r.copperToEdgeClearanceUm);
    p(tag + ".holeClearanceUm", r.holeClearanceUm);
    p(tag + ".neckWidthUm", r.neckWidthUm);
    p(tag + ".strictDrc", r.strictDrc);
    p(tag + ".jobTimeoutString", r.jobTimeoutString);
    p(tag + ".maxPasses", r.maxPasses);
    p(tag + ".maxItems", r.maxItems);
    p(tag + ".saveIntermediateStages", r.saveIntermediateStages);
    p(tag + ".ignoreNetClasses", r.ignoreNetClasses);
    p(tag + ".tracePullTightAccuracy", r.tracePullTightAccuracy);
    p(tag + ".viasAllowed", r.viasAllowed);
    p(tag + ".automaticNeckdown", r.automaticNeckdown);
    p(tag + ".maxThreads", r.maxThreads);
    p(tag + ".resultJsonPath", r.resultJsonPath);
    p(tag + ".layerCount", r.getLayerCount());
    if (r.layers != null) {
      for (int i = 0; i < r.layers.length; i++) {
        p(
            tag + ".layers[" + i + "]",
            "routable="
                + s(r.layers[i] == null ? null : r.layers[i].routable)
                + " prefHoriz="
                + s(r.layers[i] == null ? null : r.layers[i].preferredDirectionHorizontal)
                + " bendCost="
                + s(r.layers[i] == null ? null : r.layers[i].bendCost));
      }
    }
    if (r.fanout == null) {
      p(tag + ".fanout", null);
    } else {
      p(tag + ".fanout.enabled", r.fanout.enabled);
      p(tag + ".fanout.maxPasses", r.fanout.maxPasses);
      p(tag + ".fanout.maxItems", r.fanout.maxItems);
      p(tag + ".fanout.maxMillisecondsPerPin", r.fanout.maxMillisecondsPerPin);
      p(tag + ".fanout.ripupAllowed", r.fanout.ripupAllowed);
      p(tag + ".fanout.minEscapeLengthMm", r.fanout.minEscapeLengthMm);
      p(tag + ".fanout.maxEscapeLengthMm", r.fanout.maxEscapeLengthMm);
      p(tag + ".fanout.startViaDiameterMm", r.fanout.startViaDiameterMm);
      p(tag + ".fanout.endViaDiameterMm", r.fanout.endViaDiameterMm);
      p(tag + ".fanout.pinSortingOrder", r.fanout.pinSortingOrder);
      p(tag + ".fanout.fallbackToBoardVias", r.fanout.fallbackToBoardVias);
      p(tag + ".fanout.timeoutString", r.fanout.timeoutString);
    }
    if (r.optimizer == null) {
      p(tag + ".optimizer", null);
    } else {
      p(tag + ".optimizer.enabled", r.optimizer.enabled);
      p(tag + ".optimizer.algorithm", r.optimizer.algorithm);
      p(tag + ".optimizer.maxPasses", r.optimizer.maxPasses);
      p(tag + ".optimizer.maxItems", r.optimizer.maxItems);
      p(tag + ".optimizer.maxThreads", r.optimizer.maxThreads);
      p(
          tag + ".optimizer.optimizationImprovementThreshold",
          r.optimizer.optimizationImprovementThreshold);
      p(tag + ".optimizer.maxConsecutiveFailures", r.optimizer.maxConsecutiveFailures);
      p(
          tag + ".optimizer.additionalRipupCostFactorAtStart",
          r.optimizer.additionalRipupCostFactorAtStart);
      p(tag + ".optimizer.traceRipupCostFactor", r.optimizer.traceRipupCostFactor);
      p(tag + ".optimizer.maxAutoroutePasses", r.optimizer.maxAutoroutePasses);
      p(tag + ".optimizer.boardUpdateStrategy", r.optimizer.boardUpdateStrategy);
      p(tag + ".optimizer.hybridRatio", r.optimizer.hybridRatio);
      p(tag + ".optimizer.itemSelectionStrategy", r.optimizer.itemSelectionStrategy);
      p(tag + ".optimizer.timeoutString", r.optimizer.timeoutString);
    }
    if (r.scoring == null) {
      p(tag + ".scoring", null);
    } else {
      p(tag + ".scoring.preferredDirectionTraceCost", r.scoring.preferredDirectionTraceCost);
      p(tag + ".scoring.undesiredDirectionTraceCost", r.scoring.undesiredDirectionTraceCost);
      p(
          tag + ".scoring.defaultPreferredDirectionTraceCost",
          r.scoring.defaultPreferredDirectionTraceCost);
      p(
          tag + ".scoring.defaultUndesiredDirectionTraceCost",
          r.scoring.defaultUndesiredDirectionTraceCost);
      p(tag + ".scoring.viaCosts", r.scoring.viaCosts);
      p(tag + ".scoring.planeViaCosts", r.scoring.planeViaCosts);
      p(tag + ".scoring.startRipupCosts", r.scoring.startRipupCosts);
      p(tag + ".scoring.unroutedNetPenalty", r.scoring.unroutedNetPenalty);
      p(tag + ".scoring.clearanceViolationPenalty", r.scoring.clearanceViolationPenalty);
      p(tag + ".scoring.bendPenalty", r.scoring.bendPenalty);
      p(tag + ".scoring.defaultBendCost", r.scoring.defaultBendCost);
    }
  }

  /** The accessor view RulesFileSettingsTest asserts on. */
  static void accessors(String tag, RouterSettings r) {
    p(tag + ".getRunRouter", r.getRunRouter());
    p(tag + ".getRunOptimizer", r.getRunOptimizer());
    p(tag + ".getViasAllowed", r.getViasAllowed());
    p(tag + ".getViaCosts", r.getViaCosts());
    p(tag + ".getPlaneViaCosts", r.getPlaneViaCosts());
    p(tag + ".getStartRipupCosts", r.getStartRipupCosts());
    p(tag + ".getLayerCount", r.getLayerCount());
    for (int i = 0; i < r.getLayerCount(); i++) {
      p(
          tag + ".layer" + i,
          "active="
              + r.getLayerActive(i)
              + " prefHoriz="
              + r.getPreferredDirectionIsHorizontal(i)
              + " pref="
              + r.getPreferredDirectionTraceCosts(i)
              + " against="
              + r.getAgainstPreferredDirectionTraceCosts(i)
              + " bend="
              + r.getBendCost(i));
    }
  }

  static SettingsSource nullSource(int priority) {
    return new SettingsSource() {
      @Override
      public RouterSettings getSettings() {
        return null;
      }

      @Override
      public String getSourceName() {
        return "Null Source";
      }

      @Override
      public int getPriority() {
        return priority;
      }
    };
  }

  @SuppressWarnings("unchecked")
  static int sourceCount(SettingsMerger m) throws Exception {
    Field f = SettingsMerger.class.getDeclaredField("sources");
    f.setAccessible(true);
    return ((List<SettingsSource>) f.get(m)).size();
  }

  public static void main(String[] args) throws Exception {
    System.out.println("# availableProcessors = " + Runtime.getRuntime().availableProcessors());

    // ---- A: DefaultSettings, field by field ----------------------------------------------
    System.out.println("## A DefaultSettings.getSettings()");
    DefaultSettings defaults = new DefaultSettings();
    dump("A", defaults.getSettings());
    p("A.getSourceName", defaults.getSourceName());
    p("A.getPriority", defaults.getPriority());
    p("A.sameInstance", defaults.getSettings() == defaults.getSettings());

    // ---- B: SettingsMergerTest's non-CLI/env cases ----------------------------------------
    System.out.println("## B SettingsMerger");
    RouterSettings b1 = new SettingsMerger(new DefaultSettings()).merge();
    p("B.defaultSettingsOnly.maxPasses", b1.maxPasses);
    p("B.defaultSettingsOnly.enabled", b1.enabled);
    p("B.defaultSettingsOnly.viasAllowed", b1.viasAllowed);
    p("B.defaultSettingsOnly.getRunOptimizer", b1.getRunOptimizer());
    p("B.defaultSettingsOnly.maxThreads", b1.maxThreads);
    p("B.defaultSettingsOnly.layerCount", b1.getLayerCount());

    RouterSettings b2 = new SettingsMerger().merge();
    p("B.emptySourcesList.maxPasses", b2.maxPasses);
    p("B.emptySourcesList.fanoutIsNull", b2.fanout == null);
    p("B.emptySourcesList.optimizerIsNull", b2.optimizer == null);
    p("B.emptySourcesList.scoringIsNull", b2.scoring == null);

    RouterSettings b3 = new SettingsMerger(new DefaultSettings(), nullSource(100)).merge();
    p("B.nullValueHandling.maxPasses", b3.maxPasses);

    try {
      RouterSettings b4 = new SettingsMerger(nullSource(100)).merge();
      p("B.onlyNullSource", "no throw, maxPasses=" + b4.maxPasses);
    } catch (Throwable t) {
      p("B.onlyNullSource", t.getClass().getName());
    }

    // A source whose priority ties with DefaultSettings, registered after it: stable sort keeps
    // DefaultSettings first, so this one is applied on top rather than becoming the base.
    RouterSettings tie = new RouterSettings();
    tie.maxPasses = 42;
    SettingsSource tieSource =
        new SettingsSource() {
          @Override
          public RouterSettings getSettings() {
            return tie;
          }

          @Override
          public String getSourceName() {
            return "Tie Source";
          }

          @Override
          public int getPriority() {
            return 0;
          }
        };
    p(
        "B.stableSortTie.maxPasses",
        new SettingsMerger(new DefaultSettings(), tieSource).merge().maxPasses);

    // ---- C: addOrReplaceSources ------------------------------------------------------------
    System.out.println("## C addOrReplaceSources");
    SettingsMerger m = new SettingsMerger(new DefaultSettings());
    p("C.afterCtor", sourceCount(m));
    m.addOrReplaceSources(new DefaultSettings());
    p("C.afterSameClass", sourceCount(m));
    m.addOrReplaceSources(new SesFileSettings("a.ses"));
    p("C.afterOtherClass", sourceCount(m));
    m.addOrReplaceSources(new SesFileSettings("b.ses"));
    p("C.afterSameClassAgain", sourceCount(m));

    // ---- D: the priority ladder + source names ---------------------------------------------
    System.out.println("## D source names and priorities");
    SesFileSettings ses = new SesFileSettings("board.ses");
    p("D.ses.getPriority", ses.getPriority());
    p("D.ses.getSourceName", ses.getSourceName());
    p("D.ses.settingsIsNull", ses.getSettings() == null);
    p("D.ses.maxPasses", ses.getSettings().maxPasses);
    p("D.ses.layerCount", ses.getSettings().getLayerCount());

    ApiSettings apiNull = new ApiSettings(null);
    p("D.apiNull.getPriority", apiNull.getPriority());
    p("D.apiNull.getSourceName", apiNull.getSourceName());
    p("D.apiNull.maxPasses", apiNull.getSettings().maxPasses);
    RouterSettings apiPayload = new RouterSettings();
    apiPayload.maxPasses = 7;
    p("D.api.maxPasses", new ApiSettings(apiPayload).getSettings().maxPasses);

    RulesFileSettings missing = new RulesFileSettings("dummy.rules");
    p("D.rulesMissing.getPriority", missing.getPriority());
    p("D.rulesMissing.getSourceName", missing.getSourceName());
    p("D.rulesMissing.maxPasses", missing.getSettings().maxPasses);
    p("D.rulesMissing.layerCount", missing.getSettings().getLayerCount());

    // ---- E: RulesFileSettings over the two goldens -----------------------------------------
    System.out.println("## E RulesFileSettings goldens");
    String fixtures = args.length > 0 ? args[0] : "fixtures";
    File processor = new File(fixtures, "Issue191-processor.Z80/processor.rules");
    try (InputStream in = new FileInputStream(processor)) {
      RulesFileSettings src = new RulesFileSettings(in, "processor.rules");
      p("E.processor.getSourceName", src.getSourceName());
      p("E.processor.getPriority", src.getPriority());
      accessors("E.processor", src.getSettings());
      dump("E.processorRaw", src.getSettings());
    }
    File hw48na = new File(fixtures, "Issue029-hw48na_valid.rules");
    RulesFileSettings hw = new RulesFileSettings(hw48na);
    p("E.hw48na.getSourceName", hw.getSourceName());
    accessors("E.hw48na", hw.getSettings());
    dump("E.hw48naRaw", hw.getSettings());

    // ---- F: DsnFileSettings, the Q18 seeding (docs/java-quirks.md #128) --------------------
    System.out.println("## F DsnFileSettings");
    for (String name :
        new String[] {
          "Issue413-test.dsn", "Issue066-Project_GP8B.dsn", "Issue026-J2_reference.dsn"
        }) {
      try (InputStream in = new FileInputStream(new File(fixtures, name))) {
        DsnFileSettings src = new DsnFileSettings(in, name);
        p("F." + name + ".getPriority", src.getPriority());
        p("F." + name + ".getSourceName", src.getSourceName());
        dump("F." + name, src.getSettings());
      }
    }

    // ---- H: absence vs. the coalesced default — Task 6 fix round 1, controller ruling L ----
    // `Issue029-hw48na_reduced.rules` is `Issue029-hw48na_valid.rules` with eight lines deleted:
    // `(vias on)`, `(via_costs 50)`, `(plane_via_costs 5)`, `(start_ripup_costs 100)` and the
    // four per-layer trace-cost lines. Java's readScope calls no setter for any of them, so the
    // fields stay null and `boardSpecificTraceCostsApplied` stays at the `false` setLayerCount
    // left. Both `(preferred_direction …)` lines are kept, so the per-layer directions still land.
    System.out.println("## H absence vs. default (reduced rules file)");
    String probeData = args.length > 1 ? args[1] : ".";
    File reduced = new File(probeData, "Issue029-hw48na_reduced.rules");
    RulesFileSettings reducedSource = new RulesFileSettings(reduced);
    RouterSettings h = reducedSource.getSettings();
    p("H.reduced.raw.viasAllowed", h.viasAllowed);
    p("H.reduced.raw.scoring.viaCosts", h.scoring == null ? null : h.scoring.viaCosts);
    p("H.reduced.raw.scoring.planeViaCosts", h.scoring == null ? null : h.scoring.planeViaCosts);
    p("H.reduced.raw.scoring.startRipupCosts", h.scoring == null ? null : h.scoring.startRipupCosts);
    p("H.reduced.raw.enabled", h.enabled);
    p(
        "H.reduced.raw.optimizer.enabled",
        h.optimizer == null ? null : h.optimizer.enabled);
    p("H.reduced.raw.layerCount", h.getLayerCount());
    p(
        "H.reduced.raw.scoring.preferredDirectionTraceCost",
        h.scoring == null ? null : h.scoring.preferredDirectionTraceCost);
    p(
        "H.reduced.raw.scoring.undesiredDirectionTraceCost",
        h.scoring == null ? null : h.scoring.undesiredDirectionTraceCost);
    for (int i = 0; i < h.getLayerCount(); i++) {
      p(
          "H.reduced.raw.layers[" + i + "]",
          "routable="
              + s(h.layers[i].routable)
              + " prefHoriz="
              + s(h.layers[i].preferredDirectionHorizontal));
    }
    p("H.reduced.raw.areBoardSpecificTraceCostsApplied", h.areBoardSpecificTraceCostsApplied());

    RouterSettings hMerged =
        new SettingsMerger(new DefaultSettings(), new RulesFileSettings(reduced)).merge();
    p("H.reduced.merged.getViaCosts", hMerged.getViaCosts());
    p("H.reduced.merged.getPlaneViaCosts", hMerged.getPlaneViaCosts());
    p("H.reduced.merged.getStartRipupCosts", hMerged.getStartRipupCosts());
    p("H.reduced.merged.getViasAllowed", hMerged.getViasAllowed());
    p("H.reduced.merged.layerCount", hMerged.getLayerCount());
    p(
        "H.reduced.merged.areBoardSpecificTraceCostsApplied",
        hMerged.areBoardSpecificTraceCostsApplied());

    // The positive half: the unmodified file names both trace costs, so the setter *does* run
    // and the flag comes out true.
    RouterSettings hFull =
        new RulesFileSettings(new File(fixtures, "Issue029-hw48na_valid.rules")).getSettings();
    p("H.full.raw.areBoardSpecificTraceCostsApplied", hFull.areBoardSpecificTraceCostsApplied());
    p("H.full.raw.scoring.viaCosts", hFull.scoring.viaCosts);

    // And what a `.rules` file with no `(autoroute_settings …)` scope at all gives: readScope
    // never runs, so RulesFileSettings falls back to a blank `new RouterSettings()`.
    p("H.blank.layerCount", new RulesFileSettings("nope.rules").getSettings().getLayerCount());

    // ---- G: a DSN source merged under DefaultSettings — quirk Q18's (#128) consequence -----
    System.out.println("## G Q18: the DSN source's seeded arrays block later sources");
    try (InputStream in = new FileInputStream(new File(fixtures, "Issue066-Project_GP8B.dsn"))) {
      SettingsMerger g =
          new SettingsMerger(
              new DefaultSettings(), new DsnFileSettings(in, "Issue066-Project_GP8B.dsn"));
      dump("G.merged", g.merge());
    }
  }
}
