#!/usr/bin/env python3
"""Reject native screenshot evidence when its recorded state advances."""

import unittest

from check_frozen_screenshot_state import PHASES, check_state


def sample(phase: str, tick: int = 3703074, **changes: int) -> str:
    values = dict(tick=tick, pause=17, display=255, transparency=0,
                  invisibility=0, vehicles=3293, hidden=8, stations=245,
                  visual_hash=123456, clean=0)
    values.update(changes)
    return "openttdrs frozen-state: phase=" + phase + " " + " ".join(
        f"{key}={value}" for key, value in values.items())


def log(**changes: int) -> str:
    return "\n".join(sample(phase, **changes) for phase in PHASES)


class FrozenScreenshotStateTest(unittest.TestCase):
    def test_same_frozen_state_accepts_other_log_lines_and_existing_pause_bits(self) -> None:
        state = check_state("loading SAV\n" + log() + "\nwrote PNG")
        self.assertEqual(state["tick"], 3703074)
        self.assertEqual(state["vehicles"], 3293)

    def test_tick_or_vehicle_presentation_change_rejects(self) -> None:
        for changes in ({"tick": 3703075}, {"visual_hash": 123457},
                        {"hidden": 9}, {"vehicles": 3294}, {"display": 254}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                check_state("\n".join((sample(PHASES[0]), sample(PHASES[1]),
                                       sample(PHASES[2], **changes))))

    def test_load_pause_cleanup_keeps_normal_pause_and_other_fields(self) -> None:
        state = check_state("\n".join((sample(PHASES[0], pause=67),
                                      sample(PHASES[1], pause=65),
                                      sample(PHASES[2], pause=65))))
        self.assertEqual(state["pause"], 67)
        self.assertEqual(state["pause_after_raster"], 65)
        for pause in (64, 1, 193):
            with self.subTest(pause=pause), self.assertRaises(ValueError):
                check_state("\n".join((sample(PHASES[0], pause=67),
                                       sample(PHASES[1], pause=65),
                                       sample(PHASES[2], pause=pause))))

    def test_clean_and_unpaused_or_command_paused_are_not_full_frozen_scene(self) -> None:
        for changes in ({"clean": 1}, {"pause": 16}, {"pause": 129}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                check_state(log(**changes))

    def test_missing_duplicate_or_reordered_phases_reject(self) -> None:
        for phases in ((), PHASES[:2], (*PHASES, PHASES[-1]),
                       (PHASES[1], PHASES[0], PHASES[2])):
            with self.subTest(phases=phases), self.assertRaises(ValueError):
                check_state("\n".join(sample(phase) for phase in phases))

    def test_malformed_records_reject(self) -> None:
        for malformed in (log().replace("tick=3703074", "tick=no"),
                          log().replace("vehicles=3293", "vehicles=3"),
                          log().replace("clean=0", "clean=0 clean=0"),
                          log().replace("clean=0", "unknown=0"),
                          log().replace("clean=0", "clean")):
            with self.subTest(malformed=malformed), self.assertRaises(ValueError):
                check_state(malformed)


if __name__ == "__main__":
    unittest.main()
