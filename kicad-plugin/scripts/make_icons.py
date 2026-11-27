"""Draws the toolbar icons: two pads joined by a 45-degree track."""

from __future__ import annotations

import struct
import zlib
from pathlib import Path

OUT = Path(__file__).resolve().parents[1] / "copperroute" / "icons"


def png(size: int, pixels: list[list[tuple[int, int, int, int]]]) -> bytes:
    rows = b"".join(b"\0" + bytes(c for px in row for c in px) for row in pixels)

    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(rows, 9)) + chunk(b"IEND", b"")


def distance_to_segment(px, py, ax, ay, bx, by):
    dx, dy = bx - ax, by - ay
    t = max(0.0, min(1.0, ((px - ax) * dx + (py - ay) * dy) / (dx * dx + dy * dy)))
    return ((px - ax - t * dx) ** 2 + (py - ay - t * dy) ** 2) ** 0.5


def icon(size: int, copper: tuple[int, int, int]) -> bytes:
    s = size / 24.0
    path = [(5 * s, 18 * s), (5 * s, 12 * s), (11 * s, 6 * s), (19 * s, 6 * s)]
    pads = [(5 * s, 18 * s), (19 * s, 6 * s)]
    pixels = []
    for y in range(size):
        row = []
        for x in range(size):
            cx, cy = x + 0.5, y + 0.5
            track = min(distance_to_segment(cx, cy, *path[i], *path[i + 1]) for i in range(3))
            pad = min(max(abs(cx - px), abs(cy - py)) for px, py in pads)
            coverage = max(
                min(1.0, max(0.0, 1.25 * s - track + 0.5)),
                min(1.0, max(0.0, 3.0 * s - pad + 0.5)),
            )
            row.append((*copper, int(255 * coverage)))
        pixels.append(row)
    return png(size, pixels)


if __name__ == "__main__":
    OUT.mkdir(parents=True, exist_ok=True)
    for size in (24, 48):
        (OUT / f"route-{size}.png").write_bytes(icon(size, (184, 115, 51)))
        (OUT / f"route-dark-{size}.png").write_bytes(icon(size, (237, 207, 134)))
