#!/usr/bin/env python3
"""Verify a paused native capture without hiding its ordinary viewport layers."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path


PHASES = ("after_load", "before_raster_queue", "after_raster")
FIELDS = (
    "tick", "pause", "display", "transparency", "invisibility",
    "vehicles", "hidden", "stations", "visual_hash", "clean",
)
PREFIX = "openttdrs frozen-state: "


def check_state(log: str) -> dict[str, int]:
    """Require three consistent samples; this is not a whole-game state hash."""
    samples = []
    for line in log.splitlines():
        if PREFIX not in line:
            continue
        record = line.split(PREFIX, 1)[1]
        pairs = [token.split("=", 1) for token in record.split()]
        if any(len(pair) != 2 for pair in pairs):
            raise ValueError("malformed frozen-state record")
        values = dict(pairs)
        if len(values) != len(pairs) or set(values) != {"phase", *FIELDS}:
            raise ValueError("missing, duplicate or unknown frozen-state field")
        if any(re.fullmatch(r"[0-9]+", values[key]) is None for key in FIELDS):
            raise ValueError("non-integer frozen-state field")
        samples.append((values["phase"], {key: int(values[key]) for key in FIELDS}))
    if tuple(phase for phase, _ in samples) != PHASES:
        raise ValueError("expected one sample per load/queue/raster phase in order")
    initial = samples[0][1]
    if initial["clean"] != 0:
        raise ValueError("CLEAN hides viewport layers; full-scene reference requires CLEAN=0")
    # PauseMode::Normal is bit zero, not enum value one (native openttd.h).
    if not initial["pause"] & 1:
        raise ValueError("normal pause bit is absent")
    if initial["pause"] & 128:
        raise ValueError("command-during-pause mode would permit state updates")
    if initial["hidden"] > initial["vehicles"]:
        raise ValueError("hidden count exceeds vehicle count")
    for phase, values in samples[1:]:
        # AfterLoadGame still owns SaveLoad (bit 1). Its caller may clear that
        # bit after returning; Normal must remain set throughout the capture.
        allowed_pause = {initial["pause"], initial["pause"] & ~2}
        differences = [key for key in FIELDS if key != "pause" and values[key] != initial[key]]
        if values["pause"] not in allowed_pause:
            differences.append("pause")
        if differences:
            raise ValueError(f"state changed at {phase}: {', '.join(differences)}")
    return {**initial, "pause_after_raster": samples[-1][1]["pause"]}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("log", type=Path)
    args = parser.parse_args()
    try:
        state = check_state(args.log.read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        parser.exit(1, f"FAIL: {error}\n")
    print(json.dumps({"verified": True, "samples": len(PHASES), "state": state}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
