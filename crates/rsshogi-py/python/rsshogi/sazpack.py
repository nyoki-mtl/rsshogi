"""Typed SAZ2 self-play training records."""

from rsshogi._rsshogi import (
    SazGame,
    SazPolicyEntry,
    SazPosition,
    SazWdl,
    decode_sazpack,
    decode_sazpack_file,
    write_sazpack,
    write_sazpack_file,
)

__all__ = [
    "SazGame",
    "SazPolicyEntry",
    "SazPosition",
    "SazWdl",
    "decode_sazpack",
    "decode_sazpack_file",
    "write_sazpack",
    "write_sazpack_file",
]
