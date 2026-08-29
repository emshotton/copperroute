// Plan 4 Task 3 JVM probe — see README.md in this directory for the full command and the
// jar identity. Recorded command:
//   javac -cp $JAR -d . RProbe.java && java -Djava.awt.headless=true -cp "$JAR:." RProbe
// JAR=…/freerouting/build/libs/freerouting-current-executable.jar (clone HEAD, plan ruling 7)
import app.freerouting.settings.RouterSettings;
import app.freerouting.util.ReflectionUtil;
import java.util.Arrays;

/** Probes the two review-round questions: setLayerCount's effect on CLI-set per-layer values,
 *  and what setPropertyRecursive's array branch leaves behind when the *next* segment is bogus. */
public class RProbe {
  static String show(RouterSettings s) {
    if (s.layers == null) return "null";
    StringBuilder b = new StringBuilder("len=" + s.layers.length + " [");
    for (int i = 0; i < s.layers.length; i++) {
      b.append(s.layers[i] == null ? "null"
          : "r=" + s.layers[i].routable + ",pdh=" + s.layers[i].preferredDirectionHorizontal
            + ",bc=" + s.layers[i].bendCost);
      if (i + 1 < s.layers.length) b.append(" | ");
    }
    return b.append("]").toString();
  }
  static String tryset(Object o, String p, String v) {
    try { ReflectionUtil.setFieldValue(o, p, v); return "ok"; }
    catch (Throwable t) { return t.getClass().getSimpleName() + ": " + t.getMessage(); }
  }

  public static void main(String[] a) throws Exception {
    // L1: token count != board layer count -> HeadlessBoardManager:742-744's guard fires and
    //     setLayerCount reallocates, discarding every --router.layers.* value.
    RouterSettings s1 = new RouterSettings();
    System.out.println("L1a. set layers.routable=false,false,false -> " + tryset(s1, "layers.routable", "false,false,false"));
    System.out.println("L1b. set layers.bend_cost=1.5,2.5,3.5   -> " + tryset(s1, "layers.bend_cost", "1.5,2.5,3.5"));
    System.out.println("L1c. before setLayerCount(6): " + show(s1));
    s1.setLayerCount(6);
    System.out.println("L1d. after  setLayerCount(6): " + show(s1));

    // L2: token count == board layer count -> the guard does NOT fire, values survive.
    RouterSettings s2 = new RouterSettings();
    tryset(s2, "layers.routable", "false,false,false");
    tryset(s2, "layers.bend_cost", "1.5,2.5,3.5");
    System.out.println("L2a. getLayerCount()=" + s2.getLayerCount() + " boardLayerCount=3 -> guard "
        + (s2.getLayerCount() != 3 ? "FIRES" : "does not fire") + "; values: " + show(s2));

    // L3: setLayerCount's per-element reset is unconditional *when called*, even on a matching count.
    RouterSettings s3 = new RouterSettings();
    tryset(s3, "layers.routable", "false,false,false");
    tryset(s3, "layers.bend_cost", "1.5,2.5,3.5");
    s3.setLayerCount(3);
    System.out.println("L3. setLayerCount(3) on a 3-element array: " + show(s3));

    // L4: clone() calls setLayerCount but then re-clones layers wholesale, so nothing is lost.
    RouterSettings s4 = new RouterSettings();
    tryset(s4, "layers.routable", "false,false,false");
    tryset(s4, "layers.bend_cost", "1.5,2.5,3.5");
    System.out.println("L4. clone(): " + show(s4.clone()));

    // A1/A2: navigating one segment too far through a String[] / double[] field.
    RouterSettings s5 = new RouterSettings();
    System.out.println("A1. ignore_net_classes.foo=a,b -> " + tryset(s5, "ignore_net_classes.foo", "a,b"));
    System.out.println("A1. leaves ignoreNetClasses = " + Arrays.toString(s5.ignoreNetClasses));
    RouterSettings s6 = new RouterSettings();
    System.out.println("A2. scoring.preferred_direction_trace_cost.foo=1,2 -> " + tryset(s6, "scoring.preferred_direction_trace_cost.foo", "1,2"));
    System.out.println("A2. leaves preferredDirectionTraceCost = " + Arrays.toString(s6.scoring.preferredDirectionTraceCost));
    RouterSettings s7 = new RouterSettings();
    System.out.println("A3. max_passes.foo=1 -> " + tryset(s7, "max_passes.foo", "1") + " ; maxPasses=" + s7.maxPasses);
  }
}
