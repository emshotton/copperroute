# `copper-settings` test data

The expected values in `crates/copper-settings/tests/{field_path,router_settings,board_optimizations,sources,env_source,cli_source,json}.rs`
are literals in the tests themselves; the files here are the committed inputs.

## Committed inputs

`Plan4Matrix-primary.rules` and `Plan4Matrix-adjacent.rules` are Task 8's two minimal
`(autoroute_settings)` scopes — the `-dr` file and the scheduler's file of the precedence matrix
(`crates/copper-settings/tests/matrix/mod.rs`). They name `F.Cu`/`B.Cu` so that a driver's
`RulesReader.read` can map them onto a synthetic two-layer board, and they disagree on every
value they carry, so a case where merge #1 and merge #2 see different rules cannot pass by
coincidence.

`Issue029-hw48na_reduced.rules` is `tests/corpus/fixtures/Issue029-hw48na_valid.rules` with
eight lines deleted — `(vias on)`, `(via_costs 50)`, `(plane_via_costs 5)`,
`(start_ripup_costs 100)` and the four per-layer trace-cost lines. It is the input `SProbe` block
`H` and `tests/sources.rs::an_unnamed_rules_field_does_not_overwrite_a_lower_priority_source`
share: a scope that names *some* fields and omits others, which is what distinguishes "absent"
from "the coalesced default".
