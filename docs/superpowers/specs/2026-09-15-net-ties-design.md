# Net ties: design

Date: 2026-09-15. Status: approved in discussion, awaiting spec review.

## Goal

Route and check boards that contain net ties: footprints whose pads deliberately
short two or more nets together.

Today such a board is refused outright. `pcb/footprints.rs` and `web/kicad.js`
both stop at the first `net_tie_pad_groups` they see, and the whole import fails
with "Net ties are not supported yet." Two corpus boards are lost this way
(`docs/native-drc-coverage.md`).

After this work a tie board imports, routes, and passes the native DRC with the
intended short exempt and every unintended short still reported.

## What a net tie is

A net tie footprint joins nets that the schematic keeps apart: an analog and a
digital ground meeting at one star point, a shield stitched to a return path. The
join is copper inside the footprint, made either by pads that overlap directly or
by a copper graphic laid across them. `(net_tie_pad_groups "1,2" "3,4")` on the
footprint names which pad numbers are allowed to short.

The tie is the *only* place those nets may meet. Copper joining them anywhere
else is a real defect, and a router that treats tied nets as interchangeable will
produce one.

## How KiCad handles it

`DRC_ENGINE::EvalRules` zeroes the clearance constraint for a pair of items when
`FOOTPRINT::GetNetTieCache( child_item )` contains the other item's netcode. Two
properties of that code shape this design:

- The cache is keyed per **child item**, not per footprint. A pad carries its
  group's netcodes; a graphic carries the netcodes of the pads it touches.
- The exemption applies when exactly one of the two items is a connected item, or
  when both items belong to the same footprint. Two connected items of different
  nets in different footprints are never exempt.

The second point is why tie pads must keep their own single net. A trace of a
foreign net landing on a tie pad is two connected items in different footprints,
and it has to stay a violation.

## Decisions taken

| Question | Decision |
|---|---|
| Net of a tie pad | Its own net, unchanged. Pads never become multi-net. |
| Net of tie copper graphics | The nets of the tie-group pads the graphic overlaps. Multi-net, using the `net_nos: Vec<i32>` the item model already has. |
| Where the exemption lives | `Item::is_obstacle` for the board model, which both the router and clearance reporting call, and a sibling predicate in `copper-drc`, which does its own pairing. |
| DRC reach | Exempt the intended short; keep reporting a foreign net on a tie pad and a short between pads of different groups. |
| Input paths | The native `.kicad_pcb` reader and `web/kicad.js`. |
| DSN path | Unchanged. KiCad's Specctra exporter drops `net_tie_pad_groups`, and inferring ties from overlap would misread genuinely shorted boards. |
| Degenerate ties | Warn and import. An unknown pad number or a group spanning fewer than two nets must not fail the board. |

## Why not the alternatives

**Multi-net tie pads.** Giving each tie pad its whole group's net list makes the
short vanish with almost no new code, because every `shares_net_no` site then
agrees. It also makes `Item::is_obstacle` (`items/mod.rs:343`, literally
`!self.contains_net(net_number)`) report that net A may land on pad B. The router
takes that permission and the referee calls the result a short.

**Merging tied nets.** Treating the tied nets as one net internally and splitting
on export gives the simplest connectivity, and is electrically honest. It also
frees the router to join the nets anywhere on the board, bypassing the tie, and
it makes the incompleteness counts diverge from KiCad's ratsnest.

Both alternatives fail for the same reason: they encode "these nets are the same"
when the truth is "these nets meet at exactly one place".

## Data model

`ComponentJson` gains

```rust
pub netTieGroups: Option<Vec<Vec<String>>>,
```

one inner vector per group, holding pad numbers as written in the file.
`ConductionAreaJson` gains `netNames: Option<Vec<String>>` beside the existing
singular `netName`, so tie copper can name more than one net.

Both fields are transcriptions of the KiCad source file with no inference
applied, which is what keeps the Rust parser and `web/kicad.js` in parity.

`copper-board` gains a `rules::net_ties` module:

```rust
pub struct NetTies {
    pad_nets: HashMap<ItemId, Vec<i32>>,
}
```

the analogue of KiCad's net tie cache, restricted to pads. `BoardRules` holds one
`net_ties: NetTies`. It is empty on every board that has no tie, and every query
against it short-circuits on that.

Tie copper needs no entry. A `ConductionArea` whose `net_nos` holds both nets is
already transparent to both and opaque to everything else, which is the same
statement the cache would make about it.

## Data flow

`.kicad_pcb` → `pcb/footprints.rs` → `KiCadBoardJson` → `kicad/reader.rs` →
`Board`, with `web/kicad.js` producing the same DTO for the browser.

`pcb/footprints.rs` reads the groups onto the component and, for each copper
graphic in a tie footprint, tests the graphic's shape against the tie-group pads
to fill `netNames`. A graphic that touches no tie pad stays a netless obstacle.

`kicad/reader.rs`, after inserting a component's pins, resolves each group's pad
numbers to pin `ItemId`s and each pad's `netName` to a netcode, then registers
every pin in the group with that group's netcodes. Conduction areas resolve
`netNames` to the net number vector `insert_conduction_area` already accepts.

## Obstacle and clearance semantics

`Item::is_obstacle(&self, other, ctx)` (`items/mod.rs:266`) receives an `ItemCtx`
that carries `rules`, so the exemption goes there: two pins are not obstacles to
each other when each one's registered netcode set contains the other's net.

Both consumers pick it up from that one edit. The router's obstacle tests call
it, and so does `board/clearance.rs:81`, which is what reports clearance
violations in the statistics.

What the design does *not* do carries as much weight. Because pads stay
single-net, a foreign trace on a tie pad is still an obstacle and still a
violation, and pads in different groups on the same footprint are still obstacles
to each other. The policing is inherited from not weakening the model.

## DRC

`checks/copper.rs` gains `net_tie_exempt(board, a, b)` beside the existing
`same_logical_pad` (`copper.rs:27`), consulted in `check_pair` where `same_net`
and `same_pad` already are. It is exempt when both items belong to the same
component and each is in the other's tie set, or when one item is unconnected tie
copper whose net set contains the other's net. Every other pair keeps its normal
clearance, so `ShortingItems` still fires on a genuine short.

## Browser path

`web/kicad.js` stops throwing at its `net_tie_pad_groups` check and emits
`netTieGroups` and the overlap-derived `netNames`, mirroring the Rust parser.
`tests/kicad_pcb_parity.rs` holds the two to byte parity.

`kicad/writer.rs` needs no change: it emits layers, nets, outline, traces, vias
and conduction areas, and never components.

## Verification

Unit tests per layer:

- the parser emits groups for a tie footprint, replacing the rejection test at
  `kicad_pcb_footprints.rs:99`, and warns rather than fails on a group naming an
  unknown pad or spanning one net;
- the parser gives a copper graphic across two tie pads both nets, and a graphic
  touching neither no net;
- the reader registers the pads and builds the multi-net conduction area;
- `is_obstacle` exempts two in-group pads and still blocks a foreign net's trace
  from both a tie pad and tie copper;
- the DRC reports nothing for the intended short, and still reports a foreign
  trace landing on a tie pad and a short between two groups of one footprint.

Acceptance: the two previously-rejected corpus boards import, route, and are
scored by the local KiCad referee with no new violations — the intended short
absent from the report, no foreign copper on the tie pads.

## Risks

The overlap test that assigns nets to tie copper is written twice, in Rust and in
JavaScript, and the parity test compares only the boards it is run on. A graphic
that grazes a pad could be classified differently by the two. Keeping the test to
a plain shape intersection, with no tolerance term, limits the room for
divergence.

`FOOTPRINT::BuildNetTieCache`'s exact rule for assigning netcodes to graphics has
not been read, only inferred from its per-child-item key and from `EvalRules`.
Implementation should read it and the referee should confirm the result.

## Out of scope

Ties on the DSN path. Ties inferred from geometry. Writing a tie back out to a
`.kicad_pcb`. KiCad custom DRC rules that mention net ties.
