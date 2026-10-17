# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = ["fastscapelib==0.3.0", "numpy==2.2.6", "scipy==1.15.3"]
# ///
"""Independent graph-envelope controls and actual coupled-bed accounting."""

from functools import lru_cache
import importlib.util
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import numpy as np

sys.dont_write_bytecode = True


def module(name, file):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(file))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


ref = module("slope_envelope", "reference-slope-envelope.py")
mesh = module("reference_trimesh", "reference-trimesh.py")


@lru_cache(maxsize=1)
def fixture():
    n, dx = 12, 100.0
    initial = np.zeros((n, n))
    uplift = np.full_like(initial, 0.0005)
    uplift[[0, -1], :] = 0
    uplift[:, [0, -1]] = 0
    data = (
        np.stack((initial, uplift, initial, initial, initial, initial), axis=2)
        .astype("<f8")
        .tobytes()
    )
    result = mesh.simulate(
        data, n, dx, 8, 1_000_000.0, 0.0, 5.61e-7, 0.5, 0.35, 1234, "total", "poisson"
    )
    return json.dumps(result, allow_nan=False).encode()


class EnvelopeChecks(unittest.TestCase):
    def test_native_mesh_matches_independent_construction_and_rebuilds_actual_flow(
        self,
    ):
        binary = os.environ.get("PLANIMULATION_LANDFORM_MESH_BINARY")
        if not binary:
            self.skipTest(
                "Set PLANIMULATION_LANDFORM_MESH_BINARY for the native cross-check."
            )
        data = fixture()
        expected = ref.prepare(data, "channels", math.tan(math.pi / 6))
        with tempfile.TemporaryDirectory(
            prefix="planimulation-native-landform-"
        ) as tmp:
            source = Path(tmp) / "source.json"
            source.write_bytes(data)
            output = Path(tmp) / "output"
            result = subprocess.run(
                [binary, str(source), str(output)], capture_output=True
            )
            self.assertEqual(result.returncode, 0, result.stderr.decode())
            actual = json.loads((output / "physical.json").read_bytes())
            constructed = actual["construction"]
            np.testing.assert_allclose(
                constructed["elevationMeters"],
                expected["preparedBedMeters"],
                atol=2e-12,
                rtol=1e-14,
            )
            np.testing.assert_array_equal(
                actual["actualDryReceivers"], expected["actualDryReceivers"]
            )
            np.testing.assert_allclose(
                actual["actualDryContributingAreaSquareMeters"],
                expected["actualDryContributingAreaSquareMeters"],
                atol=1e-9,
                rtol=1e-14,
            )
            self.assertAlmostEqual(
                constructed["detachedMaterialCubicMeters"],
                expected["materialBookkeeping"]["channelReprofileDetachedCubicMeters"]
                + expected["materialBookkeeping"]["envelopeDetachedCubicMeters"],
                delta=1e-4,
            )
            self.assertEqual(
                constructed["modelVersion"], "drainage-landform-construction-1"
            )
            digest = 0xCBF29CE484222325
            for byte in data:
                digest = ((digest ^ byte) * 0x100000001B3) & ((1 << 64) - 1)
            self.assertEqual(
                actual["inputIdentity"]["sourceReportFnv1a64"], f"{digest:016x}"
            )
            retained = (output / "physical.json").read_bytes()
            result = subprocess.run(
                [binary, str(source), str(output)], capture_output=True
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual((output / "physical.json").read_bytes(), retained)
            for i, change in enumerate(
                [
                    lambda r: r.update(reportVersion="future"),
                    lambda r: r["referenceAreasSquareMeters"].__setitem__(0, 0),
                    lambda r: r["triangles"].append(r["triangles"][0]),
                    lambda r: r["triangles"][0].__setitem__(0, 1000000),
                    lambda r: r.update(fixedBoundary=[False] * len(r["fixedBoundary"])),
                    lambda r: r["pointsMeters"][0].__setitem__(0, math.nan),
                    lambda r: r["preparedBedMeters"].pop(),
                ]
            ):
                malformed = json.loads(data)
                change(malformed)
                source.write_bytes(json.dumps(malformed).encode())
                destination = Path(tmp) / f"invalid-{i}"
                result = subprocess.run(
                    [binary, str(source), str(destination)], capture_output=True
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(destination.exists())

    def test_known_chain_unequal_lengths_and_largest_feasible_minorant(self):
        h = np.array([0.0, 10.0, 5.0, 10.0])
        edges = np.array([[0, 1], [1, 2], [2, 3]])
        links = ref.adjacency(4, edges, np.array([1.0, 2.0, 4.0]), 0.5)
        fixed = np.array([True, False, False, False])
        expected = np.array([0.0, 0.5, 1.5, 3.5])
        original = h.copy()
        got = ref.lower_envelope(h, links, fixed)
        np.testing.assert_array_equal(got, expected)
        np.testing.assert_array_equal(h, original)
        np.testing.assert_array_equal(ref.lower_envelope(got, links, fixed), got)

    def test_independent_all_pairs_oracle_multiple_seeds_and_edge_order(self):
        rng = np.random.Generator(np.random.PCG64(17))
        n = 11
        edges = np.array([(i, j) for i in range(n) for j in range(i + 1, n)])
        lengths = rng.uniform(1, 10, len(edges))
        distance = np.full((n, n), np.inf)
        np.fill_diagonal(distance, 0)
        distance[edges[:, 0], edges[:, 1]] = lengths
        distance[edges[:, 1], edges[:, 0]] = lengths
        # Floyd-Warshall, independent of the production heap/adjacency traversal.
        for k in range(n):
            distance = np.minimum(distance, distance[:, k, None] + distance[None, k, :])
        for _ in range(5):
            h = rng.uniform(-10, 30, n)
            expected = np.min(h[None, :] + 0.5 * distance, axis=1)
            fixed = np.zeros(n, dtype=bool)
            links = ref.adjacency(n, edges, lengths, 0.5)
            got = ref.lower_envelope(h, links, fixed)
            np.testing.assert_allclose(got, expected, rtol=0, atol=8e-15)
            reverse = ref.adjacency(n, edges[::-1], lengths[::-1], 0.5)
            np.testing.assert_array_equal(ref.lower_envelope(h, reverse, fixed), got)
            self.assertTrue(np.all(got <= h))

    def test_affine_constant_disconnected_and_incompatible_boundary(self):
        links = ref.adjacency(4, np.array([[0, 1], [1, 2]]), np.array([2.0, 2.0]), 0.5)
        fixed = np.array([True, False, False, False])
        for h in [np.array([-1.0, 0.0, 1.0, 20.0]), np.full(4, 12.0)]:
            np.testing.assert_array_equal(ref.lower_envelope(h, links, fixed), h)
        bad = np.array([100.0, 0.0, 100.0, 20.0])
        before = bad.copy()
        with self.assertRaisesRegex(ValueError, "incompatible"):
            ref.lower_envelope(bad, links, fixed)
        np.testing.assert_array_equal(bad, before)
        for slope in [0, -1, math.nan, math.inf, 2.1]:
            with self.assertRaises(ValueError):
                ref.adjacency(4, np.array([[0, 1]]), np.array([2.0]), slope)

    def test_edge_slope_bound_is_not_a_triangle_gradient_bound(self):
        p = np.array([[0.0, 0.0], [1.0, 0.0], [0.0, 1.0]])
        t = np.array([[0, 1, 2]])
        # Every edge slope <= 1; continuous triangle gradient is sqrt(2).
        h = np.array([0.0, 1.0, 1.0])
        edges, lengths = ref.graph_edges(p, t)
        result = ref.lower_envelope(
            h, ref.adjacency(3, edges, lengths, 1), np.zeros(3, bool)
        )
        np.testing.assert_array_equal(result, h)
        slope, area = ref.triangle_slopes(p, t, h)
        self.assertAlmostEqual(slope[0], math.sqrt(2))
        self.assertEqual(area[0], 0.5)

    def test_coupled_and_post_actual_beds_repeat_and_material_debits_reconcile(self):
        data = fixture()
        for mode in ["post", "coupled", "channels"]:
            a, b = ref.prepare(data, mode, 0.5), ref.prepare(data, mode, 0.5)
            del a["elapsedSeconds"], b["elapsedSeconds"]
            self.assertEqual(a, b)
            self.assertTrue(a["referenceOnlyNotProduction"])
            self.assertEqual(a["settings"]["constructionMode"], mode)
            self.assertLess(a["metrics"]["terminalAreaRelativeResidual"], 1e-12)
            self.assertLessEqual(
                a["metrics"]["maximumGraphEdgeSlope"], 0.5 * (1 + 1e-12)
            )
            h = np.array(a["preparedBedMeters"])
            fixed = np.array(a["fixedBoundary"])
            areas = np.array(a["referenceAreasSquareMeters"])
            start = np.array(
                a["initialBedMeters"]
                if mode == "coupled"
                else a["sourceUncappedBedMeters"]
            )
            self.assertEqual(
                len(a["materialBookkeeping"]["history"]), 8 if mode == "coupled" else 1
            )
            np.testing.assert_array_equal(h[fixed], start[fixed])
            history = a["materialBookkeeping"]["history"]
            independent_change = math.fsum(((h - start) * areas).tolist())
            raised = math.fsum(s["acceptedUpliftPrismCubicMeters"] for s in history)
            incised = a["materialBookkeeping"]["incisionDetachedCubicMeters"]
            envelope = a["materialBookkeeping"]["envelopeDetachedCubicMeters"]
            channel = a["materialBookkeeping"]["channelReprofileDetachedCubicMeters"]
            self.assertGreater(envelope, 0)
            self.assertAlmostEqual(
                independent_change, raised - incised - channel - envelope, delta=1e-5
            )
            self.assertLess(max(s["relativeMaterialResidual"] for s in history), 1e-12)
            receiver = np.array(a["actualDryReceivers"])
            self.assertTrue(np.all(h[receiver] <= h))

    def test_channel_concavity_has_independent_known_profile_and_preserves_terminal(
        self,
    ):
        bed = np.array([1.0, 10.0, 20.0, 30.0])
        points = np.array([[0.0, 0.0], [4.0, 0.0], [8.0, 0.0], [12.0, 0.0]])
        result, selected = ref.channel_constraints(
            bed,
            points,
            np.array([0, 0, 1, 2]),
            np.array([160000.0, 160000.0, 40000.0, 10000.0]),
            10000.0,
            0.5,
            0.5,
        )
        np.testing.assert_array_equal(result, [1.0, 1.5, 2.5, 4.5])
        self.assertTrue(selected.all())
        np.testing.assert_array_equal(bed, [1.0, 10.0, 20.0, 30.0])
        with self.assertRaisesRegex(ValueError, "Cyclic"):
            ref.channel_constraints(
                bed,
                points,
                np.array([1, 0, 2, 3]),
                np.full(4, 10000.0),
                10000.0,
                0.5,
                0.5,
            )
        with self.assertRaisesRegex(ValueError, "closed"):
            ref.channel_constraints(
                bed,
                points,
                np.array([0, 0, 1, 2]),
                np.array([10000.0, 10000.0, 100.0, 10000.0]),
                10000.0,
                0.5,
                0.5,
            )

    def test_source_refusals_do_not_mutate_or_accept_mismatched_geometry(self):
        data = fixture()
        base = json.loads(data)
        changes = [
            lambda r: r.update(reportVersion="future"),
            lambda r: r["settings"].update(steps=0),
            lambda r: r["settings"].update(diffusivitySquareMetersPerYear=1),
            lambda r: r["settings"].update(years=math.nan),
            lambda r: r["referenceAreasSquareMeters"].__setitem__(0, 1.0),
            lambda r: r["triangles"][0].__setitem__(0, 0.5),
            lambda r: r["preparedBedMeters"].__setitem__(0, math.nan),
            lambda r: r["pointsMeters"][0].__setitem__(0, 1.0),
            lambda r: r["upliftMetersPerYear"].__setitem__(0, 0.0001),
        ]
        for change in changes:
            r = json.loads(data)
            change(r)
            with self.assertRaises(ValueError):
                ref.prepare(json.dumps(r).encode(), "post", 0.5)
        self.assertEqual(json.loads(data), base)

    def test_cli_refuses_existing_output_and_creates_no_file_on_invalid_settings(self):
        with tempfile.TemporaryDirectory(prefix="planimulation-envelope-test-") as tmp:
            source = Path(tmp) / "source.json"
            source.write_bytes(fixture())
            output = Path(tmp) / "result.json"
            output.write_bytes(b"preserve owner output\n")
            cmd = [
                sys.executable,
                str(Path(__file__).with_name("reference-slope-envelope.py")),
                str(source),
                str(output),
                "--mode",
                "post",
            ]
            result = subprocess.run(cmd, capture_output=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(output.read_bytes(), b"preserve owner output\n")
            new = Path(tmp) / "invalid.json"
            cmd[3] = str(new)
            for slope in ["nan", "0", "-1"]:
                result = subprocess.run(
                    cmd + ["--maximum-graph-slope", slope], capture_output=True
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(new.exists())


if __name__ == "__main__":
    unittest.main()
