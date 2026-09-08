# ADR 0004 — Host migration post-v1 (listen-server)

- **Estado:** aceptada
- **Fecha:** 2026-07-16
- **Issues:** [#21](https://github.com/cavazquez/openttdrs/issues/21) (transporte v1); [#171](https://github.com/cavazquez/openttdrs/issues/171) (auto-promote Bevy + heartbeats)
- **Supersede:** el punto «fuera de v1 — host migration» de [ADR 0001](0001-multiplayer-v1.md)
- **Protocolo:** `PROTOCOL_VERSION = 5` (`openttdrs-net`)

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
5. **Protocolo v2:** `Welcome.peer_id`, `PeerList`, `Heartbeat`, `HostAnnounce`.
6. **Protocolo v3:** la frontera snapshot + `next_seq` se publica de forma atómica respecto de commits y avances.
7. **Protocolo v4:** el dominio de `canonical_hash` v2 canoniza las listas de animación NewGRF; se rechazan peers v3 para que no comparen algoritmos de hash distintos.
8. **Protocolo v5:** la cola ordenada de ascensores activos pasa a ser estado autoritativo y el dominio de `canonical_hash` v3; se rechazan peers v4 que no pueden reanudarla.
9. **Cliente Bevy (#171):** silencio >2 s o `Disconnected` → `elect_new_host` →
   bind `puerto+1` (ganador) o reconnect (perdedor); pausa de sim + banner
   “reconectando…” / “promoviendo host…”; `HostAnnounce` fija destino sin
   orquestación manual.
10. **Fuera aún:** migración de dedicated entre máquinas, failover sin pausa,
   anti-cheat, log completo en disco.

## Consecuencias

- Clientes v1–v4 no son compatibles; cada bump explícito protege una semántica lockstep distinta.
- Bevy auto-promueve / reconecta (puerto+1 + `HostAnnounce`); banner mínimo de failover.
- ADR 0001 sigue vigente para el modelo lockstep; solo cambia el “fuera de v1” de migración.

## Alternativas descartadas

| Alternativa | Por qué no (MVP) |
|-------------|------------------|
| Solo save/restart | Status quo v1; no cubre partida en caliente. |
| Raft / consenso | Complejidad injustificada con pocos peers. |
| State-sync / rollback | Choca con lockstep de comandos (ADR 0001). |
| Elegir por latencia | No determinista; complica tests. |
