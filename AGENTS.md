# Contrato operativo para agentes

Este archivo es la fuente de verdad local para el modo de trabajo continuo de
los agentes en `openttdrs`. Su propósito es evitar que una instrucción antigua
de una conversación prevalezca por accidente sobre el objetivo actual.

## Precedencia y cambios

1. Una instrucción directa y posterior del usuario tiene prioridad sobre este
   archivo.
2. Este archivo tiene prioridad sobre planes o resúmenes históricos.
3. La documentación de paridad y los issues locales definen el backlog y la
   evidencia técnica, pero no sustituyen una indicación explícita del usuario.

El usuario puede editar este archivo directamente o pedir al agente que lo
modifique. El agente no debe cambiar estas reglas operativas por iniciativa
propia: puede proponer una mejora, pero debe conservarlas hasta recibir una
instrucción explícita.

## Objetivo de trabajo

Reducir y cerrar las brechas de paridad OpenTTD ↔ `openttdrs` de forma
reproducible, sin declarar completado un subsistema por una sola fixture. Las
fuentes canónicas del backlog son:

- `docs/parity/continuous-work-plan.md` para el orden de bloques.
- `docs/parity/random-map-issues.md` para la evidencia detallada de mapas.
- `docs/PARIDAD.md`, matrices SAV y matrices NewGRF para los demás contratos.

## Arranque de cada sesión

Antes de elegir trabajo, el agente debe leer por completo este archivo,
comprobar `git status`, revisar el último commit publicado y abrir el issue o
matriz que corresponda al bloque prioritario. No debe reiniciar una
investigación ya resuelta ni interpretar un resumen antiguo como si fuera el
estado del árbol de trabajo.

## Ciclo autónomo obligatorio

Mientras existan issues abiertos en el bloque prioritario, el agente debe:

1. Elegir la primera divergencia reproducible o el siguiente sub-issue acotado.
2. Medirla contra OpenTTD mediante un oracle apropiado.
3. Aplicar el cambio mínimo que reproduzca la regla nativa.
4. Añadir una regresión que cubra bytes, RNG, entidad, tick o raster según el
   contrato afectado.
5. Actualizar el issue local y la documentación necesaria, sin duplicar la
   misma evidencia en varios documentos.
6. Ejecutar las validaciones proporcionales al cambio.
7. Hacer `commit` y `push` de la etapa ya validada.
8. Empezar inmediatamente la siguiente etapa; no detenerse sólo porque se
   publicó un commit.

El agente se detiene únicamente si el usuario lo pide, si no quedan issues del
alcance acordado, o si necesita una decisión o autoridad que no puede inferir
de forma segura.

## Regla de cierre de issues

- Nunca cerrar un issue padre cuando sólo se resolvió un subconjunto.
- Crear y cerrar un sub-issue local acotado cuando haya criterio, regresión y
  evidencia diferencial suficientes.
- Documentar con precisión el alcance que sigue pendiente (tamaños, climas,
  configuraciones, callbacks, formatos, geometrías o capas visuales).
- La ausencia de una diferencia raster amplia no basta para declarar paridad;
  preferir comparación tile a tile, bloques 4×4, bytes raw, entidades y/o
  trazas RNG según corresponda.

## Validación

Antes de publicar una etapa, ejecutar como mínimo lo que aplique y registrar
los resultados relevantes:

```bash
cargo fmt --all -- --check
cargo clippy -p openttdrs-core --all-targets -- -D warnings
cargo test -p openttdrs-core --quiet
cargo clippy -p openttdrs-client --all-targets -- -D warnings
cargo test -p openttdrs-client --bin openttdrs-client --quiet
./scripts/check_parity_docs_fresh.sh
git diff --check
```

Para cambios de mapa, usar el comparador por fases y reportar tanto teselas
como bloques 4×4. Para cambios visuales, comprobar además varios niveles de
zoom —como mínimo `0.12×`, `0.25×`, `0.50×` y `1.00×`; usar los seis niveles
soportados cuando el cambio afecte viewport, culling u overview— y conservar
capturas sólo como diagnóstico, no como único oracle.

## Git y referencias externas

- Crear commits pequeños y descriptivos tras cada etapa validada y publicar en
  `origin/main`; el usuario autorizó explícitamente el push.
- Comprobar `git status` después del push y comunicar el hash publicado.
- No usar operaciones destructivas (`reset --hard`, checkout de descarte,
  borrados masivos) sin instrucción explícita.
- El checkout local `reference/` es un oracle de OpenTTD: puede instrumentarse
  temporalmente, pero esas trazas se retiran antes de validar y **nunca** se
  añade ni se commitea desde el repositorio principal.

## Comunicación

- Informar avances breves mientras se trabaja, incluida la primera divergencia
  hallada y el alcance pendiente real.
- Al terminar una etapa, informar: issue/sub-issue, cambio, evidencia,
  validaciones, hash y push.
- Si una nueva instrucción del usuario contradice este archivo, explicar qué
  regla se reemplaza y actualizar este archivo sólo si el usuario lo solicita.
