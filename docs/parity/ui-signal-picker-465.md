# Selector de señales localizado (#465)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

La ventana `SignalPicker` mezclaba etiquetas inglesas (`Block`, `Entry`,
`Exit`, `Combo`, `Path`) con el resto del cliente español. El título se
construía además con `signal_type_label` y fragmentos de variante fuera del
catálogo, de modo que el locale inglés no cubría toda la ventana y el locale
español mostraba nombres ingleses en el título dinámico.

## Implementación

Las seis clases de señal y las dos variantes ahora tienen claves españolas y
entradas inglesas explícitas:

| Fuente `es` | Locale `en` |
| --- | --- |
| Bloque | Block |
| Entrada | Entry |
| Salida | Exit |
| Combinada | Combo |
| Ruta PBS | Path |
| Ruta 1vía | One-way path |
| Eléctrica | Electric |
| Semáforo | Semaphore |

El título usa las mismas claves para `Señales`, la variante y `densidad`.
Los IDs de señal, el orden de ciclo, la densidad y el encoding de mapa/SAV no
cambian.

La regresión `signal_picker_title_uses_the_active_locale_without_changing_values`
verifica español e inglés sobre la misma selección. El catálogo también
comprueba que cada clave se conserva en `es` y se traduce en `en`.

## Alcance pendiente

Esta evidencia cubre sólo la herramienta de colocación de señales; los
catálogos upstream restantes y otras superficies UI siguen en #331.
