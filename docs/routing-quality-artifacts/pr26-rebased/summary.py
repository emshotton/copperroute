import hashlib,json,statistics
from pathlib import Path
root=Path('/root/copperroute-bench/benchmark')
run='quality-epyc-pr26-rebased-full-01'
names=['main','neckdown']
data={n:json.loads((root/'reports'/f'{run}-{n}-details.json').read_text())['boards'] for n in names}
ids=set(data['main'])
assert len(ids)==751
assert all(set(d)==ids for d in data.values())
assert all(v['referee']['status']=='ok' for d in data.values() for v in d.values())
def complete(v):return v['metrics'].get('self',{}).get('final_state')=='COMPLETED' and not v['metrics']['timed_out'] and not v['metrics'].get('failed')
def metric(v,k):return v['referee'].get('violations_by_type',{}).get('solder_mask_bridge',0) if k=='mask' else v['metrics'][k]
hashes={n:{b:hashlib.sha256((root/'results'/run/n/b/'seed-1/out.ses').read_bytes()).hexdigest() for b in ids} for n in names}
results={}
for a,b in [('main','neckdown')]:
 groups={}
 for group,prefix in [('pcbench','pcbench-'),('local','kicad-')]:
  allids=sorted(k for k in ids if k.startswith(prefix))
  for subset in ['all','both_completed','completed_identical_ses','completed_changed_ses']:
   selected=[k for k in allids if subset=='all' or (complete(data[a][k]) and complete(data[b][k]) and (subset=='both_completed' or (hashes[a][k]==hashes[b][k])==(subset=='completed_identical_ses')))]
   r={'count':len(selected)}
   for key in ['unrouted','violations','mask','cpu_s']:
    x=[metric(data[a][k],key) for k in selected];y=[metric(data[b][k],key) for k in selected]
    r[key]={'baseline':sum(x),'candidate':sum(y),'delta':sum(y)-sum(x),'better':sum(j<i for i,j in zip(x,y)),'worse':sum(j>i for i,j in zip(x,y))}
   r['cpu_ratio']=r['cpu_s']['candidate']/r['cpu_s']['baseline'] if r['cpu_s']['baseline'] else None
   if selected:
    r['rss_median_mb']=[statistics.median(data[n][k]['metrics']['peak_rss_mb'] for k in selected) for n in [a,b]]
    r['rss_max_mb']=[max(data[n][k]['metrics']['peak_rss_mb'] for k in selected) for n in [a,b]]
   r['completed']=[sum(complete(data[n][k]) for k in selected) for n in [a,b]]
   r['fully_connected']=[sum(data[n][k]['metrics']['unrouted']==0 for k in selected) for n in [a,b]]
   groups[group+'/'+subset]=r
 results[a+'/'+b]={'groups':groups,'rows':[{'board':k,'both_completed':complete(data[a][k]) and complete(data[b][k]),'identical_ses':hashes[a][k]==hashes[b][k],**{key+'_delta':metric(data[b][k],key)-metric(data[a][k],key) for key in ['unrouted','violations','mask','cpu_s']}} for k in sorted(ids)]}
(root/'reports'/f'{run}-comparisons.json').write_text(json.dumps(results,indent=2))
(root/'reports'/f'{run}-ses-hashes.json').write_text(json.dumps(hashes,indent=2))
print('All 1502 referees valid; comparisons and output identities written')
