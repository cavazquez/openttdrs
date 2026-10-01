#!/usr/bin/env python3
"""Compare complete opt-in sprite traces, preserving query order and aliases.

Usage: compare_map_sprite_traces.py left.json right.json
CPU image files live beside each trace in its `.images` directory.
Only temporary entity/asset identities are renamed; no other field is omitted.
"""

import argparse
import copy
import json
from pathlib import Path
import unittest


def normalized_trace(document):
    if document["schema"] != 1:
        raise ValueError("unsupported sprite trace schema")
    document = copy.deepcopy(document)
    main_identities, render_identities = {}, {}

    def rename(value, identities):
        if value not in identities:
            identities[value] = len(identities)
        return identities[value]

    inputs = [row["inputs"] for key in ("sprites", "meshes", "cameras")
              for row in document[key]]
    for row in inputs:
        row["entity"] = rename(row["entity"], main_identities)
        if row["render_entity"] is not None:
            row["render_entity"] = rename(row["render_entity"], render_identities)
    for row in inputs:
        if row["proxy_source"] is not None:
            row["proxy_source"] = rename(row["proxy_source"], main_identities)
        if row["child"] is not None:
            row["child"]["parent"] = rename(row["child"]["parent"], main_identities)
    for kind in ("images", "layouts"):
        identities = {}
        for row in document["assets"][kind]:
            row["asset_id"] = rename(row["asset_id"], identities)
        if len(identities) != len(document["assets"][kind]):
            raise ValueError("asset table contains a duplicate identity")
    return document


def compare(left_path, right_path):
    documents = [json.loads(path.read_text()) for path in (left_path, right_path)]
    tables = [document["assets"]["images"] for document in documents]
    image_bytes_equal = len(tables[0]) == len(tables[1])
    compared = missing = 0
    for left, right in zip(*tables):
        files = [row.get("data_file") for row in (left, right)]
        if files == [None, None]:
            missing += 1
        elif None in files:
            image_bytes_equal = False
        else:
            paths = [trace.with_suffix(".images") / file
                     for trace, file in zip((left_path, right_path), files)]
            image_bytes_equal &= paths[0].read_bytes() == paths[1].read_bytes()
            compared += 1
    return {
        "complete_inputs_equal_after_bijective_identity_renaming":
            normalized_trace(documents[0]) == normalized_trace(documents[1]),
        "cpu_image_bytes_equal": image_bytes_equal,
        "cpu_images_compared": compared,
        "images_without_cpu_data": missing,
    }


class IdentityContractTests(unittest.TestCase):
    @staticmethod
    def trace():
        def inputs(entity, render, source=None):
            return {"entity": entity, "render_entity": render,
                    "proxy_source": source, "child": None,
                    "transform_bits": [0x3F800000]}
        return {"schema": 1,
                "sprites": [{"inputs": inputs(10, 20)}, {"inputs": inputs(20, 10)}],
                "meshes": [{"inputs": inputs(30, 40, 10)}, {"inputs": inputs(40, 30, 20)}],
                "cameras": [],
                "assets": {"images": [{"asset_id": "a"}], "layouts": []}}

    def test_renaming_preserves_aliases_in_separate_worlds(self):
        left = self.trace()
        right = copy.deepcopy(left)
        main = {10: 80, 20: 90, 30: 60, 40: 70}
        render = {10: 80, 20: 70, 30: 90, 40: 60}
        for row in right["sprites"] + right["meshes"]:
            inputs = row["inputs"]
            inputs["entity"] = main[inputs["entity"]]
            inputs["render_entity"] = render[inputs["render_entity"]]
            if inputs["proxy_source"] is not None:
                inputs["proxy_source"] = main[inputs["proxy_source"]]
        right["assets"]["images"][0]["asset_id"] = "other"
        self.assertEqual(normalized_trace(left), normalized_trace(right))

    def test_swapped_sources_are_not_hidden_by_renaming(self):
        left = self.trace()
        right = copy.deepcopy(left)
        right["meshes"][0]["inputs"]["proxy_source"] = 20
        right["meshes"][1]["inputs"]["proxy_source"] = 10
        self.assertNotEqual(normalized_trace(left), normalized_trace(right))

    def test_float_bit_change_remains_a_difference(self):
        left = self.trace()
        right = copy.deepcopy(left)
        right["sprites"][0]["inputs"]["transform_bits"][0] += 1
        self.assertNotEqual(normalized_trace(left), normalized_trace(right))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("left", type=Path, nargs="?")
    parser.add_argument("right", type=Path, nargs="?")
    parser.add_argument("--self-test", action="store_true")
    arguments = parser.parse_args()
    if arguments.self_test:
        result = unittest.TextTestRunner().run(unittest.defaultTestLoader.loadTestsFromTestCase(IdentityContractTests))
        return 0 if result.wasSuccessful() else 1
    if arguments.left is None or arguments.right is None:
        parser.error("both trace paths are required")
    result = compare(arguments.left, arguments.right)
    print(json.dumps(result, indent=2))
    return 0 if result["complete_inputs_equal_after_bijective_identity_renaming"] and result["cpu_image_bytes_equal"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
