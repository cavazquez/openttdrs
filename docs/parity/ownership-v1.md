# V1-OWN: aislamiento de bienes por issuer de red

Contrato acotado de [#591](https://github.com/cavazquez/openttdrs/issues/591).
La prueba usa `openttdrs_net::apply_command_as_company`, la misma API que
aplica un `SessionEvent::Commit` recibido por el servidor o un cliente.

## Fixture y resultado

La fixture crea una carretera, depósito y bus de A; luego materializa B con el
issuer productivo y le asigna infraestructura propia. B no puede ejecutar sobre
los bienes de A:

- comprar un bus en el depósito de A;
- iniciar/detener el vehículo de A;
- sustituir las órdenes del vehículo de A;
- demoler la carretera de A.

Cada uno devuelve el error de ownership correspondiente y conserva exactamente
el JSON persistido, `canonical_hash`, ambos RNGs y el saldo de cada compañía.
Un issuer `CompanyId(MAX_COMPANIES)` también es rechazado antes de tocar el
estado.

Los cuatro controles equivalentes de A sí se ejecutan: compra un bus propio,
arranca su vehículo, edita sus órdenes y demuele su carretera. Las operaciones
que cuestan dinero se cargan exclusivamente a A; B conserva su saldo.

Desde la raíz del repositorio:

```sh
cargo test --locked --offline -p openttdrs-net --test v1_ownership -- --exact --nocapture
```

Takeovers, bancarrota, infraestructura compartida, hotseat/UI y ampliaciones
del protocolo siguen fuera del corte. El cierre de #591 requiere CI remota
verde para el SHA publicado que contiene esta regresión.
