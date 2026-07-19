from __future__ import annotations

from rsshogi.core import Board, PositionState, ValidationReport
from rsshogi.record import (
    EngineInfo,
    GameResultInfo,
    MoveEntry,
    Record,
    RecordBuilder,
    RecordEditor,
    RecordEntry,
    RecordMetadataBuilder,
    RecordMetadataKey,
    RecordNodeId,
    SpecialMoveEntry,
    TimeControl,
)
from rsshogi.types import Color, Piece, PieceType, Square


def build_record() -> Record:
    metadata = RecordMetadataBuilder(
        game_name="match-1",
        game_type="tournament",
        updated_date="2026-02-13T00:00:00+00:00",
        black_time_control=TimeControl(300, 10, 0),
        white_time_control=TimeControl(300, 10, 0),
    )
    move = MoveEntry(
        "7g7f",
        engine_info=EngineInfo(
            eval=120,
            wall_time_ms=830,
            latency_delta_ms=12,
        ),
    )
    record = RecordBuilder(
        init_position_sfen="lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
        moves=[move],
        metadata=metadata,
        terminal=SpecialMoveEntry("RESIGN", "WHITE_WIN"),
        initial_comment="序文",
    ).build()

    _initial_comment: str | None = record.initial_comment
    _game_name: str | None = record.game_name
    _game_type: str | None = record.game_type
    _updated: str | None = record.updated_date
    _black_tc = record.black_time_control
    _white_tc = record.white_time_control
    _end_time: int | None = record.end_time_ms
    _end_comment: str | None = record.end_comment
    _metadata_updated: str | None = record.metadata.updated_date
    _metadata_black = record.metadata.black_time_control
    _wall_time: int | None = move.engine_info.wall_time_ms if move.engine_info else None
    _latency: int | None = move.engine_info.latency_delta_ms if move.engine_info else None

    record.update_metadata({"game_name": "match-2"}, strict=True)
    _node_id = RecordNodeId(record.root_node_id())
    _raw_node_id: int = int(_node_id)
    _from_usi: Record = Record.from_usi_position("position startpos moves 7g7f")
    _usi_text: str = _from_usi.to_usi_position()
    _from_jkf: Record = Record.from_jkf_str(record.to_jkf())
    _editor: RecordEditor = record.into_editor()
    _editor_node: int = _editor.current_node
    _editor_sfen: str = _editor.position_sfen
    _editor_record: Record = _editor.record()
    _entry: RecordEntry | None = record.node_entry(record.main_line_ids()[0])
    _metadata_key: str = RecordMetadataKey.BLACK_PLAYER
    return record


def edit_raw_state() -> tuple[PositionState, ValidationReport]:
    board = Board()
    state = board.to_position_state()
    state.set_piece(Square.from_usi("7f"), Piece.from_color_type(Color.BLACK, PieceType.PAWN))
    state.side_to_move = Color.WHITE
    state.ply = 12

    board.set_position_state(state)
    _valid: bool = board.is_valid()
    report = board.validate_all()
    _issues = report.issues
    return state, report


def reject_invalid_assignments(
    record: Record,
    info: GameResultInfo,
    move: MoveEntry,
    special: SpecialMoveEntry,
    terminal: SpecialMoveEntry,
) -> None:
    # read-only properties on Record reject assignment (getter-only in PyO3).
    # Each suppression must match a real error; --error unused-ignore-comment turns a
    # missing error (i.e. the property silently became writable) into a hard failure.
    record.init_position_sfen = "x"  # ty: ignore[invalid-assignment]
    record.moves = []  # ty: ignore[invalid-assignment]
    record.result = record.result  # ty: ignore[invalid-assignment]
    record.result_info = info  # ty: ignore[invalid-assignment]
    record.game_name = "x"  # ty: ignore[invalid-assignment]
    record.game_type = "x"  # ty: ignore[invalid-assignment]
    record.updated_date = "x"  # ty: ignore[invalid-assignment]
    record.black_time_control = None  # ty: ignore[invalid-assignment]
    record.white_time_control = None  # ty: ignore[invalid-assignment]
    record.end_time_ms = 1  # ty: ignore[invalid-assignment]
    record.end_comment = "x"  # ty: ignore[invalid-assignment]

    # GameResultInfo is fully read-only.
    info.result = info.result  # ty: ignore[invalid-assignment]
    info.ply_count = 1  # ty: ignore[invalid-assignment]
    info.reason = None  # ty: ignore[invalid-assignment]
    info.end_time_ms = None  # ty: ignore[invalid-assignment]
    info.end_comment = None  # ty: ignore[invalid-assignment]

    # Identity fields on MoveEntry / SpecialMoveEntry are read-only.
    move.move = move.move  # ty: ignore[invalid-assignment]
    special.kind = "RESIGN"  # ty: ignore[invalid-assignment]
    special.unknown_name = None  # ty: ignore[invalid-assignment]
    special.result = special.result  # ty: ignore[invalid-assignment]

    # main_terminal has a setter, but the PyO3 setter rejects None at runtime, so the
    # stub setter takes a non-optional SpecialMoveEntry: None must fail to type-check,
    # while a SpecialMoveEntry assignment is accepted.
    record.main_terminal = None  # ty: ignore[invalid-assignment]
    record.main_terminal = terminal
