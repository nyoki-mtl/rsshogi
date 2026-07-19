"""NumPy dtypes for packed SFEN and related data structures."""

from rsshogi._rsshogi import dtype

# Re-export the dtype module contents
__getattr__ = dtype.__getattr__

try:
    PackedSfen = dtype.PackedSfen
    PackedSfenValue = dtype.PackedSfenValue
    HuffmanCodedPos = dtype.HuffmanCodedPos
    HuffmanCodedPosAndEval = dtype.HuffmanCodedPosAndEval
    __all__ = ["PackedSfen", "PackedSfenValue", "HuffmanCodedPos", "HuffmanCodedPosAndEval"]
except (ImportError, AttributeError):
    # numpy is not available
    __all__ = []
