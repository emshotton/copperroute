# Native DRC survey coverage

The routing compatibility benchmark covers all751 KiCad boards. The native checker accuracy survey covers549 imported boards and carries forward202 import failures from the initial native survey. Later mask surveys reimported the supported subset; they did not retry these202 failures. Thus “751 attempted” describes the original corpus coverage, not751 fresh native imports for every candidate.

These are import exclusions, not proof that the boards are unsuitable routing benchmarks. Do not add them to the outlier register solely because the native adapter rejects them. Outline closure failures can reflect actual source geometry, conversion tolerances or unsupported outline objects; this tally does not distinguish those causes.

| Initial native import failure | Boards |
|---|---:|
| The Edge.Cuts outline is not closed. | 69 |
| Unsupported board outline object: target | 25 |
| Unsupported trapezoid pads | 21 |
| Footprint copper graphics are not supported yet. | 15 |
| Invalid or excessive board coordinate. | 14 |
| Zone keepouts that restrict tracks or vias are not supported for routing yet. | 10 |
| Footprint zones are not supported yet. | 9 |
| Netless copper zones are not supported for routing yet. | 8 |
| Only plated slots with positive dimensions are supported. | 8 |
| Degenerate Edge.Cuts outline. | 5 |
| Custom pads require one filled convex polygon. | 3 |
| Custom polygon pads require a circular anchor and nonnegative stroke. | 3 |
| Net ties are not supported yet. | 2 |
| Unsupported board outline object: gr_text | 2 |
| Unsupported copper layer type: jumper | 2 |
| Curved tracks are not supported yet. | 1 |
| Degenerate outline arc. | 1 |
| Invalid routing rules. | 1 |
| Unsupported board outline object: dimension | 1 |
| Unsupported board outline object: gr_curve | 1 |
| Unsupported copper object: gr_arc | 1 |

The complete board lists and original failure categories are retained in [native-coverage.json](routing-quality-artifacts/mask-effective/native-coverage.json). Mask-count accuracy comparisons additionally exclude KiCad reports at or above the suspected199-finding cap, leaving529 boards. That exclusion is separate from import support.

The source adapter still contains explicit limitations for footprint copper graphics/zones, track/via keepouts, several pad shapes, slots and net ties. They cannot be fixed by adjusting the DRC checker alone: the checker needs faithful geometry and rule metadata. Current fixes improve represented shapes/rules and do not claim identical behavior on unsupported imports, all KiCad rule types or custom DRC rules.
