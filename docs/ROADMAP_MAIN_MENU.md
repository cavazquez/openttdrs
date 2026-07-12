# Handoff — menú de inicio (paridad OpenTTD)

**Estado (2026-06-22):** fase 1 cerrada; **fase 2.1** — fondo intro con mapa isla procedural,
cámara con paneo y agua animada. Pendiente: opciones avanzadas de generación.

**Relacionado:** [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md), [FLUJO_MAPA_Y_CLIENTE.md](FLUJO_MAPA_Y_CLIENTE.md),
upstream `OpenTTD/src/intro_gui.cpp` (`SelectGameWindow`), `OpenTTD/src/genworld_gui.cpp`
(`GenerateLandscapeWindow`).

---

## 1. Situación actual vs OpenTTD

| Aspecto | OpenTTD | openttdrs (antes) | openttdrs (fase 1) |
|---------|---------|-------------------|---------------------|
| Pantallas | Menú principal + subventanas | Panel único mezclado | Raíz / Nueva partida / Salir |
| Cargar partida | Desde menú | Solo in-game (toolbar) | Modal `SaveWindow` en menú |
| Nueva partida | `GenerateLandscapeWindow` | Opciones en mismo panel | Subpantalla dedicada |
| Demo / tutorial | Escenarios | Botón demo clásica | Raíz → demo plana |
| Fondo | Intro game animado | Gris plano | Mapa isla + paneo + agua |
| Salir | Confirmación | Esc / botón directo | Diálogo confirmar |

---

## 2. Arquitectura cliente

```
ClientScreen::MainMenu
  OnEnter → setup_main_menu_intro (mapa + cámara), setup_main_menu, setup_save_window
  Update  → pan_main_menu_intro_camera, main_menu_*, save_window_* (Load)

ClientScreen::InGame
  OnEnter → setup UI juego (incl. save_window si no existía)
  Update  → save/load toolbar + modal (Save + Load)
```

Recursos:

- `MainMenuPanel` — `Root` | `NewGame` | `QuitConfirm`
- `NewGameSettingsResource` — clima, procedural, isla, demo, seed
- `SaveWindowState` — reutilizado; abrir en `Load` desde botón «Cargar partida»
- Tras `confirm_load` en menú → `leave_main_menu` → `ClientScreen::InGame` (sin `from_new_game`)

Código principal: `crates/openttdrs-client/src/ui/main_menu.rs`,
`crates/openttdrs-client/src/ui/main_menu_intro.rs`,
`crates/openttdrs-client/src/ui/save_window/`.

---

## 3. Fase 1 — criterios de cierre

- [x] Documento handoff (este archivo)
- [x] Pantalla raíz: Nueva partida, Cargar, Demo clásica, Salir
- [x] Subpantalla Nueva partida: clima, toggles, resumen, Iniciar, Volver
- [x] Cargar partida abre modal existente; confirmar entra al juego
- [x] Esc: cierra modal → vuelve atrás en subpantallas → confirmación salir en raíz
- [x] Toggle «zona demo/tutorial» solo con `OPENTTDRS_DEV`
- [x] Tests `setup_main_menu` / panel visibility

---

## 4. Fase 2 — backlog

1. **Fondo intro** — [x] mapa procedural isla fija + paneo suave + agua animada (`main_menu_intro.rs`).
   Pendiente: vehículos/tráfico en intro como OpenTTD upstream.
2. **Generate world** — [x] tamaño mapa (demo 24×18 + ancho/alto 64…4096 como OpenTTD), año inicio (1950–2020), semilla ±/[].
   Pendiente: densidad ciudades/industrias, dinero inicial, relieve avanzado.
3. **Escenarios / heightmap** — fuera de 0.1; ver [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md).
4. **Música / SFX menú** — cuando exista pipeline de audio UI.
5. **Preferencias en menú** — resolución, idioma (hoy vía env / settings in-game).

---

## 5. Atajos (fase 1)

| Tecla | Raíz | Nueva partida | Modal cargar | Confirmar salir |
|-------|------|---------------|--------------|-----------------|
| Enter / Espacio | — | Iniciar | — | — |
| Esc | Confirmar salir | Volver | Cerrar modal | Cancelar |
| 1–4 | — | Clima | — | — |

---

## 6. Verificación manual

```bash
cargo run -p openttdrs-client
```

1. Raíz → «Nueva partida» → elegir clima/toggles → Iniciar → juego procedural.
2. Raíz → «Cargar partida» → elegir `.json` o `.sav` → Cargar → mismo estado en juego.
3. Raíz → «Demo clásica» → mapa plano con zona tutorial.
4. Raíz → Esc o Salir → confirmar → cierra app; cancelar → vuelve al menú.
5. Sin `OPENTTDRS_DEV`: no aparece toggle «zona demo/tutorial» en Nueva partida.

---

*Última actualización: 2026-06-22 (fase 2.1 fondo intro)*
