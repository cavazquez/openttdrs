#!/usr/bin/env python3
"""Regresiones del validador del menú en el smoke gráfico de release (#577)."""

from __future__ import annotations

from pathlib import Path
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[1]
CHECKER = ROOT / "scripts" / "check_release_graphical_smoke.py"
sys.path.insert(0, str(ROOT / "scripts"))

from window_visual_regression import PngImage, write_png  # noqa: E402


def patterned_image(width: int, height: int, offset: int) -> PngImage:
    pixels = bytearray(width * height * 4)
    target = 0
    for y in range(height):
        for x in range(width):
            band = ((x // 8) + (y // 6) + offset) % 12
            pixels[target : target + 4] = bytes(
                ((band * 19) % 256, (band * 41) % 256, (band * 67) % 256, 255)
            )
            target += 4
    return PngImage(width, height, bytes(pixels))


def run(menu: Path, width: int = 128, height: int = 72) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            sys.executable,
            str(CHECKER),
            "--menu",
            str(menu),
            "--width",
            str(width),
            "--height",
            str(height),
        ],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def main() -> int:
    with tempfile.TemporaryDirectory() as temp:
        root = Path(temp)
        menu = root / "menu.png"
        write_png(menu, patterned_image(128, 72, 0))

        passed = run(menu)
        if passed.returncode != 0:
            print(passed.stdout, passed.stderr, file=sys.stderr)
            print("FAIL: un menú compuesto debe pasar", file=sys.stderr)
            return 1

        blank = root / "blank.png"
        write_png(blank, PngImage(128, 72, bytes((0, 0, 0, 255)) * (128 * 72)))
        empty = run(blank)
        if empty.returncode != 1 or "plana" not in empty.stderr:
            print(empty.stdout, empty.stderr, file=sys.stderr)
            print("FAIL: una pantalla plana debe fallar", file=sys.stderr)
            return 1

        wrong_size = root / "wrong-size.png"
        write_png(wrong_size, patterned_image(127, 72, 4))
        geometry = run(wrong_size)
        if geometry.returncode != 1 or "dimensiones" not in geometry.stderr:
            print(geometry.stdout, geometry.stderr, file=sys.stderr)
            print("FAIL: una resolución distinta debe fallar", file=sys.stderr)
            return 1

    print("OK: el smoke gráfico rechaza menús vacíos o de otra geometría")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
