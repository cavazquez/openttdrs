#!/usr/bin/env bash
# Instala icono hicolor + .desktop para que GNOME/KDE muestren el icono en el dock.
#
# En Ubuntu (GNOME + Wayland) el dock ignora a menudo `_NET_WM_ICON` de XWayland
# y solo usa Icon= del .desktop si asocia la ventana (StartupWMClass / Exec).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ICON_SRC="${ROOT}/static/app/openttdrs-icon.png"
DESKTOP_SRC="${ROOT}/packaging/openttdrs.desktop"

if [[ ! -f "${ICON_SRC}" ]]; then
  echo "Falta ${ICON_SRC}. Ejecuta desde la raíz del repo." >&2
  exit 1
fi

# Preferir release; si no, debug (desarrollo con cargo run).
CLIENT_BIN=""
for candidate in \
  "${ROOT}/target/release/openttdrs-client" \
  "${ROOT}/target/debug/openttdrs-client"
do
  if [[ -x "${candidate}" ]]; then
    CLIENT_BIN="${candidate}"
    break
  fi
done

DATA_HOME="${XDG_DATA_HOME:-${HOME}/.local/share}"
ICON_DIR="${DATA_HOME}/icons/hicolor"
DESKTOP_DIR="${DATA_HOME}/applications"
BIN_DIR="${HOME}/.local/bin"

install -d "${DESKTOP_DIR}"
install -d "${BIN_DIR}"

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
if [[ -n "${CLIENT_BIN}" ]]; then
  # Symlink estable: GNOME asocia por ruta de ejecutable al lanzar con cargo.
  ln -sfn "${CLIENT_BIN}" "${BIN_DIR}/openttdrs-client"
  sed \
    -e "s|^Exec=.*|Exec=${BIN_DIR}/openttdrs-client|" \
    -e "s|^TryExec=.*|TryExec=${BIN_DIR}/openttdrs-client|" \
    "${DESKTOP_SRC}" >"${tmp_desktop}"
  echo "Exec → ${BIN_DIR}/openttdrs-client → ${CLIENT_BIN}"
else
  echo "Aviso: no hay binario en target/{release,debug}; el .desktop usa openttdrs-client en PATH." >&2
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
if command -v desktop-file-validate >/dev/null 2>&1; then
  desktop-file-validate "${DESKTOP_DIR}/openttdrs.desktop" || true
fi

echo "Instalado icono en ${ICON_DIR} y ${DESKTOP_DIR}/openttdrs.desktop"
echo "Si el dock sigue con el icono genérico: cierra el juego, ejecuta de nuevo y reinicia GNOME Shell (Alt+F2 → r) o cierra sesión."
