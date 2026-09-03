# `p8t2-batch-ses` — the pre-lane-switch, jar-written `batch.ses` corpus

These eight files are **directed test fixtures**, not references. They are byte-for-byte copies of

    tests/reference/<stem>/batch.ses

as they stood **before** Plan 9 Task 2 regenerated the B family from the port, and they are read
by `crates/fr-core/tests/stats.rs` (rows 21-28 and row 31 of the `p8t2` transcript) and by
`scripts/differential/java/probes/P8T2Probe.java`, which cuts that transcript.

## Provenance

| | |
|---|---|
| written by | `java -jar freerouting-current-executable.jar`, revision `278fe14123c49376667239659c98d41a597acce9`, version 2.3.1-SNAPSHOT — the parity jar of Plans 6, 7 and 8 |
| via | `scripts/gen-batch-reference.sh` (the `--jar` lane), driver `scripts/differential/java/probes/P7T9Probe.java` mode `batch`, `-XX:hashCode=2` |
| copied at | Plan 9 Task 2, from commit `4a5bce6` — the last commit at which `tests/reference/` was jar-written |
| stems | the eight of `tests/reference/router-fixtures.txt`, in file order, each at that table's own `-mp` cap and fanout/optimizer flags |

## Why they are here rather than read from `tests/reference/`

Plan 9 Task 2 fixed the two measured Java regressions — R1 (register **#293**, the airline-first
ordering of the work list) and R2 (**#294**, the micro-neckdown fanout fallback's rules-minimum
floor) — and both move every routed stem's SES. Ruling BT's G1 cadence therefore regenerated the
B and C families `--from-port`, and the port writes 2.3.0's **snake_case** `host_cad` where the
jar wrote camelCase `hostCad` (quirk **#92**). Measured at Plan 9 Task 1: **20** files under
`tests/reference/` carried `hostCad` before the switch and **0** after.

`crates/fr-core/tests/stats.rs`'s row 31 — "the one file shape where the host scrape SUCCEEDS" —
is exactly that camelCase spelling, read back through the **DSN** branch of
`BoardStatistics(byte[], FileFormat)`, and it is the only real-file exercise of quirks **#248(b)**
(a `reduced` parser scope carries no `(stringQuote ")` to truncate the scrape), **#250** and
**#252**. The test reads the file's *bytes* and runs the scraper over them, so the fixture **is**
the assertion and no transcribed literal can stand in for it. `tests/reference-frozen/` holds the
same bytes and BL8 (as amended by ruling BP1) forbids any test or script from reading it.

Ruling BT pre-agreed this resolution so that the task which first moved B or C would not have to
stop and ask: the files migrate here, the module's source rows are repointed at them, and the
`p8t2` transcript is re-cut against them in the same commit. **BL8 stays intact** — a committed
test data file is not the frozen tree — and the quirk exercise survives the lane switch instead of
being deleted by it.

## Do not regenerate these

They are a historical artefact of one jar build and are read only for their bytes. Nothing under
`scripts/gen-*.sh` writes here; `P8T2Probe` reads them and never writes them. If a future task
needs a *current* `batch.ses`, that is `tests/reference/<stem>/batch.ses`, which is a different
thing and now lives in a different lane.
