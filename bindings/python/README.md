# copperroute-kicad

The CopperRoute autorouter as a Python package, for the KiCad plugin in `kicad-plugin/`.

```python
import copperroute_kicad

result = copperroute_kicad.route(board_text, ripUp=False, respectZones=True,
                                 progress=lambda update: False)
result["changes"]  # {"addTracks": [...], "addVias": [...], "removeIds": [...]}
```

Build it with [maturin](https://www.maturin.rs): `maturin develop --release`. It is kept out of
the Cargo workspace so `cargo test --workspace` does not need Python.
