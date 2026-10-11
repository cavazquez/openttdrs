# Embedded OpenGFX foundations

`ogfxe_extra.grf` is the unmodified OpenGFX 8.0 extra graphics file. The core
loads only its 90 Action5 foundation sprites, then applies the game's explicit
NewGRF replacements. Embedding it makes the base geometry independent of the
working directory and of a developer's downloaded GRFs.

- Upstream: https://github.com/OpenTTD/OpenGFX/tree/8.0
- Release: https://cdn.openttd.org/opengfx-releases/8.0/
- Archive: `opengfx-8.0-all.zip`, SHA-256
  `43a0c1dabf39cb865394f3a6cc36d4da5c10ecfaaf55652043104806810903be`.
- SHA-256: `6472d934f15210dc4ad03acaad12d1e2717f4f5f836495cdd78bfc45fcbb0adb`
- Copyright and credits: the accompanying upstream `readme.txt`.
- License: GPL version 2, preserved verbatim in `license.txt`.

This is base-set data. It does not add support for arbitrary NewGRFs or claim
parity of all foundation shapes.
