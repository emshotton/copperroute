"""Add separate local copper floors to frozen mask metadata using KiCad's parser."""
import argparse
import json
from pathlib import Path
import pcbnew
from reference_mapping import remap_board_references

parser = argparse.ArgumentParser()
parser.add_argument('--corpus', type=Path, required=True)
parser.add_argument('--mask-metadata', type=Path, required=True)
parser.add_argument('--out', type=Path, required=True)
args = parser.parse_args()
args.out.mkdir(parents=True, exist_ok=False)
manifest = json.loads((args.corpus / 'manifest.json').read_text())
boards = {b['id']: b for b in manifest['boards']}
summary = []
for source in sorted(args.mask_metadata.glob('*.json')):
    board_id = source.stem
    entry = boards[board_id]
    assert entry['referee'] == 'kicad' and entry['status'] == 'ok'
    board = pcbnew.LoadBoard(str(args.corpus / entry['kicad']['raw']))
    records = json.loads(source.read_text())
    changes, unmatched = [], []
    for fp in board.GetFootprints():
        for pad in fp.Pads():
            value = pad.GetLocalClearance()
            inherited = value is None
            if inherited:
                value = fp.GetLocalClearance()
            if value is None:
                continue
            if not any(pad.IsOnLayer(layer) for layer in board.GetEnabledLayers().CuStack()):
                continue
            at = pad.GetPosition()
            matches = [r for r in records if r['component'] == fp.GetReference()
                       and r['pad'] == pad.GetNumber()
                       and abs(r['x_um'] - at.x / 1000) < 0.01
                       and abs(r['y_um'] - at.y / 1000) < 0.01]
            detail = {'component': fp.GetReference(), 'pad': pad.GetNumber(),
                      'copper_um': max(0, value) / 1000, 'source_um': value / 1000, 'inherited': inherited}
            if not matches:
                unmatched.append(dict(detail, matches=len(matches)))
                continue
            for record in matches:
                record['copper_um'] = max(record.get('copper_um', 0), value / 1000)
                drill = pad.GetDrillSize()
                size = pad.GetSize()
                if (pad.GetAttribute() == pcbnew.PAD_ATTRIB_NPTH
                        and drill.x > 0 and drill.x == drill.y
                        and max(size.x, size.y) <= drill.x):
                    record['hole_diameter_um'] = drill.x / 1000
            detail['coincident_records'] = len(matches)
            changes.append(detail)
    stripped = pcbnew.LoadBoard(str(args.corpus / entry['kicad']['stripped']))
    records, reference_changes, unresolved_references, rejected_geometry = remap_board_references(
        records, board, stripped)
    (args.out / source.name).write_text(json.dumps(records, indent=2))
    summary.append({'board': board_id, 'changed_pads': changes, 'unmatched': unmatched,
                    'reference_changes': reference_changes,
                    'unresolved_references': unresolved_references,
                    'rejected_geometry': rejected_geometry})
    print(board_id, len(changes), 'unmatched', len(unmatched), flush=True)
(args.out / 'census.json').write_text(json.dumps(summary, indent=2))
assert not any(r['unmatched'] for r in summary), 'Resolve unmatched copper pads before benchmarking'
