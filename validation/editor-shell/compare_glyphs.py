"""Compare base glyph placement across fitted, actual-size and pane states."""
import argparse
import collections
import csv
import json
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument('trace', type=Path)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
positions = collections.defaultdict(list)
scales = set()
viewports = set()
for row in csv.reader(args.trace.open()):
    if len(row) != 12 or float(row[8]) < .1:
        continue
    text = bytes.fromhex(row[3]).decode()
    if text not in ['Heading', 'Original body']:
        continue
    scale = float(row[8])
    scales.add(scale)
    viewports.add(tuple(map(float, row[:2])))
    key = (text, row[2], row[4], row[5])
    positions[key].append(((float(row[6]) - float(row[9])) / scale,
                           (float(row[7]) - float(row[10])) / scale))
assert len(positions) == len('Heading') + len('Original body'), positions.keys()
assert len(scales) >= 4 and len(viewports) >= 3, (scales, viewports)
errors = {str(key): max(max(point[axis] for point in values) - min(point[axis] for point in values)
                       for axis in [0, 1]) for key, values in positions.items()}
assert max(errors.values()) < .001, errors
args.output.write_text(json.dumps(dict(glyphs=len(positions), scales=sorted(scales),
    viewports=sorted(viewports), maxBasePositionError=max(errors.values()),
    toleranceBasePixels=.001, errors=errors), indent=2))
print('PASS base glyphs and wrapping remain unchanged across view states')
