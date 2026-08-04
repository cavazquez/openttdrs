#!/usr/bin/env bash
# Integra el driver de capturas UI (#297) en OpenTTD 15.3 fijado por #109.
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
  exit 1
fi

cp "${PATCH_DIR}/src/ui_capture.cpp" "${DEST}/src/ui_capture.cpp"
cp "${PATCH_DIR}/src/ui_capture.h" "${DEST}/src/ui_capture.h"

python3 - "${DEST}" <<'PY'
from pathlib import Path
import sys

dest = Path(sys.argv[1])
cmake = dest / "src" / "CMakeLists.txt"
text = cmake.read_text(encoding="utf-8")
if "ui_capture.cpp" not in text:
    anchor = "    console_cmds.cpp\n"
    if anchor not in text:
        raise SystemExit("no encuentro console_cmds.cpp en src/CMakeLists.txt")
    cmake.write_text(text.replace(anchor, anchor + "    ui_capture.cpp\n", 1), encoding="utf-8")
    print("CMakeLists: ui_capture.cpp")
else:
    print("CMakeLists: ui_capture.cpp ya listado")

openttd = dest / "src" / "openttd.cpp"
text = openttd.read_text(encoding="utf-8")
if '#include "ui_capture.h"' not in text:
    anchor = "\n#include "
    pos = text.find(anchor)
    if pos < 0:
        raise SystemExit("no encuentro bloque de includes en openttd.cpp")
    text = text[: pos + 1] + '#include "ui_capture.h"\n' + text[pos + 1 :]
    print("openttd: include")

hook = "\tOpenttdrsMaybeCaptureUi();\n"
# El driver puede pausar el juego para congelar el frame. Debe correr antes
# del guard de pausa de StateGameLoop; de otro modo no llegaría a pedir el PNG.
anchor = "void StateGameLoop()\n{\n"
text = text.replace(hook, "")
if hook not in text:
    if anchor not in text:
        raise SystemExit("no encuentro StateGameLoop en openttd.cpp")
    text = text.replace(anchor, anchor + hook, 1)
    print("openttd: hook StateGameLoop antes de pausa")
else:
    print("openttd: hook ya presente")
openttd.write_text(text, encoding="utf-8")
PY

echo "Integrado en ${DEST}"
echo "Build UI: cmake -S ${DEST} -B /tmp/openttdrs-openttd-15.3-ui -DOPTION_USE_ASSERTS=OFF && cmake --build /tmp/openttdrs-openttd-15.3-ui --target openttd"
