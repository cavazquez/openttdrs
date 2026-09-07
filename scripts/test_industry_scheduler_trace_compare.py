#!/usr/bin/env python3
"""Regresiones del comparador OpenTTD ↔ openttdrs del scheduler industrial."""

from __future__ import annotations

import copy
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
COMPARE = ROOT / "scripts" / "compare_industry_scheduler_traces.py"


def sample(*, tick: int, date: int, kind: str) -> dict[str, object]:
    return {
        "kind": kind,
        "tick": tick,
        "calendar": {"date": date, "year": 1950, "month": 0},
        "economy": {"date": date, "year": 1950, "month": 0},
        "random_state": {"state_0": 1, "state_1": 2},
        "industry_daily_change_counter": 17,
        "industry_daily_increment": 2114,
        "wanted_inds": 0,
        "change_loop": 0,
        "builddata": [
            {
                "type": index,
                "probability": 0,
                "min_number": 0,
                "target_count": 0,
                "max_wait": 1,
                "wait_count": 0,
            }
            for index in range(240)
        ],
        "industries": [],
        "actions": [],
    }


def trace(producer: str) -> list[dict[str, object]]:
    return [
        {
            "kind": "metadata",
            "schema_version": 1,
            "producer": producer,
            "trace": "industry_scheduler",
            "source_path": f"/{producer}.sav",
            "initial_sample_point": "after_load_game",
            "day_sample_point": "after_industry_daily_timer",
            "max_days": 1,
            "industry_type_count": 240,
        },
        sample(tick=100, date=42, kind="initial"),
        sample(tick=174, date=43, kind="day"),
    ]


class IndustrySchedulerTraceCompareTest(unittest.TestCase):
    def compare(
        self, native_rows: list[dict[str, object]], candidate_rows: list[dict[str, object]]
    ) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            native = root / "openttd.jsonl"
            candidate = root / "openttdrs.jsonl"
            native.write_text("\n".join(json.dumps(row) for row in native_rows) + "\n", encoding="utf-8")
            candidate.write_text(
                "\n".join(json.dumps(row) for row in candidate_rows) + "\n", encoding="utf-8"
            )
            return subprocess.run(
                [sys.executable, str(COMPARE), str(native), str(candidate), "1"],
                check=False,
                capture_output=True,
                text=True,
            )

    def test_accepts_equivalent_samples_from_distinct_producers(self) -> None:
        result = self.compare(trace("openttd"), trace("openttdrs"))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("scheduler industrial exacto", result.stdout)

    def test_reports_first_nested_difference(self) -> None:
        native = trace("openttd")
        candidate = copy.deepcopy(trace("openttdrs"))
        day = candidate[-1]
        assert isinstance(day, dict)
        random_state = day["random_state"]
        assert isinstance(random_state, dict)
        random_state["state_0"] = 99

        result = self.compare(native, candidate)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("day[1].random_state.state_0", result.stderr)

    def test_ignores_native_rng_attribution_diagnostic(self) -> None:
        native = trace("openttd")
        for row in native[1:]:
            assert isinstance(row, dict)
            row["random_calls"] = {"airporttiles.cpp:327": 6}

        result = self.compare(native, trace("openttdrs"))

        self.assertEqual(result.returncode, 0, result.stderr)

    def test_rejects_native_only_diagnostic_from_candidate(self) -> None:
        candidate = trace("openttdrs")
        initial = candidate[1]
        assert isinstance(initial, dict)
        initial["random_calls"] = {"airporttiles.cpp:327": 6}

        result = self.compare(trace("openttd"), candidate)

        self.assertNotEqual(result.returncode, 0)
        self.assertIn("initial[0]", result.stderr)


if __name__ == "__main__":
    unittest.main()
