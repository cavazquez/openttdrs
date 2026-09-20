#!/usr/bin/env python3
"""Regresiones fail-closed del agregado de contratos V1 (#603)."""

from __future__ import annotations

import contextlib
import hashlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import aggregate_v1_reports


CANDIDATE_SHA = "a" * 40


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_manifest(root: Path) -> Path:
    path = root / "manifest.json"
    path.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "kind": "v1-contract-manifest",
                "contracts": [
                    {"id": "V1-ONE", "issue": 1},
                    {"id": "V1-TWO", "issue": 2},
                ],
            }
        ),
        encoding="utf-8",
    )
    return path


def write_result(
    root: Path,
    contract_id: str,
    issue: int,
    *,
    candidate_sha: str = CANDIDATE_SHA,
    actual: int = 1,
    operator: str = ">=",
    threshold: int = 1,
    result: str = "passed",
) -> Path:
    artifact = root / f"{contract_id}.log"
    artifact.write_text(f"evidencia sintética para {contract_id}\n", encoding="utf-8")
    path = root / f"{contract_id}.json"
    path.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "kind": "v1-contract-result",
                "contract_id": contract_id,
                "issue": issue,
                "candidate_sha": candidate_sha,
                "fixture": f"fixtures/{contract_id}.ottdmap",
                "command": f"cargo test {contract_id}",
                "metric": {"name": "assertions", "actual": actual},
                "threshold": {"operator": operator, "value": threshold},
                "result": result,
                "artifacts": [{"path": artifact.name, "sha256": sha256(artifact)}],
            }
        ),
        encoding="utf-8",
    )
    return path


def invoke(root: Path, reports: dict[str, Path]) -> tuple[int, dict, str]:
    json_out = root / "out" / "aggregate.json"
    markdown_out = root / "out" / "aggregate.md"
    argv = [
        "--manifest",
        str(root / "manifest.json"),
        "--candidate-sha",
        CANDIDATE_SHA,
        "--json-out",
        str(json_out),
        "--markdown-out",
        str(markdown_out),
    ]
    for contract_id, report_path in reports.items():
        argv.extend(("--report", f"{contract_id}={report_path}"))
    stdout = io.StringIO()
    stderr = io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        returncode = aggregate_v1_reports.main(argv)
    return (
        returncode,
        json.loads(json_out.read_text(encoding="utf-8")),
        markdown_out.read_text(encoding="utf-8"),
    )


def contract(report: dict, contract_id: str) -> dict:
    return next(entry for entry in report["contracts"] if entry["id"] == contract_id)


class AggregateV1ReportsTest(unittest.TestCase):
    def test_complete_valid_input_is_the_only_green_result(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            write_manifest(root)
            reports = {
                "V1-ONE": write_result(root, "V1-ONE", 1),
                "V1-TWO": write_result(root, "V1-TWO", 2),
            }
            returncode, report, markdown = invoke(root, reports)

        self.assertEqual(returncode, 0)
        self.assertEqual(report["status"], "passed")
        self.assertTrue(report["v1_scoped_green"])
        self.assertEqual(report["global_parity"], "not-assessed")
        self.assertIn("Paridad global", markdown)

    def test_missing_contract_is_not_run_and_never_green(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            write_manifest(root)
            returncode, report, _ = invoke(root, {"V1-ONE": write_result(root, "V1-ONE", 1)})

        self.assertEqual(returncode, 1)
        self.assertEqual(report["status"], "not-run")
        self.assertFalse(report["v1_scoped_green"])
        missing = contract(report, "V1-TWO")
        self.assertEqual(missing["status"], "not-run")
        self.assertIn("falta resultado explícito", missing["problems"])

    def test_exceeded_threshold_fails_the_aggregate(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            write_manifest(root)
            reports = {
                "V1-ONE": write_result(root, "V1-ONE", 1, actual=2, operator="<=", threshold=1),
                "V1-TWO": write_result(root, "V1-TWO", 2),
            }
            returncode, report, _ = invoke(root, reports)

        self.assertEqual(returncode, 1)
        failed = contract(report, "V1-ONE")
        self.assertEqual(failed["status"], "failed")
        self.assertTrue(any("umbral excedido" in problem for problem in failed["problems"]))

    def test_mixed_candidate_sha_fails_the_aggregate(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            write_manifest(root)
            reports = {
                "V1-ONE": write_result(root, "V1-ONE", 1),
                "V1-TWO": write_result(root, "V1-TWO", 2, candidate_sha="b" * 40),
            }
            returncode, report, _ = invoke(root, reports)

        self.assertEqual(returncode, 1)
        failed = contract(report, "V1-TWO")
        self.assertEqual(failed["status"], "failed")
        self.assertTrue(any("candidate_sha mezclado" in problem for problem in failed["problems"]))

    def test_non_approvable_results_and_changed_artifacts_never_pass(self) -> None:
        for result in ("not-run", "skipped", "ignored", "timeout", "diagnostic", "excluded", "not-planned"):
            with self.subTest(result=result), tempfile.TemporaryDirectory() as temp:
                root = Path(temp)
                write_manifest(root)
                reports = {
                    "V1-ONE": write_result(root, "V1-ONE", 1, result=result),
                    "V1-TWO": write_result(root, "V1-TWO", 2),
                }
                returncode, report, _ = invoke(root, reports)
                self.assertEqual(returncode, 1)
                self.assertEqual(report["status"], "not-run")
                self.assertEqual(contract(report, "V1-ONE")["status"], "not-run")

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            write_manifest(root)
            first = write_result(root, "V1-ONE", 1)
            (root / "V1-ONE.log").write_text("artefacto alterado\n", encoding="utf-8")
            returncode, report, _ = invoke(
                root,
                {"V1-ONE": first, "V1-TWO": write_result(root, "V1-TWO", 2)},
            )

        self.assertEqual(returncode, 1)
        self.assertEqual(contract(report, "V1-ONE")["status"], "failed")


if __name__ == "__main__":
    unittest.main()
