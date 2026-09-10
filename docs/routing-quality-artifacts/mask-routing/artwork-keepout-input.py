from pathlib import Path
import json
import pcbnew as p
root=Path('/root/copperroute-bench/benchmark');src=root/'corpus/pcbench/8bit-cpu_programming_interface';out=root/'reports/artwork-fixed-copper-study'
b=p.LoadBoard(str(src/'raw.kicad_pcb'));additions=[]
for index,t in enumerate(b.GetTracks()):
 if t.GetNetCode()!=0:continue
 shape=p.SHAPE_POLY_SET();t.TransformShapeToPolygon(shape,t.GetLayer(),0,1000,p.ERROR_OUTSIDE)
 for j in range(shape.OutlineCount()):
  chain=shape.COutline(j);coords=' '.join(f'{chain.CPoint(i).x/1000:.3f} {-chain.CPoint(i).y/1000:.3f}' for i in range(chain.PointCount()))
  additions.append(f'    (keepout "fixed_copper_artwork_{index}_{j}" (polygon {json.dumps(b.GetLayerName(t.GetLayer()))} 0 {coords}))\n')
assert len(additions)==223
source=(src/'unrouted.dsn').read_text();assert '    (boundary' in source
(out/'preserved-obstacles.dsn').write_text(source.replace('    (boundary',''.join(additions)+'    (boundary',1))
print(len(additions),'fixed-copper keepouts added')
