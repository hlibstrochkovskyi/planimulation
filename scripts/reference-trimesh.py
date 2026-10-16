# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = ["fastscapelib==0.3.0", "numpy==2.2.6", "scipy==1.15.3"]
# ///
"""Optional planar mesh comparison, NOT a production simulation dependency.

Read one native six-column f64-le patch on stdin. Both regular and jittered
meshes use the SAME external single-flow/MST eroder and independently assembled
cotangent diffusion. A jittered node samples the retained INITIAL field by
bilinear interpolation, not the final native bed. This is not an exact match to
native spherical facet routing, protected lakes or specific-area transport.
No external implementation source is copied into the repository.
"""

import argparse
import hashlib
from importlib.metadata import version
import json
import math
import sys
import time

import fastscapelib as fs
import numpy as np
from scipy.interpolate import LinearNDInterpolator, RegularGridInterpolator
from scipy.sparse import coo_matrix, diags
from scipy.sparse.linalg import factorized
from scipy.spatial import Delaunay
from scipy.stats import qmc


def make_mesh(cells, spacing, jitter, seed, sampler="jittered"):
    axis = (np.arange(cells) + 0.5 - cells / 2) * spacing
    x, y = np.meshgrid(axis, axis)
    points = np.column_stack((x.ravel(), y.ravel()))
    fixed = np.zeros((cells, cells), dtype=bool)
    fixed[[0, -1], :] = True
    fixed[:, [0, -1]] = True
    fixed = fixed.ravel()
    if sampler == "poisson":
        # Same exact perimeter, independent non-lattice interior. The point
        # count is measured, not asserted to equal the source raster count.
        low, high = axis[0] + 0.65 * spacing, axis[-1] - 0.65 * spacing
        radius = 0.8 * spacing / (high - low)
        interior = qmc.PoissonDisk(d=2, radius=radius, seed=seed).fill_space()
        points = np.concatenate((points[fixed], low + interior * (high - low)))
        fixed = np.arange(len(points)) < np.sum(fixed)
        if len(points) > 40000:
            raise ValueError("Poisson reference exceeds its 40000-node bound.")
    elif sampler == "jittered":
        rng = np.random.Generator(np.random.PCG64(seed))
        points[~fixed] += rng.uniform(-jitter, jitter, (sum(~fixed), 2)) * spacing
    else:
        raise ValueError("Unknown reference mesh sampler.")
    triangles = Delaunay(points).simplices
    grid = fs.TriMesh(
        points,
        triangles.astype(np.uint64),
        {int(i): fs.NodeStatus.FIXED_VALUE for i in np.flatnonzero(fixed)},
    )
    if not np.array_equal(grid.nodes_status() == int(fs.NodeStatus.FIXED_VALUE), fixed):
        raise ValueError("External mesh boundary does not match the retained collar.")
    areas = np.asarray(grid.nodes_areas()).copy()
    if not np.isfinite(areas).all() or (areas <= 0).any():
        raise ValueError("Invalid external nodal reference areas.")
    return points, triangles, fixed, areas, grid, axis


def stiffness(points, triangles, fixed):
    """Independent symmetric cotangent stiffness, not library source code.

    Each opposite angle contributes cot(theta)/2 to its edge. Delaunay interior
    edge sums are nonnegative; negative fixed/fixed boundary edges do not enter
    the active system. Do not clip a negative active coefficient into validity.
    """
    n = len(points)
    rows, cols, data = [], [], []
    for opposite, a, b in [(0, 1, 2), (1, 2, 0), (2, 0, 1)]:
        v = points[triangles[:, a]] - points[triangles[:, opposite]]
        w = points[triangles[:, b]] - points[triangles[:, opposite]]
        cross = np.abs(v[:, 0] * w[:, 1] - v[:, 1] * w[:, 0])
        if (cross <= 0).any() or not np.isfinite(cross).all():
            raise ValueError("Degenerate reference triangle.")
        c = np.sum(v * w, axis=1) / (2 * cross)
        ia, ib = triangles[:, a], triangles[:, b]
        rows.extend([ia, ib, ia, ib])
        cols.extend([ia, ib, ib, ia])
        data.extend([c, c, -c, -c])
    k = coo_matrix(
        (np.concatenate(data), (np.concatenate(rows), np.concatenate(cols))),
        shape=(n, n),
    ).tocsr()
    off = k.tocoo()
    if not np.isfinite(k.data).all() or np.any(
        (off.row != off.col) & (~fixed[off.row] | ~fixed[off.col]) & (off.data > 0)
    ):
        raise ValueError("Negative active cotangent conductance.")
    return k


class Diffusion:
    def __init__(self, k, areas, fixed, dt, diffusivity):
        self.active = np.flatnonzero(~fixed)
        self.boundary = np.flatnonzero(fixed)
        self.areas = areas
        self.k = k
        self.scale = dt * diffusivity
        self.matrix = (
            diags(areas[self.active]) + self.scale * k[self.active][:, self.active]
        ).tocsc()
        self.cross = self.scale * k[self.active][:, self.boundary]
        self.solve = factorized(self.matrix) if diffusivity else None
        self.maximum_relative_linear_residual = 0.0

    def advance(self, h):
        if self.solve is None:
            return h.copy()
        rhs = self.areas[self.active] * h[self.active] - self.cross @ h[self.boundary]
        new = h.copy()
        new[self.active] = self.solve(rhs)
        residual = self.matrix @ new[self.active] - rhs
        scale = np.max(np.abs(rhs))
        relative = (
            float(np.max(np.abs(residual)) / scale)
            if scale
            else float(np.max(np.abs(residual)))
        )
        if not math.isfinite(relative) or relative > 1e-10:
            raise ValueError(
                "External diffusion linear solve failed its residual guard."
            )
        self.maximum_relative_linear_residual = max(
            self.maximum_relative_linear_residual, relative
        )
        return new


def read_fields(data, cells):
    if len(data) != cells * cells * 48:
        raise ValueError(
            "Expected exactly one six-column headerless f64-le patch record."
        )
    fields = np.frombuffer(data, dtype="<f8").reshape((cells, cells, 6))
    if not np.isfinite(fields).all():
        raise ValueError("Nonfinite retained physical record.")
    bed, uplift = fields[:, :, 0], fields[:, :, 1]
    if (
        (np.abs(bed) > 100_000).any()
        or (uplift < 0).any()
        or (uplift > 0.1).any()
        or uplift[[0, -1], :].any()
        or uplift[:, [0, -1]].any()
    ):
        raise ValueError(
            "Retained reference requires finite bounded bed, nonnegative uplift and a fixed collar."
        )
    return bed, uplift


def orientation_order(displacements, fold=8):
    """Length-weighted directional order modulo pi; one on 45-degree axes.

    A measured stencil/direction diagnostic, not a terrain-realism score.
    """
    length = np.linalg.norm(displacements, axis=1)
    if not len(length) or not np.sum(length):
        return None
    angle = np.arctan2(displacements[:, 1], displacements[:, 0])
    return float(abs(np.sum(length * np.exp(1j * fold * angle))) / np.sum(length))


def simulate(
    data,
    cells,
    spacing,
    steps,
    years,
    diffusivity,
    coefficient,
    exponent,
    jitter,
    mesh_seed,
    pressure,
    sampler="jittered",
):
    bed, uplift = read_fields(data, cells)
    started = time.perf_counter()
    points, triangles, fixed, areas, grid, axis = make_mesh(
        cells, spacing, jitter, mesh_seed, sampler
    )
    # scipy interpolation coordinates are (row/y, column/x).
    query = points[:, ::-1]
    initial = RegularGridInterpolator((axis, axis), bed, bounds_error=True)(query)
    rates = RegularGridInterpolator((axis, axis), uplift, bounds_error=True)(query)
    if rates[fixed].any():
        raise ValueError("Interpolated uplift changed the fixed collar.")
    graph = fs.FlowGraph(grid, [fs.SingleFlowRouter(), fs.MSTSinkResolver()])
    # Keep the eroder's area argument in m². Specific pressure is represented
    # by an EXPLICIT width-dependent effective K_A, not mislabeled as area.
    widths = np.sqrt(areas)
    effective_k = coefficient if pressure == "total" else coefficient / widths**exponent
    eroder = fs.SPLEroder(
        graph, k_coef=effective_k, area_exp=exponent, slope_exp=1, tolerance=1e-5
    )
    k = stiffness(points, triangles, fixed)
    diffusion = Diffusion(k, areas, fixed, years / steps, diffusivity)
    h = initial.copy()
    unit_source = (~fixed).astype(float)
    limited = 0
    for _ in range(steps):
        raised = h + years / steps * rates
        graph.update_routes(raised)
        contributing = graph.accumulate(unit_source)
        loss = eroder.erode(raised, contributing, years / steps)
        h = diffusion.advance(raised - loss)
        limited += eroder.n_corr
        if not np.isfinite(h).all() or not np.array_equal(h[fixed], initial[fixed]):
            raise ValueError("Nonfinite external bed or changed fixed boundary.")
    elapsed = time.perf_counter() - started
    # Distinguish actual dry collection from MST-imposed connectivity.
    dry = fs.FlowGraph(grid, [fs.SingleFlowRouter()])
    dry.update_routes(h)
    dry_area = dry.accumulate(unit_source)
    impl = dry.impl()
    receivers = np.asarray(impl.receivers)[:, 0]
    terminals = receivers == np.arange(len(h))
    source_area = float(np.sum(areas[~fixed]))
    terminal_area = float(np.sum(dry_area[terminals]))
    residual = abs(terminal_area - source_area) / source_area
    if residual > 1e-10:
        raise ValueError(
            "External dry terminal area does not reconcile with active sources."
        )
    sample = LinearNDInterpolator(points, h)
    x, y = np.meshgrid(axis, axis)
    regular = sample(x, y)
    half = min(8000.0, 0.4 * cells * spacing, (cells - 1) * spacing / 2)
    coordinates = np.arange(-int(half / 125), int(half / 125) + 1) * 125.0
    horizontal = sample(coordinates, np.zeros_like(coordinates))
    vertical = sample(np.zeros_like(coordinates), coordinates)
    differences = (
        np.concatenate((horizontal[8:] - horizontal[:-8], vertical[8:] - vertical[:-8]))
        / 1000
    )
    if (
        not np.isfinite(regular).all()
        or not np.isfinite(horizontal).all()
        or not np.isfinite(vertical).all()
    ):
        raise ValueError("Nonfinite diagnostic interpolation.")
    mesh_hash = hashlib.sha256(
        points.astype("<f8").tobytes() + triangles.astype("<u8").tobytes()
    ).hexdigest()
    edges = np.unique(
        np.sort(
            np.concatenate(
                [triangles[:, [0, 1]], triangles[:, [1, 2]], triangles[:, [2, 0]]]
            ),
            axis=1,
        ),
        axis=0,
    )
    edges = edges[~(fixed[edges[:, 0]] & fixed[edges[:, 1]])]
    links = np.flatnonzero((dry_area >= 5e6) & ~terminals)
    return {
        "reportVersion": "external-trimesh-review-1",
        "referenceOnlyNotProduction": True,
        "identity": {
            "inputSha256": hashlib.sha256(data).hexdigest(),
            "meshSha256": mesh_hash,
        },
        "packages": {
            **{p: version(p) for p in ("fastscapelib", "numpy", "scipy")},
            "python": sys.version,
        },
        "settings": {
            "cellsPerSide": cells,
            "spacingMeters": spacing,
            "steps": steps,
            "years": years,
            "diffusivitySquareMetersPerYear": diffusivity,
            "coefficient": coefficient,
            "exponent": exponent,
            "pressure": pressure,
            "coefficientUnits": f"meters^{1 - (2 if pressure == 'total' else 1) * exponent:g}/year",
            "specificWidthApproximation": "sqrt(external nodal area), not native derivative width"
            if pressure == "specific"
            else None,
            "meshSampler": sampler,
            "meshNodeCount": len(points),
            "jitterFraction": jitter if sampler == "jittered" else None,
            "poissonMinimumSpacingMeters": 0.8 * spacing
            if sampler == "poisson"
            else None,
            "meshSeed": mesh_seed,
            "meshRandom": "numpy-PCG64 / scipy-QMC-PoissonDisk"
            if sampler == "poisson"
            else "numpy-PCG64",
            "initialSampling": "bilinear retained INITIAL bed/uplift; unchanged perimeter, no final-bed input",
            "routing": "SingleFlowRouter + MSTSinkResolver",
            "diffusion": "symmetric cotangent / external nodal areas / backward Euler",
        },
        "metrics": {
            "diagnosticGridUnweightedHeightQuantilesMeters": np.quantile(
                regular, [0.05, 0.5, 0.95]
            ).tolist(),
            "maximumHeightMeters": float(h.max()),
            "closedDryFractionOfActiveArea": float(
                np.sum(dry_area[terminals & ~fixed]) / source_area
            ),
            "terminalAreaRelativeResidual": residual,
            "maximumRelativeDiffusionLinearResidual": diffusion.maximum_relative_linear_residual,
            "limitedNodeStages": limited,
            "transectRmsSlopeAt1km": float(np.sqrt(np.mean(differences**2)))
            if differences.size
            else None,
        },
        "directionDiagnostics": {
            "meshEdgeLengthWeightedEightfoldOrder": orientation_order(
                points[edges[:, 1]] - points[edges[:, 0]]
            ),
            "dryAreaLinkLengthWeightedEightfoldOrderAbove5km2": orientation_order(
                points[receivers[links]] - points[links]
            ),
            "scope": "R8=abs(sum(length*exp(8i*angle)))/sum(length). Axis/diagonal stencil order, not proof of geographic realism. Physical square boundaries also influence flow directions.",
        },
        "pointsMeters": points.tolist(),
        "triangles": triangles.tolist(),
        "fixedBoundary": fixed.tolist(),
        "referenceAreasSquareMeters": areas.tolist(),
        "initialBedMeters": initial.tolist(),
        "upliftMetersPerYear": rates.tolist(),
        "preparedBedMeters": h.tolist(),
        "actualDryReceivers": receivers.tolist(),
        "actualDryContributingAreaSquareMeters": dry_area.tolist(),
        "diagnosticGridBedMeters": regular.ravel().tolist(),
        "transects": {
            "chartCoordinatesMeters": coordinates.tolist(),
            "horizontalMeters": horizontal.tolist(),
            "verticalMeters": vertical.tolist(),
        },
        "elapsedSeconds": elapsed,
        "scope": "External planar reference only. Irregular sampling changes topology, nodal quadrature and sampled initial fields. MST connections assume routed depressions, not funded water. Specific width differs from the native model. Diagnostic interpolation does not create physical samples. No material/water ledger, World, checkpoint or desktop integration is claimed.",
    }


def run():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--cells", type=int, required=True)
    p.add_argument("--spacing", type=float, required=True)
    p.add_argument("--steps", type=int, default=128)
    p.add_argument("--years", type=float, default=4_800_000.0)
    p.add_argument("--diffusivity", type=float, default=1.0)
    p.add_argument("--coefficient", type=float, required=True)
    p.add_argument("--exponent", type=float, default=0.4)
    p.add_argument("--pressure", choices=["total", "specific"], default="total")
    p.add_argument("--jitter", type=float, default=0.35)
    p.add_argument("--mesh-seed", type=int, default=1234)
    p.add_argument("--mesh", choices=["jittered", "poisson"], default="jittered")
    a = p.parse_args()
    if not (4 <= a.cells <= 200 and 1 <= a.steps <= 256 and 0 <= a.mesh_seed < 2**32):
        raise ValueError("External mesh dimensions, steps or seed exceed the bound.")
    if not all(
        map(
            math.isfinite,
            [a.spacing, a.years, a.diffusivity, a.coefficient, a.exponent, a.jitter],
        )
    ) or not (
        10 <= a.spacing <= 5000
        and 0 < a.years <= 50_000_000
        and 0 <= a.diffusivity <= 1
        and 0 <= a.coefficient <= 0.01
        and 0 < a.exponent <= 1
        and 0 <= a.jitter <= 0.35
    ):
        raise ValueError("Invalid or unsupported external mesh setting.")
    if any(
        version(p) != v
        for p, v in [("fastscapelib", "0.3.0"), ("numpy", "2.2.6"), ("scipy", "1.15.3")]
    ):
        raise ValueError("This reference requires its declared package versions.")
    data = sys.stdin.buffer.read(a.cells * a.cells * 48 + 1)
    print(
        json.dumps(
            simulate(
                data,
                a.cells,
                a.spacing,
                a.steps,
                a.years,
                a.diffusivity,
                a.coefficient,
                a.exponent,
                a.jitter,
                a.mesh_seed,
                a.pressure,
                a.mesh,
            ),
            allow_nan=False,
            separators=(",", ":"),
        )
    )


if __name__ == "__main__":
    run()
