"""Game record types and typed helpers for shogi."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Literal, TypedDict

from rsshogi._rsshogi import (
    EngineInfo,
    GameResult,
    GameResultInfo,
    MoveEntry,
    Record,
    RecordEditor,
    RecordEntry,
    RecordMetadata,
    RecordMetadataKey,
    RecordNodeId,
    SpecialMoveEntry,
    TimeControl,
    decode_pack,
    decode_pack_file,
    decode_sbinpack,
    decode_sbinpack_file,
    write_pack,
    write_pack_file,
    write_sbinpack,
    write_sbinpack_file,
)

StrictMode = Literal["strict", "permissive"]
EngineExtraValue = str | int | float | bool


class TimeControlDict(TypedDict, total=False):
    base_seconds: int
    byoyomi_seconds: int
    increment_seconds: int


class EngineInfoDict(TypedDict, total=False):
    eval: int | None
    depth: int | None
    nodes: int | None
    seldepth: int | None
    extras: dict[str, EngineExtraValue]


class MoveEntryDict(TypedDict, total=False):
    move: MoveEntry | int | str
    time_ms: int | None
    comment: str | None
    engine_info: EngineInfo | EngineInfoDict | None


class GameResultInfoDict(TypedDict, total=False):
    result: GameResult | int | str
    ply_count: int
    reason: str | None
    end_time_ms: int | None
    end_comment: str | None


class RecordMetadataDict(TypedDict, total=False):
    event: str | None
    site: str | None
    black_player: str | None
    white_player: str | None
    game_name: str | None
    game_type: str | None
    time_control: TimeControl | TimeControlDict | str | None
    black_time_control: TimeControl | TimeControlDict | str | None
    white_time_control: TimeControl | TimeControlDict | str | None
    max_moves: int | None
    impasse_rule: str | None
    start_date: str | None
    end_date: str | None
    updated_date: str | None
    comment: str | None
    attributes: dict[str, str]


class RecordDict(TypedDict, total=False):
    version: int
    init_position_sfen: str
    initial_comment: str | None
    moves: list[MoveEntryDict]
    metadata: RecordMetadata | RecordMetadataDict
    result: GameResultInfoDict | GameResult | int | str


@dataclass(slots=True)
class RecordMetadataBuilder:
    event: str | None = None
    site: str | None = None
    black_player: str | None = None
    white_player: str | None = None
    game_name: str | None = None
    game_type: str | None = None
    time_control: TimeControl | None = None
    black_time_control: TimeControl | None = None
    white_time_control: TimeControl | None = None
    max_moves: int | None = None
    impasse_rule: str | None = None
    start_date: str | None = None
    end_date: str | None = None
    updated_date: str | None = None
    comment: str | None = None
    attributes: dict[str, str] = field(default_factory=dict)

    def build(self) -> RecordMetadata:
        return RecordMetadata(
            event=self.event,
            site=self.site,
            black_player=self.black_player,
            white_player=self.white_player,
            game_name=self.game_name,
            game_type=self.game_type,
            time_control=self.time_control,
            black_time_control=self.black_time_control,
            white_time_control=self.white_time_control,
            max_moves=self.max_moves,
            impasse_rule=self.impasse_rule,
            start_date=self.start_date,
            end_date=self.end_date,
            updated_date=self.updated_date,
            comment=self.comment,
            attributes=dict(self.attributes),
        )


@dataclass(slots=True)
class RecordBuilder:
    init_position_sfen: str
    moves: list[MoveEntry] = field(default_factory=list)
    metadata: RecordMetadata | RecordMetadataBuilder | None = None
    terminal: SpecialMoveEntry | None = None
    initial_comment: str | None = None

    def build(self) -> Record:
        metadata = self.metadata.build() if isinstance(self.metadata, RecordMetadataBuilder) else self.metadata
        return Record.from_main_line(
            self.init_position_sfen,
            list(self.moves),
            self.terminal,
            metadata,
            self.initial_comment,
        )


__all__ = [
    "EngineExtraValue",
    "Record",
    "RecordBuilder",
    "RecordDict",
    "RecordEditor",
    "RecordEntry",
    "RecordMetadata",
    "RecordMetadataBuilder",
    "RecordMetadataDict",
    "RecordMetadataKey",
    "RecordNodeId",
    "GameResult",
    "GameResultInfo",
    "GameResultInfoDict",
    "EngineInfo",
    "EngineInfoDict",
    "MoveEntry",
    "MoveEntryDict",
    "SpecialMoveEntry",
    "StrictMode",
    "TimeControl",
    "TimeControlDict",
    "decode_pack",
    "decode_pack_file",
    "decode_sbinpack",
    "decode_sbinpack_file",
    "write_pack",
    "write_pack_file",
    "write_sbinpack",
    "write_sbinpack_file",
]
