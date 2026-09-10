import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re

parser = argparse.ArgumentParser()
parser.add_argument('board', type=Path)
parser.add_argument('candidate', type=Path)
args = parser.parse_args()
dsn = (args.board / 'unrouted.dsn').read_text()
stack, root = [], []
for token in re.findall(r'"(?:\\.|[^"\\])*"|[()]|[^\s()]+', dsn):
    if token == '(':
        node = []
        if stack:
            stack[-1].append(node)
        else:
            root = node
        stack.append(node)
    elif token == ')':
        stack.pop()
    else:
        stack[-1].append(token)
counts, polygons = Counter(), Counter()
def walk(node, path):
    if not isinstance(node, list) or not node:
        return
    counts[node[0]] += 1
    if node[0] == 'polygon':
        polygons['/'.join(path[-3:])] += 1
    for value in node:
        if isinstance(value, list):
            walk(value, path + [node[0]])
walk(root, [])
original = json.loads((args.board / 'raw-drc.json').read_text())
candidate = json.loads((args.candidate / 'referee-drc.json').read_text())
def text_errors(report):
    return [v for v in report['violations']
            if any('Walter' in i['description'] for i in v.get('items', []))]
print(json.dumps({
    'dsn_sha256': hashlib.sha256(dsn.encode()).hexdigest(),
    'raw_has_text': 'Walter' in (args.board / 'raw.kicad_pcb').read_text(),
    'dsn_keepouts': counts['keepout'], 'dsn_wires': counts['wire'],
    'polygon_parents': dict(polygons),
    'ground_truth': json.loads((args.board / 'ground_truth.json').read_text()),
    'original_text_errors': text_errors(original),
    'candidate_text_errors': text_errors(candidate),
}, indent=2))
