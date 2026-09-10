from pathlib import Path
from collections import defaultdict
import json, pcbnew
root=Path('/root/copperroute-bench/benchmark')
manifest=json.loads((root/'corpus/manifest.json').read_text())
entries={b['id']:b for b in manifest['boards']}
summary=[]
for source in sorted((root/'reports/pad-local-copper-metadata-03').glob('*.json')):
 if source.stem not in entries: continue
 entry=entries[source.stem]
 raw_path=root/'corpus'/entry['kicad']['raw']
 stripped_path=raw_path.parent/'stripped.kicad_pcb'
 if not stripped_path.exists():
  summary.append({'board':source.stem,'error':'missing stripped board'});continue
 raw=pcbnew.LoadBoard(str(raw_path)); stripped=pcbnew.LoadBoard(str(stripped_path))
 dest={fp.m_Uuid.AsString():fp.GetReference() for fp in stripped.GetFootprints()}
 changed=[]; missing=[]; index=defaultdict(list)
 for fp in raw.GetFootprints():
  uid=fp.m_Uuid.AsString()
  if uid not in dest:
   missing.append({'reference':fp.GetReference(),'uuid':uid});continue
  for pad in fp.Pads():
   pos=pad.GetPosition()
   index[(fp.GetReference(),pad.GetNumber())].append((pos.x/1000,pos.y/1000,dest[uid],uid))
 for n,r in enumerate(json.loads(source.read_text())):
  matches=[m for m in index[(r['component'],r['pad'])] if abs(m[0]-r['x_um'])<.01 and abs(m[1]-r['y_um'])<.01]
  refs={m[2] for m in matches}
  if refs and refs!={r['component']}:
   changed.append({'record':n,'before':r['component'],'destinations':sorted(refs),'pad':r['pad'],'x_um':r['x_um'],'y_um':r['y_um'],'copper_um':r.get('copper_um'),'uuids':sorted({m[3] for m in matches})})
 row={'board':source.stem,'renamed_records':changed,'missing_footprint_uuids':missing}
 summary.append(row)
 print(source.stem,len(changed),'renamed',len(missing),'missing UUIDs',flush=True)
(root/'reports/quality-reference-census.json').write_text(json.dumps(summary,indent=2))
print('DONE',len(summary),'boards',sum(bool(r.get('renamed_records')) for r in summary),'affected',flush=True)
