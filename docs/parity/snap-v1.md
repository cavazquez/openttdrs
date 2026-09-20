# Smoke de Snap V1

Los issues [#582](https://github.com/cavazquez/openttdrs/issues/582) y
[#583](https://github.com/cavazquez/openttdrs/issues/583) separan dos cosas:
el build debe incluir los tiles OpenGFX derivados antes de empaquetar, y el
artefacto resultante debe arrancar con la raíz `$SNAP` de sólo lectura y un
perfil nuevo.

La receta `snap/snapcraft.yaml` materializa los tiles con `--check-assets`
antes de copiar `assets/` a `prime`. El smoke extrae el `.snap`, valida los
assets exigidos, vuelve la raíz extraída de sólo lectura y usa `SNAP_USER_COMMON`
temporal. Después arranca el launcher bajo Xvfb y exige una captura de menú
1280×720 no vacía. Un tile ausente, escritura en `$SNAP`, panic, timeout o
captura vacía falla: no se admite `skip`.

Para conservar evidencia aun cuando falle la sesión gráfica:

```sh
OPENTTDRS_SNAP_SMOKE_ARTIFACT_DIR=artifacts/snap-smoke \
  ./scripts/smoke_snap_package.sh openttdrs_<version>_amd64.snap
```

El directorio guarda `package.sha256`, `check-assets.log`, `menu.log` y, si el
cliente llegó a capturarla, `menu.png`. El smoke no instala ni publica el Snap,
por lo que la revisión de tienda y una instalación estricta nueva siguen siendo
evidencia externa necesaria antes de cerrar los dos issues.
