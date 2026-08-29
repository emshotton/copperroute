# `fr-settings` JVM probes (Plan 4)

JUnit-free Java drivers whose output is the source of every expected value in
`crates/fr-settings/tests/{field_path,router_settings,board_optimizations,sources,env_source,cli_source,json}.rs`
and of quirks rows 114-143 in `docs/java-quirks.md` (the whole Plan 4 block: 114-117 Task 2,
118-122 Task 3, 123-126 Task 4, 127 Task 5, 128-130 Task 6, 131-137 Task 7, 138-139 Task 6's fix
round, 140 Task 8, 141 Task 10, 142 Task 8's fix round 2, 143 Task 12's fix round — the last from the
HEAD source alone, no probe: it is a reachability fact about the router, not a value). They are committed so Task 9's `p4t*` differential can reuse the
matrix instead of re-deriving it, and so any of these expectations can be re-checked against a
rebuilt jar.

| Driver | What it probes | Needs the jar |
|---|---|---|
| `FProbe.java` | 44 `setFieldValue` cases through the real `RouterSettings`: the `-`/`:` separators (A, H12), `Boolean.parseBoolean` (B), array navigation (C), name resolution (D, H13-H20), the numeric arms (E), enums (F), `String[]`/`double[]` leaves (G), and every failure mode (H1-H21) | yes |
| `DProbe.java` | `Double.parseDouble` / `Integer.parseInt` grammar edges — the `d`/`f` suffix, exact-case `Infinity`/`NaN`, `String.trim`'s `<= ' '` rule, the hexadecimal form, Unicode digits | no |
| `TProbe.java` | The array-token vs scalar-leaf trim asymmetry (`ReflectionUtil.java:71`) | yes |
| `VProbe.java` | Task 4's accessors, clamps, `setLayerCount`, `clone` and `validate` (blocks A-F); run with `-XX:ActiveProcessorCount=4` | yes |
| `BProbe.java` | Task 5's `applyBoardSpecificOptimizations` over four synthetic stacks and any DSN fixtures named on the command line, plus the Q9 merge block | yes |
| `RProbe.java` | Quirk 119's real consequence: `setLayerCount`'s effect on `--router.layers.*` values (L1-L4), and what Java's array branch leaves behind when the *next* path segment is bogus (A1-A3) | yes |
| `CProbe.java` | Task 7's `EnvironmentVariablesSource` (A), `CliSettings`' value-consumption and flag-mapping rules (B), the five `SettingsMergerTest` CLI cases merged (C), the dead `LegacyBridge`'s whole flag table (D) and the `-de` classification matrix including a real file whose name contains `+` (E); run with `-XX:ActiveProcessorCount=4` | yes |
| `PProbe.java` | Task 8's headless precedence: what the two `validate()` calls of `Freerouting.java:146` + `RoutingJobScheduler.java:170` do to `--router.max_passes=0` (block A), the two fields that *are* stable across them (B), and the same object validated twice by hand (C) — quirk #140's evidence; run with `-XX:ActiveProcessorCount=4` | yes |
| `JProbe.java` | Task 10's Gson parity: what `GsonProvider.GSON` writes for a fully populated `RouterSettings` (A), Java's `Double.toString`/`Float.toString` thresholds and `-0.0` (B), the non-finite refusal on **write** (B4) and on **read** (H), the read side — the API payload, the four `transient` keys, the `@SerializedName` alternates, unknown keys and `{}` (C, F) — `Strictness.LENIENT`'s reader dialect (D) and its coercions (I), and the `U+2028`/`U+2029` escape that survives `disableHtmlEscaping()` (J) | yes |
| `SProbe.java` | Task 6's `SettingsMerger`, `SettingsSource` and the five in-scope `settings/sources/**` classes: the whole `DefaultSettings` table field by field (A), the ported `SettingsMergerTest` cases plus the only-null-sources NPE and the stable-sort tie (B), `addOrReplaceSources` (C), every source name and priority (D), both `.rules` goldens raw and through the accessors (E), `DsnFileSettings` over three fixtures with no `(autoroute_settings)` block — quirk 128's evidence (F), and that seeding merged under `DefaultSettings` (G), and — fix round 1 — absence versus the coalesced default on a reduced `.rules` file (H); run with `-XX:ActiveProcessorCount=4`, the fixtures directory as `argv[0]` and this directory as `argv[1]` | yes |

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

# VProbe and SProbe pin availableProcessors so their transcripts match
# HostEnvironment::with_processors(4)
/opt/homebrew/opt/openjdk@25/bin/javac -cp "$JAR" -d . VProbe.java SProbe.java
/opt/homebrew/opt/openjdk@25/bin/java -XX:ActiveProcessorCount=4 -Djava.awt.headless=true \
    -cp "$JAR:." VProbe
/opt/homebrew/opt/openjdk@25/bin/java -XX:ActiveProcessorCount=4 -Djava.awt.headless=true \
    -cp "$JAR:." SProbe /Users/em/Development/freerouting/freerouting/fixtures \
    /Users/em/Development/freerouting/freerouting-rs/crates/fr-settings/tests/data

# PProbe pins availableProcessors the same way (its rows read maxThreads)
/opt/homebrew/opt/openjdk@25/bin/javac -cp "$JAR" -d . PProbe.java
/opt/homebrew/opt/openjdk@25/bin/java -XX:ActiveProcessorCount=4 -Djava.awt.headless=true \
    -cp "$JAR:." PProbe

# JProbe needs no processor pinning — nothing it prints reads availableProcessors
/opt/homebrew/opt/openjdk@25/bin/javac -cp "$JAR" -d . JProbe.java
/opt/homebrew/opt/openjdk@25/bin/java -Djava.awt.headless=true -cp "$JAR:." JProbe

# CProbe pins availableProcessors for its `validate()` rows (block C)
/opt/homebrew/opt/openjdk@25/bin/javac -cp "$JAR" -d . CProbe.java
/opt/homebrew/opt/openjdk@25/bin/java -XX:ActiveProcessorCount=4 -Djava.awt.headless=true \
    -cp "$JAR:." CProbe

# BProbe takes DSN fixtures as arguments; Task 5's goldens used these three
F=/Users/em/Development/freerouting/freerouting/fixtures
/opt/homebrew/opt/openjdk@25/bin/javac -cp "$JAR" -d . BProbe.java
/opt/homebrew/opt/openjdk@25/bin/java -Djava.awt.headless=true -cp "$JAR:." BProbe \
    $F/Issue026-J2_reference.dsn $F/Issue143-rpi_splitter.dsn $F/Issue145-smoothieboard.dsn
```

The transcripts these produced are in
`.superpowers/sdd/2026-08-28-plan-4-settings/task-{3,4,5,6,7,8,10}-report.md`.

## Committed inputs

`Plan4Matrix-primary.rules` and `Plan4Matrix-adjacent.rules` are Task 8's two minimal
`(autoroute_settings)` scopes — the `-dr` file and the scheduler's file of the precedence matrix
(`crates/fr-settings/tests/matrix/mod.rs`). They name `F.Cu`/`B.Cu` so that a driver's
`RulesReader.read` can map them onto a synthetic two-layer board, and they disagree on every
value they carry, so a case where merge #1 and merge #2 see different rules cannot pass by
coincidence.

`Issue029-hw48na_reduced.rules` is `../freerouting/fixtures/Issue029-hw48na_valid.rules` with
eight lines deleted — `(vias on)`, `(via_costs 50)`, `(plane_via_costs 5)`,
`(start_ripup_costs 100)` and the four per-layer trace-cost lines. It is the input `SProbe` block
`H` and `tests/sources.rs::an_unnamed_rules_field_does_not_overwrite_a_lower_priority_source`
share: a scope that names *some* fields and omits others, which is what distinguishes "absent"
from "the coalesced default" (controller ruling L, quirks row for the Plan 3 gap). It is committed
rather than generated so the JVM transcript and the Rust test read the same bytes.
