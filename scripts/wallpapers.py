#!/usr/bin/env python3
# Writes assets/wallpapers/wall-1.jpg … wall-7.jpg: soft mesh gradients with a fixed-seed grain so JPEG does not band.
import math
import random
from pathlib import Path

from PIL import Image, ImageChops, ImageFilter

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets/wallpapers"

W, H = 2560, 1440
GRID = (160, 90)
QUALITY = 90

# (base, [(x, y, radius, colour)]) in unit coordinates, colours as hex.
PALETTES = [
    ("f6e7df", [(0.10, 0.15, 0.40, "ffd1b8"), (0.85, 0.20, 0.35, "f3b6cf"),
                (0.55, 0.85, 0.45, "c9b8f2"), (0.20, 0.90, 0.30, "ffe9c7")]),
    ("cfeee9", [(0.15, 0.20, 0.40, "8fd8d0"), (0.80, 0.15, 0.35, "9cc3f5"),
                (0.70, 0.85, 0.45, "4f86d8"), (0.10, 0.85, 0.35, "c8f2dc")]),
    ("2b2357", [(0.20, 0.25, 0.40, "5b3fb8"), (0.85, 0.30, 0.35, "b14fa8"),
                (0.60, 0.95, 0.40, "f08a5d"), (0.05, 0.90, 0.35, "2f3f9e")]),
    ("e7efd9", [(0.15, 0.15, 0.40, "c9dcb0"), (0.85, 0.25, 0.35, "f4e3a1"),
                (0.65, 0.90, 0.45, "8fb38a"), (0.10, 0.85, 0.30, "f1f4e4")]),
    ("0f1d33", [(0.20, 0.80, 0.45, "0e6b5c"), (0.70, 0.20, 0.40, "1c4f8f"),
                (0.90, 0.85, 0.35, "2bb3a8"), (0.10, 0.10, 0.30, "1a2a55")]),
    ("fde6e4", [(0.10, 0.20, 0.40, "ffb3b8"), (0.80, 0.10, 0.35, "ffd9a8"),
                (0.75, 0.85, 0.45, "ff8f8a"), (0.15, 0.90, 0.35, "fff1c2")]),
    ("1b1d22", [(0.80, 0.20, 0.45, "39407a"), (0.15, 0.85, 0.40, "2a3550"),
                (0.55, 0.60, 0.30, "54457f"), (0.05, 0.05, 0.30, "24262d")]),
]


def rgb(hex_colour):
    return tuple(int(hex_colour[i:i + 2], 16) / 255 for i in (0, 2, 4))


def mesh(base, blobs, gw, gh):
    base = rgb(base)
    blobs = [(x, y, r, rgb(c)) for x, y, r, c in blobs]
    data = bytearray()
    for j in range(gh):
        for i in range(gw):
            u, v = i / (gw - 1), j / (gh - 1)
            # A gentle swirl keeps the blends from reading as concentric circles.
            su = u + 0.05 * math.sin(v * 3.1 + 0.7)
            sv = v + 0.05 * math.sin(u * 2.7 + 1.9)
            total, acc = 0.35, [c * 0.35 for c in base]
            for x, y, r, c in blobs:
                d2 = ((su - x) * 16 / 9) ** 2 + (sv - y) ** 2
                w = math.exp(-d2 / (2 * r * r))
                total += w
                for k in range(3):
                    acc[k] += c[k] * w
            data.extend(round(255 * a / total) for a in acc)
    return Image.frombytes("RGB", (gw, gh), bytes(data))


def grain(seed):
    rng = random.Random(seed)
    a = Image.frombytes("L", (W, H), rng.randbytes(W * H))
    b = Image.frombytes("L", (W, H), rng.randbytes(W * H))
    return ImageChops.add(a, b, scale=2).point(lambda n: 128 + round((n - 127.5) / 48))


def main():
    OUT.mkdir(parents=True, exist_ok=True)
    for n, (base, blobs) in enumerate(PALETTES, start=1):
        image = mesh(base, blobs, *GRID).resize((W, H), Image.Resampling.BICUBIC)
        image = image.filter(ImageFilter.GaussianBlur(6))
        noise = grain(n)
        image = Image.merge(
            "RGB", [ImageChops.add(c, noise, offset=-128) for c in image.split()]
        )
        path = OUT / f"wall-{n}.jpg"
        image.save(path, quality=QUALITY, optimize=True, progressive=True, subsampling=0)
        print(f"{path.name}: {W}x{H}, {path.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
