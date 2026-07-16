# ADR 0004 — Host migration post-v1 (listen-server)

- **Estado:** aceptada
- **Fecha:** 2026-07-16
- **Issues:** [#21](https://github.com/cavazquez/openttdrs/issues/21) (transporte v1); issue de seguimiento de migración (fases)
- **Supersede:** el punto «fuera de v1 — host migration» de [ADR 0001](0001-multiplayer-v1.md)
- **Protocolo:** `PROTOCOL_VERSION = 2` (`openttdrs-net`)

## Contexto

ADR 0001 fijó lockstep TCP con listen-server + dedicated y **sin** transferencia de
autoridad: si cae el host, save/restart o dedicated. Ya existen late-join por
snapshot JSON y `next_seq` en `Welcome`, pero no hay elección de host ni
promoción `Client → ListenServer`.

El caso doloroso es el **listen-server** (jugador con UI). Dedicated en VPS mitiga
el caso desatendido y queda fuera de este MVP.

## Decisión

1. **Alcance MVP:** failover solo cuando cae el listen-server con ≥1 peer vivo.
2. **Identidad:** cada peer recibe un `peer_id` monótono (`u64`) en `Welcome`.
3. **Elección:** el **menor `peer_id` vivo** se promueve (`elect_new_host`). Sin
   latencia ni consenso distribuido.
4. **Sincronización:** pausa → nuevo host levanta `ListenServer` con snapshot
   actual + `next_seq` → resto reconecta con `Hello`/`Welcome` (mismo late-join).
5. **Protocolo v2:** `Welcome.peer_id`, `HostAnnounce { bind, next_seq, new_host_peer_id }`
   para anunciar el nuevo bind cuando haya canal entre peers; el esqueleto actual
   deja el mensaje listo (el test de integración puede orquestar el reconnect).
6. **Fuera de MVP:** migración de dedicated, heartbeats/timeouts, auto-promote en
   Bevy, failover sin pausa, anti-cheat, log completo en disco.

## Consecuencias

- Clientes v1 (`PROTOCOL_VERSION = 1`) no son compatibles; bump explícito.
- Bevy aún no auto-promueve: piezas en `openttdrs-net` + test TCP; UI en fase siguiente.
- ADR 0001 sigue vigente para el modelo lockstep; solo cambia el “fuera de v1” de migración.

## Alternativas descartadas

| Alternativa | Por qué no (MVP) |
|-------------|------------------|
| Solo save/restart | Status quo v1; no cubre partida en caliente. |
| Raft / consenso | Complejidad injustificada con pocos peers. |
| State-sync / rollback | Choca con lockstep de comandos (ADR 0001). |
| Elegir por latencia | No determinista; complica tests. |
