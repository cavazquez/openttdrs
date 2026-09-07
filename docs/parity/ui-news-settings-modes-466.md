# Modos de configuración de Noticias localizados (#466)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

`NewsSettings` construía sus botones como `Off`, `Ticker` y `Cartel`. La
explicación mezclaba inglés y español, y sólo `Cartel` estaba catalogado. Con
locale español la ventana no era coherente; con locale inglés no usaba las
etiquetas `Summary`/`Full` de OpenTTD.

## Implementación

Los botones ahora conservan el mismo enum y las mismas preferencias, pero usan
claves españolas:

| Fuente `es` | Locale `en` |
| --- | --- |
| Silencio | Off |
| Resumen | Summary |
| Completo | Full |

La explicación visible también tiene una fuente completamente española
(`Silencio = sin noticias · Resumen = ticker · Completo = periódico`) y una
traducción inglesa equivalente. El `LocalizationPlugin` actualiza las
entidades ya creadas y las creadas después del cambio de locale.

La regresión `news_mode_buttons_keep_the_preference_enum_but_localize_labels`
verifica los tres modos, ambos locales y la conservación del enum.

## Alcance pendiente

Las ocho categorías continúan preservando sus preferencias y los titulares y
cuerpos generados siguen siendo datos de la partida. Los catálogos upstream
restantes de #331 no quedan cerrados por este subconjunto.
