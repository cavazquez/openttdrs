"""Fail-closed regression checks for all-zoom renderer comparisons."""

import unittest

from compare_renderer_upgrade import apply_budget, metrics
from window_visual_regression import PngImage


class RendererGateTests(unittest.TestCase):
    @staticmethod
    def frame(*values):
        return PngImage(len(values), 1, bytes(channel for value in values for channel in (value, value, value, 255)))

    def test_improvements_cannot_hide_lost_correct_pixels(self):
        result = metrics(self.frame(0, 20), self.frame(20, 0), self.frame(0, 0))
        self.assertEqual(result["native_before"], result["native_after"])
        self.assertEqual(result["lost_correct_pixels"], 1)
        self.assertFalse(result["passed"])

    def test_identical_or_strictly_improved_frames_pass(self):
        self.assertTrue(metrics(self.frame(0, 20), self.frame(0, 20), self.frame(0, 0))["passed"])
        self.assertTrue(metrics(self.frame(0, 20), self.frame(0, 0), self.frame(0, 0))["passed"])

    def test_dimension_mismatch_is_rejected(self):
        with self.assertRaises(ValueError):
            metrics(self.frame(0), self.frame(0, 0), self.frame(0))

    def test_already_wrong_pixels_that_get_worse_are_counted(self):
        result = metrics(self.frame(10, 10, 10), self.frame(20, 5, 10), self.frame(0, 0, 0))
        self.assertEqual(result["lost_correct_pixels"], 0)
        self.assertEqual(result["worsened_pixels"], 1)
        self.assertEqual(result["improved_pixels"], 1)

    def test_exclusive_budget_does_not_replace_the_strict_result(self):
        for count, accepted in ((999, True), (1000, False)):
            report = {"passed": False, "zooms": [{"worsened_pixels": count}]}
            apply_budget(report, "worsened_pixels", 1000)
            self.assertFalse(report["passed"])
            self.assertEqual(report["budget_passed"], accepted)

    def test_budget_can_count_improvements_when_explicitly_requested(self):
        report = {"passed": True, "zooms": [{"changed_pixels": 2800, "worsened_pixels": 0}]}
        self.assertFalse(apply_budget(report, "changed_pixels", 1000)["budget_passed"])
        self.assertTrue(apply_budget(report, "worsened_pixels", 1000)["budget_passed"])


if __name__ == "__main__":
    unittest.main()
