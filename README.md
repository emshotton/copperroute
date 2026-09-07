# freerouting-rs

An open source PCB autorouter. It started as a headless Rust fork of
[freerouting](https://github.com/freerouting/freerouting) focused on KiCad
interoperability: it reads the Specctra `.dsn` files KiCad exports, writes
the `.ses` sessions KiCad imports, and produces DRC reports in KiCad's
format. There is no GUI; everything runs from the command line or over
JSON-RPC.

## Build

Requires a Rust toolchain (see `rust-toolchain.toml`).

```sh
cargo build --release
```

The binary is `target/release/freerouting`.

## Usage

```sh
freerouting route board.dsn -o board.ses
freerouting drc board.dsn --ses board.ses -o report.json
freerouting info board.dsn
freerouting mcp
```

Run `freerouting --help` or `freerouting <command> --help` for the full
option list.

## Layout

The workspace is split into crates, each with its own README:

- `fr-geometry` — exact-arithmetic planar geometry
- `fr-board` — the board model, design rules and spatial indexes
- `fr-dsn` — `.dsn`, `.ses`, `.rules` and KiCad board JSON readers and writers
- `fr-settings` — router configuration from files, environment and CLI
- `fr-router` — fanout, maze routing and the optimizer
- `fr-drc` — the design-rule checker and KiCad DRC report writer
- `fr-core` — loading, saving, the routing job and result manifest
- `freerouting` — the command-line program

## License

GPL-3.0-only. See [LICENSE](LICENSE).

This project is a derivative work of
[freerouting](https://github.com/freerouting/freerouting), which is
distributed under the GNU General Public License version 3. Parts of the
design-rule checker in `fr-drc` are ported from
[KiCad](https://gitlab.com/kicad/code/kicad), which is distributed under
the GNU General Public License version 3 or later. Both are compatible with
distributing the combined work under GPL version 3.
