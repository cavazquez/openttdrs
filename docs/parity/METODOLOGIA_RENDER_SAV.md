# Metodología de paridad de partidas `.sav`

Este documento deja el método y el estado del trabajo de carga y render de
partidas OpenTTD. El objetivo no es que una captura se parezca aproximadamente:
es explicar por tesela y por decisión de dibujo por qué OpenTTD y `openttdrs`
eligen un resultado distinto.

La foto del lote es el [checkpoint de pausa](../checkpoints/2026-08-09-parity-oracle-pause.md).
Es un registro histórico: `main` continuó incorporando cambios revisables
después de ese punto. El estado de una corrección se establece siempre con los
exports y comparadores de este documento, no por asumir que aquel checkpoint
sigue describiendo el árbol actual.

## Alcance y repositorios

| Componente | Uso | Regla de trabajo |
|---|---|---|
| `openttdrs` | Implementación Rust, importador SAV, renderer y comparadores | Trabajar primero en un checkpoint; integrar a `main` sólo cambios acotados y verdes. |
| Fork `cavazquez/OpenTTD` | Referencia C++ ejecutable | `main` sigue OpenTTD oficial. Las sondas viven únicamente en `openttdrs/oracle-parity`, nunca en upstream. |
| Partida `.sav` fija | Caso reproducible | Ambas exportaciones usan el mismo archivo y SHA-256; `Kale_TitleGame.sav` se utilizó como caso de estrés. |

El fork C++ no cambia el resultado que se estudia: agrega exportadores de
diagnóstico. Así se puede saber qué bytes hay en una coordenada, cómo se
interpretan y qué comandos produce el `draw_tile_proc` real de OpenTTD.

## Modelo de evidencia

Una captura puede fallar por bytes mal cargados, semántica mal interpretada,
sprite equivocado, geometría/altura incorrecta o por orden de dibujo. Por eso
la paridad se comprueba por capas, de menor a mayor distancia del píxel final.

| Nivel | Contrato | Pregunta | Límite |
|---|---|---|---|
| 1 | [`world-raw`](WORLD_RAW_SCHEMA.md) | ¿Los bytes de mapa de cada tesela son los mismos? | No explica su significado. |
| 2 | [`world-semantic`](WORLD_SEMANTIC_SCHEMA.md) | ¿Ambos clasifican igual vía, puente, túnel, estación, pendiente y orientación? | No garantiza el sprite final. |
| 3 | [`world-draw`](WORLD_DRAW_SCHEMA.md) | ¿Rust selecciona sprite, paleta y geometría permitidos por el `draw_tile_proc` C++? | La cobertura Rust aún no incluye todas las familias ni prueba el sort global o el framebuffer. |
| 3b | [`world-sort`](WORLD_DRAW_SCHEMA.md#orden-global-de-parents-world-sort) | ¿Los parents candidatos se emiten en el orden final de `ViewportSortParentSprites`? | El runtime aplica el sorter compartido a casas vanilla, árboles `MP_TREES` con sus capas combinadas, muelles vanilla, edificios industriales vanilla planos/estáticos, sprites directos de `DrawFoundation`, las seis capas `TILE_SEQ` del depósito naval, estructuras/catenaria y bloque combinado PBS/Action5 de puentes, y cuerpos/unidades de vehículos con las cajas `Vehicle::bounds` de OpenTTD. El suelo posterior de casas inclinadas/rampas de puente y la base/vía rail posterior a fundación quedan vinculados al último parent; sombra/rotor de aeronave son children del cuerpo. Los overlays de carretera y tranvía, incluidos reemplazos NewGRF, siguen el parent de fundación cuando existe. Restan la mitad frontal de puente, layouts/children NewGRF de estación/objeto/industria, sprite-stack, pivotes y clipping; no certifica el framebuffer global. |
| 4 | Captura enfocada | ¿La composición completa se ve correcta en el contexto real? | Es aceptación visual, no la única evidencia. |

La regla es encontrar la primera capa que diverge antes de editar. De ese modo
no se oculta un error de importación con un ajuste visual, ni se culpa al atlas
cuando el tile ya se decodificó mal.

## Trabajo incorporado al checkpoint

### Infraestructura de diagnóstico

- Exportadores C++ de `world-raw`, `world-semantic`, `world-draw` y captura
  del mundo. `world-draw` ejecuta `draw_tile_proc` sin framebuffer, UI,
  vehículos ni rótulos y puede correr headless.
- Dumper Rust equivalente para raw y semántica, más una traza headless de las
  decisiones visuales que el renderer ya instrumenta.
- Comparadores JSONL que validan encabezado, región, hash de la partida,
  teselas, sprites, paletas, fundaciones y geometría cuando está disponible.
  Las pruebas de mutación comprueban que detecten diferencias deliberadas.
- Scripts que preparan el baseset, ejecutan ambos lados y exigen una fila final
  `complete`; una traza truncada no es evidencia válida.
- `sccache` en desarrollo y GitHub Actions para acelerar las iteraciones de
  compilación y pruebas.

### Familias visuales tratadas

| Familia | Trabajo realizado | Estado |
|---|---|---|
| Viewport de mapas grandes | Completado de chunks parciales. | Corrección aislada publicada en `main` (`5b0023b`). |
| Terreno, pendientes y árboles | Revisión de altura isométrica y sprites de ladera para eliminar artefactos que parecían vías o dejaban colores extraños. El primer sprite de cada `MP_TREES` ahora registra el parent 16×16×48 de OpenTTD y sus copas `CombinedSprite` quedan vinculadas como children del mismo parent. | El foco Kale `138,7..140,10` contiene 7/7 parents candidatos en el `world-sort` C++; los árboles conservan la caja `(2208,112,4)..(2223,127,51)` de `(138,7)` aun con offset visual `(4,4)`. Capturas reales en 0,25×, 1× y 2× sin regresión visible; sigue pendiente el framebuffer global. |
| Puentes rail, túneles y conexiones | Fundaciones, pilares, rampas y altura efectiva; la traza conserva sprites y geometría de las transiciones. El suelo posterior de una rampa con fundación se adjunta como child de pantalla al último parent de `DrawFoundation`, igual que `AddChildSpriteScreen`; `DrawBridgeMiddle` y las bocas sin catenaria materializan además sus `SPR_EMPTY_BOUNDING_BOX` como constraints sin raster del sorter runtime. | El foco Kale `(109,28)` contiene los 3/3 comandos C++ de la rampa por selección, geometría, fundación y orden relativo; la regresión Bevy verifica el vínculo runtime sobre una rampa inclinada. Kale completo: parents identificables contenidos por el oráculo; `world-draw` exporta pre-sort. En checkpoint, sin declarar paridad global de composición. |
| Catenaria | La ruta común cubre vía normal, cruces a nivel eléctricos, postes de la boca de túnel, cable especial de portal y cable de entrada de depósito. Conserva el orden PPP → PCP antes de las capas `TILE_SEQ` y la altura posterior a fundación. | Kale completo 8bpp: la comparación estricta no deja comandos, geometrías, paletas ni órdenes fuera de OpenTTD. |
| Señales ferroviarias | El importador lee `vehicle.road_side` y `construction.train_signal_side` de `PATS`/`OPTS`; el renderer replica el orden de `DrawSignals` y la altura de `GetSafeSlopeZ` sobre la fundación ferroviaria efectiva. | Kale completo: las 729 señales coinciden en ID, ancla de mundo, geometría y orden relativo. |
| Monorriel y maglev | Selección diagonal tipada por railtype para no usar rail convencional. | En checkpoint; validar por región. |
| Depósitos y estaciones especiales | Geometría de depósito naval, muelle, reserva visual de depósito rail, cable eléctrico de entrada y boya con su suelo de agua explícito; los fallbacks siguen siendo distinguibles. Cada capa 4070–4075 del depósito naval y cada mitad StationGfx 2727–2732 del muelle aportan su prisma `TILE_SEQ` al sorter runtime global; el runtime también reordena localmente los parents BUILD viales y el bundle cable/fachada del depósito rail según sus bounds. | La traza de Kale cubre el suelo 4061 + boya 9282 y los cables de depósito en su orden y altura de OpenTTD. El foco naval `138,7..140,10` vincula 7/7 parents candidatos (incluidas 4073, 4075 y 4071), y el foco de muelle `136,1..139,3` vincula 7/7 (incluidas 2729 y 2731) al `world-sort` C++; `(195,17)` conserva el orden final `1063 → 5659 → 1064`. Sigue sin declarar composición global. |
| Paleta de compañía 8bpp y estación rail vanilla | Se corrigió el desplazamiento de un índice DOS en las rampas y se dejó de inferir recolor por RGB ajeno a la rampa autora. La región `120,111..128,113` de Kale compara 118/118 comandos de estación (sprite, paleta, geometría y orden). | Validada por `world-draw`; la composición raster amplia sigue teniendo familias ajenas a esta corrección. |
| Paradas viales vanilla (bus/camión) | `DrawTile_Station` nivela las paradas inclinadas, dibuja el suelo de bahía o la base pavimentada pasante y luego sus `TILE_SEQ_LINE`. El renderer registra los IDs globales de cada capa, sus cajas, paleta de compañía y el child de la fundación; deja de añadir césped o una carretera heurística bajo una bahía. | Kale completo 8bpp: las 457 paradas comparan exactamente contra OpenTTD en ID, paleta, geometría, fundación y orden. La regresión sintética verifica metadata 8bpp/32bpp y evita confundir los IDs Action5 locales 2009–2018 con `SPR_ROADSTOP_BASE` 5978–5985. |
| Paletas de casas vanilla | `HOUSE_DRAW_DATA` conserva ahora `p1`/`p2` y la caja `dx/dy/sx/sy/sz` de cada `M(...)` de `town_land.h`; la caché aplica paleta de compañía (775–790), estructura (795–801) e iglesia (1438–1439) a las capas de suelo y edificio. | En Kale 8bpp, los 8.497 buildings tienen sprite, tesela, paleta, mundo, bounds y orden relativo contenidos por C++; las pruebas exigen que todos los pares generados tengan asset recoloreado. La captura global sigue marcada como diferente por familias ajenas. |
| Edificios industriales vanilla planos | `INDUSTRY_GFX_DATA` conserva el `M(dx,dy,sx,sy,sz)` de `industry_land.h`; cuando no hay fundación, animación ni layout NewGRF, el building recibe el mismo parent runtime y la clave diagonal estable. | Kale completo 8bpp contiene los 383 comandos `industry-building` por ID, geometría y orden relativo. El sorter sólo cubre el subconjunto de caja inmutable: pendientes, animaciones y layouts NewGRF no se hacen pasar por parents completos. |
| Fundaciones, casas y rail | Las casas inclinadas usan el `foundation_draw_plan` común de `DrawFoundation(Leveled)`: el bloque 0–3 se deriva de los vecinos y la superficie de pendientes empinadas conserva su segundo nivel. Cada sprite del plan conserva su `SpriteBounds` C++ (origen, extent y desplazamiento interno), reutilizado por rail, road, estaciones, puentes y paradas. El renderer aplica el `origin` tanto al sorter como al ancla visible; los parents de edificio vanilla y los sprites directos de `DrawFoundation` pasan por el mismo sorter runtime. El suelo `s1` de la casa, el ascensor de Large Office y la base/vía rail posteriores se registran como children de pantalla del parent que deja activa la fundación. | Kale completo: las 1.943 fundaciones de casas, sus 1.943 suelos child, 188 ascensores y las 3.014 fundaciones comunes instrumentadas tienen selección, caja y orden relativo contenidos por OpenTTD. En el foco rail `(229,149)`, ese ancla elimina los rombos negros de la media foundation; el estado raster canónico y los residuos restantes se mantienen sólo en [`PARIDAD.md`](../PARIDAD.md#evidencia-visual-raster-vigente). |
| Reservas PBS 8bpp | Los overlays `PALETTE_CRASH=804` se hornean desde índices DOS con la misma pseudo-sprite de recolor que usa OpenTTD; se eliminó el tinte RGBA naranja aproximado. Incluye `SINGLE_*` rail/mono/maglev y las doce rampas PBS. | Validar en captura focalizada; la traza conserva paleta 804 y ahora distingue la ausencia del asset exacto como fallback. |
| Campos y cercas de Kale (8bpp y 32bpp) | Se instrumentó `DrawTile_Clear` para campos y sus cuatro cercas, se corrigió la altura de esquina de pendientes empinadas y el suelo natural deja de usar la elevación como profundidad. El spawn recorre las teselas en el mismo barrido diagonal de `ViewportAddLandscape`; OpenGFX2 selecciona su variante RGBA normal sin mezclar coordenadas 8bpp. | La región `225,25..251,61` valida en 8bpp 647 suelos y 476 cercas: ID, geometría y orden relativo 100 % contenidos en OpenTTD. La regresión de selección cubre los dos perfiles de baseset. Falta una captura local de aceptación tras el cambio. |
| Suelo natural `MP_CLEAR` (8bpp y 32bpp) | Se porta `DrawTile_Clear`: césped por densidad, rough plano por `TileHash`, las 19 pendientes rocosas de la serie correcta y la tabla sin tintes de nieve/desierto. La traza `world-draw` registra cada suelo que no es campo. OpenGFX y OpenGFX2 no activan `SecondRockyTileSet`, por lo que ambos usan 4023–4041. | Kale completo: 9.331 selecciones `clear-ground` coinciden en sprite, geometría y orden con el comando C++. Es paridad de selección, no todavía igualdad raster global. |
| Agua plana y costa `MP_WATER` | `push_water_tile` traza `DrawSeaWater` (4061) y `DrawShoreTile` (`SPR_SHORE_BASE + tileh_to_shoresprite`), incluso cuando el cache de costa materializa la imagen desde Action5/NewGRF. Los locks y depósitos conservan sus familias separadas. | Kale completo: 8.139 `water-ground` y 1.109 `water-shore` coinciden en sprite y orden con C++; la cobertura permite que una futura regresión de costa/agua aparezca en el auditor. |
| Cercas de vía `DrawTrackDetails` | Se eliminó la inferencia por vecinos: el renderer usa el `RailGroundType` persistido, las 16 entradas de `_fence_offsets`, la pendiente efectiva tras fundación y los anclajes NFO reales de `SPR_TRACK_FENCE_*`. Se dibuja antes de catenaria/señales y aplica la paleta de la compañía. | Kale completo 8bpp: las 3.995 cercas (1301–1308) coinciden exactamente por tesela en sprite, paleta, mundo, bounding box y orden. El generador y su regresión seleccionan anclas de zoom normal tanto para 8bpp como 32bpp; falta aún una aceptación raster completa en 32bpp. |
| Iconos y assets | Regeneración de iconos y datos de atlas asociados. | En checkpoint; revisión visual pendiente. |

Este inventario describe trabajo efectuado, no una afirmación de que todos los
casos estén resueltos. El checkpoint mezcla renderer, simulación, assets y
SAV; debe dividirse antes de una integración amplia.

## Evidencia y revalidación

En la partida de estrés se obtuvo inicialmente una exportación completa de
65.536 teselas con coincidencia exacta de `world-raw` y `world-semantic`. Eso
reduce mucho la probabilidad de que los artefactos tratados sean una
deserialización general del mapa, pero no sustituye volver a correr el contrato
cuando cambie el importador.

La revalidación completa de `Kale_TitleGame.sav` con OpenGFX 8bpp produjo
157.142 comandos visuales tanto en OpenTTD como en el candidato. El comando
`compare_world_draw.py --geometry --foundations --order --strict-reference`
termina correctamente: todas las selecciones, geometrías explícitas, paletas,
fundaciones y órdenes relativos del contrato instrumentado están contenidos
en el `draw_tile_proc` de OpenTTD.

Esto **no** es igualdad raster total ni una garantía para otros basesets o
NewGRF. El contrato verifica selección, paleta, geometría, fundación y orden
antes del atlas; la aceptación visual aún requiere capturas focalizadas y la
misma corrida con OpenGFX2 32bpp.

### Regresión `MP_OBJECT` descubierta al revalidar

Una comparación completa posterior de `Kale_TitleGame.sav` encontró dos
divergencias raw, en `(14,141)` y `(245,240)`: OpenTTD conservaba `MAP5 = 0`
y el candidato escribía `1`. Ambas teselas eran `MP_OBJECT`; la alteración
cambiaba también el `ObjectID` y por ende el sprite de objeto elegido.

El origen era doble: se usaba `location.tile` como clave del pool `OBJS` y se
sobrescribía `MAP5` con `ObjectType`. OpenTTD hace lo contrario: forma
`ObjectID = m2 | (m5 << 16)` y consulta el tipo visual en el pool por ese ID.
La corrección conserva `MAP5`, transporta `ObjectID → ObjectType` en el footer
`OBTY` y añade `details.object_type` a `world-semantic` v2. El mismo análisis
descubrió que el conversor Python separaba `MAP2` big-endian en orden inverso;
se corrigió junto con la regresión.

El guardarraíl permanente es
`scripts/verify_parse_sav_object_m5.py`, incluido en el manifiesto de CI. Usa
un fixture con transmisor y faro, exige igualdad de `MAP5` y `ObjectID`, y
comprueba que cada objeto se resuelva desde `OBTY`. Así un arreglo visual no
puede volver a ocultar una mutación de bytes del save.

La revalidación final de `Kale_TitleGame.sav` volvió a comparar las 65.536
teselas: `world-raw` y `world-semantic` terminaron sin diferencias. La
traza `world-draw` incorpora además los hitos vanilla afectados por esta
regresión; el fixture controlado compara transmisor `(47,33)` y faro `(60,55)`
con `--strict-reference`, y en ambos casos coinciden terreno, sprite,
geometría y orden de los dos comandos de OpenTTD.

### Revalidación: campos y cercas de Kale (8bpp y 32bpp)

La región de cultivo `225,25..251,61` se comparó usando el mismo baseset
OpenGFX 8bpp para OpenTTD y para el atlas de `openttdrs`. El oráculo C++ emitió
1.957 comandos y el candidato 1.740 selecciones instrumentadas. Dentro de esa
región, los 647 `field-ground` y las 476 `field-fence` coincidieron al 100 %
en ID de sprite, geometría explícita y orden relativo. La regla de composición
no depende de la profundidad de color; para OpenGFX2, la regresión de NFO
verifica que cada campo/cerca use la continuación `32bpp` de zoom normal y no
la fila 8bpp ni una variante `zi4`.

El defecto visual que quedaba no era de importación ni de selección: el
renderer Rust sumaba `height * 0.001` a la profundidad de todo suelo. OpenTTD
inserta `DrawGroundSprite` en un pase separado, barrido por `x + y` y luego
`y - x`; la altura sólo cambia la posición en pantalla. Ahora el suelo natural
usa esa profundidad diagonal, y `TileViewportBounds::iter_coords` reproduce el
barrido C++ para conservar el desempate incluso si dos valores `f32` coinciden.
El pase de suelo usa además una banda Z separada, por debajo de los padres
sortables, porque OpenTTD termina `DrawGroundSprite` antes de
`ViewportSortParentSprites`; esto evita que Bevy intercale ambos pases por
profundidad.
Las rampas/fundaciones conservan su orden local especial. Además, las rutas
`tile_pos_half`/`overlay_pos` comprimen sus capas positivas y la altura en el
margen de una fila; así una casa, estación o puente no puede invertir la fila
diagonal vecina por usar una capa local `0.4`/`0.5`. Las capas negativas de
agua/costa siguen siendo excepciones intencionales para colocar el borde entre
filas. La regresión `urban_layers_stay_inside_their_diagonal_row` cubre este
contrato.

La estación ferroviaria vanilla en `(226,42)` y `(227,42)` quedó explicada y
cubierta por regresión. La tesela tiene `RailType=Monorail`; OpenTTD aplica
`RailTypeInfo::GetRailtypeSpriteOffset()` (+82) tanto al suelo como a cada
capa de `DrawRailTileSeq`. Por eso emite `1093/1159/1151/1162/1166`, no la
familia rail/elrail `1011/1077/1069/1080/1084`. El cliente replica ese
contrato para rail/elrail, monorail y maglev (+0/+82/+164), recorta las tres
familias desde el NFO del perfil gráfico activo y conserva los offsets NFO
propios de 8bpp o 32bpp. La caja `TILE_SEQ` de los pilares Y-rear (1077 y sus
variantes) también quedó corregida a `5×16×2`, según `station_land.h`.

### Revalidación: paradas viales vanilla de Kale

El siguiente hueco de instrumentación no era una familia de terreno: eran las
paradas de bus/camión. OpenTTD usa los suelos de
`_station_display_datas_{bus,truck}` para las bahías y
`SPR_ROAD_PAVED_STRAIGHT_{X,Y}` para las pasantes; después agrega las capas
`TILE_SEQ_LINE` con paleta de compañía. En una pendiente, toda esa secuencia
se cuelga de `DrawFoundation(Foundation::Leveled)`. El cliente antes dibujaba
césped y una carretera basada en `m3` además de la bahía, y no declaraba esas
capas en `world-draw`, por lo que el auditor no podía distinguir la omisión de
una selección correcta.

La traza completa de `Kale_TitleGame.sav` identifica 457 teselas de parada y
las compara exactamente contra la referencia: 234 bases 1313 y 222 bases
1314; una bahía bus NE (`2692`, `2696`, `2700`, `2704`); y las tiras pasantes
`5978/5979 ×192`, `5980/5981 ×157`, `5982/5983 ×42` y `5984/5985 ×65`.
Cada subconjunto coincide en sprite, paleta, mundo, bbox, fundación y orden
relativo. La prueba Rust conserva la ausencia de la carretera heurística, y
`scripts/test_road_stop_sprite_variants.py` sintetiza ambos perfiles de
baseset para exigir los mismos IDs lógicos y cajas en 8bpp y 32bpp.

### Revalidación: fundaciones y ascensores de casas de Kale

`DrawTile_Town` primero aplica `DrawFoundation(Foundation::Leveled)` y sólo
después dibuja el suelo y el edificio de `town_land.h`. La implementación
anterior de casas usaba siempre `foundation_<tileh>` del bloque clásico; no
consultaba las dos paredes visibles y tampoco conservaba el segundo nivel de
una pendiente empinada. Por eso faltaban 1.943 fundaciones Action5 en la
traza, aunque ya estuvieran resueltas para vías y puentes.

La ruta de casas ahora usa el mismo plan de fundación que el renderer de
transporte. En Kale, `(9,4)` selecciona `5423` (bloque 1) y `(164,12)`
selecciona `5476` (bloque 3 sobre pendiente doble normal), como OpenTTD. La
regresión unitaria cubre además una pendiente empinada, que conserva los dos
pasos de fundación y su segundo nivel. También se incorporaron los 188
`SPR_LIFT` (`1443`) de Large Office: el child de
pantalla conserva, por ejemplo, `(14,48)` cuando `GetLiftPosition()==12`.
El suelo `s1` posterior a cada fundación inclinada ahora se adjunta al último
parent que deja `DrawFoundation`, igual que el child `1447` de Kale `(7,2)`
con offset `(0,-32)`; así el sorter global desplaza muro y suelo juntos. La
misma materialización runtime cubre el ground de rampa de puente: en Kale
`(109,28)` el foco C++/Rust contiene los 3 comandos, incluida la relación
`bridge-foundation-ground`, por selección, geometría, fundación y orden
relativo. Para rail, `(229,149)` contiene los 5 comandos: fundación, ground y
track se preservan como children del mismo parent, mientras que cerca y cable
mantienen sus propios productores. No convierte los demás children de
transporte o NewGRF en equivalentes.
La comparación completa contiene selección y orden relativo de esos 2.131
comandos de fundación/ascensor. Además, los 8.497 parents de edificio llevan
la caja exacta de `M(...)` y la altura efectiva posterior a la fundación. El
plan genérico de `DrawFoundation` conserva también la caja C++ para sus 3.014
parents de Kale: 1.943 de casas, 439 de rail, 351 de road, 154 de estación
rail, 101 de puentes y los grupos menores de depósitos, paradas y cruces. La
comparación `world-draw --geometry --foundations --order` contiene las 3.014
geometrías. La instrumentación posterior de los separadores invisibles de
puentes y túneles lleva el conjunto a 56.990 parents: el comparador los vincula
56.990/56.990 al vector final C++. La salida `world-draw` no se reordena para
obtener ese número, porque representa deliberadamente la inserción previa a
`ViewportSortParentSprites`; el runtime sí entrega esas constraints al sorter.
Los buildings vanilla y los sprites directos de esas fundaciones ya se etiquetan
como parents comunes, reciben su clave diagonal/ordinal de inserción y se
reubican con el mismo sorter. El mismo puente incorpora además los edificios
industriales vanilla planos sin animación: para el ejemplo de Kale `(186,1)`,
el sprite `2119` usa exactamente el prisma `(2976,16,8)..(2991,31,27)` de
`industry_land.h`. Es un subconjunto deliberado: la industria inclinada,
animada o NewGRF, los producers restantes, los children no vinculados fuera de
casas/rampas de puente/base-vía rail y clipping continúan como residual
explícito de #326.
Los cuerpos y unidades de vehículos ya portan la caja `Vehicle::bounds`, la
clave de inserción y la profundidad fuente del pase `ViewportAddVehicles`; la
sombra y el rotor de aeronave son children del cuerpo. Los offsets NFO de
Action1/2 también se aplican a carretera, barcos y aeronaves. El sprite-stack y
los producers dinámicos NewGRF siguen siendo residual explícito y no se
presentan como paridad completa.

### Revalidación: catenaria de estaciones ferroviarias de Kale

`DrawTile_Station` llama a `DrawRailCatenary` después del suelo y de la
reserva PBS, pero antes de `DrawRailTileSeq`. La ruta Bevy ya creaba esos
sprites, aunque los insertaba después de las capas del andén y no los emitía
en `world-draw`; la comparación no podía confirmar ni su posición ni su orden.

La ruta común ahora conserva el orden C++: postes PPP, cables PCP y recién
después las capas de estación. También separa los permisos de cables y
postes: una estación puede prohibir wire y permitir el pylon correspondiente.
En `(194,22)`, el cable `5649` tiene `bounds=(7,0,10; 1,15,1)` y aparece como
el ordinal 1, entre la vía `1011` y las capas `1077/1069/1080`, exactamente
como OpenTTD. Kale completo recupera 556 cables y 148 postes de estación sin
una selección, geometría u orden candidatos fuera del oráculo; el test unitario
mantiene ese ejemplo como regresión.

### Revalidación: catenaria especial, cruces y boyas

El tramo restante no reutiliza exactamente el mismo camino que una vía normal:
`DrawRailCatenary` usa una tabla propia para los depósitos, mientras que una
boca de túnel sólo puede reclamar el PCP de su borde exterior y combina su
cable con el techo. Reusar sin filtro el colector genérico producía postes
interiores adicionales en Kale, por ejemplo en `(170,81)` y `(180,127)`.

La traza conserva el cable de depósito antes de las capas BUILD y a la altura
de `GetTileMaxPixelZ`; en runtime, sus bounds y los de las fachadas entran en
el mismo vector local de `ViewportSortParentSprites` antes de asignar slots de
profundidad. Para un túnel filtra el PPP al lado opuesto a la dirección de
`m5`. Los cruces a nivel precargan el bloque completo
`1370..=1405`, para no omitir el suelo según railtype y superficie. Finalmente,
una boya emite primero el agua `4061` y luego el `TILE_SEQ_LINE` de `9282`.
Las regresiones unitarias cubren los cuatro sentidos de la boca/deposito y el
contrato de región `null` de una auditoría completa.

### Revalidación: señales ferroviarias de Kale

Las posiciones de señal no son una preferencia visual local: OpenTTD resuelve
`IsTrainSignalSideRight()` desde los settings persistidos. Kale guarda
`vehicle.road_side=Right` y
`construction.train_signal_side=RoadVehicleDrivingSide` en `PATS`; usar el
default izquierdo cambiaba de lado los 729 postes aunque los bytes de cada
tesela y sus sprites fueran correctos.

También se siguió `rail_cmd.cpp` hasta el draw call. En una vía X, `DrawSignals`
emite bit 3 hacia sudoeste (`pos=8`) y luego bit 2 hacia nordeste (`pos=9`).
Finalmente `GetSafeSlopeZ` consulta `GetSlopePixelZ_Rail`, que aplica
`GetRailFoundation` antes de evaluar la pendiente. En `(183,28)`, por ejemplo,
la fundación nivelada transforma la lectura de Z=2 del terreno crudo en Z=8,
que es la cota del poste de OpenTTD.

La regresión cubre el parseo sintético de `PATS`, ambos sentidos X y las
esquinas seguras de `GetSafeSlopeZ`. La exportación completa de Kale confirma
729/729 selecciones, IDs, geometrías y órdenes de señales contenidos en el
oráculo C++.

### Revalidación: bordes `Void` de Kale

Las teselas `MP_VOID` no son ausencia de dibujo. `void_cmd.cpp` siempre llama
a `DrawGroundSprite`: con `construction.freeform_edges=true` usa
`SPR_FLAT_BARE_LAND + SlopeToSpriteOffset(tileh)` y
`PALETTE_ALL_BLACK`; con bordes libres desactivados usa la familia equivalente
de agua. El importador ahora lee ese ajuste desde `PATS`/`OPTS` y conserva el
default moderno de OpenTTD (`true`).

Kale tiene 1.020 teselas `Void`, que antes se omitían por completo. Al
instrumentarlas apareció un segundo defecto específico del borde sur: el
renderer muestreaba `y + 1` fuera del mapa a altura cero. `GetTileSlopeZ`
real, en cambio, lo fija en `Map::MaxY()` (y hace lo mismo en X), por lo que
las 262 pendientes y cotas de ese borde quedaban incorrectas. La muestra de
esquina ahora se clampa como el código C++ y una regresión cubre mapas 1×1 y
2×1.

La comparación completa vuelve a contener las 1.020 decisiones del borde:
ID, paleta (`6140` para el negro), cota y orden relativo coinciden con el
oráculo. El hueco de cobertura de Kale baja de 2.067 a 1.047 comandos de
referencia. Para el modo sin bordes libres se añadieron las 19 pendientes de
agua al atlas y al extractor; su prueba sintética exige el recorte correcto
tanto en OpenGFX 8bpp como en OpenGFX2 32bpp.

## Procedimiento para investigar un caso nuevo

1. Reproducirlo con una partida inmutable y elegir una región mínima: los dos
   extremos de un puente, la boca de un túnel o el depósito afectado.
2. Comparar raw y semántica. Si alguno falla, arreglar importación o
   decodificación antes de tocar renderer.
3. Exportar `world-draw` de ambos lados para la región. Comparar primero
   sprite, rol, paleta, fundación y geometría. Si la selección coincide pero
   hay solape visible, exportar también `world-sort` y ubicar el primer parent
   que el sorter global mueve.
4. Seguir en C++ el `draw_tile_proc` y sus auxiliares; contrastar en Rust los
   bytes del tile, vecinos, pendiente, eje, railtype, reserva y altura efectiva.
5. Aplicar el cambio mínimo en la capa responsable, agregar una prueba/fixture
   del caso y repetir la traza. Recién después revisar la captura amplia.
6. Extraer el arreglo del checkpoint en un commit temático y verde. No mezclar
   un arreglo visual con simulación, atlas o SAV salvo que la traza pruebe la
   dependencia.

### Comandos reproducibles

Los scripts reciben una región inclusiva `x0,y0,x1,y1`. Ajustar las rutas al
entorno local. El binario C++ debe venir de la rama `openttdrs/oracle-parity`
del fork, no de su `main` oficial.

#### Oráculo integrado (recomendado)

Para un caso nuevo, usar primero un único comando. Exporta ambos lados en el
orden correcto (`raw → semantic → draw`), se detiene en la primera frontera
que diverge y deja JSONL, log y `report.json` por etapa. El código de salida
`1` significa una divergencia ya diagnosticable; `2` o mayor indica que no se
pudo producir la evidencia.

```bash
SAV=save/Kale_TitleGame.sav
./scripts/compare_sav_world.sh "$SAV" /tmp/kale-oracle \
  --tile 189,126 --radius 1 --max-diffs 20
```

`--tile X,Y --radius N` construye una región inclusiva; `--region
X0,Y0,X1,Y1` sirve para un rectángulo ya conocido. `--kind raw`, `semantic` o
`draw` permite repetir sólo una frontera después de haber confirmado sus
predecesoras. Por ejemplo, la salida de dibujo queda en
`/tmp/kale-oracle/draw/report.json` y contiene selección, geometría, paletas,
fundaciones, orden relativo y la primera diferencia.

Usar `--dry-run` para comprobar el alcance antes de ejecutar OpenTTD:

```bash
./scripts/compare_sav_world.sh "$SAV" /tmp/kale-oracle \
  --tile 189,126 --radius 1 --dry-run
```

```bash
SAV=save/Kale_TitleGame.sav
OTTD_BIN=/ruta/a/OpenTTD/build/openttd
REGION=171,109,171,110

# Nivel 1: bytes de mapa.
./scripts/export_openttd_world_raw.sh "$SAV" /tmp/openttd-raw.jsonl "$OTTD_BIN" "$REGION"
./scripts/export_openttdrs_world_raw.sh "$SAV" /tmp/openttdrs-raw.jsonl "$REGION"
python3 scripts/compare_world_raw.py /tmp/openttd-raw.jsonl /tmp/openttdrs-raw.jsonl --strict-metadata

# Nivel 2: semántica decodificada.
./scripts/export_openttd_world_semantic.sh "$SAV" /tmp/openttd-semantic.jsonl "$OTTD_BIN" "$REGION"
./scripts/export_openttdrs_world_semantic.sh "$SAV" /tmp/openttdrs-semantic.jsonl "$REGION"
python3 scripts/compare_world_semantic.py /tmp/openttd-semantic.jsonl /tmp/openttdrs-semantic.jsonl \
  --strict-metadata --show-inventory

# Nivel 3: decisiones de dibujo.
OPENTTDRS_OPENGFX_DIR=/ruta/a/opengfx \
  ./scripts/export_openttd_world_draw.sh "$SAV" /tmp/openttd-draw.jsonl "$OTTD_BIN" "$REGION"
./scripts/export_openttdrs_world_draw.sh "$SAV" /tmp/openttdrs-draw.jsonl "$REGION"
python3 scripts/compare_world_draw.py /tmp/openttd-draw.jsonl /tmp/openttdrs-draw.jsonl \
  --geometry --foundations --order --by-role

# Nivel 3b: ordenar los parents tras ViewportSortParentSprites.
OPENTTDRS_WORLD_SORT_OUT=/tmp/openttd-sort.jsonl \
  ./scripts/export_openttd_world_draw.sh "$SAV" /tmp/openttd-draw.jsonl "$OTTD_BIN" "$REGION"
python3 scripts/compare_world_sort.py /tmp/openttd-sort.jsonl /tmp/openttdrs-draw.jsonl
```

### Nivel 4: raster focalizado

Cuando raw, semántica y draw explican las decisiones pero la composición aún
parece distinta, capturar ambos viewports exactamente en la misma tesela. El
orquestador fija zoom normal/escala 1, pausa ambos motores, oculta UI/rótulos/
vehículos y usa el mismo perfil OpenGFX 8bpp; deja referencia, candidata, diff
y reporte hashable en un directorio. Para investigar una capa dinámica se
puede desactivar sólo esa limpieza con `OPENTTDRS_WORLD_SCREENSHOT_CLEAN=0`.

```bash
./scripts/compare_focused_world_screenshot.sh \
  "$SAV" /tmp/kale-raster-189-126 189,126 1280x720 1
```

El reporte incluye un registro de cámara de hasta ocho píxeles. Un
desplazamiento distinto de cero es una señal a investigar, no una corrección
que permita dar por buena la paridad. Ver el
[contrato raster](WORLD_SCREENSHOT_SCHEMA.md) para la semántica completa. El
resultado cuantitativo vigente se conserva únicamente en
[PARIDAD.md](../PARIDAD.md#evidencia-visual-raster-vigente); este documento no
lo duplica para que el método y el estado no diverjan.

Para el caso de regresión de la estación vanilla de Kale:

```bash
REGION=120,111,128,113
./scripts/export_openttd_world_draw.sh "$SAV" /tmp/kale-ottd-station.jsonl "$OTTD_BIN" "$REGION"
./scripts/export_openttdrs_world_draw.sh "$SAV" /tmp/kale-rust-station.jsonl "$REGION"
python3 scripts/compare_world_draw.py /tmp/kale-ottd-station.jsonl /tmp/kale-rust-station.jsonl \
  --geometry --order --by-role
```

Para repetir la regresión de campos/cercas en 8bpp:

```bash
REGION=225,25,251,61
./scripts/export_openttd_world_draw.sh "$SAV" /tmp/kale-ottd-fields.jsonl "$OTTD_BIN" "$REGION"
RUSTC_WRAPPER='' ./scripts/export_openttdrs_world_draw.sh "$SAV" /tmp/kale-rust-fields.jsonl "$REGION"
python3 scripts/compare_world_draw.py /tmp/kale-ottd-fields.jsonl /tmp/kale-rust-fields.jsonl \
  --geometry --order --by-role
```

Cuando se comparte cache de compilación, ejecutar con
`CARGO_INCREMENTAL=0`, `CARGO_NET_OFFLINE=true` y `RUSTC_WRAPPER=sccache`.
La configuración de compilación no debe cambiar la partida ni su traza.

## Interpretación de resultados

- **Falla raw:** revisar lector SAV, orden de teselas, versión del chunk o
  bytes `m1..m8` antes de mirar sprites.
- **Raw coincide y falla semántica:** revisar trackbits, railtype, eje,
  pendiente, extremos de túnel/puente, estación o depot.
- **Semántica coincide y falla draw:** revisar selección de sprite, paleta,
  fundación, offset y altura; vecinos y PBS pueden importar.
- **Orden relativo falla:** el candidato eligió comandos válidos, pero alteró
  la composición de capas de OpenTTD; inspeccionar la tesela y los vecinos que
  informa `candidate_order_missing_in_reference`.
- **Draw contenido pasa pero la captura falla:** puede faltar instrumentación,
  orden/solapamiento, clipping/cámara, atlas o una primitiva fuera de traza.
  Extender la traza antes de adivinar.
- **Hay fallback:** tratarlo como fallo explícito, con rol y motivo visibles;
  nunca reutilizar un sprite que parezca un objeto válido de otra clase.

## Límites y próximo hito

La cobertura `world-draw` permite usar `--strict-reference` como gate
estructural de la exportación completa actual de Kale/OpenGFX 8bpp; no es un
gate de composición ni una aceptación visual. La captura focalizada ya
encontró una diferencia global reproducible, cuyo estado canónico está en
[PARIDAD.md](../PARIDAD.md#evidencia-visual-raster-vigente). El próximo hito es
aislar y corregir el sort global y las familias de capas afectadas, repetir ese
baseline y luego ampliar la auditoría a OpenGFX2 32bpp, NewGRF reales y otras
partidas.

El estado exacto y la validación de la pausa se mantienen en el
[checkpoint](../checkpoints/2026-08-09-parity-oracle-pause.md). Los esquemas
son los contratos de largo plazo; este documento es el método operativo para
aplicarlos.
