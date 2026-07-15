# Handoff — menú de inicio (paridad OpenTTD)

**Estado (2026-07-12):** fases 1–2 cerradas (intro con tráfico, generate world avanzado,
escenarios/heightmap, audio y preferencias en menú).

**Relacionado:** [SIGUIENTES_PASOS.md](SIGUIENTES_PASOS.md), [FLUJO_MAPA_Y_CLIENTE.md](FLUJO_MAPA_Y_CLIENTE.md),
upstream `OpenTTD/src/intro_gui.cpp` (`SelectGameWindow`), `OpenTTD/src/genworld_gui.cpp`
(`GenerateLandscapeWindow`).

---

## 1. Situación actual vs OpenTTD

| Aspecto | OpenTTD | openttdrs |
|---------|---------|-----------|
| Pantallas | Menú principal + subventanas | Raíz / Nueva partida / Escenarios / Preferencias / Highscores / Salir |
| Cargar partida | Desde menú | Modal `SaveWindow` |
| Nueva partida | `GenerateLandscapeWindow` | Subpantalla: clima, tamaño, año, semilla, densidades, capital, relieve |
| Fondo | Intro game animado | Isla procedural + paneo + agua + tráfico (bus/camión/tren/barco) |
| Escenarios / heightmap | Nativo | `save/scenarios/` + `save/heightmaps/*.hmap` |
| Scenario editor | Toolbar 19 botones | MVP (#42 Fase 1): menú «Editor de escenarios» + sandbox |
| Audio | Theme + SFX | Theme menú + ventana Sonido/música |
| Preferencias | Resolución / idioma | Resolución persistida; idioma placeholder `es` |

---

## 2. Fase 2 — criterios de cierre

- [x] Fondo intro con vehículos/tráfico decorativo (8 rutas: bus, truck, train, ship)
- [x] Generate world: densidad ciudades/industrias, dinero inicial, relieve (llano/normal/montañoso)
- [x] Escenarios (carpeta `save/scenarios/`) + heightmaps ASCII `OTDRHMAP1`
- [x] Música / SFX accesibles desde el menú
- [x] Preferencias: resolución (1280×720 / 1600×900 / 1920×1080) + idioma placeholder

---

## 3. Heightmap `.hmap`

```
OTDRHMAP1
WIDTH HEIGHT
h00 h01 ... h0N
...
```

Alturas `0..=15`. Ejemplo: `save/heightmaps/isla_demo.hmap`.

---

*Última actualización: 2026-07-12*
