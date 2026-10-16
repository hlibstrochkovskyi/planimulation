# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = ["fastscapelib==0.3.0", "numpy==2.2.6", "scipy==1.15.3"]
# ///
"""Optional external mesh controls; no production dependency changes."""

import importlib.util
import math
from pathlib import Path
import subprocess
import sys
import unittest

import numpy as np

sys.dont_write_bytecode = True

spec = importlib.util.spec_from_file_location(
    "reference_trimesh", Path(__file__).with_name("reference-trimesh.py")
)
ref = importlib.util.module_from_spec(spec)
spec.loader.exec_module(ref)


def fixture(n, spacing=500.0, forced=True):
    x, y = np.meshgrid(
        (np.arange(n) + 0.5 - n / 2) * spacing, (np.arange(n) + 0.5 - n / 2) * spacing
    )
    h = 10.0 + 0.001 * x + 0.002 * y
    fixed = np.zeros((n, n), dtype=bool)
    fixed[[0, -1], :] = True
    fixed[:, [0, -1]] = True
    u = np.where(fixed, 0.0, 0.0005 if forced else 0.0)
    return (
        np.stack(
            (h, u, h, np.zeros_like(h), np.zeros_like(h), np.zeros_like(h)), axis=2
        )
        .astype("<f8")
        .tobytes()
    )


class MeshChecks(unittest.TestCase):
    def test_orientation_order_has_independent_axis_and_balanced_direction_controls(
        self,
    ):
        self.assertIsNone(ref.orientation_order(np.empty((0, 2))))
        theta = np.arange(8) * np.pi / 4
        self.assertAlmostEqual(
            ref.orientation_order(np.column_stack((np.cos(theta), np.sin(theta)))), 1.0
        )
        theta = np.arange(16) * np.pi / 8
        self.assertLess(
            ref.orientation_order(np.column_stack((np.cos(theta), np.sin(theta)))),
            1e-15,
        )

    def test_known_triangle_operator_and_independent_implicit_link(self):
        p = np.array([[0.0, 0.0], [500.0, 0.0], [0.0, 500.0]])
        t = np.array([[0, 1, 2]])
        fixed = np.array([False, True, True])
        k = ref.stiffness(p, t, fixed)
        np.testing.assert_array_equal(
            k.toarray(), [[1.0, -0.5, -0.5], [-0.5, 0.5, 0.0], [-0.5, 0.0, 0.5]]
        )
        d = ref.Diffusion(k, np.array([100.0, 50.0, 50.0]), fixed, 2.0, 1.0)
        h = d.advance(np.array([100.0, 10.0, 20.0]))
        self.assertAlmostEqual(
            h[0], (100.0 * 100.0 + 2.0 * (0.5 * 10.0 + 0.5 * 20.0)) / (100.0 + 2.0)
        )
        np.testing.assert_array_equal(h[1:], [10.0, 20.0])

    def test_degenerate_and_negative_active_conductance_are_not_clipped(self):
        with self.assertRaisesRegex(ValueError, "Degenerate"):
            ref.stiffness(
                np.array([[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]]),
                np.array([[0, 1, 2]]),
                np.zeros(3, dtype=bool),
            )
        # Deliberately non-Delaunay: both opposite angles of edge 0--1 obtuse.
        p = np.array([[-1.0, 0.0], [1.0, 0.0], [0.0, 0.1], [0.0, -0.1]])
        with self.assertRaisesRegex(ValueError, "Negative active"):
            ref.stiffness(
                p, np.array([[0, 1, 2], [1, 0, 3]]), np.array([False, True, True, True])
            )

    def test_mesh_and_operator_repeat_and_keep_affine_harmonic_fields(self):
        for jitter in [0.0, 0.35]:
            p, t, fixed, a, g, axis = ref.make_mesh(12, 500.0, jitter, 1234)
            other = ref.make_mesh(12, 500.0, jitter, 1234)
            np.testing.assert_array_equal(p, other[0])
            np.testing.assert_array_equal(t, other[1])
            k = ref.stiffness(p, t, fixed)
            np.testing.assert_allclose(k.toarray(), k.T.toarray(), atol=1e-15, rtol=0.0)
            self.assertTrue(np.all(a > 0.0))
            self.assertAlmostEqual(np.sum(a), (11.0 * 500.0) ** 2, delta=1e-7)
            h = 1000.0 + 0.1 * p[:, 0] + 0.2 * p[:, 1]
            np.testing.assert_allclose((k @ h)[~fixed], 0.0, atol=2e-12, rtol=0.0)
            d = ref.Diffusion(k, a, fixed, 10000.0, 1.0)
            np.testing.assert_allclose(d.advance(h), h, atol=5e-12, rtol=0.0)
            flat = np.full(len(p), 10.0)
            np.testing.assert_allclose(
                d.advance(flat), flat, atol=64 * math.ulp(10.0), rtol=0.0
            )
            np.testing.assert_array_equal(
                ref.Diffusion(k, a, fixed, 10000.0, 0.0).advance(h), h
            )

    def test_coupled_reference_repeats_and_dry_area_reconciles(self):
        args = (
            fixture(12),
            12,
            500.0,
            4,
            100000.0,
            1.0,
            2e-4,
            0.4,
            0.35,
            1234,
            "total",
        )
        first, second = ref.simulate(*args), ref.simulate(*args)
        del first["elapsedSeconds"], second["elapsedSeconds"]
        self.assertEqual(first, second)
        self.assertLess(first["metrics"]["terminalAreaRelativeResidual"], 1e-12)
        self.assertEqual(first["settings"]["coefficientUnits"], "meters^0.2/year")
        self.assertTrue(first["referenceOnlyNotProduction"])
        fixed = np.array(first["fixedBoundary"])
        np.testing.assert_array_equal(
            np.array(first["preparedBedMeters"])[fixed],
            np.array(first["initialBedMeters"])[fixed],
        )

    def test_poisson_interior_is_separated_repeatable_and_keeps_the_same_perimeter(
        self,
    ):
        from scipy.spatial.distance import pdist

        p, t, fixed, areas, _, _ = ref.make_mesh(12, 500.0, 0.35, 1234, "poisson")
        other = ref.make_mesh(12, 500.0, 0.35, 1234, "poisson")
        np.testing.assert_array_equal(p, other[0])
        np.testing.assert_array_equal(t, other[1])
        regular = ref.make_mesh(12, 500.0, 0.0, 1234)
        np.testing.assert_array_equal(p[fixed], regular[0][regular[2]])
        self.assertGreaterEqual(np.min(pdist(p[~fixed])), 400.0 - 1e-10)
        self.assertAlmostEqual(np.sum(areas), 5500.0**2, delta=1e-7)
        result = ref.simulate(
            fixture(12),
            12,
            500.0,
            4,
            100000.0,
            1.0,
            2e-4,
            0.4,
            0.35,
            1234,
            "total",
            "poisson",
        )
        self.assertLess(result["metrics"]["terminalAreaRelativeResidual"], 1e-12)
        self.assertEqual(result["settings"]["meshSampler"], "poisson")

    def test_native_final_bed_is_not_reused_as_a_mesh_initial_condition(self):
        original = fixture(12, forced=False)
        changed = np.frombuffer(original, dtype="<f8").copy().reshape((12, 12, 6))
        changed[:, :, 2:] = 999.0
        a = ref.simulate(
            original, 12, 500.0, 1, 1.0, 0.0, 0.0, 0.4, 0.35, 1234, "specific"
        )
        b = ref.simulate(
            changed.tobytes(), 12, 500.0, 1, 1.0, 0.0, 0.0, 0.4, 0.35, 1234, "specific"
        )
        self.assertNotEqual(a["identity"]["inputSha256"], b["identity"]["inputSha256"])
        self.assertEqual(a["preparedBedMeters"], b["preparedBedMeters"])
        p = np.array(a["pointsMeters"])
        np.testing.assert_allclose(
            a["initialBedMeters"],
            10.0 + 0.001 * p[:, 0] + 0.002 * p[:, 1],
            atol=1e-14,
            rtol=0.0,
        )
        self.assertEqual(a["settings"]["coefficientUnits"], "meters^0.6/year")

    def test_affine_common_transects_have_an_independent_physical_slope(self):
        r = ref.simulate(
            fixture(40, forced=False),
            40,
            500.0,
            1,
            1.0,
            0.0,
            0.0,
            0.4,
            0.35,
            1234,
            "total",
        )
        self.assertEqual(r["transects"]["chartCoordinatesMeters"][0], -8000.0)
        self.assertEqual(r["transects"]["chartCoordinatesMeters"][-1], 8000.0)
        self.assertAlmostEqual(
            r["metrics"]["transectRmsSlopeAt1km"], math.sqrt(2.5e-6), places=14
        )
        small = ref.simulate(
            fixture(4, spacing=10.0, forced=False),
            4,
            10.0,
            1,
            1.0,
            0.0,
            0.0,
            0.4,
            0.0,
            1234,
            "total",
        )
        self.assertIsNone(small["metrics"]["transectRmsSlopeAt1km"])

    def test_invalid_cli_inputs_have_no_partial_result(self):
        default = ["--cells", "7", "--spacing", "500", "--coefficient", "0"]
        data = fixture(7, forced=False)
        cases = [
            (default + ["--cells", "201"], data),
            (default + ["--steps", "0"], data),
            (default + ["--jitter", "NaN"], data),
            (default + ["--jitter", ".36"], data),
            (default + ["--mesh-seed", "-1"], data),
            (default + ["--years", "inf"], data),
            (default, data[:-1]),
            (default, data + b"\0"),
        ]
        invalid = np.frombuffer(data, dtype="<f8").copy()
        invalid[0] = np.nan
        cases.append((default, invalid.tobytes()))
        invalid = np.frombuffer(data, dtype="<f8").copy()
        invalid[1] = 0.001
        cases.append((default, invalid.tobytes()))
        for args, input_data in cases:
            r = subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).with_name("reference-trimesh.py")),
                    *args,
                ],
                input=input_data,
                capture_output=True,
                timeout=30,
                check=False,
            )
            self.assertNotEqual(r.returncode, 0, r.stdout)
            self.assertEqual(r.stdout, b"")


if __name__ == "__main__":
    unittest.main()
