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
`FOOTPRINT::GetNetTieCache( child_item )` contains the other item's netcode.
Three properties of that code shape this design:

- The cache is keyed per **child item**, not per footprint. A pad carries its
  group's netcodes; a graphic carries the netcodes of the pads it touches.
- The exemption applies when exactly one of the two items is a connected item, or
  when both items belong to the same footprint. Two connected items of different
  nets in different footprints are never exempt.
- Only the clearance constraint is zeroed. Hole clearance and the rest still
  apply.

The second point is why tie pads must keep their own single net. A trace of a
foreign net landing on a tie pad is two connected items in different footprints,
and it has to stay a violation.

## Decisions taken

| Question | Decision |
|---|---|
| Net of a tie pad | Its own net, unchanged. Pads never become multi-net. |
| Net of tie copper graphics | The nets of the tie-group pads the graphic overlaps. Multi-net, using the `net_nos: Vec<i32>` the item model already has. |
| Where a group is recorded | On the pad, as the net names that pad may short with. The parser emits one KiCad footprint as many single-pad components, so a group cannot ride on a component. |
| What identifies a group | `PadJson.sourceFootprint`, the footprint index the parser already emits. |
| Who runs the overlap test | The reader, where real shapes exist. The DTO carries only which footprint a graphic came from. |
| Where the exemption lives | `Item::is_obstacle` for the board model, which both the router and clearance reporting call, and `pair_clearance` in `copper-drc`, which does its own pairing. |
| DRC reach | Zero the clearance for the intended short, as KiCad does. Keep reporting a foreign net on a tie pad, a short between pads of different groups, and hole clearance throughout. |
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

`PadJson` gains

```rust
pub netTieNets: Option<Vec<String>>,
```

the names of the nets this pad is allowed to short with — the other nets of its
group. `ConductionAreaJson` gains `sourceFootprint: Option<String>`, matching the
field `PadJson` already carries, set only on copper graphics that come from a
footprint with a tie group.

Both fields are transcriptions of the KiCad source file with no geometry applied,
which is what keeps the Rust parser and `web/kicad.js` in parity.

`copper-board` gains a `rules::net_ties` module:

```rust
pub struct NetTies {
    pads: HashMap<ItemId, NetTiePad>,
    by_footprint: HashMap<String, Vec<ItemId>>,
}
```

where a `NetTiePad` holds the pad's footprint and the netcodes it may short with.
This is the analogue of KiCad's net tie cache, restricted to pads. `BoardRules`
holds one `net_ties: NetTies`. It is empty on every board that has no tie, and
every query against it short-circuits on that.

Tie copper needs no entry. A `ConductionArea` whose `net_nos` holds both nets is
already transparent to both and opaque to everything else, which is the same
statement the cache would make about it.

## Data flow

`.kicad_pcb` → `pcb/footprints.rs` → `KiCadBoardJson` → `kicad/reader.rs` →
`Board`, with `web/kicad.js` producing the same DTO for the browser.

`pcb/footprints.rs` reads each footprint's groups, resolves each group's pad
numbers to the net names those pads carry, and writes every group member's
`netTieNets`. Copper graphics of a footprint that has a group get its index in
`sourceFootprint`.

`kicad/reader.rs` registers each pad that carries `netTieNets` in `NetTies`,
under its `sourceFootprint` and with the netcodes those names resolve to.
Conduction areas are inserted after pins, so when one names a source footprint
the reader intersects its polygon with that footprint's registered pads on the
same layer and passes the union of the matching pads' groups to
`insert_conduction_area`, which already takes a net number vector. A graphic that
intersects no tie pad stays a netless obstacle.

## Obstacle and clearance semantics

`Item::is_obstacle(&self, other, ctx)` (`items/mod.rs:266`) receives an `ItemCtx`
that carries `rules`, so the exemption goes in its `Item::Pin` arm, at the point
where a foreign net currently returns "obstacle": two pins registered under the
same footprint, each one's netcode set holding the other's net, are not obstacles
to each other.

Both consumers pick it up from that one edit. The router's obstacle tests call
it, and so does `board/clearance.rs:81`, which is what reports clearance
violations in the statistics.

What the design does *not* do carries as much weight. Because pads stay
single-net, a foreign trace on a tie pad is still an obstacle and still a
violation, and pads in different groups of one footprint are still obstacles to
each other. The policing is inherited from not weakening the model.

## DRC

`pair_clearance` (`constraints.rs:72`) returns `Some(0)` for two pads that may
short, which is KiCad's own move. `checks/copper.rs` already skips its clearance
block when the clearance is not above zero, and its hole clearance section runs
regardless — so hole checks keep applying to tie pads, as they do in KiCad.

Tie copper needs nothing here either: `same_defined_net` already sees the shared
net between a graphic carrying both nets and a trace on one of them.

## Browser path

`web/kicad.js` stops throwing at its `net_tie_pad_groups` check and emits
`netTieNets` and `sourceFootprint`, mirroring the Rust parser. No geometry is
duplicated: the overlap test lives in the reader, which the browser path reaches
through the same DTO. `tests/kicad_pcb_parity.rs` holds the two to byte parity.

`kicad/writer.rs` needs no change: it emits layers, nets, outline, traces, vias
and conduction areas, and never components.

## Verification

Unit tests per layer:

- the parser emits `netTieNets` for each pad of a group, replacing the rejection
  test at `kicad_pcb_footprints.rs:99`, and warns rather than fails on a group
  naming an unknown pad or spanning fewer than two nets;
- the parser tags a tie footprint's copper graphics with `sourceFootprint` and
  leaves an ordinary footprint's graphics untagged;
- the reader registers the pads and gives a graphic across two tie pads both
  nets, and one clear of them no net;
- `is_obstacle` exempts two in-group pads, and still blocks a foreign net's trace
  from a tie pad, two pads of different groups from each other, and two pads of
  different footprints that happen to hold the tied nets;
- the DRC reports nothing for the intended short, and still reports a foreign
  trace landing on a tie pad.

Acceptance: the two previously-rejected corpus boards import, route, and are
scored by the local KiCad referee with no new violations — the intended short
absent from the report, no foreign copper on the tie pads.

## Risks

`FOOTPRINT::BuildNetTieCache`'s exact rule for assigning netcodes to graphics has
not been read, only inferred from its per-child-item key and from `EvalRules`.
Implementation should read it and the referee should confirm the result.

The parser reduces every footprint copper graphic to its bounding box before the
reader ever sees it. A graphic that only nearly touches a tie pad can therefore
be judged to overlap it. The error is conservative — it grants the tie's nets
access to copper that is already an obstacle to everything else — but it is a
place where our geometry is coarser than KiCad's.

## Out of scope

Ties on the DSN path. Ties inferred from geometry. Writing a tie back out to a
`.kicad_pcb`. KiCad custom DRC rules that mention net ties.
