#!/usr/bin/env python3
"""Compatibilidad mínima para recorrer píxeles de Pillow indexados o RGBA.

Pillow reciente expone :meth:`Image.get_flattened_data`; las versiones
distribuidas por algunos runners mantienen sólo ``getdata``. Los generadores
de sprites necesitan el mismo orden de píxeles en ambos casos, por lo que el
fallback queda centralizado aquí en vez de pinnear una versión del paquete.
"""

from __future__ import annotations

from typing import Any


def flattened_data(image: Any) -> Any:
    """Devuelve los píxeles en orden fila-major en Pillow nuevo y antiguo."""
    modern = getattr(image, "get_flattened_data", None)
    if callable(modern):
        return modern()
    return image.getdata()
