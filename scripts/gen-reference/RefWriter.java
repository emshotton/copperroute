import app.freerouting.board.BasicBoard; // 2.3.0 jar layout (the clone has since moved it to board.facade)
import app.freerouting.io.BoardReadResult;
import app.freerouting.io.specctra.DsnReader;
import app.freerouting.io.specctra.DsnWriter;
import app.freerouting.io.specctra.SesWriter;
import java.io.FileInputStream;
import java.io.FileOutputStream;
import java.nio.file.Path;

/**
 * Reference generator for bit-parity tests: reads a Specctra DSN with the real
 * freerouting reader and writes it back out unchanged as DSN and as an
 * (unrouted) SES, without ever running fanout/autoroute/optimizer.
 *
 * Usage: java -cp freerouting-<ver>.jar:. RefWriter <in.dsn> <out.dsn> <out.ses>
 *
 * This bypasses the -de/-do job path, which (a) cannot write DSN output in
 * headless mode and (b) writes nothing at all when every routing stage is
 * disabled (see scripts/gen-reference.sh history).
 */
public final class RefWriter {
  public static void main(String[] args) throws Exception {
    if (args.length != 3) {
      System.err.println("usage: RefWriter <in.dsn> <out.dsn> <out.ses>");
      System.exit(2);
    }
    String designName = Path.of(args[0]).getFileName().toString().replaceAll("\\.dsn$", "");
    BoardReadResult result;
    try (FileInputStream in = new FileInputStream(args[0])) {
      result = DsnReader.readBoard(in, null, null, designName);
    }
    BasicBoard board =
        switch (result) {
          case BoardReadResult.Success s -> s.board();
          case BoardReadResult.OutlineMissing o -> o.board();
          case BoardReadResult.ParseError e -> {
            System.err.println("parse error at " + e.location() + ": " + e.detail());
            System.exit(1);
            yield null;
          }
          case BoardReadResult.IoError e -> {
            System.err.println("io error: " + e.cause());
            System.exit(1);
            yield null;
          }
        };
    try (FileOutputStream out = new FileOutputStream(args[1])) {
      DsnWriter.write(board, out, designName, false);
    }
    try (FileOutputStream out = new FileOutputStream(args[2])) {
      SesWriter.write(board, out, designName);
    }
  }
}
