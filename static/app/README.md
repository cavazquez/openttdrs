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

Si al ejecutar con `cargo run -p openttdrs-client` sigues viendo un icono genérico en el **dock**:

1. Cierra instancias viejas del juego.
2. Instala el tema de iconos y el `.desktop`:

```bash
bash scripts/install-desktop.sh
```

3. Arranca desde el menú de aplicaciones **OpenTTDRS** o:

```bash
cargo run -p openttdrs-client
```

## Bandeja del sistema (tray)

No hay icono de **bandeja** (minimizar a tray): haría falta un crate aparte (`tray-icon`).
En GNOME el icono del dock es el del `.desktop` + `set_window_icon`, no un tray separado.
