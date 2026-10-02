"""Compare text ink anchors across the three saved layout sizes.

This checks whether the first dark pixel in each element follows the shared
placement and scale. It does not establish the absolute glyph origin or the
position of every glyph. Requires sips and ffmpeg; run from any directory.
"""

import csv
import subprocess
from collections import defaultdict
from pathlib import Path


ROOT = Path(__file__).resolve().parent
EVIDENCE = ROOT / "evidence"
MODES = {
    "editing-small": "small",
    "editing-large": "large",
    "fullscreen": "fullscreen",
}
KINDS = {"heading", "text", "bullets", "caption", "code"}


def load_image(path):
    description = subprocess.check_output(
        ["sips", "-g", "pixelWidth", "-g", "pixelHeight", str(path)], text=True
    )
    width = int(description.split("pixelWidth: ")[1].splitlines()[0])
    height = int(description.split("pixelHeight: ")[1].splitlines()[0])
    pixels = subprocess.check_output(
        ["ffmpeg", "-v", "error", "-i", str(path), "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]
    )
    return width, height, pixels


def first_ink(image, left, top, right, bottom):
    width, height, pixels = image
    matches = []
    for y in range(max(0, round(top)), min(height, round(bottom))):
        for x in range(max(0, round(left)), min(width, round(right))):
            start = 3 * (y * width + x)
            if max(pixels[start : start + 3]) <= 120:
                matches.append((x, y))
    if not matches:
        raise ValueError("no dark text pixel within the element")
    return min(x for x, _ in matches), min(y for _, y in matches)


with (EVIDENCE / "layout-coordinate-log-aerospace-disabled.csv").open() as source:
    placement_rows = list(csv.DictReader(source))

images = {}
groups = defaultdict(list)
for row in placement_rows:
    if row["slide"] not in {"1", "2"} or row["kind"] not in KINDS:
        continue
    # The preceding URL touches the first bullet box; its ink would be included.
    if row["slide"] == "1" and row["element"] == "bullets-1-to-1":
        continue
    mode, slide, element = row["mode"], row["slide"], row["element"]
    path = EVIDENCE / f"layout-{MODES[mode]}-1-{slide}-live.png"
    if path not in images:
        images[path] = load_image(path)
    x = float(row["pixel_x"])
    y = float(row["pixel_y"]) + (0 if mode == "fullscreen" else 32)
    width = float(row["pixel_width"])
    height = float(row["pixel_height"])
    ink_x, ink_y = first_ink(images[path], x, y, x + width, y + height)
    groups[(slide, element)].append((mode, float(row["canvas_scale"]), x, y, ink_x, ink_y))

maximum_residual = 0.0
with (EVIDENCE / "layout-text-anchor-consistency.csv").open("w", newline="") as output:
    writer = csv.writer(output, lineterminator="\n")
    writer.writerow(("slide", "element", "mode", "axis", "layout_pixel", "ink_pixel", "fitted_base_offset", "residual_px"))
    for (slide, element), observations in sorted(groups.items()):
        if len(observations) != 3:
            raise ValueError(f"expected three sizes for {slide} {element}")
        for axis in ("x", "y"):
            index = 2 if axis == "x" else 3
            ink_index = 4 if axis == "x" else 5
            base_offset = sum((item[ink_index] - item[index]) / item[1] for item in observations) / 3
            for item in observations:
                mode, scale = item[:2]
                layout_pixel, ink_pixel = item[index], item[ink_index]
                residual = abs(ink_pixel - layout_pixel - scale * base_offset)
                maximum_residual = max(maximum_residual, residual)
                writer.writerow((slide, element, mode, axis, f"{layout_pixel:.3f}", ink_pixel, f"{base_offset:.3f}", f"{residual:.3f}"))

print(f"maximum text anchor residual: {maximum_residual:.3f} px")
