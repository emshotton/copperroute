# Fixture ground-truth bank — repo snapshot

Snapshot of the Plan 9 fixture bank (pre-built directed fixtures + independently
derived expected outcomes for Tasks 8/11/12/13/19). The bank is produced by a
read-only parallel agent; this snapshot preserves it in-repo. The agent may still
be adding rows — this snapshot will be refreshed when the bank completes (check
the ledger for "fixture bank complete"). Consuming tasks copy what their tests
use into crates/*/tests/data/ and commit it with the fix; this directory is the
working set / provenance record, not the test data of record.
