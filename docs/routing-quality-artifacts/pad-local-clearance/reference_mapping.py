"""Map raw-board metadata names to the footprints used by the stripped board."""
from collections import defaultdict
from copy import deepcopy

def remap_references(records, source_pads, destination_references):
    index = defaultdict(list)
    for pad in source_pads:
        index[pad['component'], pad['pad']].append(pad)
    mapped = deepcopy(records)
    changes, unmatched = ([], [])
    for number, record in enumerate(mapped):
        matches = [pad for pad in index[record['component'], record['pad']] if abs(pad['x_um'] - record['x_um']) < 0.01 and abs(pad['y_um'] - record['y_um']) < 0.01]
        if not matches:
            unmatched.append(number)
            continue
        if any((pad['uuid'] not in destination_references for pad in matches)):
            raise ValueError(f'missing destination UUID for metadata record {number}')
        references = {destination_references[pad['uuid']] for pad in matches}
        if len(references) != 1:
            raise ValueError(f'ambiguous destination for metadata record {number}')
        reference = references.pop()
        if reference != record['component']:
            changes.append(dict(record=number, before=record['component'], after=reference))
            record['component'] = reference
    return (mapped, changes, unmatched)

def remap_board_references(records, raw, stripped):
    dest = defaultdict(list)
    for fp in stripped.GetFootprints():
        dest[fp.m_Uuid.AsString()].append(fp)
    source_ids = defaultdict(list)
    for fp in raw.GetFootprints():
        source_ids[fp.m_Uuid.AsString()].append(fp)
    pads = []
    refs = {}
    rejected = []
    for uid, fps in source_ids.items():
        if len(fps) != 1 or len(dest[uid]) != 1:
            rejected.append({'uuid': uid, 'reason': 'missing or duplicate footprint UUID'})
            continue
        fp = fps[0]
        df = dest[uid][0]
        if fp.GetPosition() != df.GetPosition() or fp.GetOrientationDegrees() != df.GetOrientationDegrees() or fp.GetLayer() != df.GetLayer():
            rejected.append({'uuid': uid, 'reason': 'footprint position, orientation or layer differs'})
            continue
        refs[uid] = df.GetReference()
        for pad in fp.Pads():
            pos = pad.GetPosition()
            dp = [p for p in df.Pads() if p.GetNumber() == pad.GetNumber() and p.GetPosition() == pos and (p.GetSize() == pad.GetSize()) and (p.GetShape() == pad.GetShape()) and (list(p.GetLayerSet().Seq()) == list(pad.GetLayerSet().Seq()))]
            if not dp:
                rejected.append({'uuid': uid, 'pad': pad.GetNumber(), 'reason': 'destination pad geometry differs'})
                continue
            pads.append(dict(component=fp.GetReference(), pad=pad.GetNumber(), x_um=pos.x / 1000, y_um=pos.y / 1000, uuid=uid))
    changes = []
    unresolved = []
    mapped = []
    for n, record in enumerate(records):
        try:
            new, changed, missing = remap_references([record], pads, refs)
        except ValueError as e:
            mapped.append(record)
            unresolved.append({'record': n, 'reason': str(e)})
            continue
        mapped.extend(new)
        if changed:
            changes.append(dict(changed[0], record=n, copper_um=record.get('copper_um')))
        if missing:
            unresolved.append({'record': n, 'reason': 'no verified source and destination pad match'})
    return (mapped, changes, unresolved, rejected)
