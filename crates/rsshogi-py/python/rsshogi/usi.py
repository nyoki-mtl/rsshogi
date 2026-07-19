"""Stateless USI protocol value objects and parsers."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum

from rsshogi.core import Move

MAX_PLY = 246
VALUE_MATE = 32000
VALUE_MATE_IN_MAX_PLY = VALUE_MATE - MAX_PLY


class UsiScore(int):
    """USI score value in centipawns or mate distance encoding."""

    def is_mate_score(self) -> bool:
        return self >= VALUE_MATE_IN_MAX_PLY

    def is_mated_score(self) -> bool:
        return self <= -VALUE_MATE_IN_MAX_PLY

    def mate_in_ply(self) -> int | None:
        if not self.is_mate_score():
            return None
        return VALUE_MATE - int(self)

    def mated_in_ply(self) -> int | None:
        if not self.is_mated_score():
            return None
        return VALUE_MATE + int(self)

    def to_string(self) -> str:
        if self.is_mate_score():
            ply = self.mate_in_ply()
            return "mate" if ply == 0 else f"mate {ply}"
        if self.is_mated_score():
            ply = self.mated_in_ply()
            return "mate -" if ply == 0 else f"mate -{ply}"
        return f"cp {int(self)}"


class UsiBound(Enum):
    NONE = ""
    LOWER = "lowerbound"
    UPPER = "upperbound"
    EXACT = "exact"

    @classmethod
    def from_string(cls, text: str) -> UsiBound:
        if text == "lowerbound":
            return cls.LOWER
        if text == "upperbound":
            return cls.UPPER
        if text in {"", "exact"}:
            return cls.EXACT if text == "exact" else cls.NONE
        raise ValueError(f"unknown USI bound: {text}")

    def to_string(self) -> str:
        return self.value


@dataclass(frozen=True, slots=True)
class UsiInfo:
    pv: tuple[Move, ...] | None = None
    score: UsiScore | None = None
    bound: UsiBound = UsiBound.NONE
    depth: int | None = None
    seldepth: int | None = None
    nodes: int | None = None
    time: int | None = None
    hashfull: int | None = None
    nps: int | None = None
    multipv: int | None = None
    string: str | None = None

    @classmethod
    def parse(cls, line: str) -> UsiInfo:
        return parse_info(line)


@dataclass(frozen=True, slots=True)
class UsiBestMove:
    bestmove: Move
    ponder: Move | None = None

    @classmethod
    def parse(cls, line: str) -> UsiBestMove:
        return parse_bestmove(line)

    def to_string(self) -> str:
        text = f"bestmove {self.bestmove.to_usi()}"
        if self.ponder is not None:
            text += f" ponder {self.ponder.to_usi()}"
        return text


@dataclass(frozen=True, slots=True)
class UsiGoCommand:
    searchmoves: tuple[Move, ...] = ()
    ponder: bool = False
    btime: int | None = None
    wtime: int | None = None
    binc: int | None = None
    winc: int | None = None
    byoyomi: int | None = None
    infinite: bool = False
    nodes: int | None = None
    depth: int | None = None
    movetime: int | None = None

    def to_string(self) -> str:
        parts = ["go"]
        if self.searchmoves:
            parts.append("searchmoves")
            parts.extend(move.to_usi() for move in self.searchmoves)
        if self.ponder:
            parts.append("ponder")
        for name in ("btime", "wtime", "binc", "winc", "byoyomi"):
            value = getattr(self, name)
            if value is not None:
                parts.extend([name, str(value)])
        if self.infinite:
            parts.append("infinite")
        for name in ("nodes", "depth", "movetime"):
            value = getattr(self, name)
            if value is not None:
                parts.extend([name, str(value)])
        return " ".join(parts)


def move_from_usi(usi: str) -> Move:
    if usi == "resign":
        return Move.MOVE_RESIGN
    if usi == "win":
        return Move.MOVE_WIN
    if usi == "0000":
        return Move.MOVE_NULL
    if usi == "none":
        return Move.MOVE_NONE
    return Move.from_usi(usi)


def parse_score(tokens: list[str], index: int) -> tuple[UsiScore, UsiBound, int]:
    if index >= len(tokens):
        raise ValueError("USI score is missing a kind")
    kind = tokens[index]
    if kind == "cp":
        if index + 1 >= len(tokens):
            raise ValueError("USI cp score is missing a value")
        score = UsiScore(int(tokens[index + 1]))
        index += 2
    elif kind == "mate":
        if index + 1 >= len(tokens):
            raise ValueError("USI mate score is missing a value")
        value = tokens[index + 1]
        if value == "+":
            score = UsiScore(VALUE_MATE)
        elif value == "-":
            score = UsiScore(-VALUE_MATE)
        else:
            ply = int(value)
            score = UsiScore(VALUE_MATE - ply if ply >= 0 else -VALUE_MATE - ply)
        index += 2
    else:
        raise ValueError(f"unknown USI score kind: {kind}")

    bound = UsiBound.NONE
    if index < len(tokens) and tokens[index] in {"lowerbound", "upperbound"}:
        bound = UsiBound.from_string(tokens[index])
        index += 1
    return score, bound, index


def parse_info(line: str) -> UsiInfo:
    tokens = line.strip().split()
    if not tokens or tokens[0] != "info":
        raise ValueError("USI info line must start with 'info'")

    data: dict[str, object] = {}
    index = 1
    while index < len(tokens):
        key = tokens[index]
        index += 1
        if key == "pv":
            data["pv"] = tuple(move_from_usi(token) for token in tokens[index:])
            break
        if key == "score":
            score, bound, index = parse_score(tokens, index)
            data["score"] = score
            data["bound"] = bound
            continue
        if key == "string":
            data["string"] = " ".join(tokens[index:])
            break
        if key in {"depth", "seldepth", "nodes", "time", "hashfull", "nps", "multipv"}:
            if index >= len(tokens):
                raise ValueError(f"USI info field '{key}' is missing a value")
            data[key] = int(tokens[index])
            index += 1
    return UsiInfo(**data)


def parse_bestmove(line: str) -> UsiBestMove:
    tokens = line.strip().split()
    if len(tokens) < 2 or tokens[0] != "bestmove":
        raise ValueError("USI bestmove line must start with 'bestmove'")
    ponder = None
    if len(tokens) >= 4 and tokens[2] == "ponder":
        ponder = move_from_usi(tokens[3])
    return UsiBestMove(move_from_usi(tokens[1]), ponder)


__all__ = [
    "MAX_PLY",
    "VALUE_MATE",
    "VALUE_MATE_IN_MAX_PLY",
    "UsiBestMove",
    "UsiBound",
    "UsiGoCommand",
    "UsiInfo",
    "UsiScore",
    "move_from_usi",
    "parse_bestmove",
    "parse_info",
    "parse_score",
]
