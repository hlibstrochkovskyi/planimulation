# /// script
# requires-python = ">=3.12,<3.13"
# dependencies = ["fastscapelib==0.3.0", "numpy==2.2.6"]
# ///
"""Optional reference-runner checks; not part of production dependencies."""
import json
import math
from pathlib import Path
import struct
import subprocess
import sys
import unittest


class ReferenceChecks(unittest.TestCase):
    arguments = ["retained", "--cells", "7", "--spacing", "250", "--steps", "1"]
    fixture = b"".join(struct.pack("<6d", 10.0, 0.0, 10.0, 0.0, 0.0, 0.0) for _ in range(49))

    def invoke(self, arguments, data=b""):
        return subprocess.run(
            [sys.executable, str(Path(__file__).with_name("reference-fastscape.py")), *arguments],
            input=data, capture_output=True, timeout=30, check=False,
        )

    def test_constant_retained_fixture_stays_within_adi_roundoff(self):
        result = self.invoke(self.arguments, self.fixture)
        self.assertEqual(result.returncode, 0, result.stderr)
        report = json.loads(result.stdout)
        # The external absolute-height ADI solve is not a bitwise flat no-op.
        # Check a declared ULP envelope; never round/correct emitted heights.
        tolerance = 64 * math.ulp(10.0)
        self.assertLessEqual(max(abs(h - 10.0) for h in report["preparedBedMetersRowMajor"]), tolerance)
        self.assertLessEqual(report["metrics"]["transectRmsSlopeAt1km"], tolerance / 1000.0)
        self.assertEqual(report["settings"]["erodibilityUnits"], "meters^0/year")
        self.assertTrue(report["referenceOnlyNotProduction"])

    def test_nine_invalid_controls_emit_no_result(self):
        cases = [
            (self.arguments + ["--years", "NaN"], self.fixture),
            (["retained", "--cells", "513", "--spacing", "250"], self.fixture),
            (self.arguments + ["--steps", "0"], self.fixture),
            (self.arguments, self.fixture[:-1]),
            (self.arguments, self.fixture + b"\0"),
        ]
        for offset, value in [(0, float("nan")), (8, float("nan")), (24 * 48, 100001.0), (8, 0.001)]:
            changed = bytearray(self.fixture)
            struct.pack_into("<d", changed, offset, value)
            cases.append((self.arguments, bytes(changed)))
        for index, (arguments, data) in enumerate(cases):
            with self.subTest(index=index):
                result = self.invoke(arguments, data)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, b"")

    def test_published_case_repeats_and_retains_numeric_control(self):
        a, b = self.invoke(["published"]), self.invoke(["published"])
        self.assertEqual(a.returncode, 0, a.stderr)
        self.assertEqual(b.returncode, 0, b.stderr)
        self.assertEqual(a.stdout, b.stdout)
        report = json.loads(a.stdout)
        self.assertEqual(len(report["preparedBedMetersRowMajor"]), 60501)
        self.assertEqual(report["settings"]["erodibilityUnits"], "meters^0.2/year")
        self.assertAlmostEqual(report["metrics"]["unweightedHeightQuantilesMeters"][2], 148.3891150343122, places=8)


if __name__ == "__main__":
    unittest.main()
