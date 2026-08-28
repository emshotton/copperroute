import java.util.Random;
public class R {
  public static void main(String[] a) {
    for (int bound : new int[]{2,3,4,5,6,7,8,16,100}) {
      Random r = new Random(99);
      StringBuilder sb = new StringBuilder("bound=" + bound + ":");
      for (int i = 0; i < 12; i++) sb.append(" ").append(r.nextInt(bound));
      System.out.println(sb);
    }
  }
}
