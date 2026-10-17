# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = ["numpy==2.2.6", "matplotlib==3.10.1"]
# ///
"""Compare retained raw, post-construction and coupled-construction heights."""

import argparse
import hashlib
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
        raise ValueError("Plot input exceeds its byte bound.")
    return json.loads(data), hashlib.sha256(data).hexdigest()


def run():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("raw")
    p.add_argument("post")
    p.add_argument("coupled")
    p.add_argument("output")
    p.add_argument("--streams", action="store_true")
    args = p.parse_args()
    raw, raw_hash = read(args.raw)
    post, _ = read(args.post)
    coupled, _ = read(args.coupled)
    if raw["reportVersion"] != "external-trimesh-review-1":
        raise ValueError("Unsupported raw report.")
    for r, modes in [(post, ("post",)), (coupled, ("coupled", "channels"))]:
        if (
            r["reportVersion"] != "slope-envelope-review-1"
            or r["identity"]["sourceReportSha256"] != raw_hash
            or r["identity"]["meshSha256"] != raw["identity"]["meshSha256"]
            or r["settings"]["constructionMode"] not in modes
            or any(r["settings"][k] != v for k, v in raw["settings"].items())
            or r["pointsMeters"] != raw["pointsMeters"]
            or r["triangles"] != raw["triangles"]
        ):
            raise ValueError(
                "Plot reports do not use the same retained source/process."
            )
    if (
        post["settings"]["maximumGraphSlope"]
        != coupled["settings"]["maximumGraphSlope"]
    ):
        raise ValueError("Construction slope limits differ.")
    if Path(args.output).suffix not in (".png", ".svg") or Path(args.output).exists():
        raise ValueError("A new PNG/SVG output is required.")
    n = raw["settings"]["cellsPerSide"]
    dx = raw["settings"]["spacingMeters"]
    if not 4 <= n <= 200:
        raise ValueError("Unsupported plot dimensions.")
    points = np.array(raw["pointsMeters"]) / 1000
    half = (n - 1) * dx / 2000
    fig, axes = plt.subplots(1, 3, figsize=(16, 6), facecolor="#10191f")
    fig.subplots_adjust(left=0.045, right=0.99, bottom=0.17, top=0.8, wspace=0.14)
    for ax, r, title in zip(
        axes,
        [raw, post, coupled],
        [
            "Uncapped incision",
            "Envelope AFTER preparation",
            "Channel profiles + envelope"
            if coupled["settings"]["constructionMode"] == "channels"
            else "Envelope EACH stage",
        ],
        strict=True,
    ):
        h = np.array(r["diagnosticGridBedMeters"]).reshape(n, n)
        if not np.isfinite(h).all():
            raise ValueError("Nonfinite diagnostic bed.")
        gy, gx = np.gradient(h, dx)
        # Normal-based shading only: no per-panel height normalization, height
        # exaggeration, render noise or alterations to the physical field.
        light = 0.25 + 0.75 * np.maximum(
            0, (gx - gy + 1) / np.sqrt(3 * (gx * gx + gy * gy + 1))
        )
        ax.imshow(
            light,
            origin="lower",
            extent=[-half, half, -half, half],
            cmap="gray",
            vmin=0,
            vmax=1,
            interpolation="nearest",
        )
        if args.streams:
            receiver = np.array(r["actualDryReceivers"])
            area = np.array(r["actualDryContributingAreaSquareMeters"])
            ids = np.flatnonzero((area >= 1e6) & (receiver != np.arange(len(points))))
            ax.add_collection(
                LineCollection(
                    np.stack((points[ids], points[receiver[ids]]), axis=1),
                    colors="#46b8d7",
                    linewidths=0.4,
                    alpha=0.75,
                )
            )
        ax.set_title(f"{title}\nHeight {h.min():.0f}–{h.max():.0f} m", color="white")
        ax.set_xlabel("Chart x (km)", color="white")
        ax.set_ylabel("Chart y (km)", color="white")
        ax.tick_params(colors="white")
    fig.suptitle(
        "Same physical input and mesh / explicit lower graph-slope construction",
        color="white",
    )
    fig.supxlabel(
        "Linear diagnostic queries; same lighting, physical slopes 1x. "
        + (
            "Cyan: rebuilt actual dry collection >= 1 km², not discharge."
            if args.streams
            else "No stream overlay; accepted physical relief only."
        )
        + "\nGraph-edge slope bounds do NOT bound triangle gradients. Generation review, not a global world.",
        color="white",
        fontsize=10,
    )
    with Path(args.output).open("xb") as f:
        fig.savefig(
            f,
            format=Path(args.output).suffix[1:],
            dpi=150,
            facecolor=fig.get_facecolor(),
        )
    plt.close(fig)


if __name__ == "__main__":
    run()
