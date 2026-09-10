from pathlib import Path
import json,subprocess
root=Path('/root');b=root/'copperroute-bench/benchmark';board='pcbench-96boards-sensors_Sensors'
a=json.loads((b/'reports/duplicate-pad-reference-audit.json').read_text());dest={r['uuid']:r for r in a['stripped.kicad_pcb']}
records=json.loads((b/'reports/pad-local-copper-metadata-03'/f'{board}.json').read_text());changes=[]
for r in records:
 matches=[fp for fp in a['raw.kicad_pcb'] if fp['reference']==r['component'] and abs(fp['x']/1000-r['x_um'])<0.01 and abs(fp['y']/1000-r['y_um'])<0.01]
 if len(matches)!=1:continue
 fp=matches[0];new=dest[fp['uuid']]['reference']
 if new!=r['component']:changes.append({'before':r['component'],'after':new,'x_um':r['x_um'],'y_um':r['y_um']});r['component']=new
assert len(changes)==3,changes
out=b/'reports/duplicate-pad-reference-pilot';out.mkdir(exist_ok=False)
(out/f'{board}.json').write_text(json.dumps(records,indent=2));(out/'changes.json').write_text(json.dumps(changes,indent=2))
config=''
for name,meta in [('control',b/'reports/pad-local-copper-metadata-03'),('renamed',out)]:
 config+=f'[candidates.{name}]\nkind="rust"\nexec=["/root/quality-pad-post-main-router","/root/copperroute-pad-main-d84ac9e/target/release/copperroute","{meta}"]\nsha="d84ac9e+pad-copper+{name}"\n'
(b/'candidates.epyc-duplicate-pad-pilot.toml').write_text(config)
s=(root/'quality-pad-post-main-rescore.py').read_text().replace('quality-epyc-pad-post-main-01','quality-epyc-duplicate-pad-pilot-01');(root/'quality-duplicate-pad-rescore.py').write_text(s)
env=(root/'quality-epyc-pad-post-main.sh').read_text().split('cargo build')[0]
s=env+'''while ps -p 3631690 -o args= | grep -q '^bash /root/quality-epyc-pad-post-main.sh$'; do sleep 10; done
cd /root/copperroute-bench/benchmark
uv run bench run --candidates control,renamed --candidates-file candidates.epyc-duplicate-pad-pilot.toml --boards pcbench-96boards-sensors_Sensors --max-passes 10 --timeout 300 --threads 1 --jobs 192 --run-id quality-epyc-duplicate-pad-pilot-01
uv run python /root/quality-duplicate-pad-rescore.py
for candidate in control renamed; do
 uv run python /root/quality-kicad-collect-details.py quality-epyc-duplicate-pad-pilot-01 "$candidate" "reports/quality-epyc-duplicate-pad-pilot-$candidate-details.json"
done
'''
p=root/'quality-epyc-duplicate-pad-pilot.sh';p.write_text(s)
log=(b/'reports/epyc-duplicate-pad-pilot.log').open('w');proc=subprocess.Popen(['bash',str(p)],stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);print('driver',proc.pid)
