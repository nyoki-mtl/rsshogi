mod bytes;
mod error;

pub use bytes::{
    export_ki2_bytes, export_ki2_bytes_with_options, export_kif_bytes,
    export_kif_bytes_with_options, parse_ki2_bytes, parse_kif_bytes,
};
pub use error::{Ki2Error, KifError};

use crate::board::{InitialPosition, Position};
use crate::records::formats::common::{
    BoardMap, HandCounts, board_map_to_sfen, ensure_hand_sides, hand_counts_to_sfen,
    refresh_position_if_needed,
};
use crate::records::record::{
    AnnotatedMoveEntry, Record, RecordAnnotation, RecordMetadata, RecordMetadataBuilder,
    SpecialMove, SpecialMoveEntry,
};
use crate::records::time_control::{TimeControl, parse_kif_time_control};
use crate::types::{
    Color, Eval, GameResult, Hand, HandPiece, Move, Move32, Piece, PieceType, Square,
};
use std::collections::HashMap;
use std::fmt::Write;

const KANJI_RANKS: [char; 9] = ['一', '二', '三', '四', '五', '六', '七', '八', '九'];
const WIDE_DIGITS: [char; 9] = ['１', '２', '３', '４', '５', '６', '７', '８', '９'];

fn normalize_kif_line(line: &str) -> String {
    let mut normalized = line.replace('王', "玉").replace('竜', "龍");
    normalized = normalized.replace("成銀", "全");
    normalized = normalized.replace("成桂", "圭");
    normalized = normalized.replace("成香", "杏");
    normalized = normalized.replace("成歩", "と");
    normalized
}

fn append_comment_text(target: &mut Option<String>, comment: &str) {
    if comment.is_empty() {
        return;
    }

    if let Some(existing) = target {
        existing.push('\n');
        existing.push_str(comment);
    } else {
        *target = Some(comment.to_string());
    }
}

fn append_move_comment(record: &mut AnnotatedMoveEntry, comment: &str) {
    let mut combined = record.comment().map(ToOwned::to_owned);
    append_comment_text(&mut combined, comment);
    record.set_comment(combined);
}

fn kanji_to_int(text: &str) -> u8 {
    if text.is_empty() {
        return 1;
    }
    let mut total = 0u8;
    let mut current = 0u8;
    for ch in text.chars() {
        match ch {
            '十' => {
                current = current.max(1);
                total = total.saturating_add(current.saturating_mul(10));
                current = 0;
            }
            '〇' | '零' => current = current.saturating_mul(10),
            '一' => current = current.saturating_mul(10).saturating_add(1),
            '二' => current = current.saturating_mul(10).saturating_add(2),
            '三' => current = current.saturating_mul(10).saturating_add(3),
            '四' => current = current.saturating_mul(10).saturating_add(4),
            '五' => current = current.saturating_mul(10).saturating_add(5),
            '六' => current = current.saturating_mul(10).saturating_add(6),
            '七' => current = current.saturating_mul(10).saturating_add(7),
            '八' => current = current.saturating_mul(10).saturating_add(8),
            '九' => current = current.saturating_mul(10).saturating_add(9),
            _ => {}
        }
    }
    let total = total.saturating_add(current);
    if total == 0 { 1 } else { total }
}

fn parse_hand_pieces(text: &str) -> HashMap<String, u8> {
    let cleaned = text.replace('　', " ").replace(' ', "");
    if cleaned.is_empty() || cleaned == "なし" {
        return HashMap::new();
    }
    let mut counts: HashMap<String, u8> = HashMap::new();
    let mut iter = cleaned.chars().peekable();
    while let Some(ch) = iter.next() {
        let piece_code = match ch {
            '歩' => "FU",
            '香' => "KY",
            '桂' => "KE",
            '銀' => "GI",
            '金' => "KI",
            '角' => "KA",
            '飛' => "HI",
            '玉' | '王' => "OU",
            _ => continue,
        };
        let mut number = String::new();
        while let Some(next) = iter.peek() {
            if "一二三四五六七八九十〇零".contains(*next) {
                number.push(*next);
                iter.next();
            } else {
                break;
            }
        }
        let count = kanji_to_int(&number);
        *counts.entry(piece_code.to_string()).or_insert(0) += count;
    }
    counts
}

fn split_metadata_field(line: &str) -> Option<(&str, &str)> {
    let trimmed = line.trim();
    if let Some((key, value)) = trimmed.split_once('：') {
        return Some((key.trim(), value.trim()));
    }
    if let Some((key, value)) = trimmed.split_once(':') {
        return Some((key.trim(), value.trim()));
    }
    None
}

fn apply_metadata_line(builder: &mut RecordMetadataBuilder, line: &str) {
    if let Some((key, value)) = split_metadata_field(line) {
        match key {
            "先手" | "下手" => {
                builder.black_player(Some(value.to_string()));
            }
            "後手" | "上手" => {
                builder.white_player(Some(value.to_string()));
            }
            "棋戦" => {
                builder.event(Some(value.to_string()));
            }
            "場所" => {
                builder.site(Some(value.to_string()));
            }
            "開始日時" | "開始日" => {
                builder.start_date(Some(value.to_string()));
            }
            "終了日時" | "終了日" => {
                builder.end_date(Some(value.to_string()));
            }
            "持ち時間" => {
                let parsed =
                    parse_kif_time_control(value).or_else(|| TimeControl::from_spec(value));
                builder.time_control(parsed);
            }
            "先手持ち時間" => {
                builder.black_time_control(TimeControl::from_spec(value));
            }
            "後手持ち時間" => {
                builder.white_time_control(TimeControl::from_spec(value));
            }
            "最大手数" => {
                builder.max_moves(value.parse::<u32>().ok());
            }
            "持将棋" => {
                builder.impasse_rule(Some(value.to_string()));
            }
            "備考" => {
                builder.append_comment_line(value);
            }
            "手合割" | "戦型" => {
                builder.add_attribute(key.to_string(), value.to_string());
            }
            _ => {
                builder.add_attribute(key.to_string(), value.to_string());
            }
        }
    }
}

fn parse_time_to_ms(line: &str) -> Option<u32> {
    let text = line.find("消費時間").map_or(line, |idx| &line[idx + "消費時間".len()..]);
    let digits: Vec<&str> = text.split(['：', ':', ' ', '　']).collect();
    let mut seconds = 0u32;
    let mut multiplier = 1;
    for part in digits.iter().rev().filter(|text| !text.is_empty()) {
        if let Ok(value) = part.parse::<u32>() {
            seconds += value * multiplier;
            multiplier *= 60;
        }
    }
    if seconds == 0 { None } else { Some(seconds * 1_000) }
}

fn parse_colon_time_to_ms(text: &str) -> Option<u32> {
    let mut seconds = 0u32;
    let mut multiplier = 1u32;
    let mut parsed_any = false;
    for part in text.split(['：', ':', ' ', '　']).filter(|part| !part.is_empty()).rev() {
        if let Ok(value) = part.parse::<u32>() {
            seconds += value * multiplier;
            multiplier *= 60;
            parsed_any = true;
        }
    }
    if parsed_any { Some(seconds * 1_000) } else { None }
}

fn parse_kif_terminal_elapsed_time_ms(text: &str) -> Option<u32> {
    let open = text.find('(').or_else(|| text.find('（'))?;
    let rest = &text[open + 1..];
    let close = rest.find(')').or_else(|| rest.find('）')).unwrap_or(rest.len());
    let inner = &rest[..close];
    let elapsed = inner.split('/').next().unwrap_or(inner).trim();
    if !elapsed.contains(':') && !elapsed.contains('：') {
        return None;
    }
    if !inner.contains('/') {
        return None;
    }
    parse_colon_time_to_ms(elapsed)
}

fn split_kif_inline_elapsed_time(line: &str) -> (String, Option<u32>) {
    let normalized = normalize_kif_line(line);

    for (open, ch) in normalized.char_indices().rev() {
        if ch != '(' && ch != '（' {
            continue;
        }
        let rest = &normalized[open + ch.len_utf8()..];
        let Some(close_rel) = rest.find([')', '）']) else {
            continue;
        };
        let inner = &rest[..close_rel];
        if !inner.contains('/') || (!inner.contains(':') && !inner.contains('：')) {
            continue;
        }
        let elapsed = inner.split('/').next().unwrap_or(inner).trim();
        let time_ms = parse_colon_time_to_ms(elapsed);
        return (normalized[..open].trim_end().to_string(), time_ms);
    }

    (normalized, None)
}

fn resolve_kif_move_time(
    inline_time_ms: Option<u32>,
    pending_time_ms: &mut Option<u32>,
) -> Option<u32> {
    if let Some(time_ms) = inline_time_ms {
        // インライン消費時間を優先するが、保留中のスタンドアロン時間もこの手に帰属する。
        *pending_time_ms = None;
        Some(time_ms)
    } else {
        pending_time_ms.take()
    }
}

fn parse_kif_terminal_record_from_line(
    line: &str,
    side_to_move: Color,
) -> Option<(SpecialMoveEntry, RecordAnnotation)> {
    let normalized = normalize_kif_line(line);
    let mut idx = 0usize;
    let ws_begin = idx;
    while let Some(ch) = normalized[idx..].chars().next() {
        if ch.is_whitespace() {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }
    let _ = ws_begin;
    let digits_begin = idx;
    while let Some(ch) = normalized[idx..].chars().next() {
        if ch.is_ascii_digit() {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }
    let digits_end = idx;
    if digits_begin == digits_end {
        return None;
    }
    let ws_after_digits_begin = idx;
    while let Some(ch) = normalized[idx..].chars().next() {
        if ch.is_whitespace() {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }
    if ws_after_digits_begin == idx {
        return None;
    }

    let body = &normalized[idx..];
    if body.is_empty() {
        return None;
    }
    let end = body
        .char_indices()
        .find(|(_, ch)| ch.is_whitespace() || *ch == '(' || *ch == '（')
        .map_or(body.len(), |(i, _)| i);
    let name = body[..end].trim();
    if name.is_empty() {
        return None;
    }

    let record = terminal_from_kif_name(name, side_to_move);
    let mut annotation = RecordAnnotation::new();
    let trailing = &body[end..];
    if let Some(time_ms) = parse_kif_terminal_elapsed_time_ms(trailing) {
        annotation.set_elapsed_ms(Some(time_ms));
    }
    Some((record, annotation))
}

fn parse_variation_start(line: &str) -> usize {
    line.chars().filter(|ch| ch.is_ascii_digit()).collect::<String>().parse::<usize>().unwrap_or(0)
}

fn is_board_header_line(line: &str) -> bool {
    line.contains('９') && line.contains('１') && line.trim_start().starts_with('９')
}

fn parse_board_block(lines: &[String], start_index: usize) -> Result<(usize, BoardMap), KifError> {
    let mut idx = start_index + 1;
    if idx < lines.len() && lines[idx].starts_with('+') {
        idx += 1;
    }
    let mut board_map: BoardMap = HashMap::new();
    for row in 0..9 {
        if idx >= lines.len() {
            break;
        }
        let row_line = &lines[idx];
        idx += 1;
        let segments: Vec<&str> = row_line.split('|').collect();
        let mut file_index = 0u8;
        for seg in segments.iter().skip(1).take(9) {
            file_index += 1;
            let token = seg.trim();
            if token.is_empty() || token == "・" || token == "　" {
                continue;
            }
            let mut chars = token.chars();
            let mut color = '+';
            let mut piece_text = token.to_string();
            if let Some(prefix) = chars.next()
                && (prefix == 'v' || prefix == '^')
            {
                color = '-';
                piece_text = chars.collect();
            }
            let piece_code = match piece_text.as_str() {
                "歩" => "FU",
                "香" => "KY",
                "桂" => "KE",
                "銀" => "GI",
                "金" => "KI",
                "角" => "KA",
                "飛" => "HI",
                "玉" | "王" => "OU",
                "と" => "TO",
                "杏" => "NY",
                "圭" => "NK",
                "全" => "NG",
                "馬" => "UM",
                "龍" | "竜" => "RY",
                _ => continue,
            };
            let file = 10 - file_index;
            let rank = (row + 1) as u8;
            board_map.insert((file, rank), (color, piece_code.to_string()));
        }
    }
    if idx < lines.len() && lines[idx].starts_with('+') {
        idx += 1;
    }
    Ok((idx, board_map))
}

fn parse_kif_destination(text: &str, last_to: Option<Square>) -> Result<(Square, usize), KifError> {
    let mut chars = text.chars();
    let first = chars.next().ok_or_else(|| KifError::InvalidMove(text.to_string()))?;
    if first == '同' {
        let sq = last_to.ok_or_else(|| KifError::InvalidMove(text.to_string()))?;
        return Ok((sq, first.len_utf8()));
    }
    let file_digit = if let Some(idx) = WIDE_DIGITS.iter().position(|ch| *ch == first) {
        (idx + 1) as u8
    } else if first.is_ascii_digit() {
        first.to_digit(10).ok_or_else(|| KifError::InvalidMove(text.to_string()))? as u8
    } else {
        return Err(KifError::InvalidMove(text.to_string()));
    };
    let second = chars.next().ok_or_else(|| KifError::InvalidMove(text.to_string()))?;
    let rank_idx = KANJI_RANKS
        .iter()
        .position(|ch| *ch == second)
        .ok_or_else(|| KifError::InvalidMove(text.to_string()))?;
    let rank_digit = (rank_idx + 1) as u8;
    let file_char = char::from(b'0' + file_digit);
    let rank_char = char::from(b'a' + (rank_digit - 1));
    let sq = Square::from_usi(&format!("{file_char}{rank_char}"))
        .ok_or_else(|| KifError::InvalidMove(text.to_string()))?;
    Ok((sq, first.len_utf8() + second.len_utf8()))
}

fn parse_kif_move(
    _pos: &Position,
    line: &str,
    last_to: Option<Square>,
) -> Result<(Move, Square, Option<u32>), KifError> {
    let (normalized, inline_time_ms) = split_kif_inline_elapsed_time(line);
    let mut idx = 0usize;
    while let Some(ch) = normalized[idx..].chars().next() {
        if ch.is_whitespace() {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }
    while let Some(ch) = normalized[idx..].chars().next() {
        if ch.is_ascii_digit() {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }
    while let Some(ch) = normalized[idx..].chars().next() {
        if ch.is_whitespace() {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }
    let dest_part = &normalized[idx..];
    let (to_sq, consumed) = parse_kif_destination(dest_part, last_to)?;
    idx += consumed;
    while let Some(ch) = normalized[idx..].chars().next() {
        if ch.is_whitespace() {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }

    let mut iter = normalized[idx..].chars();
    let piece_char = iter.next().ok_or_else(|| KifError::InvalidMove(line.to_string()))?;
    let piece_code = match piece_char {
        '歩' => "P",
        '香' => "L",
        '桂' => "N",
        '銀' => "S",
        '金' => "G",
        '角' => "B",
        '飛' => "R",
        '玉' | '王' => "K",
        'と' => "+P",
        '杏' => "+L",
        '圭' => "+N",
        '全' => "+S",
        '馬' => "+B",
        '龍' | '竜' => "+R",
        _ => return Err(KifError::InvalidMove(line.to_string())),
    };
    idx += piece_char.len_utf8();
    let mut promote = false;
    if let Some(ch) = normalized[idx..].chars().next()
        && ch == '成'
    {
        promote = true;
        idx += ch.len_utf8();
    }
    while let Some(ch) = normalized[idx..].chars().next() {
        if ch.is_whitespace() {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }

    if normalized[idx..].starts_with("打") {
        let usi = format!("{piece_code}*{to_sq}");
        let mv16 = Move::from_usi(&usi).ok_or_else(|| KifError::InvalidMove(line.to_string()))?;
        return Ok((mv16, to_sq, inline_time_ms));
    }

    if let Some(open_idx) = normalized[idx..].find('(') {
        let start = idx + open_idx + 1;
        let end = normalized[start..].find(')').map(|v| start + v);
        let end = end.ok_or_else(|| KifError::InvalidMove(line.to_string()))?;
        let from_text = &normalized[start..end];
        if from_text.len() == 2 && from_text.chars().all(|ch| ch.is_ascii_digit()) {
            let file_digit = from_text
                .chars()
                .next()
                .and_then(|ch| ch.to_digit(10))
                .ok_or_else(|| KifError::InvalidMove(line.to_string()))?
                as u8;
            let rank_digit = from_text
                .chars()
                .nth(1)
                .and_then(|ch| ch.to_digit(10))
                .ok_or_else(|| KifError::InvalidMove(line.to_string()))?
                as u8;
            let file_char = char::from(b'0' + file_digit);
            let rank_char = char::from(b'a' + (rank_digit - 1));
            let from_sq = Square::from_usi(&format!("{file_char}{rank_char}"))
                .ok_or_else(|| KifError::InvalidMove(line.to_string()))?;
            let mut usi = format!("{from_sq}{to_sq}");
            if promote {
                usi.push('+');
            }
            let mv16 =
                Move::from_usi(&usi).ok_or_else(|| KifError::InvalidMove(line.to_string()))?;
            return Ok((mv16, to_sq, inline_time_ms));
        }
    }

    Err(KifError::InvalidMove(line.to_string()))
}

fn parse_kif_variation_moves(
    lines: &[String],
    pos_start: &Position,
    last_to: Option<Square>,
) -> Result<Vec<AnnotatedMoveEntry>, KifError> {
    let mut pos = pos_start.clone();
    let mut moves: Vec<AnnotatedMoveEntry> = Vec::new();
    let mut last_to = last_to;
    let mut refresh_counter = 0usize;
    let mut pending_comment: Option<String> = None;
    let mut pending_time_ms: Option<u32> = None;

    for raw in lines {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("変化") || line.starts_with("まで") {
            break;
        }
        if line.starts_with('*') || line.starts_with('\'') {
            let comment = line.trim_start_matches(['*', '\'']).trim();
            if !comment.is_empty() {
                if let Some(last) = moves.last_mut() {
                    append_move_comment(last, comment);
                } else {
                    append_comment_text(&mut pending_comment, comment);
                }
            }
            continue;
        }
        if line.contains("消費時間") {
            if let Some(time_ms) = parse_time_to_ms(line) {
                if let Some(last) = moves.last_mut() {
                    last.set_time_ms(Some(time_ms));
                } else {
                    pending_time_ms = Some(time_ms);
                }
            }
            continue;
        }
        if line.starts_with("**評価値=") {
            if let Some(last) = moves.last_mut()
                && let Ok(value) = line.trim_start_matches("**評価値=").parse::<i32>()
            {
                last.set_eval(Some(Eval::from_i32(value)));
            }
            continue;
        }
        if line.starts_with("手数") && line.contains("指手") {
            continue;
        }

        if line.chars().next().is_some_and(|ch| ch.is_ascii_digit()) || line.contains("手数") {
            match parse_kif_move(&pos, line, last_to) {
                Ok((mv16, to_sq, inline_time_ms)) => {
                    if !pos.is_legal_move(mv16) {
                        return Err(KifError::InvalidMove(line.to_string()));
                    }
                    let mut mv_record = AnnotatedMoveEntry::new(mv16);
                    if let Some(comment) = pending_comment.take() {
                        mv_record = mv_record.with_comment(Some(comment));
                    }
                    if let Some(time_ms) =
                        resolve_kif_move_time(inline_time_ms, &mut pending_time_ms)
                    {
                        mv_record = mv_record.with_time_ms(Some(time_ms));
                    }
                    pos.apply_move(mv16);
                    moves.push(mv_record);
                    last_to = Some(to_sq);
                    refresh_position_if_needed(&mut pos, &mut refresh_counter)?;
                }
                Err(_) => {
                    if parse_kif_terminal_record_from_line(line, pos.turn()).is_some() {
                        break;
                    }
                    return Err(KifError::InvalidMove(line.to_string()));
                }
            }
        }
    }

    Ok(moves)
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Ki2HDirection {
    Left,
    Right,
    None,
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum Ki2VDirection {
    Up,
    Down,
    None,
}

fn is_ki2_move_marker(ch: char) -> bool {
    matches!(ch, '▲' | '△' | '▼' | '▽' | '☗' | '☖')
}

fn parse_ki2_piece(text: &str) -> Option<(PieceType, usize)> {
    let entries = [
        ("成銀", PieceType::PRO_SILVER),
        ("成桂", PieceType::PRO_KNIGHT),
        ("成香", PieceType::PRO_LANCE),
        ("王", PieceType::KING),
        ("玉", PieceType::KING),
        ("飛", PieceType::ROOK),
        ("龍", PieceType::DRAGON),
        ("竜", PieceType::DRAGON),
        ("角", PieceType::BISHOP),
        ("馬", PieceType::HORSE),
        ("金", PieceType::GOLD),
        ("銀", PieceType::SILVER),
        ("全", PieceType::PRO_SILVER),
        ("桂", PieceType::KNIGHT),
        ("圭", PieceType::PRO_KNIGHT),
        ("香", PieceType::LANCE),
        ("杏", PieceType::PRO_LANCE),
        ("歩", PieceType::PAWN),
        ("と", PieceType::PRO_PAWN),
    ];
    for (symbol, piece_type) in entries {
        if text.starts_with(symbol) {
            return Some((piece_type, symbol.len()));
        }
    }
    None
}

fn ki2_direction(from: Square, to: Square, side: Color) -> (Ki2HDirection, Ki2VDirection) {
    let (from_file, from_rank) = if side == Color::BLACK {
        (from.file().raw(), from.rank().raw())
    } else {
        let flipped = crate::types::square::flip(from);
        (flipped.file().raw(), flipped.rank().raw())
    };
    let (to_file, to_rank) = if side == Color::BLACK {
        (to.file().raw(), to.rank().raw())
    } else {
        let flipped = crate::types::square::flip(to);
        (flipped.file().raw(), flipped.rank().raw())
    };

    let h_dir = match from_file.cmp(&to_file) {
        std::cmp::Ordering::Greater => Ki2HDirection::Right,
        std::cmp::Ordering::Less => Ki2HDirection::Left,
        std::cmp::Ordering::Equal => Ki2HDirection::None,
    };
    let v_dir = match from_rank.cmp(&to_rank) {
        std::cmp::Ordering::Greater => Ki2VDirection::Up,
        std::cmp::Ordering::Less => Ki2VDirection::Down,
        std::cmp::Ordering::Equal => Ki2VDirection::None,
    };
    (h_dir, v_dir)
}

fn split_ki2_sections(text: &str) -> Vec<String> {
    let mut sections: Vec<String> = Vec::new();
    let mut last_index = 0usize;
    let mut indices: Vec<(usize, char)> = text.char_indices().collect();
    indices.push((text.len(), '\0'));
    for (idx, ch) in indices.into_iter().skip(1) {
        if ch == '\0' || is_ki2_move_marker(ch) {
            if last_index < idx {
                sections.push(text[last_index..idx].to_string());
            }
            last_index = idx;
        }
    }
    sections
}

fn parse_ki2_sections(
    pos: &Position,
    line: &str,
    last_to: Option<Square>,
) -> Result<Vec<(Move, Square)>, Ki2Error> {
    let clean = line.replace([' ', '　'], "");
    let sections = split_ki2_sections(&clean);
    let mut out: Vec<(Move, Square)> = Vec::new();
    let mut current_last = last_to;
    let mut tmp_pos = pos.clone();

    for section in sections {
        if section.is_empty() {
            continue;
        }
        let mut idx = 0usize;
        if let Some(ch) = section[idx..].chars().next()
            && is_ki2_move_marker(ch)
        {
            idx += ch.len_utf8();
        }
        let dest_part = &section[idx..];
        let (to_sq, consumed) = parse_kif_destination(dest_part, current_last)
            .map_err(|_| Ki2Error::InvalidMove(line.to_string()))?;
        idx += consumed;

        let piece_part = &section[idx..];
        let (piece_type, piece_len) =
            parse_ki2_piece(piece_part).ok_or_else(|| Ki2Error::InvalidMove(line.to_string()))?;
        idx += piece_len;

        let mut relative = "";
        let mut motion = "";
        if let Some(ch) = section[idx..].chars().next()
            && (ch == '左' || ch == '右' || ch == '直')
        {
            relative = if ch == '左' {
                "左"
            } else if ch == '右' {
                "右"
            } else {
                "直"
            };
            idx += ch.len_utf8();
        }
        if let Some(ch) = section[idx..].chars().next()
            && (ch == '引' || ch == '寄' || ch == '上' || ch == '行')
        {
            motion = if ch == '引' {
                "引"
            } else if ch == '寄' {
                "寄"
            } else {
                "上"
            };
            idx += ch.len_utf8();
        }

        let mut promote = false;
        let mut not_promote = false;
        let mut drop = false;
        if section[idx..].starts_with("不成") {
            not_promote = true;
            idx += "不成".len();
        } else if section[idx..].starts_with('成') {
            promote = true;
            idx += "成".len();
        } else if section[idx..].starts_with('打') {
            drop = true;
            idx += "打".len();
        }

        let mut from_sq: Option<Square> = None;
        if section[idx..].starts_with('(')
            && let Some(end) = section[idx..].find(')')
        {
            let start = idx + 1;
            let end = idx + end;
            let from_text = &section[start..end];
            if from_text.len() == 2 && from_text.chars().all(|ch| ch.is_ascii_digit()) {
                let file_digit = from_text
                    .chars()
                    .next()
                    .and_then(|ch| ch.to_digit(10))
                    .ok_or_else(|| Ki2Error::InvalidMove(line.to_string()))?
                    as u8;
                let rank_digit = from_text
                    .chars()
                    .nth(1)
                    .and_then(|ch| ch.to_digit(10))
                    .ok_or_else(|| Ki2Error::InvalidMove(line.to_string()))?
                    as u8;
                let file_char = char::from(b'0' + file_digit);
                let rank_char = char::from(b'a' + (rank_digit - 1));
                from_sq = Square::from_usi(&format!("{file_char}{rank_char}"));
            }
        }

        let side = tmp_pos.turn();
        let horse_or_dragon = piece_type == PieceType::HORSE || piece_type == PieceType::DRAGON;
        let mv16 = if drop {
            Move::drop(piece_type.demote(), to_sq)
        } else {
            let from = if let Some(from) = from_sq {
                from
            } else {
                let attackers = tmp_pos.attackers_to_color(side, to_sq, tmp_pos.pieces());
                let mut squares: Vec<Square> = Vec::new();
                let mut bb = attackers.and(tmp_pos.pieces_for(piece_type, side));
                while let Some(sq) = bb.pop_lsb() {
                    let (h_dir, v_dir) = ki2_direction(sq, to_sq, side);
                    if motion.contains('引') && v_dir != Ki2VDirection::Down {
                        continue;
                    }
                    if motion.contains('寄') && v_dir != Ki2VDirection::None {
                        continue;
                    }
                    if (motion.contains('上') || motion.contains('行'))
                        && v_dir != Ki2VDirection::Up
                    {
                        continue;
                    }
                    if relative.contains('直')
                        && (h_dir != Ki2HDirection::None || v_dir != Ki2VDirection::Up)
                    {
                        continue;
                    }
                    if horse_or_dragon {
                        if relative.contains('左') && h_dir == Ki2HDirection::Left {
                            continue;
                        }
                        if relative.contains('右') && h_dir == Ki2HDirection::Right {
                            continue;
                        }
                    } else {
                        if relative.contains('左') && h_dir != Ki2HDirection::Right {
                            continue;
                        }
                        if relative.contains('右') && h_dir != Ki2HDirection::Left {
                            continue;
                        }
                    }
                    squares.push(sq);
                }
                if squares.len() == 2 && horse_or_dragon {
                    squares.retain(|sq| ki2_direction(*sq, to_sq, side).0 != Ki2HDirection::None);
                }
                if squares.len() == 1 {
                    squares[0]
                } else if squares.is_empty() {
                    let hand_piece = crate::types::HandPiece::from_piece_type(piece_type);
                    if let Some(hand_piece) = hand_piece {
                        if tmp_pos.hand(side).count(hand_piece) > 0 {
                            drop = true;
                            Square::NONE
                        } else {
                            return Err(Ki2Error::InvalidMove(line.to_string()));
                        }
                    } else {
                        return Err(Ki2Error::InvalidMove(line.to_string()));
                    }
                } else {
                    return Err(Ki2Error::InvalidMove(line.to_string()));
                }
            };
            if drop {
                Move::drop(piece_type.demote(), to_sq)
            } else if promote && !not_promote {
                Move::promotion(from, to_sq)
            } else {
                Move::normal(from, to_sq)
            }
        };

        out.push((mv16, to_sq));
        current_last = Some(to_sq);
        let mv = tmp_pos.move32_from_move(mv16);
        if mv.is_normal() {
            tmp_pos.apply_move32(mv);
        }
    }
    Ok(out)
}

fn parse_ki2_variation_moves(
    lines: &[String],
    pos_start: &Position,
    last_to: Option<Square>,
) -> Result<Vec<AnnotatedMoveEntry>, Ki2Error> {
    let mut pos = pos_start.clone();
    let mut moves: Vec<AnnotatedMoveEntry> = Vec::new();
    let mut last_to = last_to;
    let mut refresh_counter = 0usize;
    let mut pending_comment: Option<String> = None;
    let mut pending_time_ms: Option<u32> = None;

    for raw in lines {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("変化") || line.starts_with("まで") {
            break;
        }
        if line.starts_with("**") || line.starts_with('*') || line.starts_with('\'') {
            let comment = line.trim_start_matches(&['*', '\''][..]).trim();
            if !comment.is_empty() {
                if let Some(last) = moves.last_mut() {
                    last.set_comment(Some(comment.to_string()));
                } else {
                    pending_comment = Some(comment.to_string());
                }
            }
            continue;
        }
        if line.contains("消費時間") {
            if let Some(time_ms) = parse_time_to_ms(line) {
                if let Some(last) = moves.last_mut() {
                    last.set_time_ms(Some(time_ms));
                } else {
                    pending_time_ms = Some(time_ms);
                }
            }
            continue;
        }
        if line.starts_with("手数") {
            continue;
        }

        let parsed = parse_ki2_sections(&pos, line, last_to)?;
        for (mv16, to_sq) in parsed {
            if !pos.is_legal_move(mv16) {
                return Err(Ki2Error::InvalidMove(line.to_string()));
            }
            let mut mv_record = AnnotatedMoveEntry::new(mv16);
            if let Some(comment) = pending_comment.take() {
                mv_record = mv_record.with_comment(Some(comment));
            }
            if let Some(time_ms) = pending_time_ms.take() {
                mv_record = mv_record.with_time_ms(Some(time_ms));
            }
            pos.apply_move(mv16);
            moves.push(mv_record);
            last_to = Some(to_sq);
            refresh_position_if_needed(&mut pos, &mut refresh_counter)?;
        }
    }

    Ok(moves)
}

fn terminal_from_kif_name(name: &str, side_to_move: Color) -> SpecialMoveEntry {
    let is_black_turn = side_to_move == Color::BLACK;
    let (kind, result) = match name {
        "投了" | "詰み" | "詰" => {
            if is_black_turn {
                (SpecialMove::Resign, GameResult::WhiteWin)
            } else {
                (SpecialMove::Resign, GameResult::BlackWin)
            }
        }
        "切れ負け" => {
            if is_black_turn {
                (SpecialMove::Timeout, GameResult::WhiteWinByTimeout)
            } else {
                (SpecialMove::Timeout, GameResult::BlackWinByTimeout)
            }
        }
        "反則勝ち" => {
            if is_black_turn {
                (SpecialMove::WinByIllegalMove, GameResult::BlackWinByIllegalMove)
            } else {
                (SpecialMove::WinByIllegalMove, GameResult::WhiteWinByIllegalMove)
            }
        }
        "反則負け" => {
            if is_black_turn {
                (SpecialMove::LoseByIllegalMove, GameResult::WhiteWinByIllegalMove)
            } else {
                (SpecialMove::LoseByIllegalMove, GameResult::BlackWinByIllegalMove)
            }
        }
        "入玉宣言" | "入玉勝ち" => {
            if is_black_turn {
                (SpecialMove::WinByDeclaration, GameResult::BlackWinByDeclaration)
            } else {
                (SpecialMove::WinByDeclaration, GameResult::WhiteWinByDeclaration)
            }
        }
        "不戦勝" => {
            if is_black_turn {
                (SpecialMove::WinByDefault, GameResult::BlackWinByForfeit)
            } else {
                (SpecialMove::WinByDefault, GameResult::WhiteWinByForfeit)
            }
        }
        "不戦敗" => {
            if is_black_turn {
                (SpecialMove::LoseByDefault, GameResult::WhiteWinByForfeit)
            } else {
                (SpecialMove::LoseByDefault, GameResult::BlackWinByForfeit)
            }
        }
        "千日手" => (SpecialMove::RepetitionDraw, GameResult::DrawByRepetition),
        "最大手数" => (SpecialMove::MaxMoves, GameResult::DrawByMaxPlies),
        "持将棋" => (SpecialMove::Impasse, GameResult::DrawByImpasse),
        "中断" => (SpecialMove::Interrupt, GameResult::Paused),
        "不詰" => (SpecialMove::NoMate, GameResult::Paused),
        "トライ" => (SpecialMove::Try, GameResult::Paused),
        _ => (SpecialMove::Unknown(name.to_string()), GameResult::Paused),
    };
    SpecialMoveEntry::new(kind, result)
}

fn parse_kif_summary_result(line: &str, side_to_move: Color) -> Option<SpecialMoveEntry> {
    let trimmed = line.trim();
    if !trimmed.starts_with("まで") {
        return None;
    }
    if trimmed.contains("時間切れ") {
        if trimmed.contains("先手") {
            return Some(SpecialMoveEntry::new(
                SpecialMove::Timeout,
                GameResult::BlackWinByTimeout,
            ));
        }
        if trimmed.contains("後手") {
            return Some(SpecialMoveEntry::new(
                SpecialMove::Timeout,
                GameResult::WhiteWinByTimeout,
            ));
        }
    }
    if trimmed.contains("千日手") {
        return Some(SpecialMoveEntry::new(
            SpecialMove::RepetitionDraw,
            GameResult::DrawByRepetition,
        ));
    }
    if trimmed.contains("最大手数") {
        return Some(SpecialMoveEntry::new(SpecialMove::MaxMoves, GameResult::DrawByMaxPlies));
    }
    if trimmed.contains("持将棋") {
        return Some(SpecialMoveEntry::new(SpecialMove::Impasse, GameResult::DrawByImpasse));
    }
    if trimmed.contains("中断") {
        return Some(SpecialMoveEntry::new(SpecialMove::Interrupt, GameResult::Paused));
    }
    if trimmed.contains("入玉宣言") || trimmed.contains("入玉勝ち") {
        if trimmed.contains("先手") {
            return Some(SpecialMoveEntry::new(
                SpecialMove::WinByDeclaration,
                GameResult::BlackWinByDeclaration,
            ));
        }
        if trimmed.contains("後手") {
            return Some(SpecialMoveEntry::new(
                SpecialMove::WinByDeclaration,
                GameResult::WhiteWinByDeclaration,
            ));
        }
        return None;
    }
    if trimmed.contains("反則勝ち") {
        if trimmed.contains("先手") {
            return Some(SpecialMoveEntry::new(
                SpecialMove::WinByIllegalMove,
                GameResult::BlackWinByIllegalMove,
            ));
        }
        if trimmed.contains("後手") {
            return Some(SpecialMoveEntry::new(
                SpecialMove::WinByIllegalMove,
                GameResult::WhiteWinByIllegalMove,
            ));
        }
    }
    if trimmed.contains("反則負け") {
        if trimmed.contains("先手") {
            return Some(SpecialMoveEntry::new(
                SpecialMove::LoseByIllegalMove,
                GameResult::WhiteWinByIllegalMove,
            ));
        }
        if trimmed.contains("後手") {
            return Some(SpecialMoveEntry::new(
                SpecialMove::LoseByIllegalMove,
                GameResult::BlackWinByIllegalMove,
            ));
        }
    }
    if trimmed.contains("不詰") {
        return Some(SpecialMoveEntry::new(SpecialMove::NoMate, GameResult::Paused));
    }
    if trimmed.ends_with("詰") {
        return Some(terminal_from_kif_name("詰", side_to_move));
    }
    if trimmed.contains("勝ち") {
        if trimmed.contains("先手") {
            return Some(SpecialMoveEntry::new(SpecialMove::Resign, GameResult::BlackWin));
        }
        if trimmed.contains("後手") {
            return Some(SpecialMoveEntry::new(SpecialMove::Resign, GameResult::WhiteWin));
        }
    }
    None
}

fn parse_ki2_summary_result(line: &str, side_to_move: Color) -> Option<SpecialMoveEntry> {
    let trimmed = line.trim();
    if !trimmed.starts_with("まで") {
        return None;
    }
    parse_kif_summary_result(trimmed, side_to_move)
}

fn looks_like_ki2_move_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    if let Some(ch) = trimmed.chars().next() {
        if is_ki2_move_marker(ch) || ch == '同' {
            return true;
        }
        if ch.is_ascii_digit() {
            return true;
        }
        if WIDE_DIGITS.contains(&ch) {
            return true;
        }
    }
    false
}

fn format_fullwidth_digit(value: u8) -> char {
    WIDE_DIGITS[(value - 1) as usize]
}

fn format_kanji_rank(value: u8) -> char {
    KANJI_RANKS[(value - 1) as usize]
}

fn piece_type_to_kif_move(piece_type: PieceType) -> &'static str {
    match piece_type {
        PieceType::PAWN => "歩",
        PieceType::LANCE => "香",
        PieceType::KNIGHT => "桂",
        PieceType::SILVER => "銀",
        PieceType::GOLD => "金",
        PieceType::BISHOP => "角",
        PieceType::ROOK => "飛",
        PieceType::KING => "玉",
        PieceType::PRO_PAWN => "と",
        PieceType::PRO_LANCE => "成香",
        PieceType::PRO_KNIGHT => "成桂",
        PieceType::PRO_SILVER => "成銀",
        PieceType::HORSE => "馬",
        PieceType::DRAGON => "龍",
        _ => "玉",
    }
}

fn piece_type_to_kif_board(piece_type: PieceType) -> &'static str {
    match piece_type {
        PieceType::PAWN => "歩",
        PieceType::LANCE => "香",
        PieceType::KNIGHT => "桂",
        PieceType::SILVER => "銀",
        PieceType::GOLD => "金",
        PieceType::BISHOP => "角",
        PieceType::ROOK => "飛",
        PieceType::KING => "玉",
        PieceType::PRO_PAWN => "と",
        PieceType::PRO_LANCE => "杏",
        PieceType::PRO_KNIGHT => "圭",
        PieceType::PRO_SILVER => "全",
        PieceType::HORSE => "馬",
        PieceType::DRAGON => "龍",
        _ => "玉",
    }
}

fn kif_display_width(text: &str) -> usize {
    text.chars().map(|ch| usize::from(ch.is_ascii()) + usize::from(!ch.is_ascii()) * 2).sum()
}

fn pad_kif_move_text(text: &str) -> String {
    let width = kif_display_width(text);
    if width >= 12 { text.to_string() } else { format!("{text}{}", " ".repeat(12 - width)) }
}

fn move_to_kif_text(
    pos: &Position,
    mv: Move32,
    last_to: Option<Square>,
) -> Result<(String, Square), KifError> {
    let to_sq = mv.to_sq();
    let mut dest = if Some(to_sq) == last_to {
        "同　".to_string()
    } else {
        let file = to_sq.file().raw() + 1;
        let rank = to_sq.rank().raw() + 1;
        format!("{}{}", format_fullwidth_digit(file as u8), format_kanji_rank(rank as u8))
    };
    let mut suffix = String::new();
    if mv.is_drop() {
        let piece_type = mv
            .dropped_piece()
            .ok_or_else(|| KifError::InvalidMove("drop move missing piece".to_string()))?;
        dest.push_str(piece_type_to_kif_move(piece_type));
        suffix.push('打');
    } else {
        let mover = pos.moved_piece_after(mv).piece_type();
        if mv.is_promotion() {
            let base = mover.demote();
            dest.push_str(piece_type_to_kif_move(base));
            suffix.push('成');
        } else {
            dest.push_str(piece_type_to_kif_move(mover));
        }
        let from = mv.from_sq();
        let file = from.file().raw() + 1;
        let rank = from.rank().raw() + 1;
        let _ = write!(suffix, "({file}{rank})");
    }
    Ok((pad_kif_move_text(&format!("{dest}{suffix}")), to_sq))
}

fn special_move_to_kif_text(special: &SpecialMoveEntry) -> String {
    let text = match special.kind() {
        SpecialMove::Resign => "投了",
        SpecialMove::Interrupt => "中断",
        SpecialMove::MaxMoves => "最大手数",
        SpecialMove::Impasse => "持将棋",
        SpecialMove::Draw => "引き分け",
        SpecialMove::RepetitionDraw => "千日手",
        SpecialMove::Mate => "詰み",
        SpecialMove::NoMate => "不詰",
        SpecialMove::Timeout => "切れ負け",
        SpecialMove::WinByIllegalMove => "反則勝ち",
        SpecialMove::LoseByIllegalMove => "反則負け",
        SpecialMove::WinByDeclaration => "入玉勝ち",
        SpecialMove::WinByDefault => "不戦勝",
        SpecialMove::LoseByDefault => "不戦敗",
        SpecialMove::Try => "トライ",
        SpecialMove::Unknown(name) => name,
    };
    pad_kif_move_text(text)
}

fn format_kif_move_line(
    move_no: usize,
    move_text: &str,
    elapsed_ms: u32,
    total_ms: u32,
    has_branch: bool,
) -> String {
    let mut line = format!(
        "{move_no:>4} {move_text} ({}/{})",
        format_time_mss(elapsed_ms),
        format_time_hhmmss(total_ms)
    );
    if has_branch {
        line.push('+');
    }
    line
}

fn format_kif_summary(terminal: &SpecialMoveEntry, side_to_move: Color, ply: usize) -> String {
    let next = if side_to_move == Color::BLACK { "先手" } else { "後手" };
    let last = if side_to_move == Color::BLACK { "後手" } else { "先手" };
    match terminal.kind() {
        SpecialMove::Resign => format!("まで{ply}手で{last}の勝ち"),
        SpecialMove::Interrupt => format!("まで{ply}手で中断"),
        SpecialMove::MaxMoves => format!("まで{ply}手で最大手数"),
        SpecialMove::Impasse => format!("まで{ply}手で持将棋"),
        SpecialMove::Draw => format!("まで{ply}手で引き分け"),
        SpecialMove::RepetitionDraw => format!("まで{ply}手で千日手"),
        SpecialMove::Mate => format!("まで{ply}手で詰み"),
        SpecialMove::NoMate => format!("まで{ply}手で不詰"),
        SpecialMove::Timeout => format!("まで{ply}手で時間切れにより{last}の勝ち"),
        SpecialMove::WinByIllegalMove => format!("まで{ply}手で{next}の反則勝ち"),
        SpecialMove::LoseByIllegalMove => format!("まで{ply}手で{next}の反則負け"),
        SpecialMove::WinByDeclaration => format!("まで{ply}手で{next}の入玉勝ち"),
        SpecialMove::WinByDefault => format!("まで{ply}手で{next}の不戦勝"),
        SpecialMove::LoseByDefault => format!("まで{ply}手で{next}の不戦敗"),
        SpecialMove::Try => format!("まで{ply}手で{next}のトライ"),
        SpecialMove::Unknown(name) => format!("まで{ply}手で{name}"),
    }
}

fn append_kif_eval_line(lines: &mut Vec<String>, annotation: &RecordAnnotation) {
    if let Some(eval) = annotation.eval() {
        lines.push(format!("**評価値={}", eval.to_i32()));
    }
}

fn format_time_mss(time_ms: u32) -> String {
    let total_seconds = time_ms / 1_000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes:>2}:{seconds:02}")
}

fn format_time_hhmmss(time_ms: u32) -> String {
    let total_seconds = time_ms / 1_000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
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

fn append_kif_metadata(lines: &mut Vec<String>, metadata: &RecordMetadata) {
    if let Some(event) = metadata.event() {
        lines.push(format!("棋戦：{event}"));
    }
    if let Some(site) = metadata.site() {
        lines.push(format!("場所：{site}"));
    }
    if let Some(start) = metadata.start_date() {
        lines.push(format!("開始日時：{start}"));
    }
    if let Some(end) = metadata.end_date() {
        lines.push(format!("終了日時：{end}"));
    }
    if let Some(tc) = metadata.time_control()
        && let Some(text) = format_kif_time_control_text(tc)
    {
        lines.push(format!("持ち時間：{text}"));
    }
    if let Some(tc) = metadata.black_time_control() {
        lines.push(format!("先手持ち時間：{}", tc.to_spec()));
    }
    if let Some(tc) = metadata.white_time_control() {
        lines.push(format!("後手持ち時間：{}", tc.to_spec()));
    }
    if let Some(max_moves) = metadata.max_moves() {
        lines.push(format!("最大手数：{max_moves}"));
    }
    if let Some(rule) = metadata.impasse_rule() {
        lines.push(format!("持将棋：{rule}"));
    }
    if let Some(comment) = metadata.comment() {
        for line in comment.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            lines.push(format!("備考：{trimmed}"));
        }
    }
    if let Some(black) = metadata.black_player() {
        lines.push(format!("先手：{black}"));
    }
    if let Some(white) = metadata.white_player() {
        lines.push(format!("後手：{white}"));
    }
    if !metadata.attributes().is_empty() {
        let mut keys: Vec<&String> = metadata.attributes().keys().collect();
        keys.sort();
        for key in keys {
            if matches!(
                key.as_str(),
                "棋戦"
                    | "場所"
                    | "開始日時"
                    | "終了日時"
                    | "持ち時間"
                    | "先手持ち時間"
                    | "後手持ち時間"
                    | "最大手数"
                    | "持将棋"
                    | "先手"
                    | "後手"
                    | "手合割"
            ) {
                continue;
            }
            if let Some(value) = metadata.attributes().get(key) {
                lines.push(format!("{key}：{value}"));
            }
        }
    }
}

fn append_kif_comments(lines: &mut Vec<String>, comment: &str) {
    for line in comment.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        lines.push(format!("*{trimmed}"));
    }
}

fn handicap_name_from_sfen(sfen: &str) -> Option<&'static str> {
    InitialPosition::from_sfen(sfen).and_then(InitialPosition::handicap_name_ja)
}

fn handicap_sfen_from_metadata(metadata: &RecordMetadata) -> Option<&'static str> {
    let handicap_name = metadata.attributes().get("手合割")?;
    InitialPosition::from_handicap_name_ja(handicap_name).map(InitialPosition::to_sfen)
}

/// UTF-8 文字列から KIF 形式の棋譜を解析する。
pub fn parse_kif_str(text: &str) -> Result<Record, KifError> {
    let mut lines: Vec<String> = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(|line| line.to_string())
        .collect();
    if let Some(first) = lines.first_mut() {
        *first = first.trim_start_matches('\u{feff}').to_string();
    }

    let mut board_map: BoardMap = HashMap::new();
    let mut hand_counts: HandCounts = HashMap::new();
    ensure_hand_sides(&mut hand_counts);
    let mut initial_turn: Option<char> = None;
    let mut metadata_builder = RecordMetadata::builder();
    let mut initial_comment_lines: Vec<String> = Vec::new();

    let mut idx = 0usize;
    while idx < lines.len() {
        let line = lines[idx].clone();
        let stripped = line.trim();
        if stripped.is_empty() {
            idx += 1;
            continue;
        }
        if stripped.starts_with("変化：")
            || stripped.starts_with("手数")
            || stripped.starts_with("まで")
            || stripped.chars().next().is_some_and(|ch| ch.is_ascii_digit())
        {
            break;
        }
        if stripped.starts_with('*') {
            let comment = stripped.trim_start_matches('*').trim();
            if !comment.is_empty() {
                initial_comment_lines.push(comment.to_string());
            }
            idx += 1;
            continue;
        }
        if stripped == "先手番" {
            initial_turn = Some('+');
            idx += 1;
            continue;
        }
        if stripped == "後手番" {
            initial_turn = Some('-');
            idx += 1;
            continue;
        }
        if stripped.starts_with("先手の持駒") || stripped.starts_with("下手の持駒") {
            let parts: Vec<&str> = stripped.split(['：', ':']).collect();
            if let Some(value) = parts.get(1) {
                hand_counts.insert('+', parse_hand_pieces(value));
            }
            idx += 1;
            continue;
        }
        if stripped.starts_with("後手の持駒") || stripped.starts_with("上手の持駒") {
            let parts: Vec<&str> = stripped.split(['：', ':']).collect();
            if let Some(value) = parts.get(1) {
                hand_counts.insert('-', parse_hand_pieces(value));
            }
            idx += 1;
            continue;
        }
        if is_board_header_line(&line) {
            let (next, parsed) = parse_board_block(&lines, idx)?;
            board_map = parsed;
            idx = next;
            continue;
        }
        apply_metadata_line(&mut metadata_builder, stripped);
        idx += 1;
    }

    let metadata = metadata_builder.build();

    let init_position_sfen = if !board_map.is_empty() {
        let board_sfen = board_map_to_sfen(&board_map).map_err(KifError::InvalidLine)?;
        let hands_sfen = hand_counts_to_sfen(&hand_counts).map_err(KifError::InvalidLine)?;
        let turn = if initial_turn == Some('-') { "w" } else { "b" };
        format!("{board_sfen} {turn} {hands_sfen} 1")
    } else if let Some(handicap_sfen) = handicap_sfen_from_metadata(&metadata) {
        handicap_sfen.to_string()
    } else {
        let mut pos = Position::empty();
        pos.set_hirate();
        pos.to_sfen(None)
    };

    let mut pos = Position::empty();
    pos.set_sfen(&init_position_sfen)?;

    let mut moves: Vec<AnnotatedMoveEntry> = Vec::new();
    let mut last_to: Option<Square> = None;
    let mut terminal: Option<(SpecialMoveEntry, RecordAnnotation)> = None;
    let mut refresh_counter = 0usize;
    let mut initial_comment = if initial_comment_lines.is_empty() {
        None
    } else {
        Some(initial_comment_lines.join("\n"))
    };
    let mut pending_time_ms: Option<u32> = None;
    let mut variation_blocks: Vec<(usize, Vec<String>)> = Vec::new();
    let mut pos_history: Vec<Position> = vec![pos.clone()];
    let mut last_to_history: Vec<Option<Square>> = vec![None];

    while idx < lines.len() {
        let line_raw = lines[idx].clone();
        let line = line_raw.trim();
        idx += 1;

        if line.is_empty() {
            continue;
        }
        if line.starts_with("変化") {
            let start_ply = parse_variation_start(line).max(1);
            let mut block_lines = Vec::new();
            while idx < lines.len() {
                let next_line = lines[idx].trim();
                if next_line.is_empty()
                    || next_line.starts_with("変化")
                    || next_line.starts_with("まで")
                {
                    break;
                }
                block_lines.push(lines[idx].clone());
                idx += 1;
            }
            variation_blocks.push((start_ply, block_lines));
            continue;
        }
        if line.starts_with("まで") {
            if terminal.is_none() {
                terminal = parse_kif_summary_result(line, pos.turn()).map(|record| {
                    (record.with_raw(Some(line.to_string())), RecordAnnotation::new())
                });
            }
            break;
        }
        if line.starts_with('*') {
            let comment = line.trim_start_matches('*').trim();
            if !comment.is_empty() {
                if let Some(last) = moves.last_mut() {
                    append_move_comment(last, comment);
                } else {
                    append_comment_text(&mut initial_comment, comment);
                }
            }
            continue;
        }
        if line.starts_with('\'') {
            let comment = line.trim_start_matches('\'').trim();
            if !comment.is_empty() {
                if let Some(last) = moves.last_mut() {
                    append_move_comment(last, comment);
                } else {
                    append_comment_text(&mut initial_comment, comment);
                }
            }
            continue;
        }
        if line.contains("消費時間") {
            if let Some(time_ms) = parse_time_to_ms(line) {
                if let Some(last) = moves.last_mut() {
                    last.set_time_ms(Some(time_ms));
                } else {
                    pending_time_ms = Some(time_ms);
                }
            }
            continue;
        }
        if line.starts_with("**評価値=") {
            if let Some(last) = moves.last_mut()
                && let Ok(value) = line.trim_start_matches("**評価値=").parse::<i32>()
            {
                last.set_eval(Some(Eval::from_i32(value)));
            }
            continue;
        }
        if line.starts_with("手数") && line.contains("指手") {
            continue;
        }
        if line.chars().next().is_some_and(|ch| ch.is_ascii_digit()) || line.contains("手数") {
            match parse_kif_move(&pos, line, last_to) {
                Ok((mv16, to_sq, inline_time_ms)) => {
                    if !pos.is_legal_move(mv16) {
                        return Err(KifError::IllegalMove { index: moves.len() });
                    }
                    let mut mv_record = AnnotatedMoveEntry::new(mv16);
                    if let Some(time_ms) =
                        resolve_kif_move_time(inline_time_ms, &mut pending_time_ms)
                    {
                        mv_record = mv_record.with_time_ms(Some(time_ms));
                    }
                    pos.apply_move(mv16);
                    moves.push(mv_record);
                    last_to = Some(to_sq);
                    refresh_position_if_needed(&mut pos, &mut refresh_counter)?;
                    pos_history.push(pos.clone());
                    last_to_history.push(last_to);
                    continue;
                }
                Err(_) => {
                    if let Some((record, annotation)) =
                        parse_kif_terminal_record_from_line(line, pos.turn())
                    {
                        terminal = Some((record.with_raw(Some(line.to_string())), annotation));
                        break;
                    }
                    return Err(KifError::InvalidMove(line.to_string()));
                }
            }
        }
        if let Some((record, annotation)) = parse_kif_terminal_record_from_line(line, pos.turn()) {
            terminal = Some((record.with_raw(Some(line.to_string())), annotation));
            break;
        }
        return Err(KifError::InvalidLine(line.to_string()));
    }

    let mut record = Record::from_annotated_main_line(init_position_sfen.clone(), moves, terminal)
        .map_err(|e| KifError::InvalidLine(e.to_string()))?;
    record.set_initial_comment(initial_comment);
    record.set_metadata(metadata);

    if !variation_blocks.is_empty() {
        let mut active_nodes = Vec::with_capacity(record.main_line_ids().len() + 1);
        active_nodes.push(record.root_id());
        active_nodes.extend(record.main_line_ids());
        let mut active_positions = pos_history;
        let mut active_last_to = last_to_history;

        for (start_ply, block_lines) in variation_blocks {
            if start_ply == 0 {
                return Err(KifError::InvalidLine("variation start out of range: 0".to_string()));
            }
            let parent_ply = start_ply - 1;
            let parent = *active_nodes.get(parent_ply).ok_or_else(|| {
                KifError::InvalidLine(format!("variation start out of range: {start_ply}"))
            })?;
            let pos_start = active_positions.get(parent_ply).cloned().ok_or_else(|| {
                KifError::InvalidLine(format!("variation start out of range: {start_ply}"))
            })?;
            let last_to_start = active_last_to.get(parent_ply).copied().unwrap_or(None);
            let variation_moves =
                parse_kif_variation_moves(&block_lines, &pos_start, last_to_start)?;
            let created = record
                .add_variation_line_with_annotations(parent, variation_moves.clone())
                .map_err(|e| KifError::InvalidLine(e.to_string()))?;

            active_nodes.truncate(start_ply);
            active_nodes.extend(created);
            active_positions.truncate(start_ply);
            active_last_to.truncate(start_ply);

            let mut var_pos = pos_start;
            for mv_record in variation_moves {
                let mv16 = mv_record.mv();
                if !var_pos.is_legal_move(mv16) {
                    return Err(KifError::InvalidMove(mv16.to_usi()));
                }
                var_pos.apply_move(mv16);
                active_positions.push(var_pos.clone());
                active_last_to.push(Some(mv16.to_sq()));
            }
        }
    }

    Ok(record)
}

/// UTF-8 文字列から KI2 形式の棋譜を解析する。
pub fn parse_ki2_str(text: &str) -> Result<Record, Ki2Error> {
    let mut lines: Vec<String> = text
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split('\n')
        .map(|line| line.to_string())
        .collect();
    if let Some(first) = lines.first_mut() {
        *first = first.trim_start_matches('\u{feff}').to_string();
    }

    let mut board_map: BoardMap = HashMap::new();
    let mut hand_counts: HandCounts = HashMap::new();
    ensure_hand_sides(&mut hand_counts);
    let mut initial_turn: Option<char> = None;
    let mut metadata_builder = RecordMetadata::builder();
    let mut initial_comment_lines: Vec<String> = Vec::new();

    let mut idx = 0usize;
    while idx < lines.len() {
        let line = lines[idx].clone();
        let stripped = line.trim();
        if stripped.is_empty() {
            idx += 1;
            continue;
        }
        if stripped.starts_with("変化：")
            || stripped.starts_with("まで")
            || looks_like_ki2_move_line(stripped)
        {
            break;
        }
        if stripped.starts_with('*') {
            let comment = stripped.trim_start_matches('*').trim();
            if !comment.is_empty() {
                initial_comment_lines.push(comment.to_string());
            }
            idx += 1;
            continue;
        }
        if stripped == "先手番" {
            initial_turn = Some('+');
            idx += 1;
            continue;
        }
        if stripped == "後手番" {
            initial_turn = Some('-');
            idx += 1;
            continue;
        }
        if stripped.starts_with("先手の持駒") || stripped.starts_with("下手の持駒") {
            let parts: Vec<&str> = stripped.split(['：', ':']).collect();
            if let Some(value) = parts.get(1) {
                hand_counts.insert('+', parse_hand_pieces(value));
            }
            idx += 1;
            continue;
        }
        if stripped.starts_with("後手の持駒") || stripped.starts_with("上手の持駒") {
            let parts: Vec<&str> = stripped.split(['：', ':']).collect();
            if let Some(value) = parts.get(1) {
                hand_counts.insert('-', parse_hand_pieces(value));
            }
            idx += 1;
            continue;
        }
        if is_board_header_line(&line) {
            let (next, parsed) = parse_board_block(&lines, idx)
                .map_err(|err| Ki2Error::InvalidLine(err.to_string()))?;
            board_map = parsed;
            idx = next;
            continue;
        }
        apply_metadata_line(&mut metadata_builder, stripped);
        idx += 1;
    }

    let metadata = metadata_builder.build();

    let init_position_sfen = if !board_map.is_empty() {
        let board_sfen = board_map_to_sfen(&board_map).map_err(Ki2Error::InvalidLine)?;
        let hands_sfen = hand_counts_to_sfen(&hand_counts).map_err(Ki2Error::InvalidLine)?;
        let turn = if initial_turn == Some('-') { "w" } else { "b" };
        format!("{board_sfen} {turn} {hands_sfen} 1")
    } else if let Some(handicap_sfen) = handicap_sfen_from_metadata(&metadata) {
        handicap_sfen.to_string()
    } else {
        let mut pos = Position::empty();
        pos.set_hirate();
        pos.to_sfen(None)
    };

    let mut pos = Position::empty();
    pos.set_sfen(&init_position_sfen)?;

    let mut moves: Vec<AnnotatedMoveEntry> = Vec::new();
    let mut last_to: Option<Square> = None;
    let mut terminal: Option<(SpecialMoveEntry, RecordAnnotation)> = None;
    let mut refresh_counter = 0usize;
    let mut initial_comment = if initial_comment_lines.is_empty() {
        None
    } else {
        Some(initial_comment_lines.join("\n"))
    };
    let mut pending_time_ms: Option<u32> = None;
    let mut variation_blocks: Vec<(usize, Vec<String>)> = Vec::new();
    let mut pos_history: Vec<Position> = vec![pos.clone()];
    let mut last_to_history: Vec<Option<Square>> = vec![None];

    while idx < lines.len() {
        let line_raw = lines[idx].clone();
        let line = line_raw.trim();
        idx += 1;

        if line.is_empty() {
            continue;
        }
        if line.starts_with("変化") {
            let start_ply = parse_variation_start(line).max(1);
            let mut block_lines = Vec::new();
            while idx < lines.len() {
                let next_line = lines[idx].trim();
                if next_line.is_empty()
                    || next_line.starts_with("変化")
                    || next_line.starts_with("まで")
                {
                    break;
                }
                block_lines.push(lines[idx].clone());
                idx += 1;
            }
            variation_blocks.push((start_ply, block_lines));
            continue;
        }
        if line.starts_with("まで") {
            if terminal.is_none() {
                terminal = parse_ki2_summary_result(line, pos.turn()).map(|record| {
                    (record.with_raw(Some(line.to_string())), RecordAnnotation::new())
                });
            }
            break;
        }
        if line.starts_with("**") || line.starts_with('*') {
            let comment = line.trim_start_matches('*').trim();
            if !comment.is_empty() {
                if let Some(last) = moves.last_mut() {
                    append_move_comment(last, comment);
                } else {
                    append_comment_text(&mut initial_comment, comment);
                }
            }
            continue;
        }
        if line.starts_with('\'') {
            let comment = line.trim_start_matches('\'').trim();
            if !comment.is_empty() {
                if let Some(last) = moves.last_mut() {
                    append_move_comment(last, comment);
                } else {
                    append_comment_text(&mut initial_comment, comment);
                }
            }
            continue;
        }
        if line.contains("消費時間") {
            if let Some(time_ms) = parse_time_to_ms(line) {
                if let Some(last) = moves.last_mut() {
                    last.set_time_ms(Some(time_ms));
                } else {
                    pending_time_ms = Some(time_ms);
                }
            }
            continue;
        }
        if line.starts_with("手数") {
            continue;
        }

        let parsed = parse_ki2_sections(&pos, line, last_to)?;
        for (mv16, to_sq) in parsed {
            if !pos.is_legal_move(mv16) {
                return Err(Ki2Error::IllegalMove { index: moves.len() });
            }
            let mut mv_record = AnnotatedMoveEntry::new(mv16);
            if let Some(time_ms) = pending_time_ms.take() {
                mv_record = mv_record.with_time_ms(Some(time_ms));
            }
            pos.apply_move(mv16);
            moves.push(mv_record);
            last_to = Some(to_sq);
            refresh_position_if_needed(&mut pos, &mut refresh_counter)?;
            pos_history.push(pos.clone());
            last_to_history.push(last_to);
        }
    }

    let terminal = terminal.unwrap_or_else(|| {
        (SpecialMoveEntry::new(SpecialMove::Interrupt, GameResult::Paused), RecordAnnotation::new())
    });
    let mut record =
        Record::from_annotated_main_line(init_position_sfen.clone(), moves, Some(terminal))
            .map_err(|e| Ki2Error::InvalidLine(e.to_string()))?;
    record.set_initial_comment(initial_comment);
    record.set_metadata(metadata);

    if !variation_blocks.is_empty() {
        let mut active_nodes = Vec::with_capacity(record.main_line_ids().len() + 1);
        active_nodes.push(record.root_id());
        active_nodes.extend(record.main_line_ids());
        let mut active_positions = pos_history;
        let mut active_last_to = last_to_history;

        for (start_ply, block_lines) in variation_blocks {
            if start_ply == 0 {
                return Err(Ki2Error::InvalidLine("variation start out of range: 0".to_string()));
            }
            let parent_ply = start_ply - 1;
            let parent = *active_nodes.get(parent_ply).ok_or_else(|| {
                Ki2Error::InvalidLine(format!("variation start out of range: {start_ply}"))
            })?;
            let pos_start = active_positions.get(parent_ply).cloned().ok_or_else(|| {
                Ki2Error::InvalidLine(format!("variation start out of range: {start_ply}"))
            })?;
            let last_to_start = active_last_to.get(parent_ply).copied().unwrap_or(None);
            let variation_moves =
                parse_ki2_variation_moves(&block_lines, &pos_start, last_to_start)?;
            let created = record
                .add_variation_line_with_annotations(parent, variation_moves.clone())
                .map_err(|e| Ki2Error::InvalidLine(e.to_string()))?;

            active_nodes.truncate(start_ply);
            active_nodes.extend(created);
            active_positions.truncate(start_ply);
            active_last_to.truncate(start_ply);

            let mut var_pos = pos_start;
            for mv_record in variation_moves {
                let mv16 = mv_record.mv();
                if !var_pos.is_legal_move(mv16) {
                    return Err(Ki2Error::InvalidMove(mv16.to_usi()));
                }
                var_pos.apply_move(mv16);
                active_positions.push(var_pos.clone());
                active_last_to.push(Some(mv16.to_sq()));
            }
        }
    }

    Ok(record)
}

/// [`Record`] を KIF 形式の文字列に変換する。
pub fn export_kif(record: &Record) -> Result<String, KifError> {
    let mut pos = Position::empty();
    pos.set_sfen(record.init_position_sfen())?;

    let mut lines: Vec<String> = Vec::new();
    append_kif_metadata(&mut lines, record.metadata());
    if let Some(handicap) = handicap_name_from_sfen(record.init_position_sfen()) {
        lines.push(format!("手合割：{handicap}"));
    } else {
        lines.extend(render_bod(&pos));
    }

    lines.push("手数----指手---------消費時間--".to_string());
    if let Some(comment) = record.initial_comment() {
        append_kif_comments(&mut lines, comment);
    }

    let mut last_to: Option<Square> = None;
    let mut refresh_counter = 0usize;
    let mut main_positions: Vec<Position> = vec![pos.clone()];
    let mut last_to_history: Vec<Option<Square>> = vec![None];
    let mut main_time_totals: Vec<(u32, u32)> = vec![(0, 0)];
    let mut black_total_ms: u32 = 0;
    let mut white_total_ms: u32 = 0;
    let main_ids = record.main_line_ids();
    for (index, node_id) in main_ids.iter().enumerate() {
        let node = record.node(*node_id);
        let mv_record =
            node.mv().ok_or_else(|| KifError::InvalidLine("main node missing move".to_string()))?;
        let mv16 = mv_record.mv();
        if !pos.is_legal_move(mv16) {
            return Err(KifError::IllegalMove { index });
        }
        let mv = pos.move32_from_move(mv16);
        let (move_text, to_sq) = move_to_kif_text(&pos, mv, last_to)?;
        let elapsed = node.time_ms().unwrap_or(0);
        if pos.turn() == Color::BLACK {
            black_total_ms = black_total_ms.saturating_add(elapsed);
        } else {
            white_total_ms = white_total_ms.saturating_add(elapsed);
        }
        let total = if pos.turn() == Color::BLACK { black_total_ms } else { white_total_ms };
        let line = format_kif_move_line(
            index + 1,
            &move_text,
            elapsed,
            total,
            record.children(*node_id).len() > 1,
        );
        lines.push(line);
        append_kif_eval_line(&mut lines, node.annotation());
        if let Some(comment) = node.comment() {
            append_kif_comments(&mut lines, comment);
        }
        pos.apply_move(mv16);
        last_to = Some(to_sq);
        refresh_position_if_needed(&mut pos, &mut refresh_counter)?;
        main_positions.push(pos.clone());
        last_to_history.push(last_to);
        main_time_totals.push((black_total_ms, white_total_ms));
    }

    let mut parent_nodes: Vec<(crate::records::record::RecordNodeId, usize)> =
        Vec::with_capacity(main_ids.len() + 1);
    parent_nodes.push((record.root_id(), 0));
    for (idx, node_id) in main_ids.iter().enumerate() {
        parent_nodes.push((*node_id, idx + 1));
    }

    for (parent_id, parent_ply) in parent_nodes {
        let children = record.children(parent_id);
        if children.len() <= 1 {
            continue;
        }
        let start_ply = parent_ply + 1;
        let pos_start = main_positions
            .get(parent_ply)
            .ok_or_else(|| KifError::InvalidLine("variation start out of range".to_string()))?;
        let last_to_start = last_to_history.get(parent_ply).copied().unwrap_or(None);
        let (mut black_total, mut white_total) =
            main_time_totals.get(parent_ply).copied().unwrap_or((0, 0));
        for child_id in children.iter().skip(1) {
            lines.push(String::new());
            lines.push(format!("変化：{start_ply}手"));
            let mut var_pos = pos_start.clone();
            let mut var_last_to = last_to_start;
            let mut move_no = start_ply;
            let mut current = Some(*child_id);
            while let Some(node_id) = current {
                let node = record.node(node_id);
                let mv_record = node.mv().ok_or_else(|| {
                    KifError::InvalidLine("variation node missing move".to_string())
                })?;
                let mv16 = mv_record.mv();
                if !var_pos.is_legal_move(mv16) {
                    return Err(KifError::InvalidMove(format!(
                        "illegal variation move at ply {move_no}"
                    )));
                }
                let mv = var_pos.move32_from_move(mv16);
                let (move_text, to_sq) = move_to_kif_text(&var_pos, mv, var_last_to)?;
                let elapsed = node.time_ms().unwrap_or(0);
                if var_pos.turn() == Color::BLACK {
                    black_total = black_total.saturating_add(elapsed);
                } else {
                    white_total = white_total.saturating_add(elapsed);
                }
                let total = if var_pos.turn() == Color::BLACK { black_total } else { white_total };
                let line = format_kif_move_line(
                    move_no,
                    &move_text,
                    elapsed,
                    total,
                    record.children(node_id).len() > 1,
                );
                lines.push(line);
                append_kif_eval_line(&mut lines, node.annotation());
                if let Some(comment) = node.comment() {
                    append_kif_comments(&mut lines, comment);
                }
                var_pos.apply_move(mv16);
                var_last_to = Some(to_sq);
                move_no += 1;
                current = record.children(node_id).first().copied();
            }
        }
    }

    if let Some(terminal_id) = record.main_terminal_node() {
        let terminal_node = record.node(terminal_id);
        let terminal = terminal_node.special().expect("terminal node has special entry");
        if record.node_count() > main_ids.len() + 1 {
            lines.push(String::new());
        }
        let terminal_elapsed = terminal_node.time_ms().unwrap_or(0);
        if pos.turn() == Color::BLACK {
            black_total_ms = black_total_ms.saturating_add(terminal_elapsed);
        } else {
            white_total_ms = white_total_ms.saturating_add(terminal_elapsed);
        }
        let terminal_total =
            if pos.turn() == Color::BLACK { black_total_ms } else { white_total_ms };
        lines.push(format_kif_move_line(
            record.move_count() + 1,
            &special_move_to_kif_text(terminal),
            terminal_elapsed,
            terminal_total,
            false,
        ));
        lines.push(format_kif_summary(terminal, pos.turn(), record.move_count()));
    }

    Ok(format!("{}\n", lines.join("\n")))
}

/// [`Record`] を KI2 形式の文字列に変換する。
pub fn export_ki2(record: &Record) -> Result<String, Ki2Error> {
    let mut pos = Position::empty();
    pos.set_sfen(record.init_position_sfen())?;

    let mut lines: Vec<String> = Vec::new();
    append_kif_metadata(&mut lines, record.metadata());
    if let Some(handicap) = handicap_name_from_sfen(record.init_position_sfen()) {
        lines.push(format!("手合割：{handicap}"));
    } else {
        lines.extend(render_bod(&pos));
    }
    if let Some(comment) = record.initial_comment() {
        append_kif_comments(&mut lines, comment);
    }

    let mut current_line = String::new();
    let mut move_count_in_line = 0usize;
    let mut last_move_len = 0usize;
    let mut refresh_counter = 0usize;
    let mut main_positions: Vec<Position> = vec![pos.clone()];
    let main_ids = record.main_line_ids();
    for (index, node_id) in main_ids.iter().enumerate() {
        let node = record.node(*node_id);
        let mv_record =
            node.mv().ok_or_else(|| Ki2Error::InvalidLine("main node missing move".to_string()))?;
        let mv16 = mv_record.mv();
        if !pos.is_legal_move(mv16) {
            return Err(Ki2Error::IllegalMove { index });
        }
        let mv = pos.move32_from_move(mv16);
        let ki2 = mv
            .to_ki2(&pos)
            .ok_or_else(|| Ki2Error::InvalidMove(format!("invalid move at index {index}")))?;
        if current_line.is_empty() {
            current_line.push_str(&ki2);
            move_count_in_line = 1;
        } else {
            let spaces = 12usize.saturating_sub(last_move_len.saturating_mul(2));
            if spaces > 0 {
                current_line.push_str(&" ".repeat(spaces));
            }
            current_line.push_str(&ki2);
            move_count_in_line += 1;
        }
        last_move_len = ki2.chars().count();
        if move_count_in_line >= 6 {
            lines.push(current_line);
            current_line = String::new();
            move_count_in_line = 0;
            last_move_len = 0;
        }
        if (node.eval().is_some() || node.comment().is_some()) && !current_line.is_empty() {
            lines.push(current_line);
            current_line = String::new();
            move_count_in_line = 0;
            last_move_len = 0;
        }
        append_kif_eval_line(&mut lines, node.annotation());
        if let Some(comment) = node.comment() {
            append_kif_comments(&mut lines, comment);
        }
        pos.apply_move(mv16);
        refresh_position_if_needed(&mut pos, &mut refresh_counter)?;
        main_positions.push(pos.clone());
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }
    let mut parent_nodes: Vec<(crate::records::record::RecordNodeId, usize)> =
        Vec::with_capacity(main_ids.len() + 1);
    parent_nodes.push((record.root_id(), 0));
    for (idx, node_id) in main_ids.iter().enumerate() {
        parent_nodes.push((*node_id, idx + 1));
    }

    for (parent_id, parent_ply) in parent_nodes {
        let children = record.children(parent_id);
        if children.len() <= 1 {
            continue;
        }
        let start_ply = parent_ply + 1;
        let pos_start = main_positions
            .get(parent_ply)
            .ok_or_else(|| Ki2Error::InvalidLine("variation start out of range".to_string()))?;
        for child_id in children.iter().skip(1) {
            lines.push(String::new());
            lines.push(format!("変化：{start_ply}手"));
            let mut var_pos = pos_start.clone();
            let mut move_no = start_ply;
            let mut var_line = String::new();
            let mut var_count = 0usize;
            let mut var_last_len = 0usize;
            let mut current = Some(*child_id);
            while let Some(node_id) = current {
                let node = record.node(node_id);
                let mv_record = node.mv().ok_or_else(|| {
                    Ki2Error::InvalidLine("variation node missing move".to_string())
                })?;
                let mv16 = mv_record.mv();
                if !var_pos.is_legal_move(mv16) {
                    return Err(Ki2Error::InvalidMove(format!(
                        "illegal variation move at ply {move_no}"
                    )));
                }
                let mv = var_pos.move32_from_move(mv16);
                let ki2 = mv.to_ki2(&var_pos).ok_or_else(|| {
                    Ki2Error::InvalidMove(format!("invalid variation move at ply {move_no}"))
                })?;
                if var_line.is_empty() {
                    var_line.push_str(&ki2);
                    var_count = 1;
                } else {
                    let spaces = 12usize.saturating_sub(var_last_len.saturating_mul(2));
                    if spaces > 0 {
                        var_line.push_str(&" ".repeat(spaces));
                    }
                    var_line.push_str(&ki2);
                    var_count += 1;
                }
                var_last_len = ki2.chars().count();
                if var_count >= 6 {
                    lines.push(var_line);
                    var_line = String::new();
                    var_count = 0;
                    var_last_len = 0;
                }
                if (node.eval().is_some() || node.comment().is_some()) && !var_line.is_empty() {
                    lines.push(var_line);
                    var_line = String::new();
                    var_count = 0;
                    var_last_len = 0;
                }
                append_kif_eval_line(&mut lines, node.annotation());
                if let Some(comment) = node.comment() {
                    append_kif_comments(&mut lines, comment);
                }
                var_pos.apply_move(mv16);
                move_no += 1;
                current = record.children(node_id).first().copied();
            }
            if !var_line.is_empty() {
                lines.push(var_line);
            }
        }
    }

    if let Some(terminal) = record.main_terminal() {
        if record.node_count() > main_ids.len() + 1 {
            lines.push(String::new());
        }
        lines.push(format_kif_summary(terminal, pos.turn(), record.move_count()));
    }

    Ok(format!("{}\n", lines.join("\n")))
}

fn render_bod(pos: &Position) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!("後手の持駒：{}", format_hand_text(pos.hand(Color::WHITE))));
    lines.push("  ９ ８ ７ ６ ５ ４ ３ ２ １".to_string());
    lines.push("+---------------------------+".to_string());
    for rank in 1..=9u8 {
        let mut row = String::from("|");
        for file in (1..=9u8).rev() {
            let file_char = char::from(b'0' + file);
            let rank_char = char::from(b'a' + (rank - 1));
            let sq = Square::from_usi(&format!("{file_char}{rank_char}")).expect("valid square");
            let piece = pos.piece_on(sq);
            if piece == Piece::NONE {
                row.push(' ');
                row.push('・');
                continue;
            }
            if piece.color() == Color::WHITE {
                row.push('v');
                row.push_str(piece_type_to_kif_board(piece.piece_type()));
            } else {
                row.push(' ');
                row.push_str(piece_type_to_kif_board(piece.piece_type()));
            }
        }
        row.push('|');
        row.push_str(&kanji_number(rank));
        lines.push(row);
    }
    lines.push("+---------------------------+".to_string());
    lines.push(format!("先手の持駒：{}", format_hand_text(pos.hand(Color::BLACK))));
    if pos.turn() == Color::BLACK {
        lines.push("先手番".to_string());
    } else {
        lines.push("後手番".to_string());
    }
    lines
}

/// cshogi互換のBOD形式で盤面を出力する。
#[must_use]
pub fn board_to_bod(pos: &Position) -> String {
    let mut lines = Vec::new();
    lines.push(format!("後手の持駒：{}", format_hand_text_bod(pos.hand(Color::WHITE))));
    lines.push("  ９ ８ ７ ６ ５ ４ ３ ２ １".to_string());
    lines.push("+---------------------------+".to_string());
    for rank in 1..=9u8 {
        let mut row = String::from("|");
        for file in (1..=9u8).rev() {
            let file_char = char::from(b'0' + file);
            let rank_char = char::from(b'a' + (rank - 1));
            let sq = Square::from_usi(&format!("{file_char}{rank_char}")).expect("valid square");
            let piece = pos.piece_on(sq);
            if piece == Piece::NONE {
                row.push_str(" ・");
                continue;
            }
            if piece.color() == Color::WHITE {
                row.push('v');
                row.push_str(piece_type_to_kif_board(piece.piece_type()));
            } else {
                row.push(' ');
                row.push_str(piece_type_to_kif_board(piece.piece_type()));
            }
        }
        row.push('|');
        row.push_str(&kanji_number(rank));
        lines.push(row);
    }
    lines.push("+---------------------------+".to_string());
    lines.push(format!("先手の持駒：{}", format_hand_text_bod(pos.hand(Color::BLACK))));
    if pos.turn() == Color::WHITE {
        lines.push("後手番".to_string());
    }
    lines.join("\n")
}

/// cshogi互換のBOD手表示に変換する。
#[must_use]
pub fn move_to_bod(pos: &Position, mv: Move32) -> Option<String> {
    let ki2 = mv.to_ki2(pos)?;
    let mut iter = ki2.chars();
    let turn = iter.next()?;
    let second = iter.next()?;
    if second == '同' {
        let _ = iter.next();
        let suffix: String = iter.collect();
        let square = kifu_square_name(mv.to_sq())?;
        return Some(format!("{turn}{square}同{suffix}"));
    }
    Some(ki2)
}

fn kifu_square_name(sq: Square) -> Option<String> {
    if !sq.is_on_board() {
        return None;
    }
    let file = sq.file().raw() + 1;
    let rank = sq.rank().raw() + 1;
    if !(1..=9).contains(&file) || !(1..=9).contains(&rank) {
        return None;
    }
    let file_char = WIDE_DIGITS[(file - 1) as usize];
    let rank_char = KANJI_RANKS[(rank - 1) as usize];
    Some(format!("{file_char}{rank_char}"))
}

fn format_hand_text_bod(hand: Hand) -> String {
    let mut parts = Vec::new();
    for (piece_type, name) in [
        (PieceType::ROOK, "飛"),
        (PieceType::BISHOP, "角"),
        (PieceType::GOLD, "金"),
        (PieceType::SILVER, "銀"),
        (PieceType::KNIGHT, "桂"),
        (PieceType::LANCE, "香"),
        (PieceType::PAWN, "歩"),
    ] {
        let count = HandPiece::from_piece_type(piece_type).map(|hp| hand.count(hp)).unwrap_or(0);
        if count == 0 {
            continue;
        }
        if count == 1 {
            parts.push(name.to_string());
        } else {
            parts.push(format!("{name}{}", kanji_number(count as u8)));
        }
    }
    if parts.is_empty() { "なし".to_string() } else { parts.join("　") }
}

fn format_hand_text(hand: crate::types::Hand) -> String {
    let mut parts = Vec::new();
    for (piece_type, name) in [
        (PieceType::PAWN, "歩"),
        (PieceType::LANCE, "香"),
        (PieceType::KNIGHT, "桂"),
        (PieceType::SILVER, "銀"),
        (PieceType::GOLD, "金"),
        (PieceType::BISHOP, "角"),
        (PieceType::ROOK, "飛"),
    ] {
        let count = crate::types::HandPiece::from_piece_type(piece_type)
            .map(|hp| hand.count(hp))
            .unwrap_or(0);
        if count == 0 {
            continue;
        }
        if count == 1 {
            parts.push(name.to_string());
        } else {
            parts.push(format!("{name}{}", kanji_number(count as u8)));
        }
    }
    if parts.is_empty() { "なし".to_string() } else { parts.join("　") }
}

fn kanji_number(value: u8) -> String {
    match value {
        1 => "一".to_string(),
        2 => "二".to_string(),
        3 => "三".to_string(),
        4 => "四".to_string(),
        5 => "五".to_string(),
        6 => "六".to_string(),
        7 => "七".to_string(),
        8 => "八".to_string(),
        9 => "九".to_string(),
        10 => "十".to_string(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{InitialPosition, hirate_position};
    use crate::records::formats::common::{ExportOptions, TextEncoding};
    use crate::records::record::{
        EngineInfo, MoveEntry, RecordAnnotation, RecordMetadata, RecordNode, SpecialMove,
    };

    fn main_node(record: &Record, index: usize) -> &RecordNode {
        let node_id = record.main_line_ids()[index];
        record.node(node_id)
    }

    fn terminal_node(record: &Record) -> &RecordNode {
        record.node(record.main_terminal_node().expect("terminal node"))
    }

    fn annotation_with_eval(eval: i32) -> RecordAnnotation {
        RecordAnnotation::new()
            .with_engine_info(Some(EngineInfo::new().with_eval(Some(Eval::from_i32(eval)))))
    }

    #[test]
    fn test_parse_kif_basic() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77)
 2 投了
まで1手で先手の勝ち";

        let record = parse_kif_str(kif).unwrap();
        assert_eq!(record.main_moves().count(), 1);
        assert_eq!(record.result(), GameResult::BlackWin);
    }

    #[test]
    fn test_parse_kif_uses_handicap_initial_position() {
        let kif = "\
手合割：三枚落ち
まで0手で中断";

        let record = parse_kif_str(kif).unwrap();
        assert_eq!(record.init_position_sfen(), InitialPosition::Handicap3Pieces.to_sfen());
    }

    #[test]
    fn test_parse_kif_summary_max_moves() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77)
 2 ３四歩(33)
まで2手で最大手数";

        let record = parse_kif_str(kif).unwrap();
        assert_eq!(record.result(), GameResult::DrawByMaxPlies);
        let terminal = record.main_terminal().expect("terminal");
        assert_eq!(terminal.kind(), &SpecialMove::MaxMoves);
    }

    #[test]
    fn test_parse_kif_summary_impasse() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77)
 2 ３四歩(33)
まで2手で持将棋";

        let record = parse_kif_str(kif).unwrap();
        assert_eq!(record.result(), GameResult::DrawByImpasse);
        let terminal = record.main_terminal().expect("terminal");
        assert_eq!(terminal.kind(), &SpecialMove::Impasse);
    }

    #[test]
    fn test_parse_kif_terminal_line_with_time() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77)
 2 ３四歩(33)
 3 投了 ( 0:03/00:00:04)";

        let record = parse_kif_str(kif).unwrap();
        let terminal = record.main_terminal().expect("terminal");
        assert_eq!(terminal.kind(), &SpecialMove::Resign);
        assert_eq!(terminal.result(), GameResult::WhiteWin);
        assert_eq!(terminal_node(&record).time_ms(), Some(3_000));
    }

    #[test]
    fn test_parse_kif_terminal_line_with_time_compact_format() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77)
 2 ３四歩(33)
 3 投了   (0:11/0:22:44)";

        let record = parse_kif_str(kif).unwrap();
        let terminal = record.main_terminal().expect("terminal");
        assert_eq!(terminal.kind(), &SpecialMove::Resign);
        assert_eq!(terminal.result(), GameResult::WhiteWin);
        assert_eq!(terminal_node(&record).time_ms(), Some(11_000));
    }

    #[test]
    fn test_parse_kif_move_line_with_inline_time() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77) (0:27/00:00:27)
 2 ３四歩(33) (0:13/00:00:13)
 3 投了";

        let record = parse_kif_str(kif).unwrap();
        assert_eq!(main_node(&record, 0).time_ms(), Some(27_000));
        assert_eq!(main_node(&record, 1).time_ms(), Some(13_000));
    }

    #[test]
    fn test_parse_kif_preserves_multiline_comments_on_move() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77)
*コメント1
*コメント2
 2 ３四歩(33)
 3 投了";

        let record = parse_kif_str(kif).unwrap();
        assert_eq!(main_node(&record, 0).comment(), Some("コメント1\nコメント2"));
    }

    #[test]
    fn test_parse_kif_preserves_multiline_comments_before_first_move() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
*序文1
*序文2
 1 ７六歩(77)
 2 投了";

        let record = parse_kif_str(kif).unwrap();
        assert_eq!(record.initial_comment(), Some("序文1\n序文2"));
        assert_eq!(main_node(&record, 0).comment(), None);
    }

    #[test]
    fn test_parse_kif_maps_note_metadata_to_metadata_comment() {
        let kif = "\
備考：備考1
備考：備考2
手数----指手---------消費時間--
 1 ７六歩(77)
";

        let record = parse_kif_str(kif).unwrap();
        assert_eq!(record.metadata().comment(), Some("備考1\n備考2"));
        assert_eq!(record.initial_comment(), None);
    }

    #[test]
    fn test_export_kif_writes_initial_comment_before_first_move() {
        let pos = hirate_position();
        let mut record = Record::from_main_line(
            pos.to_sfen(None),
            vec![MoveEntry::new(Move::from_usi("7g7f").unwrap())],
            None,
        )
        .unwrap();
        record.set_initial_comment(Some("序文1\n序文2".to_string()));

        let kif = export_kif(&record).unwrap();
        let expected = "手数----指手---------消費時間--\n*序文1\n*序文2\n   1 ７六歩(77)";
        assert!(kif.contains(expected));
    }

    #[test]
    fn test_export_kif_writes_metadata_comment_as_note() {
        let pos = hirate_position();
        let mut record = Record::from_main_line(
            pos.to_sfen(None),
            vec![MoveEntry::new(Move::from_usi("7g7f").unwrap())],
            None,
        )
        .unwrap();
        let mut metadata_builder = RecordMetadata::builder();
        metadata_builder.comment(Some("備考1\n備考2".to_string()));
        record.set_metadata(metadata_builder.build());
        record.set_initial_comment(Some("序文".to_string()));

        let kif = export_kif(&record).unwrap();
        assert!(kif.contains("備考：備考1\n備考：備考2"));
        assert!(kif.contains("手数----指手---------消費時間--\n*序文"));
        assert!(!kif.contains("*備考1"));
    }

    #[test]
    fn test_parse_kif_inline_time_discards_pending_standalone_time() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
消費時間：0:05
 1 ７六歩(77) (0:27/00:00:27)
 2 ３四歩(33)
 3 投了";

        let record = parse_kif_str(kif).unwrap();
        assert_eq!(main_node(&record, 0).time_ms(), Some(27_000));
        assert_eq!(main_node(&record, 1).time_ms(), None);
    }

    #[test]
    fn test_parse_kif_variation_move_line_with_inline_time() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77)
 2 ３四歩(33)

変化：1手
 1 ２六歩(27) (0:11/00:00:11)
 2 ８四歩(83)";

        let record = parse_kif_str(kif).unwrap();
        let root_children = record.children(record.root_id());
        assert_eq!(root_children.len(), 2);
        let variation_node = record.node(root_children[1]);
        let variation = variation_node.mv().expect("variation move");
        assert_eq!(variation.mv().to_usi(), "2g2f");
        assert_eq!(variation_node.time_ms(), Some(11_000));
    }

    #[test]
    fn v1_parse_kif_nested_variation_attaches_to_variation_parent() {
        let kif = "\
棋戦：nested variation fixture
手数----指手---------消費時間--
 1 ７六歩(77)
 2 ３四歩(33)

変化：1手
 1 ２六歩(27)

変化：2手
 2 ８四歩(83)";

        let record = parse_kif_str(kif).unwrap();
        let root_children = record.children(record.root_id());
        assert_eq!(root_children.len(), 2);

        let variation = root_children[1];
        assert_eq!(record.node(variation).mv().expect("variation move").mv().to_usi(), "2g2f");

        let variation_children = record.children(variation);
        assert_eq!(variation_children.len(), 1);
        assert_eq!(
            record.node(variation_children[0]).mv().expect("nested variation move").mv().to_usi(),
            "8c8d"
        );
    }

    #[test]
    fn test_parse_kif_without_terminal_line() {
        let kif = "\
手合割：平手
先手：
後手：
手数----指手---------消費時間--
   1 ２六歩(27)        ( 0:00/00:00:00)
   2 ３四歩(33)        ( 0:00/00:00:00)
   3 ７六歩(77)        ( 0:00/00:00:00)
   4 ４四歩(43)        ( 0:00/00:00:00)";

        let record = parse_kif_str(kif).unwrap();
        assert_eq!(record.move_count(), 4);
        assert!(record.main_terminal().is_none());
        assert_eq!(record.result(), GameResult::Invalid);
    }

    #[test]
    fn test_parse_kif_rejects_terminal_line_without_move_number() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77)
投了 ( 0:03/00:00:04)";

        let err = parse_kif_str(kif).unwrap_err();
        assert!(matches!(err, KifError::InvalidLine(_)));
    }

    #[test]
    fn test_parse_ki2_basic() {
        let ki2 = "\
先手：Alice
後手：Bob

▲７六歩
△３四歩
まで2手で先手の勝ち
";
        let record = parse_ki2_str(ki2).unwrap();
        assert_eq!(record.main_moves().count(), 2);
        assert_eq!(record.moves()[0].mv().to_usi(), "7g7f");
        assert_eq!(record.moves()[1].mv().to_usi(), "3c3d");
        assert_eq!(record.moves().last().unwrap().mv().to_usi(), "3c3d");
    }

    #[test]
    fn test_parse_ki2_preserves_initial_comment_before_first_move() {
        let ki2 = "\
*序文1
*序文2
▲７六歩
△３四歩
まで2手で先手の勝ち
";

        let record = parse_ki2_str(ki2).unwrap();
        assert_eq!(record.initial_comment(), Some("序文1\n序文2"));
        assert_eq!(main_node(&record, 0).comment(), None);
    }

    #[test]
    fn test_parse_ki2_maps_note_metadata_to_metadata_comment() {
        let ki2 = "\
備考：備考1
備考：備考2
▲７六歩
";

        let record = parse_ki2_str(ki2).unwrap();
        assert_eq!(record.metadata().comment(), Some("備考1\n備考2"));
    }

    #[test]
    fn test_parse_ki2_uses_handicap_initial_position() {
        let ki2 = "\
手合割：左五枚落ち
まで0手で中断
";

        let record = parse_ki2_str(ki2).unwrap();
        assert_eq!(record.init_position_sfen(), InitialPosition::HandicapLeft5Pieces.to_sfen());
    }

    #[test]
    fn test_parse_ki2_multi_move_line() {
        let ki2 = "\
開始日時：1582/06/02 04:00:00
先手：織田信長
後手：明智光秀

▲２六歩 △３四歩 ▲７六歩 △５四歩
まで4手で先手の勝ち
";
        let record = parse_ki2_str(ki2).unwrap();
        assert_eq!(record.main_moves().count(), 4);
        assert_eq!(record.moves()[0].mv().to_usi(), "2g2f");
        assert_eq!(record.moves()[1].mv().to_usi(), "3c3d");
        assert_eq!(record.moves()[2].mv().to_usi(), "7g7f");
        assert_eq!(record.moves()[3].mv().to_usi(), "5c5d");
        assert_eq!(record.result(), GameResult::BlackWin);
    }

    #[test]
    fn test_export_kif_basic() {
        let pos = hirate_position();
        let mv16 = Move::from_usi("7g7f").unwrap();
        let mut record = Record::new(pos.to_sfen(None)).unwrap();
        record
            .append_move_with_annotation(
                record.root_id(),
                MoveEntry::new(mv16),
                annotation_with_eval(120),
            )
            .unwrap();
        record
            .set_main_terminal(SpecialMoveEntry::new(SpecialMove::Resign, GameResult::BlackWin))
            .unwrap();

        let kif = export_kif(&record).unwrap();
        assert!(kif.contains("７六歩"));
        assert!(kif.contains("( 0:00/00:00:00)"));
        assert!(kif.contains("**評価値=120"));
        assert!(kif.ends_with('\n'));
    }

    #[test]
    fn test_export_kif_roundtrip_with_variation_and_terminal() {
        let pos = hirate_position();
        let mut record = Record::from_main_line_with_result(
            pos.to_sfen(None),
            vec![
                MoveEntry::new(Move::from_usi("7g7f").unwrap()),
                MoveEntry::new(Move::from_usi("3c3d").unwrap()),
            ],
            GameResult::WhiteWin,
        )
        .unwrap();
        record
            .add_variation_line(
                record.root_id(),
                vec![MoveEntry::new(Move::from_usi("2g2f").unwrap())],
            )
            .unwrap();

        let kif = export_kif(&record).unwrap();
        let parsed = parse_kif_str(&kif).unwrap();

        assert_eq!(parsed.main_moves().count(), 2);
        assert_eq!(parsed.result(), GameResult::WhiteWin);
        let root_children = parsed.children(parsed.root_id());
        assert_eq!(root_children.len(), 2);
        assert_eq!(
            parsed.node(root_children[1]).mv().expect("variation move").mv().to_usi(),
            "2g2f"
        );
    }

    #[test]
    fn test_export_kif_terminal_line_uses_time_column() {
        let pos = hirate_position();
        let record = Record::from_main_line_with_result(
            pos.to_sfen(None),
            vec![MoveEntry::new(Move::from_usi("7g7f").unwrap())],
            GameResult::BlackWin,
        )
        .unwrap();

        let kif = export_kif(&record).unwrap();
        assert!(kif.contains("   2 投了         ( 0:00/00:00:00)"));
        assert!(kif.contains("まで1手で先手の勝ち"));
    }

    #[test]
    fn test_export_kif_without_terminal() {
        let pos = hirate_position();
        let record = Record::from_main_line(
            pos.to_sfen(None),
            vec![MoveEntry::new(Move::from_usi("7g7f").unwrap())],
            None,
        )
        .unwrap();

        let kif = export_kif(&record).unwrap();
        assert!(kif.contains("   1 ７六歩(77)"));
        assert!(!kif.contains("投了"));
        assert!(!kif.contains("まで1手で"));
        assert!(kif.ends_with('\n'));
    }

    #[test]
    fn test_export_kif_roundtrip_without_terminal() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
 1 ７六歩(77) (0:27/00:00:27)
 2 ３四歩(33) (0:13/00:00:13)";

        let record = parse_kif_str(kif).unwrap();
        let exported = export_kif(&record).unwrap();
        let reparsed = parse_kif_str(&exported).unwrap();

        assert_eq!(reparsed.move_count(), 2);
        assert!(reparsed.main_terminal().is_none());
        assert_eq!(main_node(&reparsed, 0).time_ms(), Some(27_000));
        assert_eq!(main_node(&reparsed, 1).time_ms(), Some(13_000));
    }

    #[test]
    fn test_export_kif_writes_new_handicap_name() {
        let mut pos = Position::empty();
        pos.set_sfen(InitialPosition::Handicap5Pieces.to_sfen()).expect("valid handicap sfen");
        let mv16 = Move::from_usi("3c3d").expect("valid white move");
        let record = Record::from_main_line_with_result(
            pos.to_sfen(None),
            vec![MoveEntry::new(mv16)],
            GameResult::WhiteWin,
        )
        .expect("record");

        let kif = export_kif(&record).expect("export");
        assert!(kif.contains("手合割：五枚落ち"));
    }

    #[test]
    fn test_export_ki2_basic() {
        let pos = hirate_position();
        let mv16 = Move::from_usi("7g7f").unwrap();
        let mut record = Record::new(pos.to_sfen(None)).unwrap();
        record
            .append_move_with_annotation(
                record.root_id(),
                MoveEntry::new(mv16),
                annotation_with_eval(80),
            )
            .unwrap();
        record
            .set_main_terminal(SpecialMoveEntry::new(SpecialMove::Resign, GameResult::BlackWin))
            .unwrap();

        let ki2 = export_ki2(&record).unwrap();
        assert!(ki2.contains("▲７六歩"));
        assert!(ki2.contains("**評価値=80"));
        assert!(ki2.contains("まで1手で先手の勝ち"));
        assert!(ki2.ends_with('\n'));
    }

    #[test]
    fn test_export_ki2_writes_initial_comment_before_first_move() {
        let pos = hirate_position();
        let mut record = Record::from_main_line(
            pos.to_sfen(None),
            vec![MoveEntry::new(Move::from_usi("7g7f").unwrap())],
            None,
        )
        .unwrap();
        record.set_initial_comment(Some("序文1\n序文2".to_string()));

        let ki2 = export_ki2(&record).unwrap();
        assert!(ki2.contains("手合割：平手\n*序文1\n*序文2\n▲７六歩"));
    }

    #[test]
    fn test_export_ki2_writes_metadata_comment_as_note() {
        let pos = hirate_position();
        let mut record = Record::from_main_line(
            pos.to_sfen(None),
            vec![MoveEntry::new(Move::from_usi("7g7f").unwrap())],
            None,
        )
        .unwrap();
        let mut metadata_builder = RecordMetadata::builder();
        metadata_builder.comment(Some("備考1\n備考2".to_string()));
        record.set_metadata(metadata_builder.build());
        record.set_initial_comment(Some("序文".to_string()));

        let ki2 = export_ki2(&record).unwrap();
        assert!(ki2.contains("備考：備考1\n備考：備考2"));
        assert!(ki2.contains("*序文\n▲７六歩"));
        assert!(!ki2.contains("*備考1"));
    }

    #[test]
    fn test_export_ki2_roundtrip_with_variation_and_terminal() {
        let pos = hirate_position();
        let mut record = Record::from_main_line_with_result(
            pos.to_sfen(None),
            vec![
                MoveEntry::new(Move::from_usi("7g7f").unwrap()),
                MoveEntry::new(Move::from_usi("3c3d").unwrap()),
            ],
            GameResult::WhiteWin,
        )
        .unwrap();
        record
            .add_variation_line(
                record.root_id(),
                vec![MoveEntry::new(Move::from_usi("2g2f").unwrap())],
            )
            .unwrap();

        let ki2 = export_ki2(&record).unwrap();
        let parsed = parse_ki2_str(&ki2).unwrap();

        assert_eq!(parsed.main_moves().count(), 2);
        assert_eq!(parsed.result(), GameResult::WhiteWin);
        let root_children = parsed.children(parsed.root_id());
        assert_eq!(root_children.len(), 2);
        assert_eq!(
            parsed.node(root_children[1]).mv().expect("variation move").mv().to_usi(),
            "2g2f"
        );
    }

    #[test]
    fn test_export_ki2_without_terminal() {
        let pos = hirate_position();
        let record = Record::from_main_line(
            pos.to_sfen(None),
            vec![MoveEntry::new(Move::from_usi("7g7f").unwrap())],
            None,
        )
        .unwrap();

        let ki2 = export_ki2(&record).unwrap();
        assert!(ki2.contains("▲７六歩"));
        assert!(!ki2.contains("まで1手で"));
        assert!(ki2.ends_with('\n'));
    }

    #[test]
    fn test_export_kif_nonstandard_position_uses_board_labels() {
        let mut pos = Position::empty();
        pos.set_sfen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1")
            .expect("valid sfen");
        let record = Record::from_main_line_with_result(
            pos.to_sfen(None),
            vec![MoveEntry::new(Move::from_usi("3c3d").unwrap())],
            GameResult::WhiteWin,
        )
        .unwrap();

        let kif = export_kif(&record).unwrap();
        assert!(kif.contains("|v香v桂v銀v金v玉v金v銀v桂v香|一"));
        assert!(kif.contains("後手番"));
    }

    #[test]
    fn test_bod_output_matches_cshogi_layout() {
        let pos = hirate_position();
        let bod = board_to_bod(&pos);
        assert!(bod.contains("後手の持駒：なし"));
        assert!(bod.contains("先手の持駒：なし"));
        assert!(!bod.contains("後手番"));
    }

    #[test]
    fn test_bod_output_marks_white_turn() {
        let mut pos = Position::empty();
        pos.set_sfen("lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1")
            .expect("valid sfen");
        let bod = board_to_bod(&pos);
        assert!(bod.contains("後手番"));
    }

    #[test]
    fn test_move_to_bod_uses_ki2_notation() {
        let pos = hirate_position();
        let mv16 = Move::from_usi("7g7f").expect("valid move");
        let mv = pos.move32_from_move(mv16);
        let bod = move_to_bod(&pos, mv).expect("bod move");
        assert_eq!(bod, "▲７六歩");
    }

    // --- フェーズ 2 受け入れテスト ---

    /// A09: KIF UTF-8 バイト列エクスポート
    #[test]
    fn test_export_kif_bytes_utf8() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
まで1手で中断
";
        let record = parse_kif_str(kif).unwrap();
        let encoded = export_kif_bytes(&record, TextEncoding::Utf8).unwrap();
        assert!(!encoded.has_unmappable_chars());
        let text = std::str::from_utf8(encoded.bytes()).expect("valid UTF-8");
        assert!(text.contains("７六歩"));
    }

    /// A10: KIF Shift_JIS バイト列エクスポート
    #[test]
    fn test_export_kif_bytes_shift_jis() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
まで1手で中断
";
        let record = parse_kif_str(kif).unwrap();
        let encoded = export_kif_bytes(&record, TextEncoding::ShiftJis).unwrap();
        assert!(!encoded.has_unmappable_chars());
        // Shift_JIS バイト列を再パースできることを確認する
        let parsed = parse_kif_bytes(encoded.bytes()).unwrap();
        assert_eq!(parsed.main_moves().count(), 1);
    }

    /// A10: KI2 Shift_JIS バイト列エクスポート
    #[test]
    fn test_export_ki2_bytes_shift_jis() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
まで1手で中断
";
        let record = parse_kif_str(kif).unwrap();
        let encoded = export_ki2_bytes(&record, TextEncoding::ShiftJis).unwrap();
        assert!(!encoded.has_unmappable_chars());
        let parsed = parse_ki2_bytes(encoded.bytes()).unwrap();
        assert_eq!(parsed.main_moves().count(), 1);
    }

    /// A10: マッピング不能文字の検出
    #[test]
    fn test_export_kif_bytes_unmappable_chars() {
        let pos = hirate_position();
        let mv = Move::from_usi("7g7f").unwrap();
        // 絵文字コメント付き（Shift_JIS に変換できない文字）
        let mut record = Record::new(pos.to_sfen(None)).unwrap();
        record
            .append_move_with_annotation(
                record.root_id(),
                MoveEntry::new(mv),
                RecordAnnotation::new().with_comment(Some("🔥 good move!".to_string())),
            )
            .unwrap();
        record
            .set_main_terminal(SpecialMoveEntry::new(SpecialMove::Resign, GameResult::BlackWin))
            .unwrap();
        let encoded = export_kif_bytes(&record, TextEncoding::ShiftJis).unwrap();
        assert!(encoded.has_unmappable_chars());
    }

    /// A13: 正規エクスポーターの決定論的な出力
    #[test]
    fn test_export_kif_deterministic() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
まで2手で中断
";
        let record = parse_kif_str(kif).unwrap();
        let output1 = export_kif(&record).unwrap();
        let output2 = export_kif(&record).unwrap();
        assert_eq!(output1, output2);
    }

    #[test]
    fn test_export_kif_bytes_with_options() {
        let kif = "\
手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
まで1手で中断
";
        let record = parse_kif_str(kif).unwrap();
        let encoded =
            export_kif_bytes_with_options(&record, ExportOptions::new(TextEncoding::Utf8)).unwrap();
        assert!(!encoded.has_unmappable_chars());
        assert!(std::str::from_utf8(encoded.bytes()).unwrap().contains("７六歩"));
    }
}
