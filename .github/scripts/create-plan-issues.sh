#!/usr/bin/env bash
# Crea issues del roadmap (ejecutar una sola vez). Requiere gh autenticado.
set -euo pipefail
cd "$(dirname "$0")/../.."

num_from_url() { sed -n 's#.*/issues/\([0-9]*\).*#\1#p' <<<"$1"; }

I1=$(gh issue create \
  --title "Infra: workspace Rust 2024, CI y Dependabot" \
  --label "enhancement" --label "dificultad:baja" \
  --body "## Objetivo
Workspace Cargo (\`edition = \"2024\"\`, \`rust-version\`), \`rust-toolchain.toml\`, CI (fmt, clippy, test, build) y Dependabot mensual (Cargo + Actions).

## Criterios de aceptación
- [ ] CI verde en \`main\` en push/PR.
- [ ] Dependabot activo con intervalo mensual.

## Dependencias
**Ninguna** (raíz del grafo de trabajo).

## Notas
Parte del alcance ya puede estar implementada; este issue sirve para seguimiento y mejoras (p. ej. límites de paralelismo en CI)."
)
N1=$(num_from_url "$I1")

I2=$(gh issue create \
  --title "Docs: clon de referencia OpenTTD e informe de arquitectura" \
  --label "documentation" --label "dificultad:baja" \
  --body "## Objetivo
Mantener \`scripts/fetch-openttd-reference.sh\`, \`reference/openttd-upstream/\` (gitignored) y \`docs/INFORME_ARQUITECTURA_OPENTTD.md\` al día con la versión analizada del upstream.

## Criterios de aceptación
- [ ] Instrucciones claras en README.
- [ ] Informe revisado al menos una vez por release mayor de OpenTTD de interés.

## Dependencias
**Opcional:** puede hacerse en paralelo a #$N1; conviene tener el repo (#$N1) para enlazar CI/docs.

## Notas
No versionar el clon completo; solo documentación y script."
)
N2=$(num_from_url "$I2")

I3=$(gh issue create \
  --title "Core: mapa por teselas, reloj de tick y estado de simulación" \
  --label "enhancement" --label "dificultad:baja" \
  --body "## Objetivo
Crate \`openttdrs-core\`: coordenadas de tesela, mapa denso, altura/tipo mínimo, \`GameTick\`, \`GameState\`, API sin I/O ni Bevy.

## Criterios de aceptación
- [ ] API estable documentada para el cliente.
- [ ] Tests unitarios del mapa y del tick.

## Dependencias
- Bloqueado por: **#$N1** (workspace y publicación del crate).

## Notas
Base para toda la simulación determinista."
)
N3=$(num_from_url "$I3")

I4=$(gh issue create \
  --title "Core: tests de invariantes y regresión" \
  --label "enhancement" --label "dificultad:media" \
  --body "## Objetivo
Ampliar tests en \`openttdrs-core\` con invariantes inspiradas en OpenTTD (documentadas en el informe) y casos límite de mapa/tick.

## Criterios de aceptación
- [ ] Conjunto de tests que cubra bordes del modelo actual.
- [ ] Documentación de invariantes en \`docs/\` o en el crate.

## Dependencias
- Bloqueado por: **#$N3**.

## Notas
Prepara terreno para determinismo y red futura."
)
N4=$(num_from_url "$I4")

I5=$(gh issue create \
  --title "Cliente Bevy: ventana, cámara y vista debug del mapa" \
  --label "enhancement" --label "dificultad:media" \
  --body "## Objetivo
\`openttdrs-client\`: app Bevy con cámara 2D, sincronización visual del \`GameState\` (rejilla/gizmos), título con tick.

## Criterios de aceptación
- [ ] Arranque estable en Linux (documentar deps X11/Vulkan según CI).
- [ ] Sin acoplar lógica de simulación al ECS más allá del puente mínimo.

## Dependencias
- Bloqueado por: **#$N1**, **#$N3**.

## Notas
Features Bevy recortadas (sin Wayland/gilrs) según necesidad del equipo."
)
N5=$(num_from_url "$I5")

I6=$(gh issue create \
  --title "Simulación: industrias y economía reducida" \
  --label "enhancement" --label "dificultad:media" \
  --body "## Objetivo
Primer bucle de producción/consumo de carga y 1–2 tipos de industria en \`openttdrs-core\`, integrado con el tick.

## Criterios de aceptación
- [ ] Producción determinista por tick reproducible en tests.
- [ ] Datos mínimos (sin NewGRF).

## Dependencias
- Bloqueado por: **#$N3**, **#$N4** (tests antes de crecer la superficie).

## Notas
Acotar tipos de carga e industria para no explotar el alcance."
)
N6=$(num_from_url "$I6")

I7=$(gh issue create \
  --title "Pathfinding mínimo (carretera o raíl simplificado)" \
  --label "enhancement" --label "dificultad:alta" \
  --body "## Objetivo
Implementar búsqueda de caminos sobre el grafo/vías del mapa (subconjunto), API clara para \`openttdrs-core\`.

## Criterios de aceptación
- [ ] Benchmarks básicos en mapas medianos.
- [ ] Tests con fixtures de vías simples.

## Dependencias
- Bloqueado por: **#$N3**, **#$N6** (destinos/industria o estaciones dummy).

## Notas
YAPF en OpenTTD es complejo; aquí solo un MVP alineado al modelo de mapa Rust."
)
N7=$(num_from_url "$I7")

I8=$(gh issue create \
  --title "Vehículos, órdenes y estaciones" \
  --label "enhancement" --label "dificultad:alta" \
  --body "## Objetivo
Máquina de estados de vehículo, cola de órdenes mínima, interacción con estaciones y carga en \`openttdrs-core\`.

## Criterios de aceptación
- [ ] Un modo de transporte primero (p. ej. carretera o raíl).
- [ ] Tests de órdenes y paradas.

## Dependencias
- Bloqueado por: **#$N6**, **#$N7**.

## Notas
Uno de los bloques más grandes del port; dividir en PRs pequeños."
)
N8=$(num_from_url "$I8")

I9=$(gh issue create \
  --title "UI Bevy: construcción e interacción con infraestructura" \
  --label "enhancement" --label "dificultad:alta" \
  --body "## Objetivo
Controles in-game, feedback de construcción, validación de comandos y visualización en Bevy.

## Criterios de aceptación
- [ ] Flujo jugable: colocar vías/estaciones de prueba y ver vehículos usarlas.
- [ ] Errores de validación claros (UX mínima).

## Dependencias
- Bloqueado por: **#$N5**, **#$N8**.

## Notas
Mantener comandos como datos serializables para futura red."
)
N9=$(num_from_url "$I9")

I10=$(gh issue create \
  --title "Contenido: NewGRF / basesets (compatibilidad parcial)" \
  --label "enhancement" --label "dificultad:muy-alta" \
  --body "## Objetivo
Pipeline de carga de un subconjunto del spec NewGRF o basesets propios, detrás de feature flags.

## Criterios de aceptación
- [ ] Documentar qué acciones/sprites/properties se soportan.
- [ ] Tests con GRF de ejemplo mínimos.

## Dependencias
- Bloqueado por: **#$N3**, **#$N8** (estaciones/vehículos reales para muchos GRF).

## Notas
Paridad total con OpenTTD es multi-año; acotar explícitamente el subset."
)
N10=$(num_from_url "$I10")

I11=$(gh issue create \
  --title "Red: multijugador cliente/servidor determinista" \
  --label "enhancement" --label "dificultad:muy-alta" \
  --body "## Objetivo
Protocolo mínimo: servidor autoritativo, clientes aplican la misma secuencia de comandos, detección de desync.

## Criterios de aceptación
- [ ] Replay de comandos idéntico en dos instancias core.
- [ ] Documento de amenazas (seguridad, cheating).

## Dependencias
- Bloqueado por: **#$N4** (determinismo), **#$N8** (comandos de vehículo/infra).

## Notas
Requiere RNG y estructuras 100% deterministas."
)
N11=$(num_from_url "$I11")

I12=$(gh issue create \
  --title "Guardados: formato versionado y compatibilidad" \
  --label "enhancement" --label "dificultad:muy-alta" \
  --body "## Objetivo
Formato de save propio versionado; opcionalmente importación parcial desde OpenTTD (investigación).

## Criterios de aceptación
- [ ] roundtrip save/load en tests de integración del core.
- [ ] Política de compatibilidad documentada.

## Dependencias
- Bloqueado por: **#$N8** (estado a serializar).
- **Recomendado** tras **#$N11** para alinear reglas de determinismo con red.

## Notas
Compatibilidad binaria con saves de OpenTTD es un proyecto aparte."
)
N12=$(num_from_url "$I12")

echo "Creados: #$N1 #$N2 #$N3 #$N4 #$N5 #$N6 #$N7 #$N8 #$N9 #$N10 #$N11 #$N12"
echo "$I1"
echo "$I12"
