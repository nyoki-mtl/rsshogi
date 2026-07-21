"""rsshogi: Python bindings for shogi library.

This package provides structured imports through submodules:

Core primitives:
    >>> from rsshogi.core import Board, Move32, Move

Core primitives:
    >>> from rsshogi.core import Board, Move32, Move

Types and Constants:
    >>> from rsshogi.types import Color, PieceType, Piece, Square

Initial Positions:
    >>> from rsshogi.initial_positions import InitialPosition

Records:
    >>> from rsshogi.record import Record, RecordMetadata, GameResult

Record conversion:
    >>> record = Record.from_kif_str(kif_text)
    >>> kif_text = record.to_kif()

Record file I/O:
    >>> record = Record.from_kif_file("example.kif")
    >>> record.write_kif("example_out.kif")

Book (Opening Book):
    >>> from rsshogi.book import StaticBook, MemoryBook, BookBuilder
    >>> from rsshogi.book import book_key_from_position, book_key_after

Policy labels:
    >>> from rsshogi.policy import move_label, compact_move_label

NumPy:
    >>> from rsshogi.numpy import PackedSfen, PackedSfenValue

AlphaZero training format (sazpack / SAZ1):
    >>> from rsshogi.sazpack import SazGame, SazPosition, SazPolicyEntry, SazWdl
    >>> from rsshogi.sazpack import write_sazpack, decode_sazpack
"""

# Import all submodules
from . import book, core, initial_positions, numpy, policy, record, sazpack, svg, types, usi

# Version
__version__ = "1.0.2"

__all__ = [
    # Submodules
    "types",
    "core",
    "record",
    "book",
    "policy",
    "numpy",
    "sazpack",
    "svg",
    "usi",
    "initial_positions",
]
