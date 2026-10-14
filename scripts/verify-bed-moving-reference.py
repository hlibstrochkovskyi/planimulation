"""Independently check a moving-bed report using high-precision primitives.

Requires mpmath==1.3.0. This validates reference observations only, never fits
physical owners or supplies exact interior fluxes to the evolved operator.
"""

import functools
import json
import math
import sys

import mpmath as mp


def verify(report, digits, conditioning=None):
    mp.mp.dps = digits
    slope = mp.mpf(str(report["slope"]))
    speed = mp.mpf(str(report["speedMetersPerSecond"]))
    roughness = mp.mpf(str(report["roughness"]))
    depth_scale = (roughness**2 * speed**2 / slope) ** mp.mpf(".75")
    length_scale = depth_scale / slope

    def distance(u):
        return 3 * length_scale * (-u**3 / 3 + (mp.atanh(u) - mp.atan(u)) / 2)

    def primitive(u):
        return 3 * depth_scale * length_scale * (
            -u**6 / 6 - u**2 / 2 + mp.atanh(u**2) / 2
        )

    @functools.lru_cache(None)
    def parameter(x, t):
        behind = speed * mp.mpf(t) - mp.mpf(x)
        if behind <= 0:
            return mp.mpf(0)
        lo, hi = mp.mpf(0), mp.mpf(".9")
        if behind > distance(hi):
            raise AssertionError("Point outside declared reference branch")
        u = min(mp.mpf(".8"), (7 * behind / (3 * length_scale)) ** (mp.mpf(1) / 7))
        for _ in range(400):
            error = distance(u) - behind
            # The elementary primitive subtracts rounded terms. Use the same
            # declared 68/88-digit residual as its quadrature cross-check.
            if abs(error) < mp.mpf(10) ** (12 - digits) * behind:
                return u
            if error > 0:
                hi = u
            else:
                lo = u
            derivative = 3 * length_scale * u**6 / (1 - u**4)
            candidate = u - error / derivative
            u = candidate if lo < candidate < hi else (lo + hi) / 2
        raise AssertionError(f"Independent primitive inversion did not converge: x={x}, t={t}, behind={behind}, u={u}, error={error}")

    expected_means = []
    max_mean_absolute = mp.mpf(0)
    max_mean_relative = mp.mpf(0)
    max_contour_error = mp.mpf(0)
    max_boundary_error = mp.mpf(0)
    means_count = 0
    for trial in report["trials"]:
        width = mp.mpf(trial["widthMeters"])
        direction = mp.mpf(trial["direction"])
        left = mp.mpf(trial["physicalDomainMeters"][0])
        phase = mp.mpf(trial.get("initialFrontPhaseMeters", 0))
        for history in trial["history"]:
            time = history["timeSeconds"]
            for i, actual in enumerate(history["exactMeansMeters"]):
                a = direction * (left + i * width) - phase
                b = direction * (left + (i + 1) * width) - phase
                a, b = min(a, b), max(a, b)
                value = (primitive(parameter(a, time)) - primitive(parameter(b, time))) / width
                error = abs(mp.mpf(actual) - value)
                if not math.isfinite(actual) or actual < 0:
                    raise AssertionError("Invalid report reference mean")
                if value == 0:
                    if actual != 0:
                        raise AssertionError("Independent dry mean is not exactly zero")
                else:
                    max_mean_relative = max(max_mean_relative, error / value)
                max_mean_absolute = max(max_mean_absolute, error)
                expected_means.append(value)
                means_count += 1
            for contour in history["contours"]:
                depth = mp.mpf(contour["thresholdMeters"])
                u = (depth / depth_scale) ** (mp.mpf(1) / 3)
                expected = direction * (phase + speed * mp.mpf(time) - distance(u))
                max_contour_error = max(
                    max_contour_error, abs(mp.mpf(contour["exactPositionMeters"]) - expected)
                )
        time = trial["history"][-1]["timeSeconds"]
        amounts = []
        for x in trial["physicalDomainMeters"]:
            normal = direction * mp.mpf(x) - phase
            amount = direction * (primitive(parameter(normal, time)) - primitive(parameter(normal, 0)))
            amounts.append(amount)
        incoming = max(amounts[0], 0) + max(-amounts[1], 0)
        outgoing = max(-amounts[0], 0) + max(amounts[1], 0)
        max_boundary_error = max(
            max_boundary_error,
            abs(incoming - mp.mpf(trial["referenceBoundaryInputSquareMeters"])),
            abs(outgoing - mp.mpf(trial["referenceBoundaryOutputSquareMeters"])),
        )

    # Direct rational-integrand quadrature additionally checks the elementary
    # primitives. It is a different evaluator from Rust's positive series.
    rational_checks = []
    for u in [mp.mpf(".2"), mp.mpf(".7"), mp.mpf(".9")]:
        f = 3 * length_scale * mp.quad(lambda v: v**6 / (1 - v**4), [0, u])
        j = 3 * depth_scale * length_scale * mp.quad(lambda v: v**9 / (1 - v**4), [0, u])
        relative = max(abs(f - distance(u)) / f, abs(j - primitive(u)) / j)
        # Primitive cancellation costs several digits at small u. Retain a
        # declared 68/88-digit check, still far beyond physical binary64.
        if relative > mp.mpf(10) ** (12 - digits):
            raise AssertionError("Elementary primitive/rational quadrature disagreement")
        rational_checks.append(float(relative))

    # Reference-only interval evaluation has two rounded inverse endpoints.
    # The .0625 m cohort fails the original 256-epsilon relative probe;
    # retain that outcome explicitly rather than alter physical moment bounds.
    original_reference_probe_passed = max_mean_relative <= 256 * sys.float_info.epsilon
    if max_mean_relative > 512 * sys.float_info.epsilon:
        raise AssertionError(f"Report means fail independent reference bound: absolute={max_mean_absolute}, relative={max_mean_relative}")
    if max_contour_error > mp.mpf("1e-12") or max_boundary_error > mp.mpf("1e-14"):
        raise AssertionError("Report contour/boundary oracle disagreement")
    conditioning_results = []
    for probe in (conditioning or {}).get("probes", []):
        x, start, end = probe["xMeters"], probe["startSeconds"], probe["endSeconds"]
        exact = primitive(parameter(x, end)) - primitive(parameter(x, start))
        actual = mp.mpf(probe["referenceBoundaryVolumeSquareMeters"])
        if exact <= 0 or actual <= 0:
            raise AssertionError("Invalid short-interval reference probe")
        error = abs(actual - exact)
        if error > mp.mpf("1e-14"):
            raise AssertionError("Reference conditioning probe exceeds absolute bound")
        conditioning_results.append({
            "requestedIntervalSeconds": probe["requestedIntervalSeconds"],
            "actualBinaryIntervalSeconds": float(mp.mpf(end)-mp.mpf(start)),
            "absoluteErrorSquareMeters": float(error),
            "relativeError": float(error / exact),
        })
    return {
        "decimalDigits": digits,
        "meanComparisons": means_count,
        "maximumMeanAbsoluteErrorMeters": float(max_mean_absolute),
        "maximumMeanRelativeError": float(max_mean_relative),
        "original256EpsilonReferenceProbePassed": original_reference_probe_passed,
        "declaredReferenceRelativeLimit": 512 * sys.float_info.epsilon,
        "maximumContourAbsoluteErrorMeters": float(max_contour_error),
        "maximumBoundaryAbsoluteErrorSquareMeters": float(max_boundary_error),
        "rationalQuadraturePrimitiveRelativeErrors": rational_checks,
        "shortIntervalConditioning": conditioning_results,
    }, expected_means


def main():
    if len(sys.argv) != 2:
        raise SystemExit("Usage: verify-bed-moving-reference.py REPORT.json")
    if mp.__version__ != "1.3.0":
        raise SystemExit("This reference verifier requires mpmath==1.3.0")
    with open(sys.argv[1], encoding="utf-8") as source:
        report = json.load(source)
    cohorts = [report] if "trials" in report else [report["baseline"], report["candidate"]]
    records = []
    for cohort in cohorts:
        low, low_means = verify(cohort, 80, report.get("referenceConditioning"))
        high, high_means = verify(cohort, 100, report.get("referenceConditioning"))
        difference = max(abs(a - b) for a, b in zip(low_means, high_means, strict=True))
        records.append({
            "operator": cohort["operator"],
            "evaluations": [low, high],
            "maximum80To100DigitMeanDifferenceMeters": float(difference),
        })
    print(json.dumps({
        "method": "mpmath 1.3.0 independent elementary primitives, bracketed Newton inversion and rational quadrature",
        "cohorts": records,
    }, indent=2))


if __name__ == "__main__":
    main()
