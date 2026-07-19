"""Policy label helpers for machine-learning pipelines."""

from rsshogi._rsshogi import (
    COMPACT_MOVE_LABEL_COUNT as COMPACT_MOVE_LABEL_COUNT,
)
from rsshogi._rsshogi import (
    COMPACT_TO_MOVE_LABEL as _COMPACT_TO_MOVE_LABEL,
)
from rsshogi._rsshogi import (
    MOVE_LABEL_COUNT as MOVE_LABEL_COUNT,
)
from rsshogi._rsshogi import (
    MOVE_LABEL_TO_COMPACT as _MOVE_LABEL_TO_COMPACT,
)
from rsshogi._rsshogi import (
    compact_move_label as compact_move_label,
)
from rsshogi._rsshogi import (
    compact_move_label_to_move_label as compact_move_label_to_move_label,
)
from rsshogi._rsshogi import (
    is_structurally_valid_move_label as is_structurally_valid_move_label,
)
from rsshogi._rsshogi import (
    move_label as move_label,
)
from rsshogi._rsshogi import (
    move_label_to_compact as move_label_to_compact,
)

MOVE_LABEL_TO_COMPACT = tuple(_MOVE_LABEL_TO_COMPACT)
COMPACT_TO_MOVE_LABEL = tuple(_COMPACT_TO_MOVE_LABEL)

__all__ = [
    "MOVE_LABEL_COUNT",
    "COMPACT_MOVE_LABEL_COUNT",
    "MOVE_LABEL_TO_COMPACT",
    "COMPACT_TO_MOVE_LABEL",
    "move_label",
    "compact_move_label",
    "move_label_to_compact",
    "compact_move_label_to_move_label",
    "is_structurally_valid_move_label",
]
