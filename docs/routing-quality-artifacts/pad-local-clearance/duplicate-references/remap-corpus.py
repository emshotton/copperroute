from pathlib import Path
from collections import defaultdict
import json, pcbnew
from reference_mapping import remap_references
root=Path('/root/copperroute-bench/benchmark')
entries={b['id']:b for b in json.loads((root/'corpus/manifest.json').read_text())['boards']}
out=root/'reports/pad-local-copper-metadata-05'
out.mkdir(exist_ok=False)
summary=[]
for source in sorted((root/'reports/pad-local-copper-metadata-03').glob('*.json')):
 if source.stem not in entries:continue
 entry=entries[source.stem]; k=entry['kicad']
 raw=pcbnew.LoadBoard(str(root/'corpus'/k['raw']))
 stripped=pcbnew.LoadBoard(str(root/'corpus'/k['stripped']))
 dest=defaultdict(list)
 for fp in stripped.GetFootprints():dest[fp.m_Uuid.AsString()].append(fp)
 source_ids=defaultdict(list)
 for fp in raw.GetFootprints():source_ids[fp.m_Uuid.AsString()].append(fp)
 pads=[];refs={};rejected=[]
 for uid,fps in source_ids.items():
  if len(fps)!=1 or len(dest[uid])!=1:
   rejected.append({'uuid':uid,'reason':'missing or duplicate footprint UUID'});continue
  fp=fps[0];df=dest[uid][0]
  if fp.GetPosition()!=df.GetPosition() or fp.GetOrientationDegrees()!=df.GetOrientationDegrees() or fp.GetLayer()!=df.GetLayer():
   rejected.append({'uuid':uid,'reason':'footprint position, orientation or layer differs'});continue
  refs[uid]=df.GetReference()
  for pad in fp.Pads():
   pos=pad.GetPosition()
   dp=[p for p in df.Pads() if p.GetNumber()==pad.GetNumber() and p.GetPosition()==pos and p.GetSize()==pad.GetSize() and p.GetShape()==pad.GetShape() and list(p.GetLayerSet().Seq())==list(pad.GetLayerSet().Seq())]
   if not dp:
    rejected.append({'uuid':uid,'pad':pad.GetNumber(),'reason':'destination pad geometry differs'});continue
   pads.append(dict(component=fp.GetReference(),pad=pad.GetNumber(),x_um=pos.x/1000,y_um=pos.y/1000,uuid=uid))
 records=json.loads(source.read_text());changes=[];unresolved=[];mapped=[]
 for n,record in enumerate(records):
  try:
   new,changed,missing=remap_references([record],pads,refs)
  except ValueError as e:
   mapped.append(record);unresolved.append({'record':n,'reason':str(e)});continue
  mapped.extend(new)
  if changed:changes.append(dict(changed[0],record=n,copper_um=record.get('copper_um')))
  if missing:unresolved.append({'record':n,'reason':'no verified source and destination pad match'})
 (out/source.name).write_text(json.dumps(mapped,indent=2))
 summary.append({'board':source.stem,'changes':changes,'unresolved':unresolved,'rejected_geometry':rejected})
 print(source.stem,len(changes),'changes',len(unresolved),'unresolved',flush=True)
(root/'reports/quality-reference-remap-verified-census.json').write_text(json.dumps(summary,indent=2))
print('DONE',len(summary),'boards',sum(bool(r['changes']) for r in summary),'affected',flush=True)
