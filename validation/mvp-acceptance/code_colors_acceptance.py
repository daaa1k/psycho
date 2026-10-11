import argparse
import csv
import json
from pathlib import Path

import numpy as np
from PIL import Image

from gui_session import Gui, root

parser = argparse.ArgumentParser()
parser.add_argument('--output', type=Path, required=True)
cases = [
    ('rust', 'rust', 'let value = 42; // comment', True),
    ('yaml', 'yaml', 'key: "value"', True),
    ('json', 'json', '{"key": 42}', True),
    ('toml', 'toml', 'key = "value"', True),
    ('unsupported', 'python', 'let value = 42; // comment', False),
    ('unspecified', None, 'let value = 42; // comment', False),
]
slides = []
for name, language, text, _ in cases:
    attribute = '' if language is None else ' language=' + json.dumps(language)
    slides.append('slide { heading ' + json.dumps(name) + '; code' + attribute + ' ' + json.dumps(text) + '; }')
gui = Gui(parser.parse_args().output,
          'presentation { metadata { title "Code colors" }; ' + '; '.join(slides) + '; }\n')
records = []
try:
    gui.click(900, 750)
    for index, (name, language, text, highlighted) in enumerate(cases):
        gui.select_slide(index + 1)
        state, _ = gui.snapshot(name)
        glyphs = [row for row in csv.reader(gui.trace.open())
                  if len(row) == 12 and row[2] == '22' and bytes.fromhex(row[3]).decode() == text]
        start = max(i for i, row in enumerate(glyphs) if row[4] == '0')
        glyphs = glyphs[start:]
        assert glyphs, 'Code must be drawn before checking its colors'
        first = glyphs[0]
        physical = state['screenshot']['scale']
        title_height = state['snapshot']['window']['height'] - float(first[1])
        slide_scale = float(first[8])
        box = (round(min(float(row[6]) for row in glyphs) * physical),
               round((float(first[7]) + title_height - 24 * slide_scale) * physical),
               round((max(float(row[6]) for row in glyphs) + 22 * slide_scale) * physical),
               round((float(first[7]) + title_height + 8 * slide_scale) * physical))
        pixels = np.array(Image.open(gui.out / (name + '.png')).convert('RGB').crop(box)).astype(int)
        colored = int(((pixels.max(axis=2) - pixels.min(axis=2) > 30) & (pixels.min(axis=2) < 220)).sum())
        assert (colored > 10 if highlighted else colored == 0), (name, colored, box)
        assert gui.fixture.read_bytes() == gui.original
        records.append(dict(case=name, language=language, expected_highlight=highlighted,
                            colored_pixels=colored, crop=list(box), result='PASS'))
        print('PASS', name, 'highlighted' if highlighted else 'plain text', flush=True)
finally:
    (gui.out / 'observations.json').write_text(json.dumps(records, indent=2) + '\n')
    gui.stop()
