import concurrent.futures,dataclasses,hashlib,json,os,shutil,subprocess
from pathlib import Path
from bench import corpus
from bench.referee import kicad
root=Path.cwd();out=root/'reports/artwork-fixed-copper-study';board=next(b for b in corpus.load_manifest() if b.id=='pcbench-8bit-cpu_programming_interface')
original=root/'results/quality-epyc-mask-via-01/mask-via'/board.id/'seed-1/out.ses'
if not original.exists():
 candidates=list((root/'results/quality-epyc-mask-via-01').glob('*/'+board.id+'/seed-1/out.ses'))
 candidates=[p for p in candidates if p.parts[-4]!='main'];assert len(candidates)==1,candidates;original=candidates[0]

def run(case):
 cell=out/case;cell.mkdir(exist_ok=True)
 if case=='copper-aware':
  shutil.copy2(out/'preserved-obstacles.dsn',cell/'in.dsn')
  env=os.environ.copy();env['COPPERROUTE_PAD_CLEARANCE_JSON']='/root/copperroute-mask-via/pad-metadata/'+board.id+'.json';env['COPPERROUTE_MASK_GAP_CAP']='1';env.pop('COPPERROUTE_FAILED_FIRST',None)
  argv=['/root/copperroute-mask-via/target/release/copperroute','route',str(cell/'in.dsn'),'-o',str(cell/'out.ses'),'--max-passes','10','--timeout','00:05:00','--result-json',str(cell/'result.json'),'--set','router.max_threads=1']
  (cell/'argv.json').write_text(json.dumps(argv))
  with (cell/'routing.log').open('w') as f:subprocess.run(argv,stdout=f,stderr=f,env=env,check=True,timeout=330)
 else:shutil.copy2(original,cell/'out.ses')
 k=dict(board.kicad)
 if case!='stripped-control':k['stripped']=str(out/'preserved.kicad_pcb')
 modified=dataclasses.replace(board,kicad=k)
 r=kicad.run(modified,cell)
 result={'case':case,'ses_sha256':hashlib.sha256((cell/'out.ses').read_bytes()).hexdigest(),'referee':r}
 (cell/'pilot-result.json').write_text(json.dumps(result,indent=2));print(case,r['status'],r.get('unrouted'),r.get('violations'),r.get('violations_by_type'),flush=True);return result
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:results=list(pool.map(run,['stripped-control','preserved-referee-only','copper-aware']))
(out/'preservation-pilot-results.json').write_text(json.dumps(results,indent=2))
assert all(r['referee']['status']=='ok' for r in results)
