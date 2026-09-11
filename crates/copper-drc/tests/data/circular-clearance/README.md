# Circular copper clearance controls

Extracted U6 pad4 and one via from the saved routed Sizif XXS board, preserving original pad geometry and project settings. The two fixtures differ only in the via x position:185.0336mm (clear) and184.9336mm (violating). KiCad10.0.4 reports zero and one copper-clearance errors respectively. Analytic rounded-rectangle/circle gaps are0.20560595mm and0.15860413mm against a0.2mm rule. The violating fixture also has a solder-mask finding.

Native JSON was generated with the browser adapter from the effective-mask experiment. Its additional effective-mask metadata is ignored by the main-based reader used for this separate copper-clearance experiment. Tests compare copper-clearance behavior. Original full diagnostic reports and extraction/import scripts are archived on workbench at /home/em/copperroute-mask-results/provenance/rounded-clearance-audit.
