# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = ["fastscapelib==0.3.0", "numpy==2.2.6"]
# ///
"""Optional external numerical reference, not a production model or test dependency.

Published case: the public Fastscapelib mountain example's numerical setup.
Retained case: feed one six-column native patch record on stdin. Geographic
metrics become planar here; routes, lake treatment and diffusion integrator
differ. Agreement is not expected bit-for-bit and disagreement is not alone
evidence of a bug. Only JSON numerical results are emitted to stdout.

Reference: https://fastscapelib.readthedocs.io/en/latest/examples/mountain_py.html
No external implementation source is copied into this repository.
"""
import argparse
import hashlib
from importlib.metadata import version
import json
import math
import sys

import fastscapelib as fs
import numpy as np


def run():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="case", required=True)
    commands.add_parser("published")
    retained = commands.add_parser("retained")
    retained.add_argument("--cells", type=int, required=True)
    retained.add_argument("--spacing", type=float, required=True)
    retained.add_argument("--steps", type=int, default=64)
    retained.add_argument("--years", type=float, default=4_800_000.0)
    retained.add_argument("--diffusivity", type=float, default=1.0)
    retained.add_argument("--erodibility", type=float, default=1e-6)
    retained.add_argument("--area-exponent", type=float, default=0.5)
    args = parser.parse_args()
    if version("fastscapelib") != "0.3.0" or version("numpy") != "2.2.6":
        raise ValueError("This reference requires its exact declared package versions.")
    if args.case == "published":
        shape, spacing = (201, 301), 250.0
        initial = np.random.Generator(np.random.PCG64(1234)).uniform(0.0, 1.0, shape)
        uplift = np.full(shape, 1e-3)
        uplift[[0, -1], :] = 0.0
        uplift[:, [0, -1]] = 0.0
        steps, years, diffusion, coefficient, exponent = 50, 1_000_000.0, 0.01, 2e-4, 0.4
        identity = {"case": "published", "initialField": "PCG64 seed 1234 uniform [0,1) per node"}
    else:
        if not (4 <= args.cells <= 512 and 1 <= args.steps <= 256):
            raise ValueError("Retained reference dimensions or steps exceed the bound.")
        for value in (args.spacing, args.years, args.diffusivity, args.erodibility, args.area_exponent):
            if not math.isfinite(value):
                raise ValueError("Nonfinite reference setting.")
        if not (10 <= args.spacing <= 5000 and 0 < args.years <= 50_000_000
                and 0 <= args.diffusivity <= 1 and 0 <= args.erodibility <= 1e-3
                and 0 < args.area_exponent <= 1):
            raise ValueError("Reference setting exceeds its declared bound.")
        shape, spacing = (args.cells, args.cells), args.spacing
        expected = args.cells * args.cells * 48
        data = sys.stdin.buffer.read(expected + 1)
        if len(data) != expected:
            raise ValueError("Expected exactly one six-column headerless f64-le patch record.")
        columns = np.frombuffer(data, dtype="<f8").reshape((*shape, 6))
        if not np.isfinite(columns).all():
            raise ValueError("Nonfinite physical record.")
        initial, uplift = columns[:, :, 0].copy(), columns[:, :, 1].copy()
        if ((np.abs(initial) > 100_000.0).any() or (uplift < 0).any() or (uplift > 0.1).any()
                or uplift[[0, -1], :].any() or uplift[:, [0, -1]].any()):
            raise ValueError("External reference requires |bed| <= 100 km, uplift 0..0.1 m/year and a fixed perimeter.")
        steps, years = args.steps, args.years
        diffusion, coefficient, exponent = args.diffusivity, args.erodibility, args.area_exponent
        identity = {"case": "retained", "inputSha256": hashlib.sha256(data).hexdigest()}
    grid = fs.RasterGrid.from_length(
        shape, [(shape[0] - 1) * spacing, (shape[1] - 1) * spacing], fs.NodeStatus.FIXED_VALUE
    )
    graph = fs.FlowGraph(grid, [fs.SingleFlowRouter(), fs.MSTSinkResolver()])
    river = fs.SPLEroder(graph, k_coef=coefficient, area_exp=exponent, slope_exp=1, tolerance=1e-5)
    hillslope = fs.DiffusionADIEroder(grid, diffusion)
    height = initial.copy()
    dt = years / steps
    for _ in range(steps):
        uplifted = height + dt * uplift
        graph.update_routes(uplifted)
        area = graph.accumulate(1.0)
        channel_loss = river.erode(uplifted, area, dt)
        incised = uplifted - channel_loss
        height = incised - hillslope.erode(incised, dt)
        if not np.isfinite(height).all():
            raise ValueError("External reference produced a nonfinite bed.")
    if not (np.array_equal(height[[0, -1], :], initial[[0, -1], :])
            and np.array_equal(height[:, [0, -1]], initial[:, [0, -1]])):
        raise ValueError("External reference changed the fixed perimeter.")
    # Match the native 20/50 km profiles without sampling outside a smaller
    # physical record. A domain too small for 1 km cannot provide that metric.
    half_extent = min(20000.0, 0.4 * min(shape) * spacing, 0.5 * (min(shape) - 1) * spacing)
    query_spacing = 125.0 if half_extent < 20000.0 else 250.0
    count = int(half_extent / query_spacing)
    coordinates = np.arange(-count, count + 1) * query_spacing
    native_x = (np.arange(shape[1]) + 0.5 - shape[1] / 2) * spacing
    native_y = (np.arange(shape[0]) + 0.5 - shape[0] / 2) * spacing
    # Same central point-query diagnostic as the native observer, not coupling.
    horizontal = np.interp(coordinates, native_x,
        [np.interp(0.0, native_y, height[:, i]) for i in range(shape[1])])
    vertical = np.interp(coordinates, native_y,
        [np.interp(0.0, native_x, height[i, :]) for i in range(shape[0])])
    stride = round(1000.0 / query_spacing)
    differences = np.concatenate((horizontal[stride:] - horizontal[:-stride], vertical[stride:] - vertical[:-stride])) / 1000.0
    output = {
        "reportVersion": "external-fastscape-review-2",
        "referenceOnlyNotProduction": True,
        "identity": identity,
        "packages": {"fastscapelib": version("fastscapelib"), "numpy": version("numpy"), "python": sys.version},
        "settings": {"shape": shape, "spacingMeters": spacing, "steps": steps, "years": years,
            "diffusivitySquareMetersPerYear": diffusion, "erodibilityCoefficient": coefficient,
            "areaExponent": exponent, "slopeExponent": 1,
            "erodibilityUnits": f"meters^{1 - 2 * exponent:g}/year",
            "routing": "SingleFlowRouter + MSTSinkResolver", "diffusionIntegrator": "DiffusionADIEroder"},
        "metrics": {"unweightedHeightQuantilesMeters": np.quantile(height, [0.05, 0.5, 0.95]).tolist(),
            "maximumHeightMeters": float(height.max()),
            "transectRmsSlopeAt1km": float(np.sqrt(np.mean(differences**2))) if differences.size else None},
        "transectDiagnostics": {"chartCoordinatesMeters": coordinates.tolist(), "queryIntervalMeters": query_spacing,
            "scope": "Interior bilinear point queries; a missing 1 km metric is null, never fabricated by out-of-domain clamping."},
        "preparedBedMetersRowMajor": height.ravel().tolist(),
        "scope": "External planar reference; not spherical geometry, protected-potential-lake incision, material accounting, World state or production integration. K units depend on the area exponent. Published and retained setups are different controls, not a bitwise equivalence claim."
    }
    print(json.dumps(output, allow_nan=False, separators=(",", ":")))


if __name__ == "__main__":
    run()
