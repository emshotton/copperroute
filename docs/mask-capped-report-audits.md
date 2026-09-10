# Additional findings on boards with capped KiCad reports

These diagnostics validate specific added mask findings. They do not replace the full routing evaluation or imply an uncapped full-board comparison.

## Dropbot

The NPTH correction changes the native full-board mask count 331→332; the archived KiCad report stops at 199. The added pair is SW1's unnetted NPTH pad and rear pad 3. Retained those two pads from the exact saved source, with their footprint and original project; removed other footprints, routing, zones and other SW1 pads from a diagnostic copy.

KiCad 10.0.4 reports one rear mask bridge between those same items, matching the cumulative candidate. The source NPTH pad has 0.254 mm effective expansion despite having no remaining copper. The other pad's expansion is also 0.254 mm. The required gap is 0.508 mm and the copper-shape gap 0.150 mm. The specific new finding is therefore confirmed independently. Other unmodified findings are retained in the diagnostic report.

## Mechaduino DR

The layer correction adds eight mask findings while the full-board KiCad report is capped at 199. Two overlapping HS pads have identical size, position, net and logical pad number but opposite copper sides; both declare front and back mask openings. The added findings are two pad/track comparisons on the opposite mask side and six pad/pad comparisons with U2. This includes KiCad's multiplicity for the two distinct HS pad objects, rather than eight independent new physical defects.

Retained the two HS copper pads and the six affected U2 pads, preserving exact source text, the original project, outline and all tracks/vias. Removed other footprints, other pads and zones. The extractor initially asserted the wrong retained pad count because HS also contains six additional pad objects; it stopped before writing a board. The corrected extractor explicitly selects the two referenced HS copper pads and verifies exactly eight retained pads.

| Checker on the diagnostic subset | Mask findings |
|---|---:|
| Main 7ecbeac2 | 13 |
| Cumulative mask candidate | 21 |
| KiCad 10.0.4 | 21 |

The subset is below the mask-report cap. KiCad also retains two padstack warnings; dangling tracks caused by removing other pads are expected in this diagnostic copy and are not a routing-quality result. The existing difference in dangling-track counts is unchanged between main and candidate. No production source or frozen benchmark binary changed during either audit.

Full source copies, native inputs, unmodified reports and extraction/audit scripts are stored on workbench under `copperroute-mask-results/provenance/{dropbot-mask-audit,mechaduino-mask-audit}/`.
