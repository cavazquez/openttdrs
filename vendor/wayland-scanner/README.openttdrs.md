Vendored crates.io `wayland-scanner` 0.31.10 with:
- `quick-xml` bumped 0.39 → 0.41 (RUSTSEC-2026-0194 / 0195)
- `xml_content()` → `xml10_content()` (quick-xml 0.41 API)

Equivalent intent to Smithay/wayland-rs@d07c4f9 without taking newer scanner codegen
that breaks wayland-client 0.31.14. Remove when upstream publishes a compatible release.
