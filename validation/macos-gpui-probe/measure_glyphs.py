"""Compare independent CoreText glyph coordinates and raster evidence.

CoreText shapes fixed reference lines at 1280x720, independently of GPUI. The
actual log records origins submitted to GPUI; GPUI quantizes x to 1/4 px and y to
1 px. Check every nonblank glyph, its ID, order, and both quantized coordinates.
Also check strong ink in each screenshot against the independent reference's
weak ink within 1px in both directions, allowing antialiasing intensity changes.
Requires numpy and Pillow. Any missing glyph, >1px coordinate error or missing
stroke fails. No fitted offsets, screenshot-derived positions, or thresholds
selected per screenshot are used.
"""
import argparse
import csv
import json
from collections import defaultdict
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent
parser = argparse.ArgumentParser()
parser.add_argument('--reference', type=Path, default=ROOT/'target/layout-reference')
parser.add_argument('--images', type=Path, default=ROOT/'evidence/shared-layout')
parser.add_argument('--origins', type=Path)
parser.add_argument('--mode', choices=['small','large','fullscreen'])
parser.add_argument('--slide', type=int, choices=[1,2])
parser.add_argument('--output', type=Path, default=ROOT/'evidence/shared-layout/glyph-measurements.csv')
args = parser.parse_args()
origins = args.origins or args.images/'glyph-origins.csv'
refs = [row for row in json.loads((args.reference/'glyphs.json').read_text()) if (not args.mode or row['mode']==args.mode) and (not args.slide or row['slide']==args.slide)]
groups = defaultdict(list)
for row in refs:
    groups[row['mode'], row['slide'], row['element']].append(row)
actual = {}
modes = {('1024','760'):'small', ('1440','960'):'large', ('1920','1200'):'fullscreen'}
for row in csv.reader(origins.open()):
    mode = modes.get(tuple(row[:2]))
    if not mode:
        continue
    assert float(row[8]) == 1, 'this acceptance fixture requires window scale 1'
    title = bytes.fromhex(row[7]).decode()
    slide = 2 if '1-to-2' in row[2] or (row[2] == 'heading' and title.startswith('比率')) else 1
    actual[mode, slide, row[2], int(row[3])] = row


def dilate(mask):
    padded = np.pad(mask, 1)
    result = np.zeros_like(mask)
    for y in range(3):
        for x in range(3):
            result |= padded[y:y+mask.shape[0], x:x+mask.shape[1]]
    return result


cache = {}
for mode in ([args.mode] if args.mode else ['small', 'large', 'fullscreen']):
    for slide in ([args.slide] if args.slide else [1, 2]):
        expected = np.array(Image.open(args.reference/f'reference-{mode}-1-{slide}.png').convert('RGB')).max(axis=2)
        observed = np.array(Image.open(args.images/f'layout-{mode}-1-{slide}-live.png').convert('RGB')).max(axis=2)
        assert expected.shape == observed.shape
        # On white, the usual foreground is 34 and Caption's lightest channel is 87.
        # Strong ink <100 (<140 for Caption); weak ink <220. The interval between
        # the two masks is deliberately ignored as permitted antialiasing variation.
        strong, observed_strong = expected < 100, observed < 100
        for row in refs:
            if row['mode'] == mode and row['slide'] == slide and row['element'].startswith('caption'):
                l,t,r,b = [int(round(row[name])) for name in ('left','top','right','bottom')]
                region = np.s_[t-2:b+3, l-2:r+3]
                strong[region] = expected[region] < 140
                observed_strong[region] = observed[region] < 140
        if mode != 'fullscreen':
            observed_strong[:176] = False
            observed_strong[observed.shape[0]-72:] = False
        missing = strong & ~dilate(observed < 220)
        extra = observed_strong & ~dilate(expected < 220)
        if missing.any() or extra.any():
            print(f'raster mismatch {mode}/{slide}: missing={missing.sum()} extra={extra.sum()}')
        cache[mode, slide] = missing, extra, observed < 220

failures = []
maximum = 0.0
with args.output.open('w', newline='') as output:
    writer = csv.writer(output, lineterminator='\n')
    writer.writerow(['mode','slide','element','glyph_index','font','glyph','expected_x','expected_y','quantized_x','quantized_y','error_x_px','error_y_px','missing_ink','extra_ink','verdict'])
    for key, references in groups.items():
        present = {index for mode,slide,element,index in actual if (mode,slide,element) == key}
        if present != set(range(len(references))):
            raise AssertionError(f'glyph count/order mismatch: {key}')
        for index, ref in enumerate(references):
            row = actual[(*key, index)]
            assert int(row[4]) == ref['glyph'], f'glyph ID mismatch: {key}/{index}'
            x = round(float(row[5])*4)/4
            y = round(float(row[6])) + (0 if key[0] == 'fullscreen' else 32)
            error_x, error_y = abs(x-ref['x']), abs(y-ref['y'])
            maximum = max(maximum, error_x, error_y)
            l,t,r,b = [int(round(ref[name])) for name in ('left','top','right','bottom')]
            region = np.s_[t-2:b+3, l-2:r+3]
            missing,extra,ink = cache[key[:2]]
            absent,unexpected = int(missing[region].sum()),int(extra[region].sum())
            ok = error_x <= 1 and error_y <= 1 and absent == 0 and unexpected == 0 and ink[region].any()
            verdict = 'PASS' if ok else 'FAIL'
            writer.writerow([*key,index,ref['font'],ref['glyph'],f"{ref['x']:.6f}",f"{ref['y']:.6f}",x,y,f'{error_x:.6f}',f'{error_y:.6f}',absent,unexpected,verdict])
            if not ok:
                failures.append((*key,index,round(error_x,6),round(error_y,6),absent,unexpected))
print(f'glyphs={len(refs)} failures={len(failures)} maximum_quantized_coordinate_error={maximum:.6f} px')
for failure in failures[:10]:
    print(failure)
raise SystemExit(bool(failures) or any(missing.any() or extra.any() for missing, extra, _ in cache.values()))
