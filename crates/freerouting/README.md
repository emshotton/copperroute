# `freerouting` — the command-line program

Two command lines reach the same program.

* The **legacy** form is Java's, reproduced bug for bug:
  `freerouting -de board.dsn -do board.ses -mp 100`.
* The **native** form is this port's own: `freerouting route board.dsn -o board.ses`.

`legacy::is_legacy_form` decides which: **the native form starts with a subcommand name; every
other argv — including an empty one — is legacy.** So `freerouting` with no arguments is a *legacy*
command line, and it answers what the jar answers: `Both an input file and an output file must be
specified …` and **exit 1**, not a usage screen.

> **Status.** Plan 8 Task 5 landed the command line: the parse, the mode ladder, the exit ladder
> and the log surface. It did **not** land the run path — `route`, `drc` and `info` are stubs that
> answer exit 3 until Tasks 6, 7 and 12. Every "reaches the router" sentence below describes the
> spelling and the decision, not a wired behaviour. Task 13 expands this file into the full
> reference (every accepted flag, the `p8t1`–`p8t7` acceptance table, the MCP delta table).

---

## The exit ladder (plan ruling AR)

| code | when | Java |
|---|---|---|
| **0** | a completed run; `--help`; `-drc` unconditionally (plan label **B** — `initializeDrc` returns `true` whatever happens; Task 7 lands the row) | `Freerouting.java:1495`, `:1397` |
| **1** | any failure — **including every refusal on the legacy path** | `Freerouting.java:1474` |
| **2** | **port only:** a usage error on the *native* subcommand form (clap's own) | — |
| **3** | **port only, reserved:** a subcommand not wired up yet | — |

Ruling AR is why 2 and 3 are native-form-only: *a command line the jar accepts must never come
back with a code the jar cannot produce.* `legacy::rewrite` therefore never fails — it warns — and
every argv it builds parses, so clap can only refuse an argv the user typed in the native form.

The same rule decided `--version`: the jar **refuses** `--version` and `-V`
(`Unknown command line argument: --version`, then `initializeCli`'s refusal, **exit 1** — measured
on the HEAD jar, and pinned by `p8t5`'s `version-long`/`version-short` rows). So the legacy path has
no `--version` arm, and clap's lives behind a subcommand: `freerouting route --version`.

---

## Port-only spellings

Java has none of these. Each is marked "Port only" in its own `--help` text too, so that a reader
who learns this CLI does not type them at the jar.

| spelling | what it is | Java's equivalent |
|---|---|---|
| `-v` / `-vv` / `--verbose` | raise the log level | **none** — the jar's log-level flag is `-ll <level>` (quirk #260) |
| `--log-level <level>` | set it outright | `-ll <level>`, or `--logging.console.level=<level>` |
| `--settings <file>` | name a `freerouting.json` for the priority-10 tier | **none** — the jar only reads the file under its OS-standard user-data path |
| `--version` / `-V` | print the version (native form only) | **none**; the jar prints its version in the startup banner (`Freerouting.java:1120`) |
| `--kicad-json <file>` | a KiCad board file in its own slot (native form only, ruling 14) | **none** — on the legacy form a `.json` takes Java's own slot |
| `--set <section.field=value>` | a generic settings override | `--section.field=value`, which the legacy form still accepts |

`--settings` is accepted and unread today; see the `// obligation:` at `cli::Cli::settings`.

---

## The dead legacy knobs, and `--set` (plan ruling AQ)

Seven legacy writes are **parsed and dead**, in Java and therefore here:

| flag | what it writes | who reads it |
|---|---|---|
| `-oit` | `routerSettings.optimizer.optimizationImprovementThreshold` | **nobody** |
| `-us` | `routerSettings.optimizer.boardUpdateStrategy` | **nobody** |
| `-is` | `routerSettings.optimizer.itemSelectionStrategy` | **nobody** |
| `-hr` | `routerSettings.optimizer.hybridRatio` | **nobody** |
| `-inc` | `routerSettings.ignoreNetClasses` | **nobody** |
| `-drc`'s `routerSettings.enabled = false` | the same object | **nobody** |
| `-mt` | `routerSettings.optimizer.maxThreads` | **nobody** (quirk #143: the whole multithreaded path is unreachable) |

All seven write Java's `@Deprecated public final RouterSettings routerSettings` bridge
(`GlobalSettings.java:51-53`), which no routing path consults. `docs/java-quirks.md` #131 and #143
are the rows; `crates/fr-settings/src/sources/cli.rs`'s `LegacyBridge` carries the writes with a
`// Java bug:` marker per field, and nothing in `resolve_headless` reads it.

**Ruling AQ: the port keeps them dead, and keeps parsing them.** Keeping the parse is not
pedantry — it is observable, because the value decides how far the cursor moves and therefore which
*other* arguments get warned about, and `p8t5` compares that. Wiring them up would make the port
**more capable than the jar**, which is a product decision and not a parity fix.

**The same knobs are reachable through `--set`.** Every one of them is a field of `RouterSettings`,
so the supported spellings are the two generic ones, which go through `CliSettings` at priority 60
— the parser that actually reaches the router:

```sh
# native form
freerouting route board.dsn -o board.ses \
    --set router.optimizer.optimization_improvement_threshold=0.005 \
    --set router.optimizer.board_update_strategy=GLOBAL_OPTIMAL \
    --set router.optimizer.item_selection_strategy=SEQUENTIAL \
    --set router.optimizer.hybrid_ratio=1:2 \
    --set router.optimizer.max_threads=4

# legacy form — Java's own `--section.field=value`, accepted unchanged
freerouting -de board.dsn -do board.ses --router.optimizer.max_threads=4
```

Two warnings that are not hedges:

* **`--set` is not wired to the run path yet** (Task 6). The spelling and the decision are settled;
  the plumbing is not.
* **The two spellings are not equivalent to the dead flags.** `--router.optimizer.max_threads=4`
  goes through `ReflectionUtil.setFieldValue` and therefore carries **no clamp**, where `-mt`
  clamps to `[0, 1024]` on the bridge (`GlobalSettings.java:692-697`). `-mt 99999` is 1024 there;
  `--router.optimizer.max_threads=99999` is 99999. Both are measured — `p8t5`'s `mt` and
  `router-optimizer-max-threads` rows.

---

## Logging

Everything goes to **stderr**, and there is no log file. That is a deliberate divergence from the
jar, which writes `INFO`/`WARN` to **stdout** and an `ERROR` three times over (stdout, the log
file, and stderr) — `docs/java-quirks.md` #261. The port also emits no timestamp, so its output is
byte-reproducible; `normalize_log` strips the jar's before comparing.

The level ladder is Java's: the console default is `INFO`, `-ll <level>` moves it (matched by
prefix, **last** occurrence wins), and an unrecognised name silently means `INFO`
(`Log4j2ConfigurationFactory.parseLevel:130-135`).

---

## Where the rest of it is written down

| what | where |
|---|---|
| every legacy flag's value normalisation, and the `-de` rule | `docs/cli-legacy-flags.md` |
| the parse rules, arm by arm, with the Java line for each | `src/legacy.rs`'s module docs |
| the log level ladder, the stderr divergence, `MESSAGE_MAP`, and the `logger/**` roster | `src/logging.rs` |
| the Java behaviours reproduced on purpose | `docs/java-quirks.md` #131, #143, #259-#264 |
| the differential that pins all of it against the jar | `scripts/differential/{run.sh p8t5, sweep-p8t5.sh}`, 86 argv shapes |
