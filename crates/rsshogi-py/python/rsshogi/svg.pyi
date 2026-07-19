"""Type stubs for rsshogi.svg — SVG rendering for shogi board positions."""

from __future__ import annotations

class Svg:
    def _repr_svg_(self) -> str: ...
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...

__all__ = ["Svg"]
