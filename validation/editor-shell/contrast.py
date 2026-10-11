"""Check palette contrast and actual selected diagnostic screenshot colors."""
import argparse
import json
from pathlib import Path
import re
import subprocess

parser = argparse.ArgumentParser()
parser.add_argument('--evidence', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
source = (root / 'src/editor_view.rs').read_text()
blocks = re.findall(r'Self \{\s*background: rgb\(0x[0-9a-f]+\).*?\n            \}', source, re.DOTALL)
assert len(blocks) == 2, blocks
palettes = dict(zip(['dark', 'light'], [dict(re.findall(r'(\w+): rgb\(0x([0-9a-f]+)\)', block)) for block in blocks]))


def luminance(hex_color):
    value = int(hex_color, 16)
    components = [((value >> shift) & 255) / 255 for shift in [16, 8, 0]]
    linear = [value / 12.92 if value <= .04045 else ((value + .055) / 1.055) ** 2.4 for value in components]
    return sum(value * weight for value, weight in zip(linear, [.2126, .7152, .0722]))


ratios = {}
for mode, palette in palettes.items():
    for foreground in ['text', 'secondary', 'warning', 'focus']:
        for background in ['surface', 'background', 'selection', 'input']:
            low, high = sorted([luminance(palette[foreground]), luminance(palette[background])])
            ratio = (high + .05) / (low + .05)
            assert ratio >= 4.5, (mode, foreground, background, ratio)
            ratios[f'{mode}-{foreground}-{background}'] = ratio
helper = args.output.with_suffix('.pixel-counts')
subprocess.run(['xcrun', 'swiftc', str(Path(__file__).with_name('pixel_counts.swift')), '-o', str(helper)], check=True)
sampled = {}
for kind in ['syntax', 'schema']:
    image = args.evidence / kind / 'selected-source-evidence.png'
    counts = json.loads(subprocess.check_output([str(helper), str(image)], text=True))
    def distance(left, right):
        return max(abs(int(left[index:index + 2], 16) - int(right[index:index + 2], 16)) for index in [0, 2, 4])
    observed = {}
    for role in ['surface', 'text', 'warning', 'selection', 'focus']:
        candidates = [(color, count) for color, count in counts.items() if distance(color, palettes['dark'][role]) <= 12]
        assert candidates, (kind, role)
        color, count = max(candidates, key=lambda item: item[1])
        assert count > 10, (kind, role, color, count)
        observed[role] = dict(rgb=color, pixels=count)
    measured = {}
    for foreground in ['text', 'warning', 'focus']:
        low, high = sorted([luminance(observed[foreground]['rgb']), luminance(observed['selection']['rgb'])])
        measured[foreground] = (high + .05) / (low + .05)
        assert measured[foreground] >= 4.5, (kind, foreground, measured)
    sampled[kind] = dict(screenshot=str(image), rawScreenshotColors=observed,
        selectedSourceContrast=measured, sourcePaletteMatchingChannelTolerance=12)
args.output.write_text(json.dumps(dict(ratios=ratios, minimumRatio=min(ratios.values()), selectedDiagnosticPixels=sampled), indent=2))
print('PASS UI palette contrast and actual dark diagnostic/source pixels')
