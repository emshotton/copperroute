#!/usr/bin/env python3
"""Reduce a KiCad DRC report to what is reproducible across JVMs (plan-5 ruling 3).

This is the Python twin of `parity::normalize_drc_json` (tests/parity/src/lib.rs). It is used
by `scripts/gen-drc-reference.sh --verify-hash-modes`, which regenerates every stem once per
`-XX:hashCode=0..4` and requires the normalised documents to be byte-identical.

It is deliberately **not** on the path of any committed byte: `tests/reference/<stem>/drc.json`
is the jar's verbatim output, and the Rust normaliser is applied to both sides inside the parity
test. So the two implementations never have to agree on number formatting — only on the three
rules below, and only well enough for the sweep to compare five runs of the *same* JVM.

The three rules, and why there are only three:

1. `date` is dropped. Java fills it from `ZonedDateTime.now()` (io/kicad/KiCadDrcReport.java:70);
   the port takes it as an injected string (plan-5 ruling 5), so there is nothing to compare.
2. Every `unconnectedItems[]` entry's `items` array is sorted by **numeric** uuid. On the JVM
   that array is `connectedSets.get(0)` followed by `connectedSets.get(1)`
   (drc/DesignRulesChecker.java:143-145), two `HashSet<Item>`s over a class with no `hashCode`
   override — identity-hash order, i.e. nothing to port (quirk #144). The port emits ascending
   item id (ruling 3).
3. `violations` is left **completely** alone — neither the array nor any entry's `items`.
   The array order is hash-independent (ruling 3's probe) and is this plan's bit-parity surface;
   a clearance entry's `items` is `[firstItem, secondItem]` in `ClearanceViolation`'s own
   deterministic order (DesignRulesChecker.java:319-324), so sorting it would hide a real
   ordering regression. A dangling entry has one item, where sorting is a no-op anyway.

Nothing else is touched: not the `unconnectedItems` entry order (ascending net number on both
sides), not `schematicParity`, not `qualityScore`.

Usage: normalize-drc.py <report.json>   # normalised document on stdout
"""

import json
import sys


def normalize(report):
    """Apply the three rules to a parsed report, in place, and return it."""
    report.pop("date", None)
    for entry in report.get("unconnectedItems") or []:
        items = entry.get("items")
        if isinstance(items, list):
            # Numeric, not lexicographic: the uuids are `String.valueOf(item.getId())`
            # (DesignRulesChecker.java:320-321), so "1000" sorts before "99".
            items.sort(key=lambda item: int(item["uuid"]))
    return report


def main(argv):
    if len(argv) != 2:
        sys.stderr.write("usage: normalize-drc.py <report.json>\n")
        return 2
    with open(argv[1], encoding="utf-8") as handle:
        report = json.load(handle)
    json.dump(normalize(report), sys.stdout, indent=2, ensure_ascii=False)
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
