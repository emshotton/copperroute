# CopperRoute homepage

A static webpage that accepts a `.kicad_pcb` by dropping it onto the routing
preview window or using the file picker,
runs the existing Rust autorouter in a WebAssembly module worker, displays an
SVG copper preview, and downloads the routed PCB and SVG. No board data is
uploaded. **Route board** removes the existing top-level segments, track arcs and
vias of the selected nets (including locked items) and routes those from scratch.
The initial preview shows the original board; clicking Route shows the unrouted
copy while WASM runs, then updates live with routed copper. The pass number,
remaining connections, and routes completed in the pass update alongside it.
Footprint pads and through holes remain. Original files
on disk are not modified.

The preview appears immediately on file selection, independently of
routing compatibility. Drop `.kicad_pcb` and `.kicad_pro` together to import project
rules, or use the separate optional project picker. Includes a tiny example board
and an immediate Cancel button.

**Board rules** and **Nets to route** are collapsed panels sharing one style.
Each summary names its current state — the project file, embedded classes or
manual rules, and the number of selected nets — so both stay readable while
closed. Board rules holds the zone refill and via-in-pad options.

The example board loads and renders straight away, under a hint inviting a
board to be dropped on the preview. The hint clears on the first click, key
press, scroll or drag, leaving the board visible.

A terminal-styled block below the results carries the commands for cloning and
building the command-line router from GitHub, which the header also links to,
and for serving the routing tools over MCP.
Import notes are no longer surfaced; the status line keeps the pass count and
the review reminder, and DRC findings stay in their own disclosure.

The preview header carries the routing result — remaining connections and the
router DRC count, split into violations that predate the route and new ones.
The line below the preview keeps the pass count and the review reminder.

## Run

From the repository root:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.127 --locked
cd web
npm ci
npm run build
npm start
```

Open http://127.0.0.1:8080. Easyduino Uno loads automatically with its project rules.
Choose Uno or Nano in the example selector, then click **Route board**.
**Try a small example** loads a minimal two-net board.
Rust and the matching wasm-bindgen CLI are build dependencies; Node is used for
scripts/tests, and Python 3 serves the static files. The generated `pkg/` is
ignored by Git. Serve the `web/` directory over HTTP(S), not `file://`.
Any static host serving `.wasm` as `application/wasm` can host the page. There
are no runtime CDN dependencies, backend services, or cross-origin isolation
requirements; the logo face is the vendored
`fonts/bakbak-one-latin.woff2` (9.5 KiB), served from the same origin under
the SIL Open Font License in `fonts/OFL.txt`, so no request leaves the page. The current release WASM is
approximately 2.9 MiB uncompressed.

## Net selection

**Nets to route** lists every net on the board with its net class. All nets
start selected, so the default route is unchanged. Deselecting a net excludes
it from routing and keeps its existing tracks and vias, which the router treats
as fixed obstacles; only the selected nets are ripped up and routed from
scratch. Unselected nets stay unrouted and are counted in the remaining
connections. **Select all** and **Select none** apply to the nets currently
matching the filter box. Routing is blocked while no net is selected.

The list comes from the board itself, so it appears once the preview loads and
resets with each new board. Selection is passed to the router by net name.
A board the router cannot import still previews, without the net list, and DSN
boards have no net list.

Rerouting a subset of an already routed board is harder than routing the whole
board: the copper of every unselected net is fixed, so the router must fit the
selected nets around it. Expect more remaining connections than a full route of
the same board.

## Via-in-pad rules

**Allow vias in SMD pads** is off by default for KiCad boards and resets when
another board is selected. It supplies the Rust KiCad JSON bridge's explicit
`viaInPadAllowed` permission (absent means false), independently of the project's
width and clearance rules. This is a router option, not an imported KiCad project
property.

Rust stores this permission in each `ViaInfo.attach_smd_allowed`, for the default
via and every net-class via template. Maze search, fanout and forced via insertion
use those rules. Disallowing attachment keeps the new via's copper clear of SMD
pad copper, including off-centre overlaps. `smd_via_relaxation` may reduce via costs
on pure-SMD nets; it cannot override attachment permission for KiCad JSON inputs.

DSN routing continues to use its embedded `(control (via_at_smd on/off))` and
per-via attachment rules, including the legacy pure-SMD search relaxation. The
browser checkbox is disabled for DSN. See the [native permission guide](../docs/fixes/via-in-pad-permissions.md)
for JSON class overrides and format-conversion behavior. Native KiCad imports
preserve existing vias in place; this setting governs newly routed vias.
The browser removes existing vias before rerouting. Disallowing via-in-pad may
require more routing space and can leave connections unrouted.

## Scope and limits

This proves browser routing, rather than complete native KiCad format support.
For KiCad boards, the browser bridge imports KiCad **JSON**, so `kicad.js` translates a supported
subset of the native S-expression format into that existing representation.

- Standard circular, rectangular, oval, and rounded rectangular pads; rotated
  footprints and pads on either side; straight tracks; through vias; up to 32
  copper layers; one outer board outline with internal cutouts, made from lines, arcs, circles, a rectangle,
  or a polygon, including Edge.Cuts inside footprints. Curves are approximated
  within 0.005 mm; legacy center/angle arcs and modern three-point arcs are supported.
  The original outline remains intact in downloads. Cutouts become fixed obstacles
  on every copper layer, with the imported copper-edge clearance.
- Copper layers retain stack order and canonical names independently of KiCad
  numeric IDs; the original layer table (including IDs and aliases) is preserved
  in the PCB download. Signal/mixed copper layers allow routing; power layers
  become non-routing planes in Rust. Unsupported layer types are rejected.
  Export rejects changed layer mappings and traces on non-routing layers.
  The preview legend lists every copper layer, including aliases and plane status.
- Rounded pads preserve corner radii for physical hole-clearance DRC, while routing
  and other DRC checks still use enclosing rectangles. The core
  approximates oval pads with its existing polygon representation.
- Plated oval slots retain their original geometry in downloads. Routing uses
  the copper pad shape and the smaller drill dimension; slot-specific DRC needs
  KiCad. Connector pads are supported.
- Both legacy numeric net references and KiCad 10 named net references are
  supported. Paste-only pad apertures are excluded from copper routing.
- Copper zones are accepted when **Refill copper zones in KiCad after routing**
  is selected (the visible default). Routing operates on tracks and pads without
  relying on pours. The download preserves zone definitions and removes cached
  `filled_polygon`/`fill_segments` data. Open it in KiCad, press **B** to refill,
  then run DRC. This is not an in-browser zone fill implementation. Keepout areas that
  explicitly allow both tracks and vias are preserved, including pour-only
  keepouts. Areas restricting tracks or vias, and netless copper zones, remain
  rejected. Deselecting the option rejects copper zones but still allows these
  non-routing keepout areas.
- Offset pad copper is translated in its local coordinate system while the hole
  stays at the pad anchor. Convex custom pads with one filled polygon and a covered
  circular anchor include their stroke, approximated within 0.005 mm. Other custom
  pad constructions remain rejected.
- Copper text is reserved using conservative rectangles. Footprint copper
  rectangles include their stroke and reserve their entire interior. Original
  graphics remain in the download; the reserved text bounds can reduce routability.
- Keepouts restricting tracks or vias, separate outer boards, other footprint
  copper graphics, net ties, and non-plated slots are explicitly rejected. Existing routing is
  discarded before import, including locked tracks, arcs and all via types. This excludes many production boards.
- An optional `.kicad_pro` supplies net-class widths, clearances, via diameters,
  drills and class assignments to the existing KiCad JSON loader. The WASM entry
  point also calls the same `fr_drc::apply_kicad_project` API as the native CLI
  for supported board minima, DRC constraints and severities. Project rules take
  precedence over the disabled manual fields. Simple `*`/`?` net-name patterns
  and explicit assignments are supported; composite classes and richer patterns
  are rejected. `.kicad_dru` custom rules and local pad overrides are not applied.
  Legacy embedded `net_class` records supply widths, clearances, via dimensions
  and exact net assignments without a project. An accompanying project takes
  precedence. Manual rules apply when neither source is available. Project files are
  limited to 5 MiB. Browsers cannot read adjacent project files automatically.
- Original non-routing source sections are preserved byte for byte, including
  footprints and board metadata, except cached zone fills when rebuilding zones and the modification notice
  added to bundled example downloads. Top-level tracks and vias are replaced with
  the pipeline output, so their individual UUIDs/properties are not retained.
- KiCad pad and via drills, including net-class via templates, are stored explicitly
  in Rust padstacks in board units. DRC and hole-clearance search geometry use these
  dimensions, and the writer exports them directly. Non-plated holes are marked
  separately and are exempt from annular-ring checks. Pads with different drill
  diameters or plating do not share a padstack. Slotted-hole drill geometry remains
  approximate; legacy inputs without drill metadata retain their fallback behavior.
- `preview.js` displays native outlines, pads, tracks, vias, filled copper zones,
  and reference labels, including modern arc geometry. Previewing does not need
  WASM or a successful routing import, and routing errors retain the preview.
  Some unsupported custom pad shapes are shown by their bounding dimensions.
  It does not render complete silkscreen, component bodies or airwires.
- Routing is single threaded; optimization is disabled. A time limit is
  cooperative within the router, and final DRC/export can take additional time.
  Cancel terminates the worker immediately and discards that attempt. A new
  attempt starts with a fresh WASM instance, including after a panic.
- The UI reports incomplete connections, timeouts, and the Rust DRC count.
  A completed run can still have unrouted connections or violations. Always
  inspect in KiCad and run its DRC with the original project rules.
- Files are capped at 20 MiB. Large accepted boards may still exceed the browser's
  practical memory or runtime limits. Tested in Chromium; other browsers have
  not yet been verified.

## Implementation

`app.js` manages file selection, worker lifecycle, result state, and Blob downloads.
`worker.js` renders native geometry on selection. On Route it imports the board,
applies project classes, loads WASM, routes, then sends the final preview and PCB. All parsing and routing happens in the worker.
`crates/copper-web` resolves normal headless settings, loads the existing JSON format,
invokes `fr_core::RoutingPipeline`, and returns the JSON output plus route metrics.
Routing `std::time::Instant` uses are replaced with `web_time`, a native alias on
native targets and a browser clock on WASM. `ProgressSink::on_board_update` lends
observers the board at existing autorouter checkpoints (250 ms throttle, plus
pass boundaries); its default forwards the unchanged event to existing sinks.
`RoutingPipeline::run_with_progress` allows the browser to use a local callback
without the native sink's `Send` requirement. Snapshots are serialized and rendered
in the worker, then posted while synchronous WASM routing continues. This shows
committed board geometry, not individual maze-search steps. Long searches can
leave a frame unchanged until the next checkpoint. Cancel/replacement terminates
the worker and ignores stale messages. Final output remains the pipeline result.

Relevant format/runtime references:
[KiCad board format](https://dev-docs.kicad.org/en/file-formats/sexpr-pcb/),
[KiCad common S-expressions](https://dev-docs.kicad.org/en/file-formats/sexpr-intro/),
[web-time](https://docs.rs/web-time/1.1.0/web_time/).

Next steps for broader board support: a full native-format importer with exact curved
geometry, in-browser copper zone fills/keepouts and custom rules; geometry oracle tests against
KiCad for real corpus boards; fanout/maze-search visualization; and richer layer/airwire viewing.

## Validation

```sh
# From web/ (build first):
npm test
npx playwright install chromium
npx playwright test

# From repository root:
cargo check --workspace
cargo test -p fr-core --lib
cargo test -p fr-board --lib time_limit
cargo test -p fr-router --lib pipeline::stop
```

Browser tests cover actual drag/drop → WASM route → SVG → PCB/SVG downloads,
read-back of downloaded routing, no uploads/external requests, mobile layout,
invalid input, cancellation and retry. Adapter tests cover placement, source
preservation, drill preservation, rejected geometry, and SVG metadata isolation.

The included board routed both connections with zero Rust DRC violations.
KiCad 10.0.3 independently loaded the browser download and reported zero
unconnected items and no routing errors. Its two warnings were missing local
`Browser` footprint-library definitions for the example's embedded footprints.

Lantern import regression (2026-09-06, before the route-from-scratch change): the user-supplied KiCad 10 PCB and project load
53 nets, 210 copper pads, 413 existing segments, and 128 vias. A one-pass browser
run produced a downloadable result with 10 track-only incomplete connections and
43 Rust DRC findings. KiCad 10.0.3 loaded that download with the original project,
refilled its two ground pours, and reported **zero unconnected items**; remaining
findings were two footprint-library warnings and one dangling-via warning.
The private board is not copied into this repository.

Live preview validation: the v2 Lantern PCB/project streamed six frames during
one routing pass, with remaining connections decreasing from 63 to 11. Browser
tests verify intermediate copper frames arrive before completion and dropping a
replacement onto an existing SVG cancels the old worker. The preview also opens
the file picker with click, Enter, or Space.

## Bundled examples

The picker offers six boards spanning 9 to 225 nets: Hubble jacks, CATs-Eurosynth
Slimline VCA, Easyduino Uno and Nano, Adafruit Feather ICE40, and real-time
chess. Each is fetched only when selected, so opening the page costs one board.
Every board keeps its upstream `LICENSE.txt` and notice beside it, and the
sidebar credit, the routed download's title block and the SVG description all
name that board's own project, author, commit and licence. Boards other than
the Easyduino pair ship without a KiCad project file, so the manual rules apply
to them.

## Bundled Easyduino examples

Uno and Nano are Arduino-compatible designs by Hanqaqa and contributors, from
[Easyduino](https://github.com/Hanqaqa/Easyduino/tree/53b14b66d64f25c55f88971c1c1b51656126bd30).
The unmodified PCB and project pairs are in `examples/easyduino/`, with the
upstream CERN-OHL-P-2.0 license, README, and a provenance notice. These are not
endorsed by Arduino. Keep these notices and the license when redistributing the
examples. Routed example PCBs include a dated modification notice in their title
block; SVG downloads include attribution in a description element.

Both are four-layer boards with one Default net class (no distinct per-net classes):

| Example | Track width | Clearance | Via diameter / drill |
| --- | --- | --- | --- |
| Uno | 0.20 mm | 0.15 mm | 0.50 / 0.30 mm |
| Nano | 0.25 mm | 0.128 mm | 0.50 / 0.30 mm |

The corresponding project is loaded automatically when selecting an example.
A user file selection takes precedence over pending example downloads.

Short one-pass browser runs produced PCB and SVG downloads for both examples.
After refilling zones, KiCad 10 loaded the results and reported 13 unconnected
items for Uno and 19 for Nano, plus 9 and 23 other findings respectively. These
are useful realistic routing demonstrations, not finished production layouts.

Layer regression: the bundled Uno declares F.Cu, In1.Cu, In2.Cu and B.Cu as
signal layers. The Uno example restricts the router to F.Cu and B.Cu via Rust
layer activation settings, while preserving all four layers and their original
types in downloads. Nano and uploaded boards retain their normal routing-layer
behavior. Inner copper now has distinct colors and legend
labels. Browser tests verify that power-plane inner layers receive no traces.

Uno DRC investigation before the drill fix (five passes, outer copper only): all 34 Rust findings were
estimated `drill_out_of_range` results: 32 vias estimated as 0.225 mm and two USB
NPTHs estimated as 0.2925 mm against a 0.30 mm minimum. Actual exported drills
were 0.30 mm and 0.65 mm respectively. The core padstack fallback estimates drill
diameter as 45% of copper diameter when its naming convention does not encode a
drill. At that point, the browser restored via drills only during export, after Rust DRC.
KiCad independently found no undersized holes, but did find one genuine
hole-to-copper clearance error (0.1753 mm versus 0.25 mm), one dangling-via
warning, and three unconnected items. The Rust count is not a manufacturing
sign-off. WASM now includes per-finding `drcDetails` and pre-route
`initialDrcDetails`, with estimate flags and dimensions in mm, for diagnosis.

After the explicit-drill fix, the same Uno run has zero drill-size violations.
Rust reports nine hole-clearance findings: eight already present in its imported
USB footprint approximation and one involving a routed track. KiCad still needs
to validate the complete native geometry, particularly slots and rounded pads.
Regression tests check the unmodified WASM result's via drills, so export-time
repair cannot hide a loss of metadata inside Rust.

## Specctra DSN import

Drop a `.dsn` into the preview window or select it with **Choose a board**.
The native Rust DSN importer supplies embedded net classes, widths, clearances,
via definitions, planes, keepouts, and layer settings to the router. KiCad project
and manual rule controls are disabled for DSN. The preview displays physical board
geometry and updates at routing checkpoints. Loading its preview requires WASM.

**Route board** removes existing traces and vias, including protected routing,
before routing from scratch. Downloads include a routed DSN, an SES session for
import into the original PCB editor, and an SVG. The DSN is reserialized by the
native writer; it is not a byte-preserving copy of the source. Artwork and
source-editor metadata are outside this interchange format. Run the source
editor's DRC after importing the SES. Partially imported DSNs can be previewed
but are rejected for routing.


USB hole-clearance follow-up: see [the investigation](docs/usb-hole-clearance-investigation.md).
The bridge now preserves rounded-pad corner radii and hole-to-copper DRC uses
physical circle, segment, rectangle and rounded-rectangle distances. Routing
retains enclosing rectangles for rounded pads. Result details distinguish
findings already present before routing from new findings; the original project
minimum is not reduced to hide existing footprint violations.


To check a local PCBench download through import, preview, WASM routing and both
PCB/SVG downloads, start the web server and run:

```sh
node web/scripts/check-pcbench.mjs /path/to/PCBench-top-5-new-boards /tmp/pcbench-results
```

The check imports both raw and processed boards and routes each processed board
with its adjacent `raw.kicad_pro` when available. It uses five passes and a
60-second routing budget per board, writes downloaded files and a JSON report,
and verifies unchanged footprints, layer tables and source rules. Passing this
check means the browser produces a usable partial result; the report separately
records remaining connections and DRC findings. Refill pours and run KiCad DRC
before treating an output as complete.
