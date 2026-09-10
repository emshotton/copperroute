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
def edge_errors(report):
    return [v for v in report['violations'] if v['type'] in ('copper_edge_clearance', 'invalid_outline')]
print(json.dumps({
    'dsn_sha256': hashlib.sha256(dsn.encode()).hexdigest(),
    'dsn_boundary_count': counts['boundary'],
    'dsn_keepout_count': counts['keepout'],
    'ground_truth': json.loads((args.board / 'ground_truth.json').read_text()),
    'original_edge_errors': edge_errors(original),
    'candidate_edge_errors': edge_errors(candidate),
}, indent=2))
