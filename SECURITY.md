# Política de seguridad

## Versiones soportadas

| Rama / artefacto | Soporte de seguridad |
|------------------|----------------------|
| `main` (HEAD) | Activo |
| Releases / tags puntuales | Mejor esfuerzo; priorizamos `main` |
| Forks / builds locales modificados | Sin soporte formal |

Este proyecto es un simulador en desarrollo activo (no un servicio en producción). Aun así se aceptan reportes que afecten integridad del estado de partida, desync multijugador, o la cadena de build/dependencias.

## Cómo reportar

**Preferido:** [GitHub Security Advisories](https://github.com/cavazquez/openttdrs/security/advisories/new) (reporte privado).

Si no podés usar advisories, contactá al maintainer del repo (`cavazquez`) por canal privado de GitHub **sin** publicar POC en issues públicos.

Incluí:

1. Descripción del impacto (RCE, desync, corrupción de save, DoS local, etc.).
2. Pasos de reproducción o PoC mínimo.
3. Versión / commit (`git rev-parse HEAD`) y SO.
4. Si el vector depende de un `.sav` / NewGRF malicioso.

## Alcance

**En alcance:** parser de saves/NewGRF, red (`openttdrs-net`), dependencias Rust (`cargo audit` / `cargo deny` en CI), paths de carga de assets locales.

**Fuera de alcance habitual:** bugs de jugabilidad/paridad OpenTTD, DoS trivial “mapa enorme”, issues que requieran ya comprometer la máquina del jugador.

## Dependencias

CI ejecuta `cargo audit` y `cargo deny check` (versiones fijadas en el workflow). Excepciones justificadas viven en `deny.toml`.

## Divulgación

Tras un reporte válido: confirmación, parche en `main` cuando sea posible, y crédito si el reporter lo desea. No hay SLA formal de tiempos.
