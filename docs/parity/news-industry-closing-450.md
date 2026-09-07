# Cuerpo de cierre de industria y locale (#450)

Implementado en `main` el 2026-09-07.

`report_industry_closing` utiliza un body fijo junto a un headline que incluye
coordenadas. Se agregó al catálogo inglés sólo el body:
`Dejará de producir y desaparecerá el mes que viene.` → `It will stop
producing and disappear next month.`. El headline y cualquier nombre o
coordenada siguen siendo datos de la partida y permanecen en el idioma fuente.

La regresión `localization_plugin_translates_industry_closing_body_only`
comprueba que el body sigue el locale en vivo mientras el headline parametrizado
no se altera. La migración de cuerpos parametrizados y el resto del catálogo de
noticias continúa en #331.
