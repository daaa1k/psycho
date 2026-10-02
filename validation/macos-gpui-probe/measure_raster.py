"""Measure image and code box edges against the probe's placement log.

Requires ffmpeg for decoding the saved PNG evidence. Run from the repository root.
"""

import csv
import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parent
EVIDENCE = ROOT / "evidence"
FILLS = {
    "image-1-to-1": (bytes((229, 232, 235)), 1),
    "image-1-to-2": (bytes((229, 232, 235)), 1),
    "tabbed-code": (bytes((241, 242, 243)), 0),
    "code-1-to-2": (bytes((241, 242, 243)), 0),
}
MODES = {
    "editing-small": "small",
    "editing-large": "large",
    "fullscreen": "fullscreen",
}


def image_size(path):
    lines = subprocess.check_output(
        ["sips", "-g", "pixelWidth", "-g", "pixelHeight", str(path)], text=True
    )
    return tuple(int(lines.split(f"pixel{axis}: ")[1].splitlines()[0]) for axis in ("Width", "Height"))


def fill_bounds(path, color, border):
    width, height = image_size(path)
    pixels = subprocess.check_output(
        ["ffmpeg", "-v", "error", "-i", str(path), "-f", "rawvideo", "-pix_fmt", "rgb24", "-"]
    )
    matches = [i for i in range(width * height) if pixels[i * 3 : i * 3 + 3] == color]
    if not matches:
        raise ValueError(f"placeholder fill missing: {path}")
    xs = [i % width for i in matches]
    ys = [i // width for i in matches]
    # Right and bottom are exclusive edges, matching the CSV bounds.
    return min(xs) - border, min(ys) - border, max(xs) + border + 1, max(ys) + border + 1


with (EVIDENCE / "layout-coordinate-log-aerospace-disabled.csv").open() as source:
    rows = list(csv.DictReader(source))

max_error = 0.0
with (EVIDENCE / "layout-raster-measurements.csv").open("w", newline="") as output:
    writer = csv.writer(output, lineterminator="\n")
    writer.writerow(("mode", "slide", "element", "edge", "expected_pixel", "observed_pixel", "error_px"))
    for row in rows:
        if row["element"] not in FILLS:
            continue
        mode, slide = row["mode"], row["slide"]
        path = EVIDENCE / f"layout-{MODES[mode]}-1-{slide}-live.png"
        color, border = FILLS[row["element"]]
        observed = fill_bounds(path, color, border)
        x = float(row["pixel_x"])
        y = float(row["pixel_y"]) + (0 if mode == "fullscreen" else 32)
        expected = (x, y, x + float(row["pixel_width"]), y + float(row["pixel_height"]))
        for edge, predicted, actual in zip(("left", "top", "right", "bottom"), expected, observed):
            error = abs(actual - predicted)
            max_error = max(max_error, error)
            writer.writerow((mode, slide, row["element"], edge, f"{predicted:.3f}", actual, f"{error:.3f}"))

print(f"maximum edge error: {max_error:.3f} px")
if max_error > 1:
    raise SystemExit("raster edge error exceeds 1 px")
