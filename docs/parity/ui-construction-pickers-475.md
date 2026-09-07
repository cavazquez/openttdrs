# Pickers de construcción localizados (#475)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

Los pickers de muelle, boya, waypoints, arbolado, terraformación y cartel
usaban un helper que reescribía el título en español durante cada actualización.
Las ayudas estáticas tampoco tenían catálogo completo. Además, `Cartel` ya era
la clave de la categoría de noticias y no podía reutilizarse para un cartel de
texto de construcción.

## Implementación

El helper común y los sync específicos reciben el locale activo. Se catalogan
los títulos y ayudas de cada picker, y el título de construcción usa la clave
no ambigua `Cartel de texto` (`Text sign`) sin alterar la categoría de noticias.
Orientaciones, herramientas activas, IDs y comandos permanecen intactos.

La regresión confirma las claves de waypoint/arbolado y que `Cartel de texto`
no colisiona con `Cartel` (`Newspaper`).

## Alcance pendiente

Los nombres de tipos de puente, objetos y demás datos de partida siguen siendo
literales; #331 continúa abierto por las superficies y catálogos upstream
restantes.
