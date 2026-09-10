from pathlib import Path
import pcbnew,json
root=Path('/root/copperroute-bench/benchmark')
p=root/'corpus/pcbench/96boards-sensors_Sensors'
out={}
for name in ['raw.kicad_pcb','stripped.kicad_pcb']:
 b=pcbnew.LoadBoard(str(p/name));rows=[]
 for fp in b.GetFootprints():
  if not fp.GetReference().startswith('REF**'):continue
  rows.append({'reference':fp.GetReference(),'uuid':fp.m_Uuid.AsString(),'x':fp.GetPosition().x,'y':fp.GetPosition().y})
 out[name]=rows
print(json.dumps(out,indent=2))
(root/'reports/duplicate-pad-reference-audit.json').write_text(json.dumps(out,indent=2))
