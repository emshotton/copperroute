import app.freerouting.settings.RouterSettings;
import app.freerouting.settings.SettingsMerger;
import app.freerouting.settings.sources.ApiSettings;
import app.freerouting.settings.sources.CliSettings;
import app.freerouting.settings.sources.DefaultSettings;

public final class PProbe {

  public static void main(String[] args) {
    for (String value : new String[] {"0", "12345", "9999", "-1"}) {
      RouterSettings merged1 =
          new SettingsMerger(
                  new DefaultSettings(), new CliSettings(new String[] {"--router.max_passes=" + value}))
              .merge();
      RouterSettings merged2 =
          new SettingsMerger(
                  new DefaultSettings(),
                  new CliSettings(new String[] {"--router.max_passes=" + value}),
                  new ApiSettings(merged1))
              .merge();
      System.out.printf(
          "A.max_passes=%s merge1=%d merge2=%d%n", value, merged1.maxPasses, merged2.maxPasses);
    }

    for (String arg :
        new String[] {
          "--router.max_threads=99", "--router.max_threads=-3", "--router.trace_pull_tight_accuracy=0"
        }) {
      RouterSettings merged1 =
          new SettingsMerger(new DefaultSettings(), new CliSettings(new String[] {arg})).merge();
      RouterSettings merged2 =
          new SettingsMerger(
                  new DefaultSettings(), new CliSettings(new String[] {arg}), new ApiSettings(merged1))
              .merge();
      System.out.printf(
          "B.%s merge1=[mp=%d mt=%d tpta=%d] merge2=[mp=%d mt=%d tpta=%d]%n",
          arg,
          merged1.maxPasses,
          merged1.maxThreads,
          merged1.tracePullTightAccuracy,
          merged2.maxPasses,
          merged2.maxThreads,
          merged2.tracePullTightAccuracy);
    }

    RouterSettings settings = new DefaultSettings().getSettings();
    settings.maxPasses = 0;
    settings.validate();
    int once = settings.maxPasses;
    settings.validate();
    System.out.printf("C.validateTwice once=%d twice=%d%n", once, settings.maxPasses);
  }
}
