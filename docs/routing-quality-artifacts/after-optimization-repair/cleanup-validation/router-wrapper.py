import json,os,pathlib,sys
arm,binary,*args=sys.argv[1:]
assert args[0]=='route'
for name in ('COPPERROUTE_DEMAND_GUIDANCE','COPPERROUTE_TARGET_BOUND','COPPERROUTE_STRUCTURAL_REPAIR','COPPERROUTE_ADAPTIVE_GUIDANCE','COPPERROUTE_ADAPTIVE_TRACE','COPPERROUTE_CONNECTION_RECOVERY','COPPERROUTE_GROUP_PLANNING','COPPERROUTE_GROUP_TRACE','COPPERROUTE_GROUP_TIMING'):
 os.environ.pop(name,None)
if arm in ('demand','combined'): os.environ['COPPERROUTE_DEMAND_GUIDANCE']='1'
if arm in ('targets','combined'): os.environ['COPPERROUTE_TARGET_BOUND']='1'
if arm in ('alternatives','connections'):
 os.environ['COPPERROUTE_ADAPTIVE_GUIDANCE']='1'
 os.environ['COPPERROUTE_ADAPTIVE_TRACE']='1'
if arm=='connections': os.environ['COPPERROUTE_CONNECTION_RECOVERY']='1'
os.environ['COPPERROUTE_AFTER_OPTIMIZATION_REPAIR']='1'
cell=pathlib.Path(args[1]).parent
root=pathlib.Path('/home/em/copperroute-congestion-main/benchmark')
entry=next(b for b in json.loads((root/'corpus/manifest.json').read_text())['boards'] if b['id']==cell.parent.name)
project=entry.get('kicad',{}).get('project')
if project: args+=['--kicad-project',str(root/'corpus'/project)]
(cell/'actual-router-argv.json').write_text(json.dumps({'arm':arm,'argv':[binary,*args],'experiment_env':{k:v for k,v in os.environ.items() if k.startswith('COPPERROUTE_')}}))
os.execv(binary,[binary,*args])
