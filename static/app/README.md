# Icono de la aplicación

## Archivos

| Ruta | Uso |
|------|-----|
| `openttdrs-icon.png` | Fuente (alta resolución) |
| `icons/{16,32,48,64,128,256}x*.png` | Tema **hicolor** para el lanzador / dock (GNOME, KDE) |

Regenerar tamaños hicolor:

```bash
python3 scripts/gen_app_icons.py
```

## Icono de ventana (barra de título + muchos docks)

El cliente carga `static/app/openttdrs-icon.png`, lo escala a **128×128** y lo aplica con winit
(`AppIconPlugin` en `crates/openttdrs-client/src/app_icon.rs`). `StartupWMClass` / `Window.name`
deben ser **`openttdrs`** (ver `bevy_app.rs`).

Si al ejecutar con `cargo run -p openttdrs-client` ves un **engranaje gris** en el dock de Ubuntu:

En GNOME/Wayland el dock casi no usa el icono X11 (`set_window_icon`); asocia la
ventana al `.desktop` por `StartupWMClass=openttdrs` / `app_id` y muestra `Icon=openttdrs`.
Si `Exec=` apunta a un binario inexistente, GNOME ignora la entrada y cae al icono genérico.

1. Compila al menos una vez (`cargo build -p openttdrs-client`).
2. Instala iconos hicolor + `.desktop` (y un symlink en `~/.local/bin`):

```bash
bash scripts/install-desktop.sh
```

3. Cierra el juego, vuelve a lanzarlo y, si hace falta, reinicia GNOME Shell
   (`Alt+F2` → `r` en Xorg) o cierra sesión.

El cliente compila con features Bevy `wayland` + `x11` para que el `app_id` sea
`openttdrs` en sesiones Wayland nativas.

## Bandeja del sistema (tray)

En **Linux**, el cliente registra un icono StatusNotifierItem / AppIndicator con
`static/app/openttdrs-icon.png` (`TrayIconPlugin` + crate `ksni`).

- Clic: mostrar / enfocar la ventana.
- Menú: «Mostrar ventana» / «Salir».
- GNOME: suele hacer falta la extensión *AppIndicator and KStatusNotifierItem Support*.
- Sin sesión D-Bus (p. ej. headless): se omite con un warning; el juego sigue.

No hay tray en Windows/macOS todavía.
