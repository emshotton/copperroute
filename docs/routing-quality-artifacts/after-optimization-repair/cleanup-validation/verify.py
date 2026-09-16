import pathlib,json,hashlib
out=pathlib.Path(__file__).resolve().parent
root=pathlib.Path('/home/em/copperroute-congestion-main/benchmark/results')
rows=[]
for b in json.loads((out/'selection.json').read_text())['boards']:
 name=b['id'];old=root/'conflict-terminal-full-01/final'/name/'seed-1';new=root/'after-repair-cleanup-01/repair'/name/'seed-1'
 if not (new/'metrics.json').exists():continue
 a=json.loads((old/'metrics.json').read_text());b=json.loads((new/'metrics.json').read_text());r=json.loads((new/'referee.json').read_text())
 rows.append({'board':name,'status':r['status'],'same_ses':(old/'out.ses').read_bytes()==(new/'out.ses').read_bytes(),'prototype_unrouted':a['unrouted'],'cleaned_unrouted':b['unrouted'],'prototype_copper':a['violations'],'cleaned_copper':b['violations'],'cpu_s':b['cpu_s'],'timed_out':b['timed_out']})
result={'completed':len(rows),'rows':rows}
(out/'verification.json').write_text(json.dumps(result,indent=2))
print(json.dumps(result,indent=2))
