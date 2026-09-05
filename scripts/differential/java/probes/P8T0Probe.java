package app.freerouting.util;

import app.freerouting.util.TextManager;

/**
 * Plan 8 Task 0's JVM-pinned evidence: {@code TextManager.parseTimespanString} and the timeout
 * ladder it feeds, over thirty inputs.
 *
 * <p>Declares {@code package app.freerouting.util} so it sits beside {@link TextManager}; both of
 * the methods it drives are {@code public static}, so the package is a convention here rather than
 * an access requirement (unlike {@code P7T2Probe}, which needs it).
 *
 * <p>For every input the probe prints one tab-separated row:
 *
 * <pre>
 *   RAW &lt;quoted input&gt;
 *   CONV &lt;convertFromTimespanToDurationFormat's answer&gt;
 *   PARSE &lt;parseTimespanString's Long, or "null", or "&lt;ExceptionClass&gt;"&gt;
 *   CAPPED &lt;the value after RoutingJobSchedulerActionThread.java:47-49, or "null"&gt;
 *   OFFSET &lt;Instant.EPOCH.plusSeconds(capped) as an epoch-second, or "null"&gt;
 * </pre>
 *
 * <p>{@code PARSE} carries an exception class only if {@code parseTimespanString} ever throws —
 * scan ruling R11 says it does not (it catches {@code DateTimeParseException} itself at
 * {@code TextManager.java:90-92} and answers {@code null}), and the column exists so that the
 * transcript proves it rather than assuming it. {@code OFFSET} is {@code :51}'s
 * {@code job.startedAt.plusSeconds(timeout)} with {@code startedAt} pinned to
 * {@code Instant.EPOCH}, so the arithmetic is reproducible; a value the {@code Instant} range
 * cannot represent prints {@code overflow}.
 *
 * <p>The two literals are re-read by reflection rather than transcribed, so the transcript records
 * what the jar actually holds.
 *
 * <p>Run (this is exactly what {@code scripts/differential/run.sh p8t0} does):
 *
 * <pre>
 *   javac -cp &lt;HEAD jar&gt; -d &lt;out&gt; scripts/differential/java/probes/P8T0Probe.java
 *   java -Djava.awt.headless=true -Duser.language=en -Duser.country=US \
 *        -XX:+UnlockExperimentalVMOptions -XX:hashCode=2 \
 *        -cp &lt;out&gt;:&lt;HEAD jar&gt; app.freerouting.util.P8T0Probe
 * </pre>
 */
public final class P8T0Probe {

  private P8T0Probe() {}

  /**
   * The thirty inputs. The brief names ten of them explicitly ({@code "1:30:00"}, {@code "90"},
   * {@code "1.5"}, {@code "1:2:3:4"}, {@code "x"}, {@code ""}, {@code "25:00:00"}, {@code "-1"},
   * {@code " 1:00 "} and "whatever the method's own grammar rejects"); the rest probe the arms a
   * reader would guess wrong:
   *
   * <ul>
   *   <li>{@code ":"}, {@code "1:"}, {@code ":1"} — {@code String.split} drops the trailing empty
   *       run but keeps a leading one, so these are three different part counts.
   *   <li>{@code "00:00:00"}, {@code "0"} — a zero timeout is not the same as no timeout.
   *   <li>{@code "1:5.5"}, {@code "1.5:00"} — only the seconds field may be fractional.
   *   <li>{@code "-1.5"}, {@code "-0.5"} — {@code getSeconds()} floors, it does not truncate.
   *   <li>{@code "+90"}, {@code "1:-30"} — each field carries its own optional sign.
   *   <li>{@code "24:00:00"}, {@code "24:00:01"} — either side of {@code MAX_TIMEOUT}.
   *   <li>{@code "99999999999999999999"} — a number too large for the field parse.
   *   <li>{@code "1:60"}, {@code "0:0:90"} — out-of-range components are NOT normalised away.
   * </ul>
   */
  private static final String[] INPUTS = {
    "1:30:00",
    "90",
    "1.5",
    "1:2:3:4",
    "x",
    "",
    "25:00:00",
    "-1",
    " 1:00 ",
    "   ",
    ":",
    "1:",
    ":1",
    "0",
    "00:00:00",
    "24:00:00",
    "24:00:01",
    "1:60",
    "0:0:90",
    "1:5.5",
    "1.5:00",
    "-1.5",
    "-0.5",
    "+90",
    "1:-30",
    "1:2:3",
    "99999999999999999999",
    "PT1H",
    "1:00:00.5",
    "01:00:00",
  };

  /** Prints the thirty rows. */
  public static void main(String[] args) throws Exception {
    long maxTimeout = readLongConstant("MAX_TIMEOUT");
    int gracePeriod = (int) readLongConstant("GRACE_PERIOD");
    System.out.println("MAX_TIMEOUT\t" + maxTimeout);
    System.out.println("GRACE_PERIOD\t" + gracePeriod);
    System.out.println("ROWS\t" + INPUTS.length);

    for (String input : INPUTS) {
      String conv;
      try {
        conv = TextManager.convertFromTimespanToDurationFormat(input);
      } catch (Throwable t) {
        conv = "<" + t.getClass().getSimpleName() + ">";
      }

      String parsed;
      Long value = null;
      try {
        value = TextManager.parseTimespanString(input);
        parsed = (value == null) ? "null" : Long.toString(value);
      } catch (Throwable t) {
        parsed = "<" + t.getClass().getSimpleName() + ">";
      }

      // RoutingJobSchedulerActionThread.java:45-51, with startedAt pinned to the epoch.
      String capped = "null";
      String offset = "null";
      if (value != null) {
        long timeout = value;
        if (timeout > maxTimeout) {
          timeout = maxTimeout;
        }
        capped = Long.toString(timeout);
        try {
          offset = Long.toString(java.time.Instant.EPOCH.plusSeconds(timeout).getEpochSecond());
        } catch (Throwable t) {
          offset = "overflow";
        }
      }

      System.out.println(
          "RAW\t"
              + quote(input)
              + "\tCONV\t"
              + conv
              + "\tPARSE\t"
              + parsed
              + "\tCAPPED\t"
              + capped
              + "\tOFFSET\t"
              + offset);
    }
  }

  /** Reads one of the two private static finals out of the scheduler action thread. */
  private static long readLongConstant(String name) throws Exception {
    Class<?> cls = Class.forName("app.freerouting.management.jobs.RoutingJobSchedulerActionThread");
    java.lang.reflect.Field field = cls.getDeclaredField(name);
    field.setAccessible(true);
    return ((Number) field.get(null)).longValue();
  }

  /** Renders the raw input unambiguously, so a trailing space is visible in the transcript. */
  private static String quote(String s) {
    StringBuilder sb = new StringBuilder("\"");
    for (int i = 0; i < s.length(); i++) {
      char c = s.charAt(i);
      if (c == '"' || c == '\\') {
        sb.append('\\').append(c);
      } else if (c < 0x20 || c > 0x7e) {
        sb.append(String.format("\\u%04x", (int) c));
      } else {
        sb.append(c);
      }
    }
    return sb.append('"').toString();
  }
}
