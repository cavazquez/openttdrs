#!/usr/bin/env python3
"""Contrato estático y de esquema para el oráculo del scheduler industrial."""

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
HEADER = ROOT / "patches" / "openttd-15.3-snapshot-export" / "src" / "snapshot_export.h"
SOURCE = ROOT / "patches" / "openttd-15.3-snapshot-export" / "src" / "snapshot_export.cpp"
INTEGRATOR = ROOT / "patches" / "openttd-15.3-snapshot-export" / "integrate.sh"
EXPORTER = ROOT / "scripts" / "export_openttd_industry_trace.sh"
VALIDATOR = ROOT / "scripts" / "validate_industry_trace.py"


def valid_trace(days: int = 1) -> list[dict[str, object]]:
    builddata = [
        {
            "type": index,
            "probability": 0,
            "min_number": 0,
            "target_count": 0,
            "max_wait": 1,
            "wait_count": 0,
        }
        for index in range(240)
    ]
    sample = {
        "tick": 100,
        "calendar": {"date": 42, "year": 1950, "month": 0},
        "economy": {"date": 42, "year": 1950, "month": 0},
        "random_state": {"state_0": 1, "state_1": 2},
        "industry_daily_change_counter": 17,
        "industry_daily_increment": 2114,
        "wanted_inds": 0,
        "change_loop": 0,
        "builddata": builddata,
        "industries": [],
        "actions": [],
    }
    rows: list[dict[str, object]] = [
        {
            "kind": "metadata",
            "schema_version": 1,
            "producer": "openttd",
            "trace": "industry_scheduler",
            "initial_sample_point": "after_load_game",
            "day_sample_point": "after_industry_daily_timer",
            "max_days": days,
            "industry_type_count": 240,
        },
        {"kind": "initial", **sample},
    ]
    for day in range(days):
        rows.append(
            {
                "kind": "day",
                **{
                    **sample,
                    "tick": 174 + day * 74,
                    "calendar": {"date": 43 + day, "year": 1950, "month": 0},
                    "economy": {"date": 43 + day, "year": 1950, "month": 0},
                },
            }
        )
    return rows


class IndustryTraceSourceContractTest(unittest.TestCase):
    def run_validator(self, rows: list[dict[str, object]], *, expect_ok: bool) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as temp:
            trace = Path(temp) / "industry.jsonl"
            trace.write_text("\n".join(json.dumps(row) for row in rows) + "\n", encoding="utf-8")
            result = subprocess.run(
                [sys.executable, str(VALIDATOR), str(trace), str(len(rows) - 2), "openttd"],
                check=False,
                capture_output=True,
                text=True,
            )
        if expect_ok:
            self.assertEqual(result.returncode, 0, result.stderr)
        else:
            self.assertNotEqual(result.returncode, 0)
        return result

    def test_native_hook_is_opt_in_and_records_the_post_timer_state(self) -> None:
        header = HEADER.read_text(encoding="utf-8")
        source = SOURCE.read_text(encoding="utf-8")
        integrator = INTEGRATOR.read_text(encoding="utf-8")

        for declaration in (
            "OpenttdrsMaybeStartIndustryTrace",
            "OpenttdrsMaybeExportIndustryTraceDay",
            "OpenttdrsTraceIndustryDailyAction",
            "OpenttdrsTraceIndustryFoundationResult",
        ):
            self.assertIn(declaration, header)
            self.assertIn(declaration, source)
        self.assertIn('std::getenv("OPENTTDRS_INDUSTRY_TRACE_OUT")', source)
        self.assertIn('std::getenv("OPENTTDRS_INDUSTRY_TRACE_DAYS")', source)
        self.assertIn('metadata["day_sample_point"] = "after_industry_daily_timer"', source)
        self.assertIn('row["random_state"]', source)
        self.assertIn('row["builddata"] = builddata', source)
        self.assertIn('row["actions"]', source)
        self.assertIn("this trace is also the oracle for the global RNG stream", source)

        # El integrador cubre los dos returns del timer y preserva los hooks
        # cuando el árbol no pinneado ya descendía de la instrumentación.
        self.assertIn('"industry_cmd: hook scheduler diario sin cambios"', integrator)
        self.assertIn('"industry_cmd: muestra post scheduler diario"', integrator)
        self.assertIn('"industry_cmd: decisiones scheduler diario"', integrator)
        self.assertIn('"industry_cmd: resultado de fundación runtime"', integrator)
        self.assertIn('industry_hook = "\\tOpenttdrsMaybeStartIndustryTrace', integrator)
        self.assertIn('"OpenttdrsMaybeExportIndustryTraceDay"', integrator)

    def test_validator_accepts_contract_and_rejects_wrong_action_cardinality(self) -> None:
        rows = valid_trace()
        self.run_validator(rows, expect_ok=True)

        invalid = valid_trace()
        day = invalid[-1]
        assert isinstance(day, dict)
        day["change_loop"] = 1
        result = self.run_validator(invalid, expect_ok=False)
        self.assertIn("actions", result.stderr)

    def test_runner_exports_the_same_contract(self) -> None:
        exporter = EXPORTER.read_text(encoding="utf-8")
        self.assertIn('OPENTTDRS_INDUSTRY_TRACE_OUT="$OUT"', exporter)
        self.assertIn('OPENTTDRS_INDUSTRY_TRACE_DAYS="$DAYS"', exporter)
        self.assertIn('validate_industry_trace.py" "$OUT" "$DAYS" openttd', exporter)


if __name__ == "__main__":
    unittest.main()
