# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = ["numpy==2.2.6", "matplotlib==3.10.1"]
# ///
"""Render two matched external reference beds; never creates physical detail."""

import argparse
import json
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection
import numpy as np


def read(path):
    with Path(path).open("rb") as f:
        data = f.read(32 * 1024 * 1024 + 1)
    if len(data) > 32 * 1024 * 1024:
        raise ValueError("Reference plot input exceeds its byte bound.")
    r = json.loads(data)
    if r["reportVersion"] != "external-trimesh-review-1":
        raise ValueError("Unsupported external mesh report.")
    n = r["settings"]["cellsPerSide"]
    h = np.array(r["diagnosticGridBedMeters"]).reshape((n, n))
    if not (4 <= n <= 200) or not np.isfinite(h).all():
        raise ValueError("Invalid diagnostic bed.")
    return r, h


def run():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("left")
    p.add_argument("right")
    p.add_argument("output")
    p.add_argument("--own-range", action="store_true")
    a = p.parse_args()
    left, lh = read(a.left)
    right, rh = read(a.right)
    if left["identity"]["inputSha256"] != right["identity"]["inputSha256"]:
        raise ValueError("Comparison must use the same retained physical input.")
    settings = [
        "cellsPerSide",
        "spacingMeters",
        "steps",
        "years",
        "diffusivitySquareMetersPerYear",
        "coefficient",
        "exponent",
        "pressure",
    ]
    if any(left["settings"][k] != right["settings"][k] for k in settings):
        raise ValueError("Comparison process settings differ.")
    if Path(a.output).suffix not in [".png", ".svg"]:
        raise ValueError("Plot supports PNG or SVG only.")
    lo, hi = min(lh.min(), rh.min()), max(lh.max(), rh.max())
    fig, axes = plt.subplots(1, 2, figsize=(13, 7), facecolor="#10191f")
    fig.subplots_adjust(left=0.065, right=0.99, bottom=0.12, top=0.84, wspace=0.18)
    for ax, r, h in zip(axes, [left, right], [lh, rh]):
        n = r["settings"]["cellsPerSide"]
        dx = r["settings"]["spacingMeters"]
        lower, upper = (h.min(), h.max()) if a.own_range else (lo, hi)
        dy, gx = np.gradient(h, dx)
        light = 0.3 + 0.7 * np.maximum(
            0, (gx - dy + 1) / np.sqrt(3 * (gx * gx + dy * dy + 1))
        )
        value = (
            np.full_like(h, 0.5) if upper == lower else (h - lower) / (upper - lower)
        )
        image = (0.12 + 0.88 * value) * light
        half = (n - 1) * dx / 2000
        ax.imshow(
            image,
            origin="lower",
            extent=[-half, half, -half, half],
            cmap="gray",
            vmin=0,
            vmax=1,
            interpolation="nearest",
        )
        points = np.array(r["pointsMeters"]) / 1000
        receiver = np.array(r["actualDryReceivers"])
        area = np.array(r["actualDryContributingAreaSquareMeters"])
        ids = np.flatnonzero((area >= 5e6) & (receiver != np.arange(len(points))))
        ax.add_collection(
            LineCollection(
                np.stack((points[ids], points[receiver[ids]]), axis=1),
                colors="#46b8d7",
                linewidths=0.5,
                alpha=0.7,
            )
        )
        label = (
            "Poisson disk"
            if r["settings"].get("meshSampler") == "poisson"
            else f"Jitter={r['settings']['jitterFraction']}"
        )
        ax.set_title(
            f"{label} / actual external bed\nDiagnostic range {lower:.1f}–{upper:.1f} m",
            color="white",
        )
        ax.set_xlabel("Chart x (km)", color="white")
        ax.set_ylabel("Chart y (km)", color="white")
        ax.tick_params(colors="white")
    fig.suptitle(
        "Same initial input / external single-flow + MST + cotangent diffusion",
        color="white",
    )
    fig.supxlabel(
        "Linear diagnostic projection; slope lighting 1x. Cyan: actual dry area links, not discharge. "
        + ("Own height ranges." if a.own_range else "Common height range."),
        color="white",
        fontsize=9,
    )
    with Path(a.output).open("xb") as f:
        fig.savefig(
            f, format=Path(a.output).suffix[1:], dpi=150, facecolor=fig.get_facecolor()
        )
    plt.close(fig)


if __name__ == "__main__":
    run()
