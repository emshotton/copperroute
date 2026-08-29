import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
public final class CProbe {
  public static void main(String[] args) throws Exception {
    PrintStream out = new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);
    System.setOut(new PrintStream(OutputStream.nullOutputStream()));
    System.setErr(new PrintStream(OutputStream.nullOutputStream()));
    List<Path> paths = new ArrayList<>();
    try (DirectoryStream<Path> ds = Files.newDirectoryStream(Path.of(args[0]), "*.dsn")) {
      for (Path p : ds) paths.add(p);
    }
    Collections.sort(paths);
    for (Path p : paths) {
      BoardReadResult r;
      try (InputStream in = Files.newInputStream(p)) { r = DsnReader.readBoard(in, null, null); }
      String kind = r.getClass().getSimpleName();
      List<String> w = switch (r) {
        case BoardReadResult.Success s -> s.warnings();
        case BoardReadResult.OutlineMissing o -> o.warnings();
        default -> List.of();
      };
      out.println("FIXTURE " + p.getFileName() + " " + kind + " warnings=" + w.size());
      for (String s : w) out.println("  W " + s);
    }
  }
}
