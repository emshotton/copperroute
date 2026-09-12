# CoppeRoute

An open source PCB autorouter. This started as a rust port of [freerouting](https://github.com/freerouting/freerouting) that got out of hand.

[Live Web Demo](https://copperroute.net)

## Build

Requires a Rust toolchain (see `rust-toolchain.toml`).

```sh
cargo build --release
```

The binary is `target/release/copperroute`.

## Usage

```sh
copperroute route board.dsn -o board.ses
copperroute drc board.dsn --ses board.ses -o report.json
copperroute info board.dsn
copperroute mcp
```

Run `copperroute --help` or `copperroute <command> --help` for the full
option list.

Corridor guidance is enabled by default: related signals are encouraged to share
routing corridors. To disable it for a run:

```sh
COPPERROUTE_CORRIDOR_GUIDANCE=0 copperroute route board.dsn -o board.ses
```

### With a KiCad project

`copperroute` reads a `.kicad_pcb` board directly:

```sh
copperroute route board.kicad_pcb -o board.ses
copperroute drc board.kicad_pcb --ses board.ses -o report.json
```

KiCad 6 and later keep net classes in the `.kicad_pro` project file, not the
board, so a `.kicad_pcb` from those versions carries no clearance or via
sizing of its own. Pass `--kicad-project` alongside it — the flag applies to
a `.kicad_pcb` input the same way it does to a `.dsn` — unless the board is
old enough to embed its own `(net_class ...)` rules:

```sh
copperroute route board.kicad_pcb --kicad-project board.kicad_pro -o board.ses
copperroute drc board.kicad_pcb --kicad-project board.kicad_pro --ses board.ses -o report.json
```

The same flag routes a Specctra export using the project's design rules
instead of the defaults baked into the DSN:

```sh
copperroute route board.dsn --kicad-project board.kicad_pro -o board.ses
copperroute drc board.dsn --kicad-project board.kicad_pro --ses board.ses -o report.json
```

Import `board.ses` back into KiCad with **File → Import → Specctra
Session**. The DRC report uses KiCad's own report schema, so it can be
compared directly with the output of `kicad-cli pcb drc`.

## Layout

The workspace is split into crates, each with its own README:

- `copper-geometry` — exact-arithmetic planar geometry
- `copper-board` — the board model, design rules and spatial indexes
- `copper-dsn` — `.dsn`, `.ses`, `.rules` and KiCad board JSON readers and writers, and a `.kicad_pcb` reader
- `copper-settings` — router configuration from files, environment and CLI
- `copper-router` — fanout, maze routing and the optimizer
- `copper-drc` — the design-rule checker and KiCad DRC report writer
- `copper-core` — loading, saving, the routing job and result manifest
- `copperroute` — the command-line program

## License

GPL-3.0-only. See [LICENSE](LICENSE).

This project is a derivative work of
[freerouting](https://github.com/freerouting/freerouting), which is
distributed under the GNU General Public License version 3. Parts of the
design-rule checker in `copper-drc` are ported from
[KiCad](https://gitlab.com/kicad/code/kicad), which is distributed under
the GNU General Public License version 3 or later. Both are compatible with
distributing the combined work under GPL version 3.
