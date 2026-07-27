#!/usr/bin/env bash
# Build reproducible del oráculo visual OpenTTD 15.3 (#240).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SOURCE="${OPENTTD_REFERENCE_SOURCE:-${ROOT}/reference/openttd-upstream}"
BUILD="${OPENTTD_REFERENCE_BUILD:-/tmp/openttdrs-openttd-15.3}"
BASESET="${ROOT}/.deps/openttd-baseset/opengfx-8.0"
EXPECTED_COMMIT="14ec60f248547d4d062a1160f0fc26d742319888"

if [[ ! -f "${SOURCE}/CMakeLists.txt" ]]; then
  echo "No existe el checkout OpenTTD: ${SOURCE}" >&2
  exit 1
fi

actual_commit="$(git -C "${SOURCE}" rev-parse HEAD)"
if [[ "${actual_commit}" != "${EXPECTED_COMMIT}" ]]; then
  echo "Commit OpenTTD inesperado: ${actual_commit}; se requiere ${EXPECTED_COMMIT}" >&2
  exit 1
fi

for file in opengfx.obg ogfx1_base.grf ogfxc_arctic.grf ogfxe_extra.grf \
  ogfxh_tropical.grf ogfxi_logos.grf ogfxt_toyland.grf; do
  if [[ ! -f "${BASESET}/${file}" ]]; then
    echo "Falta OpenGFX 8.0: ${BASESET}/${file}" >&2
    exit 1
  fi
done

cmake -S "${SOURCE}" -B "${BUILD}" -G "Unix Makefiles" \
  -DCMAKE_BUILD_TYPE=Release \
  -DOPTION_USE_ASSERTS=OFF
cmake --build "${BUILD}" --target openttd --parallel "${OPENTTD_BUILD_JOBS:-4}"

# `opntitle.dat` de 15.3 usa compresión LZMA. Una build gráfica puede enlazar
# correctamente y aun así no servir como oráculo si falta el loader.
if ! ldd "${BUILD}/openttd" | grep -q 'liblzma'; then
  echo "La build no tiene soporte LZMA; instala liblzma-dev y vuelve a configurarla." >&2
  exit 1
fi

mkdir -p "${BUILD}/baseset"
for file in opengfx.obg ogfx1_base.grf ogfxc_arctic.grf ogfxe_extra.grf \
  ogfxh_tropical.grf ogfxi_logos.grf ogfxt_toyland.grf; do
  cp "${BASESET}/${file}" "${BUILD}/baseset/${file}"
done

version_output="$(${BUILD}/openttd --version || true)"
version="${version_output%%$'\n'*}"
if [[ "${version}" != "OpenTTD 15.3" ]]; then
  echo "Versión inesperada: ${version}" >&2
  exit 1
fi

echo "Oráculo listo: ${BUILD}/openttd (${version})"
echo "Perfil aislado recomendado: XDG_DATA_HOME=/tmp/openttdrs-ref-data XDG_CONFIG_HOME=/tmp/openttdrs-ref-config"
