#!/usr/bin/env bash
# Instala icono hicolor + .desktop para que GNOME/KDE muestren el icono en el dock.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICON_SRC="${ROOT}/static/app/openttdrs-icon.png"
DESKTOP_SRC="${ROOT}/packaging/openttdrs.desktop"
CLIENT_BIN="${ROOT}/target/release/openttdrs-client"

if [[ ! -f "${ICON_SRC}" ]]; then
  echo "Falta ${ICON_SRC}. Ejecuta desde la raíz del repo." >&2
  exit 1
fi

DATA_HOME="${XDG_DATA_HOME:-${HOME}/.local/share}"
ICON_DIR="${DATA_HOME}/icons/hicolor"
DESKTOP_DIR="${DATA_HOME}/applications"

install -d "${DESKTOP_DIR}"

for size in 16 32 48 64 128 256; do
  src="${ROOT}/static/app/icons/${size}x${size}.png"
  if [[ ! -f "${src}" ]]; then
    echo "Falta ${src}. Genera con: python3 scripts/gen_app_icons.py" >&2
    exit 1
  fi
  dest_dir="${ICON_DIR}/${size}x${size}/apps"
  install -d "${dest_dir}"
  install -m 0644 "${src}" "${dest_dir}/openttdrs.png"
done

tmp_desktop="$(mktemp)"
sed "s|^Exec=openttdrs-client|Exec=${CLIENT_BIN}|" "${DESKTOP_SRC}" >"${tmp_desktop}"
if [[ ! -x "${CLIENT_BIN}" ]]; then
  echo "Aviso: ${CLIENT_BIN} no existe; el .desktop usará openttdrs-client en PATH." >&2
  cp "${DESKTOP_SRC}" "${tmp_desktop}"
fi
install -m 0644 "${tmp_desktop}" "${DESKTOP_DIR}/openttdrs.desktop"
rm -f "${tmp_desktop}"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "${DESKTOP_DIR}" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "${ICON_DIR}" 2>/dev/null || true
fi

echo "Instalado icono en ${ICON_DIR} y ${DESKTOP_DIR}/openttdrs.desktop"
