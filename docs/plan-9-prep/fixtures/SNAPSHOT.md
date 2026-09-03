# Fixture ground-truth bank — repo snapshot

COMPLETE snapshot of the Plan 9 fixture bank: 37/37 rows of pre-built directed
fixtures + independently derived expected outcomes for Tasks 8 (9/9), 11 (10/10),
12 (2 rows = 8 tests), 13 (5/5) and 19 (11/11). Produced by a read-only parallel
agent; derivations in each task-N/expected-outcomes.md, summary in task-N/READY.md.
Four plan/survey corrections discovered during derivation are indexed in README.md
and ledgered as pending rulings at the affected tasks' dispatch. Consuming tasks
copy what their tests use into crates/*/tests/data/ and commit it with the fix;
this directory is the provenance record, not the test data of record.
