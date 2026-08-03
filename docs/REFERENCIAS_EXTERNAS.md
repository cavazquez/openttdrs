# Referencias externas de implementación

Estos repositorios son material de estudio local. No son dependencias, no se
compilan como parte de `openttdrs` y no se debe copiar código sin revisar antes
su licencia y atribución. El oráculo de paridad sigue siendo únicamente
[OpenTTD 15.3](parity/openttd-reference.json).

| Proyecto | Commit local | Licencia | Valor para `openttdrs` |
|---|---|---|---|
| [OpenLoco](https://github.com/OpenLoco/OpenLoco) | `21264c5bbf83` | MIT | Reimplementación madura de un simulador de transporte: modelos de vehículo, ventanas, flujos de construcción y compatibilidad de saves. |
| [FreeRCT](https://github.com/FreeRCT/FreeRCT) | `d532ee693d1a` | GPL-2.0-only | UI y render isométricos, manejo de sprites, mapa y herramientas de construcción con assets libres. |

Ambos clones viven en `reference/ecosystem/`, están excluidos de Git y se usan
solo como consulta. Para actualizarlos deliberadamente:

```bash
git -C reference/ecosystem/openloco fetch --depth 1 origin master
git -C reference/ecosystem/freerct fetch --depth 1 origin master
```

Tras actualizar, registrar los nuevos SHA en esta tabla. No sustituir los
oráculos ni la referencia fijada de OpenTTD con estos proyectos.
