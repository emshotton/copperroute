# Rotated-pad mask precision audit

Diagnostic only. No production code or evaluated candidate changed.

The ten candidate-only pad-pair findings on Sizif XXS involve rotated rectangular pads at the mask-clearance boundary. One pair on RN3 has 0.8×0.4 mm pads at 135 degrees, with 0.2 mm mask expansion on each. CopperRoute reports 0.3998 mm separation against a 0.4000 mm requirement; KiCad does not report that pair.

Extracted RN3 pads 2 and 3 from the saved KiCad board, preserving their exact source text and the original project. Removed other footprints, routing and zones from a diagnostic copy, retaining board setup and outline. KiCad 10.0.4 reports no mask bridge on the isolated pair. The report's unrelated finding is retained.

Reimported this same isolated board using the evaluated native adapter, then changed only the native JSON resolution for a data-only checker probe:

| Units per mm | Grid spacing | CopperRoute mask findings |
|---:|---:|---:|
| 10,000 (native default) | 0.1 µm | 1 |
| 100,000 | 0.01 µm | 0 |
| 1,000,000 | 0.001 µm | 0 |

The reader rounds placement coordinates to its integer grid, and `TileShape::rotate_approx` / `PolygonShape::rotate_approx` round rotated corners too. The resolution counterfactual is sufficient to remove the false finding on this reproduced pair. It does not prove that raising the routing grid globally is safe: larger coordinates can overflow existing integer geometry, and routing behavior and runtime would change. A future fix should preserve accurate geometry for DRC and include near-boundary positive controls; simply suppressing submicron errors would also hide genuine bridges. No global resolution or epsilon change was accepted.

All inputs, native JSON variants, unmodified KiCad and CopperRoute reports, exact extraction script and the ten-pair audit are on workbench under `copperroute-mask-results/provenance/rotated-mask-audit/`. The evaluated mask-effective-v1 binary was used throughout.


Whole-board precision counterfactual (2026-09-10): on the exact saved Sizif XXS native input, changing only resolution from10000 to100000units/mm changes mask175→165. Spatial pair matching confirms exactly the ten previously unmatched pairs disappear, no pair is added, and all165 remaining pairs match KiCad. However clearance errors change70→71 while the saved KiCad report has zero clearance errors (and23 courtyard errors). This directly rejects a global-resolution change as an established accuracy fix. The added native finding is U6 pad4 /VA9 against via /A9 at(185.1336,79.0631), reported gap0.1994mm versus expected0.1999mm. U6's pad is a2.05×0.6mm roundrect, radius0.15mm, at(184.56,80.45), rotated−90degrees; the via diameter is0.5mm. Copper checking uses tile-shape expansion/intersection, so rounded-pad/circular-via approximation is a concrete follow-up lead, not yet an isolated proven cause. No production geometry or candidate binary changed. Scripts, inputs, full reports, pair comparison and the added-clearance record are archived on workbench under provenance/rotated-mask-audit/full-board.
