# Roadmap de importación desde OpenTTD

**Fecha:** 2026-07-05  
**Alcance:** animaciones, sonido, música y dinámicas de juego importables del original (`OpenTTD/src/`) al port Rust/Bevy (`openttdrs/crates/`).

**Relacionado:** [PARIDAD_OPENTTD.md](PARIDAD_OPENTTD.md), [ROADMAP_SPRINTS.md](ROADMAP_SPRINTS.md), [parity/rail_status.md](parity/rail_status.md), [SPRITES_OPENGFX.md](SPRITES_OPENGFX.md).

---

## Resumen: qué ya tenemos vs. qué falta

El port tiene un **core jugable** (transporte carretera/rail, industrias, economía básica, señales v1, IA rival #86) y un **cliente Bevy con animaciones visuales fieles** (agua, humo, paletas). Los grandes huecos son **música**, **sonido completo**, **CargoDist** y **NewGRF runtime**.

| Área | Nivel actual | Gap principal |
|------|--------------|---------------|
| Animaciones visuales | Avanzado + humo trenes/FX | NewGRF tile callbacks completos |
| Sonido | Catálogo 73 SFX + mixer 8 ch + motores | Ambiente / NewGRF Action11 |
| Música | MusicPlugin + script OGG | Playlist completa OpenMSX |
| Dinámicas de juego | Préstamos, ciudades, averías, subsidios, desastres, IA rival (#86), CargoDist MCF nivel 2 + LGRP | NewGRF runtime |

---

## 1. Animaciones y efectos visuales

Sorprendentemente, esta es el área **más avanzada** del port. Ya está portado: ciclo de paleta del agua, fuego de refinería, burbujas de fábrica, humo de central eléctrica y mina de cobre, animación de edificios de industria, remapeo de color por compañía.

### 1.1 Lo que se puede importar del original

| Efecto | Sistema en C++ | Complejidad | Estado port |
|--------|----------------|-------------|-------------|
| **Humo de locomotoras** (vapor/diésel/chispas eléctricas) | `effectvehicle.cpp` | Media | **Hecho** — sprites `3073–3089` (`gen_effect_vehicle_sprites.py`, `render/train_smoke.rs`) |
| **Explosiones / humo de avería** | `effectvehicle.cpp:152-253` | Media | **Hecho** — sprites `3709–3724` / `3737–3740` (`render/effect_fx.rs`) |
| **Bulldozer en obras** | `effectvehicle.cpp:255-325` | Baja | **Hecho** (road works FX) |
| **Animación de estaciones/aeropuertos** | `newgrf_station.cpp` | Media | **OK radar** (`airport_radar_anim.rs` + `step_airport_tiles` m7) |
| **Ascensor de edificios de ciudad** | `town_cmd.cpp:346-368` | Baja | **MVP** (`HouseLiftAnimPlugin`, s2 1442/4569) |
| **Cursores animados** | `table/animcursors.h` | Baja | **MVP demolish** (4 frames + `anim_cursor_frame`) |
| **Árboles creciendo / cultivos** | `tree_cmd.cpp:679` | Baja–Media | **Hecho** (sim + render) |
| **Scroll suave del viewport** | `viewport.cpp:1947` | Media | **Hecho** (lerp 300 ms) |

### 1.2 Ya implementado en el cliente

| Efecto | Módulo Rust | Archivo |
|--------|-------------|---------|
| Ciclo de paleta del agua | `WaterAnimationPlugin` | `render/water.rs` |
| Llama de refinería | `RefineryFireAnimPlugin` | `render/refinery_fire.rs` |
| Burbujas fábrica de bebidas | `FizzyDrinkAnimPlugin` | `render/fizzy_drink.rs` |
| Faro / luces de estadio | `LighthouseAnimPlugin` | `render/lighthouse_anim.rs` |
| Humo chimenea / mina cobre | `IndustrySmokePlugin` | `render/smoke.rs` |
| Animación edificios industria | `IndustryBuildingAnimPlugin` | `render/industry_anim.rs` |
| Radar aeropuerto (m7) | `AirportRadarAnimPlugin` | `render/airport_radar_anim.rs` |
| Overlays draw_proc (chispas, burbujas) | `IndustryDrawProcPlugin` | `render/industry_draw_proc.rs` |
| Tween sprites fantasma construcción | ghost lerp | `ui/toolbar/preview/ghost_lerp.rs` |
| Popups de ingreso animados | income popup | `ui/hud/income_popup.rs` |
| Paleta por compañía | recolor | `sprites/company_palette.rs`, `render/company_recolor.rs` |
| Tile loop industrial (sim) | anim frames | `map/industry_tile_anim.rs` |

### 1.3 Notas del original (no son “faltantes”)

| Fenómeno | Comportamiento OpenTTD |
|----------|------------------------|
| Lluvia / copos de nieve | **No existen** como partículas; el clima es estado de tesela + línea de nieve |
| Fades de ventana UI | **No existen**; redraw inmediato con borde blanco al activar |
| Balanceo de vehículos | **No hay** sway/roll; solo ajuste Z en pendiente |

### 1.4 Framework de animación de teselas (referencia)

| Subsistema | Clase C++ | Archivo original |
|------------|-----------|------------------|
| Lista global de teselas animadas | `_animated_tiles` | `animated_tile.cpp` |
| Industrias | `IndustryAnimationBase` | `industry_cmd.cpp:693` |
| Estaciones / roadstops | `StationAnimationBase` | `newgrf_station.cpp` |
| Aeropuertos | `AirportTileAnimationBase` | `newgrf_airporttiles.cpp` |
| Casas | `HouseAnimationBase` | `newgrf_house.cpp` |
| Objetos (faros, antenas) | `ObjectAnimationBase` | `newgrf_object.cpp` |

**Complejidad global del framework NewGRF:** Alta (callbacks, triggers, `m7` frame counter).

---

## 2. Sonido

Estado actual: **73 SFX** vía `SoundId` + mixer de 8 canales (`audio/world_sfx.rs`); motores por `motion_counter` / `VehicleRunning`; 6 WAV HUD heredados; script `preparar_sonidos_opensfx.sh` genera `snd_00`…`snd_72`.

### 2.1 Arquitectura del original

| Componente | Archivo OpenTTD | Descripción |
|------------|-----------------|-------------|
| Mixer (8 canales) | `mixer.cpp:43` | Resampling, volumen estéreo, mezcla con música |
| Reproducción SFX | `sound.cpp` | `StartSound`, `SndPlayFx/TileFx/VehicleFx` |
| Paneo por viewport | `sound.cpp:203` | `SndPlayScreenCoordFx` |
| Catálogo (73 sonidos) | `sound_type.h:46-122` | Enum `SoundFx` / `SoundID` |
| Carga baseset | `sound.cpp:27` | `.obs` → `samples.cat` (OpenSFX) |
| Pool NewGRF | `newgrf_sound.cpp` | Sonidos custom en `.grf` |

### 2.2 Catálogo SFX por categoría (importables)

| Categoría | Sonidos ejemplo | Disparo en original |
|-----------|-----------------|---------------------|
| Construcción | `SND_20_CONSTRUCTION_RAIL`, `SND_1F_CONSTRUCTION_OTHER` | `rail_gui.cpp`, `road_gui.cpp` |
| Demolición | `SND_12_EXPLOSION` | `main_gui.cpp`, `terraform_gui.cpp` |
| GUI | `SND_15_BEEP` (click/confirm) | `sound.cpp:254` |
| Economía | `SND_14_CASHTILL` (ingreso carga) | `economy.cpp:1193` |
| Noticias | `SND_16_NEWS_TICKER`, `SND_1D_APPLAUSE`, `SND_1E_NEW_ENGINE` | `news_gui.cpp` |
| Año bueno/malo | `SND_00_GOOD_YEAR`, `SND_01_BAD_YEAR` | `company_cmd.cpp:826` |
| Desastres | `SND_12_EXPLOSION`, `SND_13_TRAIN_COLLISION` | `disaster_vehicle.cpp`, `train_cmd.cpp` |
| Ambiente | `SND_0E_LEVEL_CROSSING`, `SND_21_ROAD_WORKS`, pájaros/selva | `train_cmd.cpp`, `tree_cmd.cpp` |
| Industrias | sonidos mina, central, aserradero (aleatorio) | `industry_cmd.cpp:1167` |

### 2.3 Sonidos de vehículos

| Tipo | Evento | Archivo original |
|------|--------|------------------|
| Tren — salida estación | Vapor / diésel / monorail / maglev | `train_cmd.cpp:2273` |
| Tren — túnel | `SND_05_TRAIN_THROUGH_TUNNEL` | `tunnelbridge_cmd.cpp:1979` |
| Carretera — motor | `RoadVehInfo->sfx` | `roadveh_cmd.cpp:610` |
| Avión — despegue/aterrizaje | por tipo de motor | `aircraft_cmd.cpp:585` |
| Claxon | reutiliza salida de estación (`force=true`) | `vehicle_gui.cpp:3379` |
| Motor en marcha | `VSE_RUNNING` cada tick | `vehicle.cpp:1037` |
| Avería | fallback por landscape/tipo | `vehicle.cpp:1398` |

### 2.4 Estado en el port

| Aspecto | Estado | Ubicación |
|---------|--------|-----------|
| Dependencia audio | `bevy` + `bevy_audio` + `wav` | `openttdrs-client/Cargo.toml` |
| SFX HUD (5 tipos) | Implementado | `ui/hud/sound_ping.rs` |
| SFX mundo (73 `SoundId`) | Implementado | `sound_id.rs`, `audio/sim_events.rs` |
| Audio espacial (paneo por cámara) | Implementado | `audio/world_sfx.rs` |
| Volumen SFX / música | `sfx_volume`, `music_volume` | `settings.rs`, ventana **Audio...** |
| Flags granulares | `sound_vehicle/ambient/disaster/confirm/click_beep` | `settings.rs`, `audio_settings_window.rs` |
| OpenSFX metadatos | En repo | `assets/opensfx/opensfx-1.0.3/` |
| WAV runtime | Scripts HUD + OpenSFX (73) | `preparar_sonidos_hud.sh`, `preparar_sonidos_opensfx.sh` |
| Eventos cruce / salida tren | `LevelCrossing`, `VehicleDepart` | `sim_step.rs`, `map/level_crossing.rs` |
| Mixer 8 canales estilo original | Implementado (MVP Bevy) | `audio/world_sfx.rs` `SfxMixer` |
| Catálogo 73 SFX completo | Implementado | `sound_id.rs` + script |
| Motores en marcha por tick | Implementado (MVP) | `motion_counter` + `SimEvent::VehicleRunning` |

### 2.5 Mapeo HUD actual → OpenTTD

| `HudSfxKind` (port) | Analogía OpenTTD |
|---------------------|------------------|
| `ClickBeep` | `SND_15_BEEP` / `sound.click_beep` |
| `Error` | — (no directo; beep extra del port) |
| `NewsTicker` | `SND_16_NEWS_TICKER` |
| `NewsApplause` | `SND_1D_APPLAUSE` |
| `NewsChime` | `SND_1E_NEW_ENGINE` |

Ingreso de carga (`SND_14_CASHTILL`) y construcción en mapa van por `SimEvent` → `PlayWorldSfx`, no por HUD.

**Complejidad de portado:** Media. **Assets:** OpenSFX (GPL, libre) — ya parcialmente en `assets/opensfx/`.

---

## 3. Música

**Estado:** `MusicPlugin` reproduce OGG pre-decodificado (OpenMSX vía `descargar_musica.sh` + `fluidsynth`/`ffmpeg`). Volumen `music_volume` separado de SFX; sin UI play/pause/skip ni playlists completas.

### 3.1 Sistema del original

| Componente | Archivo OpenTTD | Descripción |
|------------|-----------------|-------------|
| Baseset OpenMSX | `music.cpp:71` | 31 slots: `theme`, `old_0..9`, `new_0..9`, `ezy_0..9` |
| Manifiesto | `.obm` | Metadatos, `[catindex]`, `[timingtrim]` |
| Formatos | `base_media_music.h` | MIDI estándar o MPS/CAT (DOS) |
| Playlists | `music_gui.cpp:40` | All / Old / New / Ezy / Custom1/2 / Theme |
| Drivers | `music/fluidsynth.cpp`, `extmidi.cpp`, etc. | Síntesis o proceso externo |
| Mezcla con SFX | `mixer.cpp:236` | Música en mismo buffer que efectos |

### 3.2 Playlists

| Playlist | Contenido |
|----------|-----------|
| `PLCH_ALLMUSIC` | theme + 30 pistas |
| `PLCH_OLDSTYLE` | `old_0`…`old_9` |
| `PLCH_NEWSTYLE` | `new_0`…`new_9` |
| `PLCH_EZYSTREET` | `ezy_0`…`ezy_9` |
| `PLCH_CUSTOM1/2` | hasta 32 pistas cada una |
| `PLCH_THEMEONLY` | menú principal, loop |

### 3.3 Estado en el port

| Aspecto | Estado |
|---------|--------|
| Reproducción OGG en juego | **Hecho** (`audio/music.rs`) |
| Playlists / shuffle OpenMSX | **Falta** |
| Volumen música separado de SFX | **Hecho** (`music_volume` + ventana Audio; sync en caliente en `music.rs`) |
| Script descarga OpenMSX | Existe | `scripts/descargar_musica.sh` |
| Assets OpenMSX en repo | Gitignored; generar con script |
| Controles play/pause/skip en UI | **Falta** |

### 3.4 Atajo pragmático para Bevy

| Enfoque | Complejidad | Nota |
|---------|-------------|------|
| Pre-decodificar MIDIs OpenMSX → OGG/WAV | Media | Evita FluidSynth embebido; script one-shot |
| Crate MIDI + SoundFont en runtime | Alta | Paridad fiel, dependencia SoundFont (licencia propia) |
| Solo theme en menú | Baja | Primer hito jugable |

**Complejidad global:** Alta. **Assets:** OpenMSX (GPL, libre).

---

## 4. Dinámicas de juego

Inventario de mecánicas del original cruzado con `openttdrs-core`. Estados: **EXISTE**, **PARCIAL**, **FALTA**.

### 4.1 Tabla principal

| Dinámica | Referencia OpenTTD | Estado port | Prioridad sugerida |
|----------|-------------------|-------------|-------------------|
| **Crecimiento de ciudades + autoridad local** | `town_cmd.cpp:890-4190` | Parcial (rating, publicidad, fondos UI) | ⭐ Alta |
| **Préstamos, intereses, quiebra** | `economy.cpp:799`, `misc_cmd.cpp:41` | Parcial→casi completo (préstamos + compra rival en quiebra) | ⭐ Alta |
| **Averías + fiabilidad + servicio** | `vehicle.cpp:1303-1492` | Parcial (averías sim + servicio depósito) | ⭐ Alta |
| **Subsidios** | `subsidy.cpp` | Parcial→casi (noticias/SFX + compañía adjudicada) | Media |
| **Decaimiento carga en estación + ratings** | `station_cmd.cpp:3959` | Parcial→casi (rating por compañía + gate urbana) | Media |
| **Desastres** (UFO, accidentes, submarinos) | `disaster_vehicle.cpp` | Parcial→casi (noticias + toggle nueva partida) | Media |
| **Árboles** (crecer / talar / plantar) | `tree_cmd.cpp` | Parcial (`tree_tile_loop.rs`) | Baja–Media |
| **IA de compañías rivales** | `ai/` (Squirrel) | TransCargo Rust ✅ (`archive/epics/ai_rivals.md`) | Baja (Squirrel OOS) |
| **Barcos y aviones** | `ship_cmd.cpp`, `aircraft_cmd.cpp` | Parcial (movimiento básico) | Media |
| **NewGRF (mods)** | `newgrf.cpp` + ecosistema | Falta | Fuera de alcance actual |

### 4.2 Detalle por bloque

#### Economía avanzada

| Mecánica | Original | Port |
|----------|----------|------|
| Pago por distancia/tránsito | `economy.cpp:952` | **EXISTE** (`economy.rs`) |
| Inflación ingresos/precios | `economy.cpp:695` | **PARCIAL** |
| Costes operativos | `economy.cpp:644` | **EXISTE** (`sim_step.rs`) |
| Préstamos pedir/devolver | `misc_cmd.cpp:41` | **PARCIAL** (`command/economy.rs`, `finances_window.rs`) |
| Intereses mensuales | `economy.cpp:799` | **PARCIAL** (`sim_step.rs`) |
| Quiebra / compra rivales | `company_cmd.cpp:546` | **EXISTE** (`BuyCompany` + `bankruptcy_months` / streak) |
| Subsidios en pagos | `subsidy.cpp` | **EXISTE** (`subsidy.rs`; ×2 solo compañía adjudicada) |
| Valoración trimestral compañía | `economy.cpp:637` | **EXISTE** (`economy_quarterly.rs`) |

#### Desastres y averías

| Mecánica | Original | Port |
|----------|----------|------|
| Desastres ambientales (UFO, zeppelin, etc.) | `disaster_vehicle.cpp:939` | **PARCIAL** (`disaster.rs` + noticias; sin vehículo animado) |
| Breakdowns vehículos | `vehicle.cpp:1303` | **PARCIAL** (`vehicle.rs`, `sim_step.rs`) |
| Choques de trenes | `train_cmd.cpp:3205` | **EXISTE** (`train_collision.rs`; `force_proceed` puede forzar) |
| Servicio en depósito vs fiabilidad | `vehicle.cpp:187` | **PARCIAL** (`service_at_depot`) |

#### Ciudades

| Mecánica | Original | Port |
|----------|----------|------|
| Demanda pasajeros/correo | `town_cmd.cpp:522` | **PARCIAL** (`town.rs`) |
| Expansión física (casas, calles) | `town_cmd.cpp:1184` | **OK MVP** (`town_expand.rs` + `grow_town_if_served`) |
| Rating autoridad local | `town_cmd.cpp:3257` | **PARCIAL** (`town.rs`, estaciones) |
| Acciones de ciudad (publicidad, fondos, vías) | `town_cmd.cpp:3421` | **PARCIAL** (publicidad/fondos UI) |
| Metas de carga para crecer | `town_cmd.cpp:3916` | **EXISTE** (`town.rs` goals/received/is_growing) |

#### Vehículos (envejecimiento)

| Mecánica | Original | Port |
|----------|----------|------|
| Autoreemplazo en depósito | `vehicle.cpp:695` | **PARCIAL** (`autoreplace.rs`) |
| Edad calendario | `vehicle.cpp:1440` | **PARCIAL** (`vehicle_age_years`) |
| Fiabilidad dinámica | `vehicle.cpp:1318` | **PARCIAL** (`check_breakdown`) |
| Órdenes de servicio / revisión | `vehicle.cpp:210` | **EXISTE** (`requires_service` + skip depósito) |

#### Clima

| Mecánica | Original | Port |
|----------|----------|------|
| 4 climas (LandscapeType) | `landscape.h` | **PARCIAL** (`world_gen.rs` `Climate`) |
| Nieve por altura / tile-loop | `clear_cmd.cpp` `TileLoopClearAlps` | **EXISTE** (#196: franja + `DEF_SNOW_LINE_HEIGHT`; NewGRF snow table OOS) |
| Zonas desierto/selva tropical | `landscape.cpp:984` | **PARCIAL** |
| Industrias por clima | `industry_cmd.cpp` | **EXISTE** (`industry.rs`) |

#### Cargo

| Mecánica | Original | Port |
|----------|----------|------|
| 6 tipos básicos | `cargotype.h` | **EXISTE** (`cargo.rs`) |
| Cadena fábrica (madera+carbón→goods) | `industry_cmd.cpp` | **PARCIAL** |
| Envejecimiento en vehículo | `cargopacket.cpp` | **EXISTE** (`cargo_transit_ticks`) |
| Decaimiento en estación | `station_cmd.cpp:3959` | **EXISTE** (edad + truncate; rating por compañía + gate pax) |
| Link graph / flow stats | `linkgraph/` | **EXISTE** (`linkgraph_parity/` + `sav/linkgraph` LGRP + overlay; LGRJ async OOS) |

#### Puentes y túneles

| Mecánica | Original | Port |
|----------|----------|------|
| 13 tipos puente (specs) | `bridge_land.h` | **EXISTE** (`bridge_spec.rs`) |
| Construcción rail/road | `tunnelbridge_cmd.cpp` | **PARCIAL** (`bridge.rs`) |
| Límite velocidad en puente | specs → movimiento | **HECHO** (`bridge_max_speed_for_tile` + `step_with_map`) |
| Ocultamiento tren en túnel | `_tunnel_visibility_frame` | **EXISTE** (`vehicle_hidden_in_tunnel` + render) |

---

## 5. Estado actual del port (inventario)

### 5.1 Core (`openttdrs-core`)

| Sistema | Nivel | Módulo |
|---------|-------|--------|
| Simulación por tick | ✅ | `sim_step.rs` |
| Economía básica (pago, inflación, costes) | ✅ Parcial | `economy.rs` |
| Industrias + producción | ✅ | `industry.rs` |
| Vehículos + órdenes + horarios | ✅ Parcial | `vehicle.rs`, `timetable.rs` |
| Pathfinding road/rail | ✅ | `pathfinder.rs` |
| Señales ferroviarias v1 | ✅ | `rail_signals.rs` |
| Autoreemplazo depósito | ✅ Parcial | `autoreplace.rs` |
| Noticias | ✅ | `news.rs` |
| Save JSON + import `.sav` | ✅ | `save.rs`, `sav/` |
| Paridad headless (trazas) | ✅ | `parity/` |
| Ciudades (crecimiento, rating, acciones) | ✅ Parcial | `town.rs`, `command/town.rs` |
| Subsidios | ✅ Parcial | `subsidy.rs` |
| Desastres | ✅ Parcial | `disaster.rs` |
| Préstamos activos | ✅ Parcial | `economy.rs`, `command/economy.rs` |
| IA rivales | ✅ (#86) | `docs/archive/epics/ai_rivals.md` |
| Barcos / aviones | ✅ Parcial | `ship_movement.rs`, `aircraft_movement.rs`; FTA Country–Metropolitan (`airport_fta/`, #198 cortes 1–5) |

### 5.2 Cliente (`openttdrs-client`)

| Sistema | Nivel | Módulo |
|---------|-------|--------|
| Render isométrico + atlas | ✅ | `render/`, `sprites/` |
| Animaciones agua/industria/humo | ✅ | `render/water.rs`, `smoke.rs`, etc. |
| SFX HUD + mundo (~20) | ✅ Parcial | `ui/hud/sound_ping.rs`, `audio/` |
| Música OGG | ✅ Parcial | `audio/music.rs` |
| Ventana audio (volúmenes/flags) | ✅ | `ui/audio_settings_window.rs` |
| Finanzas + préstamo UI | ✅ Parcial | `ui/finances_window.rs` |
| Pueblo (publicidad/fondos) | ✅ Parcial | `ui/town_window.rs` |
| Vehículos sub-tesela + extrapolación | ✅ | `render/vehicles.rs` |
| UI toolbar / ventanas flota | ✅ Parcial | `ui/toolbar/`, `vehicle_window.rs` |

### 5.3 Dependencias principales

| Crate | Versión | Uso |
|-------|---------|-----|
| `bevy` | 0.19 | Motor (2d, UI, state, **audio**, wav, png) |
| `openttdrs-core` | path | Simulación sin gráficos |
| `serde` / `serde_json` | 1.0 | Saves |
| `flate2` / `lzma-rs` | — | Descompresión `.sav` |

**Sin:** `kira`, `rodio`, crates MIDI, sistema de partículas Bevy dedicado.

### 5.4 Assets

| Carpeta | En repo | Tras scripts |
|---------|---------|--------------|
| `assets/opengfx/` | Metadatos `.obg` | Miles de PNG + atlas |
| `assets/opensfx/` | Metadatos `.obs` | `samples.cat` → WAV |
| `assets/sounds/` | Vacía (gitignored) | 6 WAV HUD |
| `assets/openmsx/` | No presente | vía `descargar_musica.sh` |
| `reference/openttd-upstream/` | Gitignored | Clone C++ referencia |

---

## 6. Complejidad de portado (resumen)

| Categoría | Complejidad | Bloqueadores principales |
|-----------|-------------|--------------------------|
| Animaciones vehículo (humo/chispas) | Media | Reusa `IndustrySmokePlugin`; 12 tipos `EffectVehicle` |
| Framework tile animation NewGRF | Alta | Callbacks, `m7`, triggers por dominio |
| Mixer + paneo SFX | Media | Bevy no expone paneo nativo; cámara isométrica |
| Catálogo 73 SFX + triggers | Media–Alta | ~73 samples, decenas de call sites |
| Sonidos vehículos | Alta | `motion_counter`, callbacks NewGRF |
| Música MIDI + playlists | Alta | SoundFont o pre-decode a OGG |
| Base sets `.obs`/`.obm` | Media | Parser + MD5; assets GPL |
| Dinámicas economía/ciudades | Media–Alta | Muchos comandos y UI |
| Desastres | Media | Flavor; no bloquea gameplay core |
| IA rivales | Muy alta | Motor script o IA propia |
| NewGRF runtime | Muy alta | Fuera de alcance |

---

## 7. Dependencias de assets y copyright

| Asset | Licencia | Uso recomendado en port |
|-------|----------|-------------------------|
| **OpenSFX** (`samples.cat`, `.obs`) | GPL v2, contenido libre | ✅ Recomendado |
| **OpenMSX** (`.obm`, MIDIs) | GPL v2, música libre | ✅ Recomendado para música |
| **OpenGFX** (sprites) | GPL v2 | ✅ Ya en uso |
| **TTD original** (`sample.cat`, `gm.cat`) | Propietario | Solo si el usuario posee TTD; no redistribuir |
| **SoundFonts** (FluidSynth) | Licencia propia (FluidR3, etc.) | Necesario solo para síntesis MIDI in-process |
| **NewGRF custom** | Depende de cada GRF | Mismo modelo que OpenTTD |

---

## 8. Orden sugerido de implementación

Combinando impacto en el “feel” del juego y esfuerzo de desarrollo:

| Fase | Ítem | Tipo | Esfuerzo | Impacto |
|------|------|------|----------|---------|
| **A1** | Humo de locomotoras (`EffectVehicle`) | Visual | S | Alto — vida visible a trenes |
| **A2** | SFX espaciales (construcción, cajero, cruce) | Audio | S–M | Alto — feedback inmediato |
| **A3** | Volumen dual música/efectos + flags settings | Audio | S | Medio — base para expansión |
| **B1** | Préstamos + intereses + quiebra | Dinámica | M | Alto — ciclo económico |
| **B2** | Crecimiento ciudades + autoridad local | Dinámica | M–L | Muy alto — mundo vivo |
| **B3** | Averías / fiabilidad / servicio | Dinámica | M | Alto — loop flota |
| **C1** | Música (OGG pre-decodificado OpenMSX) | Audio | M | Medio — ambiente |
| **C2** | Subsidios | Dinámica | M | Medio — objetivos |
| **C3** | Decaimiento carga + ratings estación | Dinámica | M | Medio — logística |
| **C4** | Desastres | Dinámica + FX | M | Medio — flavor |
| **C5** | Árboles + campos | Visual + sim | M | Bajo–Medio |
| **D1** | Barcos / aviones | Vehículo nuevo | L | Medio |
| **D2** | IA rivales | Dinámica | XL | ✅ Cerrado (#86, jul 2026) |
| **—** | NewGRF runtime | Mods | XL | Fuera de alcance actual |

### 8.1 Próximo paso concreto (recomendado)

Feel de partida + IA rivales (#86) ✅. CargoDist MVP (#49) ✅ + paridad MCF nivel 2 ✅. Pendiente de impacto jugable: NewGRF runtime. «Segunda humana» local (#41) descartada (las varias humanas son modelo MP, #21). Paridad rail fina: ver `docs/parity/rail_unknown_features.md`.

---

## 9. Referencias OpenTTD (archivos clave)

| Área | Rutas en `OpenTTD/src/` |
|------|---------------------------|
| Animación teselas | `animated_tile.cpp`, `newgrf_animation_base.h` |
| Effect vehicles | `effectvehicle.cpp`, `effectvehicle_func.h` |
| Paleta animada | `palette.cpp`, `table/palettes.h` |
| Sonido | `sound.cpp`, `mixer.cpp`, `sound_type.h` |
| Música | `music.cpp`, `music_gui.cpp`, `music/` |
| Base sets | `base_media_sounds.h`, `base_media_music.h` |
| Economía | `economy.cpp`, `subsidy.cpp`, `misc_cmd.cpp` |
| Ciudades | `town_cmd.cpp` |
| Desastres | `disaster_vehicle.cpp` |
| Vehículos | `vehicle.cpp`, `train_cmd.cpp`, `roadveh_cmd.cpp` |
| Árboles / campos | `tree_cmd.cpp`, `clear_cmd.cpp` |
| NewGRF | `newgrf.cpp` + `newgrf_*.cpp` |

| Área | Rutas en `openttdrs/` |
|------|----------------------|
| Sim | `crates/openttdrs-core/src/sim_step.rs` |
| Economía | `crates/openttdrs-core/src/economy.rs` |
| Audio HUD | `crates/openttdrs-client/src/ui/hud/sound_ping.rs` |
| Animaciones | `crates/openttdrs-client/src/render/*.rs` |
| Assets scripts | `scripts/descargar_assets.sh`, `preparar_sonidos_hud.sh`, `preparar_sonidos_opensfx.sh`, `preparar_musica_ogg.sh`, `descargar_musica.sh` |
| Paridad rail | `docs/parity/rail_status.md` |

---

## 10. Historial

| Fecha | Cambio |
|-------|--------|
| 2026-07-05 | Documento inicial: inventario post-auditoría animaciones, audio, música y dinámicas |
