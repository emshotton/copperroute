# JOB 2 — Java CLI argv parse behaviour, measured

Jar `freerouting-current-executable.jar` (v2.3.1-SNAPSHOT, build-date 2026-08-29), JDK 25,
`-Djava.awt.headless=true -Duser.language=en -Duser.country=US` on every run, `timeout 60`.
**87 shapes, 0 timeouts.** Raw per-shape blocks (argv / exit / files / normalised stdout+stderr)
are in `job2-argv.txt`; the normalisation rule set is stated at the top of that file.

Boards used: `fixtures/empty_board.dsn` (0 unrouted items — the router runs but routes nothing)
for every routing shape with `-mp 1`; `fixtures/Issue143-rpi_splitter.dsn` only as the second DSN
in shape 28; `fixtures/Issue026-J2_reference.dsn` only for `--compare-boards` (shape 16).
**No shape let a real route run**; the router phase is a ~0.05 s no-op everywhere.

---

## 1. There are FOUR argv parsers, and they disagree

| # | parser | Java site | what it reads |
|---|---|---|---|
| P0 | stdio pre-parse | `Freerouting.java:901-917` | `--mcp_server.stdio=` (`true`/`1`, case-insensitive for `true`) |
| P1 | user-data pre-parse | `Freerouting.java:944-956` | `--user_data_path=` — `startsWith`, **first** occurrence (`findFirst`) |
| P2 | logging pre-parse | `Freerouting.java:1032-1065` | `--logging.*=`, `--debug.enable_detailed_logging=`, **`-dl` (exact `equals`)**, **`-ll` (exact `equals`, value taken from the FIRST `-ll` via `indexOf`)** |
| P3 | the flag table | `GlobalSettings.applyCommandLineArguments:521-838` | everything else; every short flag except `-l` matched with `startsWith`; **last occurrence wins** |
| P4 | `CliSettings` | `settings/sources/CliSettings.java:39-79` | `--router.*=…`; `-mp`/`-mt` by **exact** `switch` on `arg.substring(1)`; `-de`/`-do` exact, only to force `router.enabled = true` |

P2 is what actually configures log4j (`:1105 reconfigure()`); P3's `logging.*` writes land after
log4j is already built and are inert for console/file output. P3 and P4 both see the same argv and
both warn independently — one shape can therefore produce two different complaints about one flag.

---

## 2. Flag table — match rule / value / missing value / effect / observed exit

`SW` = `arg.startsWith("<flag>")`, case-sensitive. `EQ-IC` = `equalsIgnoreCase`. `EQ` = `equals`.
"Value" = consumes `args[i+1]` **only if** `!args[i+1].startsWith("-")` (`GlobalSettings.java:566,651,656,664,671,676,689,701,711,722,733,739,804,812,817,828`).
Exit codes are dominated by the `initializeCli` guard (`Freerouting.java:80-86`): **anything without
both an input and an output file exits 1**, whatever the flag did.

| flag | match rule | site | value | missing value (flag last / next is a flag) | effect | shapes | exit |
|---|---|---|---|---|---|---|---|
| `-help` / `--help` / `-h` | EQ-IC | `:524-526` | no | n/a | `showHelpOption=true` → localised help to stdout, `System.exit(0)` | 02,03,04,83 | 0 |
| `-help…` (prefix) | SW | `:808` | no | n/a | same; `-helpX` shows help | 05 | 0 |
| `--compare-boards=` | SW | `:530` | inline, `split(",")` | **exactly 2 fields required**; 0/1/3+ silently ignored | sets `compareFile1/2`; `main:1293-1296` runs the comparison and `System.exit(0|1)` **before** anything else | 13,14,15,16 | 1 |
| `--<sec>.<field>=<v>` | SW `--` | `:538-563` | inline (`split("=",2)`) | no `=` at all → `WARN Unknown command line argument: <arg>` | `setValue` via reflection; unknown path → `WARN Unknown settings property: <p>` | 07,08,09,10,11,12,54,79,80,81 | 0/1 |
| `--user_data_path=` | SW `--` then name check | `:543`,`:561` | inline | n/a | **explicitly exempted** from both the setter and the unknown-arg warning; consumed by P1 only | 17 | 1 |
| `-de` | SW | `:564-648` | **1..N** trailing non-`-` args | **silent no-op**, `i` not advanced | multi-slot classifier, see §4 | 18-33,82,86 | 0/1 |
| `-di` | SW | `:649-654` | 1 | silent no-op | `guiSettings.inputDirectory` — **inert in CLI mode** | 34,35,36 | 1 |
| `-do` | SW | `:655-659` | 1 | silent no-op | `initialOutputFile` | 21,22,37,38 | 0/1 |
| `-drc` | SW, **before `-dr`** | `:660-669` | 1 (report path) | sets `router.enabled=false` + `drc.enabled=true` **but leaves `drcReportFile` null → NOT DRC mode** | with a value: DRC-only mode, GUI/API/MCP forced off (`main:1401-1405`), report written, always exit 0 | 43,44,45,46,84,87 | 0/1 |
| `-dr` | SW | `:670-674` | 1 | silent no-op | `initialRulesFile` | 39,40,41,42 | 0/1 |
| `-mp` | SW | `:675-687` | 1, `Integer.decode` | silent no-op in P3; **P4 still warns** (`""` → `parseInt`) | `router.maxPasses`, clamped `<0→1`, `>9999→9999`, `0` = unlimited | 21,47-52 | 0 |
| `-mt` | SW | `:688-699` | 1, `Integer.decode` | silent no-op in P3; P4 warns P3 → `optimizer.maxThreads`, clamped `<0→0`, `>1024→1024`; P4 → the **different** field `router.max_threads`, later capped at `availableProcessors` by `RouterSettings.validate` (`RouterSettings.java:951-955`) — see §5.9 | 53 | 0 |
| `-oit` | SW | `:700-709` | 1, `Float.parseFloat`/100 | silent no-op | dead bridge (quirk #131) | 55,56 | 0 |
| `-us` | SW | `:710-720` | 1 | silent no-op | dead bridge | 57 | 0 |
| `-is` | SW | `:721-731` | 1 | silent no-op | dead bridge | 58 | 0 |
| `-hr` | SW | `:732-736` | 1 | silent no-op | dead bridge | 59 | 0 |
| `-l` | **EQ (the only exact match)** | `:737` | 1, lower-cased, `-`→`_` | `localeString=""` → falls through all 27 prefixes → locale unchanged | `currentLocale`; drives the help text and the GUI only | 73,74,75 | 0 |
| `-dl` | SW | `:799-800` | **no** | n/a | sets `logging.file.enabled=false` **too late to matter**; the real switch is P2's `equals("-dl")` | 71,72 | 0 |
| `-da` | SW | `:801-802` | no | n/a | `disableAnalytics=true` | 76 | 0 |
| `-host` | SW | `:803-807` | 1, trimmed | silent no-op | `runtimeEnvironment.host` (log/analytics only) | 77,78 | 0 |
| `-inc` | SW | `:810-815` | 1, `split(",")` | silent no-op | dead bridge | 60,61 | 0 |
| `-dct` | SW | `:816-824` | 1, `Integer.parseInt` | silent no-op | `guiSettings.dialogConfirmationTimeout`, `<=0→0` — GUI only | 62,63 | 1 |
| `-ll` | SW | `:825-831` | 1, upper-cased | silent no-op | writes `logging.console.level` **after log4j is configured → no effect on output**; P2's exact `-ll` is what works | 64-70 | 0 |
| anything else | — | `:832-833` | — | — | `WARN Unknown command line argument: <arg>` and **continue** | 06,75,82,86,87 | 0/1 |
| any exception | — | `:835-837` | — | — | `ERROR There was a problem parsing the '<arg>' parameter` + stack trace, loop **continues**, `i` **not advanced** | 47,62 | 0/1 |

Nothing in the table can make the process exit non-zero by itself. **Exit is only ever 0 or 1**
(`main:1295`, `:1397`, `:1474`, `:1493`), which matches ruling AR.

---

## 3. Prefix semantics — measured, not assumed

The rule is **`arg.startsWith(flag)`**, not `flag.startsWith(arg)`, and it is **case-sensitive**:

| shape | argv | result |
|---|---|---|
| 32 | `-decoy <dsn> -do out.ses -mp 1` | routed — `-decoy` **is** `-de` (`out.ses` written, exit 0) |
| 33 | `-dedupe <dsn> …` | routed — same |
| 86 | `-d <dsn>` | `WARN Unknown command line argument: -d` **plus** a second `WARN` for the filename → proves the direction is `arg.startsWith("-de")` |
| 36 | `-diff <dir>` | consumed as `-di` (no warn) |
| 42 | `-drums <rules>` | consumed as `-dr` (the rules file was read and rejected on design-name mismatch) |
| 46 | `-drcx report.json` | **DRC mode** — `drc.json` (333 B) written, no `.ses`; `-drc` at `:660` is tested before `-dr` at `:670` |
| 52 | `-mpx 5` | consumed as `-mp` |
| 70 | `-llx DEBUG` | consumed as `-ll` by P3 — **but the console stayed at INFO** because P2 wants exact `-ll` |
| 72 | `-dlx` | consumed as `-dl` by P3 — **but `freerouting.log` was still written** because P2 wants exact `-dl` |
| 82 | `-DE <dsn>` | unknown ×2 (flag + filename) — matching is case-sensitive |
| 87 | `-DRC <path>` | unknown ×2 — likewise |
| 75 | `-lx` | unknown — `-l` is the one flag matched with `equals` (`:737`) |
| 05/83 | `-helpX` / `-HELP` | both show help (`:808` SW, and `:524` EQ-IC) |

A warned-about unknown flag whose *value* also does not start with `-` produces **two** warnings,
because the else-branch does not consume the value (shapes 82, 86, 87).

---

## 4. The `-de` multi-slot classifier (`GlobalSettings.java:564-648`)

`-de` greedily consumes **every** following argv element until one starts with `-`, then classifies
each by lower-cased extension into three slots:

| observed | shape | result |
|---|---|---|
| `-de a.dsn` | 21 | `initialInputFile = a.dsn` |
| `-de a.dsn b.rules` | 23 | `.rules` → `initialRulesFile`; both slots filled, routed, exit 0 |
| `-de a.dsn b.ses c.rules` | 24 | three slots filled in one flag, routed, exit 0 |
| `-de a.dsn junk.txt` | 25 | `WARN Unknown file type in -de argument: …junk.txt. Expected .dsn, .json, .ses, or .rules`, then routes normally, exit 0 |
| `-de x.json` (no `.dsn` yet) | 26 | the `.json` becomes the **input** (`:609-612`), board loaded, exit 0 |
| `-de a.dsn y.json` | 27 | `.json` after a `.dsn` becomes the **session** file (`:613-621`) |
| `-de a.dsn b.dsn` | 28 | `WARN Multiple DSN files provided in -de argument. Only the last one will be used.` → the **last** wins (`Issue143-rpi_splitter.dsn` loaded) |
| `-de missing.dsn` | 29 | classified fine; fails later: `ERROR Couldn't load the input file '/nope/missing.dsn'` → exit 1 |
| `-de "a.dsn+b.rules"` | 30 | the `+`-split legacy form (`:573-581`) fires only because the literal path does not exist; two files produced |
| `-de x.ses -do y.ses` | 31 | **`.ses` never becomes the input** — it fills the *session* slot, `initialInputFile` stays null → `ERROR Both an input file and an output file must be specified…` → exit 1 in 2 s. **The quirk-N infinite spin is NOT reachable this way.** |
| `-de` (last argv element) | 19 | silent no-op |
| `-de -do out.ses` | 20 | `-de` consumes nothing (next starts with `-`), `-do` still parsed → exit 1 (no input) |

Order is irrelevant: `-do out.ses -de a.dsn -mp 1` (shape 22) is byte-identical in behaviour to
shape 21.

---

## 5. Surprising findings

### 5.1 `-dl` and `-ll`: P2 (exact) vs P3 (`startsWith`) — the disagreement is observable in the FILE SYSTEM
- `-dl` (shape 71) → **no `freerouting.log` in the run dir**. `-dlx` (shape 72) → **`freerouting.log` written**, even though P3 set `logging.file.enabled=false`.
  Cause: `Freerouting.java:1056` `"-dl".equals(arg)` vs `GlobalSettings.java:799` `startsWith("-dl")`; log4j is configured from P2's value at `:1086-1105`, before P3 ever runs (`main:1291`).
- `-ll DEBUG` (64) → console DEBUG (49 DEBUG lines). `-llx DEBUG` (70) → console stays INFO. Same cause (`main:1057` `"-ll".equals(arg)`).

### 5.2 Duplicate `-ll`: FIRST wins, and P3's LAST-wins assignment is dead
`-ll DEBUG -ll TRACE -h` (shape 68) produced a **DEBUG** console (13 DEBUG lines, zero real TRACE
lines — the only "TRACE" string is inside the echoed command line). P2 resolves the value with
`Arrays.asList(args).indexOf("-ll")` (`Freerouting.java:1060-1065`) so the **first** occurrence wins;
P3's `logging.console.level = args[i+1]` (`:825-830`) is a last-wins assignment that never reaches
log4j. `-ll TRACE` alone (65) does produce real `TRACE ITEM_ACTIVITY` lines; `-ll OFF` (67) produced
**zero** stdout lines while still writing the file log; `-ll BOGUS` (66) behaved exactly like the
default INFO.

### 5.3 `-mp`: two parsers, two number formats, two failure modes
| shape | argv tail | P3 (`Integer.decode`, `GlobalSettings.java:677`) | P4 (`parseInt` via `ReflectionUtil`, `CliSettings.java:88-97`) |
|---|---|---|---|
| 51 | `-mp 0x10` | **accepted** (decode → 16), silent | `WARN Failed to apply CLI router setting: router.max_passes: For input string: "0x10"` |
| 47 | `-mp abc` | `ERROR There was a problem parsing the '-mp' parameter` + `NumberFormatException` stack, then — because `i` was **not** advanced — `WARN Unknown command line argument: abc` | `WARN … For input string: "abc"` |
| 49 | `-mp` (last) | silent no-op | `WARN … For input string: ""` |
| 50 | `-mp -5` | not consumed (`"-5".startsWith("-")`) → `WARN Unknown command line argument: -5`; **a negative pass count is inexpressible** | `WARN … For input string: ""` |
All four still exit **0** and still write `out.ses`. The same "`i` not advanced → the value is
re-warned as an unknown flag" pattern appears for `-dct abc` (shape 62, `:818`).

### 5.4 `-drc` without a value is not DRC mode
`-drc` as the last element (44) or followed by another flag (45) sets `routerSettings.enabled=false`
and `drcSettings.enabled=true` but leaves `drcReportFile` null, so `main:1462` never enters DRC mode.
Shape 45 (`-drc -de x.dsn -do out.ses -mp 1`) **routed normally and exited 0** — the router-disable
is a dead write (quirk #131 / survey label A confirmed). `-drc report.json` with no `-de` (84) gives
`ERROR An input file must be specified with -de argument in DRC mode.` → exit 1. With `-de` (43) the
report is written and exit is 0 (`initializeDrc` returns `true` unconditionally).

### 5.5 `--router.enabled=false` suppresses only the autorouter, not the run
Shape 81 (`--router.enabled=false -de empty_board.dsn -do out.ses -mp 1`): **no** "Auto-routing stage
started" line, but the fanout and **optimization** stages still ran, `out.ses` (212 B) was still
written and the exit code was **0**. `--router.enabled=` with an empty value (11) parses without a
warning at all — `Boolean.parseBoolean("")` is `false` (`GlobalSettings.java:559` → `setValue`).

### 5.6 `-do <non-.ses>` writes SES bytes anyway (quirk L confirmed)
Shape 38 (`-do out.txt`) produced `out.txt` of **212 bytes — byte-for-byte the size of the baseline
`out.ses`** — and exited 0. `tryToSetOutputFile`'s return value is discarded at `Freerouting.java:123`.

### 5.7 `--compare-boards` is an early, total exit
Shape 16 (two real, different DSNs) printed `WARN WARNING: Differences detected between the loaded
boards.` and exited **1** — `main:1293-1296` runs before help, before DRC, before the CLI. Two
missing files (13) → `ERROR Comparison file 1 does not exist: /nope1.dsn` → exit 1. **One** file (14)
or an empty value (15) leaves `compareFile1/2` null (the `files.length == 2` guard at `:532`) and the
run continues into the normal CLI path.

### 5.8 `-dr` with a non-existent path is silent (quirk V confirmed)
Shape 40 (`-dr /nope/missing.rules`) produced **no warning at all**, routed and exited 0. A rules file
that exists but names a different design (39, 42) produces
`WARN RulesReader.read: designName not matching…` + `ERROR Failed to apply rules from rules file`
and still exits **0**.

### 5.9 `-mt` is clamped twice, and the second clamp is not the documented one
Shape 53 (`-mt 99999`) hits **two different fields**. P3 writes
`routerSettings.optimizer.maxThreads` and clamps it to 1024 (`GlobalSettings.java:690-697`); P4 maps
the same flag to the *other* field, `router.max_threads` = `RouterSettings.maxThreads`
(`CliSettings.java:102-108`), un-clamped, and `RouterSettings.validate` then emits
`WARN Invalid maxThreads value: 99999, capping at 8` and caps at `availableProcessors`
(`RouterSettings.java:951-955`). The warning prints **99999**, i.e. it is reporting the P4 field, not
P3's 1024 — direct evidence for quirk #132 ("two fields, two clamps"). Consistent with quirk #143,
nothing on the headless path reads either result.

### 5.10 `FREEROUTING__USER_DATA_PATH` throws on every run that sets it (new)
Shapes 01 and 85: the env var is accepted by P1 (`Freerouting.java:930-932`) but
`applyNonRouterEnvironmentVariables` then tries to reflect it back into the **static `Path`** field:
```
ERROR  Failed to set property value for: user_data_path
java.lang.IllegalArgumentException: Can not set static java.nio.file.Path field
  app.freerouting.settings.GlobalSettings.userDataPath to java.lang.String
  at app.freerouting.settings.GlobalSettings.setValue(GlobalSettings.java:500)
  at app.freerouting.settings.GlobalSettings.applyNonRouterEnvironmentVariables(GlobalSettings.java:487)
  at app.freerouting.Freerouting.main(Freerouting.java:1245)
```
The path itself still works (logs and `freerouting.json` land in the right place); the error is
cosmetic but appears on **stdout and stderr** on every such run. The `--user_data_path=` **argv**
form is clean — it is excluded from the setter at `:543` and from the unknown-arg warning at `:561`.

### 5.11 Log routing: stdout carries the log, stderr carries a second copy of every ERROR
Every `ERROR` line (and its stack trace) appears in **both** streams — Console→`SYSTEM_OUT`
(`Log4j2ConfigurationFactory.java:58`) plus a `Level.ERROR` appender→`SYSTEM_ERR` (`:88-96`), plus
the file. `WARN` and `INFO` go to stdout only. Any stdout-JSON mode the port adds must account for
this (survey label AI confirmed).

### 5.12 Help text vs reality
`-help` documents 11 forms (`-de -di -dr -do -mp -l -mt -us -hr -is -h`). Measured as accepted but
**undocumented**: `-drc`, `-oit`, `-dl`, `-da`, `-host`, `-inc`, `-dct`, `-ll`, `--compare-boards=`,
`--user_data_path=`, and the whole `--section.field=value` form. There is **no `-v`/`--verbose`**.
`-l de -help` (shape 73) prints the German help (`VERWENDUNG`), so `-l` is applied before the help
branch at `main:1394-1398` even when it appears first.

---

## 6. Exit-code summary as measured

| exit | shapes | rule |
|---|---|---|
| 0 | 02-05, 10, 21-28, 32, 33, 38-40, 42, 43, 45-53, 55-61, 64-81, 83, 85 | help; a completed route with a written output; DRC-with-report; **and every "bad flag" shape that still had a valid `-de`+`-do` pair** |
| 1 | 01, 06-09, 11-20, 29-31, 34-37, 41, 44, 54, 62, 63, 82, 84, 86, 87 | the `initializeCli` guard (missing input and/or output), an unreadable input, DRC without `-de`, or a `--compare-boards` difference |
| TIMEOUT | none | no shape hung; the quirk-N spin is not reachable through `-de <ses>` (see §4, shape 31) |

**No flag-syntax error ever changes the exit code.** The port reproducing this bug-for-bug (ruling
AR) means: warn-and-continue, silent missing-value no-op, `startsWith` matching in the documented
order, and exit determined solely by whether a job ran and produced a non-empty output.
