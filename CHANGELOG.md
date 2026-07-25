# Changelog

Los cambios relevantes de cada release se documentan aquí. El formato sigue
[Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y las versiones usan
[SemVer](https://semver.org/lang/es/).

## [Unreleased]

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

[Unreleased]: https://github.com/cavazquez/openttdrs/compare/v0.1.0-alpha.1...HEAD
[0.1.0-alpha.1]: https://github.com/cavazquez/openttdrs/releases/tag/v0.1.0-alpha.1
