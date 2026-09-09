import json,statistics,sys
from pathlib import Path
ap,bp,out=map(Path,sys.argv[1:]);ad=json.loads(ap.read_text());bd=json.loads(bp.read_text());a=ad['boards'];b=bd['boards'];assert a.keys()==b.keys()
assert all(v['referee']['status']=='ok' for d in [a,b] for v in d.values())
def metric(row,key):
 if key=='mask':return row['referee'].get('violations_by_type',{}).get('solder_mask_bridge',0)
 return row['metrics'][key]
def complete(row):return row['metrics'].get('self',{}).get('final_state')=='COMPLETED' and not row['metrics']['timed_out']
result={'baseline':str(ap),'candidate':str(bp),'count':len(a),'groups':{},'rows':[]}
for group,prefix in [('pcbench','pcbench-'),('local','kicad-')]:
 ids=[k for k in sorted(a) if k.startswith(prefix)]
 if not ids:continue
 r={'boards':len(ids)}
 for key in ['unrouted','violations','mask','cpu_s']:
  av=[metric(a[k],key) for k in ids];bv=[metric(b[k],key) for k in ids]
  r[key]={'baseline':sum(av),'candidate':sum(bv),'delta':sum(bv)-sum(av),'better':sum(y<x for x,y in zip(av,bv)),'worse':sum(y>x for x,y in zip(av,bv))}
 r['cpu_ratio']=r['cpu_s']['candidate']/r['cpu_s']['baseline']
 for label,fn in [('completed',complete),('connected',lambda row:row['metrics']['unrouted']==0)]:r[label]=[sum(fn(d[k]) for k in ids) for d in [a,b]]
 r['rss_median_mb']=[statistics.median(d[k]['metrics']['peak_rss_mb'] for k in ids) for d in [a,b]]
 r['rss_max_mb']=[max(d[k]['metrics']['peak_rss_mb'] for k in ids) for d in [a,b]]
 result['groups'][group]=r
for k in sorted(a):
 r={'board':k,'both_completed':complete(a[k]) and complete(b[k])}
 for key in ['unrouted','violations','mask','cpu_s']:r[key+'_delta']=metric(b[k],key)-metric(a[k],key)
 result['rows'].append(r)
out.write_text(json.dumps(result,indent=2));print(json.dumps(result['groups'],indent=2))
