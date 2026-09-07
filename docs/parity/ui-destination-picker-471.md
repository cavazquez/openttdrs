# Selector de destinos localizado (#471)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

El `DestinationPicker` tenía título, ayuda y botón de selección en mapa en
español fijo. Sus filas se reconstruyen en cada actualización y los destinos
sin nombre usaban labels vanilla (`Parada bus`, `Estación tren`, `Depósito`,
etc.) sin consultar el locale.

## Implementación

El sync recibe `ClientPreferences` y localiza el chrome y únicamente los
fallbacks vanilla. Un nombre de estación/depósito personalizado, un label
NewGRF y las coordenadas `(x, y)` permanecen byte a byte intactos. La
selección, el orden de candidatos, las IDs de vehículo y los comandos de
órdenes no cambian.

La regresión cubre un fallback inglés/español y un nombre personalizado con
coordenadas negativas.

## Alcance pendiente

Los nombres aportados por la partida, NewGRF o GameScript no se traducen; #331
continúa abierto por los catálogos upstream y las superficies UI restantes.
