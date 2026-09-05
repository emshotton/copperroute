import app.freerouting.settings.*;
import java.lang.reflect.Field;

public class VProbe {
  static RouterSettings mk() { return new RouterSettings(); }

  static String v(Object o) { return o == null ? "null" : o.toString(); }

  public static void main(String[] a) throws Exception {
    System.out.println("availableProcessors = " + Runtime.getRuntime().availableProcessors());

    RouterSettings s = mk();
    s.setLayerCount(2);
    s.algorithm = "alg";
    s.jobTimeoutString = "1:00:00";
    s.maxPasses = 42;
    s.maxItems = 7;
    s.saveIntermediateStages = true;
    s.copperToEdgeClearanceUm = 11.0;
    s.holeClearanceUm = 12.0;
    s.neckWidthUm = 130.0;
    s.strictDrc = true;
    s.ignoreNetClasses = new String[] {"GND"};
    s.tracePullTightAccuracy = 33;
    s.enabled = false;
    s.viasAllowed = false;
    s.automaticNeckdown = true;
    s.maxThreads = 3;
    s.resultJsonPath = "/tmp/result.json";
    s.setBendCost(0, 2.5);
    s.setPreferredDirectionTraceCosts(0, 2.5);
    s.scoring.defaultBendCost = 1.25;
    s.optimizer.maxPasses = 55;
    s.fanout.maxPasses = 66;
    RouterSettings c = s.clone();
    System.out.println("A.resultJsonPath(src)  = " + v(s.resultJsonPath));
    System.out.println("A.resultJsonPath(clone)= " + v(c.resultJsonPath));
    System.out.println("A.algorithm=" + v(c.algorithm) + " jobTimeout=" + v(c.jobTimeoutString)
        + " maxPasses=" + v(c.maxPasses) + " maxItems=" + v(c.maxItems)
        + " saveStages=" + v(c.saveIntermediateStages) + " cu2edge=" + v(c.copperToEdgeClearanceUm)
        + " hole=" + v(c.holeClearanceUm) + " neck=" + v(c.neckWidthUm)
        + " strict=" + v(c.strictDrc) + " tpta=" + v(c.tracePullTightAccuracy)
        + " enabled=" + v(c.enabled) + " vias=" + v(c.viasAllowed)
        + " neckdown=" + v(c.automaticNeckdown) + " maxThreads=" + v(c.maxThreads));
    System.out.println("A.ignoreNetClasses = " + java.util.Arrays.toString(c.ignoreNetClasses));
    System.out.println("A.layerCount = " + c.getLayerCount() + " bendCost0=" + c.getBendCost(0)
        + " prefTrace0=" + c.getPreferredDirectionTraceCosts(0)
        + " defaultBendCost=" + v(c.scoring.defaultBendCost)
        + " opt.maxPasses=" + v(c.optimizer.maxPasses) + " fanout.maxPasses=" + v(c.fanout.maxPasses));
    Field f = RouterSettings.class.getDeclaredField("boardSpecificTraceCostsApplied");
    f.setAccessible(true);
    System.out.println("A.applied(src)=" + v(f.get(s)) + " applied(clone)=" + v(f.get(c)));
    System.out.println("A.layersNotSame = " + (c.layers != s.layers));

    RouterSettings n = mk(); n.setLayerCount(2); n.scoring = null;
    RouterSettings nc = n.clone();
    System.out.println("A.nullScoringClone.scoringNotNull = " + (nc.scoring != null));

    Integer[] mp = {-1, 10000, 0, 50};
    for (Integer x : mp) {
      RouterSettings t = mk(); t.maxPasses = x; t.tracePullTightAccuracy = 500; t.maxThreads = 2;
      t.validate();
      System.out.println("B.maxPasses " + x + " -> " + t.maxPasses);
    }
    Integer[] mt = {null, -1, 0, 9, 2};
    for (Integer x : mt) {
      RouterSettings t = mk(); t.maxPasses = 50; t.tracePullTightAccuracy = 500; t.maxThreads = x;
      t.validate();
      System.out.println("B.maxThreads " + v(x) + " -> " + t.maxThreads);
    }
    Integer[] tp = {0, 1, 500};
    for (Integer x : tp) {
      RouterSettings t = mk(); t.maxPasses = 50; t.tracePullTightAccuracy = x; t.maxThreads = 2;
      t.validate();
      System.out.println("B.tpta " + x + " -> " + t.tracePullTightAccuracy);
    }
    try { RouterSettings t = mk(); t.tracePullTightAccuracy = 500; t.validate();
          System.out.println("B.nullMaxPasses -> no throw, " + v(t.maxPasses)); }
    catch (Throwable e) { System.out.println("B.nullMaxPasses -> " + e.getClass().getName()); }
    try { RouterSettings t = mk(); t.maxPasses = 50; t.validate();
          System.out.println("B.nullTpta -> no throw, " + v(t.tracePullTightAccuracy)); }
    catch (Throwable e) { System.out.println("B.nullTpta -> " + e.getClass().getName()); }

    for (Integer x : mt) {
      RouterSettings t = mk(); t.setMaxThreads(x);
      System.out.println("C.setMaxThreads " + v(x) + " -> " + v(t.maxThreads)
          + " optimizer.maxThreads=" + v(t.optimizer.maxThreads));
    }
    RouterSettings nullOpt = mk(); nullOpt.optimizer = null; nullOpt.setMaxThreads(0);
    System.out.println("C.nullOptimizer -> maxThreads=" + v(nullOpt.maxThreads)
        + " optimizerStillNull=" + (nullOpt.optimizer == null));

    RouterSettings d = mk();
    d.setLayerCount(2);
    d.setPreferredDirectionTraceCosts(0, 2.5);
    d.setAgainstPreferredDirectionTraceCosts(1, 3.5);
    d.setBendCost(0, 4.0);
    d.setLayerActive(1, false);
    System.out.println("D.before applied=" + v(f.get(d)) + " pref0=" + d.getPreferredDirectionTraceCosts(0)
        + " und1=" + d.getAgainstPreferredDirectionTraceCosts(1)
        + " bend0=" + d.getBendCost(0) + " active1=" + d.getLayerActive(1));
    d.setLayerCount(2);
    System.out.println("D.after(same) applied=" + v(f.get(d)) + " pref0=" + d.getPreferredDirectionTraceCosts(0)
        + " und1=" + d.getAgainstPreferredDirectionTraceCosts(1)
        + " bend0=" + d.getBendCost(0) + " active1=" + d.getLayerActive(1));
    d.setLayerCount(4);
    System.out.println("D.after(diff) applied=" + v(f.get(d)) + " layerCount=" + d.getLayerCount());

    RouterSettings e = mk();
    e.layers = new LayerSettings[] { new LayerSettings(), new LayerSettings() };
    e.scoring = new ScoringSettings(); 
    System.out.println("E.getPreferredDirectionTraceCosts(0) = " + e.getPreferredDirectionTraceCosts(0));
    System.out.println("E.getAgainstPreferredDirectionTraceCosts(0) = " + e.getAgainstPreferredDirectionTraceCosts(0));
    try { System.out.println("E.getHorizontalTraceCosts(0) = " + e.getHorizontalTraceCosts(0)); }
    catch (Throwable t) { System.out.println("E.getHorizontalTraceCosts(0) -> " + t.getClass().getName()); }
    try { System.out.println("E.getVerticalTraceCosts(0) = " + e.getVerticalTraceCosts(0)); }
    catch (Throwable t) { System.out.println("E.getVerticalTraceCosts(0) -> " + t.getClass().getName()); }
    System.out.println("E.getTraceCosts().length (arrays null) = " + e.getTraceCosts().length);
    try { RouterSettings e2 = mk(); System.out.println("E.getTraceCosts on new = " + e2.getTraceCosts().length); }
    catch (Throwable t) { System.out.println("E.getTraceCosts on new -> " + t.getClass().getName()); }
    System.out.println("E.getHorizontalTraceCosts(-1) = " + e.getHorizontalTraceCosts(-1));
    System.out.println("E.getHorizontalTraceCosts(9) = " + e.getHorizontalTraceCosts(9));

    RouterSettings g = mk(); g.setLayerCount(2);
    g.setPreferredDirectionTraceCosts(0, 2.0); g.setAgainstPreferredDirectionTraceCosts(0, 3.0);
    g.setPreferredDirectionTraceCosts(1, 4.0); g.setAgainstPreferredDirectionTraceCosts(1, 5.0);
    System.out.println("E.layer0 prefIsHoriz=" + g.getPreferredDirectionIsHorizontal(0)
        + " h=" + g.getHorizontalTraceCosts(0) + " v=" + g.getVerticalTraceCosts(0));
    System.out.println("E.layer1 prefIsHoriz=" + g.getPreferredDirectionIsHorizontal(1)
        + " h=" + g.getHorizontalTraceCosts(1) + " v=" + g.getVerticalTraceCosts(1));
    AutorouteControlShim.print(g);

    RouterSettings x = mk();
    System.out.println("F.runRouter=" + x.getRunRouter() + " runOptimizer=" + x.getRunOptimizer()
        + " viasAllowed=" + x.getViasAllowed() + " viaCosts=" + x.getViaCosts()
        + " planeViaCosts=" + x.getPlaneViaCosts() + " startRipup=" + x.getStartRipupCosts()
        + " neck=" + x.getNeckWidthUm() + " strictDrc=" + x.isStrictDrc()
        + " autoNeckdown=" + x.getAutomaticNeckdown() + " layerCount=" + x.getLayerCount());
    System.out.println("F.layerActive(0)=" + x.getLayerActive(0) + " bendCost(0)=" + x.getBendCost(0)
        + " prefHoriz(0)=" + x.getPreferredDirectionIsHorizontal(0)
        + " prefTrace(0)=" + x.getPreferredDirectionTraceCosts(0));
    RouterSettings y = mk(); y.setLayerCount(3);
    System.out.println("F.prefHoriz 0/1/2 = " + y.getPreferredDirectionIsHorizontal(0) + "/"
        + y.getPreferredDirectionIsHorizontal(1) + "/" + y.getPreferredDirectionIsHorizontal(2));
    y.layers[1] = null;
    System.out.println("F.nullElement layerActive(1)=" + y.getLayerActive(1)
        + " bendCost(1)=" + y.getBendCost(1) + " prefHoriz(1)=" + y.getPreferredDirectionIsHorizontal(1));
    y.scoring.defaultBendCost = 15.0;
    System.out.println("F.defaultBendCost 15.0 -> getBendCost(0)=" + y.getBendCost(0));
    y.scoring.defaultBendCost = -3.0;
    System.out.println("F.defaultBendCost -3.0 -> getBendCost(0)=" + y.getBendCost(0));
    RouterSettings z = mk(); z.neckWidthUm = -5.0;
    System.out.println("F.neckWidthUm -5.0 -> " + z.getNeckWidthUm());
    RouterSettings w = mk(); w.setViaCosts(-4); w.setPlaneViaCosts(0); w.setStartRipupCosts(-9);
    System.out.println("F.clamps via=" + w.getViaCosts() + " plane=" + w.getPlaneViaCosts()
        + " ripup=" + w.getStartRipupCosts());
    RouterSettings q = mk(); q.setLayerCount(2); q.setPreferredDirectionTraceCosts(0, 0.05);
    System.out.println("F.prefTraceClamp = " + q.getPreferredDirectionTraceCosts(0)
        + " applied=" + v(f.get(q)));
    RouterSettings q2 = mk(); q2.setLayerCount(2); q2.setAgainstPreferredDirectionTraceCosts(0, -1.0);
    System.out.println("F.undesiredClamp = " + q2.getAgainstPreferredDirectionTraceCosts(0)
        + " applied=" + v(f.get(q2)));
    RouterSettings q3 = mk(); q3.setPreferredDirectionTraceCosts(0, 9.0);
    System.out.println("F.setterOutOfRange applied=" + v(f.get(q3)) + " scoringPref="
        + (q3.scoring.preferredDirectionTraceCost == null ? "null" : "len " + q3.scoring.preferredDirectionTraceCost.length));
    RouterSettings q4 = mk(); q4.setLayerCount(2);
    q4.scoring.preferredDirectionTraceCost = new double[] {7.0};
    q4.setPreferredDirectionTraceCosts(1, 2.0);
    System.out.println("F.realloc = " + java.util.Arrays.toString(q4.scoring.preferredDirectionTraceCost));
    RouterSettings r = mk(); r.optimizer = null; r.setRunOptimizer(true);
    System.out.println("F.setRunOptimizer on null optimizer -> " + r.getRunOptimizer());
    RouterSettings r2 = mk(); r2.optimizer = null;
    System.out.println("F.getRunOptimizer null optimizer -> " + r2.getRunOptimizer());
  }
}

class AutorouteControlShim {
  static void print(RouterSettings s) {
    app.freerouting.autoroute.maze.AutorouteControl.ExpansionCostFactor[] tc = s.getTraceCosts();
    StringBuilder sb = new StringBuilder("E.getTraceCosts = ");
    for (var t : tc) sb.append("(").append(t.horizontal()).append(",").append(t.vertical()).append(") ");
    System.out.println(sb);
  }
}
