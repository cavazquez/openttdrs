#!/usr/bin/env bash
# doctor.sh — Diagnóstico de entorno para compilar y jugar openttdrs.
#
# Uso:
#   ./scripts/doctor.sh           # chequeo completo (exit 1 si falta algo crítico)
#   ./scripts/doctor.sh --fix  # solo imprime comandos de instalación sugeridos
#   ./scripts/doctor.sh --quiet   # exit code únicamente (útil en CI/hooks)
#
# Fuente de verdad de paquetes de sistema: .github/apt-packages.txt
# (la misma lista que usa CI).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

FIX_ONLY=0
QUIET=0
for arg in "$@"; do
  case "$arg" in
    --fix) FIX_ONLY=1 ;;
    --quiet|-q) QUIET=1 ;;
    -h|--help)
      sed -n '2,12p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "Uso: $0 [--fix|--quiet|-h]" >&2
      exit 2
      ;;
  esac
done

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

ok=0
warn=0
fail=0
FIX_CMDS=()

log() { [[ "$QUIET" -eq 1 ]] || echo -e "$*"; }
pass() { ok=$((ok + 1)); [[ "$QUIET" -eq 1 ]] || echo -e "  ${GREEN}OK${NC}    $*"; }
soft() { warn=$((warn + 1)); [[ "$QUIET" -eq 1 ]] || echo -e "  ${YELLOW}WARN${NC}  $*"; }
need() {
  fail=$((fail + 1))
  [[ "$QUIET" -eq 1 ]] || echo -e "  ${RED}FAIL${NC}  $*"
}
suggest() { FIX_CMDS+=("$*"); }

have_cmd() { command -v "$1" &>/dev/null; }

# --- Rust ---
check_rust() {
  log "${CYAN}== Rust ==${NC}"
  local want
  want="$(grep -E '^\s*channel\s*=' rust-toolchain.toml | head -1 | sed -E 's/.*"([^"]+)".*/\1/')"
  if ! have_cmd rustup; then
    need "rustup no está en PATH"
    suggest "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    return
  fi
  pass "rustup $(rustup --version 2>/dev/null | head -1 | awk '{print $2}')"

  if ! have_cmd rustc || ! have_cmd cargo; then
    need "rustc/cargo no disponibles (instalá el toolchain del repo)"
    suggest "cd \"$ROOT\" && rustup show && rustup toolchain install \"$want\""
    return
  fi

  local got
  got="$(rustc --version | awk '{print $2}')"
  if [[ "$got" == "$want"* ]] || [[ "$got" == "$want" ]]; then
    pass "rustc $got (pedido: $want vía rust-toolchain.toml)"
  else
    # rustup override por rust-toolchain.toml debería activar el channel;
    # si no coincide, el build puede fallar por edition/MSRV.
    need "rustc activo es $got; el repo pide $want (rust-toolchain.toml)"
    suggest "cd \"$ROOT\" && rustup toolchain install $want && rustup component add rustfmt clippy --toolchain $want"
  fi

  if rustup component list --installed 2>/dev/null | grep -q '^rustfmt'; then
    pass "rustfmt instalado"
  else
    soft "rustfmt ausente (hace falta para ./scripts/check.sh)"
    suggest "rustup component add rustfmt"
  fi
  if rustup component list --installed 2>/dev/null | grep -q '^clippy'; then
    pass "clippy instalado"
  else
    soft "clippy ausente (hace falta para ./scripts/check.sh)"
    suggest "rustup component add clippy"
  fi

  if have_cmd cargo-nextest; then
    pass "cargo-nextest presente"
  else
    soft "cargo-nextest opcional (CI lo usa; check.sh cae a cargo test)"
    suggest "cargo install cargo-nextest --locked"
  fi
}

# --- Paquetes APT (misma lista que CI) ---
check_apt() {
  log "${CYAN}== Paquetes de sistema (APT) ==${NC}"
  if ! have_cmd dpkg; then
    soft "dpkg no disponible — omito chequeo APT (¿no-Debian?)"
    soft "Instalá a mano equivalentes de: .github/apt-packages.txt"
    return
  fi

  local missing=()
  local pkg
  while IFS= read -r pkg; do
    [[ -z "$pkg" || "$pkg" =~ ^# ]] && continue
    if dpkg -s "$pkg" &>/dev/null; then
      pass "$pkg"
    else
      need "falta paquete APT: $pkg"
      missing+=("$pkg")
    fi
  done < .github/apt-packages.txt

  if [[ ${#missing[@]} -gt 0 ]]; then
    suggest "sudo apt-get update && sudo apt-get install -y ${missing[*]}"
  fi
}

# --- pkg-config (lo que suelen romper al linkear Bevy) ---
check_pkgconfig() {
  log "${CYAN}== pkg-config (headers de enlace) ==${NC}"
  if ! have_cmd pkg-config; then
    need "pkg-config no está en PATH"
    # apt check ya sugiere el paquete
    return
  fi
  pass "pkg-config $(pkg-config --version)"

  # Nombre .pc → para qué sirve
  local -A libs=(
    [x11]="libx11-dev"
    [xcursor]="libxcursor-dev"
    [xi]="libxi-dev"
    [xkbcommon]="libxkbcommon-dev"
    [wayland-client]="libwayland-dev"
    [alsa]="libasound2-dev"
    [openssl]="libssl-dev"
  )
  local name pkg
  for name in "${!libs[@]}"; do
    pkg="${libs[$name]}"
    if pkg-config --exists "$name" 2>/dev/null; then
      pass "pkg-config $name"
    else
      need "pkg-config no encuentra '$name' (paquete: $pkg)"
    fi
  done
}

# --- Herramientas para descargar/procesar assets ---
check_asset_tools() {
  log "${CYAN}== Herramientas de assets ==${NC}"
  local cmd
  for cmd in curl tar unzip; do
    if have_cmd "$cmd"; then
      pass "$cmd"
    else
      need "falta comando: $cmd"
      suggest "sudo apt-get install -y $cmd"
    fi
  done
  if have_cmd grfcodec; then
    pass "grfcodec (necesario para ./scripts/descargar_graficos.sh)"
  else
    need "falta grfcodec (sin esto no se generan assets/opengfx/tiles)"
    suggest "sudo apt-get install -y grfcodec"
  fi
  if have_cmd python3; then
    pass "python3 $(python3 --version 2>&1 | awk '{print $2}')"
    # pip es opcional (alternativa en distros sin paquetes APT de numpy/Pillow).
    # No se exige ni se reporta WARN si falta.
    local have_pip=0
    if python3 -m pip --version &>/dev/null; then
      have_pip=1
      pass "python3 -m pip ($(python3 -m pip --version 2>/dev/null | awk '{print $2}')) [opcional]"
    fi
    # Módulos usados por descargar_graficos.sh / gen_tile_select.py / gen_tile_atlas.py
    local mod
    local py_missing=()
    for mod in numpy PIL; do
      if python3 -c "import ${mod}" 2>/dev/null; then
        pass "python3: import ${mod}"
      else
        need "falta módulo Python '${mod}' (post-proceso de gráficos)"
        py_missing+=("$mod")
      fi
    done
    if [[ ${#py_missing[@]} -gt 0 ]]; then
      # Preferir APT en Ubuntu/Debian (no necesita pip).
      suggest "sudo apt-get install -y python3-numpy python3-pil"
      # Alternativa multi-distro vía pip (solo si ya hay pip, o indicar cómo).
      if [[ "$have_pip" -eq 1 ]]; then
        suggest "python3 -m pip install --user -r scripts/requirements-assets.txt"
      else
        suggest "# alternativa sin APT: instalá pip de tu distro y luego: python3 -m pip install --user -r scripts/requirements-assets.txt"
      fi
    fi
  else
    soft "python3 ausente (scripts de parse_sav / auditoría / gráficos)"
    suggest "sudo apt-get install -y python3   # o el equivalente de tu distro"
  fi
}

# --- Assets del juego ---
check_assets() {
  log "${CYAN}== Assets del juego ==${NC}"
  local tiles="$ROOT/assets/opengfx/tiles"
  local sounds="$ROOT/assets/sounds"
  local music="$ROOT/assets/music"
  local font="$ROOT/static/fonts/DejaVuSansMono.ttf"

  if [[ -d "$tiles" ]]; then
    local n
    n="$(find "$tiles" -type f -name '*.png' 2>/dev/null | wc -l | tr -d ' ')"
    if [[ "$n" -ge 100 ]]; then
      pass "OpenGFX tiles: $n PNG en assets/opengfx/tiles"
    elif [[ "$n" -gt 0 ]]; then
      soft "OpenGFX tiles incompletos ($n PNG). Suele indicar descarga a medias."
      suggest "./scripts/descargar_assets.sh graficos --32bpp"
    else
      need "assets/opengfx/tiles vacío"
      suggest "./scripts/descargar_assets.sh graficos --32bpp"
    fi
    if [[ -f "$tiles/grass.png" ]]; then
      pass "sprite clave: grass.png"
    else
      need "falta assets/opengfx/tiles/grass.png"
      suggest "./scripts/descargar_assets.sh graficos --32bpp"
    fi
    # Señales de post-proceso incompleto (p. ej. falló gen_tile_select por falta de numpy)
    if [[ -f "$tiles/tile_select.png" ]]; then
      pass "post-proceso: tile_select.png"
    else
      soft "falta tile_select.png (post-proceso de descargar_graficos interrumpido)"
      suggest "./scripts/descargar_assets.sh graficos --32bpp"
    fi
    if [[ -d "$ROOT/assets/opengfx/atlas" ]] && [[ -n "$(find "$ROOT/assets/opengfx/atlas" -type f 2>/dev/null | head -1)" ]]; then
      pass "texture atlas presente"
    else
      soft "atlas vacío/ausente (último paso: gen_tile_atlas.py)"
      suggest "./scripts/descargar_assets.sh graficos --32bpp"
    fi
  else
    need "no existe assets/opengfx/tiles (gráficos no descargados)"
    suggest "./scripts/descargar_assets.sh graficos --32bpp"
  fi

  if [[ -d "$sounds" ]] && [[ "$(find "$sounds" -type f 2>/dev/null | wc -l | tr -d ' ')" -gt 0 ]]; then
    pass "sonidos presentes en assets/sounds"
  else
    soft "assets/sounds vacío (en el repo deberían venir; regenerar con descargar_assets)"
    suggest "./scripts/descargar_assets.sh sonidos"
  fi

  if [[ -d "$music" ]] && [[ "$(find "$music" -type f 2>/dev/null | wc -l | tr -d ' ')" -gt 0 ]]; then
    pass "música presente en assets/music"
  else
    soft "assets/music vacío (opcional para jugar)"
    suggest "./scripts/descargar_assets.sh musica"
  fi

  if [[ -f "$font" ]]; then
    pass "fuente UI: static/fonts/DejaVuSansMono.ttf"
  else
    need "falta static/fonts/DejaVuSansMono.ttf"
  fi
}

print_fix() {
  if [[ ${#FIX_CMDS[@]} -eq 0 ]]; then
    log "${GREEN}Nada que instalar.${NC}"
    return
  fi
  log ""
  log "${CYAN}== Comandos sugeridos (en orden) ==${NC}"
  # Deduplicar preservando orden
  local -A seen=()
  local cmd
  for cmd in "${FIX_CMDS[@]}"; do
    [[ -n "${seen[$cmd]+x}" ]] && continue
    seen[$cmd]=1
    log "  $cmd"
  done
  log ""
  log "Después: ${CYAN}./scripts/doctor.sh${NC} otra vez, y si todo OK:"
  log "  ${CYAN}cargo run -p openttdrs-client${NC}"
}

# --- main ---
if [[ "$FIX_ONLY" -eq 1 ]]; then
  # Recolectar sin ruido y solo imprimir fixes
  QUIET=1
fi

check_rust
check_apt
check_pkgconfig
check_asset_tools
check_assets

if [[ "$FIX_ONLY" -eq 1 ]]; then
  QUIET=0
  print_fix
  exit "$fail"
fi

log ""
log "Resumen: ${GREEN}${ok} ok${NC}, ${YELLOW}${warn} warn${NC}, ${RED}${fail} fail${NC}"
if [[ "$fail" -gt 0 ]]; then
  print_fix
  exit 1
fi
if [[ "$warn" -gt 0 ]]; then
  soft "Hay avisos no bloqueantes; el cliente debería poder arrancar."
  print_fix
fi
log "${GREEN}Entorno listo para cargo run -p openttdrs-client${NC}"
exit 0
