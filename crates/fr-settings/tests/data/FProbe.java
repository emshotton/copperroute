import app.freerouting.settings.RouterSettings;
import app.freerouting.util.ReflectionUtil;
import java.util.Arrays;

public class FProbe {
  static String tryset(RouterSettings s, String path, String value) {
    try {
      ReflectionUtil.setFieldValue(s, path, value);
      return "ok";
    } catch (Throwable t) {
      return t.getClass().getSimpleName() + ": " + t.getMessage();
    }
  }

  public static void main(String[] args) throws Exception {
    RouterSettings a = new RouterSettings();
    System.out.println("A. set optimizer-max_passes=7 -> " + tryset(a, "optimizer-max_passes", "7")
        + " ; optimizer.maxPasses=" + a.optimizer.maxPasses);

    RouterSettings b = new RouterSettings();
    System.out.println("B1. enabled=yes -> " + tryset(b, "enabled", "yes") + " ; enabled=" + b.enabled);
    System.out.println("B2. enabled=TRUE -> " + tryset(b, "enabled", "TRUE") + " ; enabled=" + b.enabled);
    System.out.println("B3. enabled=1 -> " + tryset(b, "enabled", "1") + " ; enabled=" + b.enabled);
    System.out.println("B4. enabled=0 -> " + tryset(b, "enabled", "0") + " ; enabled=" + b.enabled);
    System.out.println("B5. enabled=' true ' -> " + tryset(b, "enabled", " true ") + " ; enabled=" + b.enabled);

    RouterSettings c1 = new RouterSettings();
    c1.setLayerCount(2);
    System.out.println("C1. layers.routable=a,b,c on len2 -> " + tryset(c1, "layers.routable", "a,b,c")
        + " ; len=" + c1.layers.length + " routable=" + c1.layers[0].routable + "," + c1.layers[1].routable);
    RouterSettings c2 = new RouterSettings();
    System.out.println("C2. layers.routable=a,b,c on null -> " + tryset(c2, "layers.routable", "a,b,c")
        + " ; len=" + c2.layers.length);
    RouterSettings c3 = new RouterSettings();
    System.out.println("C3. layers.routable=false,true on null -> " + tryset(c3, "layers.routable", "false,true")
        + " ; len=" + c3.layers.length + " [" + c3.layers[0].routable + "," + c3.layers[1].routable + "]");
    RouterSettings c4 = new RouterSettings();
    System.out.println("C4. layers.routable='false,true,' on null -> " + tryset(c4, "layers.routable", "false,true,")
        + " ; len=" + c4.layers.length);
    RouterSettings c5 = new RouterSettings();
    c5.setLayerCount(3);
    System.out.println("C5. layers.routable=true,true on len3 -> " + tryset(c5, "layers.routable", "true,true")
        + " ; len=" + c5.layers.length + " [" + c5.layers[0].routable + "," + c5.layers[1].routable + "," + c5.layers[2].routable + "]");

    RouterSettings d = new RouterSettings();
    d.setLayerCount(2);
    System.out.println("D1. LAYERS.PREFERRED_DIRECTION_HORIZONTAL -> " + tryset(d, "LAYERS.PREFERRED_DIRECTION_HORIZONTAL", "true,false")
        + " ; [" + d.layers[0].preferredDirectionHorizontal + "," + d.layers[1].preferredDirectionHorizontal + "]");
    System.out.println("D2. layers.preferredDirectionHorizontal -> " + tryset(d, "layers.preferredDirectionHorizontal", "false,true")
        + " ; [" + d.layers[0].preferredDirectionHorizontal + "," + d.layers[1].preferredDirectionHorizontal + "]");
    System.out.println("D3. Layers.Bend_Cost=1.5,2.5 -> " + tryset(d, "Layers.Bend_Cost", "1.5,2.5")
        + " ; [" + d.layers[0].bendCost + "," + d.layers[1].bendCost + "]");
    RouterSettings d2 = new RouterSettings();
    System.out.println("D4. TRACEPULLTIGHTACCURACY=7 -> " + tryset(d2, "TRACEPULLTIGHTACCURACY", "7") + " ; " + d2.tracePullTightAccuracy);
    System.out.println("D5. trace_pull_tight_accuracy=8 -> " + tryset(d2, "trace_pull_tight_accuracy", "8") + " ; " + d2.tracePullTightAccuracy);
    System.out.println("D6. allowed_via_types=true -> " + tryset(d2, "allowed_via_types", "true") + " ; viasAllowed=" + d2.viasAllowed);
    System.out.println("D7. vias_allowed=false -> " + tryset(d2, "vias_allowed", "false") + " ; viasAllowed=" + d2.viasAllowed);
    System.out.println("D8. viasAllowed=true -> " + tryset(d2, "viasAllowed", "true") + " ; viasAllowed=" + d2.viasAllowed);
    System.out.println("D9. job_timeout=5m -> " + tryset(d2, "job_timeout", "5m") + " ; " + d2.jobTimeoutString);
    System.out.println("D10. jobTimeoutString=6m -> " + tryset(d2, "jobTimeoutString", "6m") + " ; " + d2.jobTimeoutString);
    System.out.println("D11. job_timeout_string=7m -> " + tryset(d2, "job_timeout_string", "7m") + " ; " + d2.jobTimeoutString);
    System.out.println("D12. result_json=/tmp/x -> " + tryset(d2, "result_json", "/tmp/x") + " ; " + d2.resultJsonPath);
    System.out.println("D13. resultJsonPath=/tmp/y -> " + tryset(d2, "resultJsonPath", "/tmp/y") + " ; " + d2.resultJsonPath);

    RouterSettings e = new RouterSettings();
    System.out.println("E1. max_passes=' 7 ' -> " + tryset(e, "max_passes", " 7 ") + " ; " + e.maxPasses);
    System.out.println("E2. max_passes='+7' -> " + tryset(e, "max_passes", "+7") + " ; " + e.maxPasses);
    System.out.println("E3. max_passes='7_0' -> " + tryset(e, "max_passes", "7_0") + " ; " + e.maxPasses);
    System.out.println("E4. copper_to_edge_clearance_um=' 7 ' -> " + tryset(e, "copper_to_edge_clearance_um", " 7 ") + " ; " + e.copperToEdgeClearanceUm);
    System.out.println("E5. hole_clearance_um='1e5' -> " + tryset(e, "hole_clearance_um", "1e5") + " ; " + e.holeClearanceUm);
    System.out.println("E6. hole_clearance_um='5d' -> " + tryset(e, "hole_clearance_um", "5d") + " ; " + e.holeClearanceUm);
    System.out.println("E7. hole_clearance_um='Infinity' -> " + tryset(e, "hole_clearance_um", "Infinity") + " ; " + e.holeClearanceUm);
    System.out.println("E8. hole_clearance_um='inf' -> " + tryset(e, "hole_clearance_um", "inf") + " ; " + e.holeClearanceUm);
    System.out.println("E9. hole_clearance_um='NaN' -> " + tryset(e, "hole_clearance_um", "NaN") + " ; " + e.holeClearanceUm);
    System.out.println("E10. hole_clearance_um='0x1p3' -> " + tryset(e, "hole_clearance_um", "0x1p3") + " ; " + e.holeClearanceUm);
    System.out.println("E11. neck_width_um='.5' -> " + tryset(e, "neck_width_um", ".5") + " ; " + e.neckWidthUm);
    System.out.println("E12. neck_width_um='5.' -> " + tryset(e, "neck_width_um", "5.") + " ; " + e.neckWidthUm);
    System.out.println("E13. neck_width_um='' -> " + tryset(e, "neck_width_um", "") + " ; " + e.neckWidthUm);
    System.out.println("E14. fanout.max_milliseconds_per_pin='9000000000' -> " + tryset(e, "fanout.max_milliseconds_per_pin", "9000000000") + " ; " + e.fanout.maxMillisecondsPerPin);
    System.out.println("E15. scoring.unrouted_net_penalty='1e40' -> " + tryset(e, "scoring.unrouted_net_penalty", "1e40") + " ; " + e.scoring.unroutedNetPenalty);
    System.out.println("E16. max_passes='99999999999' -> " + tryset(e, "max_passes", "99999999999") + " ; " + e.maxPasses);

    RouterSettings f = new RouterSettings();
    System.out.println("F1. optimizer.board_update_strategy=global_optimal -> " + tryset(f, "optimizer.board_update_strategy", "global_optimal") + " ; " + f.optimizer.boardUpdateStrategy);
    System.out.println("F2. =GLOBAL_OPTIMAL -> " + tryset(f, "optimizer.board_update_strategy", "GLOBAL_OPTIMAL") + " ; " + f.optimizer.boardUpdateStrategy);
    System.out.println("F3. =' hybrid ' -> " + tryset(f, "optimizer.board_update_strategy", " hybrid ") + " ; " + f.optimizer.boardUpdateStrategy);
    System.out.println("F4. =globalOptimal -> " + tryset(f, "optimizer.board_update_strategy", "globalOptimal") + " ; " + f.optimizer.boardUpdateStrategy);
    System.out.println("F5. optimizer.hybrid_ratio=1:1 -> " + tryset(f, "optimizer.hybrid_ratio", "1:1") + " ; " + f.optimizer.hybridRatio);
    System.out.println("F6. optimizer.item_selection_strategy=prioritized -> " + tryset(f, "optimizer.item_selection_strategy", "prioritized") + " ; " + f.optimizer.itemSelectionStrategy);

    RouterSettings g = new RouterSettings();
    System.out.println("G1. scoring.preferred_direction_trace_cost='1.5, 2.0' -> " + tryset(g, "scoring.preferred_direction_trace_cost", "1.5, 2.0")
        + " ; " + Arrays.toString(g.scoring.preferredDirectionTraceCost));
    System.out.println("G2. ignore_net_classes=' a , b ,' -> " + tryset(g, "ignore_net_classes", " a , b ,")
        + " ; " + Arrays.toString(g.ignoreNetClasses));
    System.out.println("G3. ignore_net_classes='' -> " + tryset(g, "ignore_net_classes", "")
        + " ; " + (g.ignoreNetClasses == null ? "null" : g.ignoreNetClasses.length + " " + Arrays.toString(g.ignoreNetClasses)));
    System.out.println("G4. ignore_net_classes='a,,b' -> " + tryset(g, "ignore_net_classes", "a,,b")
        + " ; " + Arrays.toString(g.ignoreNetClasses));
    System.out.println("G5. scoring.preferred_direction_trace_cost='x' -> " + tryset(g, "scoring.preferred_direction_trace_cost", "x"));

    RouterSettings h = new RouterSettings();
    System.out.println("H1. nope -> " + tryset(h, "nope", "1"));
    System.out.println("H2. min_bend_cost -> " + tryset(h, "min_bend_cost", "1"));
    System.out.println("H3. MIN_BEND_COST -> " + tryset(h, "MIN_BEND_COST", "1"));
    System.out.println("H4. board_specific_trace_costs_applied=true -> " + tryset(h, "board_specific_trace_costs_applied", "true"));
    System.out.println("H5. fanout=x -> " + tryset(h, "fanout", "x"));
    System.out.println("H6. layers=x -> " + tryset(h, "layers", "x"));
    System.out.println("H7. enabled.foo=1 -> " + tryset(h, "enabled.foo", "1"));
    System.out.println("H8. '.' -> " + tryset(h, ".", "1"));
    System.out.println("H9. '_' -> " + tryset(h, "_", "1"));
    System.out.println("H10. 'enabled.' -> " + tryset(h, "enabled.", "false") + " ; enabled=" + h.enabled);
    System.out.println("H11. 'optimizer..max_passes' -> " + tryset(h, "optimizer..max_passes", "3"));
    System.out.println("H12. 'optimizer:max_passes'=4 -> " + tryset(h, "optimizer:max_passes", "4") + " ; " + h.optimizer.maxPasses);
    System.out.println("H13. 'algorithm'=freerouting-router -> " + tryset(h, "algorithm", "freerouting-router") + " ; " + h.algorithm);
    System.out.println("H14. 'max_items'=5 -> " + tryset(h, "max_items", "5") + " ; " + h.maxItems);
    System.out.println("H15. 'scoring.viaCosts'=3 -> " + tryset(h, "scoring.viaCosts", "3") + " ; " + h.scoring.viaCosts);
    System.out.println("H16. 'scoring.via_costs'=4 -> " + tryset(h, "scoring.via_costs", "4") + " ; " + h.scoring.viaCosts);
    System.out.println("H17. 'fanout.ripupAllowed'=true -> " + tryset(h, "fanout.ripupAllowed", "true") + " ; " + h.fanout.ripupAllowed);
    System.out.println("H18. 'optimizer.improvement_threshold'=0.5 -> " + tryset(h, "optimizer.improvement_threshold", "0.5") + " ; " + h.optimizer.optimizationImprovementThreshold);
    System.out.println("H19. 'optimizer.optimization_improvement_threshold'=0.25 -> " + tryset(h, "optimizer.optimization_improvement_threshold", "0.25") + " ; " + h.optimizer.optimizationImprovementThreshold);
    System.out.println("H20. 'optimizer.timeout'=9m -> " + tryset(h, "optimizer.timeout", "9m") + " ; " + h.optimizer.timeoutString);
    RouterSettings h2 = new RouterSettings();
    h2.fanout = null; h2.optimizer = null; h2.scoring = null;
    System.out.println("H21. null nested: fanout.max_passes=3 -> " + tryset(h2, "fanout.max_passes", "3") + " ; " + (h2.fanout == null ? "null" : h2.fanout.maxPasses));
  }
}
