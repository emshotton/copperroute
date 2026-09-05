package app.freerouting.logger;

import app.freerouting.geometry.planar.Point;

public class FRLogger {
  public static boolean VERBOSE = false;
  public static void warn(String s) { if (VERBOSE) System.err.println("WARN " + s); }
  public static void warn(String s, Throwable t) { if (VERBOSE) System.err.println("WARN " + s); }
  public static void info(String s) { if (VERBOSE) System.err.println("INFO " + s); }
  public static void debug(String s) { if (VERBOSE) System.err.println("DEBUG " + s); }
  public static void trace(String s) { }
  public static void trace(String a, String b, String c, String d, Point[] e) { }
  public static void trace(String a, String b, String c) { }
  public static void error(String s, Throwable t) { if (VERBOSE) System.err.println("ERROR " + s); }
}
