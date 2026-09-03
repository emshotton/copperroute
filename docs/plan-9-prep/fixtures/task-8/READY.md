# Task 8 — what the implementer gets

**Coverage: 9 of 9 fix rows have ground truth ready.**

| row | artefact | kind of ground truth | still owed by the implementer |
|---|---|---|---|
| #159 | `fixtures/p9t8-ninety-degree.dsn` + `.expected.ses` | fixture built and validated (routes, all-orthogonal); the `4` derived arithmetically | **a decision**: `[4,4,1]` (fix as sketched) vs `[4,4,4]` (also add `divide_large_room` to the 90° path) — see the ⚠ section |
| #160+#161 | two `Equal`-witness constructions + the total-order property | constructed witnesses | wiring the witnesses into `sorted_neighbours.rs` |
| #162 | termination invariant + `iterations <= border_line_count` tripwire | invariant | the `p6t3` mode-5 coverage count |
| #163 | **fully hand-derived**: 8th room `Oct[-10000,-10000,8736,8736,-18736,18736,-20000,-1264]`, 8th door `Oct[-1340,-240,-1024,76,-1416,-784,-1264,-1264]` | hand-computed, model cross-checked against 4 rooms the pinning test already prints | nothing — paste the two literals |
| #164 | two invariants (not-skipped; no panic at `len()==1`) | invariant | build the `touching_sides.len()==1` case deliberately |
| #165+#166 | three set-equality / non-empty invariants | invariant | a fixture for I2 (abandoned room with live doors) |
| #171+#170 | value-distinct pair that compares `Equal`; door-id collision `(1,63)`/`(2,32)` → 94; NaN transitivity counter-example | arithmetic | choosing reject-vs-order for NaN (the plan says reject) |
| #178 | `get_instance == None` iff queue empty | invariant | nothing |
| #156+#167+#158 | `id(item,index) == id(item+2^22,index)`; `id(1,1024)==id(0,1024)==1024`; `id(5,1024)==id(6,0)==6144` | arithmetic | nothing |

## The three things to read first

1. **`#159`'s `[4,4,4]` does not follow from the fix as written.** The plan's binding test
   asserts 4 in the 90-degree regime, but `complete_shape_90` never calls `divide_large_room`
   (`tree_ext.rs:1261`, and the port's own test comment at `tests/tree_ext.rs:414` says so).
   Restoring only the fallthrough gives **1**. Decide before Commit 2, and if the answer is
   "fallthrough only", amend the binding test's literal.

2. **`#163` is a copy-paste job now.** Both octagons above are derived from geometry the
   existing pinning test already prints, and the normalisation model used to derive them was
   validated against four of that test's own rooms (4/4 exact). The invariant behind the
   literal is *k distinct corners → k edge rooms*, so a future octagon does not need a new
   derivation.

3. **The comparator fixes have an order dependency the plan already knows about but the
   derivations sharpen.** `#160/#161`'s Witness A shows the drop happens because the
   last-corner refinement only runs on *exact* first-corner equality; `#171`'s door-id
   collision shows key 3 is a hash even after `#156` makes the room ids injective. If a real
   identity is wanted for doors, `ExpansionDoor::id` needs a counter too — record the choice.

## What was NOT built here
* No A/B or golden regeneration (that is the implementer's, and it needs the tree).
* `#162`'s production trigger board — the invariant is termination, which needs no oracle, but
  a fixture that *reaches* 0.4 % of room completions is still a corpus run, not a hand fixture.
* `p6t2`'s final MATCH count (retirement bookkeeping, needs the differential harness).

## Provenance
Everything was computed from the repo at **tag `v1.0.0` (`ecc0abf`)**, cloned read-only into
the scratchpad. The `plan-9-post-parity` working tree was never read for behaviour and never
written to. Source line numbers quoted are v1.0.0's.
