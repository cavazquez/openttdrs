#!/usr/bin/env python3
"""Agrega resultados V1 explícitos sin convertir evidencia incompleta en verde.

Cada ``--report`` tiene la forma ``V1-ID=ruta/al/result.json``. El manifiesto
define el conjunto completo obligatorio; un contrato no entregado queda
``not-run`` y hace fallar el proceso. Los artefactos de cada resultado se
resuelven relativos al archivo JSON que los declara y se validan por SHA-256.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import sys
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
SHA_RE = re.compile(r"[0-9a-f]{40}\Z")
ARTIFACT_SHA_RE = re.compile(r"[0-9a-f]{64}\Z")
COMPARISONS = {"<=", "<", "==", ">=", ">"}
NOT_RUN_RESULTS = {
    "not-run",
    "skipped",
    "ignored",
    "timeout",
    "diagnostic",
    "excluded",
    "not-planned",
}


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def is_nonempty_string(value: object) -> bool:
    return isinstance(value, str) and bool(value.strip())


def is_number(value: object) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def compare(actual: float | int, operator: str, maximum: float | int) -> bool:
    if operator == "<=":
        return actual <= maximum
    if operator == "<":
        return actual < maximum
    if operator == "==":
        return actual == maximum
    if operator == ">=":
        return actual >= maximum
    if operator == ">":
        return actual > maximum
    raise ValueError(f"operador no admitido: {operator}")


def portable_path(path: Path) -> str:
    try:
        return path.resolve().relative_to(ROOT).as_posix()
    except ValueError:
        return path.as_posix()


def read_json(path: Path) -> tuple[dict[str, Any] | None, str | None]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return None, f"no se pudo leer JSON: {exc}"
    if not isinstance(value, dict):
        return None, "el resultado debe ser un objeto JSON"
    return value, None


def validate_manifest(path: Path) -> list[dict[str, Any]]:
    raw, error = read_json(path)
    if error:
        raise ValueError(f"manifiesto inválido ({path}): {error}")
    assert raw is not None
    if raw.get("schema_version") != 1 or raw.get("kind") != "v1-contract-manifest":
        raise ValueError("manifiesto V1 con schema_version o kind incorrecto")
    contracts = raw.get("contracts")
    if not isinstance(contracts, list) or not contracts:
        raise ValueError("manifiesto V1 sin contratos")

    seen: set[str] = set()
    normalized: list[dict[str, Any]] = []
    for contract in contracts:
        if not isinstance(contract, dict):
            raise ValueError("cada contrato del manifiesto debe ser un objeto")
        contract_id = contract.get("id")
        issue = contract.get("issue")
        if not is_nonempty_string(contract_id) or not isinstance(issue, int) or issue <= 0:
            raise ValueError("cada contrato requiere id e issue positivo")
        assert isinstance(contract_id, str)
        if contract_id in seen:
            raise ValueError(f"id V1 repetido en manifiesto: {contract_id}")
        seen.add(contract_id)
        normalized.append({"id": contract_id, "issue": issue})
    return normalized


def parse_report_arguments(values: list[str], valid_ids: set[str]) -> tuple[dict[str, Path], list[str]]:
    reports: dict[str, Path] = {}
    errors: list[str] = []
    for value in values:
        contract_id, separator, raw_path = value.partition("=")
        if not separator or not contract_id or not raw_path:
            errors.append(f"--report inválido: {value!r}; usar V1-ID=ruta.json")
            continue
        if contract_id not in valid_ids:
            errors.append(f"--report declara contrato fuera del manifiesto: {contract_id}")
            continue
        if contract_id in reports:
            errors.append(f"--report duplicado para {contract_id}")
            continue
        reports[contract_id] = Path(raw_path)
    return reports, errors


def validate_artifacts(report: dict[str, Any], report_path: Path) -> list[str]:
    errors: list[str] = []
    artifacts = report.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        return ["faltan artefactos verificables"]
    base = report_path.parent.resolve()
    for index, artifact in enumerate(artifacts, start=1):
        if not isinstance(artifact, dict):
            errors.append(f"artefacto #{index} no es objeto")
            continue
        raw_path = artifact.get("path")
        expected_sha = artifact.get("sha256")
        if not is_nonempty_string(raw_path) or not isinstance(expected_sha, str):
            errors.append(f"artefacto #{index} requiere path y sha256")
            continue
        assert isinstance(raw_path, str)
        if not ARTIFACT_SHA_RE.fullmatch(expected_sha):
            errors.append(f"artefacto #{index} tiene sha256 inválido")
            continue
        relative = Path(raw_path)
        if relative.is_absolute() or ".." in relative.parts:
            errors.append(f"artefacto #{index} debe ser relativo al resultado")
            continue
        target = (base / relative).resolve()
        try:
            target.relative_to(base)
        except ValueError:
            errors.append(f"artefacto #{index} escapa el directorio del resultado")
            continue
        if not target.is_file():
            errors.append(f"falta artefacto #{index}: {relative.as_posix()}")
            continue
        if sha256(target) != expected_sha:
            errors.append(f"SHA distinto para artefacto #{index}: {relative.as_posix()}")
    return errors


def validate_required_fields(report: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    if report.get("schema_version") != 1:
        errors.append("schema_version debe ser 1")
    if report.get("kind") != "v1-contract-result":
        errors.append("kind debe ser v1-contract-result")
    for field in ("fixture", "command"):
        if not is_nonempty_string(report.get(field)):
            errors.append(f"falta {field}")

    metric = report.get("metric")
    if not isinstance(metric, dict) or not is_nonempty_string(metric.get("name")) or not is_number(metric.get("actual")):
        errors.append("metric requiere name y actual numérico finito")
    threshold = report.get("threshold")
    if (
        not isinstance(threshold, dict)
        or threshold.get("operator") not in COMPARISONS
        or not is_number(threshold.get("value"))
    ):
        errors.append("threshold requiere operator válido y value numérico finito")
    if not is_nonempty_string(report.get("result")):
        errors.append("falta result")
    return errors


def assess_contract(
    contract: dict[str, Any],
    candidate_sha: str,
    report_path: Path | None,
) -> dict[str, Any]:
    entry: dict[str, Any] = {
        "id": contract["id"],
        "issue": contract["issue"],
        "status": "not-run",
        "problems": [],
    }
    if report_path is None:
        entry["problems"].append("falta resultado explícito")
        return entry
    entry["report"] = portable_path(report_path)
    if not report_path.is_file():
        entry["problems"].append("falta archivo de resultado")
        return entry

    report, read_error = read_json(report_path)
    if read_error:
        entry["status"] = "failed"
        entry["problems"].append(read_error)
        return entry
    assert report is not None

    errors = validate_required_fields(report)
    if report.get("contract_id") != contract["id"]:
        errors.append("contract_id no coincide con el manifiesto")
    if report.get("issue") != contract["issue"]:
        errors.append("issue no coincide con el manifiesto")
    source_sha = report.get("candidate_sha")
    if not isinstance(source_sha, str) or not SHA_RE.fullmatch(source_sha):
        errors.append("candidate_sha debe ser un SHA completo de 40 hexadecimales")
    elif source_sha != candidate_sha:
        errors.append(f"candidate_sha mezclado ({source_sha} != {candidate_sha})")
    errors.extend(validate_artifacts(report, report_path))

    metric = report.get("metric")
    threshold = report.get("threshold")
    if isinstance(metric, dict):
        entry["metric"] = metric
    if isinstance(threshold, dict):
        entry["threshold"] = threshold
    entry["fixture"] = report.get("fixture")
    entry["command"] = report.get("command")
    entry["artifact_count"] = len(report.get("artifacts", [])) if isinstance(report.get("artifacts"), list) else 0

    result = report.get("result")
    if result == "passed" and not errors:
        assert isinstance(metric, dict)
        assert isinstance(threshold, dict)
        actual = metric["actual"]
        maximum = threshold["value"]
        operator = threshold["operator"]
        assert is_number(actual) and is_number(maximum) and isinstance(operator, str)
        if compare(actual, operator, maximum):
            entry["status"] = "passed"
        else:
            entry["status"] = "failed"
            errors.append(f"umbral excedido: {actual} {operator} {maximum} es falso")
    elif result in NOT_RUN_RESULTS and not errors:
        entry["status"] = "not-run"
        errors.append(f"resultado no aprobable: {result}")
    elif result == "failed" and not errors:
        entry["status"] = "failed"
        errors.append("el contrato informa failed")
    else:
        entry["status"] = "failed"
        if result not in {"passed", "failed", *NOT_RUN_RESULTS}:
            errors.append(f"result desconocido: {result!r}")
    entry["problems"].extend(errors)
    return entry


def aggregate(
    contracts: list[dict[str, Any]],
    candidate_sha: str,
    reports: dict[str, Path],
    input_errors: list[str],
) -> dict[str, Any]:
    entries = [assess_contract(contract, candidate_sha, reports.get(contract["id"])) for contract in contracts]
    statuses = {entry["status"] for entry in entries}
    if input_errors or "failed" in statuses:
        status = "failed"
    elif statuses == {"passed"}:
        status = "passed"
    else:
        status = "not-run"
    return {
        "schema_version": 1,
        "kind": "v1-contract-aggregate",
        "candidate_sha": candidate_sha,
        "status": status,
        "v1_scoped_green": status == "passed",
        "global_parity": "not-assessed",
        "input_errors": input_errors,
        "contracts": entries,
    }


def markdown(report: dict[str, Any]) -> str:
    lines = [
        "# Resultado V1 por contrato",
        "",
        f"SHA candidata: `{report['candidate_sha']}`",
        "",
        "| Contrato | Issue | Resultado | Fixture | Métrica / umbral | Artefactos |",
        "|---|---:|---|---|---|---:|",
    ]
    for entry in report["contracts"]:
        metric = entry.get("metric")
        threshold = entry.get("threshold")
        metric_text = "—"
        if isinstance(metric, dict) and isinstance(threshold, dict):
            metric_text = f"{metric.get('name')}: {metric.get('actual')} {threshold.get('operator')} {threshold.get('value')}"
        fixture = entry.get("fixture") if is_nonempty_string(entry.get("fixture")) else "—"
        lines.append(
            f"| {entry['id']} | #{entry['issue']} | {entry['status']} | {fixture} | {metric_text} | {entry.get('artifact_count', 0)} |"
        )
        for problem in entry["problems"]:
            lines.append(f"| ↳ |  |  |  | {problem} |  |")
    if report["input_errors"]:
        lines.extend(["", "## Errores de entrada", ""])
        lines.extend(f"- {error}" for error in report["input_errors"])
    lines.extend(
        [
            "",
            f"**Resultado V1 acotado:** `{report['status']}`.",
            "",
            "**Paridad global:** no evaluada. Este informe no cuenta issues cerrados, diagnósticos, exclusiones ni contratos `not-planned` como aprobación.",
            "",
        ]
    )
    return "\n".join(lines)


def write_outputs(report: dict[str, Any], json_out: Path, markdown_out: Path) -> None:
    json_out.parent.mkdir(parents=True, exist_ok=True)
    markdown_out.parent.mkdir(parents=True, exist_ok=True)
    json_out.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    markdown_out.write_text(markdown(report), encoding="utf-8")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--candidate-sha", required=True)
    parser.add_argument("--report", action="append", default=[], metavar="V1-ID=PATH")
    parser.add_argument("--json-out", required=True, type=Path)
    parser.add_argument("--markdown-out", required=True, type=Path)
    args = parser.parse_args(argv)

    candidate_sha = args.candidate_sha.lower()
    if not SHA_RE.fullmatch(candidate_sha):
        parser.error("--candidate-sha debe tener 40 hexadecimales")
    try:
        contracts = validate_manifest(args.manifest)
    except ValueError as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 2
    reports, input_errors = parse_report_arguments(args.report, {contract["id"] for contract in contracts})
    report = aggregate(contracts, candidate_sha, reports, input_errors)
    write_outputs(report, args.json_out, args.markdown_out)
    print(
        f"V1 report: status={report['status']} contracts={len(report['contracts'])} "
        f"candidate_sha={candidate_sha} json={args.json_out} markdown={args.markdown_out}"
    )
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
