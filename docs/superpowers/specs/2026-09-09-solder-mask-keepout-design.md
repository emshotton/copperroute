# Solder mask keepout: design

Date: 2026-09-09. Status: design settled; the branch base is an open decision.

## Goal

Stop the router creating solder mask bridges. A foreign net must keep clear of a pad's
mask aperture, not merely of its copper.

This is stage two of the work begun in
`2026-09-09-native-kicad-board-input-design.md`. Stage one built the data path — the
command line reads a `.kicad_pcb` directly. This stage uses it, and is the stage that
changes boards.

## The rule, measured

A pad's mask aperture is its copper grown by the board's mask expansion. KiCad reports
`solder_mask_bridge` when copper of a different net enters that aperture. The gap a
foreign net must keep from a pad's copper was measured against `kicad-cli` 10.0.3 across
sixteen configurations, to 0.01 mm:

```
threshold gap (mm) at which solder_mask_bridge clears
  expansion \ mask-to-copper    0.00   0.10   0.20   0.30
                       0.00     0.00   0.10   0.20   0.30
                       0.10     0.10   0.20   0.20   0.30
                       0.20     0.20   0.30   0.40   0.40
                       0.30     0.30   0.40   0.50   0.60
```

The required gap is `pad_to_mask_clearance + solder_mask_to_copper_clearance`. That fits
thirteen of the sixteen cells exactly. The three it misses — `(0.1, 0.2)`, `(0.1, 0.3)`
and `(0.2, 0.3)`, all where expansion is the smaller of the two nonzero terms — it
over-estimates by 0.1 mm, so it never under-clears.

The keepout the router enforces around a pad is therefore

```
max(net_class_clearance, pad_to_mask_clearance + solder_mask_to_copper_clearance)
```

`docs/solder-mask-bridge-investigation.md` currently states this as a three-way maximum,
with the two mask terms not adding. That is wrong: a maximum under-estimates in six of the
sixteen cells, which would leave in place the violations this stage exists to remove. The
error came from a sweep whose sampled points all lay where the two candidate models
coincide. This stage corrects that document.

On the benchmark corpus the distinction does not arise.
`benchmark/vendor/kicad/legacy_rules.py` lists `solder_mask_to_copper_clearance` among
`_ZEROED_RULE_KEYS`, so every generated `.kicad_pro` sets it to zero — the matrix's first
column, where the required gap is exactly the mask expansion.

## Decisions taken

| Question | Decision |
|---|---|
| Keepout formula | `expansion + mask_to_copper`, per the measurements above. |
| Where it is applied | A per-pad clearance class in the clearance matrix, raised before routing. |
| Guard against harm | Each raised floor is capped at the pad's existing gap to the nearest foreign-net pad. Raising a mask clearance may never close a gap that was already routable, and may never lower an existing copper rule. |
| Enforcement source | Extracted from PR #25, which implements exactly this and has been measured across 751 boards. |
| Representation source | Extracted from PR #22, which carries the board-model fields any enforcement needs. |
| Mask data source | The `.kicad_pcb` parser from stage one, reading `(setup pad_to_mask_clearance)` and per-pad `(solder_mask_margin)`. No sidecar file. |

## Why not the alternatives

**A sidecar metadata file.** PR #25 reaches pad mask data on the DSN path through
`COPPERROUTE_PAD_CLEARANCE_JSON`, a KiCad-derived file passed alongside the board. It
works, but it is a third artifact that must be generated per board and kept in step with
the DSN, and a run that silently lacks it routes differently with no signal. Stage one
removed the need for it: the command line reads the board KiCad wrote.

**A global clearance increase.** Raising every clearance by the mask expansion would clear
the apertures, but would also push apart nets nowhere near a pad, costing routability
across the whole board for a constraint that only applies around pads.

## Components

| File | Change | Purpose |
|---|---|---|
| `crates/copper-board/src/items/drill.rs` | extend | `Pin::solder_mask_expansion` per layer, `Pin::allow_solder_mask_bridges`. From #22. |
| `crates/copper-board/src/rules/drc_constraints.rs` | extend | `solder_mask_to_copper_clearance`, resolved from project then DSN. From #22. |
| `crates/copper-board/src/board/clearance_override.rs` | extend | `raise_solder_mask_clearances`, `solder_mask_clearance_limit`, `raise_pin_clearance`. From #25, with the sidecar loader removed. |
| `crates/copper-router/src/pipeline/board_prep.rs` | extend | Raise the clearances before routing. From #25. |
| `crates/copper-dsn/src/kicad/dto.rs` | extend | `PadJson::solderMaskExpansion`, `PadJson::allowSolderMaskBridges`. From #22. |
| `crates/copper-dsn/src/kicad/reader.rs` | extend | Populate the pin from the DTO. From #22. |
| `crates/copper-dsn/src/kicad/pcb/footprints.rs` | extend | **New.** Populate `solderMaskExpansion` from the board's `(setup pad_to_mask_clearance)` and each pad's `(solder_mask_margin)` override. |
| `crates/copper-board/tests/pad_clearance.rs` | new | From #25. |
| `docs/solder-mask-bridge-investigation.md` | correct | Replace the maximum model with the measured matrix. |

The sidecar loader in #25's `clearance_override.rs` is dropped: its purpose was to reach
mask data the DSN could not carry, which the native reader now supplies directly.

## Verification

**The formula, against KiCad.** A test sweeping the sixteen measured configurations and
asserting the keepout the board computes matches the table above.

**The cap, against routability.** A test with two foreign pads closer together than their
mask expansion would demand, asserting the raised floor is capped at their existing gap
rather than closing it.

**The parser.** The stage-one parity test extends to `solderMaskExpansion`, so the Rust
and the JavaScript adapter must agree on the per-pad value.

**End to end.** A board with a known mask bridge routes clean after the change, verified
by `kicad-cli`, and its copper clearance violations do not increase.

**The corpus.** The eleven boards with the worst `solder_mask_bridge` counts, scored before
and after by the KiCad referee. This is the measurement that says whether the stage worked,
and it must run where the corpus lives.

## Risks

Routability. Raising pad clearances makes boards harder to route, and PR #25's 751-board
run showed both large gains and real regressions — though its packaging bundles nominal
clearance with the mask floors, so the mask work's own contribution is not isolated there.
Extracting the floors alone makes that attribution possible for the first time.

Two of the eleven worst boards — `8bit-cpu_programming_interface` and
`FRM16_Relay_Module_I2C_Controller_relay_controller` — are documented in PR #25 as artwork
boards whose mask reports come from copper lost during stripping, or from mask artwork
outside the pad model. Those are not router-caused and will not improve. The achievable
win is smaller than the raw counts suggest.

## Open decision

The branch base. This stage needs PR #22's representation, and #22 is a separate open pull
request. Rebasing onto it keeps history clean and avoids duplicating those fields across
two branches, at the cost of this branch inheriting #22's review surface and merge order.
Cherry-picking only the representation keeps this branch independently mergeable but
guarantees a conflict for whichever branch merges second.

## Out of scope

Detecting mask bridges in `copper-drc` — PR #22 does that, and this stage is enforcement.
Pad-to-pad mask webs, mask artwork, and unnetted pads, none of which a router can move.
Nominal clearance, smoothing and dogleg changes, which are independent of mask work
despite sharing a commit in PR #25.
