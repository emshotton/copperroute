import app.freerouting.settings.GlobalSettings;
import app.freerouting.settings.RouterSettings;
import app.freerouting.settings.SettingsMerger;
import app.freerouting.settings.SettingsSource;
import app.freerouting.settings.sources.CliSettings;
import app.freerouting.settings.sources.DefaultSettings;
import app.freerouting.settings.sources.EnvironmentVariablesSource;
import java.io.File;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.TreeMap;

/**
 * Plan 4 Task 7 probe: EnvironmentVariablesSource, CliSettings and
 * GlobalSettings.applyCommandLineArguments' legacy flag table + -de classification.
 */
public class CProbe {

  static void p(String k, Object v) {
    System.out.println(k + " = " + v);
  }

  static EnvironmentVariablesSource env(String... kv) {
    Map<String, String> m = new LinkedHashMap<>();
    for (int i = 0; i < kv.length; i += 2) {
      m.put(kv[i], kv[i + 1]);
    }
    return new EnvironmentVariablesSource(m);
  }

  static GlobalSettings gs(String... args) {
    GlobalSettings g = new GlobalSettings();
    g.applyCommandLineArguments(args);
    return g;
  }

  public static void main(String[] argv) throws Exception {
    System.out.println("# ---------- A: EnvironmentVariablesSource ----------");
    p("A.priority", new EnvironmentVariablesSource(new HashMap<>()).getPriority());
    p("A.sourceName", new EnvironmentVariablesSource(new HashMap<>()).getSourceName());
    p("A.empty.parsedCount", new EnvironmentVariablesSource(new HashMap<>()).getParsedCount());

    var a1 = env("FREEROUTING__ROUTER__MAX_PASSES", "50");
    p("A1.maxPasses", a1.getSettings().maxPasses);
    p("A1.parsedCount", a1.getParsedCount());

    var a2 = env("freerouting__router__max_passes", "50");
    p("A2.lowercase.maxPasses", a2.getSettings().maxPasses);
    p("A2.lowercase.parsedCount", a2.getParsedCount());
    p("A2.lowercase.parsedVariables", new TreeMap<>(a2.getParsedVariables()));

    var a2b = env("FreeRouting__Router__Max_Passes", "50");
    p("A2b.mixedcase.maxPasses", a2b.getSettings().maxPasses);
    p("A2b.mixedcase.parsedVariables", new TreeMap<>(a2b.getParsedVariables()));

    var a3 = env("FREEROUTING__ROUTER__OPTIMIZER__MAX_THREADS", "8");
    p("A3.optimizer.maxThreads", a3.getSettings().optimizer.maxThreads);

    var a4 = env("FREEROUTING__ROUTER__OPTIMIZER__BOARD_UPDATE_STRATEGY", "GREEDY");
    p("A4.upper.boardUpdateStrategy", a4.getSettings().optimizer.boardUpdateStrategy);
    var a4b = env("FREEROUTING__ROUTER__OPTIMIZER__BOARD_UPDATE_STRATEGY", "greedy");
    p("A4b.lower.boardUpdateStrategy", a4b.getSettings().optimizer.boardUpdateStrategy);

    var a5 = env("FREEROUTING__ROUTER__OPTIMIZER__HYBRID_RATIO", "1:1");
    p("A5.hybridRatio", a5.getSettings().optimizer.hybridRatio);
    p("A5.parsedCount", a5.getParsedCount());

    var a6 = env("FREEROUTING__ROUTER__SCORING__PREFERRED_DIRECTION_TRACE_COST", "1.5,2.0");
    double[] pd = a6.getSettings().scoring.preferredDirectionTraceCost;
    p("A6.preferredDirectionTraceCost.length", pd == null ? "null" : pd.length);
    p("A6.preferredDirectionTraceCost", pd == null ? "null" : (pd[0] + "," + pd[1]));

    var a7 = env("FREEROUTING__ROUTER__ALGORITHM", "freerouting-router-v19");
    p("A7.algorithm", a7.getSettings().algorithm);

    var a8 = env("FREEROUTING__ROUTER__SAVE_INTERMEDIATE_STAGES", "true");
    p("A8.saveIntermediateStages", a8.getSettings().saveIntermediateStages);

    var a9 = env("FREEROUTING__ROUTER__INVALID_PROPERTY", "value");
    p("A9.unknown.parsedCount", a9.getParsedCount());
    p("A9.unknown.maxPasses", a9.getSettings().maxPasses);

    var a10 = env("FREEROUTING__ROUTER__MAX_PASSES", "abc");
    p("A10.badValue.parsedCount", a10.getParsedCount());
    p("A10.badValue.maxPasses", a10.getSettings().maxPasses);

    var a11 = env("MAX_PASSES", "100", "ROUTER__MAX_PASSES", "200", "FREEROUTING__GUI__X", "1");
    p("A11.nonRouter.parsedCount", a11.getParsedCount());
    p("A11.nonRouter.maxPasses", a11.getSettings().maxPasses);

    var a12 = env("FREEROUTING__ROUTER__VIAS_ALLOWED", "false");
    p("A12.viasAllowed", a12.getSettings().viasAllowed);

    System.out.println("# ---------- B: CliSettings value consumption ----------");
    p("B.priority", new CliSettings(new String[] {}).getPriority());
    p("B.sourceName", new CliSettings(new String[] {}).getSourceName());

    var b1 = new CliSettings(new String[] {"-mp", "5"});
    p("B1.mp5.maxPasses", b1.getSettings().maxPasses);
    p("B1.mp5.parsedArguments", new TreeMap<>(b1.getParsedArguments()));

    var b2 = new CliSettings(new String[] {"-mp"});
    p("B2.mpAtEnd.maxPasses", b2.getSettings().maxPasses);
    p("B2.mpAtEnd.parsedArguments", new TreeMap<>(b2.getParsedArguments()));

    var b3 = new CliSettings(new String[] {"-mp", "-5"});
    p("B3.mpMinus5.maxPasses", b3.getSettings().maxPasses);
    p("B3.mpMinus5.parsedArguments", new TreeMap<>(b3.getParsedArguments()));

    var b4 = new CliSettings(new String[] {"-mpx", "5"});
    p("B4.mpx.maxPasses", b4.getSettings().maxPasses);
    p("B4.mpx.parsedArguments", new TreeMap<>(b4.getParsedArguments()));

    var b5 = new CliSettings(new String[] {"-mp", "0x10"});
    p("B5.mpHex.maxPasses", b5.getSettings().maxPasses);
    p("B5.mpHex.parsedArguments", new TreeMap<>(b5.getParsedArguments()));

    var b6 = new CliSettings(new String[] {"-mt", "2000"});
    p("B6.mt2000.maxThreads", b6.getSettings().maxThreads);
    p("B6.mt2000.optimizer.maxThreads", b6.getSettings().optimizer.maxThreads);

    var b7 = new CliSettings(new String[] {"--router.neck_width_um=250"});
    p("B7.neckWidthUm", b7.getSettings().getNeckWidthUm());

    var b8 = new CliSettings(new String[] {"--router.enabled=", "-de", "a.dsn", "-do", "a.ses"});
    p("B8.emptyEnabled.enabled", b8.getSettings().enabled);

    var b9 = new CliSettings(new String[] {"--gui.foo=1", "--router.max_passes=7"});
    p("B9.nonRouterIgnored.maxPasses", b9.getSettings().maxPasses);
    p("B9.nonRouterIgnored.parsedArguments", new TreeMap<>(b9.getParsedArguments()));

    var b10 = new CliSettings(new String[] {"-de", "a.dsn", "-do", "a.ses"});
    p("B10.batch.enabled", b10.getSettings().enabled);
    var b11 = new CliSettings(new String[] {"-de", "a.dsn"});
    p("B11.deOnly.enabled", b11.getSettings().enabled);
    var b12 = new CliSettings(new String[] {"-de", "a.dsn", "-do", "a.ses", "--router.enabled=false"});
    p("B12.explicitFalse.enabled", b12.getSettings().enabled);

    var b13 = new CliSettings(new String[] {"--router.max_passes"});
    p("B13.noEquals.maxPasses", b13.getSettings().maxPasses);
    var b14 = new CliSettings(new String[] {"--router.optimizer.max_threads=3"});
    p("B14.nested.optimizer.maxThreads", b14.getSettings().optimizer.maxThreads);

    System.out.println("# ---------- C: merged CLI cases (SettingsMergerTest) ----------");
    DefaultSettings defaults = new DefaultSettings();
    RouterSettings c1 =
        new SettingsMerger(
                defaults,
                new CliSettings(
                    new String[] {"--router.enabled=false", "--router.optimizer.enabled=false"}))
            .merge();
    p("C1.runRouter", c1.getRunRouter());
    p("C1.runOptimizer", c1.getRunOptimizer());

    RouterSettings jsonSettings = new RouterSettings();
    jsonSettings.enabled = false;
    SettingsSource json =
        new SettingsSource() {
          public RouterSettings getSettings() {
            return jsonSettings;
          }

          public String getSourceName() {
            return "JSON test source";
          }

          public int getPriority() {
            return 10;
          }
        };
    RouterSettings c2 =
        new SettingsMerger(
                defaults,
                json,
                new CliSettings(new String[] {"-de", "a.dsn", "-do", "a.ses"}))
            .merge();
    p("C2.runRouter", c2.getRunRouter());

    RouterSettings c3 =
        new SettingsMerger(
                defaults,
                new CliSettings(
                    new String[] {"-de", "a.dsn", "-do", "a.ses", "--router.enabled=false"}))
            .merge();
    p("C3.runRouter", c3.getRunRouter());

    RouterSettings c4 =
        new SettingsMerger(
                defaults,
                new CliSettings(
                    new String[] {"-de", "a.dsn", "--router.layers.routable=false,true"}))
            .merge();
    p("C4.runRouter", c4.getRunRouter());
    p("C4.layerCount", c4.getLayerCount());

    RouterSettings c5 =
        new SettingsMerger(defaults, new CliSettings(new String[] {"-mt", "2000"})).merge();
    p("C5.mt2000.maxThreads(validated)", c5.maxThreads);
    p("C5.mt2000.optimizer.maxThreads", c5.optimizer.maxThreads);

    System.out.println("# ---------- D: GlobalSettings legacy bridge ----------");
    var d1 = gs("-oit", "5", "-us", "global", "-is", "seq", "-hr", "2:3", "-inc", "GND, VCC");
    p("D1.oit", d1.routerSettings.optimizer.optimizationImprovementThreshold);
    p("D1.us", d1.routerSettings.optimizer.boardUpdateStrategy);
    p("D1.is", d1.routerSettings.optimizer.itemSelectionStrategy);
    p("D1.hr", d1.routerSettings.optimizer.hybridRatio);
    p("D1.inc", java.util.Arrays.toString(d1.routerSettings.ignoreNetClasses));

    var d2 = gs("-oit", "-5");
    p("D2.oitNegative", d2.routerSettings.optimizer.optimizationImprovementThreshold);
    var d2b = gs("-oit", "0");
    p("D2b.oitZero", d2b.routerSettings.optimizer.optimizationImprovementThreshold);
    var d2c = gs("-oit", "1");
    p("D2c.oit1", d2c.routerSettings.optimizer.optimizationImprovementThreshold);
    p("D2c.oit1.bits", Float.floatToIntBits(d2c.routerSettings.optimizer.optimizationImprovementThreshold));
    var d2d = gs("-oit", "33");
    p("D2d.oit33", d2d.routerSettings.optimizer.optimizationImprovementThreshold);
    p("D2d.oit33.bits", Float.floatToIntBits(d2d.routerSettings.optimizer.optimizationImprovementThreshold));

    p("D3.usHybrid", gs("-us", "HYBRID").routerSettings.optimizer.boardUpdateStrategy);
    p("D3.usBogus", gs("-us", "nonsense").routerSettings.optimizer.boardUpdateStrategy);
    p("D3.usGlobalSpaced", gs("-us", " global ").routerSettings.optimizer.boardUpdateStrategy);
    p("D4.isSequestered", gs("-is", "sequestered").routerSettings.optimizer.itemSelectionStrategy);
    p("D4.isRandom", gs("-is", "RANDOMIZE").routerSettings.optimizer.itemSelectionStrategy);
    p("D4.isBogus", gs("-is", "nonsense").routerSettings.optimizer.itemSelectionStrategy);
    p("D5.hrSpaced", gs("-hr", " 2:3 ").routerSettings.optimizer.hybridRatio);

    var d6 = gs("-mp", "100000");
    p("D6.mpClampHigh", d6.routerSettings.maxPasses);
    var d7 = gs("-mp", "0x10");
    p("D7.mpDecodeHex", d7.routerSettings.maxPasses);
    var d8 = gs("-mp", "010");
    p("D8.mpDecodeOctal", d8.routerSettings.maxPasses);
    var d9 = gs("-mp", "0");
    p("D9.mpZero", d9.routerSettings.maxPasses);
    var d10 = gs("-mpx", "5");
    p("D10.mpxPrefixMatches", d10.routerSettings.maxPasses);
    var d11 = gs("-mt", "2000");
    p("D11.mtClampHigh", d11.routerSettings.optimizer.maxThreads);
    p("D11.mtClampHigh.routerMaxThreads", d11.routerSettings.maxThreads);
    var d12 = gs("-mt", "-3");
    p("D12.mtNegative(notConsumed)", d12.routerSettings.optimizer.maxThreads);
    var d13 = gs("-drc");
    p("D13.drc.routerEnabled", d13.routerSettings.enabled);
    p("D13.drc.drcEnabled", d13.drcSettings.enabled);
    var d14 = gs("-dr", "x.rules");
    p("D14.dr.routerEnabled", d14.routerSettings.enabled);
    p("D14.dr.initialRulesFile", d14.initialRulesFile);
    var d15 = gs("-inc", "GND");
    p("D15.incSingle", java.util.Arrays.toString(d15.routerSettings.ignoreNetClasses));
    var d16 = gs("-inc", "GND,");
    p("D16.incTrailingComma", java.util.Arrays.toString(d16.routerSettings.ignoreNetClasses));
    var d17 = gs("-mp");
    p("D17.mpAtEnd", d17.routerSettings.maxPasses);
    var d18 = gs("-mp", "notanumber", "-hr", "9:9");
    p("D18.mpBad.maxPasses", d18.routerSettings.maxPasses);
    p("D18.mpBad.hybridRatio", d18.routerSettings.optimizer.hybridRatio);


    var d19 = gs("-incoming", "GND");
    p("D19.incPrefix", java.util.Arrays.toString(d19.routerSettings.ignoreNetClasses));
    p("D20.mtZero", gs("-mt", "0").routerSettings.optimizer.maxThreads);
    p("D21.mt1024", gs("-mt", "1024").routerSettings.optimizer.maxThreads);
    p("D22.mp9999", gs("-mp", "9999").routerSettings.maxPasses);
    var d23 = gs("-drc", "report.json");
    p("D23.drcWithReport.routerEnabled", d23.routerSettings.enabled);
    p("D23.drcWithReport.drcEnabled", d23.drcSettings.enabled);
    p("D24.doOnly.enabled", new CliSettings(new String[] {"-do", "a.ses"}).getSettings().enabled);
    p("D25.mt5.cli", new CliSettings(new String[] {"-mt", "5"}).getSettings().maxThreads);

    System.out.println("# ---------- E: -de classification ----------");
    String[][] deCases = {
      {"-de", "myboard.dsn"},
      {"-de", "myboard.dsn+myboard.ses"},
      {"-de", "myboard.dsn+myboard.rules"},
      {"-de", "myboard.dsn+myboard.ses+myboard.rules"},
      {"-de", "myboard.rules+myboard.dsn+myboard.ses"},
      {"-de", "myboard.dsn", "myboard.ses", "myboard.rules"},
      {"-de", "sonde xilinx.dsn"},
      {"-de", "myboard.dsn+myboard.ses", "myboard.rules"},
      {"-de", "/path/to/myboard.dsn+/path/to/myboard.ses"},
      {"-de", "myboard.DSN+myboard.SES+myboard.RULES"},
      {"-de", "myboard.Dsn+myboard.Ses"},
      {"-de", "board1.dsn+board2.dsn"},
      {"-de", "myboard.ses"},
      {"-de", "myboard.rules"},
      {"-de"},
      {"-de", "myboard.dsn+myboard.ses", "-do", "output.ses", "-mp", "10"},
      {"-de", " a.dsn + b.ses "},
      {"-de", "a.txt"},
      {"-de", "a.txt+b.dsn"},
      {"-de", "a.json"},
      {"-de", "a.dsn+b.json"},
      {"-de", "a.json+b.json"},
      {"-de", "a.json+b.dsn"},
      {"-de", "a.ses+b.ses"},
      {"-de", "a.rules+b.rules"},
      {"-de", "a.dsn", "-de", "b.dsn"},
      {"-de", "+"},
      {"-de", "a.dsn+"},
      {"-dexyz", "a.dsn"},
      {"--de", "a.dsn"},
    };
    for (String[] c : deCases) {
      GlobalSettings g = gs(c);
      System.out.println(
          "E "
              + java.util.Arrays.toString(c)
              + " -> in="
              + g.initialInputFile
              + " ses="
              + g.designSessionFilename
              + " rules="
              + g.initialRulesFile
              + " out="
              + g.initialOutputFile);
    }

    // A real file whose name contains '+' must survive verbatim.
    File tmp = File.createTempFile("cprobe+plus", ".dsn");
    tmp.deleteOnExit();
    GlobalSettings gplus = gs("-de", tmp.getAbsolutePath());
    System.out.println("E.realFileWithPlus name = " + tmp.getName());
    System.out.println(
        "E.realFileWithPlus -> in="
            + gplus.initialInputFile
            + " (equalsPath="
            + tmp.getAbsolutePath().equals(gplus.initialInputFile)
            + ")");
    // The same path when it does NOT exist: split on '+'.
    String ghost = tmp.getAbsolutePath() + ".ghost";
    GlobalSettings gghost = gs("-de", ghost);
    System.out.println(
        "E.missingFileWithPlus -> in=" + gghost.initialInputFile + " ses=" + gghost.designSessionFilename
            + " rules=" + gghost.initialRulesFile);
  }
}
