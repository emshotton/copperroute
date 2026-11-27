# CopperRoute for KiCad

A KiCad 10/11 plugin, built on KiCad's IPC API, that autoroutes the board open in the PCB editor
and adds the result as one undoable change.

## Actions

| Action | What it does |
|---|---|
| **Autoroute board** (toolbar button) | Routes every missing connection. Existing tracks and vias stay as they are. |
| **Autoroute selected nets** | Routes only the nets of the selected tracks, pads or footprints. |
| **Rip up and reroute** | Removes unlocked tracks and vias, on the selected nets if anything is selected, and routes them again. |

Locked tracks and vias, curved tracks, and blind, buried and micro vias are never moved or
removed. Keepout areas, zone fills, net classes from the `.kicad_pro` and the common rules in a
`.kicad_dru` (clearance, track width, via size and hole size by net class, and edge clearance)
are respected. Differential pairs and length-tuned nets are left for KiCad's own tools.

Progress and the final summary appear in the PCB editor's status bar messages (KiCad 10.0.1+).

## Settings

Put a `copperroute.json` beside the project to change the defaults:

```json
{
  "maxPasses": 20,
  "timeoutSeconds": 300,
  "optimize": true,
  "respectZones": true,
  "routeTunedNets": false,
  "refillZones": true
}
```

## Installing

The plugin needs the IPC API: in KiCad, enable **Preferences → Plugins → Enable KiCad API**.

- **Plugin and Content Manager:** install the `copperroute-kicad-<version>.zip` package with
  **Install from File...**.
- **By hand:** copy the `copperroute` folder into KiCad's plugins folder
  (`~/.local/share/kicad/<version>/plugins` on Linux,
  `~/Documents/KiCad/<version>/plugins` on Windows and macOS).

KiCad then creates the plugin's Python environment and installs `requirements.txt`, which pulls
the prebuilt router (`copperroute-kicad`) and `kicad-python` from PyPI.

## Development

```sh
python -m venv .venv && . .venv/bin/activate
pip install maturin kicad-python pytest
(cd ../bindings/python && maturin develop --release)
pytest -q tests
python scripts/make_icons.py      # redraw the toolbar icons
python scripts/build_package.py   # build the PCM zip into dist/
```

`scripts/check_with_kicad.py` routes example boards with the `copperroute` CLI and checks the
results with `kicad-cli pcb drc`; CI runs it in the `kicad/kicad` container.
