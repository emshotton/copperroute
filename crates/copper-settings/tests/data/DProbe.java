public class DProbe {
  static void d(String s) {
    try { System.out.println("parseDouble(" + esc(s) + ") = " + Double.parseDouble(s)); }
    catch (Throwable t) { System.out.println("parseDouble(" + esc(s) + ") -> " + t.getClass().getSimpleName()); }
  }
  static void i(String s) {
    try { System.out.println("parseInt(" + esc(s) + ") = " + Integer.parseInt(s)); }
    catch (Throwable t) { System.out.println("parseInt(" + esc(s) + ") -> " + t.getClass().getSimpleName()); }
  }
  static String esc(String s) { StringBuilder b = new StringBuilder("\""); for (char c : s.toCharArray()) { if (c < 32 || c > 126) b.append(String.format("\\u%04x", (int) c)); else b.append(c); } return b.append('"').toString(); }
  public static void main(String[] a) {
    d("5f"); d("5F"); d("5D"); d("1e5f"); d("1e5d"); d("  Infinity  "); d("+Infinity"); d("-NaN");
    d("Infinityd"); d("NaNf"); d("infinity"); d("0X1P3"); d("0x1.8p1"); d(" 7"); d("\t7\n"); d("7 "); d(" "); d("5.5.5"); d("1e"); d("."); d("--5"); d("1_0");
    i("٣"); i("١٢٣"); i(" 7"); i("7 "); i("+7"); i("-7"); i(""); i("-"); i("0007");
  }
}
