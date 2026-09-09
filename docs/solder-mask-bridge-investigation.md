# Solder mask bridges: a real routing rule the router cannot see

Routed boards scored by `kicad-cli` report `solder_mask_bridge` errors. This report
establishes where they come from, shows they are caused by the routing rather than by the
source board, and identifies the three places the rule is missing.

## Result

A solder mask aperture is the pad opening: the pad copper grown by the board's mask
expansion. KiCad reports `solder_mask_bridge` when copper of a *different* net enters that
aperture — two nets exposed through one opening will short when the board is soldered.

The keepout a foreign-net track must respect around a pad is therefore

```
max(net_class_clearance, pad_to_mask_clearance, solder_mask_to_copper_clearance)
```

The router enforces only the first term. Whenever either mask term is the larger, the
router routes to a gap that is legal under copper clearance and illegal under mask, and
`kicad-cli` reports a bridge.

## Evidence

Synthetic board, `kicad-cli` 10.0.3: one pad on `NETA`, a `NETB` track running past it at a
controlled copper-edge-to-copper-edge gap, net class clearance 0.2 mm.

With `pad_to_mask_clearance` 0.4 mm:

| gap (mm) | KiCad errors |
|---:|---|
| 0.10 | `clearance`, `solder_mask_bridge` |
| 0.15 | `clearance`, `solder_mask_bridge` |
| 0.20 | `solder_mask_bridge` |
| 0.25 | `solder_mask_bridge` |
| 0.30 | `solder_mask_bridge` |
| 0.40 | clean |

The three rows at 0.20–0.30 mm are the finding: copper clearance is satisfied, and the
board is still a DRC failure. The threshold tracks the mask term exactly — sweeping
`pad_to_mask_clearance` at 0.0 / 0.2 / 0.3 / 0.4 mm moves the first clean gap to 0.2 / 0.2 /
0.3 / 0.4 mm respectively, and setting `solder_mask_to_copper_clearance` to 0.3 mm with zero
expansion moves it to 0.3 mm. The two mask terms do not add; the larger wins.

The violations are caused by the routing. The same board, same mask settings, at a gap of
0.25 mm — copper-legal — differs only in whether the track is present:

```
routed  : {'solder_mask_bridge': 1}
stripped: {}
```

This is not pre-existing pad-to-pad noise. It is geometry the router chose, and could have
chosen differently.

## Why nothing caught it

**The router has no mask data.** `pad_to_mask_clearance` and `solder_mask_min_width` live in
the `.kicad_pcb` `(setup)` block. Neither the Specctra DSN nor the `.kicad_pro` carries them,
and `KiCadBoardJson` (`crates/copper-dsn/src/kicad/dto.rs`) has no field for either — it
carries `clearance`, `traceWidth`, `viaDiameter`, `viaDrill` and nothing about mask.
`BoardRules` has no mask concept at all.

**The port has no mask check.** `DrcViolationKind` (`crates/copper-drc/src/violation.rs:5`)
has eleven variants and none is a mask kind.

**The oracle cannot fail on it.** `oracle_counts` in
`crates/copper-drc/tests/kicad_oracle.rs:132` tallies a KiCad violation only when
`DrcViolationKind::from_kicad_type(kind).is_some()`. A check the port does not implement is
dropped from the expected counts as well as the actual ones, so a missing check is invisible
to the test by construction. The six oracle fixtures happen to carry no mask violations, so
nothing pointed at the gap from the data either.

**The benchmark scores it as noise.** `ROUTING_DRC_TYPES`
(`benchmark/bench/referee/kicad.py:39`) lists the types that count toward `violations` and
`clean_pass`. `solder_mask_bridge` is absent, so mask bridges are classed with footprint and
silkscreen findings as pre-existing artwork noise the router never touches. The experiment
above shows that classification is wrong: this type belongs in the routing set. It still
surfaces in `violations_by_type` and `violations_all`, which is where it is visible today.

## What closing it requires

1. **Represent** — carry `pad_to_mask_clearance` and `solder_mask_to_copper_clearance` into
   `BoardRules`. The DSN cannot supply them; they need a path from the `.kicad_pcb`
   `(setup)` block, alongside the existing `apply_kicad_project` route for `.kicad_pro`
   rules.
2. **Check** — add a mask violation kind and check to `copper-drc`, which also makes the
   oracle able to compare the type.
3. **Enforce** — widen the pad keepout the maze and shover use from the net class clearance
   to the maximum of the three terms. This is the half that removes the violations rather
   than reporting them.

Steps 1 and 2 make the failures visible and measurable. Only step 3 changes a board.
Because the effective keepout only grows where a mask term exceeds the net class clearance,
boards whose mask expansion is small or zero are unaffected.

## Reproducing

The measurements above come from a generated board and a gap sweep driven through
`kicad-cli pcb drc --format json`. The generator writes a board with one `NETA` pad, a `NETB`
track at a chosen gap, and chosen `pad_to_mask_clearance` / `solder_mask_min_width` values;
the sweep runs DRC per gap and counts error-severity violations by type. `solder_mask_min_width`
had no effect on this geometry at any value tested — it governs the web between two
apertures, not copper under mask.

## Can the native KiCad reader measure stage two?

Stage two needs mask settings out of a `.kicad_pcb`'s `(setup)` block, which the native
reader (`copper_dsn::kicad::pcb::read_pcb`) reads directly rather than through a DSN plus
`--kicad-board`. Whether stage two can run on that path instead depends on how much of the
corpus the reader accepts, which requires the corpus.

`crates/copper-dsn/tests/kicad_pcb_corpus.rs` checks the eleven cells with the largest
`solder_mask_bridge` counts; `scripts/kicad-pcb-import-survey.sh <corpus root>` runs
`copperroute info` across the whole corpus and reports an accept rate. Both are opt-in and
have not been run against the corpus: it lives on em@workbench, not in this checkout. Run

```
COPPERROUTE_PCBENCH=<corpus root> cargo test -p copper-dsn --test kicad_pcb_corpus -- --nocapture
scripts/kicad-pcb-import-survey.sh <corpus root>
```

to get the accept rate and the per-board refusal reasons before stage two is planned. Sort
each refusal into one of two categories:

- a **porting bug**, where `node crates/copper-dsn/tests/data/kicad_pcb_parity.mjs <board>`
  accepts the same board the Rust refuses — fix it;
- a **shared limitation**, where the JS adapter refuses it too, for a construct neither
  supports — leave it, and record it here rather than chasing a bug that is not there. See
  `crates/copper-dsn/README.md`'s "Reading a `.kicad_pcb` file directly" section for the
  current list of constructs `read_pcb` refuses outright; do not copy that list here, since a
  second copy is exactly what would let this section and the README drift apart.

If the accept rate against the corpus turns out low and the refusals are mostly shared
limitations rather than porting bugs, stage two should reach the mask settings through
`--kicad-board` beside a DSN instead of the native path.
