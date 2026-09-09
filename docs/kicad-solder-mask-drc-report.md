# Solder-mask checker validation

The proposed checker detects foreign tracks and vias that violate a pad's mask
aperture or the project's mask-to-copper clearance. KiCad remains the external
referee. This change does not include nominal clearance, smoothing, router mask
floors, dogleg escapes, or connection scheduling experiments.

The browser import retains per-pad front/back exposure, board/footprint/pad
expansion precedence, explicit zero and negative expansion, and explicit
footprint permission for solder-mask bridges. Native board import preserves
those values. The check refines corner distances instead of relying solely on
the router's octagonal approximation. Copper-clearance rules remain independent
of mask-bridge permission.

Four included synthetic KiCad 10.0.3 references exercise an illegal mask gap
with legal copper clearance, additional project clearance, a clear off-axis
corner, and an explicitly permitted bridge. Each new behavior had a failing
regression test before implementation. Full validation: **2,570 Rust tests
passed, 77 ignored; 47 web tests passed**.

| Finished session | Internal routing-mask reports | External KiCad evidence |
|---|---:|---|
| LPC2148 nominal/smoothing | 111 | 107 reports match full-board pad/net/layer keys; four more are confirmed on isolated boards |
| LPC2148 mask/dogleg control | 0 | Zero routing-related mask reports; 68 total mask reports include fixed geometry |
| EncoderBoard nominal/smoothing | 0 | Zero routing-related mask reports; 39 total mask reports include fixed geometry |
| Own-Mailbox main, whole board | 353 | All reports match KiCad probes; internal check finds 302 of 305 pad/net/layer keys, missing three unnetted-pad contacts |

Two initial LPC corner false positives, involving a track and a circular via,
were removed by the distance refinement. Report counts do not directly express
precision or recall because Rust joins track segments and KiCad groups apertures.
The Mailbox comparison uses 465 separate probes retaining copper while
exposing one pad aperture per probe. Each stays below KiCad's per-type report
limit; this is local contact evidence rather than a substitute full-board score.

Metadata matching covers all 331 LPC pads and all 461 routed Mailbox pins
(465 original pads, including mechanical geometry). Encoder matches 137/150
original pads and leaves eight synthetic router pins unmatched. The diagnostic
session loader's matching is not a production metadata-import feature.

Coverage is intentionally explicit: unnetted pads, pad-to-pad mask webs and mask
artwork are not implemented in this check; the browser already rejects net-tie footprints.
The three unmatched KiCad keys concern unnetted pads D2.3, IC2.58, and one
physical P3.5 pad. Plain DSN inputs do not carry pad-mask metadata. A clean internal result does
not certify a board as KiCad-clean.

The independent full-corpus run `quality-kicad-drc-01` is complete: all
751 boards have valid KiCad scores. It compares the frozen checker candidate
against main `e9d10c2`, using 10 passes, 300 seconds, one routing thread and
12 jobs on workbench. The measured code matches all 30 implementation/test
source hashes in the local PR worktree.

| Population | Unrouted main → checker | Routing violations | Reported mask violations | CPU seconds main → checker | CPU ratio | Fully connected boards |
|---|---:|---:|---:|---:|---:|---:|
| 740 PCBench, KiCad | 5,321 → 5,069 | 947 → 945 | 13,834 → 13,915 | 37,427.84 → 34,932.14 | 0.9333 | 520 → 521 |
| 11 local, KiCad | 243 → 235 | 59 → 59 | 0 → 0 | 714.45 → 762.77 | 1.0676 | 6 → 6 |
| 751 total | 5,564 → 5,304 | 1,006 → 1,004 | 13,834 → 13,915 | 38,142.29 → 35,694.91 | 0.9358 | 526 → 527 |

PCBench has 14 boards with fewer unrouted connections and four with more;
two gain ordinary routing violations (net change −2). Local fixtures have
one connection improvement and no regression, with unchanged violations.
Total all-category KiCad errors are 17,978 → 18,057. Eighteen boards gain
reported mask errors and eleven lose them. There are no Java-refereed rows.

The standard benchmark gate **fails** on four routing-quality losses.
This result is retained, not replaced by the diagnostic comparisons below.
All 718 pairs where neither run timed out have byte-identical SES routing.
The other 33 pairs involve a timeout on at least one side. Some byte-identical
pairs have different mask totals near KiCad's per-type report cap (for example
badge2016: 203 → 229). These identical-routing pairs account for +62 of the
reported +81 mask delta; the other +19 is among timeout-affected pairs.
The counts on identical routes cannot establish a geometric regression.

The four boards with more unrouted connections are:

| Board | Unrouted main → checker | Routing violations main → checker |
|---|---:|---:|
| karabas-nano_karabas-nano-revC | 92 → 93 | 1 → 1 |
| decelerator4030_decelerator4030 | 481 → 486 | 10 → 11 |
| bms-8s50-ic_bms-8s50-ic | 52 → 53 | 0 → 0 |
| zx-sizif-512-ext_sizif512ext | 44 → 48 | 0 → 0 |

Memory (per-job peak RSS; medians and maxima across the population):

| Population | Median MiB main → checker | Maximum MiB main → checker |
|---|---:|---:|
| PCBench | 15.20 → 15.40 | 453.10 → 453.60 |
| Local | 20.40 → 20.50 | 75.20 → 109.10 |
| All | 15.20 → 15.40 | 453.10 → 453.60 |

Longer 1,200-second controls are complete, runs
`quality-kicad-drc-controls-01` and `-02`. They use the same frozen binaries,
one thread and ten passes; every referee succeeded.

| Board | State on both sides | Unrouted, each | Ordinary violations, each | Mask reports, each | SES files | CPU seconds main → checker |
|---|---|---:|---:|---:|---|---:|
| azalea | Completed, ten passes | 76 | 40 | 159 | Identical | 286.74 → 313.96 |
| bms-8s50-ic | Completed, ten passes | 53 | 0 | 88 | Identical | 542.78 → 544.90 |
| zx-sizif-512-ext | Completed, ten passes | 38 | 0 | 183 | Identical | 749.44 → 794.59 |
| karabas-nano-revC | Completed, ten passes | 40 | 1 | 199 | Identical | 845.90 → 912.89 |
| decelerator4030 | Timed out, six passes | 192 | 14 | 0 | Different partial routes | 1,197.02 → 1,197.17 |

The four completed controls establish identical finished routing for their
boards. Decelerator has equal quality counts at the longer cutoff but
still produces different partial routes; complete-route equivalence on that
board is unproven. The standard 300-second gate remains failed, and its
measurements are not replaced with these diagnostics.

This DSN run tests routing compatibility and cost; native-import oracle tests
and session audits separately validate the new mask behavior. No timing or
routing-quality improvement is claimed for the checker itself. One timing
repetition does not establish a speedup.

The deterministic-work diagnostic `quality-kicad-drc-items-01` is complete.
All five pairs stop after 100 routed items without a timeout, produce
byte-identical SES files, and have identical external KiCad scores:

| Board | Unrouted, each | Ordinary violations, each | Mask reports, each |
|---|---:|---:|---:|
| azalea | 110 | 39 | 159 |
| decelerator4030 | 499 (report cap) | 2 | 0 |
| bms-8s50-ic | 101 | 0 | 88 |
| karabas-nano-revC | 184 | 1 | 199 (report cap) |
| zx-sizif-512-ext | 306 | 0 | 183 |

This establishes matching output for the tested routing prefix on all five
problem boards; it does not claim complete-board convergence for decelerator. All ten
referee checks succeeded.
