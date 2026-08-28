package app.freerouting.io.specctra;

import java.io.BufferedWriter;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.util.Random;

/**
 * Plan 3 Task 2: the number-formatting differential driver — {@code Double.toString},
 * {@code Float.toString} and {@code SesWriter.formatPlacementRotation}, the three functions every
 * DSN/SES writer's byte-parity rests on.
 *
 * <p>Usage: {@code P3T2 <count> <seed> <mode>}. One line per value: the raw bits in hex, then
 * {@code Double.toString}, then {@code Float.toString} of the {@code float} cast, then — for every
 * mode but 0 — {@code formatPlacementRotation}.
 *
 * <p>Modes:
 *
 * <ul>
 *   <li><b>0</b> — uniformly random 64-bit patterns, skipping NaN and the infinities (those are
 *       pinned by the unit tests, and 2^52 of the patterns are NaN). Rotation formatting is
 *       *omitted* here: {@code %.0f} of a 1e300-scale double is a 300-digit line, and these values
 *       are not rotations.
 *   <li><b>1</b> — {@code (nextInt(2000001) - 1000000) / 10^nextInt(7)}: the shape real DSN
 *       coordinates take, and the only mode that feeds negative values to the rotation formatter.
 *   <li><b>2</b> — random integers in ±10^7.
 *   <li><b>3</b> — random rotations in [0, 360) quantised to three decimals.
 * </ul>
 *
 * <p>Declares {@code package app.freerouting.io.specctra} so it can reach the package-private
 * {@code SesWriter.formatPlacementRotation}, and is therefore compiled and run against the clone's
 * own {@code build/libs/freerouting-current-executable.jar} with a JDK 25, like {@code p2t10} /
 * {@code p2t11} / {@code p2t15}. Twin: {@code p3t2}.
 */
public class P3T2 {

  /** {@code 10^k} for {@code k} in 0..6, as literals: {@code Math.pow} is not part of the port. */
  private static final double[] POW10 = {
    1.0, 10.0, 100.0, 1000.0, 10000.0, 100000.0, 1000000.0
  };

  public static void main(String[] args) throws Exception {
    if (args.length != 3) {
      System.err.println("usage: P3T2 <count> <seed> <mode>");
      System.exit(1);
    }
    int count = Integer.parseInt(args[0]);
    long seed = Long.parseLong(args[1]);
    int mode = Integer.parseInt(args[2]);
    Random random = new Random(seed);
    BufferedWriter out =
        new BufferedWriter(
            new OutputStreamWriter(System.out, StandardCharsets.US_ASCII), 1 << 20);
    StringBuilder line = new StringBuilder(64);

    int emitted = 0;
    while (emitted < count) {
      double value;
      switch (mode) {
        case 0:
          {
            double candidate = Double.longBitsToDouble(random.nextLong());
            if (Double.isNaN(candidate) || Double.isInfinite(candidate)) {
              continue;
            }
            value = candidate;
            break;
          }
        case 1:
          value = (random.nextInt(2000001) - 1000000) / POW10[random.nextInt(7)];
          break;
        case 2:
          value = random.nextInt(20000001) - 10000000;
          break;
        case 3:
          value = random.nextInt(360000) / 1000.0;
          break;
        default:
          throw new IllegalArgumentException("unknown mode " + mode);
      }
      line.setLength(0);
      line.append(Long.toHexString(Double.doubleToRawLongBits(value)));
      line.append(' ').append(Double.toString(value));
      line.append(' ').append(Float.toString((float) value));
      if (mode != 0) {
        line.append(' ').append(SesWriter.formatPlacementRotation(value));
      }
      line.append('\n');
      out.write(line.toString());
      emitted++;
    }
    out.flush();
  }
}
