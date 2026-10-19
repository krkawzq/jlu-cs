# /// script
# requires-python = ">=3.10"
# dependencies = ["matplotlib>=3.8"]
# ///
"""Draw the Prim logo and the `.pr` file icons.

The mark is a "P" assembled from Prim's closure model rather than drawn as a
letter:

- The ring is the environment. A closure is an enclosure; the ring's counter
  is the space where bindings live.
- The diamond inside the ring is a value held in a slot. Slot and value are
  distinct, so the value sits inside the ring without touching it.
- The stem is the code entry, cut like a terminal `/` and set apart from the
  ring by a gap because code is the optional part of a closure.

Ring alone reads as an object (environment only); ring plus stem reads as a
function (environment with code), and together they spell the language name.

Usage (from the repository root):

    uv run scripts/fig_logo.py

Output goes to the repository's `icons/`; the language-server build script
regenerates it and copies the icons into the extension.
"""

from __future__ import annotations

import argparse
import math
from dataclasses import dataclass
from pathlib import Path
from typing import NamedTuple

import matplotlib

matplotlib.use("Agg")
matplotlib.rcParams["svg.hashsalt"] = "prim"

import matplotlib.pyplot as plt
from matplotlib.patches import Annulus, FancyBboxPatch, Polygon

ROOT = Path(__file__).resolve().parent.parent
ICON_DIR = ROOT / "icons"


class Layout(NamedTuple):
    stem: list[tuple[float, float]]
    ring: tuple[float, float]
    view: tuple[float, float]


@dataclass(frozen=True)
class Geometry:
    """Shape parameters on a 100x100 canvas with y pointing down, like SVG."""

    top: float = 15.0
    bottom: float = 85.0
    stem_width: float = 13.5  # horizontal width of the slash
    stem_slant: float = 15.0  # horizontal run of the slash over its height
    ring_radius: float = 24.5
    ring_width: float = 11.5
    gap: float = 4.5  # perpendicular clearance between slash and ring
    slot_radius: float = 7.0
    tile_radius: float = 22.0
    view_margin: float = 6.0  # padding around the bare mark for file icons

    def layout(self) -> Layout:
        """Place the slash and ring so the gap is exact and the mark is centered."""
        height = self.bottom - self.top
        cos = height / math.hypot(height, self.stem_slant)
        cy = self.top + self.ring_radius
        # Right edge of the slash at the ring's center line, measured from x=0.
        edge = self.stem_width + self.stem_slant * (self.bottom - cy) / height
        cx = edge + (self.ring_radius + self.gap) / cos
        shift = 50.0 - (cx + self.ring_radius) / 2
        stem = [
            (shift, self.bottom),
            (shift + self.stem_width, self.bottom),
            (shift + self.stem_width + self.stem_slant, self.top),
            (shift + self.stem_slant, self.top),
        ]
        half = max(cx + self.ring_radius, height) / 2 + self.view_margin
        return Layout(stem, (cx + shift, cy), (50.0 - half, 50.0 + half))


@dataclass(frozen=True)
class Palette:
    env: str
    code: str
    tile: str | None = None


GEOMETRY = Geometry()
# Amber code on warm paper and ink, after amber-phosphor terminals.
LOGO = Palette(env="#F2EEE6", code="#FFB23F", tile="#15171D")
FILE_LIGHT = Palette(env="#24292F", code="#D97706")
FILE_DARK = Palette(env="#E6EDF3", code="#FFB547")


def draw(palette: Palette, geometry: Geometry = GEOMETRY, *, tile: bool = True):
    shape = geometry.layout()
    fig = plt.figure(figsize=(1, 1))
    fig.patch.set_alpha(0)
    ax = fig.add_axes((0, 0, 1, 1))
    lo, hi = (0.0, 100.0) if tile else shape.view
    ax.set_xlim(lo, hi)
    ax.set_ylim(hi, lo)
    ax.set_aspect("equal")
    ax.axis("off")

    if tile and palette.tile is not None:
        ax.add_patch(
            FancyBboxPatch(
                (0, 0), 100, 100,
                boxstyle=f"round,pad=0,rounding_size={geometry.tile_radius}",
                facecolor=palette.tile, edgecolor="none",
            )
        )

    cx, cy = shape.ring
    r = geometry.slot_radius
    ax.add_patch(
        Annulus((cx, cy), geometry.ring_radius, geometry.ring_width,
                facecolor=palette.env, edgecolor="none")
    )
    ax.add_patch(
        Polygon(shape.stem, closed=True,
                facecolor=palette.code, edgecolor="none")
    )
    ax.add_patch(
        Polygon([(cx, cy - r), (cx + r, cy), (cx, cy + r), (cx - r, cy)],
                closed=True, facecolor=palette.code, edgecolor="none")
    )
    return fig


def save(fig, path: Path, png_size: int | None = None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if png_size is None:
        fig.savefig(path, format="svg", transparent=True,
                    metadata={"Date": None, "Creator": None})
    else:
        fig.savefig(path, format="png", dpi=png_size, transparent=True)
    print(f"wrote {path}")


def main() -> None:
    parser = argparse.ArgumentParser(description="Draw the Prim logo and file icons.")
    parser.add_argument("--out", type=Path, default=ICON_DIR,
                        help="output directory (default: <repo>/icons)")
    parser.add_argument("--size", type=int, default=256,
                        help="PNG edge length in pixels")
    args = parser.parse_args()
    out = args.out.resolve()

    logo = draw(LOGO)
    save(logo, out / "logo.svg")
    save(logo, out / "logo.png", png_size=args.size)
    plt.close(logo)

    for name, palette in (("prim-light.svg", FILE_LIGHT), ("prim-dark.svg", FILE_DARK)):
        fig = draw(palette, tile=False)
        save(fig, out / name)
        plt.close(fig)


if __name__ == "__main__":
    main()
