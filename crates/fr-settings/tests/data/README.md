# `ReflectionUtil.setFieldValue` JVM probes (Plan 4, Task 3)

Four JUnit-free Java drivers whose output is the source of every expected value in
`crates/fr-settings/tests/field_path.rs` and of quirks rows 118-122 in `docs/java-quirks.md`.
They are committed so Task 9's `p4t*` differential can reuse the matrix instead of re-deriving
it, and so any of these expectations can be re-checked against a rebuilt jar.

| Driver | What it probes | Needs the jar |
|---|---|---|
| `FProbe.java` | 44 `setFieldValue` cases through the real `RouterSettings`: the `-`/`:` separators (A, H12), `Boolean.parseBoolean` (B), array navigation (C), name resolution (D, H13-H20), the numeric arms (E), enums (F), `String[]`/`double[]` leaves (G), and every failure mode (H1-H21) | yes |
| `DProbe.java` | `Double.parseDouble` / `Integer.parseInt` grammar edges — the `d`/`f` suffix, exact-case `Infinity`/`NaN`, `String.trim`'s `<= ' '` rule, the hexadecimal form, Unicode digits | no |
| `TProbe.java` | The array-token vs scalar-leaf trim asymmetry (`ReflectionUtil.java:71`) | yes |
| `RProbe.java` | Quirk 119's real consequence: `setLayerCount`'s effect on `--router.layers.*` values (L1-L4), and what Java's array branch leaves behind when the *next* path segment is bogus (A1-A3) | yes |

## Recorded commands

Run from a scratch directory; the jar is the clone's HEAD build (plan ruling 7), which was
`freerouting-current-executable.jar`, 63 288 650 bytes, mtime 2026-08-27 20:03 when these
transcripts were taken.

```sh
JAR=/Users/em/Development/freerouting/freerouting/build/libs/freerouting-current-executable.jar
ls -la "$JAR"   # record size + mtime alongside any transcript

# jar-backed drivers
for p in FProbe TProbe RProbe; do
  /opt/homebrew/opt/openjdk@25/bin/javac -cp "$JAR" -d . "$p.java"
  /opt/homebrew/opt/openjdk@25/bin/java -Djava.awt.headless=true -cp "$JAR:." "$p"
done

# JDK-only driver
/opt/homebrew/opt/openjdk@25/bin/javac -d . DProbe.java
/opt/homebrew/opt/openjdk@25/bin/java DProbe
```

The transcripts these produced are in
`.superpowers/sdd/2026-08-28-plan-4-settings/task-3-report.md`.
