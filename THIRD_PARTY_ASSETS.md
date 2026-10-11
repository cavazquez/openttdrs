# Assets de terceros

Los paquetes binarios de openttdrs agrupan obras independientes junto al programa.
Cada una conserva su licencia original:

| Obra | Versión / uso | Licencia y atribución | Fuente |
|------|---------------|-----------------------|--------|
| OpenGFX | 8.0; sprites procesados en `assets/opengfx/` | GPL-2.0; The OpenGFX Team y contribuidores | [OpenGFX](https://github.com/OpenTTD/OpenGFX) |
| OpenSFX | 1.0.3; sonidos convertidos a WAV | CC BY-SA 3.0; The OpenSFX Project y autores detallados por el proyecto | [OpenSFX](https://github.com/OpenTTD/OpenSFX) · [licencia](https://creativecommons.org/licenses/by-sa/3.0/) |
| OpenMSX | 0.4.2; música convertida a OGG | GPL-2.0; The OpenMSX Project y compositores acreditados por el proyecto | [OpenMSX](https://github.com/OpenTTD/OpenMSX) |
| DejaVu Sans Mono | fuente UI sin modificar | DejaVu Fonts License | [DejaVu Fonts](https://dejavu-fonts.github.io/License.html) |

El texto GPL-2.0 completo se distribuye en `LICENSE`. Los scripts de conversión y
la fuente correspondiente de openttdrs están disponibles en el tag de la release.
Los créditos detallados y fuentes preferidas de OpenSFX/OpenMSX están enlazados
arriba; la conversión de formato no cambia autoría ni licencia.

Los 90 sprites Action5 de cimientos se leen del archivo OpenGFX 8.0
`ogfxe_extra.grf` incluido dentro del binario. Sus bytes originales, créditos,
licencia y procedencia verificada se conservan en
`crates/openttdrs-core/assets/opengfx-8.0/` en la fuente de esta release.
