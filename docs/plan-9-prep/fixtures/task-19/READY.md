# Task 19 — what the implementer gets

**Coverage: 11 of 11 fix rows have ground truth ready. None of them needs the jar.**

## `kicad-cli` availability
`kicad-cli` is **not on `PATH`** but **is installed**, at
`/Applications/KiCad/KiCad.app/Contents/MacOS/kicad-cli`, **version 10.0.3**.
It was not needed: KiCad's own DRC verdicts for the two Issue575 boards are already committed
to the Java clone and are copied into `fixtures/` here. Use the binary above if a third board's
verdict is wanted.

| row | ground truth | oracle |
|---|---|---|
| #152 | complete per-stem reclassification table + padstack layer spans | port goldens + DSN parse |
| #146 | uniqueness invariant; KiCad reports **5** `track_dangling` where the port reports **111** (and **4 = 4** on `via_dangling`) | KiCad 10 |
| #153 | two-call improvement invariant | invariant |
| #271 | exit-code table over a 107-violation stem and a 0-violation stem | port goldens |
| #272 | `qualityScore` must differ with/without `-dr` on `drc-issue593-rules`; violations stay 0/74 | before/after pair |
| #151 | exact mm / um / inch / mil table for `pos = (96.52, −119.38)` | arithmetic |
| #154 | **the five key spellings, extracted from KiCad 10's own file** | KiCad 10 |
| #195 | minimal witness `total_length = 1200` vs `h+v+a = 1200.002` + the f32 summation caution | measured at v1.0.0 |
| #196 | `bounding_box.{width,height} == size.{width,height}` — no literal | measured at v1.0.0 |
| #110 | `p8t13-conduction-area.dsn` already exists; "no non-integer token in the SES" | existing fixture |
| #111 | quote-and-round-trip a design name with a space | invariant |
| #81 / #212 / #201 | `validate()` = `true` on a triangulation missing an edge; field-vs-recomputation agreement; comment reword | measured / internal |

## The three things to read first

1. **`an_smd_pad_is_not_a_hole` must not assert on BBD Mars-64.** The plan binds *"the BBD
   Mars-64 64/12 split moves"*. It does not. All 64 are `(SMD Pin, Via)` pairs, and the
   classification is `is_hole(a) **||** is_hole(b)` — the Via alone keeps them `holeClearance`
   whatever the fix does to pins. The two stems that actually move are
   **`drc-dev-board` 2/0 → 0/2** and **`drc-issue753-cpu85` 78/0 → 76/2**; the padstack
   evidence for every candidate pin is in `expected-outcomes.md`. Keep BBD Mars-64 as a
   *no-change* guard, and do **not** change `||` to `&&` to make it move — KiCad's own rule is
   hole-vs-any-copper, so `||` is right.

2. **#154 is proven by artefact, not argued.** The port's file and KiCad 10's file claim the
   **same** `$schema` (`https://schemas.kicad.org/drc.v1.json`) and disagree on five keys:
   `coordinateUnits`/`coordinate_units`, `kicadVersion`/`kicad_version`,
   `schematicParity`/`schematic_parity`, `unconnectedItems`/`unconnected_items`,
   `holeClearance`/`hole_clearance`. The port is even internally inconsistent — it already
   writes `track_dangling` and `via_dangling` in snake_case in the *same field* as
   `holeClearance`. Both files are in `fixtures/` for diffing.

3. **#195's invariant needs a floating-point decision, and the plan does not name it.**
   `total_length` is a Kahan-compensated `f64` sum cast to `f32`; `h`/`v`/`a` are naive `f32`
   accumulations. Exact `h+v+a == total` will not hold on a large board. Accumulate the
   breakdown with the same `java_double_stream_sum` (already in the file) and the equality
   becomes exact — otherwise state a relative tolerance in the test. There is also a **minimal
   witness** available that is much better than the survey's `121 606.75 / 130 610.65` pair: a
   board with one straight horizontal trace already gives `1200.002` against `1200`, and the
   `0.002` is exactly the two bounding lines.

## Files here
* `expected-outcomes.md` — the derivations, one section per fix row.
* `current-port-behavior.txt` — per-stem violation tables, the `holeClearance` item-kind
  breakdown, the port-vs-KiCad key comparison, and KiCad 10's own verdicts.
* `fixtures/kicad10-bbd-mars-64-drc.json`, `fixtures/kicad10-natural-tone-preamp-drc.json` —
  KiCad 10's verdicts, copied from the Java clone so the comparison is self-contained.

## What was NOT built here
* No golden regeneration (all 8 D stems re-cut in the task itself).
* `#110`'s exact expected integer coordinates were not pinned to a scale factor — the
  "no non-integer token anywhere in the SES" invariant is scale-independent and is the
  assertion to write.
* `#111`'s golden impact was not resolved: check whether
  `Issue029-hw48na-written.rules` / `Issue593-BBD_Mars-64-written.rules` carry a design name
  with a space. If not, the row is golden-neutral.

## Provenance
Port numbers are the committed D goldens at tag **`v1.0.0` (`ecc0abf`)** and measurements from
a read-only scratchpad clone of that tag. KiCad numbers are KiCad 10's own committed verdicts
in the Java clone (read-only). The `plan-9-post-parity` working tree was never written to.
