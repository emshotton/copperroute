package app.freerouting.io.specctra;

import java.io.FileDescriptor;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.io.InputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.IdentityHashMap;
import java.util.Map;

/**
 * Plan 3 Task 3 differential driver: dumps the token stream the Specctra DSN scanner produces
 * for a file, one line per token.
 *
 * <p>Usage: {@code P3T3 <file.dsn>}. Each line is
 *
 * <pre>
 *   &lt;index&gt; &lt;TAG&gt; &lt;value&gt; &lt;lexicalStateAfterTheToken&gt;
 * </pre>
 *
 * with {@code TAG} one of {@code OPEN}, {@code CLOSE}, {@code KW}, {@code STR}, {@code INT},
 * {@code DBL}. A {@code KW}'s value is the *field name* of the {@code Keyword} singleton the
 * scanner returned (found by reflection over {@code Keyword}'s public static fields), not
 * {@code Keyword.getName()}: the field name is the identity the parser dispatches on and is the
 * one thing that is provably identical between the pinned 2.3.0 jar and the clone's HEAD, whose
 * fifteen renamed name strings are plan ruling 1's subject. A {@code DBL}'s value is
 * {@code Double.toString} (Task 2's rules); {@code STR} values are escaped so that a token is
 * always exactly one line.
 *
 * <p>Compiled against a freerouting jar (see {@code run.sh}); the scanner's methods were renamed
 * between 2.3.0 ({@code next_token}) and HEAD ({@code nextToken}), so both are looked up.
 */
public final class P3T3 {

  private P3T3() {}

  public static void main(String[] args) throws Exception {
    if (args.length < 1) {
      System.err.println("usage: P3T3 <file>");
      System.exit(2);
    }

    // The scanner's action 2 calls `FRLogger.warn` for every non-ANSI character, and FRLogger
    // logs to `System.out`, which would interleave with the token stream. Bind the driver's own
    // output to the real stdout and silence `System.out` before FRLogger is ever loaded.
    PrintStream out =
        new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));

    Class<?> scannerClass =
        Class.forName("app.freerouting.io.specctra.parser.SpecctraDsnStreamReader");
    Method nextToken = method(scannerClass, "next_token", "nextToken");
    Method yystate = scannerClass.getMethod("yystate");

    Map<Object, String> keywordNames = keywordNames();

    try (InputStream in = new FileInputStream(args[0])) {
      Object scanner = scannerClass.getConstructor(InputStream.class).newInstance(in);
      int index = 0;
      while (true) {
        Object token;
        try {
          token = nextToken.invoke(scanner);
        } catch (Exception e) {
          Throwable cause = e.getCause() == null ? e : e.getCause();
          out.println(index + " ERROR " + cause.getClass().getName());
          return;
        }
        if (token == null) {
          out.println(index + " EOF - " + yystate.invoke(scanner));
          return;
        }
        out.println(index + " " + describe(token, keywordNames) + " " + yystate.invoke(scanner));
        index++;
      }
    }
  }

  private static Method method(Class<?> c, String... names) throws NoSuchMethodException {
    for (String name : names) {
      try {
        return c.getMethod(name);
      } catch (NoSuchMethodException ignored) {
        // try the next spelling
      }
    }
    throw new NoSuchMethodException(String.join("/", names));
  }

  /** Maps every {@code Keyword} singleton to the name of the field that holds it. */
  private static Map<Object, String> keywordNames() throws Exception {
    Class<?> keywordClass = Class.forName("app.freerouting.io.specctra.parser.Keyword");
    Map<Object, String> names = new IdentityHashMap<>();
    for (Field f : keywordClass.getFields()) {
      Object value = f.get(null);
      if (value != null && keywordClass.isInstance(value)) {
        names.put(value, f.getName());
      }
    }
    return names;
  }

  private static String describe(Object token, Map<Object, String> keywordNames) {
    String keyword = keywordNames.get(token);
    if (keyword != null) {
      if (keyword.equals("OPEN_BRACKET")) {
        return "OPEN -";
      }
      if (keyword.equals("CLOSED_BRACKET")) {
        return "CLOSE -";
      }
      return "KW " + keyword;
    }
    if (token instanceof String s) {
      return "STR " + escape(s);
    }
    if (token instanceof Integer i) {
      return "INT " + i;
    }
    if (token instanceof Double d) {
      return "DBL " + d;
    }
    return "OTHER " + token.getClass().getName() + ":" + escape(String.valueOf(token));
  }

  /** Escapes a token value so that one token is always exactly one output line. */
  private static String escape(String s) {
    StringBuilder sb = new StringBuilder(s.length() + 2);
    for (int i = 0; i < s.length(); i++) {
      char c = s.charAt(i);
      switch (c) {
        case '\\' -> sb.append("\\\\");
        case '\n' -> sb.append("\\n");
        case '\r' -> sb.append("\\r");
        case '\t' -> sb.append("\\t");
        default -> {
          if (c < 0x20 || c == 0x7f) {
            sb.append(String.format("\\u%04x", (int) c));
          } else {
            sb.append(c);
          }
        }
      }
    }
    return sb.toString();
  }
}
