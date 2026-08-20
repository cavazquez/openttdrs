#!/usr/bin/env python3
"""Contrato estático del replay de corpus que corre en PRs."""

from __future__ import annotations

import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github" / "workflows" / "fuzz-replay.yml"
WEEKLY = ROOT / ".github" / "workflows" / "fuzz.yml"
REPLAY = ROOT / "scripts" / "replay_fuzz_regressions.sh"


def require(text: str, needle: str, source: Path, errors: list[str]) -> None:
    if needle not in text:
        errors.append(f"{source.relative_to(ROOT)}: falta {needle!r}")


def main() -> int:
    errors: list[str] = []
    workflow = WORKFLOW.read_text(encoding="utf-8")
    weekly = WEEKLY.read_text(encoding="utf-8")
    replay = REPLAY.read_text(encoding="utf-8")

    for needle in (
        "pull_request:",
        "push:",
        "workflow_dispatch:",
        "actions/checkout@v7",
        "./.github/composite/sccache",
        "FUZZ_TOOLCHAIN: nightly-2026-07-31",
        "CARGO_FUZZ_VERSION: 0.13.2",
        'rustup toolchain install "${FUZZ_TOOLCHAIN}" --profile minimal',
        'cargo +"${FUZZ_TOOLCHAIN}" install cargo-fuzz --version "${CARGO_FUZZ_VERSION}" --locked',
        "./scripts/replay_fuzz_regressions.sh",
        "if: always()",
        "fuzz-replay-artifacts",
        "path: fuzz/artifacts/",
    ):
        require(workflow, needle, WORKFLOW, errors)

    for needle in (
        "metadata --manifest-path Cargo.toml --locked --format-version 1",
        "sav_load newgrf_parse net_message",
        "regression-corpus/$target",
        "regression-anchors/sav_load",
        "-runs=0",
        "-timeout=10",
        "-rss_limit_mb=1024",
        "-print_final_stats=1",
    ):
        require(replay, needle, REPLAY, errors)

    require(weekly, "schedule:", WEEKLY, errors)
    require(weekly, "regression-corpus/${{ matrix.target }}", WEEKLY, errors)
    if "paths:" in workflow:
        errors.append("fuzz-replay.yml no debe usar paths: un check requerido no puede quedar pendiente")
    if "pull_request:" in weekly:
        errors.append("fuzz.yml debe conservar sólo fuzz aleatorio programado/manual, no PR")

    if errors:
        print("FAIL: contrato del replay fuzz", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1
    print("OK: replay fuzz obligatorio en PR y fuzz aleatorio semanal separado")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
