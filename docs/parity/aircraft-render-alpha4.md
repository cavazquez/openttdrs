# Corte alpha.4: rumbo de aeronaves y tráfico del menú

Base auditada: `f1c3a5fc`, 2026-09-26. Son contratos de presentación
acotados; no certifican paridad completa de vuelo ni sustituyen V1-AIR.

## Tareas atómicas

- **AIR-POSE — completado:** el sprite vanilla debe seguir el rumbo físico
  durante despegue, crucero y aterrizaje. Dakota entre dos Country, 64×64,
  1950, semilla `0x59400064`, 500 pasajeros por estación, 10.000 ticks,
  sin averías/desastres. Cero discrepancias con alphas 0/0,5/1, con y sin
  mapa; mantener inversión NewGRF. Regresión adicional para los ocho rumbos.
  Base: 6.530 discrepancias con pose de tick; 6.422/7.742 con alpha 0,5/1.
- **MENU-DIR — completado:** deducir el rumbo de la trayectoria decorativa,
  no de constantes S/N. Cero discrepancias en ambas direcciones de las
  rutas; inversión coherente al volver.
- **MENU-POS — pendiente:** posición fraccional para aviones/barcos y conversión
  correcta de fracciones de tesela a píxeles en el mismo helper terrestre, con
  continuidad en fronteras de tesela. Tolerancia numérica explícita de
  `0,001` unidades de mundo; comprobar 0,12×/0,25×/0,50×/1,00×.
- **AIR-ORACLE — completado:** comparar `direction` por tick con la traza
  Helidepot de OpenTTD 15.3 existente. Cero diferencias, sin cambiar el golden
  ni relajar umbrales; no extender el resultado a todos los aeropuertos.
- **ALPHA4 — pendiente:** formato, Clippy, tests, CI, paquetes y smoke sobre
  el candidato; después publicar prerelease GitHub y Snap `latest/edge`.

## Alcance excluido

El menú sigue siendo un showcase decorativo; no se convierte en simulación
de una partida. No se añade física de giros curvos al invertir las rutas.
No se cambian reglas de carga, economía, reservas de aeropuerto, saves,
protocolo de red, sprites ni umbrales visuales generales.

## Evidencia

Se registra al completar cada tarea, con comandos, resultados y SHA. Una
tarea o publicación no ejecutada permanece pendiente, nunca aprobada.

### AIR-POSE

El selector usa el rumbo de la aeronave también en crucero, como
`Vehicle::GetImage(v->direction)` y `Aircraft::GetImage` en la referencia
OpenTTD 15.3. La prueba nueva falló antes del arreglo en el tick 75 y con el
primer rumbo de la matriz; después aprobó los 10.000 ticks, alphas y ocho
direcciones, incluida inversión NewGRF. Cero discrepancias, sin cambiar el
estado de simulación. V1-AIR conserva entrega en tick 1.391, 8 pasajeros,
$303 y hash `14221659823881627150`.

Validado con `cargo fmt --all -- --check`, Clippy core `--all-targets -D
warnings`, suite completa core, `aircraft_render_heading` (2),
`airport_fta_openttd_oracle` (4), `v1_air_delivery` (1), checker de documentación
y `git diff --check`. Se conservan los seis ignores explícitos preexistentes
del core; no se cuentan como pruebas ejecutadas.

### AIR-ORACLE

La regresión de `airport_fta_openttd_oracle` exige ahora igualdad exacta de
dirección en cada tick de la traza Helidepot 15.3, además de nodo, estado y
altura. Pasaron los cuatro tests sin tocar el golden ni el controlador.
Comparte la validación completa core de AIR-POSE; no certifica vuelo libre
universal ni otros layouts de aeropuerto.

### MENU-DIR

Se eliminaron las direcciones literales de las doce rutas. El octante se
deduce de su geometría; la vuelta usa el octante opuesto. Los dos tests nuevos
comprueban los ocho rumbos y ambos sentidos de cada ruta (incluido el barco
no alineado), con cero diferencias. Se mantiene la vuelta decorativa
instantánea, sin prometer un giro físico de aeronave.

Pasaron seis tests del intro, Clippy cliente `--all-targets -D warnings`,
la suite de cliente (1.635 aprobados, dos ignores preexistentes), formato y
diff. La revisión gráfica del candidato se conserva en ALPHA4.
