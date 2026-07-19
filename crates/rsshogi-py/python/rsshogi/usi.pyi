from __future__ import annotations

from enum import Enum

from rsshogi.core import Move

MAX_PLY: int
VALUE_MATE: int
VALUE_MATE_IN_MAX_PLY: int

class UsiScore(int):
    def is_mate_score(self) -> bool: ...
    def is_mated_score(self) -> bool: ...
    def mate_in_ply(self) -> int | None: ...
    def mated_in_ply(self) -> int | None: ...
    def to_string(self) -> str: ...

class UsiBound(Enum):
    NONE: UsiBound
    LOWER: UsiBound
    UPPER: UsiBound
    EXACT: UsiBound
    @classmethod
    def from_string(cls, text: str) -> UsiBound: ...
    def to_string(self) -> str: ...

class UsiInfo:
    pv: tuple[Move, ...] | None
    score: UsiScore | None
    bound: UsiBound
    depth: int | None
    seldepth: int | None
    nodes: int | None
    time: int | None
    hashfull: int | None
    nps: int | None
    multipv: int | None
    string: str | None
    def __init__(
        self,
        pv: tuple[Move, ...] | None = ...,
        score: UsiScore | None = ...,
        bound: UsiBound = ...,
        depth: int | None = ...,
        seldepth: int | None = ...,
        nodes: int | None = ...,
        time: int | None = ...,
        hashfull: int | None = ...,
        nps: int | None = ...,
        multipv: int | None = ...,
        string: str | None = ...,
    ) -> None: ...
    @classmethod
    def parse(cls, line: str) -> UsiInfo: ...

class UsiBestMove:
    bestmove: Move
    ponder: Move | None
    def __init__(self, bestmove: Move, ponder: Move | None = ...) -> None: ...
    @classmethod
    def parse(cls, line: str) -> UsiBestMove: ...
    def to_string(self) -> str: ...

class UsiGoCommand:
    searchmoves: tuple[Move, ...]
    ponder: bool
    btime: int | None
    wtime: int | None
    binc: int | None
    winc: int | None
    byoyomi: int | None
    infinite: bool
    nodes: int | None
    depth: int | None
    movetime: int | None
    def __init__(
        self,
        searchmoves: tuple[Move, ...] = ...,
        ponder: bool = ...,
        btime: int | None = ...,
        wtime: int | None = ...,
        binc: int | None = ...,
        winc: int | None = ...,
        byoyomi: int | None = ...,
        infinite: bool = ...,
        nodes: int | None = ...,
        depth: int | None = ...,
        movetime: int | None = ...,
    ) -> None: ...
    def to_string(self) -> str: ...

def move_from_usi(usi: str) -> Move: ...
def parse_score(tokens: list[str], index: int) -> tuple[UsiScore, UsiBound, int]: ...
def parse_info(line: str) -> UsiInfo: ...
def parse_bestmove(line: str) -> UsiBestMove: ...

__all__: list[str]
