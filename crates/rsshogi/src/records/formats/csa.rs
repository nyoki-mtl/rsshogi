use crate::board::{Position, SfenError};
use crate::records::formats::common::{
    BoardMap, CsaVersion, EncodedText, ExportOptions, HandCounts, TextEncoding, board_map_to_sfen,
    encode_text, encode_text_with_options, ensure_hand_sides, hand_counts_to_sfen,
    refresh_position_if_needed,
};
use crate::records::record::{
    AnnotatedMoveEntry, EngineExtraValue, EngineInfo, Record, RecordAnnotation, RecordMetadata,
    RecordMetadataBuilder, SpecialMove, SpecialMoveEntry,
};
use crate::records::time_control::{TimeControl, parse_csa_time_control};
use crate::types::{Color, Eval, GameResult, Piece, PieceType, Square};
use encoding_rs::SHIFT_JIS;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CsaError {
    #[error("failed to parse SFEN: {0}")]
    Sfen(#[from] SfenError),
    #[error("invalid CSA line: {0}")]
    InvalidLine(String),
    #[error("invalid CSA board line: {0}")]
    InvalidBoard(String),
    #[error("invalid CSA move line: {0}")]
    InvalidMove(String),
    /// 指し手自体は合法でも、手番記号が手番と食い違う行。
    ///
    /// [`CsaError::IllegalMove`] と分けているのは、原文を持たないと「どの producer が
    /// 誤った符号を書いたか」を追えないため。
    #[error("move sign does not match the side to move: {0}")]
    MoveSideMismatch(String),
    #[error("illegal move at index {index}")]
    IllegalMove { index: usize },
    #[error("missing CSA end marker")]
    MissingEndMarker,
}

/// `'**` 行の読み筋を原文のまま保持する `EngineInfo` extras キー。
const CSA_PV_EXTRA_KEY: &str = "csa_pv";
/// 解析できなかった `'**` 行を原文のまま保持する `EngineInfo` extras キー。
const CSA_ANALYSIS_RAW_EXTRA_KEY: &str = "csa_analysis_raw";
/// 複数棋譜を区切るセパレータ行。
const CSA_GAME_SEPARATOR: &str = "/";

const INITIAL_COUNTS: [(char, [(&str, u8); 8]); 2] = [
    ('+', [("FU", 9), ("KY", 2), ("KE", 2), ("GI", 2), ("KI", 2), ("KA", 1), ("HI", 1), ("OU", 1)]),
    ('-', [("FU", 9), ("KY", 2), ("KE", 2), ("GI", 2), ("KI", 2), ("KA", 1), ("HI", 1), ("OU", 1)]),
];

fn base_piece(piece_code: &str) -> &str {
    match piece_code {
        "TO" => "FU",
        "NY" => "KY",
        "NK" => "KE",
        "NG" => "GI",
        "UM" => "KA",
        "RY" => "HI",
        _ => piece_code,
    }
}

fn parse_row_lines(lines: &[String]) -> Result<BoardMap, CsaError> {
    let mut board_map: BoardMap = HashMap::new();
    let mut rows: HashMap<u8, String> = HashMap::new();
    for line in lines {
        let rank = line
            .chars()
            .nth(1)
            .and_then(|ch| ch.to_digit(10))
            .ok_or_else(|| CsaError::InvalidBoard(line.clone()))? as u8;
        rows.insert(rank, line[2..].to_string());
    }
    for rank in 1..=9 {
        let row_spec = rows.get(&rank).cloned().unwrap_or_default();
        let compact: String = row_spec.chars().filter(|ch| *ch != ' ').collect();
        let mut file = 9u8;
        let mut idx = 0usize;
        let chars: Vec<char> = compact.chars().collect();
        while file >= 1 && idx < chars.len() {
            let token = chars[idx];
            if token == '+' || token == '-' {
                if idx + 2 >= chars.len() {
                    return Err(CsaError::InvalidBoard(row_spec));
                }
                let piece_code = format!("{}{}", chars[idx + 1], chars[idx + 2]);
                board_map.insert((file, rank), (token, piece_code));
                idx += 3;
            } else if token == '*' {
                idx += 1;
            } else {
                return Err(CsaError::InvalidBoard(row_spec));
            }
            if file == 1 {
                break;
            }
            file -= 1;
        }
    }
    Ok(board_map)
}

fn parse_piece_line(line: &str) -> Result<Vec<(char, String, String)>, CsaError> {
    let mut chars = line.chars();
    if chars.next() != Some('P') {
        return Err(CsaError::InvalidLine(line.to_string()));
    }
    let color = chars
        .next()
        .filter(|ch| *ch == '+' || *ch == '-')
        .ok_or_else(|| CsaError::InvalidLine(line.to_string()))?;
    let mut data: String = chars.collect();
    data.retain(|ch| ch != ' ');
    if data.is_empty() {
        return Ok(Vec::new());
    }
    if !data.len().is_multiple_of(4) {
        return Err(CsaError::InvalidLine(line.to_string()));
    }
    let mut out = Vec::new();
    let bytes = data.as_bytes();
    for idx in (0..bytes.len()).step_by(4) {
        let square = String::from_utf8(bytes[idx..idx + 2].to_vec())
            .map_err(|_| CsaError::InvalidLine(line.to_string()))?;
        let piece_code = String::from_utf8(bytes[idx + 2..idx + 4].to_vec())
            .map_err(|_| CsaError::InvalidLine(line.to_string()))?;
        out.push((color, square, piece_code));
    }
    Ok(out)
}

fn parse_time_text(text: &str) -> Option<u32> {
    let mut seconds = 0u32;
    let mut multiplier = 1u32;
    let mut parsed_any = false;
    for part in text.split(['：', ':', ' ', '　']).filter(|part| !part.is_empty()).rev() {
        if let Ok(value) = part.parse::<u32>() {
            // 桁溢れする値は解釈しない。debug の panic と release の値化けを避ける。
            seconds = seconds.checked_add(value.checked_mul(multiplier)?)?;
            multiplier = multiplier.checked_mul(60)?;
            parsed_any = true;
        }
    }
    if parsed_any { seconds.checked_mul(1_000) } else { None }
}

/// CSA の消費時間本体（秒、小数 3 桁まで）をミリ秒へ変換する。
///
/// ミリ秒表記は V3.0 で追加された。整数秒だけの V2.2 表記も同じ経路で扱う。
fn parse_csa_elapsed_ms(text: &str) -> Option<u32> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (seconds_text, fraction_text) = match trimmed.split_once('.') {
        Some((seconds, fraction)) => (seconds, Some(fraction)),
        None => (trimmed, None),
    };
    if !seconds_text.chars().all(|ch| ch.is_ascii_digit()) || seconds_text.is_empty() {
        return None;
    }
    let mut total_ms = seconds_text.parse::<u32>().ok()?.checked_mul(1_000)?;
    if let Some(fraction) = fraction_text {
        if fraction.is_empty()
            || fraction.len() > 3
            || !fraction.chars().all(|ch| ch.is_ascii_digit())
        {
            return None;
        }
        let mut padded = fraction.to_string();
        while padded.len() < 3 {
            padded.push('0');
        }
        total_ms = total_ms.checked_add(padded.parse::<u32>().ok()?)?;
    }
    Some(total_ms)
}

fn parse_csa_time_token(token: &str) -> Option<u32> {
    let trimmed = token.trim();
    let body = match trimmed.strip_prefix("T=") {
        Some(rest) => rest,
        None => trimmed.strip_prefix('T')?,
    };
    // 仕様どおりの秒表記を優先し、`T1:30` のような非標準表記だけ従来解釈へ落とす。
    parse_csa_elapsed_ms(body).or_else(|| parse_time_text(body))
}

/// CSA 消費時間行を組み立てる。
///
/// ミリ秒表記は過去のプログラムが読めなくなるため、V3.0 出力かつ端数がある場合だけ使う。
fn format_csa_elapsed(elapsed_ms: u32, version: CsaVersion) -> String {
    if version == CsaVersion::V3_0 && !elapsed_ms.is_multiple_of(1_000) {
        format!("T{}.{:03}", elapsed_ms / 1_000, elapsed_ms % 1_000)
    } else {
        format!("T{}", elapsed_ms / 1_000)
    }
}

/// `'**` 行から取り出した評価値・読み筋・ノード数。
struct CsaAnalysis {
    eval: Option<i32>,
    pv: Option<String>,
    nodes: Option<u64>,
}

/// V3.0 の `'** (評価値) (読み筋) #(ノード数)` 行を解析する。
///
/// 読み筋は `+PASS` や `%TORYO` を含みうるうえ再生には局面が要るので、
/// 型付けせず元の文字列のまま保持する。
fn parse_csa_analysis_line(line: &str) -> Option<CsaAnalysis> {
    let body = line.trim().strip_prefix("'**")?.trim();
    if body.is_empty() {
        return None;
    }
    let mut nodes = None;
    let mut rest = body;
    if let Some(idx) = body.rfind('#')
        && let Ok(value) = body[idx + 1..].trim().parse::<u64>()
    {
        nodes = Some(value);
        rest = body[..idx].trim_end();
    }
    let mut tokens = rest.split_whitespace();
    let eval = tokens.next().and_then(|token| token.parse::<i32>().ok());
    let pv: Vec<&str> = tokens.collect();
    let pv = if pv.is_empty() { None } else { Some(pv.join(" ")) };
    Some(CsaAnalysis { eval, pv, nodes })
}

fn parse_csa_program_comment_text(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.starts_with("'**") {
        return None;
    }
    let comment = trimmed.strip_prefix("'*")?;
    Some(comment.trim())
}

fn parse_csa_note_value(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn format_csa_note_value(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            _ => out.push(ch),
        }
    }
    out
}

fn append_comment_text(slot: &mut Option<String>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(existing) = slot {
        existing.push('\n');
        existing.push_str(text);
    } else {
        *slot = Some(text.to_string());
    }
}

/// `'**` 行を `EngineInfo` へ取り込む。
///
/// 評価値を読めない行、および `Eval` が保持できない範囲の評価値は解析を諦め、原文を
/// extras へ追記して情報を落とさない。原文を追記する経路は同じ指し手に複数行あっても
/// 累積する。一方、解析できた行は後の行が前の行を上書きする。仕様は `'**` を一手一行と
/// 定めているため、複数行は想定外の入力として最後の行を採る。
fn apply_csa_analysis(info: &mut EngineInfo, line: &str) {
    // `Eval` は i16 なので、収まらない評価値は数値化せず原文のまま持ち回す。
    let parsed = parse_csa_analysis_line(line)
        .filter(|analysis| analysis.eval.is_some_and(|eval| i16::try_from(eval).is_ok()));
    let Some(analysis) = parsed else {
        let mut raw = match info.extras().get(CSA_ANALYSIS_RAW_EXTRA_KEY) {
            Some(EngineExtraValue::String(existing)) => format!("{existing}\n"),
            _ => String::new(),
        };
        raw.push_str(line.trim());
        info.set_extra(CSA_ANALYSIS_RAW_EXTRA_KEY.to_string(), EngineExtraValue::String(raw));
        return;
    };
    info.set_eval(analysis.eval.map(Eval::from_i32));
    if let Some(nodes) = analysis.nodes {
        info.set_nodes(Some(nodes));
    }
    if let Some(pv) = analysis.pv {
        info.set_extra(CSA_PV_EXTRA_KEY.to_string(), EngineExtraValue::String(pv));
    }
}

/// `AnnotatedMoveEntry` に `'**` 行を取り込む。
fn apply_csa_analysis_to_move(record: &mut AnnotatedMoveEntry, line: &str) {
    let mut info = record.engine_info().cloned().unwrap_or_default();
    apply_csa_analysis(&mut info, line);
    record.set_engine_info(Some(info));
}

fn append_move_comment(record: &mut AnnotatedMoveEntry, text: &str) {
    let mut comment = record.comment().map(ToOwned::to_owned);
    append_comment_text(&mut comment, text);
    record.set_comment(comment);
}

fn apply_csa_metadata(builder: &mut RecordMetadataBuilder, key: &str, value: &str) {
    match key {
        "EVENT" => {
            builder.event(Some(value.to_string()));
        }
        "SITE" => {
            builder.site(Some(value.to_string()));
        }
        "START_TIME" => {
            builder.start_date(Some(value.to_string()));
        }
        "END_TIME" => {
            builder.end_date(Some(value.to_string()));
        }
        "TIME_LIMIT" | "TIME" => {
            let parsed = parse_csa_time_control(value).or_else(|| TimeControl::from_spec(value));
            builder.time_control(parsed);
        }
        // V3.0 仕様は本文で `TIMET+` / `TIMET-`、例で `TIME+` / `TIME-` と綴りが割れている。
        // どちらも同じ意味として受理し、書き出しは例に合わせた `TIME+` / `TIME-` に寄せる。
        "TIME+" | "TIMET+" => {
            let parsed = parse_csa_time_control(value).or_else(|| TimeControl::from_spec(value));
            builder.black_time_control(parsed);
        }
        "TIME-" | "TIMET-" => {
            let parsed = parse_csa_time_control(value).or_else(|| TimeControl::from_spec(value));
            builder.white_time_control(parsed);
        }
        "MAX_MOVES" => {
            builder.max_moves(value.trim().parse::<u32>().ok());
        }
        "JISHOGI" => {
            builder.impasse_rule(Some(value.to_string()));
        }
        "NOTE" => {
            builder.append_comment_line(&parse_csa_note_value(value));
        }
        _ => {
            builder.add_attribute(key.to_string(), value.to_string());
        }
    }
}

/// 盤面を CSA の局面行 (P1..P9, P+, P-, +/-) に変換する。
#[must_use]
pub fn board_to_csa(pos: &Position) -> String {
    let mut lines = Vec::with_capacity(12);
    append_csa_position_lines(&mut lines, pos);
    lines.join("\n")
}

fn decode_start_board() -> BoardMap {
    let mut board = HashMap::new();
    let mut pos = Position::empty();
    pos.set_hirate();
    for rank in 1..=9 {
        for file in 1..=9 {
            let file_char = char::from(b'0' + file);
            let rank_char = char::from(b'a' + (rank - 1));
            let sq = Square::from_usi(&format!("{file_char}{rank_char}")).expect("valid square");
            let piece = pos.piece_on(sq);
            if piece == Piece::NONE {
                continue;
            }
            let color = if piece.color() == Color::BLACK { '+' } else { '-' };
            let piece_code = match piece.piece_type() {
                PieceType::PAWN => "FU",
                PieceType::LANCE => "KY",
                PieceType::KNIGHT => "KE",
                PieceType::SILVER => "GI",
                PieceType::GOLD => "KI",
                PieceType::BISHOP => "KA",
                PieceType::ROOK => "HI",
                PieceType::KING => "OU",
                PieceType::PRO_PAWN => "TO",
                PieceType::PRO_LANCE => "NY",
                PieceType::PRO_KNIGHT => "NK",
                PieceType::PRO_SILVER => "NG",
                PieceType::HORSE => "UM",
                PieceType::DRAGON => "RY",
                _ => "OU",
            };
            board.insert((file, rank), (color, piece_code.to_string()));
        }
    }
    board
}

fn terminal_from_marker(marker: &str, side_to_move: Color) -> SpecialMoveEntry {
    let is_black_turn = side_to_move == Color::BLACK;
    match marker {
        "%TORYO" => {
            if is_black_turn {
                SpecialMoveEntry::new(SpecialMove::Resign, GameResult::WhiteWin)
            } else {
                SpecialMoveEntry::new(SpecialMove::Resign, GameResult::BlackWin)
            }
        }
        "%SENNICHITE" => {
            SpecialMoveEntry::new(SpecialMove::RepetitionDraw, GameResult::DrawByRepetition)
        }
        "%HIKIWAKE" => SpecialMoveEntry::new(SpecialMove::Draw, GameResult::DrawByImpasse),
        "%JISHOGI" => SpecialMoveEntry::new(SpecialMove::Impasse, GameResult::DrawByImpasse),
        "%MAX_MOVES" => SpecialMoveEntry::new(SpecialMove::MaxMoves, GameResult::DrawByMaxPlies),
        "%KACHI" => {
            if is_black_turn {
                SpecialMoveEntry::new(
                    SpecialMove::WinByDeclaration,
                    GameResult::BlackWinByDeclaration,
                )
            } else {
                SpecialMoveEntry::new(
                    SpecialMove::WinByDeclaration,
                    GameResult::WhiteWinByDeclaration,
                )
            }
        }
        "%CHUDAN" => SpecialMoveEntry::new(SpecialMove::Interrupt, GameResult::Paused),
        "%TSUMI" => {
            if is_black_turn {
                SpecialMoveEntry::new(SpecialMove::Mate, GameResult::WhiteWin)
            } else {
                SpecialMoveEntry::new(SpecialMove::Mate, GameResult::BlackWin)
            }
        }
        "%FUZUMI" => SpecialMoveEntry::new(SpecialMove::NoMate, GameResult::Paused),
        // 手番側の反則負けなので、勝つのは手番の相手。
        "%ILLEGAL_MOVE" => {
            if is_black_turn {
                SpecialMoveEntry::new(
                    SpecialMove::WinByIllegalMove,
                    GameResult::WhiteWinByIllegalMove,
                )
            } else {
                SpecialMoveEntry::new(
                    SpecialMove::WinByIllegalMove,
                    GameResult::BlackWinByIllegalMove,
                )
            }
        }
        // ILLEGAL_ACTION は反則を犯した側を記号自身が示すので、手番に依存しない。
        // `%+` は先手の反則行為による後手勝ち、`%-` はその逆。
        "%+ILLEGAL_ACTION" => {
            SpecialMoveEntry::new(SpecialMove::WinByIllegalMove, GameResult::WhiteWinByIllegalMove)
        }
        "%-ILLEGAL_ACTION" => {
            SpecialMoveEntry::new(SpecialMove::WinByIllegalMove, GameResult::BlackWinByIllegalMove)
        }
        // 時間切れも手番側の負けなので、勝つのは手番の相手。
        "%TIME_UP" => {
            if is_black_turn {
                SpecialMoveEntry::new(SpecialMove::Timeout, GameResult::WhiteWinByTimeout)
            } else {
                SpecialMoveEntry::new(SpecialMove::Timeout, GameResult::BlackWinByTimeout)
            }
        }
        "%ERROR" => {
            SpecialMoveEntry::new(SpecialMove::Unknown("ERROR".to_string()), GameResult::Error)
        }
        _ => SpecialMoveEntry::new(
            SpecialMove::Unknown(marker.trim_start_matches('%').to_string()),
            GameResult::Paused,
        ),
    }
}

fn marker_from_terminal(terminal: &SpecialMoveEntry) -> String {
    // 元の棋譜が持っていたマーカーを最優先する。`%+ILLEGAL_ACTION` のように
    // kind へ畳み込むと戻せない情報があるため。
    if let Some(raw) = terminal.raw() {
        let trimmed = raw.trim();
        if trimmed.starts_with('%') && !trimmed.contains(char::is_whitespace) {
            return trimmed.to_string();
        }
    }
    match terminal.kind() {
        SpecialMove::Interrupt => "%CHUDAN".to_string(),
        SpecialMove::Resign => "%TORYO".to_string(),
        SpecialMove::MaxMoves => "%MAX_MOVES".to_string(),
        SpecialMove::Impasse => "%JISHOGI".to_string(),
        SpecialMove::Draw => "%HIKIWAKE".to_string(),
        SpecialMove::RepetitionDraw => "%SENNICHITE".to_string(),
        SpecialMove::Mate => "%TSUMI".to_string(),
        SpecialMove::NoMate => "%FUZUMI".to_string(),
        SpecialMove::Timeout => "%TIME_UP".to_string(),
        SpecialMove::WinByIllegalMove | SpecialMove::LoseByIllegalMove => {
            "%ILLEGAL_MOVE".to_string()
        }
        SpecialMove::WinByDeclaration => "%KACHI".to_string(),
        // 不戦勝 / 不戦敗 / トライは CSA に対応するマーカーがない。勝敗を反転させる
        // `%KACHI` を書くより、勝敗を主張しない中断へ落とす。
        SpecialMove::WinByDefault | SpecialMove::LoseByDefault | SpecialMove::Try => {
            "%CHUDAN".to_string()
        }
        SpecialMove::Unknown(name) => {
            if name.starts_with('%') {
                name.clone()
            } else {
                format!("%{name}")
            }
        }
    }
}

fn decode_csa_bytes(data: &[u8]) -> Cow<'_, str> {
    std::str::from_utf8(data).map_or_else(
        |_| {
            let (decoded, _, _) = SHIFT_JIS.decode(data);
            decoded
        },
        Cow::Borrowed,
    )
}

/// バイト列から CSA 形式の棋譜を解析する。
///
/// UTF-8 として解釈を試み、失敗した場合は Shift_JIS としてデコードする。
/// BOM 付き入力にも対応する。
///
/// # Examples
///
/// ```
/// use rsshogi::records::formats::csa;
///
/// let csa_bytes = b"V2.2\nPI\n+\n+7776FU\n%TORYO\n";
/// let record = csa::parse_csa_bytes(csa_bytes).unwrap();
/// assert_eq!(record.main_moves().count(), 1);
/// ```
pub fn parse_csa_bytes(data: &[u8]) -> Result<Record, CsaError> {
    let decoded = decode_csa_bytes(data);
    parse_csa_str(&decoded)
}

/// UTF-8 文字列から `/` 区切りの複数棋譜を解析する。
///
/// CSA では `/` だけの行で複数の棋譜を 1 ファイルに収められる。単一棋譜の
/// ファイルでも要素数 1 の `Vec` を返す。空の区画は読み飛ばす。
///
/// # Examples
///
/// ```
/// use rsshogi::records::formats::csa;
///
/// let text = "V2.2\nPI\n+\n+7776FU\n%TORYO\n/\nV2.2\nPI\n+\n+2726FU\n%TORYO\n";
/// let games = csa::parse_csa_games(text).unwrap();
/// assert_eq!(games.len(), 2);
/// ```
pub fn parse_csa_games(text: &str) -> Result<Vec<Record>, CsaError> {
    let mut games = Vec::new();
    let mut chunk: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.trim_end_matches('\r').trim() == CSA_GAME_SEPARATOR {
            if let Some(record) = parse_csa_chunk(&chunk)? {
                games.push(record);
            }
            chunk.clear();
            continue;
        }
        chunk.push(line);
    }
    if let Some(record) = parse_csa_chunk(&chunk)? {
        games.push(record);
    }
    Ok(games)
}

/// バイト列から `/` 区切りの複数棋譜を解析する。
///
/// デコード規則は [`parse_csa_bytes`] と同じ。
pub fn parse_csa_games_bytes(data: &[u8]) -> Result<Vec<Record>, CsaError> {
    let decoded = decode_csa_bytes(data);
    parse_csa_games(&decoded)
}

fn parse_csa_chunk(lines: &[&str]) -> Result<Option<Record>, CsaError> {
    // 空行とコメントしかない区画は棋譜ではない。末尾セパレータの後ろに残る
    // コメントを 1 局として数えないための判定。
    let has_content = lines.iter().any(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && !trimmed.starts_with('\'')
    });
    if !has_content {
        return Ok(None);
    }
    parse_csa_str(&lines.join("\n")).map(Some)
}

/// UTF-8 文字列から CSA 形式の棋譜を解析する。
///
/// `/` 区切りで複数の棋譜が入っている場合、最初の 1 局だけを返す。全局が必要なら
/// [`parse_csa_games`] を使う。
pub fn parse_csa_str(text: &str) -> Result<Record, CsaError> {
    let mut raw_lines: Vec<String> =
        text.lines().map(|line| line.trim_end_matches('\r').to_string()).collect();
    if let Some(first) = raw_lines.first_mut() {
        *first = first.trim_start_matches('\u{feff}').to_string();
    }
    let mut lines: Vec<String> = Vec::new();
    for line in raw_lines {
        if line.starts_with('\'') || line.starts_with('N') || line.starts_with('$') {
            lines.push(line);
            continue;
        }
        for token in line.split(',') {
            if token.is_empty() {
                continue;
            }
            lines.push(token.to_string());
        }
    }

    let mut pos = Position::empty();
    let mut position_lines: Vec<String> = Vec::new();
    let mut side_to_move_token: Option<char> = None;
    let mut header_end = lines.len();
    let mut metadata_builder = RecordMetadata::builder();
    let mut initial_comment: Option<String> = None;

    for (idx, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            header_end = idx + 1;
            break;
        }
        // 指し手のない第 1 局が `/` で終わるとき、次局の header を混ぜない。
        if trimmed == CSA_GAME_SEPARATOR {
            header_end = idx;
            break;
        }
        if trimmed.starts_with("'CSA encoding=") {
            continue;
        }
        if trimmed.starts_with('\'') {
            let comment = parse_csa_program_comment_text(trimmed).unwrap_or_default();
            if !comment.is_empty() {
                append_comment_text(&mut initial_comment, comment);
            }
            continue;
        }
        if let Some(stripped) = trimmed.strip_prefix('$') {
            if let Some((key, value)) = stripped.split_once(':') {
                apply_csa_metadata(&mut metadata_builder, key.trim(), value.trim());
            }
            continue;
        }
        if let Some(stripped) = trimmed.strip_prefix("N+") {
            metadata_builder.black_player(Some(stripped.trim().to_string()));
            continue;
        }
        if let Some(stripped) = trimmed.strip_prefix("N-") {
            metadata_builder.white_player(Some(stripped.trim().to_string()));
            continue;
        }
        // `%` 行は常に終局行であり、手番行にはなりえない。長さで判定すると
        // `%TORYO` のような 6 文字マーカーが手番行として食われる。
        if trimmed.starts_with('%') {
            header_end = idx;
            break;
        }
        if trimmed.starts_with('+') || trimmed.starts_with('-') {
            if trimmed.len() >= 7 {
                header_end = idx;
                break;
            }
            side_to_move_token = trimmed.chars().next();
            continue;
        }
        if trimmed.starts_with('P') {
            position_lines.push(trimmed.to_string());
        }
    }

    let row_lines: Vec<String> = position_lines
        .iter()
        .filter(|line| {
            line.len() >= 2
                && line.starts_with('P')
                && line.chars().nth(1).is_some_and(|ch| ch.is_ascii_digit())
        })
        .cloned()
        .collect();
    let pi_line = position_lines.iter().find(|line| line.starts_with("PI")).cloned();
    let piece_lines: Vec<String> = position_lines
        .iter()
        .filter(|line| line.starts_with("P+") || line.starts_with("P-"))
        .cloned()
        .collect();

    let mut board_map: BoardMap = if position_lines.is_empty() {
        decode_start_board()
    } else if !row_lines.is_empty() {
        parse_row_lines(&row_lines)?
    } else if let Some(pi_line) = pi_line {
        let mut start = decode_start_board();
        let suffix = pi_line[2..].trim();
        if !suffix.is_empty() {
            if suffix.len() % 4 != 0 {
                return Err(CsaError::InvalidBoard(pi_line));
            }
            let bytes = suffix.as_bytes();
            for idx in (0..bytes.len()).step_by(4) {
                let square = String::from_utf8(bytes[idx..idx + 2].to_vec())
                    .map_err(|_| CsaError::InvalidBoard(pi_line.clone()))?;
                let file = square.chars().next().and_then(|ch| ch.to_digit(10)).unwrap_or(0);
                let rank = square.chars().nth(1).and_then(|ch| ch.to_digit(10)).unwrap_or(0);
                if file >= 1 && rank >= 1 {
                    start.remove(&(file as u8, rank as u8));
                }
            }
        }
        start
    } else {
        HashMap::new()
    };

    let mut hand_counts: HandCounts = HashMap::new();
    ensure_hand_sides(&mut hand_counts);
    let mut fill_remaining: HashMap<char, bool> = HashMap::new();
    fill_remaining.insert('+', false);
    fill_remaining.insert('-', false);

    for entry_line in piece_lines {
        for (color, square, piece_code) in parse_piece_line(&entry_line)? {
            if square == "00" {
                if piece_code == "AL" {
                    fill_remaining.insert(color, true);
                    continue;
                }
                let entry = hand_counts.entry(color).or_default().entry(piece_code).or_insert(0);
                *entry += 1;
            } else {
                let file = square.chars().next().and_then(|ch| ch.to_digit(10)).unwrap_or(0);
                let rank = square.chars().nth(1).and_then(|ch| ch.to_digit(10)).unwrap_or(0);
                if file >= 1 && rank >= 1 {
                    board_map.insert((file as u8, rank as u8), (color, piece_code));
                }
            }
        }
    }

    let mut placed_counts: HashMap<char, HashMap<String, u8>> = HashMap::new();
    for (color, initial) in INITIAL_COUNTS {
        let mut map = HashMap::new();
        for (piece_code, _) in initial {
            map.insert(piece_code.to_string(), 0);
        }
        placed_counts.insert(color, map);
    }

    for (color, piece_code) in board_map.values().map(|(c, p)| (*c, p.clone())) {
        let base = base_piece(&piece_code).to_string();
        if let Some(counts) = placed_counts.get_mut(&color)
            && let Some(entry) = counts.get_mut(&base)
        {
            *entry += 1;
        }
    }

    for (color, counts) in hand_counts.clone() {
        for (piece_code, count) in counts {
            let base = base_piece(&piece_code).to_string();
            if let Some(entries) = placed_counts.get_mut(&color)
                && let Some(entry) = entries.get_mut(&base)
            {
                *entry += count;
            }
        }
    }

    for (color, initial) in INITIAL_COUNTS {
        if !*fill_remaining.get(&color).unwrap_or(&false) {
            continue;
        }
        for (piece_code, initial_count) in initial {
            let base = piece_code.to_string();
            let placed = placed_counts
                .get(&color)
                .and_then(|counts| counts.get(&base))
                .copied()
                .unwrap_or(0);
            if placed < initial_count {
                let remaining = initial_count - placed;
                let entry = hand_counts.entry(color).or_default().entry(base.clone()).or_insert(0);
                *entry += remaining;
            }
        }
    }

    let board_sfen = board_map_to_sfen(&board_map).map_err(CsaError::InvalidBoard)?;
    let hands_sfen = hand_counts_to_sfen(&hand_counts).map_err(CsaError::InvalidBoard)?;
    let turn = if side_to_move_token == Some('-') { "w" } else { "b" };
    let init_position_sfen = format!("{board_sfen} {turn} {hands_sfen} 1");

    let metadata = metadata_builder.build();

    pos.set_sfen(&init_position_sfen)?;

    let mut moves: Vec<AnnotatedMoveEntry> = Vec::new();
    let mut terminal: Option<(SpecialMoveEntry, RecordAnnotation)> = None;
    let mut idx = header_end;
    let mut refresh_counter = 0usize;
    let mut pending_time_ms: Option<u32> = None;

    while idx < lines.len() {
        let line = lines[idx].trim();
        idx += 1;
        if line.is_empty() {
            continue;
        }
        if line == CSA_GAME_SEPARATOR {
            break;
        }
        // 非標準だが実在する `'**評価値=N` 表記。読み取りだけ互換で残す。
        // 数値として読めない場合は下の汎用分岐へ落とし、原文を退避する。
        if let Some(value) =
            line.strip_prefix("'**評価値=").and_then(|text| text.trim().parse::<i16>().ok())
        {
            if let Some(last) = moves.last_mut() {
                last.set_eval(Some(Eval::from_i32(i32::from(value))));
            }
            continue;
        }
        if line.starts_with("'**") {
            if let Some(last) = moves.last_mut() {
                apply_csa_analysis_to_move(last, line);
            }
            continue;
        }
        if line.starts_with('\'') {
            let comment = parse_csa_program_comment_text(line).unwrap_or_default();
            if !comment.is_empty() {
                if let Some(last) = moves.last_mut() {
                    append_move_comment(last, comment);
                } else {
                    append_comment_text(&mut initial_comment, comment);
                }
            }
            continue;
        }
        if line.starts_with('T') {
            if let Some(time_ms) = parse_csa_time_token(line) {
                if let Some(last) = moves.last_mut() {
                    last.set_time_ms(Some(time_ms));
                } else {
                    pending_time_ms = Some(time_ms);
                }
            }
            continue;
        }
        if line.starts_with('%') {
            let record = terminal_from_marker(line, pos.turn()).with_raw(Some(line.to_string()));
            let mut terminal_annotation = RecordAnnotation::new();
            let mut terminal_comment_lines: Vec<String> = Vec::new();
            let mut terminal_engine_info = EngineInfo::new();
            while idx < lines.len() {
                let next = lines[idx].trim();
                if next.is_empty() {
                    idx += 1;
                    continue;
                }
                if next.starts_with('T') {
                    if let Some(time_ms) = parse_csa_time_token(next) {
                        terminal_annotation.set_elapsed_ms(Some(time_ms));
                    }
                    idx += 1;
                    continue;
                }
                if next.starts_with("'**") {
                    apply_csa_analysis(&mut terminal_engine_info, next);
                    idx += 1;
                    continue;
                }
                if next.starts_with('\'') {
                    let comment = parse_csa_program_comment_text(next).unwrap_or_default();
                    if !comment.is_empty() {
                        terminal_comment_lines.push(comment.to_string());
                    }
                    idx += 1;
                    continue;
                }
                break;
            }
            if !terminal_engine_info.is_empty() {
                terminal_annotation.set_engine_info(Some(terminal_engine_info));
            }
            if !terminal_comment_lines.is_empty() {
                terminal_annotation.set_comment(Some(terminal_comment_lines.join("\n")));
            }
            terminal = Some((record, terminal_annotation));
            break;
        }
        if line.starts_with('+') || line.starts_with('-') {
            let move_token = line.split(',').next().unwrap_or(line);
            // 符号を含む 7 文字のまま渡し、手番と手番記号の一致を `move_from_csa` に検査させる。
            let move_body =
                move_token.get(0..7).ok_or_else(|| CsaError::InvalidMove(line.to_string()))?;
            let expected_sign = if pos.turn() == Color::BLACK { '+' } else { '-' };
            if !move_body.starts_with(expected_sign) {
                return Err(CsaError::MoveSideMismatch(line.to_string()));
            }
            let mv = pos.move_from_csa(move_body);
            if !pos.is_legal_move32(mv) {
                return Err(CsaError::IllegalMove { index: moves.len() });
            }
            let mut mv_record = AnnotatedMoveEntry::new(mv.to_move());
            if let Some(time_ms) = line.split(',').skip(1).filter_map(parse_csa_time_token).next() {
                mv_record = mv_record.with_time_ms(Some(time_ms));
            } else if let Some(time_ms) = pending_time_ms.take() {
                mv_record = mv_record.with_time_ms(Some(time_ms));
            }
            pos.apply_move32(mv);
            moves.push(mv_record);
            refresh_position_if_needed(&mut pos, &mut refresh_counter)?;
            continue;
        }
        return Err(CsaError::InvalidLine(line.to_string()));
    }

    let mut record = Record::from_annotated_main_line(init_position_sfen.clone(), moves, terminal)
        .map_err(|e| CsaError::InvalidLine(e.to_string()))?;
    record.set_initial_comment(initial_comment);
    record.set_metadata(metadata);
    Ok(record)
}

fn append_csa_position_lines(lines: &mut Vec<String>, pos: &Position) {
    for rank in 1..=9u8 {
        let mut row = format!("P{rank}");
        for file in (1..=9u8).rev() {
            let file_char = char::from(b'0' + file);
            let rank_char = char::from(b'a' + (rank - 1));
            let sq = Square::from_usi(&format!("{file_char}{rank_char}")).expect("valid square");
            let piece = pos.piece_on(sq);
            if piece == Piece::NONE {
                row.push_str("*  ");
                continue;
            }
            let color = if piece.color() == Color::BLACK { '+' } else { '-' };
            let piece_code = match piece.piece_type() {
                PieceType::PAWN => "FU",
                PieceType::LANCE => "KY",
                PieceType::KNIGHT => "KE",
                PieceType::SILVER => "GI",
                PieceType::GOLD => "KI",
                PieceType::BISHOP => "KA",
                PieceType::ROOK => "HI",
                PieceType::KING => "OU",
                PieceType::PRO_PAWN => "TO",
                PieceType::PRO_LANCE => "NY",
                PieceType::PRO_KNIGHT => "NK",
                PieceType::PRO_SILVER => "NG",
                PieceType::HORSE => "UM",
                PieceType::DRAGON => "RY",
                _ => "OU",
            };
            row.push(color);
            row.push_str(piece_code);
        }
        lines.push(row);
    }

    for (color, label) in [(Color::BLACK, "P+"), (Color::WHITE, "P-")] {
        let hand = pos.hand(color);
        let mut line = label.to_string();
        for piece_code in ["FU", "KY", "KE", "GI", "KI", "KA", "HI"] {
            let piece_type = match piece_code {
                "FU" => PieceType::PAWN,
                "KY" => PieceType::LANCE,
                "KE" => PieceType::KNIGHT,
                "GI" => PieceType::SILVER,
                "KI" => PieceType::GOLD,
                "KA" => PieceType::BISHOP,
                "HI" => PieceType::ROOK,
                _ => PieceType::PAWN,
            };
            let hand_piece =
                crate::types::HandPiece::from_piece_type(piece_type).expect("valid hand piece");
            let count = hand.count(hand_piece);
            for _ in 0..count {
                line.push_str("00");
                line.push_str(piece_code);
            }
        }
        lines.push(line);
    }

    let stm = if pos.turn() == Color::BLACK { "+" } else { "-" };
    lines.push(stm.to_string());
}

fn format_csa_time_limit(tc: &TimeControl) -> Option<(String, String)> {
    let base = tc.base_seconds();
    let byoyomi = tc.byoyomi_seconds();
    let increment = tc.increment_seconds();
    if base == 0 && byoyomi == 0 && increment == 0 {
        return None;
    }
    if increment == 0 {
        let hours = base / 3600;
        let minutes = (base % 3600) / 60;
        return Some(("TIME_LIMIT".to_string(), format!("{hours:02}:{minutes:02}+{byoyomi}")));
    }
    Some(("TIME".to_string(), tc.to_spec()))
}

fn append_csa_metadata(lines: &mut Vec<String>, metadata: &RecordMetadata) {
    let mut emitted: HashSet<String> = HashSet::new();
    if let Some(comment) = metadata.comment() {
        lines.push(format!("$NOTE:{}", format_csa_note_value(comment)));
        emitted.insert("NOTE".to_string());
    }
    if let Some(black) = metadata.black_player() {
        lines.push(format!("N+{black}"));
        emitted.insert("N+".to_string());
    }
    if let Some(white) = metadata.white_player() {
        lines.push(format!("N-{white}"));
        emitted.insert("N-".to_string());
    }
    if let Some(event) = metadata.event() {
        lines.push(format!("$EVENT:{event}"));
        emitted.insert("EVENT".to_string());
    }
    if let Some(site) = metadata.site() {
        lines.push(format!("$SITE:{site}"));
        emitted.insert("SITE".to_string());
    }
    if let Some(start) = metadata.start_date() {
        lines.push(format!("$START_TIME:{start}"));
        emitted.insert("START_TIME".to_string());
    }
    if let Some(end) = metadata.end_date() {
        lines.push(format!("$END_TIME:{end}"));
        emitted.insert("END_TIME".to_string());
    }
    if let Some(tc) = metadata.time_control()
        && let Some((key, text)) = format_csa_time_limit(tc)
    {
        lines.push(format!("${key}:{text}"));
        emitted.insert(key);
    }
    if let Some(tc) = metadata.black_time_control() {
        lines.push(format!("$TIME+:{}", tc.to_spec()));
        emitted.insert("TIME+".to_string());
    }
    if let Some(tc) = metadata.white_time_control() {
        lines.push(format!("$TIME-:{}", tc.to_spec()));
        emitted.insert("TIME-".to_string());
    }
    if let Some(max_moves) = metadata.max_moves() {
        lines.push(format!("$MAX_MOVES:{max_moves}"));
        emitted.insert("MAX_MOVES".to_string());
    }
    if let Some(rule) = metadata.impasse_rule() {
        lines.push(format!("$JISHOGI:{rule}"));
        emitted.insert("JISHOGI".to_string());
    }
    if !metadata.attributes().is_empty() {
        let mut keys: Vec<&String> = metadata.attributes().keys().collect();
        keys.sort();
        for key in keys {
            if emitted.contains(key.as_str()) {
                continue;
            }
            if let Some(value) = metadata.attributes().get(key) {
                lines.push(format!("${key}:{value}"));
            }
        }
    }
}

fn csa_position_p_command(sfen: &str) -> Option<(&'static str, &'static str)> {
    match sfen {
        "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1" => Some(("PI", "+")),
        "lnsgkgsn1/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some(("PI11KY", "-")),
        "1nsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some(("PI91KY", "-")),
        "lnsgkgsnl/1r7/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some(("PI22KA", "-")),
        "lnsgkgsnl/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some(("PI82HI", "-")),
        "lnsgkgsn1/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
            Some(("PI82HI11KY", "-"))
        }
        "lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some(("PI82HI22KA", "-")),
        "1nsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
            Some(("PI82HI22KA11KY91KY", "-"))
        }
        "2sgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
            Some(("PI82HI22KA21KE81KE11KY91KY", "-"))
        }
        "3gkg3/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
            Some(("PI82HI22KA31GI71GI21KE81KE11KY91KY", "-"))
        }
        "4k4/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
            Some(("PI82HI22KA41KI61KI31GI71GI21KE81KE11KY91KY", "-"))
        }
        _ => None,
    }
}

/// `EngineInfo` から V3.0 の `'**` 行を `lines` へ追記する。
///
/// 解析済みの評価値行と、解析できずに退避した原文の双方を出力する。extras は
/// public に書き換えられるので、行として成立しない値は出力しない。
fn append_csa_analysis_lines(lines: &mut Vec<String>, info: &EngineInfo) {
    if let Some(eval) = info.eval() {
        let mut line = format!("'** {}", eval.raw());
        if let Some(EngineExtraValue::String(pv)) = info.extras().get(CSA_PV_EXTRA_KEY)
            && !pv.is_empty()
            && !pv.contains(['\n', '\r'])
        {
            line.push(' ');
            line.push_str(pv);
        }
        if let Some(nodes) = info.nodes() {
            line.push_str(&format!(" #{nodes}"));
        }
        lines.push(line);
    }
    if let Some(EngineExtraValue::String(raw)) = info.extras().get(CSA_ANALYSIS_RAW_EXTRA_KEY) {
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("'**") {
                lines.push(trimmed.to_string());
            }
        }
    }
}

/// [`Record`] を CSA 形式の文字列に変換する。
///
/// 既定の [`ExportOptions`] を使う。バージョンや文字コード宣言を指定する場合は
/// [`export_csa_with_options`] を使う。
pub fn export_csa(record: &Record) -> Result<String, CsaError> {
    export_csa_with_options(record, ExportOptions::default())
}

/// [`Record`] を CSA 形式の文字列に変換する。
///
/// `options` の `csa_version` が [`CsaVersion::V3_0`] のとき、先頭に
/// `'CSA encoding=...` 宣言を置き、端数のある消費時間をミリ秒表記で出力する。
pub fn export_csa_with_options(
    record: &Record,
    options: ExportOptions,
) -> Result<String, CsaError> {
    let version = options.csa_version();
    let mut pos = Position::empty();
    pos.set_sfen(record.init_position_sfen())?;

    let mut lines: Vec<String> = Vec::new();
    if version == CsaVersion::V3_0 {
        // V3.0 は宣言がない場合の既定を Shift_JIS と定めているので、宣言は省略できない。
        let encoding = match options.encoding() {
            TextEncoding::Utf8 => "UTF-8",
            TextEncoding::ShiftJis => "SHIFT_JIS",
        };
        lines.push(format!("'CSA encoding={encoding}"));
    }
    lines.push(version.header().to_string());
    append_csa_metadata(&mut lines, record.metadata());
    if let Some((pi, turn)) = csa_position_p_command(record.init_position_sfen()) {
        lines.push(pi.to_string());
        lines.push(turn.to_string());
    } else {
        append_csa_position_lines(&mut lines, &pos);
    }
    if let Some(comment) = record.initial_comment() {
        for line in comment.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            lines.push(format!("'*{trimmed}"));
        }
    }
    let mut refresh_counter = 0usize;

    for (index, node_id) in record.main_line_ids().into_iter().enumerate() {
        let node = record.node(node_id);
        let mv_record = node
            .mv()
            .ok_or_else(|| CsaError::InvalidMove(format!("main node missing move at {index}")))?;
        let mv16 = mv_record.mv();
        let mv = pos.move32_from_move(mv16);
        if !pos.is_legal_move(mv16) {
            return Err(CsaError::IllegalMove { index });
        }
        let csa = mv
            .to_csa()
            .ok_or_else(|| CsaError::InvalidMove(format!("invalid move at index {index}")))?;
        lines.push(csa);
        // 消費時間は仕様上省略可能なので、記録がないときに `T0` を捏造しない。
        if let Some(elapsed) = node.time_ms() {
            lines.push(format_csa_elapsed(elapsed, version));
        }
        if let Some(comment) = node.comment() {
            for line in comment.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                lines.push(format!("'*{trimmed}"));
            }
        }
        if let Some(info) = node.engine_info() {
            append_csa_analysis_lines(&mut lines, info);
        }
        pos.apply_move(mv16);
        refresh_position_if_needed(&mut pos, &mut refresh_counter)?;
    }

    if let Some(terminal_id) = record.main_terminal_node() {
        let terminal_node = record.node(terminal_id);
        let terminal = terminal_node.special().expect("terminal node has special entry");
        lines.push(marker_from_terminal(terminal));
        if let Some(time_ms) = terminal_node.time_ms() {
            lines.push(format_csa_elapsed(time_ms, version));
        }
        if let Some(comment) = terminal_node.comment() {
            for line in comment.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                lines.push(format!("'*{trimmed}"));
            }
        }
        if let Some(info) = terminal_node.engine_info() {
            append_csa_analysis_lines(&mut lines, info);
        }
    }
    Ok(format!("{}\n", lines.join("\n")))
}

/// [`Record`] を CSA 形式でエンコードしたバイト列に変換する。
///
/// # Examples
///
/// ```
/// use rsshogi::records::formats::csa;
/// use rsshogi::records::formats::common::TextEncoding;
///
/// # let csa_text = "V2.2\nPI\n+\n+7776FU\n%TORYO\n";
/// # let record = csa::parse_csa_str(csa_text).unwrap();
/// let encoded = csa::export_csa_bytes(&record, TextEncoding::Utf8).unwrap();
/// assert!(!encoded.has_unmappable_chars());
/// let text = std::str::from_utf8(encoded.bytes()).unwrap();
/// assert!(text.contains("+7776FU"));
/// ```
pub fn export_csa_bytes(record: &Record, encoding: TextEncoding) -> Result<EncodedText, CsaError> {
    let text = export_csa(record)?;
    Ok(encode_text(&text, encoding))
}

/// [`Record`] を CSA 形式でエンコードしたバイト列に変換する。
///
/// [`ExportOptions`] を受け取る拡張版。`encoding` と `csa_version` を解釈する。
pub fn export_csa_bytes_with_options(
    record: &Record,
    options: ExportOptions,
) -> Result<EncodedText, CsaError> {
    let text = export_csa_with_options(record, options)?;
    Ok(encode_text_with_options(&text, options))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::record::{MoveEntry, RecordNode};

    fn main_node(record: &Record, index: usize) -> &RecordNode {
        let node_id = record.main_line_ids()[index];
        record.node(node_id)
    }

    fn terminal_node(record: &Record) -> &RecordNode {
        record.node(record.main_terminal_node().expect("terminal"))
    }
    use crate::board::hirate_position;
    use crate::records::record::{RecordMetadata, SpecialMove};
    use crate::types::Move;

    #[test]
    fn test_csa_roundtrip_basic() {
        let pos = hirate_position();
        let mv = Move::from_usi("7g7f").unwrap();
        let record = Record::from_main_line_with_result(
            pos.to_sfen(None),
            vec![MoveEntry::new(mv)],
            GameResult::BlackWin,
        )
        .unwrap();

        let csa = export_csa(&record).unwrap();
        assert!(csa.ends_with('\n'));
        let parsed = parse_csa_str(&csa).unwrap();
        assert_eq!(parsed.main_moves().count(), 1);
        assert_eq!(parsed.result(), GameResult::BlackWin);
    }

    /// 駒落ちのように後手から始まる棋譜でも、writer は指し手が持つ色から正しい手番記号を出す。
    #[test]
    fn test_export_csa_signs_moves_for_white_first_record() {
        let text = "V2.2\nPI82HI22KA\n-\n-5152OU\nT0\n+7776FU\nT0\n%TORYO\n";
        let record = parse_csa_str(text).unwrap();
        let exported = export_csa(&record).unwrap();

        assert!(exported.contains("\n-5152OU\n"), "{exported}");
        assert!(exported.contains("\n+7776FU\n"), "{exported}");

        // 再パース・再出力で符号が二重化も欠落もしないこと。
        let reexported = export_csa(&parse_csa_str(&exported).unwrap()).unwrap();
        assert_eq!(exported, reexported);
    }

    /// 6 文字の終局マーカーは手番行と長さが近く、0 手棋譜で取りこぼしやすい。
    #[test]
    fn test_parse_csa_keeps_terminal_without_moves() {
        for (marker, expected) in [
            ("%TORYO", GameResult::WhiteWin),
            ("%TSUMI", GameResult::WhiteWin),
            ("%KACHI", GameResult::BlackWinByDeclaration),
            ("%ERROR", GameResult::Error),
        ] {
            let text = format!("V2.2\nPI\n+\n{marker}\n");
            let record = parse_csa_str(&text).unwrap();
            assert_eq!(record.main_moves().count(), 0, "{marker}");
            assert_eq!(record.result(), expected, "{marker}");
            assert!(export_csa(&record).unwrap().contains(marker), "{marker}");
        }
    }

    #[test]
    fn test_parse_csa_elapsed_ms_accepts_fraction_and_rejects_garbage() {
        assert_eq!(parse_csa_elapsed_ms("15"), Some(15_000));
        assert_eq!(parse_csa_elapsed_ms("15.1"), Some(15_100));
        assert_eq!(parse_csa_elapsed_ms("15.12"), Some(15_120));
        assert_eq!(parse_csa_elapsed_ms("15.123"), Some(15_123));
        assert_eq!(parse_csa_elapsed_ms("0.001"), Some(1));
        // 小数点以下は最大 3 桁。
        assert_eq!(parse_csa_elapsed_ms("15.1234"), None);
        assert_eq!(parse_csa_elapsed_ms("15."), None);
        assert_eq!(parse_csa_elapsed_ms("-1"), None);
        assert_eq!(parse_csa_elapsed_ms("abc"), None);
    }

    #[test]
    fn test_csa_millisecond_elapsed_time_roundtrip() {
        let record = parse_csa_str("V3.0\nPI\n+\n+7776FU\nT15.123\n%TORYO\n").unwrap();
        assert_eq!(main_node(&record, 0).time_ms(), Some(15_123));

        let v30 = ExportOptions::new(TextEncoding::Utf8).with_csa_version(CsaVersion::V3_0);
        let exported = export_csa_with_options(&record, v30).unwrap();
        assert!(exported.starts_with("'CSA encoding=UTF-8\nV3.0\n"), "{exported}");
        assert!(exported.contains("\nT15.123\n"), "{exported}");

        // V2.2 出力では小数を使わず秒へ丸める。
        let exported_v22 = export_csa(&record).unwrap();
        assert!(exported_v22.starts_with("V2.2\n"), "{exported_v22}");
        assert!(exported_v22.contains("\nT15\n"), "{exported_v22}");
    }

    /// 消費時間は仕様上省略可能なので、記録がない指し手に `T0` を作らない。
    #[test]
    fn test_export_csa_omits_elapsed_line_when_unrecorded() {
        let record = parse_csa_str("V2.2\nPI\n+\n+7776FU\n%TORYO\n").unwrap();
        assert_eq!(main_node(&record, 0).time_ms(), None);
        assert_eq!(export_csa(&record).unwrap(), "V2.2\nPI\n+\n+7776FU\n%TORYO\n");
    }

    #[test]
    fn test_csa_analysis_line_roundtrip() {
        let text = "V3.0\nPI\n+\n+7776FU\nT15\n'** 99 -8384FU +7776FU #1234\n%TORYO\n";
        let record = parse_csa_str(text).unwrap();
        let node = main_node(&record, 0);
        assert_eq!(node.eval(), Some(Eval::from_i32(99)));
        assert_eq!(node.nodes(), Some(1234));

        let exported = export_csa(&record).unwrap();
        assert!(exported.contains("\n'** 99 -8384FU +7776FU #1234\n"), "{exported}");
    }

    /// 評価値として読めない `'**` 行は原文のまま持ち回して往復で失わない。
    #[test]
    fn test_csa_unparsable_analysis_line_is_preserved() {
        let text = "V3.0\nPI\n+\n+7776FU\n'** ???garbage\n%TORYO\n";
        let record = parse_csa_str(text).unwrap();
        assert_eq!(main_node(&record, 0).eval(), None);
        assert!(export_csa(&record).unwrap().contains("\n'** ???garbage\n"));
    }

    /// 解析できる行とできない行が混ざっても、どちらも失わない。
    #[test]
    fn test_csa_mixed_analysis_lines_keep_both() {
        let text = "V3.0\nPI\n+\n+7776FU\n'** 50 -3334FU\n'** garbage\n'** more garbage\n%TORYO\n";
        let record = parse_csa_str(text).unwrap();
        assert_eq!(main_node(&record, 0).eval(), Some(Eval::from_i32(50)));

        let exported = export_csa(&record).unwrap();
        assert!(exported.contains("\n'** 50 -3334FU\n"), "{exported}");
        assert!(exported.contains("\n'** garbage\n"), "{exported}");
        assert!(exported.contains("\n'** more garbage\n"), "{exported}");
    }

    /// `Eval` は i16 なので、収まらない評価値は数値化せず原文で往復させる。
    #[test]
    fn test_csa_out_of_range_eval_is_kept_verbatim() {
        let record = parse_csa_str("V3.0\nPI\n+\n+7776FU\n'** 100000\n%TORYO\n").unwrap();
        assert_eq!(main_node(&record, 0).eval(), None);

        let exported = export_csa(&record).unwrap();
        assert!(exported.contains("\n'** 100000\n"), "{exported}");
        assert!(!exported.contains("32767"), "{exported}");
    }

    /// 数値として読めない `'**評価値=` も落とさず原文で持ち回す。
    #[test]
    fn test_csa_unparsable_legacy_eval_line_is_preserved() {
        let record = parse_csa_str("V2.2\nPI\n+\n+7776FU\n'**評価値=abc\n%TORYO\n").unwrap();
        assert_eq!(main_node(&record, 0).eval(), None);
        assert!(export_csa(&record).unwrap().contains("\n'**評価値=abc\n"));
    }

    /// 終局行の直後に付く `'**` も保持する。
    #[test]
    fn test_csa_analysis_line_after_terminal_is_preserved() {
        let record = parse_csa_str("V3.0\nPI\n+\n+7776FU\n%TORYO\n'** 30000 #99\n").unwrap();
        let terminal = terminal_node(&record);
        assert_eq!(terminal.eval(), Some(Eval::from_i32(30000)));
        assert_eq!(terminal.nodes(), Some(99));
        assert!(export_csa(&record).unwrap().contains("\n'** 30000 #99\n"));
    }

    /// `%ILLEGAL_MOVE` は手番側の反則負け、`%±ILLEGAL_ACTION` は記号側の反則負け。
    #[test]
    fn test_parse_csa_illegal_markers_assign_the_winner_per_spec() {
        let black_to_move = "V2.2\nPI\n+\n";
        let white_to_move = "V2.2\nPI\n+\n+7776FU\n";

        for (prefix, expected) in [
            (black_to_move, GameResult::WhiteWinByIllegalMove),
            (white_to_move, GameResult::BlackWinByIllegalMove),
        ] {
            let record = parse_csa_str(&format!("{prefix}%ILLEGAL_MOVE\n")).unwrap();
            assert_eq!(record.result(), expected);
        }

        // ILLEGAL_ACTION は反則側を記号が示すので、手番によらず勝敗が決まる。
        for prefix in [black_to_move, white_to_move] {
            let plus = parse_csa_str(&format!("{prefix}%+ILLEGAL_ACTION\n")).unwrap();
            assert_eq!(plus.result(), GameResult::WhiteWinByIllegalMove);
            let minus = parse_csa_str(&format!("{prefix}%-ILLEGAL_ACTION\n")).unwrap();
            assert_eq!(minus.result(), GameResult::BlackWinByIllegalMove);
        }
    }

    /// 時間切れも手番側の負けなので、勝つのは手番の相手。
    #[test]
    fn test_parse_csa_time_up_awards_the_game_to_the_opponent() {
        let black_to_move = "V2.2\nPI\n+\n";
        let white_to_move = "V2.2\nPI\n+\n+7776FU\n";

        for (prefix, expected) in [
            (black_to_move, GameResult::WhiteWinByTimeout),
            (white_to_move, GameResult::BlackWinByTimeout),
        ] {
            let record = parse_csa_str(&format!("{prefix}%TIME_UP\n")).unwrap();
            assert_eq!(record.result(), expected);
        }
    }

    /// 手番と食い違う手番記号は、符号を捨てて解釈せず拒否する。
    #[test]
    fn test_parse_csa_rejects_move_line_with_wrong_side_sign() {
        // 指し手自体は合法なので、原文を保持する専用の error で返す。
        assert!(matches!(
            parse_csa_str("V2.2\nPI\n+\n-7776FU\n%TORYO\n"),
            Err(CsaError::MoveSideMismatch(line)) if line == "-7776FU"
        ));
        assert!(matches!(
            parse_csa_str("V2.2\nPI\n+\n+7776FU\n+3334FU\n%TORYO\n"),
            Err(CsaError::MoveSideMismatch(line)) if line == "+3334FU"
        ));
        // 正しい符号は従来どおり通る。
        assert!(parse_csa_str("V2.2\nPI\n+\n+7776FU\n-3334FU\n%TORYO\n").is_ok());
    }

    #[test]
    fn test_parse_time_text_rejects_overflowing_values() {
        // u32 ミリ秒に収まらない秒数は、panic も wrap もせず不採用にする。
        assert_eq!(parse_csa_time_token("T4294968"), None);
        assert_eq!(parse_csa_time_token("T4294967295"), None);
        assert_eq!(parse_csa_time_token("T4294967"), Some(4_294_967_000));
    }

    /// 末尾セパレータの後ろに残るコメントを 1 局として数えない。
    #[test]
    fn test_parse_csa_games_ignores_trailing_comment_chunk() {
        let text = "V2.2\nPI\n+\n+7776FU\n%TORYO\n/\n'trailing comment\n";
        assert_eq!(parse_csa_games(text).unwrap().len(), 1);
    }

    /// 指し手のない第 1 局でも、次局の header を取り込まない。
    #[test]
    fn test_parse_csa_str_does_not_bleed_into_next_game() {
        let text = "V2.2\nPI\n+\n/\nV2.2\nPI\n+\n+2726FU\n%TORYO\n";
        let first = parse_csa_str(text).unwrap();
        assert_eq!(first.main_moves().count(), 0);
        assert_eq!(parse_csa_games(text).unwrap().len(), 2);
    }

    /// `%+ILLEGAL_ACTION` は kind へ畳むと戻せないので raw を優先して書き戻す。
    #[test]
    fn test_export_csa_preserves_raw_terminal_marker() {
        for marker in ["%+ILLEGAL_ACTION", "%-ILLEGAL_ACTION", "%ILLEGAL_MOVE"] {
            let text = format!("V2.2\nPI\n+\n+7776FU\n{marker}\n");
            let record = parse_csa_str(&text).unwrap();
            let exported = export_csa(&record).unwrap();
            assert!(exported.contains(&format!("\n{marker}\n")), "{marker}: {exported}");
        }
    }

    #[test]
    fn test_export_csa_v30_declares_shift_jis_encoding() {
        let record = parse_csa_str("V2.2\nPI\n+\n+7776FU\n%TORYO\n").unwrap();
        let options = ExportOptions::new(TextEncoding::ShiftJis).with_csa_version(CsaVersion::V3_0);
        let exported = export_csa_with_options(&record, options).unwrap();
        assert!(exported.starts_with("'CSA encoding=SHIFT_JIS\nV3.0\n"), "{exported}");
    }

    #[test]
    fn test_parse_csa_accepts_timet_key_spelling() {
        let text = "V3.0\n$TIMET+:450+0+5\n$TIMET-:900+0+5\nPI\n+\n+7776FU\n%TORYO\n";
        let record = parse_csa_str(text).unwrap();
        assert!(record.metadata().black_time_control().is_some());
        assert!(record.metadata().white_time_control().is_some());

        // 書き出しは仕様の例に合わせた綴りへ正規化する。
        let exported = export_csa(&record).unwrap();
        assert!(exported.contains("\n$TIME+:450+0+5\n"), "{exported}");
        assert!(!exported.contains("$TIMET+"), "{exported}");
    }

    #[test]
    fn test_parse_csa_games_splits_on_separator() {
        let text = "V2.2\nPI\n+\n+7776FU\nT15\n%TORYO\n/\nV2.2\nPI\n+\n+2726FU\nT9\n%TORYO\n";
        let games = parse_csa_games(text).unwrap();
        assert_eq!(games.len(), 2);
        assert_eq!(games[0].main_moves().count(), 1);
        assert_eq!(games[1].main_moves().count(), 1);

        // 単一棋譜 API は最初の 1 局を返し、セパレータでエラーにしない。
        let first = parse_csa_str(text).unwrap();
        assert_eq!(first.main_moves().count(), 1);
    }

    /// 終局マーカーがないまま `/` に達しても、エラーにせず 1 局として閉じる。
    #[test]
    fn test_parse_csa_stops_at_separator_without_terminal() {
        let record = parse_csa_str("V2.2\nPI\n+\n+7776FU\n/\n").unwrap();
        assert_eq!(record.main_moves().count(), 1);
        assert!(record.main_terminal_node().is_none());
    }

    #[test]
    fn test_parse_csa_with_commas_and_comments() {
        let csa = "\
V2.2
N+Alice
N-Bob
$EVENT:TestEvent
PI
+
+7776FU,T12
'*初手コメント
-3334FU,T6
%TORYO
";
        let record = parse_csa_str(csa).unwrap();
        assert_eq!(record.metadata().black_player(), Some("Alice"));
        assert_eq!(record.metadata().white_player(), Some("Bob"));
        assert_eq!(record.metadata().event(), Some("TestEvent"));
        assert_eq!(record.main_moves().count(), 2);
        assert_eq!(record.moves()[0].mv().to_usi(), "7g7f");
        assert_eq!(main_node(&record, 0).time_ms(), Some(12_000));
        assert_eq!(main_node(&record, 0).comment(), Some("初手コメント"));
        assert_eq!(record.moves()[1].mv().to_usi(), "3c3d");
        assert_eq!(main_node(&record, 1).time_ms(), Some(6_000));
        assert_eq!(record.moves().last().unwrap().mv().to_usi(), "3c3d");
        assert_eq!(record.result(), GameResult::WhiteWin);
    }

    #[test]
    fn test_parse_csa_preserves_initial_comment_after_side_to_move() {
        let csa = "\
V2.2
'対局メモ
'*序文0
PI
+
'*序文1
'*序文2
+7776FU
%TORYO
";
        let record = parse_csa_str(csa).unwrap();
        assert_eq!(record.metadata().comment(), None);
        assert_eq!(record.initial_comment(), Some("序文0\n序文1\n序文2"));
        assert_eq!(main_node(&record, 0).comment(), None);
    }

    #[test]
    fn test_parse_csa_maps_note_to_metadata_comment() {
        let csa = "\
V2.2
$NOTE:備考1\\n備考2\\\\
PI
+
+7776FU
%TORYO
";
        let record = parse_csa_str(csa).unwrap();
        assert_eq!(record.metadata().comment(), Some("備考1\n備考2\\"));
    }

    #[test]
    fn test_parse_csa_preserves_multiline_comments_on_move() {
        let csa = "\
V2.2
PI
+
+7776FU
'*コメント1
'*コメント2
%TORYO
";
        let record = parse_csa_str(csa).unwrap();
        assert_eq!(main_node(&record, 0).comment(), Some("コメント1\nコメント2"));
    }

    #[test]
    fn test_parse_csa_terminal_inline_time_and_side_time_settings() {
        let csa = "\
V2.2
$TIME+:600+30+0
$TIME-:300+10+0
$MAX_MOVES:256
$JISHOGI:24点法
PI
+
+7776FU
-3334FU
%TSUMI,T0
'*終局コメント1
'*終局コメント2
";
        let record = parse_csa_str(csa).unwrap();

        let black_tc = record.metadata().black_time_control().expect("black time control");
        assert_eq!(black_tc.base_seconds(), 600);
        assert_eq!(black_tc.byoyomi_seconds(), 30);
        assert_eq!(black_tc.increment_seconds(), 0);

        let white_tc = record.metadata().white_time_control().expect("white time control");
        assert_eq!(white_tc.base_seconds(), 300);
        assert_eq!(white_tc.byoyomi_seconds(), 10);
        assert_eq!(white_tc.increment_seconds(), 0);

        assert_eq!(record.metadata().max_moves(), Some(256));
        assert_eq!(record.metadata().impasse_rule(), Some("24点法"));

        let terminal = record.main_terminal().expect("terminal");
        assert_eq!(terminal.kind(), &SpecialMove::Mate);
        assert_eq!(terminal.result(), GameResult::WhiteWin);
        let terminal_node = terminal_node(&record);
        assert_eq!(terminal_node.time_ms(), Some(0));
        assert_eq!(terminal_node.comment(), Some("終局コメント1\n終局コメント2"));
    }

    #[test]
    fn test_parse_csa_extended_comments_without_asterisk_prefix_in_result() {
        let csa = "\
V2.2
PI
+
+7776FU
'*初手コメント
-3334FU
%TORYO
'*終局コメント
";
        let record = parse_csa_str(csa).unwrap();
        assert_eq!(main_node(&record, 0).comment(), Some("初手コメント"));
        assert_eq!(terminal_node(&record).comment(), Some("終局コメント"));
    }

    #[test]
    fn test_parse_csa_ignores_plain_comments() {
        let csa = "\
V2.2
'ヘッダコメント
PI
+
'開始局面コメント
+7776FU
'指し手コメント
%TORYO
'終局コメント
";
        let record = parse_csa_str(csa).unwrap();
        assert_eq!(record.initial_comment(), None);
        assert_eq!(main_node(&record, 0).comment(), None);
        assert_eq!(terminal_node(&record).comment(), None);
    }

    #[test]
    fn test_export_csa_roundtrip_preserves_extended_comments() {
        let pos = hirate_position();
        let mv = Move::from_usi("7g7f").unwrap();
        let mut record = Record::new(pos.to_sfen(None)).unwrap();
        record
            .append_move_with_annotation(
                record.root_id(),
                MoveEntry::new(mv),
                RecordAnnotation::new().with_comment(Some("opening".to_string())),
            )
            .unwrap();
        record
            .set_main_terminal_with_annotation(
                SpecialMoveEntry::new(SpecialMove::Resign, GameResult::BlackWin),
                RecordAnnotation::new().with_comment(Some("terminal".to_string())),
            )
            .unwrap();

        let csa = export_csa(&record).unwrap();
        assert!(csa.contains("'*opening"));
        assert!(csa.contains("'*terminal"));

        let parsed = parse_csa_str(&csa).unwrap();
        assert_eq!(main_node(&parsed, 0).comment(), Some("opening"));
        assert_eq!(terminal_node(&parsed).comment(), Some("terminal"));
    }

    #[test]
    fn test_export_csa_writes_metadata_comment_as_note() {
        let pos = hirate_position();
        let mut record = Record::from_main_line(
            pos.to_sfen(None),
            vec![MoveEntry::new(Move::from_usi("7g7f").unwrap())],
            None,
        )
        .unwrap();
        let mut metadata_builder = RecordMetadata::builder();
        metadata_builder.comment(Some("備考1\n備考2\\".to_string()));
        record.set_metadata(metadata_builder.build());

        let csa = export_csa(&record).unwrap();
        assert!(csa.contains("$NOTE:備考1\\n備考2\\\\"));
        assert!(!csa.contains("'備考1"));
    }

    #[test]
    fn test_export_csa_writes_initial_comment_before_first_move() {
        let pos = hirate_position();
        let mut record = Record::from_main_line(
            pos.to_sfen(None),
            vec![MoveEntry::new(Move::from_usi("7g7f").unwrap())],
            None,
        )
        .unwrap();
        record.set_initial_comment(Some("序文1\n序文2".to_string()));

        let csa = export_csa(&record).unwrap();
        assert!(csa.contains("+\n'*序文1\n'*序文2\n+7776FU"));
    }

    #[test]
    fn test_parse_csa_without_end_marker() {
        let csa = "\
V2.2
PI
+
+7776FU
-3334FU
";

        let record = parse_csa_str(csa).unwrap();
        assert_eq!(record.main_moves().count(), 2);
        assert_eq!(record.moves()[0].mv().to_usi(), "7g7f");
        assert_eq!(record.moves()[1].mv().to_usi(), "3c3d");
        assert!(record.main_terminal().is_none());
        assert_eq!(record.result(), GameResult::Invalid);
    }

    #[test]
    fn test_parse_csa_rejects_non_ascii_move_without_panic() {
        let csa = "\
V2.2
PI
+
+aaaaあ
%TORYO
";

        assert!(matches!(parse_csa_str(csa), Err(CsaError::InvalidMove(_))));
    }

    #[test]
    fn test_export_csa_without_terminal() {
        let pos = hirate_position();
        let mut record = Record::new(pos.to_sfen(None)).unwrap();
        let first = record
            .append_move_with_annotation(
                record.root_id(),
                MoveEntry::new(Move::from_usi("7g7f").unwrap()),
                RecordAnnotation::new().with_elapsed_ms(Some(12_000)),
            )
            .unwrap();
        record.append_move(first, MoveEntry::new(Move::from_usi("3c3d").unwrap())).unwrap();

        let csa = export_csa(&record).unwrap();
        assert!(csa.contains("+7776FU"));
        assert!(csa.contains("-3334FU"));
        assert!(csa.contains("T12"));
        assert!(!csa.contains("%TORYO"));
        assert!(!csa.contains("%KACHI"));
        assert!(csa.ends_with('\n'));
    }

    // --- フェーズ 2 受け入れテスト ---

    /// A07: UTF-8 CSA bytes の解析
    #[test]
    fn test_parse_csa_bytes_utf8() {
        let csa = "\
V2.2
N+先手
N-後手
PI
+
+7776FU
%TORYO
";
        let record = parse_csa_bytes(csa.as_bytes()).unwrap();
        assert_eq!(record.main_moves().count(), 1);
        assert_eq!(record.metadata().black_player(), Some("先手"));
        assert_eq!(record.metadata().white_player(), Some("後手"));
    }

    /// A08: Shift_JIS CSA bytes の解析
    #[test]
    fn test_parse_csa_bytes_shift_jis() {
        let csa_utf8 = "\
V2.2
N+先手
N-後手
PI
+
+7776FU
%TORYO
";
        let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode(csa_utf8);
        let record = parse_csa_bytes(&encoded).unwrap();
        assert_eq!(record.main_moves().count(), 1);
        assert_eq!(record.metadata().black_player(), Some("先手"));
    }

    /// A10: CSA bytes の Shift_JIS エクスポート。
    #[test]
    fn test_export_csa_bytes_shift_jis() {
        let pos = hirate_position();
        let mv = Move::from_usi("7g7f").unwrap();
        let record = Record::from_main_line_with_result(
            pos.to_sfen(None),
            vec![MoveEntry::new(mv)],
            GameResult::BlackWin,
        )
        .unwrap();

        let encoded = export_csa_bytes(&record, TextEncoding::ShiftJis).unwrap();
        assert!(!encoded.has_unmappable_chars());
        // Shift_JIS バイト列を再解析できることを確認する。
        let parsed = parse_csa_bytes(encoded.bytes()).unwrap();
        assert_eq!(parsed.main_moves().count(), 1);
    }

    /// A13: 正規エクスポーターの方針。出力が決定的であること。
    #[test]
    fn test_export_csa_deterministic() {
        let pos = hirate_position();
        let mv = Move::from_usi("7g7f").unwrap();
        let record = Record::from_main_line_with_result(
            pos.to_sfen(None),
            vec![MoveEntry::new(mv)],
            GameResult::BlackWin,
        )
        .unwrap();

        let output1 = export_csa(&record).unwrap();
        let output2 = export_csa(&record).unwrap();
        assert_eq!(output1, output2);
    }

    #[test]
    fn test_export_csa_bytes_with_options() {
        let pos = hirate_position();
        let mv = Move::from_usi("7g7f").unwrap();
        let record = Record::from_main_line_with_result(
            pos.to_sfen(None),
            vec![MoveEntry::new(mv)],
            GameResult::BlackWin,
        )
        .unwrap();

        let encoded =
            export_csa_bytes_with_options(&record, ExportOptions::new(TextEncoding::Utf8)).unwrap();
        assert!(!encoded.has_unmappable_chars());
        assert!(std::str::from_utf8(encoded.bytes()).unwrap().contains("+7776FU"));
    }
}
