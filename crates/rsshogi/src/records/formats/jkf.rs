use crate::board::{Position, SfenError};
use crate::records::error::RecordError;
use crate::records::formats::common::refresh_position_if_needed;
use crate::records::record::{
    MoveEntry, Record, RecordAnnotation, RecordInitialPosition, RecordMetadata, RecordNodeId,
    SpecialMove, SpecialMoveEntry,
};
use crate::records::time_control::{TimeControl, parse_kif_time_control};
use crate::types::{Color, GameResult, HandPiece, Move, Move32, Piece, PieceType, Square};
use serde_json::{Map, Value, json};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JkfError {
    #[error("failed to parse SFEN: {0}")]
    Sfen(#[from] SfenError),
    #[error("invalid JKF structure: {0}")]
    InvalidStructure(String),
    #[error("unsupported JKF preset: {0}")]
    UnsupportedPreset(String),
    #[error("invalid move: {0}")]
    InvalidMove(String),
    #[error("illegal move at index {index}")]
    IllegalMove { index: usize },
    #[error("record construction failed: {0}")]
    Record(#[from] RecordError),
    #[error("json serialization failed: {0}")]
    Json(#[from] serde_json::Error),
}

fn piece_type_to_csa(piece_type: PieceType) -> Option<&'static str> {
    match piece_type {
        PieceType::PAWN => Some("FU"),
        PieceType::LANCE => Some("KY"),
        PieceType::KNIGHT => Some("KE"),
        PieceType::SILVER => Some("GI"),
        PieceType::GOLD => Some("KI"),
        PieceType::BISHOP => Some("KA"),
        PieceType::ROOK => Some("HI"),
        PieceType::KING => Some("OU"),
        PieceType::PRO_PAWN => Some("TO"),
        PieceType::PRO_LANCE => Some("NY"),
        PieceType::PRO_KNIGHT => Some("NK"),
        PieceType::PRO_SILVER => Some("NG"),
        PieceType::HORSE => Some("UM"),
        PieceType::DRAGON => Some("RY"),
        _ => None,
    }
}

fn square_to_place(sq: Square) -> Value {
    let file = sq.file().raw() + 1;
    let rank = sq.rank().raw() + 1;
    json!({ "x": file, "y": rank })
}

fn time_ms_to_jkf(time_ms: u32) -> Value {
    let total_seconds = time_ms / 1_000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    let mut now = Map::new();
    if hours > 0 {
        now.insert("h".to_string(), json!(hours));
    }
    now.insert("m".to_string(), json!(minutes));
    now.insert("s".to_string(), json!(seconds));
    let mut time = Map::new();
    time.insert("now".to_string(), Value::Object(now));
    Value::Object(time)
}

fn time_ms_to_jkf_with_total(now_ms: u32, total_ms: u32) -> Value {
    let now = time_ms_to_jkf(now_ms);
    let total_seconds = total_ms / 1_000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    let mut total = Map::new();
    if hours > 0 {
        total.insert("h".to_string(), json!(hours));
    }
    total.insert("m".to_string(), json!(minutes));
    total.insert("s".to_string(), json!(seconds));
    let mut time = now.as_object().cloned().unwrap_or_else(Map::new);
    time.insert("total".to_string(), Value::Object(total));
    Value::Object(time)
}

fn format_kif_time_control_text(time_control: &TimeControl) -> Option<String> {
    let base = time_control.base_seconds();
    let byoyomi = time_control.byoyomi_seconds();
    let increment = time_control.increment_seconds();
    if base == 0 && byoyomi == 0 && increment == 0 {
        return None;
    }
    let mut parts = Vec::new();
    if base > 0 {
        let hours = base / 3600;
        let minutes = (base % 3600) / 60;
        if hours > 0 {
            parts.push(format!("{hours}時間{minutes}分"));
        } else {
            parts.push(format!("{minutes}分"));
        }
    }
    if byoyomi > 0 {
        parts.push(format!("秒読み{byoyomi}秒"));
    }
    if increment > 0 {
        parts.push(format!("加算{increment}秒"));
    }
    Some(parts.join(""))
}

fn build_move_entry(
    pos: &Position,
    mv_record: &MoveEntry,
    annotation: &RecordAnnotation,
    last_to: Option<Square>,
    total_ms: u32,
) -> Result<Value, JkfError> {
    let mv16 = mv_record.mv();
    let mv = pos.move32_from_move(mv16);
    let mut move_obj = Map::new();
    let color = if pos.turn() == Color::BLACK { 0 } else { 1 };
    let to_sq = mv.to_sq();
    move_obj.insert("to".to_string(), square_to_place(to_sq));
    move_obj.insert("color".to_string(), json!(color));

    if mv.is_drop() {
        let dropped = mv
            .dropped_piece()
            .ok_or_else(|| JkfError::InvalidMove("drop move missing piece".to_string()))?;
        let piece_code = piece_type_to_csa(dropped)
            .ok_or_else(|| JkfError::InvalidMove("invalid drop piece".to_string()))?;
        move_obj.insert("piece".to_string(), json!(piece_code));
        move_obj.insert("relative".to_string(), json!("H"));
    } else {
        let moved = pos.moved_piece_after(mv).piece_type();
        let piece_type = if mv.is_promotion() { moved.demote() } else { moved };
        let piece_code = piece_type_to_csa(piece_type)
            .ok_or_else(|| JkfError::InvalidMove("invalid piece".to_string()))?;
        move_obj.insert("piece".to_string(), json!(piece_code));
        move_obj.insert("from".to_string(), square_to_place(mv.from_sq()));
    }

    if mv.is_promotion() {
        move_obj.insert("promote".to_string(), json!(true));
    } else if should_output_promote_false(pos, mv) {
        move_obj.insert("promote".to_string(), json!(false));
    }
    if last_to == Some(to_sq) {
        move_obj.insert("same".to_string(), json!(true));
    }

    let captured = pos.piece_on(to_sq);
    if captured != Piece::NONE
        && captured.color() != pos.turn()
        && let Some(code) = piece_type_to_csa(captured.piece_type().demote())
    {
        move_obj.insert("capture".to_string(), json!(code));
    }

    if !mv.is_drop() {
        let ki2 =
            mv.to_ki2(pos).ok_or_else(|| JkfError::InvalidMove("invalid move".to_string()))?;
        let relative = relative_from_ki2(&ki2);
        if !relative.is_empty() {
            move_obj.insert("relative".to_string(), json!(relative));
        }
    }

    let mut entry = Map::new();
    entry.insert("move".to_string(), Value::Object(move_obj));
    if let Some(comment) = annotation.comment() {
        let comments: Vec<String> = comment
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        if !comments.is_empty() {
            entry.insert("comments".to_string(), json!(comments));
        }
    }
    if let Some(time_ms) = annotation.elapsed_ms() {
        entry.insert("time".to_string(), time_ms_to_jkf_with_total(time_ms, total_ms));
    } else {
        entry.insert("time".to_string(), time_ms_to_jkf_with_total(0, total_ms));
    }

    Ok(Value::Object(entry))
}

fn build_jkf_line(
    record: &Record,
    start: RecordNodeId,
    pos_start: &Position,
    last_to_start: Option<Square>,
    black_total: u32,
    white_total: u32,
) -> Result<Vec<Value>, JkfError> {
    let mut pos = pos_start.clone();
    let mut last_to = last_to_start;
    let mut refresh_counter = 0usize;
    let mut out = Vec::new();
    let mut black_total_ms = black_total;
    let mut white_total_ms = white_total;
    let mut current = Some(start);
    while let Some(node_id) = current {
        let node = record.node(node_id);
        if let Some(special) = node.special() {
            let mut entry = build_special_entry(special, node.annotation());
            if let Some(parent) = node.parent() {
                let siblings = record.children(parent);
                if siblings.first() == Some(&node_id) && siblings.len() > 1 {
                    let mut forks = Vec::new();
                    for sibling in siblings.iter().skip(1) {
                        let fork_moves = build_jkf_line(
                            record,
                            *sibling,
                            &pos,
                            last_to,
                            black_total_ms,
                            white_total_ms,
                        )?;
                        forks.push(fork_moves);
                    }
                    if let Some(obj) = entry.as_object_mut() {
                        obj.insert("forks".to_string(), json!(forks));
                    }
                }
            }
            out.push(entry);
            break;
        }
        let mv_record = node
            .mv()
            .ok_or_else(|| JkfError::InvalidMove("variation node missing move".to_string()))?;
        let mv16 = mv_record.mv();
        if !pos.is_legal_move(mv16) {
            return Err(JkfError::InvalidMove("illegal variation move".to_string()));
        }
        let pos_before = pos.clone();
        let last_to_before = last_to;
        let elapsed = node.time_ms().unwrap_or(0);
        if pos_before.turn() == Color::BLACK {
            black_total_ms = black_total_ms.saturating_add(elapsed);
        } else {
            white_total_ms = white_total_ms.saturating_add(elapsed);
        }
        let total = if pos_before.turn() == Color::BLACK { black_total_ms } else { white_total_ms };
        let mut entry =
            build_move_entry(&pos_before, mv_record, node.annotation(), last_to_before, total)?;
        if let Some(parent) = node.parent() {
            let siblings = record.children(parent);
            if siblings.first() == Some(&node_id) && siblings.len() > 1 {
                let mut forks = Vec::new();
                for sibling in siblings.iter().skip(1) {
                    let fork_moves = build_jkf_line(
                        record,
                        *sibling,
                        &pos_before,
                        last_to_before,
                        black_total_ms,
                        white_total_ms,
                    )?;
                    forks.push(fork_moves);
                }
                if let Some(obj) = entry.as_object_mut() {
                    obj.insert("forks".to_string(), json!(forks));
                }
            }
        }
        out.push(entry);
        pos.apply_move(mv16);
        last_to = Some(mv16.to_sq());
        refresh_position_if_needed(&mut pos, &mut refresh_counter)?;
        current = record.children(node_id).first().copied();
    }
    Ok(out)
}

fn special_to_jkf_special(special: &SpecialMoveEntry) -> Option<String> {
    let mapped = match special.kind() {
        SpecialMove::Interrupt => "CHUDAN",
        SpecialMove::Resign => "TORYO",
        SpecialMove::MaxMoves => "MAX_MOVES",
        SpecialMove::Impasse => "JISHOGI",
        SpecialMove::Draw => "HIKIWAKE",
        SpecialMove::RepetitionDraw => "SENNICHITE",
        SpecialMove::Mate => "TSUMI",
        SpecialMove::NoMate => "FUZUMI",
        SpecialMove::Timeout => "TIME_UP",
        SpecialMove::WinByIllegalMove | SpecialMove::LoseByIllegalMove => "ILLEGAL_MOVE",
        SpecialMove::WinByDeclaration => "KACHI",
        SpecialMove::WinByDefault | SpecialMove::LoseByDefault => "KACHI",
        SpecialMove::Try => "KACHI",
        SpecialMove::Unknown(name) => return Some(name.to_uppercase()),
    };
    Some(mapped.to_string())
}

fn result_to_jkf_special(result: GameResult) -> Option<&'static str> {
    match result {
        GameResult::BlackWin | GameResult::WhiteWin => Some("TORYO"),
        GameResult::DrawByRepetition => Some("SENNICHITE"),
        GameResult::DrawByImpasse => Some("JISHOGI"),
        GameResult::DrawByMaxPlies => Some("MAX_MOVES"),
        GameResult::BlackWinByDeclaration | GameResult::WhiteWinByDeclaration => Some("KACHI"),
        GameResult::BlackWinByTryRule | GameResult::WhiteWinByTryRule => Some("KACHI"),
        GameResult::BlackWinByForfeit | GameResult::WhiteWinByForfeit => Some("KACHI"),
        GameResult::BlackWinByIllegalMove | GameResult::WhiteWinByIllegalMove => {
            Some("ILLEGAL_MOVE")
        }
        GameResult::BlackWinByTimeout | GameResult::WhiteWinByTimeout => Some("TIME_UP"),
        GameResult::Error => Some("ERROR"),
        GameResult::Invalid | GameResult::Paused => Some("CHUDAN"),
    }
}

fn build_special_entry(special: &SpecialMoveEntry, annotation: &RecordAnnotation) -> Value {
    let name = special_to_jkf_special(special)
        .or_else(|| result_to_jkf_special(special.result()).map(str::to_string));
    let mut obj = Map::new();
    if let Some(name) = name {
        obj.insert("special".to_string(), json!(name));
    }
    if let Some(comment) = annotation.comment() {
        let comments: Vec<String> = comment
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect();
        if !comments.is_empty() {
            obj.insert("comments".to_string(), json!(comments));
        }
    }
    if let Some(time_ms) = annotation.elapsed_ms() {
        obj.insert("time".to_string(), time_ms_to_jkf(time_ms));
    }
    Value::Object(obj)
}

fn build_initial_data(pos: &Position) -> Result<Value, JkfError> {
    let mut board: Vec<Vec<Value>> = Vec::with_capacity(9);
    for file in 1..=9u8 {
        let mut column: Vec<Value> = Vec::with_capacity(9);
        for rank in 1..=9u8 {
            let file_char = char::from(b'0' + file);
            let rank_char = char::from(b'a' + (rank - 1));
            let sq = Square::from_usi(&format!("{file_char}{rank_char}"))
                .ok_or_else(|| JkfError::InvalidMove("invalid square".to_string()))?;
            let piece = pos.piece_on(sq);
            if piece == Piece::NONE {
                column.push(json!({}));
                continue;
            }
            let color = if piece.color() == Color::BLACK { 0 } else { 1 };
            let kind = piece_type_to_csa(piece.piece_type())
                .ok_or_else(|| JkfError::InvalidMove("invalid piece".to_string()))?;
            column.push(json!({ "color": color, "kind": kind }));
        }
        board.push(column);
    }

    let mut hands = Vec::new();
    for color in [Color::BLACK, Color::WHITE] {
        let hand = pos.hand(color);
        let mut map = Map::new();
        for (piece_type, code) in [
            (PieceType::PAWN, "FU"),
            (PieceType::LANCE, "KY"),
            (PieceType::KNIGHT, "KE"),
            (PieceType::SILVER, "GI"),
            (PieceType::GOLD, "KI"),
            (PieceType::BISHOP, "KA"),
            (PieceType::ROOK, "HI"),
        ] {
            let hand_piece = HandPiece::from_piece_type(piece_type)
                .ok_or_else(|| JkfError::InvalidMove("invalid hand piece".to_string()))?;
            let count = hand.count(hand_piece);
            map.insert(code.to_string(), json!(count));
        }
        hands.push(Value::Object(map));
    }

    let color = if pos.turn() == Color::BLACK { 0 } else { 1 };
    Ok(json!({ "board": board, "color": color, "hands": hands }))
}

fn preset_from_sfen(sfen: &str) -> Option<&'static str> {
    match sfen {
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1" => Some("HIRATE"),
        "lnsgkgsn1/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some("KY"),
        "1nsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some("KY_R"),
        "lnsgkgsnl/1r7/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some("KA"),
        "lnsgkgsnl/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some("HI"),
        "lnsgkgsn1/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some("HIKY"),
        "lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some("2"),
        "lnsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some("3"),
        "1nsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some("4"),
        "2sgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some("5"),
        "1nsgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some("5_L"),
        "2sgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some("6"),
        "3gkg3/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some("8"),
        "4k4/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some("10"),
        _ => None,
    }
}

fn sfen_from_preset(preset: &str) -> Option<&'static str> {
    match preset {
        "HIRATE" => Some("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"),
        "KY" => Some("lnsgkgsn1/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"),
        "KY_R" => Some("1nsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"),
        "KA" => Some("lnsgkgsnl/1r7/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"),
        "HI" => Some("lnsgkgsnl/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"),
        "HIKY" => Some("lnsgkgsn1/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"),
        "2" => Some("lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"),
        "3" => Some("lnsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"),
        "4" => Some("1nsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"),
        "5" => Some("2sgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"),
        "5_L" => Some("1nsgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"),
        "6" => Some("2sgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"),
        "8" => Some("3gkg3/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"),
        "10" => Some("4k4/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"),
        _ => None,
    }
}

fn csa_to_piece_type(kind: &str) -> Option<PieceType> {
    match kind {
        "FU" => Some(PieceType::PAWN),
        "KY" => Some(PieceType::LANCE),
        "KE" => Some(PieceType::KNIGHT),
        "GI" => Some(PieceType::SILVER),
        "KI" => Some(PieceType::GOLD),
        "KA" => Some(PieceType::BISHOP),
        "HI" => Some(PieceType::ROOK),
        "OU" | "GY" => Some(PieceType::KING),
        "TO" => Some(PieceType::PRO_PAWN),
        "NY" => Some(PieceType::PRO_LANCE),
        "NK" => Some(PieceType::PRO_KNIGHT),
        "NG" => Some(PieceType::PRO_SILVER),
        "UM" => Some(PieceType::HORSE),
        "RY" => Some(PieceType::DRAGON),
        _ => None,
    }
}

fn piece_type_to_sfen(piece_type: PieceType, color: Color) -> Option<String> {
    let base = match piece_type.demote() {
        PieceType::PAWN => 'P',
        PieceType::LANCE => 'L',
        PieceType::KNIGHT => 'N',
        PieceType::SILVER => 'S',
        PieceType::GOLD => 'G',
        PieceType::BISHOP => 'B',
        PieceType::ROOK => 'R',
        PieceType::KING => 'K',
        _ => return None,
    };
    let piece = if color == Color::BLACK { base } else { base.to_ascii_lowercase() };
    if piece_type.is_promoted() { Some(format!("+{piece}")) } else { Some(piece.to_string()) }
}

fn csa_to_drop_char(kind: &str) -> Option<char> {
    match csa_to_piece_type(kind)?.demote() {
        PieceType::PAWN => Some('P'),
        PieceType::LANCE => Some('L'),
        PieceType::KNIGHT => Some('N'),
        PieceType::SILVER => Some('S'),
        PieceType::GOLD => Some('G'),
        PieceType::BISHOP => Some('B'),
        PieceType::ROOK => Some('R'),
        _ => None,
    }
}

fn jkf_initial_position(initial: Option<&Value>) -> Result<RecordInitialPosition, JkfError> {
    let Some(initial) = initial else {
        return Ok(RecordInitialPosition::Startpos);
    };
    let obj = initial
        .as_object()
        .ok_or_else(|| JkfError::InvalidStructure("initial must be an object".to_string()))?;
    if let Some(preset) = obj.get("preset").and_then(Value::as_str) {
        if preset == "HIRATE" {
            return Ok(RecordInitialPosition::Startpos);
        }
        if let Some(sfen) = sfen_from_preset(preset) {
            return Ok(RecordInitialPosition::Sfen(sfen.to_string()));
        }
        if preset != "OTHER" {
            return Err(JkfError::UnsupportedPreset(preset.to_string()));
        }
    }
    if let Some(data) = obj.get("data") {
        return Ok(RecordInitialPosition::Sfen(jkf_initial_data_to_sfen(data)?));
    }
    Ok(RecordInitialPosition::Startpos)
}

fn jkf_initial_data_to_sfen(data: &Value) -> Result<String, JkfError> {
    let obj = data
        .as_object()
        .ok_or_else(|| JkfError::InvalidStructure("initial.data must be an object".to_string()))?;
    let board = obj.get("board").and_then(Value::as_array).ok_or_else(|| {
        JkfError::InvalidStructure("initial.data.board must be an array".to_string())
    })?;
    if board.len() != 9 {
        return Err(JkfError::InvalidStructure("initial.data.board must have 9 files".to_string()));
    }

    let mut board_sfen = String::new();
    for rank in 0..9 {
        if rank > 0 {
            board_sfen.push('/');
        }
        let mut empty = 0u8;
        for file in (0..9).rev() {
            let column = board[file].as_array().ok_or_else(|| {
                JkfError::InvalidStructure("initial.data.board file must be an array".to_string())
            })?;
            if column.len() != 9 {
                return Err(JkfError::InvalidStructure(
                    "initial.data.board file must have 9 ranks".to_string(),
                ));
            }
            let Some(cell_obj) = column[rank].as_object() else {
                empty += 1;
                continue;
            };
            let Some(kind) = cell_obj.get("kind").and_then(Value::as_str) else {
                empty += 1;
                continue;
            };
            if empty > 0 {
                board_sfen.push(char::from(b'0' + empty));
                empty = 0;
            }
            let color = parse_jkf_color(cell_obj.get("color"))?;
            let piece_type = csa_to_piece_type(kind)
                .ok_or_else(|| JkfError::InvalidStructure(format!("invalid piece kind: {kind}")))?;
            let piece = piece_type_to_sfen(piece_type, color).ok_or_else(|| {
                JkfError::InvalidStructure(format!("invalid piece kind for SFEN: {kind}"))
            })?;
            board_sfen.push_str(&piece);
        }
        if empty > 0 {
            board_sfen.push(char::from(b'0' + empty));
        }
    }

    let color = match obj.get("color").and_then(Value::as_u64).unwrap_or(0) {
        0 => "b",
        1 => "w",
        other => {
            return Err(JkfError::InvalidStructure(format!("invalid initial.data.color: {other}")));
        }
    };
    let hands = jkf_hands_to_sfen(obj.get("hands"))?;
    Ok(format!("{board_sfen} {color} {hands} 1"))
}

fn parse_jkf_color(value: Option<&Value>) -> Result<Color, JkfError> {
    match value.and_then(Value::as_u64).unwrap_or(0) {
        0 => Ok(Color::BLACK),
        1 => Ok(Color::WHITE),
        other => Err(JkfError::InvalidStructure(format!("invalid color: {other}"))),
    }
}

fn jkf_hands_to_sfen(value: Option<&Value>) -> Result<String, JkfError> {
    let Some(hands) = value.and_then(Value::as_array) else {
        return Ok("-".to_string());
    };
    let mut out = String::new();
    for (index, color) in [(0usize, Color::BLACK), (1usize, Color::WHITE)] {
        let Some(hand) = hands.get(index).and_then(Value::as_object) else {
            continue;
        };
        for (kind, piece) in [
            ("HI", 'R'),
            ("KA", 'B'),
            ("KI", 'G'),
            ("GI", 'S'),
            ("KE", 'N'),
            ("KY", 'L'),
            ("FU", 'P'),
        ] {
            let count = hand.get(kind).and_then(Value::as_u64).unwrap_or(0);
            if count == 0 {
                continue;
            }
            if count > 1 {
                out.push_str(&count.to_string());
            }
            out.push(if color == Color::BLACK { piece } else { piece.to_ascii_lowercase() });
        }
    }
    if out.is_empty() { Ok("-".to_string()) } else { Ok(out) }
}

fn comments_from_entry(obj: &Map<String, Value>) -> Result<Option<String>, JkfError> {
    let Some(comments) = obj.get("comments") else {
        return Ok(None);
    };
    let array = comments
        .as_array()
        .ok_or_else(|| JkfError::InvalidStructure("comments must be an array".to_string()))?;
    let lines: Result<Vec<String>, JkfError> = array
        .iter()
        .map(|value| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                JkfError::InvalidStructure("comments item must be a string".to_string())
            })
        })
        .collect();
    let lines: Vec<String> = lines?.into_iter().filter(|line| !line.is_empty()).collect();
    if lines.is_empty() { Ok(None) } else { Ok(Some(lines.join("\n"))) }
}

fn time_ms_from_entry(obj: &Map<String, Value>) -> Result<Option<u32>, JkfError> {
    let Some(time) = obj.get("time") else {
        return Ok(None);
    };
    let time = time
        .as_object()
        .ok_or_else(|| JkfError::InvalidStructure("time must be an object".to_string()))?;
    let Some(now) = time.get("now") else {
        return Ok(None);
    };
    let now = now
        .as_object()
        .ok_or_else(|| JkfError::InvalidStructure("time.now must be an object".to_string()))?;
    let hours = now.get("h").and_then(Value::as_u64).unwrap_or(0);
    let minutes = now.get("m").and_then(Value::as_u64).unwrap_or(0);
    let seconds = now.get("s").and_then(Value::as_u64).unwrap_or(0);
    let total_ms = hours
        .saturating_mul(3_600_000)
        .saturating_add(minutes.saturating_mul(60_000))
        .saturating_add(seconds.saturating_mul(1_000));
    u32::try_from(total_ms)
        .map(Some)
        .map_err(|_| JkfError::InvalidStructure("time.now exceeds u32 milliseconds".to_string()))
}

fn place_from_value(value: &Value) -> Result<Square, JkfError> {
    let obj = value
        .as_object()
        .ok_or_else(|| JkfError::InvalidStructure("place must be an object".to_string()))?;
    let x = obj
        .get("x")
        .and_then(Value::as_u64)
        .ok_or_else(|| JkfError::InvalidStructure("place.x must be an integer".to_string()))?;
    let y = obj
        .get("y")
        .and_then(Value::as_u64)
        .ok_or_else(|| JkfError::InvalidStructure("place.y must be an integer".to_string()))?;
    if !(1..=9).contains(&x) || !(1..=9).contains(&y) {
        return Err(JkfError::InvalidStructure(format!("place out of board: {x},{y}")));
    }
    let rank = char::from(b'a' + u8::try_from(y - 1).expect("rank in range"));
    Square::from_usi(&format!("{x}{rank}"))
        .ok_or_else(|| JkfError::InvalidStructure(format!("invalid square: {x},{y}")))
}

fn move_from_jkf_move(obj: &Map<String, Value>, last_to: Option<Square>) -> Result<Move, JkfError> {
    let to = if let Some(to) = obj.get("to") {
        place_from_value(to)?
    } else if obj.get("same").and_then(Value::as_bool).unwrap_or(false) {
        last_to
            .ok_or_else(|| JkfError::InvalidMove("same move without previous square".to_string()))?
    } else {
        return Err(JkfError::InvalidStructure("move.to is required".to_string()));
    };

    let usi = if let Some(from) = obj.get("from") {
        let from = place_from_value(from)?;
        let suffix =
            if obj.get("promote").and_then(Value::as_bool).unwrap_or(false) { "+" } else { "" };
        format!("{from}{to}{suffix}")
    } else {
        let kind = obj
            .get("piece")
            .and_then(Value::as_str)
            .ok_or_else(|| JkfError::InvalidStructure("drop move requires piece".to_string()))?;
        let piece = csa_to_drop_char(kind)
            .ok_or_else(|| JkfError::InvalidMove(format!("invalid drop piece: {kind}")))?;
        format!("{piece}*{to}")
    };

    Move::from_usi(&usi).ok_or_else(|| JkfError::InvalidMove(format!("invalid USI move: {usi}")))
}

fn special_from_jkf_name(name: &str, side_to_move: Color) -> SpecialMoveEntry {
    let upper = name.to_ascii_uppercase();
    let winner = side_to_move.flip();
    let (kind, result) = match upper.as_str() {
        "CHUDAN" => (SpecialMove::Interrupt, GameResult::Paused),
        "TORYO" => (SpecialMove::Resign, GameResult::win_from_color(winner)),
        "MAX_MOVES" => (SpecialMove::MaxMoves, GameResult::DrawByMaxPlies),
        "JISHOGI" => (SpecialMove::Impasse, GameResult::DrawByImpasse),
        "HIKIWAKE" => (SpecialMove::Draw, GameResult::DrawByRepetition),
        "SENNICHITE" => (SpecialMove::RepetitionDraw, GameResult::DrawByRepetition),
        "TSUMI" => (SpecialMove::Mate, GameResult::win_from_color(winner)),
        "FUZUMI" => (SpecialMove::NoMate, GameResult::DrawByMaxPlies),
        "TIME_UP" => (SpecialMove::Timeout, GameResult::win_by_timeout_from_color(winner)),
        "ILLEGAL_MOVE" => {
            (SpecialMove::WinByIllegalMove, GameResult::win_by_illegal_move_from_color(winner))
        }
        "KACHI" => {
            (SpecialMove::WinByDeclaration, GameResult::win_by_declaration_from_color(side_to_move))
        }
        "ERROR" => (SpecialMove::Interrupt, GameResult::Error),
        _ => (SpecialMove::Unknown(name.to_string()), GameResult::Invalid),
    };
    SpecialMoveEntry::new(kind, result).with_raw(Some(name.to_string()))
}

fn parse_jkf_forks(
    record: &mut Record,
    parent: RecordNodeId,
    pos: &Position,
    last_to: Option<Square>,
    forks: &Value,
) -> Result<(), JkfError> {
    let forks = forks
        .as_array()
        .ok_or_else(|| JkfError::InvalidStructure("forks must be an array".to_string()))?;
    for fork in forks {
        let line = fork
            .as_array()
            .ok_or_else(|| JkfError::InvalidStructure("fork must be an array".to_string()))?;
        parse_jkf_line(record, parent, pos, last_to, line)?;
    }
    Ok(())
}

fn merge_node_comment(
    record: &mut Record,
    node_id: RecordNodeId,
    comment: String,
) -> Result<(), JkfError> {
    let node = record.node_mut(node_id)?;
    let merged = if let Some(existing) = node.annotation().comment() {
        format!("{existing}\n{comment}")
    } else {
        comment
    };
    node.annotation_mut().set_comment(Some(merged));
    Ok(())
}

fn merge_initial_comment(record: &mut Record, comment: String) {
    let merged = if let Some(existing) = record.initial_comment() {
        format!("{existing}\n{comment}")
    } else {
        comment
    };
    record.set_initial_comment(Some(merged));
}

fn parse_jkf_line(
    record: &mut Record,
    parent: RecordNodeId,
    pos_start: &Position,
    last_to_start: Option<Square>,
    line: &[Value],
) -> Result<(), JkfError> {
    let mut current = parent;
    let mut pos = pos_start.clone();
    let mut last_to = last_to_start;

    for (index, entry) in line.iter().enumerate() {
        let obj = entry.as_object().ok_or_else(|| {
            JkfError::InvalidStructure("move entry must be an object".to_string())
        })?;
        let comment = comments_from_entry(obj)?;
        let time_ms = time_ms_from_entry(obj)?;
        let annotation =
            RecordAnnotation::new().with_comment(comment.clone()).with_elapsed_ms(time_ms);

        if let Some(move_value) = obj.get("move") {
            let move_obj = move_value
                .as_object()
                .ok_or_else(|| JkfError::InvalidStructure("move must be an object".to_string()))?;
            let pos_before = pos.clone();
            let last_to_before = last_to;
            let parent_before = current;
            let mv = move_from_jkf_move(move_obj, last_to_before)?;
            let mv32 = pos.move32_from_move(mv);
            if !mv32.is_normal() || !pos.is_legal_move32(mv32) {
                return Err(JkfError::IllegalMove { index });
            }
            let child = record.append_move_with_annotation(
                parent_before,
                MoveEntry::new(mv),
                annotation,
            )?;
            if let Some(forks) = obj.get("forks") {
                parse_jkf_forks(record, parent_before, &pos_before, last_to_before, forks)?;
            }
            pos.apply_move32(mv32);
            last_to = Some(mv.to_sq());
            current = child;
            continue;
        }

        if let Some(special_value) = obj.get("special") {
            let name = special_value.as_str().ok_or_else(|| {
                JkfError::InvalidStructure("special must be a string".to_string())
            })?;
            let parent_before = current;
            let special = special_from_jkf_name(name, pos.turn());
            record.append_special_with_annotation(parent_before, special, annotation)?;
            if let Some(forks) = obj.get("forks") {
                parse_jkf_forks(record, parent_before, &pos, last_to, forks)?;
            }
            break;
        }

        if let Some(comment) = comment {
            if current == record.root_id() {
                merge_initial_comment(record, comment);
            } else {
                merge_node_comment(record, current, comment)?;
            }
        }
        if let Some(forks) = obj.get("forks") {
            parse_jkf_forks(record, current, &pos, last_to, forks)?;
        }
    }
    Ok(())
}

fn metadata_from_jkf_header(header: &Value) -> Result<RecordMetadata, JkfError> {
    let obj = header
        .as_object()
        .ok_or_else(|| JkfError::InvalidStructure("header must be an object".to_string()))?;
    let mut builder = RecordMetadata::builder();
    for (key, value) in obj {
        match key.as_str() {
            "先手" => builder.black_player(value.as_str().map(ToOwned::to_owned)),
            "後手" => builder.white_player(value.as_str().map(ToOwned::to_owned)),
            "棋戦" => builder.event(value.as_str().map(ToOwned::to_owned)),
            "場所" => builder.site(value.as_str().map(ToOwned::to_owned)),
            "開始日時" => builder.start_date(value.as_str().map(ToOwned::to_owned)),
            "終了日時" => builder.end_date(value.as_str().map(ToOwned::to_owned)),
            "備考" => builder.comment(value.as_str().map(ToOwned::to_owned)),
            "持将棋" => builder.impasse_rule(value.as_str().map(ToOwned::to_owned)),
            "最大手数" => {
                let max_moves = value
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .or_else(|| value.as_str().and_then(|s| s.parse::<u32>().ok()));
                builder.max_moves(max_moves)
            }
            "持ち時間" => builder.time_control(value.as_str().and_then(parse_kif_time_control)),
            "先手持ち時間" => {
                builder.black_time_control(value.as_str().and_then(TimeControl::from_spec))
            }
            "後手持ち時間" => {
                builder.white_time_control(value.as_str().and_then(TimeControl::from_spec))
            }
            _ => {
                if let Some(text) = header_value_to_attribute(value) {
                    builder.add_attribute(key.clone(), text);
                }
                &mut builder
            }
        };
    }
    Ok(builder.build())
}

fn header_value_to_attribute(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        other => Some(other.to_string()),
    }
}

pub fn parse_jkf_str(text: &str) -> Result<Record, JkfError> {
    let value: Value = serde_json::from_str(text)?;
    let root = value
        .as_object()
        .ok_or_else(|| JkfError::InvalidStructure("JKF root must be an object".to_string()))?;
    let initial = jkf_initial_position(root.get("initial"))?;
    let mut record = Record::new(initial)?;
    if let Some(header) = root.get("header") {
        record.set_metadata(metadata_from_jkf_header(header)?);
    }

    let mut pos = Position::empty();
    pos.set_sfen(record.init_position_sfen())?;
    if let Some(moves) = root.get("moves") {
        let moves = moves
            .as_array()
            .ok_or_else(|| JkfError::InvalidStructure("moves must be an array".to_string()))?;
        let root_id = record.root_id();
        parse_jkf_line(&mut record, root_id, &pos, None, moves)?;
    }
    Ok(record)
}

pub fn parse_jkf_bytes(bytes: &[u8]) -> Result<Record, JkfError> {
    let text = std::str::from_utf8(bytes)
        .map_err(|err| JkfError::InvalidStructure(format!("JKF must be UTF-8: {err}")))?;
    parse_jkf_str(text)
}

fn is_promotable_piece_type(piece_type: PieceType) -> bool {
    matches!(
        piece_type,
        PieceType::PAWN
            | PieceType::LANCE
            | PieceType::KNIGHT
            | PieceType::SILVER
            | PieceType::BISHOP
            | PieceType::ROOK
    )
}

fn is_promotable_rank(color: Color, rank: u8) -> bool {
    if color == Color::BLACK { rank <= 3 } else { rank >= 7 }
}

fn should_output_promote_false(pos: &Position, mv: Move32) -> bool {
    if mv.is_drop() {
        return false;
    }
    let moved = pos.moved_piece_after(mv).piece_type();
    let piece_type = moved;
    if !is_promotable_piece_type(piece_type) {
        return false;
    }
    let from = mv.from_sq();
    let to = mv.to_sq();
    let from_rank = u8::try_from(from.rank().raw() + 1).unwrap_or(0);
    let to_rank = u8::try_from(to.rank().raw() + 1).unwrap_or(0);
    is_promotable_rank(pos.turn(), from_rank) || is_promotable_rank(pos.turn(), to_rank)
}

fn relative_from_ki2(ki2: &str) -> String {
    let mut out = String::new();
    for ch in ki2.chars() {
        match ch {
            '左' => out.push('L'),
            '直' => out.push('C'),
            '右' => out.push('R'),
            '上' => out.push('U'),
            '寄' => out.push('M'),
            '引' => out.push('D'),
            '打' => out.push('H'),
            _ => {}
        }
    }
    out
}

/// [`Record`] を JKF（JSON Kifu Format）形式の文字列に変換する。
pub fn export_jkf(record: &Record) -> Result<String, JkfError> {
    let mut pos = Position::empty();
    pos.set_sfen(record.init_position_sfen())?;

    let mut header = Map::new();
    if let Some(black) = record.metadata().black_player() {
        header.insert("先手".to_string(), json!(black));
    }
    if let Some(white) = record.metadata().white_player() {
        header.insert("後手".to_string(), json!(white));
    }
    if let Some(event) = record.metadata().event() {
        header.insert("棋戦".to_string(), json!(event));
    }
    if let Some(site) = record.metadata().site() {
        header.insert("場所".to_string(), json!(site));
    }
    if let Some(start) = record.metadata().start_date() {
        header.insert("開始日時".to_string(), json!(start));
    }
    if let Some(end) = record.metadata().end_date() {
        header.insert("終了日時".to_string(), json!(end));
    }
    if let Some(tc) = record.metadata().time_control()
        && let Some(text) = format_kif_time_control_text(tc)
    {
        header.insert("持ち時間".to_string(), json!(text));
    }
    if let Some(tc) = record.metadata().black_time_control() {
        header.insert("先手持ち時間".to_string(), json!(tc.to_spec()));
    }
    if let Some(tc) = record.metadata().white_time_control() {
        header.insert("後手持ち時間".to_string(), json!(tc.to_spec()));
    }
    if let Some(max_moves) = record.metadata().max_moves() {
        header.insert("最大手数".to_string(), json!(max_moves));
    }
    if let Some(rule) = record.metadata().impasse_rule() {
        header.insert("持将棋".to_string(), json!(rule));
    }
    if let Some(comment) = record.metadata().comment() {
        header.insert("備考".to_string(), json!(comment));
    }
    if !record.metadata().attributes().is_empty() {
        let mut keys: Vec<&String> = record.metadata().attributes().keys().collect();
        keys.sort();
        for key in keys {
            if header.contains_key(key) {
                continue;
            }
            if let Some(value) = record.metadata().attributes().get(key) {
                header.insert(key.clone(), json!(value));
            }
        }
    }

    let initial = if let Some(preset) = preset_from_sfen(record.init_position_sfen()) {
        json!({ "preset": preset })
    } else {
        let data = build_initial_data(&pos)?;
        json!({ "preset": "OTHER", "data": data })
    };

    let mut moves: Vec<Value> = Vec::new();
    let mut initial_comments: Vec<String> = Vec::new();
    if let Some(comment) = record.initial_comment() {
        initial_comments.extend(
            comment.lines().map(|line| line.trim().to_string()).filter(|line| !line.is_empty()),
        );
    }
    if !initial_comments.is_empty() {
        moves.push(json!({ "comments": initial_comments }));
    } else {
        moves.push(json!({}));
    }

    let main_ids = record.main_line_ids();
    let mut last_to: Option<Square> = None;
    let mut refresh_counter = 0usize;
    let mut black_total_ms: u32 = 0;
    let mut white_total_ms: u32 = 0;
    for (index, node_id) in main_ids.iter().enumerate() {
        let node = record.node(*node_id);
        let mv_record = node
            .mv()
            .ok_or_else(|| JkfError::InvalidMove("main line node missing move".to_string()))?;
        let mv16 = mv_record.mv();
        if !pos.is_legal_move(mv16) {
            return Err(JkfError::IllegalMove { index });
        }
        let pos_before = pos.clone();
        let last_to_before = last_to;
        let elapsed = node.time_ms().unwrap_or(0);
        if pos_before.turn() == Color::BLACK {
            black_total_ms = black_total_ms.saturating_add(elapsed);
        } else {
            white_total_ms = white_total_ms.saturating_add(elapsed);
        }
        let total = if pos_before.turn() == Color::BLACK { black_total_ms } else { white_total_ms };
        let mut entry =
            build_move_entry(&pos_before, mv_record, node.annotation(), last_to_before, total)?;
        if let Some(parent) = node.parent() {
            let siblings = record.children(parent);
            if siblings.first() == Some(node_id) && siblings.len() > 1 {
                let mut forks = Vec::new();
                for sibling in siblings.iter().skip(1) {
                    let fork_moves = build_jkf_line(
                        record,
                        *sibling,
                        &pos_before,
                        last_to_before,
                        black_total_ms,
                        white_total_ms,
                    )?;
                    forks.push(fork_moves);
                }
                if let Some(obj) = entry.as_object_mut() {
                    obj.insert("forks".to_string(), json!(forks));
                }
            }
        }
        moves.push(entry);
        pos.apply_move(mv16);
        last_to = Some(mv16.to_sq());
        refresh_position_if_needed(&mut pos, &mut refresh_counter)?;
    }

    if let Some(terminal_id) = record.main_terminal_node() {
        let terminal_node = record.node(terminal_id);
        let terminal = terminal_node.special().expect("terminal node has special entry");
        let mut entry = build_special_entry(terminal, terminal_node.annotation());
        if let Some(parent) = terminal_node.parent() {
            let siblings = record.children(parent);
            if siblings.first() == Some(&terminal_id) && siblings.len() > 1 {
                let mut forks = Vec::new();
                for sibling in siblings.iter().skip(1) {
                    let fork_moves = build_jkf_line(
                        record,
                        *sibling,
                        &pos,
                        last_to,
                        black_total_ms,
                        white_total_ms,
                    )?;
                    forks.push(fork_moves);
                }
                if let Some(obj) = entry.as_object_mut() {
                    obj.insert("forks".to_string(), json!(forks));
                }
            }
        }
        moves.push(entry);
    }

    let mut root = Map::new();
    if !header.is_empty() {
        root.insert("header".to_string(), Value::Object(header));
    }
    root.insert("initial".to_string(), initial);
    root.insert("moves".to_string(), Value::Array(moves));

    Ok(serde_json::to_string(&Value::Object(root))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::hirate_position;
    use crate::records::record::{
        EngineInfo, MoveEntry, RecordAnnotation, RecordInitialPosition, RecordNode,
    };
    use crate::types::{Eval, GameResult, Move};

    fn main_node(record: &Record, index: usize) -> &RecordNode {
        let node_id = record.main_line_ids()[index];
        record.node(node_id)
    }

    fn annotation_with_eval(eval: i32) -> RecordAnnotation {
        RecordAnnotation::new()
            .with_engine_info(Some(EngineInfo::new().with_eval(Some(Eval::from_i32(eval)))))
    }

    #[test]
    fn test_export_jkf_basic() {
        let pos = hirate_position();
        let mv16 = Move::from_usi("7g7f").unwrap();
        let mut record = Record::new(pos.to_sfen(None)).unwrap();
        record
            .append_move_with_annotation(
                record.root_id(),
                MoveEntry::new(mv16),
                annotation_with_eval(0),
            )
            .unwrap();
        record
            .set_main_terminal(SpecialMoveEntry::new(SpecialMove::Resign, GameResult::BlackWin))
            .unwrap();

        let jkf = export_jkf(&record).unwrap();
        let value: Value = serde_json::from_str(&jkf).unwrap();
        let moves = value["moves"].as_array().unwrap();
        assert_eq!(moves.len(), 3);
        assert_eq!(moves[1]["move"]["to"]["x"], 7);
        assert_eq!(moves[1]["move"]["to"]["y"], 6);
        assert_eq!(moves[2]["special"], "TORYO");
    }

    #[test]
    fn test_export_jkf_uses_three_piece_preset() {
        let mut pos = Position::empty();
        pos.set_sfen("lnsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1")
            .expect("valid handicap sfen");
        let mv16 = Move::from_usi("3c3d").expect("valid move");
        let record = Record::from_main_line_with_result(
            pos.to_sfen(None),
            vec![MoveEntry::new(mv16)],
            GameResult::WhiteWin,
        )
        .expect("record");

        let jkf = export_jkf(&record).expect("export");
        let value: Value = serde_json::from_str(&jkf).expect("json");
        assert_eq!(value["initial"]["preset"], "3");
    }

    #[test]
    fn test_export_jkf_writes_initial_comment_to_root_comments() {
        let pos = hirate_position();
        let mut record = Record::from_main_line(
            pos.to_sfen(None),
            vec![MoveEntry::new(Move::from_usi("7g7f").unwrap())],
            None,
        )
        .expect("record");
        record.set_initial_comment(Some("序文1\n序文2".to_string()));

        let jkf = export_jkf(&record).expect("export");
        let value: Value = serde_json::from_str(&jkf).expect("json");
        let moves = value["moves"].as_array().expect("moves");
        let comments = moves[0]["comments"].as_array().expect("comments");
        assert_eq!(comments[0], "序文1");
        assert_eq!(comments[1], "序文2");
    }

    #[test]
    fn test_export_jkf_writes_metadata_comment_to_header_note() {
        let pos = hirate_position();
        let mut record = Record::from_main_line(
            pos.to_sfen(None),
            vec![MoveEntry::new(Move::from_usi("7g7f").unwrap())],
            None,
        )
        .expect("record");
        let mut metadata_builder = crate::records::record::RecordMetadata::builder();
        metadata_builder.comment(Some("備考1\n備考2".to_string()));
        let metadata = metadata_builder.build();
        record.set_metadata(metadata);
        record.set_initial_comment(Some("序文".to_string()));

        let jkf = export_jkf(&record).expect("export");
        let value: Value = serde_json::from_str(&jkf).expect("json");
        assert_eq!(value["header"]["備考"], "備考1\n備考2");
        let moves = value["moves"].as_array().expect("moves");
        let comments = moves[0]["comments"].as_array().expect("comments");
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0], "序文");
    }

    #[test]
    fn test_parse_jkf_roundtrips_main_variation_comments_time_and_terminal() {
        let mut record = Record::new(RecordInitialPosition::Startpos).expect("record");
        record.set_initial_comment(Some("root comment".to_string()));
        let first = record
            .append_move_with_annotation(
                record.root_id(),
                MoveEntry::new(Move::from_usi("7g7f").unwrap()),
                RecordAnnotation::new()
                    .with_comment(Some("main comment".to_string()))
                    .with_elapsed_ms(Some(3_000)),
            )
            .expect("append main");
        record
            .append_move(first, MoveEntry::new(Move::from_usi("3c3d").unwrap()))
            .expect("append second");
        record
            .append_move_with_annotation(
                record.root_id(),
                MoveEntry::new(Move::from_usi("2g2f").unwrap()),
                RecordAnnotation::new().with_comment(Some("fork comment".to_string())),
            )
            .expect("append fork");
        record
            .set_main_terminal(SpecialMoveEntry::new(SpecialMove::Resign, GameResult::WhiteWin))
            .expect("terminal");

        let jkf = export_jkf(&record).expect("export");
        let parsed = parse_jkf_str(&jkf).expect("parse");

        assert!(parsed.initial_position().is_startpos());
        assert_eq!(parsed.initial_comment(), Some("root comment"));
        assert_eq!(
            parsed.moves().iter().map(|mv| mv.mv().to_usi()).collect::<Vec<_>>(),
            ["7g7f", "3c3d"]
        );
        assert_eq!(main_node(&parsed, 0).comment(), Some("main comment"));
        assert_eq!(main_node(&parsed, 0).time_ms(), Some(3_000));
        assert_eq!(parsed.result(), GameResult::WhiteWin);
        assert_eq!(parsed.children(parsed.root_id()).len(), 2);
        let fork = parsed.children(parsed.root_id())[1];
        assert_eq!(parsed.node(fork).mv().unwrap().mv().to_usi(), "2g2f");
        assert_eq!(parsed.node(fork).comment(), Some("fork comment"));
    }

    #[test]
    fn test_export_jkf_roundtrips_terminal_sibling_fork() {
        let mut record = Record::new(RecordInitialPosition::Startpos).expect("record");
        let first = record
            .append_move(record.root_id(), MoveEntry::new(Move::from_usi("7g7f").unwrap()))
            .expect("append first");
        let second = record
            .append_move(first, MoveEntry::new(Move::from_usi("3c3d").unwrap()))
            .expect("append second");
        record
            .set_main_terminal(SpecialMoveEntry::new(SpecialMove::Resign, GameResult::BlackWin))
            .expect("terminal");
        record
            .append_move(second, MoveEntry::new(Move::from_usi("2g2f").unwrap()))
            .expect("terminal sibling fork");

        let jkf = export_jkf(&record).expect("export");
        let parsed = parse_jkf_str(&jkf).expect("parse");
        let parsed_second = parsed.main_line_ids()[1];
        let children = parsed.children(parsed_second);

        assert_eq!(children.len(), 2);
        assert_eq!(parsed.node(children[0]).special().unwrap().kind(), &SpecialMove::Resign);
        assert_eq!(parsed.node(children[1]).mv().unwrap().mv().to_usi(), "2g2f");
    }

    #[test]
    fn test_parse_jkf_header_fields() {
        let text = r#"{
            "header": {
                "先手": "black",
                "後手": "white",
                "棋戦": "event",
                "最大手数": 256,
                "extra": 42
            },
            "initial": { "preset": "HIRATE" },
            "moves": [ {} ]
        }"#;

        let parsed = parse_jkf_str(text).expect("parse");

        assert_eq!(parsed.metadata().black_player(), Some("black"));
        assert_eq!(parsed.metadata().white_player(), Some("white"));
        assert_eq!(parsed.metadata().event(), Some("event"));
        assert_eq!(parsed.metadata().max_moves(), Some(256));
        assert_eq!(parsed.metadata().attributes().get("extra").map(String::as_str), Some("42"));
    }

    #[test]
    fn test_parse_jkf_other_initial_data() {
        let sfen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1";
        let record = Record::new(sfen.to_string()).expect("record");
        let jkf = export_jkf(&record).expect("export");
        let parsed = parse_jkf_str(&jkf).expect("parse");

        assert_eq!(parsed.init_position_sfen(), sfen);
    }
}
