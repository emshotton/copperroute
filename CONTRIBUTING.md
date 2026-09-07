# Contributing

## Pull requests

All PRs that change behaviour should include a report from the benchmarking tool to validate
that there are no regressions in performance or quality. The report should include a comparison
between the latest HEAD of main and the proposed changes.

PRs with significant quality improvements that come at the cost of performance may be merged on
a case-by-case basis.

A PR that only touches documentation, tests or tooling does not need a benchmark run. Say so in
the description instead.

## Benchmarking a change

Route every board with `main` and with your branch, then paste the summary into the PR.

You need a Rust toolchain, [uv](https://docs.astral.sh/uv/), and KiCad providing `kicad-cli`
and a Python with `pcbnew`. No Java toolchain is needed.

Import the corpus once per machine. This clones
[PCBench](https://github.com/emshotton/PCBench) if it is missing:

```bash
cd benchmark && uv sync
uv run bench corpus pcbench --clone ~/PCBench --licensed-only --jobs 4
```

Build both sides into separate target directories, from the repository root:

```bash
git fetch origin
git worktree add --detach ../copperroute-main origin/main
cargo build --release --locked --manifest-path ../copperroute-main/Cargo.toml \
  --target-dir ../copperroute-main/target
cargo build --release --locked
```

Create `benchmark/candidates.local.toml`, with paths relative to that file:

```toml
[candidates.main]
kind = "rust"
exec = ["../../copperroute-main/target/release/copperroute"]
sha_from = "../../copperroute-main"

[candidates.change]
kind = "rust"
exec = ["../target/release/copperroute"]
sha_from = ".."
```

Run, compare, and print the summary. This is 1182 boards per side, so match `--jobs` to your
cores and expect it to take hours:

```bash
uv run bench run --candidates-file candidates.local.toml --candidates main,change \
  --tier pcbench --seeds 1 --threads 1 --jobs 12 --max-passes 10 --timeout 300 \
  --run-id main-vs-change
uv run bench compare --baseline main --against change --runs main-vs-change \
  --out main-vs-change --fail-on-regression
uv run bench pr-summary --compare main-vs-change
```

Paste the last command's output into the pull request. One repetition settles quality because
the router is deterministic; add `--seeds 3` if you are claiming a timing improvement.

[benchmark/README.md](benchmark/README.md) covers the rest of the suite.

## Comments

This codebase is heavily edited by AI. Best practices for how to comment code for agents is
still in flux, and as such we have chosen to adopt the policy that comments in code should be
kept to an absolute minimum. They should only ever be used to indicate unexpected behaviour.
The need for comments is a failure of the code to be clear and understandable. They should:

- Not duplicate information that's already available by reading the code.
- Not describe the previous or future state of code.
- Not refer to design documents, tickets, or plans.
