// Plan 4 Task 3 JVM probe — see README.md in this directory for the full command and the
// jar identity. Recorded command:
//   javac -cp $JAR -d . TProbe.java && java -Djava.awt.headless=true -cp "$JAR:." TProbe
// JAR=…/freerouting/build/libs/freerouting-current-executable.jar (clone HEAD, plan ruling 7)
import app.freerouting.settings.RouterSettings;
import app.freerouting.util.ReflectionUtil;
public class TProbe {
  public static void main(String[] a) throws Exception {
    RouterSettings s = new RouterSettings();
    s.setLayerCount(2);
    ReflectionUtil.setFieldValue(s, "layers.routable", " true , true ");
    System.out.println("array tokens trimmed before parseBoolean: " + s.layers[0].routable + "," + s.layers[1].routable);
    RouterSettings t = new RouterSettings();
    ReflectionUtil.setFieldValue(t, "enabled", " true ");
    System.out.println("scalar leaf not trimmed: " + t.enabled);
  }
}
