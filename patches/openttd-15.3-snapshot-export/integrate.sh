#!/usr/bin/env bash
# Integra el export de snapshots (#110) en un clon OpenTTD @ pin #109.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PATCH_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEST="${1:-${ROOT}/reference/openttd-upstream}"

# shellcheck source=../../scripts/lib/openttd_reference.sh
source "${ROOT}/scripts/lib/openttd_reference.sh"

EXPECTED="$(openttd_manifest_get "$ROOT" commit)"
if [[ ! -d "${DEST}/.git" ]]; then
  echo "error: no hay clon en ${DEST}; corré ./scripts/fetch-openttd-reference.sh" >&2
  exit 1
fi
ACTUAL="$(git -C "${DEST}" rev-parse HEAD)"
if [[ "${ACTUAL}" != "${EXPECTED}" ]]; then
  echo "error: ${DEST} está en ${ACTUAL}, manifiesto espera ${EXPECTED}" >&2
  echo "  ./scripts/fetch-openttd-reference.sh" >&2
  exit 1
fi

cp "${PATCH_DIR}/src/snapshot_export.cpp" "${DEST}/src/snapshot_export.cpp"
cp "${PATCH_DIR}/src/snapshot_export.h" "${DEST}/src/snapshot_export.h"

python3 - "$DEST" <<'PY'
from pathlib import Path
import sys

dest = Path(sys.argv[1])
cmake = dest / "src" / "CMakeLists.txt"
text = cmake.read_text(encoding="utf-8")
if "snapshot_export.cpp" not in text:
    if "console_cmds.cpp" not in text:
        raise SystemExit("no encuentro console_cmds.cpp en src/CMakeLists.txt")
    text = text.replace(
        "    console_cmds.cpp\n",
        "    console_cmds.cpp\n    snapshot_export.cpp\n",
        1,
    )
    cmake.write_text(text, encoding="utf-8")
    print("CMakeLists: snapshot_export.cpp")
else:
    print("CMakeLists: ya listado")

after = dest / "src" / "saveload" / "afterload.cpp"
at = after.read_text(encoding="utf-8")
if '#include "../snapshot_export.h"' not in at:
    # Tras el primer bloque de includes del archivo.
    nl = at.find("\n#include ")
    if nl < 0:
        raise SystemExit("no encuentro includes en afterload.cpp")
    at = at[: nl + 1] + '#include "../snapshot_export.h"\n' + at[nl + 1 :]
    print("afterload: include")

hook = (
    "\tif (!OpenttdrsMaybeExportSnapshot({})) {\n"
    "\t\tDebug(misc, 0, \"openttdrs snapshot export failed\");\n"
    "\t}\n"
)
anchor = "\treturn true;\n}\n\n/**\n * Reload all NewGRF"
if "OpenttdrsMaybeExportSnapshot" not in at:
    if anchor not in at:
        raise SystemExit("no encuentro ancla return true de AfterLoadGame")
    at = at.replace(anchor, hook + "\treturn true;\n}\n\n/**\n * Reload all NewGRF", 1)
    print("afterload: hook AfterLoadGame")
else:
    print("afterload: hook ya presente")

after.write_text(at, encoding="utf-8")
PY

echo "Integrado en ${DEST}"
echo "Build dedicated (ejemplo):"
echo "  cmake -B ${DEST}/build -S ${DEST} -DOPTION_DEDICATED=ON && cmake --build ${DEST}/build -j"
echo "Export:"
echo "  OPENTTDRS_SNAPSHOT_OUT=/tmp/openttd.json OPENTTDRS_OPENTTD_COMMIT=${EXPECTED} \\"
echo "    ${DEST}/build/openttd -D -g path/to/game.sav"
openttd_manifest_summary "$ROOT"
