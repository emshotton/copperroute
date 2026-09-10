# Outlier boards

This register records unusual design features and benchmark transformations that complicate interpretation of routing results. Each entry needs evidence from the original board and an explanation of what the router actually receives. A routing failure or timeout alone does not establish an outlier.

All listed boards remain in the headline benchmark. This register does not exclude boards or waive KiCad violations. Any future filtered comparison must also show the full-corpus result and identify its exclusions.

Review policy: strongly consider changes with broad improvements whose regressions are concentrated on evidenced outliers. A handful of outlier regressions should not automatically prevent landing. PRs must name those boards, quantify their contribution, and separately disclose regressions on ordinary boards.

Updated: 2026-09-09.

| Board ID | Unusual feature | Why routing or scoring is affected | Classification and confidence |
|---|---|---|---|
| `pcbench-8bit-cpu_programming_interface` | The original contains 223 unnetted F.Cu tracks beneath 223 F.Mask line objects, forming exposed copper artwork. | Stripping removes all 223 copper tracks but retains the mask openings. The stripped board has no copper contacts in those openings; after routing and zone refill, 30 openings touch multiple nets in a direct geometry audit. The combined candidate reports 34 KiCad mask violations versus zero on main. This creates a materially different copper/mask situation from the original. | **Confirmed input-fidelity outlier.** Artwork removal is verified. A controlled preservation test reduces the candidate from 7 U/32 mask/0 copper violations to 3 U/0 mask/0 copper when its original copper is also supplied as router obstacles. The original is a real design, with zero mask violations and zero unconnected items in a fresh KiCad check. |
| `pcbench-avr-fuser-32_adapter` | Copper-layer text is part of the physical board: 13 text objects, including labels such as `v18`, `TOP`, and `T84`. | The exported DSN omits this copper geometry. All 28 copper violations in the combined candidate's targeted run involve copper text. A diagnostic DSN containing the actual text outlines eliminates those violations but leaves 26 unrouted connections; it exposes a harder physical problem. | **Confirmed input-fidelity outlier.** Omitted obstacles are proven. The original uses 145 vias and is connected; its via sizes match the DSN, so a via-size mismatch was tested and ruled out. |
| `pcbench-FRM16_Relay_Module_I2C_Controller_relay_controller` | Text and polygon artwork is placed directly on B.Mask, including `AndrewSowa.com` and `Made In Chicago`. | The combined candidate reports 41 mask violations versus five on main. An earlier detailed candidate audit attributes all 41 to mask artwork. Pad-based mask clearance cannot represent these independent apertures. The original has zero errors and zero unconnected items in a fresh KiCad check. | **Confirmed unusual-feature case; representativeness unresolved.** This is valid artwork that the current routing model does not cover. The original has no unnetted tracks, so the programming-interface stripping mechanism does not explain it. |
| `pcbench-Brushless_ESC_Brushless_ESC` | Board text `D. Walter / ESC v2.1` is physical copper on B.Cu. | The pad-local candidate produces one KiCad `shorting_items` violation between that text and a Q2-Pad3 track. DSN contains zero keepouts and zero wires; all 23 polygon nodes are library footprint outlines, with no representation of the board copper text. | **Confirmed input-fidelity outlier.** The original is connected with zero routing DRC (1061.5428mm, 67vias); its saved DRC has no text-related error. This missing obstacle explains the specific added short, not other boards' regressions. The board remains in every headline result. |

| `pcbench-oled-bmp280-touch_oled-bmp280-touch` | The original has 15 malformed-outline errors, including disconnected Edge.Cuts segments/arcs. | Exported DSN contains one rectangular boundary and zero keepouts. The pad-local candidate adds two copper-edge violations against an original circular Edge.Cuts cutout absent from that DSN. | **Confirmed original-outline/input-fidelity outlier.** Original routing is connected with zero routing DRC, 419mm tracks and 21 vias, but its outline is invalid. The specific cutout collisions remain counted; repairing the outline and re-exporting has not yet been measured. |
| `pcbench-VC4000MultiROM_MultiRomCard` | Back-copper text reads “VC4000 MultiROM v0.4 / Keller/Maibaum”. | KiCad identifies this text in all reported shorts for main and corridor+guard. Its DSN contains no keepout or wire geometry and no text string; all 13 polygons are footprint outlines. Original is fully connected with zero routing DRC. Combined candidate improves U 4→2 but copper violations rise 11→13 (shorts 7→9). | **Confirmed input-fidelity outlier: omitted copper text.** All violations and the −2U/+2 copper trade remain in headline results; no preservation reroute has yet been measured for this board. |

## Evidence

- [Routing-quality experiment log](routing-quality-log.md): original-board checks, controlled experiments, referee caveats, and negative hypotheses.
- Programming interface: the corrected occupancy audit excludes rule areas and uses filled copper polygons for zones. Original/stripped/combined aperture counts are respectively 223/0/178 single-net, 0/223/15 empty, and 0/0/30 multi-net. Zero-clearance contact counts are diagnostics, not a replacement for KiCad DRC. Script: `/tmp/quality-artwork-fixed-copper-audit.py`; server report: `reports/artwork-fixed-copper-study/occupancy.json`, covered by the workbench results backup.
- AVR Fuser: `/tmp/quality-avr-fuser/avr-text-pilot-results.json` and `text-shapes.json`. Matched 300-second experiment: original DSN 0 unrouted / 28 copper violations, timed out; text-aware DSN 26 unrouted / 0 copper violations, completed. This is an input experiment, not a demonstrated routing improvement.
- Fresh original artwork checks: `/tmp/quality-kicad-mask-pilot/new-mask-regressions/original-fresh-summary.json` and the per-board `raw-drc-fresh.json` reports.

## Maintenance

For each new entry, record the exact board ID, original feature, input transformation if any, reproducible evidence, remaining uncertainty, and benchmark treatment. Promote a suspected cause to confirmed only after inspecting geometry or testing it. Remove or reclassify an entry when its explanation is disproved or the input/model issue is fixed.

Ordinary dense layouts, buses, fine-pitch pads, and high via counts do not establish nonrepresentative problems. The Apple M0110 and GB-CART256K-A connectivity regressions remain routing defects under investigation; neither currently has evidence warranting an outlier designation.

## Controlled programming-interface preservation test

KiCad 10.0.4, same frozen combined router and pad metadata, ten passes and 300-second limit. No corpus files were modified.

| Input/referee | Unrouted | Copper violations | Mask violations |
|---|---:|---:|---:|
| Original stripped input and referee | 7 | 0 | 32 |
| Same SES, original 223 unnetted tracks restored only in referee | 7 | 26 | 0 |
| Original223 copper tracks represented as router obstacles and restored in referee | 3 | 0 | 0 |

The first two cases have identical SES hashes. Restoring only the missing copper converts hidden conflicts into copper clearance violations; supplying that geometry before routing resolves them in this experiment. Obstacles are the actual copper outlines, approximated outward by 1µm, not blanket mask keepouts. KiCad's DSN exporter omits these tracks even when they are retained in the PCB, so preservation must also reach the router input. The relay-controller case has a different mechanism and is not explained by this test. Raw results and reproducible scripts are in [mask-routing artifacts](routing-quality-artifacts/mask-routing/artwork-preservation-pilot-results.json). This strengthens the outlier diagnosis but does not replace the unchanged full-corpus headline.

### Brushless ESC evidence

[Reproducible audit](routing-quality-artifacts/brushless-copper-text/audit.py) and [recorded result](routing-quality-artifacts/brushless-copper-text/audit.json).

Run `quality-epyc-pad-copper-full-01`, candidate `pad-copper`: KiCad identifies original copper-text UUID `512c8116-5fc4-4e65-b568-c663158d1edd` at (174.8155,81.026)mm and an8.6418mm B.Cu track on `Net-(Q2-Pad3)` as the short. Control has no short. The DSN node audit finds23 polygons, all under `library/image/outline`, no keepouts and no wires; no copper-text geometry is supplied. Corpus original `ground_truth.json` reports0routing violations and0unconnected. No diagnostic repair or filtered headline has been substituted for the actual result.

- OLED outline: [audit script](routing-quality-artifacts/oled-outline/audit.py) and [saved original/candidate DRC evidence](routing-quality-artifacts/oled-outline/audit.json). Original circle UUID `00000000-0000-0000-0000-0000578ea680`, centre (122.555, 105.156)mm, is involved in both added OLED_RST track collisions. The rectangular DSN boundary does not represent this cutout.

VC4000 evidence: shove-revalidation-copper-gainers.json and shove-revalidation-vc4000-shorts.json in the shove-revalidation report artifacts. DSN polygon contexts were checked: all belong to image outlines.
