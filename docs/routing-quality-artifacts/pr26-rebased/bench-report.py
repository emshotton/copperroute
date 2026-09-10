import json
from pathlib import Path
from bench import compare as compare_mod,corpus,paths
from bench.report import markdown as md_report,pr as pr_report
run=Path('results/quality-epyc-pr26-rebased-full-01').resolve()
for baseline,against,label in [('main',['neckdown'],'main')]:
 c=compare_mod.compare([run],baseline,against,corpus.load_manifest(),time_metric='cpu')
 name='quality-epyc-pr26-rebased-full-01-vs-'+label
 (paths.REPORTS/f'{name}.json').write_text(json.dumps(c,indent=2))
 (paths.REPORTS/f'{name}.md').write_text(md_report.render(c))
 (paths.REPORTS/f'{name}.pr.md').write_text(pr_report.render(c,name))
 (paths.REPORTS/f'{name}-gate.json').write_text(json.dumps(compare_mod.gate_failures(c,require_complete=True),indent=2))
 print(name,'written')
