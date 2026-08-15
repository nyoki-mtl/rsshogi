use crate::board::{self, HuffmanCodedPos, HuffmanCodedPosError, Position, SfenError};
use crate::records::{MoveEntry, Record, RecordError, SpecialMove, SpecialMoveEntry};
use crate::types::{AperyMove, Eval, GameResult, Move};
use core::fmt;
use std::io::{Read, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackStartPosition {
    Startpos,
    Hcp { hcp: HuffmanCodedPos, game_ply: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackPly {
    pub mv: AperyMove,
    pub eval: i16,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackGameResult {
    Draw = 0,
    BlackWin = 1,
    WhiteWin = 2,
}

impl PackGameResult {
    pub const fn terminal_marker(self) -> u16 {
        let value = self as u16;
        value | (value << 7)
    }
}

impl TryFrom<u8> for PackGameResult {
    type Error = PackError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Draw),
            1 => Ok(Self::BlackWin),
            2 => Ok(Self::WhiteWin),
            other => Err(PackError::InvalidGameResult(other)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackEndReason {
    Resign,
    RepetitionDraw,
    MaxMoves,
    Interrupt,
    TimeUp,
    IllegalMove,
    RepetitionCheck,
    WinCsa24,
    DrawCsa24,
    WinCsa27,
    WinTryRule,
    Reserved(u8),
}

impl PackEndReason {
    pub const fn to_byte(self) -> u8 {
        match self {
            Self::Resign => 0,
            Self::RepetitionDraw => 1,
            Self::MaxMoves => 2,
            Self::Interrupt => 3,
            Self::TimeUp => 4,
            Self::IllegalMove => 5,
            Self::RepetitionCheck => 6,
            Self::WinCsa24 => 10,
            Self::DrawCsa24 => 11,
            Self::WinCsa27 => 12,
            Self::WinTryRule => 13,
            Self::Reserved(value) => value,
        }
    }

    pub const fn from_byte(value: u8) -> Self {
        match value {
            0 => Self::Resign,
            1 => Self::RepetitionDraw,
            2 => Self::MaxMoves,
            3 => Self::Interrupt,
            4 => Self::TimeUp,
            5 => Self::IllegalMove,
            6 => Self::RepetitionCheck,
            10 => Self::WinCsa24,
            11 => Self::DrawCsa24,
            12 => Self::WinCsa27,
            13 => Self::WinTryRule,
            other => Self::Reserved(other),
        }
    }

    pub const fn raw_label(self) -> &'static str {
        match self {
            Self::Resign => "resign",
            Self::RepetitionDraw => "draw",
            Self::MaxMoves => "max_moves",
            Self::Interrupt => "interrupt",
            Self::TimeUp => "time_up",
            Self::IllegalMove => "illegal_move",
            Self::RepetitionCheck => "repetition_check",
            Self::WinCsa24 => "win_csa24",
            Self::DrawCsa24 => "draw_csa24",
            Self::WinCsa27 => "win_csa27",
            Self::WinTryRule => "win_try_rule",
            Self::Reserved(_) => "reserved",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackGame {
    pub start_position: PackStartPosition,
    pub plies: Vec<PackPly>,
    pub result: PackGameResult,
    pub end_reason: PackEndReason,
}

#[derive(Debug)]
pub enum PackError {
    Truncated,
    InvalidStartState(u8),
    InvalidTerminalMarker(u16),
    InvalidGameResult(u8),
    InvalidMove { index: usize, raw: u16 },
    IllegalMove { index: usize, raw: u16, sfen: String },
    MissingEval(usize),
    MissingTerminal,
    Hcp(HuffmanCodedPosError),
    Sfen(SfenError),
    Record(RecordError),
    InconsistentOutcome { result: PackGameResult, reason: PackEndReason },
    Io(std::io::Error),
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => f.write_str("truncated PACK game"),
            Self::InvalidStartState(value) => write!(f, "invalid PACK start tag {value}"),
            Self::InvalidTerminalMarker(value) => write!(f, "invalid PACK terminal marker {value}"),
            Self::InvalidGameResult(value) => write!(f, "invalid PACK game result {value}"),
            Self::InvalidMove { index, raw } => write!(f, "invalid PACK move {index}: {raw}"),
            Self::IllegalMove { index, raw, sfen } => {
                write!(f, "illegal PACK move {index}: {raw} at {sfen}")
            }
            Self::MissingEval(index) => write!(f, "missing evaluation for move {index}"),
            Self::MissingTerminal => f.write_str("missing PACK terminal"),
            Self::Hcp(error) => error.fmt(f),
            Self::Sfen(error) => error.fmt(f),
            Self::Record(error) => error.fmt(f),
            Self::InconsistentOutcome { result, reason } => {
                write!(f, "PACK result {result:?} is inconsistent with {}", reason.raw_label())
            }
            Self::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for PackError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hcp(error) => Some(error),
            Self::Sfen(error) => Some(error),
            Self::Record(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<std::io::Error> for PackError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<SfenError> for PackError {
    fn from(value: SfenError) -> Self {
        Self::Sfen(value)
    }
}
impl From<HuffmanCodedPosError> for PackError {
    fn from(value: HuffmanCodedPosError) -> Self {
        Self::Hcp(value)
    }
}
impl From<RecordError> for PackError {
    fn from(value: RecordError) -> Self {
        Self::Record(value)
    }
}

fn marker_result(marker: u16) -> Option<PackGameResult> {
    [PackGameResult::Draw, PackGameResult::BlackWin, PackGameResult::WhiteWin]
        .into_iter()
        .find(|result| result.terminal_marker() == marker)
}

fn valid_move(raw: u16) -> bool {
    let mv = Move::from_raw(raw);
    if !mv.is_normal() || mv.to_sq().raw() > 80 {
        return false;
    }
    if mv.is_drop() {
        !mv.is_promotion() && matches!((raw >> 7) & 0x7f, 1..=7)
    } else {
        mv.from_sq().raw() <= 80 && mv.from_sq() != mv.to_sq()
    }
}

fn valid_apery_move(raw: u16) -> bool {
    if raw & (1 << 15) != 0 {
        return false;
    }
    let to = raw & 0x7f;
    let from = (raw >> 7) & 0x7f;
    if to > 80 {
        return false;
    }
    if from >= 81 { from <= 87 && raw & (1 << 14) == 0 } else { from != to }
}

pub fn encode_game(game: &PackGame) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + game.plies.len() * 4 + 35);
    match game.start_position {
        PackStartPosition::Startpos => out.push(1),
        PackStartPosition::Hcp { hcp, game_ply } => {
            out.push(0);
            out.extend_from_slice(&hcp.data);
            out.extend_from_slice(&game_ply.to_le_bytes());
        }
    }
    for ply in &game.plies {
        out.extend_from_slice(&ply.mv.raw().to_le_bytes());
        out.extend_from_slice(&ply.eval.to_le_bytes());
    }
    out.extend_from_slice(&game.result.terminal_marker().to_le_bytes());
    out.push(game.end_reason.to_byte());
    out
}

pub fn decode_game(input: &[u8]) -> Result<(PackGame, usize), PackError> {
    let Some(&tag) = input.first() else { return Err(PackError::Truncated) };
    let mut offset = 1;
    let start_position = match tag {
        1 => PackStartPosition::Startpos,
        0 => {
            if input.len() < offset + 34 {
                return Err(PackError::Truncated);
            }
            let mut data = [0; 32];
            data.copy_from_slice(&input[offset..offset + 32]);
            offset += 32;
            let game_ply =
                u16::from_le_bytes(input[offset..offset + 2].try_into().expect("two bytes"));
            offset += 2;
            let hcp = HuffmanCodedPos { data };
            let mut position = Position::empty();
            position.set_huffman_coded_pos(&hcp, game_ply)?;
            PackStartPosition::Hcp { hcp, game_ply }
        }
        other => return Err(PackError::InvalidStartState(other)),
    };
    let mut plies = Vec::new();
    loop {
        if input.len() < offset + 2 {
            return Err(PackError::MissingTerminal);
        }
        let raw = u16::from_le_bytes(input[offset..offset + 2].try_into().expect("two bytes"));
        if let Some(result) = marker_result(raw) {
            if input.len() < offset + 3 {
                return Err(PackError::Truncated);
            }
            let end_reason = PackEndReason::from_byte(input[offset + 2]);
            if !reason_consistent(result, end_reason) {
                return Err(PackError::InconsistentOutcome { result, reason: end_reason });
            }
            return Ok((PackGame { start_position, plies, result, end_reason }, offset + 3));
        }
        if !valid_apery_move(raw) {
            return Err(PackError::InvalidMove { index: plies.len(), raw });
        }
        if input.len() < offset + 4 {
            return Err(PackError::MissingEval(plies.len()));
        }
        let eval = i16::from_le_bytes(input[offset + 2..offset + 4].try_into().expect("two bytes"));
        plies.push(PackPly { mv: AperyMove::from_raw(raw), eval });
        offset += 4;
    }
}

pub fn decode_games<R: Read>(reader: &mut R) -> Result<Vec<PackGame>, PackError> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    let mut games = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let (game, used) = decode_game(&bytes[offset..])?;
        games.push(game);
        offset += used;
    }
    Ok(games)
}

pub fn write_games<W: Write>(writer: &mut W, games: &[PackGame]) -> Result<(), PackError> {
    for game in games {
        writer.write_all(&encode_game(game))?;
    }
    Ok(())
}

fn start_position(game: &PackGame) -> Result<Position, PackError> {
    match game.start_position {
        PackStartPosition::Startpos => Ok(board::hirate_position()),
        PackStartPosition::Hcp { hcp, game_ply } => {
            let mut position = Position::empty();
            position.set_huffman_coded_pos(&hcp, game_ply)?;
            Ok(position)
        }
    }
}

fn result_to_game(result: GameResult) -> Option<PackGameResult> {
    match result {
        GameResult::BlackWin
        | GameResult::BlackWinByDeclaration
        | GameResult::BlackWinByForfeit
        | GameResult::BlackWinByIllegalMove
        | GameResult::BlackWinByTryRule
        | GameResult::BlackWinByTimeout => Some(PackGameResult::BlackWin),
        GameResult::WhiteWin
        | GameResult::WhiteWinByDeclaration
        | GameResult::WhiteWinByForfeit
        | GameResult::WhiteWinByIllegalMove
        | GameResult::WhiteWinByTryRule
        | GameResult::WhiteWinByTimeout => Some(PackGameResult::WhiteWin),
        GameResult::DrawByRepetition
        | GameResult::DrawByMaxPlies
        | GameResult::DrawByImpasse
        | GameResult::Paused => Some(PackGameResult::Draw),
        GameResult::Error | GameResult::Invalid => None,
    }
}

fn reason_from_special(record: &Record, special: &SpecialMoveEntry) -> PackEndReason {
    if let Some(raw) = special.raw().and_then(|raw| raw.strip_prefix("pack:end_reason=")) {
        let parsed = match raw {
            "resign" => Some(PackEndReason::Resign),
            "draw" => Some(PackEndReason::RepetitionDraw),
            "max_moves" => Some(PackEndReason::MaxMoves),
            "interrupt" => Some(PackEndReason::Interrupt),
            "time_up" => Some(PackEndReason::TimeUp),
            "illegal_move" => Some(PackEndReason::IllegalMove),
            "repetition_check" => Some(PackEndReason::RepetitionCheck),
            "win_csa24" => Some(PackEndReason::WinCsa24),
            "draw_csa24" => Some(PackEndReason::DrawCsa24),
            "win_csa27" => Some(PackEndReason::WinCsa27),
            "win_try_rule" => Some(PackEndReason::WinTryRule),
            _ => raw.parse::<u8>().ok().map(PackEndReason::from_byte),
        };
        if let Some(reason) = parsed {
            return reason;
        }
    }
    match special.result() {
        GameResult::BlackWinByDeclaration | GameResult::WhiteWinByDeclaration => {
            if record.metadata().impasse_rule().is_some_and(|rule| rule.starts_with("CSARule24")) {
                return PackEndReason::WinCsa24;
            }
            return PackEndReason::WinCsa27;
        }
        GameResult::DrawByImpasse => return PackEndReason::DrawCsa24,
        GameResult::BlackWinByTryRule | GameResult::WhiteWinByTryRule => {
            return PackEndReason::WinTryRule;
        }
        _ => {}
    }
    match special.kind() {
        SpecialMove::Resign => PackEndReason::Resign,
        SpecialMove::RepetitionDraw => PackEndReason::RepetitionDraw,
        SpecialMove::MaxMoves => PackEndReason::MaxMoves,
        SpecialMove::Interrupt => PackEndReason::Interrupt,
        SpecialMove::Timeout => PackEndReason::TimeUp,
        SpecialMove::WinByIllegalMove | SpecialMove::LoseByIllegalMove => {
            PackEndReason::IllegalMove
        }
        SpecialMove::Try => PackEndReason::WinTryRule,
        _ => PackEndReason::Reserved(255),
    }
}

fn reason_consistent(result: PackGameResult, reason: PackEndReason) -> bool {
    match reason {
        PackEndReason::RepetitionDraw
        | PackEndReason::MaxMoves
        | PackEndReason::Interrupt
        | PackEndReason::DrawCsa24 => result == PackGameResult::Draw,
        PackEndReason::Resign
        | PackEndReason::TimeUp
        | PackEndReason::IllegalMove
        | PackEndReason::RepetitionCheck
        | PackEndReason::WinCsa24
        | PackEndReason::WinCsa27
        | PackEndReason::WinTryRule => result != PackGameResult::Draw,
        PackEndReason::Reserved(_) => true,
    }
}

fn special_for(result: PackGameResult, reason: PackEndReason) -> SpecialMoveEntry {
    let game_result = match (result, reason) {
        (PackGameResult::Draw, PackEndReason::MaxMoves) => GameResult::DrawByMaxPlies,
        (PackGameResult::Draw, PackEndReason::Interrupt) => GameResult::Paused,
        (PackGameResult::Draw, PackEndReason::DrawCsa24) => GameResult::DrawByImpasse,
        (PackGameResult::Draw, _) => GameResult::DrawByRepetition,
        (PackGameResult::BlackWin, PackEndReason::TimeUp) => GameResult::BlackWinByTimeout,
        (PackGameResult::WhiteWin, PackEndReason::TimeUp) => GameResult::WhiteWinByTimeout,
        (PackGameResult::BlackWin, PackEndReason::IllegalMove) => GameResult::BlackWinByIllegalMove,
        (PackGameResult::WhiteWin, PackEndReason::IllegalMove) => GameResult::WhiteWinByIllegalMove,
        (PackGameResult::BlackWin, PackEndReason::WinTryRule) => GameResult::BlackWinByTryRule,
        (PackGameResult::WhiteWin, PackEndReason::WinTryRule) => GameResult::WhiteWinByTryRule,
        (PackGameResult::BlackWin, PackEndReason::WinCsa24 | PackEndReason::WinCsa27) => {
            GameResult::BlackWinByDeclaration
        }
        (PackGameResult::WhiteWin, PackEndReason::WinCsa24 | PackEndReason::WinCsa27) => {
            GameResult::WhiteWinByDeclaration
        }
        (PackGameResult::BlackWin, _) => GameResult::BlackWin,
        (PackGameResult::WhiteWin, _) => GameResult::WhiteWin,
    };
    let kind = match reason {
        PackEndReason::Resign => SpecialMove::Resign,
        PackEndReason::RepetitionDraw => SpecialMove::RepetitionDraw,
        PackEndReason::DrawCsa24 => SpecialMove::Impasse,
        PackEndReason::MaxMoves => SpecialMove::MaxMoves,
        PackEndReason::Interrupt => SpecialMove::Interrupt,
        PackEndReason::TimeUp => SpecialMove::Timeout,
        PackEndReason::IllegalMove | PackEndReason::RepetitionCheck => {
            SpecialMove::WinByIllegalMove
        }
        PackEndReason::WinTryRule => SpecialMove::Try,
        PackEndReason::WinCsa24 | PackEndReason::WinCsa27 => SpecialMove::WinByDeclaration,
        _ => SpecialMove::Unknown(reason.raw_label().to_string()),
    };
    let raw = match reason {
        PackEndReason::Reserved(value) => format!("pack:end_reason={value}"),
        _ => format!("pack:end_reason={}", reason.raw_label()),
    };
    SpecialMoveEntry::new(kind, game_result).with_raw(Some(raw))
}

pub fn record_from_game(game: &PackGame) -> Result<Record, PackError> {
    if !reason_consistent(game.result, game.end_reason) {
        return Err(PackError::InconsistentOutcome {
            result: game.result,
            reason: game.end_reason,
        });
    }
    let mut position = start_position(game)?;
    let mut record = Record::new(position.to_sfen(None))?;
    let mut parent = record.root_id();
    for (index, ply) in game.plies.iter().enumerate() {
        let raw = ply.mv.raw();
        if !valid_apery_move(raw) {
            return Err(PackError::InvalidMove { index, raw });
        }
        let mv = ply.mv.to_move();
        if !position.is_legal_move(mv) {
            return Err(PackError::IllegalMove { index, raw, sfen: position.to_sfen(None) });
        }
        let node = record.append_move(parent, MoveEntry::new(mv))?;
        record.node_mut(node)?.set_eval(Some(Eval::from_raw(ply.eval)));
        position.apply_move(mv);
        parent = node;
    }
    record.set_main_terminal(special_for(game.result, game.end_reason))?;
    Ok(record)
}

pub fn game_from_record(record: &Record) -> Result<PackGame, PackError> {
    let mut position = Position::from_sfen(record.init_position_sfen())?;
    let start_position = if record.init_position_sfen() == board::STARTPOS_SFEN {
        PackStartPosition::Startpos
    } else {
        PackStartPosition::Hcp {
            hcp: position.to_huffman_coded_pos(),
            game_ply: position.game_ply(),
        }
    };
    let mut plies = Vec::new();
    for (index, node_id) in record.main_line_ids().into_iter().enumerate() {
        let node = record.node(node_id);
        let Some(entry) = node.mv() else { continue };
        let mv = entry.mv();
        let raw = mv.raw();
        if !valid_move(raw) {
            return Err(PackError::InvalidMove { index, raw });
        }
        if !position.is_legal_move(mv) {
            return Err(PackError::IllegalMove { index, raw, sfen: position.to_sfen(None) });
        }
        let eval = clamp_pack_eval(node.eval().ok_or(PackError::MissingEval(index))?.raw());
        plies.push(PackPly { mv: mv.to_apery(), eval });
        position.apply_move(mv);
    }
    let terminal = record.main_terminal().ok_or(PackError::MissingTerminal)?;
    let result = result_to_game(terminal.result()).ok_or(PackError::MissingTerminal)?;
    let end_reason = reason_from_special(record, terminal);
    if !reason_consistent(result, end_reason) {
        return Err(PackError::InconsistentOutcome { result, reason: end_reason });
    }
    Ok(PackGame { start_position, plies, result, end_reason })
}

pub fn records_from_bytes(input: &[u8]) -> Result<Vec<Record>, PackError> {
    let mut cursor = std::io::Cursor::new(input);
    decode_games(&mut cursor)?.iter().map(record_from_game).collect()
}

pub fn games_from_records(records: &[Record]) -> Result<Vec<PackGame>, PackError> {
    records.iter().map(game_from_record).collect()
}

const fn clamp_pack_eval(eval: i16) -> i16 {
    if eval < Eval::CP_MIN {
        Eval::CP_MIN
    } else if eval > Eval::CP_MAX {
        Eval::CP_MAX
    } else {
        eval
    }
}
