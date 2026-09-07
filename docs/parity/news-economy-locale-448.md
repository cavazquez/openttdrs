# Noticias de economía y locale (#448)

Implementado en `main` el 2026-09-07.

## Divergencia auditada

Las noticias de recesión se generan en el core con cuatro frases estáticas,
pero el cliente sólo podía traducir los títulos de configuración de noticias.
Al cambiar a inglés, el ticker y el popup dejaban esas frases en español.

## Implementación

El catálogo de `Locale::En` incorpora las dos variantes de headline y las dos
variantes de body emitidas por `push_economy_fluctuation_news`. El sistema de
localización las registra como texto de UI aunque la noticia se cree después de
inicializar el HUD, por lo que el locale activo también se aplica al texto
materializado tardíamente. Las cadenas con parámetros (carga, importe,
coordenadas o nombre de compañía) no se traducen por coincidencia literal y
conservan los datos de la partida.

La regresión
`localization_plugin_translates_static_economy_news_only` cubre el cambio
Español→English→Español y verifica que una headline parametrizada permanece
intacta. La tabla de idiomas completa y la migración de `NewsItem` a plantillas
parametrizadas siguen pendientes en #331.
