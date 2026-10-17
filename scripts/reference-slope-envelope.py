# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = ["fastscapelib==0.3.0", "numpy==2.2.6", "scipy==1.15.3"]
# ///
"""Bounded physical landform construction, not thermal erosion or deposition.

Compare a lower graph-Lipschitz envelope after preparation and after EVERY
uplift/incision stage on a retained external TriMesh. Every lowered reference
prism enters a detached bookkeeping reservoir. Graph-edge slope bounds do not
bound the piecewise-linear triangle gradient; both are measured separately.
"""

import argparse
import hashlib
import heapq
from importlib.metadata import version
import json
import math
from pathlib import Path
import sys
import time

import fastscapelib as fs
import numpy as np
from scipy.interpolate import LinearNDInterpolator

MODEL_VERSION = "lower-graph-slope-envelope-1"
REPORT_VERSION = "slope-envelope-review-1"


def graph_edges(points, triangles):
    edges = np.unique(
        np.sort(
            np.concatenate(
                [triangles[:, [0, 1]], triangles[:, [1, 2]], triangles[:, [2, 0]]]
            ),
            axis=1,
        ),
        axis=0,
    )
    lengths = np.linalg.norm(points[edges[:, 1]] - points[edges[:, 0]], axis=1)
    if not np.isfinite(lengths).all() or np.any(lengths <= 0):
        raise ValueError("Invalid graph edge lengths.")
    return edges, lengths


def adjacency(node_count, edges, lengths, maximum_slope):
    if not math.isfinite(maximum_slope) or not 0 < maximum_slope <= 2:
        raise ValueError("Invalid maximum graph slope.")
    result = [[] for _ in range(node_count)]
    for (i, j), length in zip(edges, lengths, strict=True):
        cost = float(maximum_slope * length)
        if not math.isfinite(cost) or cost <= 0:
            raise ValueError("Unrepresentable positive graph slope cost.")
        result[i].append((int(j), cost))
        result[j].append((int(i), cost))
    return result


def lower_envelope(bed, links, fixed):
    """Multi-source Dijkstra: min_j(bed[j] + slope * graph_distance(i,j)).

    This is a lower-only generation operator with no rate law. Fixed boundaries
    must already be compatible; an incompatible constraint REFUSES, not clamps.
    No inputs are mutated. Equal heap keys break ties by stable node index.
    """
    if (
        bed.ndim != 1
        or len(links) != len(bed)
        or fixed.shape != bed.shape
        or fixed.dtype != bool
        or not np.isfinite(bed).all()
    ):
        raise ValueError("Invalid envelope fields.")
    result = bed.copy()
    queue = [(float(h), i) for i, h in enumerate(bed)]
    heapq.heapify(queue)
    while queue:
        value, i = heapq.heappop(queue)
        if value != result[i]:
            continue
        for j, cost in links[i]:
            candidate = value + cost
            if candidate < result[j]:
                result[j] = candidate
                heapq.heappush(queue, (candidate, j))
    if not np.array_equal(result[fixed], bed[fixed]):
        raise ValueError("Graph slope envelope is incompatible with fixed boundary.")
    return result


def triangle_slopes(points, triangles, bed):
    a = points[triangles[:, 1]] - points[triangles[:, 0]]
    b = points[triangles[:, 2]] - points[triangles[:, 0]]
    det = a[:, 0] * b[:, 1] - a[:, 1] * b[:, 0]
    if np.any(det == 0) or not np.isfinite(det).all():
        raise ValueError("Degenerate physical triangle.")
    dh_a = bed[triangles[:, 1]] - bed[triangles[:, 0]]
    dh_b = bed[triangles[:, 2]] - bed[triangles[:, 0]]
    gx = (dh_a * b[:, 1] - dh_b * a[:, 1]) / det
    gy = (a[:, 0] * dh_b - b[:, 0] * dh_a) / det
    return np.hypot(gx, gy), np.abs(det) / 2


def channel_constraints(
    bed, points, receivers, collected, head_area, head_slope, exponent
):
    """Lower-only stream-profile construction, not integrated fluvial erosion.

    S = head_slope * (C/head_area)^(-exponent) gives an explicit concave
    longitudinal profile. Retain actual structural receivers and terminal
    heights. This generation choice is not a replay of the source's history.
    """
    if (
        not math.isfinite(head_area)
        or not 10000 <= head_area <= 100_000_000
        or not math.isfinite(head_slope)
        or not 0 < head_slope <= 1
    ):
        raise ValueError("Invalid channel construction parameters.")
    n = len(bed)
    donors = np.zeros(n, dtype=np.int64)
    for i, j in enumerate(receivers):
        if i != j:
            donors[j] += 1
    queue = [int(i) for i in np.flatnonzero(donors == 0)]
    order = []
    while queue:
        i = queue.pop()
        order.append(i)
        j = receivers[i]
        if i != j:
            donors[j] -= 1
            if donors[j] == 0:
                queue.append(int(j))
    if len(order) != n:
        raise ValueError("Cyclic structural channel graph.")
    result = bed.copy()
    channel = collected >= head_area
    for i in reversed(order):
        j = receivers[i]
        if i == j or not channel[i]:
            continue
        if not channel[j]:
            raise ValueError("Channel threshold is not downstream closed.")
        length = float(np.linalg.norm(points[i] - points[j]))
        grade = head_slope * (collected[i] / head_area) ** (-exponent)
        candidate = result[j] + grade * length
        if candidate < result[i]:
            result[i] = candidate
    return result, channel


def read_source(data):
    if len(data) > 32 * 1024 * 1024:
        raise ValueError("Source reference exceeds its byte bound.")
    r = json.loads(data)
    if r["reportVersion"] != "external-trimesh-review-1":
        raise ValueError("Unsupported source reference version.")
    s = r["settings"]
    n = s["meshNodeCount"]
    if (
        type(n) is not int
        or not 4 <= n <= 40000
        or type(s["cellsPerSide"]) is not int
        or not 4 <= s["cellsPerSide"] <= 200
        or type(s["steps"]) is not int
        or not 1 <= s["steps"] <= 256
        or s["diffusivitySquareMetersPerYear"] != 0
        or s["pressure"] != "total"
        or not all(
            math.isfinite(s[k])
            for k in ["spacingMeters", "years", "coefficient", "exponent"]
        )
        or not 10 <= s["spacingMeters"] <= 5000
        or not 0 < s["years"] <= 50_000_000
        or not 0 <= s["coefficient"] <= 0.01
        or not 0 < s["exponent"] <= 1
    ):
        raise ValueError("Unsupported source process settings or workload.")
    points = np.array(r["pointsMeters"], dtype=float)
    triangles = np.array(r["triangles"])
    fixed = np.array(r["fixedBoundary"])
    arrays = [
        np.array(r[k], dtype=float)
        for k in [
            "referenceAreasSquareMeters",
            "initialBedMeters",
            "upliftMetersPerYear",
            "preparedBedMeters",
        ]
    ]
    if (
        points.shape != (n, 2)
        or not np.isfinite(points).all()
        or triangles.ndim != 2
        or triangles.shape[1] != 3
        or not 1 <= len(triangles) <= 80000
        or triangles.dtype.kind not in "iu"
        or np.any(triangles < 0)
        or np.any(triangles >= n)
        or fixed.shape != (n,)
        or fixed.dtype != bool
        or not fixed.any()
        or fixed.all()
        or any(a.shape != (n,) or not np.isfinite(a).all() for a in arrays)
        or np.any(arrays[0] <= 0)
        or np.any(np.abs(arrays[1]) > 100000)
        or np.any(np.abs(arrays[3]) > 100000)
        or np.any(arrays[2] < 0)
        or np.any(arrays[2] > 0.1)
        or arrays[2][fixed].any()
        or not np.array_equal(arrays[1][fixed], arrays[3][fixed])
    ):
        raise ValueError("Invalid retained physical mesh fields.")
    digest = hashlib.sha256(
        points.astype("<f8").tobytes() + triangles.astype("<u8").tobytes()
    ).hexdigest()
    if digest != r["identity"]["meshSha256"]:
        raise ValueError("Retained mesh identity mismatch.")
    grid = fs.TriMesh(
        points,
        triangles.astype(np.uint64),
        {int(i): fs.NodeStatus.FIXED_VALUE for i in np.flatnonzero(fixed)},
    )
    if not np.array_equal(grid.nodes_areas(), arrays[0]) or not np.array_equal(
        grid.nodes_status() == int(fs.NodeStatus.FIXED_VALUE), fixed
    ):
        raise ValueError("Retained physical mesh area/boundary mismatch.")
    return r, points, triangles, fixed, arrays, grid


def prepare(data, mode, maximum_slope, head_area=250000.0, head_slope=0.1):
    r, points, triangles, fixed, arrays, grid = read_source(data)
    areas, initial, rates, raw = arrays
    if mode not in ("post", "coupled", "channels"):
        raise ValueError("Unknown construction mode.")
    if mode == "channels" and head_slope > maximum_slope:
        raise ValueError("Channel head slope exceeds the roof graph slope.")
    edges, lengths = graph_edges(points, triangles)
    links = adjacency(len(points), edges, lengths, maximum_slope)
    source = (~fixed).astype(float)
    settings = r["settings"]
    started = time.perf_counter()
    h = initial.copy() if mode == "coupled" else raw.copy()
    graph = fs.FlowGraph(grid, [fs.SingleFlowRouter(), fs.MSTSinkResolver()])
    eroder = fs.SPLEroder(
        graph,
        k_coef=settings["coefficient"],
        area_exp=settings["exponent"],
        slope_exp=1,
        tolerance=1e-5,
    )
    histories = []
    channel_mask = None
    step_years = settings["years"] / settings["steps"]
    for step in range(settings["steps"] if mode == "coupled" else 1):
        old = h.copy()
        if mode == "coupled":
            raised = h + step_years * rates
            graph.update_routes(raised)
            collected = graph.accumulate(source)
            incised = raised - eroder.erode(raised, collected, step_years)
        else:
            raised, incised = h, h
        constrained = incised
        if mode == "channels":
            structural = fs.FlowGraph(grid, [fs.SingleFlowRouter()])
            structural.update_routes(raw)
            structural_area = structural.accumulate(source)
            structural_receivers = np.asarray(structural.impl().receivers)[:, 0]
            constrained, channel_mask = channel_constraints(
                raw,
                points,
                structural_receivers,
                structural_area,
                head_area,
                head_slope,
                settings["exponent"],
            )
        h = lower_envelope(constrained, links, fixed)
        if not np.isfinite(h).all() or not np.array_equal(h[fixed], initial[fixed]):
            raise ValueError("Nonfinite bed or changed fixed boundary.")
        uplift = math.fsum(((raised - old) * areas).tolist())
        incision = math.fsum(((raised - incised) * areas).tolist())
        channel_detached = math.fsum(((incised - constrained) * areas).tolist())
        detached = math.fsum(((constrained - h) * areas).tolist())
        change = math.fsum(((h - old) * areas).tolist())
        scale = abs(uplift) + abs(incision) + abs(channel_detached) + abs(detached)
        residual = abs(change - (uplift - incision - channel_detached - detached))
        relative = residual / scale if scale else residual
        if (
            incision < 0
            or channel_detached < 0
            or detached < 0
            or not math.isfinite(relative)
            or relative > 1e-12
        ):
            raise ValueError("Accepted construction material does not reconcile.")
        histories.append(
            {
                "stage": step + 1,
                "acceptedUpliftPrismCubicMeters": uplift,
                "acceptedIncisionDetachedPrismCubicMeters": incision,
                "acceptedChannelReprofileDetachedPrismCubicMeters": channel_detached,
                "acceptedEnvelopeDetachedPrismCubicMeters": detached,
                "acceptedNetBedPrismChangeCubicMeters": change,
                "relativeMaterialResidual": relative,
                "changedEnvelopeNodes": int(np.sum(h != constrained)),
                "changedChannelProfileNodes": int(np.sum(constrained != incised)),
            }
        )
    dry = fs.FlowGraph(grid, [fs.SingleFlowRouter()])
    dry.update_routes(h)
    collected = dry.accumulate(source)
    receivers = np.asarray(dry.impl().receivers)[:, 0]
    terminal = receivers == np.arange(len(h))
    active_area = float(np.sum(areas[~fixed]))
    area_residual = abs(float(np.sum(collected[terminal])) - active_area) / active_area
    if area_residual > 1e-12:
        raise ValueError("Actual dry source areas do not reconcile.")
    structural_links = (
        channel_mask & (structural_receivers != np.arange(len(h)))
        if mode == "channels"
        else None
    )
    triangle_gradient, triangle_area = triangle_slopes(points, triangles, h)
    edge_slope = np.abs(h[edges[:, 1]] - h[edges[:, 0]]) / lengths
    if float(edge_slope.max()) > maximum_slope * (1 + 1e-10):
        raise ValueError("Accepted graph edge slopes exceed their bound.")
    axis = (
        np.arange(settings["cellsPerSide"]) + 0.5 - settings["cellsPerSide"] / 2
    ) * settings["spacingMeters"]
    x, y = np.meshgrid(axis, axis)
    query = LinearNDInterpolator(points, h)(x, y)
    if not np.isfinite(query).all():
        raise ValueError("Nonfinite diagnostic height query.")
    elapsed = time.perf_counter() - started
    return {
        "reportVersion": REPORT_VERSION,
        "referenceOnlyNotProduction": True,
        "identity": {
            **r["identity"],
            "sourceReportSha256": hashlib.sha256(data).hexdigest(),
        },
        "packages": {
            **{p: version(p) for p in ("fastscapelib", "numpy", "scipy")},
            "python": sys.version,
        },
        "settings": {
            **settings,
            "constructionModelVersion": MODEL_VERSION,
            "constructionMode": mode,
            "constructionStartingBed": "retained initial bed"
            if mode == "coupled"
            else "retained uncapped prepared bed",
            "constructionAdvanceYears": settings["years"]
            if mode == "coupled"
            else None,
            "maximumGraphSlope": maximum_slope,
            "channelProfile": {
                "modelVersion": "concave-channel-roof-construction-1",
                "headContributingAreaSquareMeters": head_area,
                "headSlope": head_slope,
                "concavityExponent": settings["exponent"],
                "structuralChannelNodes": int(np.sum(channel_mask)),
                "scope": "Lower constraints from source ACTUAL dry graph, then graph-slope roof. Recomputed output drainage is authoritative, not the structural graph. Not an erosion-time solution.",
            }
            if mode == "channels"
            else None,
            "boundaryAssumption": "fixed external collar; incompatible envelope refuses",
        },
        "metrics": {
            "maximumHeightMeters": float(h.max()),
            "maximumGraphEdgeSlope": float(edge_slope.max()),
            "maximumTriangleGradient": float(triangle_gradient.max()),
            "triangleAreaFractionAboveGraphSlope": float(
                np.sum(triangle_area[triangle_gradient > maximum_slope * (1 + 1e-10)])
                / np.sum(triangle_area)
            ),
            "diagnosticGridUnweightedHeightQuantilesMeters": np.quantile(
                query, [0.05, 0.5, 0.95]
            ).tolist(),
            "closedDryFractionOfActiveArea": float(
                np.sum(collected[terminal & ~fixed]) / active_area
            ),
            "terminalAreaRelativeResidual": area_residual,
            "fractionOfSelectedStructuralChannelReceiversChanged": float(
                np.mean(
                    receivers[structural_links]
                    != structural_receivers[structural_links]
                )
            )
            if mode == "channels" and np.any(structural_links)
            else None,
        },
        "materialBookkeeping": {
            "scope": "Accepted reference-prism debits to detached bookkeeping only. NOT thermal deposition, routed outlet export, material density or seasonal water.",
            "incisionDetachedCubicMeters": math.fsum(
                s["acceptedIncisionDetachedPrismCubicMeters"] for s in histories
            ),
            "envelopeDetachedCubicMeters": math.fsum(
                s["acceptedEnvelopeDetachedPrismCubicMeters"] for s in histories
            ),
            "channelReprofileDetachedCubicMeters": math.fsum(
                s["acceptedChannelReprofileDetachedPrismCubicMeters"] for s in histories
            ),
            "history": histories,
        },
        **{
            k: r[k]
            for k in [
                "pointsMeters",
                "triangles",
                "fixedBoundary",
                "referenceAreasSquareMeters",
                "initialBedMeters",
                "upliftMetersPerYear",
            ]
        },
        "sourceUncappedBedMeters": raw.tolist(),
        "preparedBedMeters": h.tolist(),
        "actualDryReceivers": receivers.tolist(),
        "actualDryContributingAreaSquareMeters": collected.tolist(),
        "diagnosticGridBedMeters": query.ravel().tolist(),
        "elapsedSeconds": elapsed,
        "scope": "Generation-only graph slope construction, no time-resolved hillslope physics. Edge limits do not bound triangle gradients. Physical dry drainage is rebuilt on accepted heights. MST preparation is not funded lake water. No global World, checkpoint, LOD, coupled sediment transport or desktop integration.",
    }


def run():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("source")
    p.add_argument("output")
    p.add_argument("--mode", choices=["post", "coupled", "channels"], required=True)
    p.add_argument("--maximum-graph-slope", type=float, default=math.tan(math.pi / 6))
    p.add_argument("--channel-head-area", type=float, default=250000.0)
    p.add_argument("--channel-head-slope", type=float, default=0.1)
    args = p.parse_args()
    if any(
        version(p) != v
        for p, v in [("fastscapelib", "0.3.0"), ("numpy", "2.2.6"), ("scipy", "1.15.3")]
    ):
        raise ValueError("This reference requires its declared package versions.")
    if Path(args.output).exists():
        raise ValueError("Refusing to replace an existing construction report.")
    with Path(args.source).open("rb") as f:
        data = f.read(32 * 1024 * 1024 + 1)
    result = prepare(
        data,
        args.mode,
        args.maximum_graph_slope,
        args.channel_head_area,
        args.channel_head_slope,
    )
    encoded = json.dumps(result, allow_nan=False, separators=(",", ":"))
    with Path(args.output).open("x", encoding="utf-8") as f:
        f.write(encoded)
        f.write("\n")
    print(json.dumps({"output": args.output, "metrics": result["metrics"]}))


if __name__ == "__main__":
    run()
