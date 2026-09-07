# `tests/reference`

The committed outputs the test suites compare the crates against. Every file here was written
by this repository's own code, and every family is re-cut the same way:

```sh
COPPERROUTE_REGOLDEN=<label> cargo test -p <crate> --test <suite>
```

The label is free text; the run rewrites the family's files instead of comparing against them.
Inputs come from `tests/corpus`, whose layout the table files below name relative to it.

| family | files | table | cut by |
|---|---|---|---|
| DSN/SES writers | `<stem>/unrouted.ses` | `fixtures.txt` | `copper-dsn`'s `parity_ses` |
| per-connection routing | `router-<stem>/router.jsonl`, `router-steps18.jsonl` | `router-fixtures.txt` | `copper-router`'s `reference_parity` |
| whole-board routing | `router-<stem>/batch.ses`, `batch.passes.jsonl` | `router-fixtures.txt` | `copper-router`'s `batch_parity` |
| DRC reports | `drc-<stem>/drc.json` | `drc-fixtures.txt` | `copperroute`'s `cli_e2e` |
| the command line | `cli-<stem>/argv.txt`, `route.ses`, `route.exit`, `manifest.json` | `cli-fixtures.txt` | `copperroute`'s `cli_e2e` |
| KiCad's DRC | `<stem>/kicad-drc.json`, `kicad-drc.meta.txt` | `kicad-drc-fixtures.txt` | `scripts/gen-kicad-drc-reference.sh` |

The `<stem>/roundtrip.dsn` files are inputs to `copper-core`'s `stats` suite, not references.

The KiCad DRC family is the one oracle: `scripts/gen-kicad-drc-reference.sh` runs `kicad-cli pcb
drc` over each stem's board with the routed session imported, and `copper-drc`'s `kicad_oracle`
compares the checker's per-type counts against it. It needs a KiCad installation and is run by
hand.

Stems marked `slow` in `cli-fixtures.txt` and the `#[cfg_attr(debug_assertions, ignore)]` tests
run only under `COPPERROUTE_SLOW=1 cargo test --release`; re-cut those in release mode too.

`_scratch/` receives the actual output of a failed text comparison and is ignored by git.
