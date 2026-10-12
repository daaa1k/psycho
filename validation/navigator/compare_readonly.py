import argparse
import json
from pathlib import Path
from PIL import Image, ImageChops

parser = argparse.ArgumentParser()
parser.add_argument('output')
args = parser.parse_args()
out = Path(args.output)
observations = json.loads((out / 'observations.json').read_text())
scale = next(record['screenshotScale'] for record in observations if record['name'] == 'nested-elements')
region = tuple(int(value * scale) for value in (21, 168, 155, 244))
before = Image.open(out / 'nested-elements.png').convert('RGB').crop(region)
after = Image.open(out / 'nested-selected-no-thumbnail-border.png').convert('RGB').crop(region)
changed = ImageChops.difference(before, after).getbbox()
assert changed is None, changed
(out / 'readonly-thumbnail.json').write_text(json.dumps(dict(crop=region, changedBounds=changed,
    assertion='Selecting a nested canvas element does not add editor borders to the thumbnail'), indent=2))
print('PASS readonly thumbnail remains unchanged when a nested element is selected')
