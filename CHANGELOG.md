# Changelog

Los cambios relevantes de cada release se documentan aquí. El formato sigue
[Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y las versiones usan
[SemVer](https://semver.org/lang/es/).

## [Unreleased]

## [0.1.0-alpha.5] - 2026-10-02

### Mejorado

- Se reutilizan rutas de carretera y agua mientras su topología y destino
  permanezcan vigentes; cambiar una reserva ferroviaria ya no las invalida.
- CargoDist resuelve trabajos en workers y publica sus resultados en las fechas
  previstas. Los saves JSON conservan los flows y trabajos pendientes.
- Las flotas grandes reutilizan índices de vehículos, ocupación, depósitos y
  huellas de estaciones. El renderer conserva sprites y proxies sin cambios,
  y evita preparar el HUD técnico cuando está oculto.
- El perfil de desarrollo usa tablas de líneas para reducir símbolos y costo
  de recompilación, conservando archivo y línea en los backtraces.

### Corregido

- Trenes y vehículos de carretera conservan las distancias por píxel, fracciones
  de movimiento y giros estacionarios de los casos nativos verificados.
- Las formaciones respetan piezas ferroviarias cortas y mantienen distintas
  las sucesivas visitas a un mismo tramo del recorrido.
- Carga, descarga, espera de carga completa y horarios mantienen quietos los
  vehículos físicos. La salida sin transferencia resuelve la siguiente ruta.
- Se corrigen el orden de sprites de igual profundidad, detalles viales al
  cambiar zoom y cadencias de animación de edificios y aeropuertos vanilla.
- Los efectos de humo y chispas conservan los sprites, paleta y decisiones RNG
  nativos verificados, incluso cuando el pool de efectos está lleno.
- El benchmark de terminales prepara el índice del mapa clonado antes de medir
  ticks estables, sin confundir la nueva identidad del mapa con un reescaneo
  durante la simulación.

### Compatibilidad y alcance

- El protocolo propio de red es v6; cliente y servidor deben usar esta alpha.
- Se mejora el rendimiento de flotas grandes, pero no se garantizan 30 FPS en
  todas las partidas. La paridad con OpenTTD continúa parcial.

## [0.1.0-alpha.4] - 2026-09-26

### Mejorado

- La cámara no roba el mouse de ventanas y controles; el clic derecho se
  distingue del paneo, y los arrastres de construcción confirman el extremo
  actual o se cancelan al perder foco.
- Las obras comprueban fondos antes de mutar el mapa y muestran rechazos,
  causa y costo real cuando una construcción por arrastre queda parcial.
- El fondo del menú muestra una partida isométrica animada. Aviones y barcos
  conservan poses coherentes al rotar y cruzar subteselas.

### Corregido

- El arranque de capturas que cargan un JSON ya no intenta abandonar el menú
  desde dos automatismos a la vez ni encola despawns duplicados.
- El primer clic en el mapa después de usar la barra de herramientas ya no se
  pierde.

### CI

- El job de GitHub Actions que valida scripts Python reutiliza su caché.

## [0.1.0-alpha.3] - 2026-09-20

### Agregado

- La semilla de una partida nueva se muestra y se puede editar o regenerar
  desde el mismo menú.

### Mejorado

- El formulario de Nueva partida aprovecha pantallas amplias con dos columnas
  y conserva un diseño de una columna con scroll como respaldo para ventanas
  angostas.

### Corregido

- Cada partida nueva automática recibe una semilla concreta distinta y crear
  otra partida no hace retroceder el tick de una sesión ya iniciada.
- El Snap materializa los PNG OpenGFX derivados durante el build; al iniciar
  no intenta escribir dentro del mount de sólo lectura `$SNAP`.
- Se agregó un smoke del artefacto Snap con raíz de assets read-only y perfil
  de usuario efímero.

## [0.1.0-alpha.1] - 2026-07-25

### Agregado

- Primera distribución alpha para Linux x86_64, Windows x86_64 y macOS arm64.
- Cliente Bevy jugable, servidor dedicado lockstep y simulación headless.
- Importación parcial de saves OpenTTD, saves JSON propios y export `.sav` parcial.
- CI con rustfmt, Clippy, rustdoc, 1.900+ tests, cobertura, auditoría de dependencias,
  fuzzing semanal y checks en Windows/macOS.

### Mejorado

- CargoDist agrupa las entregas y ejecuta Demand + MCF una vez por tick.
- Las señales ferroviarias usan un índice persistente y actualizaciones acotadas.
- La raíz de assets se resuelve junto al ejecutable en paquetes descargables.

### Limitaciones conocidas

- Es una alpha: NewGRF, barcos, aeronaves, multiplayer y paridad visual siguen parciales.
- Los paquetes no están firmados ni notarizados.
- La compatibilidad completa de ida y vuelta con `.sav` de OpenTTD no está garantizada.

[Unreleased]: https://github.com/cavazquez/openttdrs/compare/v0.1.0-alpha.5...HEAD
[0.1.0-alpha.5]: https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.5
[0.1.0-alpha.4]: https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.4
[0.1.0-alpha.3]: https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.3
[0.1.0-alpha.1]: https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1
