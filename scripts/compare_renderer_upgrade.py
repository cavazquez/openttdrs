#!/usr/bin/env python3
"""Gate a renderer upgrade at all six zooms against a baseline and native oracle.

Each input directory contains SCALE/world.png. Compare the complete RGBA frames
without alignment or crops. The default gate is strict. An explicitly selected
exclusive pixel budget is reported separately, preserving the strict result.
"""

import argparse
import csv
import hashlib
import json
from pathlib import Path

from window_visual_regression import PngImage, read_png

SCALES = ("0.25", "0.5", "1", "2", "4", "8")


def metrics(before, after, native):
    if (before.width, before.height) != (after.width, after.height) or (
        before.width, before.height
    ) != (native.width, native.height):
        raise ValueError("all frames must have identical dimensions")
    counts = dict(changed_pixels=0, worsened_pixels=0, improved_pixels=0,
                  equally_wrong_changed_pixels=0, native_before=0, native_after=0,
                  lost_correct_pixels=0, gained_correct_pixels=0)
    blocks = set()
    for index in range(before.width * before.height):
        offset = index * 4
        old = before.rgba[offset:offset + 4]
        new = after.rgba[offset:offset + 4]
        ref = native.rgba[offset:offset + 4]
        if old != new:
            counts["changed_pixels"] += 1
            blocks.add(((index % before.width) // 4, (index // before.width) // 4))
            old_error = sum(abs(a - b) for a, b in zip(old, ref))
            new_error = sum(abs(a - b) for a, b in zip(new, ref))
            counts["worsened_pixels"] += new_error > old_error
            counts["improved_pixels"] += new_error < old_error
            counts["equally_wrong_changed_pixels"] += new_error == old_error
        counts["native_before"] += old != ref
        counts["native_after"] += new != ref
        counts["lost_correct_pixels"] += old == ref and new != ref
        counts["gained_correct_pixels"] += old != ref and new == ref
    counts["changed_blocks_4x4"] = len(blocks)
    counts["total_pixels"] = before.width * before.height
    counts["passed"] = counts["lost_correct_pixels"] == 0 and counts["native_after"] <= counts["native_before"]
    return counts


def apply_budget(report, metric, exclusive_limit):
    if metric not in ("changed_pixels", "worsened_pixels") or exclusive_limit <= 0:
        raise ValueError("budget requires a positive exclusive limit and a known metric")
    report["acceptance"] = {"metric": metric, "exclusive_limit": exclusive_limit}
    for row in report["zooms"]:
        row["budget_passed"] = row[metric] < exclusive_limit
    report["budget_passed"] = all(row["budget_passed"] for row in report["zooms"])
    return report


def compare(baseline, candidate, oracle):
    results = []
    for scale in SCALES:
        paths = [root / scale / "world.png" for root in (baseline, candidate, oracle)]
        row = {"scale": scale, **metrics(*(read_png(path) for path in paths))}
        for label, path in zip(("baseline", "candidate", "oracle"), paths):
            row[f"{label}_sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
        results.append(row)
    return {"schema": 1, "alignment": [0, 0], "passed": all(row["passed"] for row in results), "zooms": results}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True, type=Path)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--oracle", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--csv", type=Path)
    budget = parser.add_mutually_exclusive_group()
    budget.add_argument("--max-changed-pixels-exclusive", type=int)
    budget.add_argument("--max-worsened-pixels-exclusive", type=int)
    args = parser.parse_args()
    report = compare(args.baseline, args.candidate, args.oracle)
    if args.max_changed_pixels_exclusive is not None:
        apply_budget(report, "changed_pixels", args.max_changed_pixels_exclusive)
    elif args.max_worsened_pixels_exclusive is not None:
        apply_budget(report, "worsened_pixels", args.max_worsened_pixels_exclusive)
    args.report.write_text(json.dumps(report, indent=2) + "\n")
    if args.csv:
        with args.csv.open("w", newline="") as stream:
            writer = csv.DictWriter(stream, fieldnames=list(report["zooms"][0]), lineterminator="\n")
            writer.writeheader()
            writer.writerows(report["zooms"])
    print(json.dumps({"strict_passed": report["passed"], "budget_passed": report.get("budget_passed"), "zooms": [{key: row[key] for key in ("scale", "changed_pixels", "worsened_pixels", "improved_pixels", "lost_correct_pixels", "gained_correct_pixels", "passed")} for row in report["zooms"]]}))
    return 0 if report.get("budget_passed", report["passed"]) else 1


if __name__ == "__main__":
    raise SystemExit(main())
