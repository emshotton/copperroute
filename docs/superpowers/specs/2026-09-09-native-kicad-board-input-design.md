# Native KiCad board input: design

Date: 2026-09-09. Status: approved in discussion, awaiting spec review.

## Goal

Let the command line route a `.kicad_pcb` directly, with no Specctra DSN
involved.

```
copperroute route board.kicad_pcb -o out.ses
```

This is stage one of two. It adds an input path and changes no routing
behaviour. Stage two, specified separately, carries solder mask expansion into
`BoardRules`, widens the router's pad keepout, and adds `--kicad-board` so a DSN
run can borrow board setup from a `.kicad_pcb` alongside it. Stage two depends on
this stage's parser for its data and is the stage that changes boards.

`--kicad-board` is deliberately held back. In stage one it would parse a setup
block and discard it, since nothing yet consumes those values; a flag that
accepts input and does nothing is worse than an absent one.

## Why this stage exists

Routed corpus boards fail `kicad-cli` with `solder_mask_bridge` errors. The
cause is measured in `docs/solder-mask-bridge-investigation.md`: the keepout a
foreign net must respect around a pad is
`max(net_class_clearance, pad_to_mask_clearance + solder_mask_to_copper_clearance)`,
and the router enforces only the first term. The two mask terms live in the
`.kicad_pcb` `(setup)` block. Neither a Specctra DSN nor a `.kicad_pro` carries
them, so no data path reaches the router at all. This stage builds that path,
and is worth having on its own as a command line capability.

## Decisions taken

| Question | Decision |
|---|---|
| Target of the new parser | `KiCadBoardJson`, the DTO `kicad::reader` already consumes. Not `Board` directly. |
| Where it lives | `copper-dsn::kicad`, which already owns `dto.rs` and `reader.rs`. |
| Relationship to `web/kicad.js` | The Rust parser is a port of its import half. The JS remains for now; the web path can call the Rust one through wasm later, retiring the duplicate. |
| Output format | Unchanged. A `.kicad_pcb` input writes a `.ses`, so the benchmark referee pipeline is untouched. |
| Export | Out of scope. Writing a routed `.kicad_pcb` back out stays in the web app. |
| Correctness argument | Differential testing against the JS adapter on the example boards. |

## Why not the alternatives

**Parsing `.kicad_pcb` straight to `Board`** skips an intermediate structure but
duplicates net resolution, padstack construction, clearance class assignment and
outline handling that `kicad::reader` already performs and tests. It would leave
two ways to build a `Board` from KiCad data, diverging under maintenance.

**A separate crate for the parser** splits KiCad ingestion across two crates.
`copper-dsn` already hosts the KiCad DTO and reader despite its name.

**Extending the Specctra exporter** with a mask property was the other way to
reach the same data. Specctra permits it — `(property ...)` is the grammar's user
property escape hatch, and KiCad already emits `(property (index 0))` per layer —
but it only helps boards whose DSN we exported ourselves, requires re-preparing
the cached corpus, and must be re-verified against the seven Java freerouting
jars still listed as benchmark baselines. Reading the board KiCad already wrote
avoids all three.

## What the parser must cover

Ported from `importBoard` in `web/kicad.js`, which is the working reference:

- Copper layers, rejecting anything that is not `signal`, `mixed` or `power`, capped at 32.
- Nets, including the named-net form used from board `version` 20260101.
- Edge.Cuts outline: loop assembly, largest loop as the boundary, inner loops as
  cutouts, rejection of separate outlines and unclosed edges.
- Copper text and footprint copper rectangles, reserved as rectangular obstacle
  conduction areas.
- Zones: validated and counted, not routed against. Copper zones and
  track/via-permitting keepouts pass through as warnings; KiCad refills them
  afterwards. This matches the DSN path, which also carries no zones — KiCad's
  own Specctra export drops them.
- Footprints to components and pads: one component per pad, carrying absolute
  placement and angle, because the DTO has component rotation but not pad
  rotation. Roundrect ratio, custom convex pads including stroke radius, plated
  slots, drills, and `*.Cu` / `F&B.Cu` layer spans.
- Tracks and through vias, skipping ripped-up ones.
- Embedded net classes and their net assignments, with a synthesised `Default`
  when the board has none.

Unsupported constructs keep the JS wording so the command line and the web app
refuse the same boards for the same stated reason: net ties, footprint zones,
footprint copper graphics, locked tracks, blind and micro vias, curved tracks,
concave custom pads, netless copper zones, restrictive keepouts.

## Components

| File | Change | Purpose |
|---|---|---|
| `crates/copper-dsn/src/kicad/sexpr.rs` | new | KiCad s-expression node reader: nested nodes, quoted strings with escapes, a depth cap. |
| `crates/copper-dsn/src/kicad/pcb.rs` | new | `.kicad_pcb` text to `KiCadBoardJson`, plus the warnings the adapter produces. |
| `crates/copper-dsn/src/kicad/reader.rs` | split | `read_board_json(KiCadBoardJson, ...)` holds the body; `read_board(&str, ...)` deserialises and delegates. The new parser hands over the DTO in memory. |
| `crates/copper-core/src/job.rs` | extend | `FileFormat::KicadPcb`: variant, `name()`, the `kicad_pcb` extension arm, content sniffing on a leading `(kicad_pcb`. |
| `crates/copper-core/src/load.rs` | extend | `parse_board_if_needed` gains a third arm; the two format guards accept it. |
| `crates/copperroute/src/ops/load.rs` | extend | Accepted-format guard widened to admit the new format. |

`drc` and `info` accept `.kicad_pcb` without further change, since they load
through `ops::load`.

## Data flow

```
board.kicad_pcb ──> kicad::pcb ──> KiCadBoardJson ──> kicad::reader ──> Board ──> router ──> .ses
                                        ^
web/kicad.js ─────> (JSON string) ──────┘        (unchanged wasm path)

in.dsn ───────────> dsn parser ──────────────────────────────────────> Board
```

Stage two adds one edge to this diagram: a `.kicad_pcb` parsed by the same
`kicad::pcb` module for its `(setup)` block alone, applied to a `Board` that came
from a DSN.

## Verification

**Differential against the JS reference.** A test runs the JS adapter through
`node` over `web/example.kicad_pcb` and `web/examples/*`, and compares the
resulting `KiCadBoardJson` to the Rust parser's, field for field. The test skips
when `node` is absent so it does not break environments without it. This is the
primary correctness argument: the JS adapter is in production behind the web app,
so agreement with it is agreement with known-good behaviour.

**Round trip through the existing reader.** Each example board loads to a `Board`;
net counts, pad counts and outline area are asserted. Where a board also has a
`.dsn`, the two paths are compared.

**Rejection cases.** A table test over the unsupported constructs asserts the
same errors the JS raises.

**Command line.** `copperroute route web/example.kicad_pcb -o out.ses` produces a
session, and `copperroute info` reports the same board summary for a `.kicad_pcb`
and its exported `.dsn`.

## Risks

Floating point differences between the JS and Rust geometry, particularly the
custom pad convex hull and the curved outline approximation, may make exact
differential comparison too brittle. The test compares numbers within the
existing 0.005 mm outline tolerance rather than exactly.

The parser is a large single file. If `pcb.rs` grows past roughly 700 lines the
footprint and pad handling separates into its own module, since that is the part
with the most independent logic.

## Out of scope

Writing a routed `.kicad_pcb`. Retiring the JS adapter. Copper zone routing.
The `--kicad-board` side-load flag. Anything that changes routing behaviour,
including the mask keepout itself.
