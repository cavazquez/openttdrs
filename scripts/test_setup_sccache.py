#!/usr/bin/env python3
"""Regresión del bootstrap local de sccache usado por ``check.sh``."""

from __future__ import annotations

import os
import subprocess
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
SETUP = ROOT / "scripts" / "setup_sccache.sh"


def run_setup(*, sccache_rustc_exit: int, wrapper: str | None = None) -> str:
    with tempfile.TemporaryDirectory() as tmp:
        bin_dir = Path(tmp) / "bin"
        bin_dir.mkdir()
        rustc = bin_dir / "rustc"
        rustc.write_text("#!/usr/bin/env bash\nexit 0\n", encoding="utf-8")
        sccache = bin_dir / "sccache"
        sccache.write_text(
            "#!/usr/bin/env bash\n"
            "if [[ \"$1\" == \"--version\" ]]; then echo 'sccache test'; exit 0; fi\n"
            "if [[ \"$2\" == \"-vV\" ]]; then exit \"${SCCACHE_RUSTC_EXIT}\"; fi\n"
            "exit 99\n",
            encoding="utf-8",
        )
        rustc.chmod(0o755)
        sccache.chmod(0o755)
        env = os.environ.copy()
        env["PATH"] = f"{bin_dir}:{env['PATH']}"
        env["SCCACHE_RUSTC_EXIT"] = str(sccache_rustc_exit)
        if wrapper is None:
            env.pop("RUSTC_WRAPPER", None)
        else:
            env["RUSTC_WRAPPER"] = wrapper
        shell = "info() { :; }; warn() { :; }; source \"$1\"; printf '%s' \"${RUSTC_WRAPPER-unset}\""
        result = subprocess.run(
            ["bash", "-c", shell, "bash", str(SETUP)],
            env=env,
            capture_output=True,
            text=True,
            check=True,
        )
        return result.stdout


def main() -> int:
    assert run_setup(sccache_rustc_exit=0) == "sccache"
    assert run_setup(sccache_rustc_exit=2) == "unset"
    assert run_setup(sccache_rustc_exit=2, wrapper="chosen-by-user") == "chosen-by-user"
    print("OK: sccache sólo se activa cuando puede ejecutar rustc")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
