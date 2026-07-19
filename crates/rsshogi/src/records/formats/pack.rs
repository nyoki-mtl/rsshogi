use crate::board::{
    HuffmanCodedPos, HuffmanCodedPosError, Position, SfenError, hirate_position, position_from_sfen,
};
use crate::records::record::{
    AnnotatedMoveEntry, Record, RecordAnnotation, RecordMetadata, SpecialMove, SpecialMoveEntry,
};
use crate::types::{AperyMove, Eval, GameResult};
use std::fmt;
use std::io::{self, Read, Write};

const START_POSITION_HCP: u8 = 0;
const START_POSITION_STARTPOS: u8 = 1;
const PACK_RAW_PREFIX: &str = "pack:end_reason=";

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
    #[must_use]
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
            _ => Err(PackError::InvalidGameResult(value)),
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
    #[must_use]
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

    #[must_use]
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

    #[must_use]
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
    Record(crate::records::error::RecordError),
    InconsistentOutcome { result: PackGameResult, reason: PackEndReason },
    Io(io::Error),
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated => write!(f, "truncated pack data"),
            Self::InvalidStartState(state) => write!(f, "invalid pack start state: {state}"),
            Self::InvalidTerminalMarker(raw) => {
                write!(f, "invalid pack terminal marker: 0x{raw:04x}")
            }
            Self::InvalidGameResult(value) => write!(f, "invalid pack game result: {value}"),
            Self::InvalidMove { index, raw } => {
                write!(f, "invalid pack move at ply {index}: 0x{raw:04x}")
            }
            Self::IllegalMove { index, raw, sfen } => {
                write!(f, "illegal pack move at ply {index}: 0x{raw:04x} on {sfen}")
            }
            Self::MissingEval(index) => write!(f, "missing eval for record move {index}"),
            Self::MissingTerminal => write!(f, "pack export requires a terminal result"),
            Self::Hcp(err) => write!(f, "{err}"),
            Self::Sfen(err) => write!(f, "{err}"),
            Self::Record(err) => write!(f, "{err}"),
            Self::InconsistentOutcome { result, reason } => {
                write!(f, "inconsistent pack outcome: result={result:?}, reason={reason:?}")
            }
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for PackError {}

impl From<io::Error> for PackError {
    fn from(value: io::Error) -> Self {
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

impl From<crate::records::error::RecordError> for PackError {
    fn from(value: crate::records::error::RecordError) -> Self {
        Self::Record(value)
    }
}

#[must_use]
pub fn encode_game(game: &PackGame) -> Vec<u8> {
    let mut out = Vec::with_capacity(40 + game.plies.len() * 4);
    match game.start_position {
        PackStartPosition::Startpos => out.push(START_POSITION_STARTPOS),
        PackStartPosition::Hcp { hcp, game_ply } => {
            out.push(START_POSITION_HCP);
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
    if input.is_empty() {
        return Err(PackError::Truncated);
    }

    let mut offset = 0usize;
    let start_state = input[offset];
    offset += 1;
    let start_position = match start_state {
        START_POSITION_STARTPOS => PackStartPosition::Startpos,
        START_POSITION_HCP => {
            if input.len() < offset + 34 {
                return Err(PackError::Truncated);
            }
            let mut hcp = [0u8; 32];
            hcp.copy_from_slice(&input[offset..offset + 32]);
            offset += 32;
            let game_ply = u16::from_le_bytes([input[offset], input[offset + 1]]);
            offset += 2;
            PackStartPosition::Hcp { hcp: HuffmanCodedPos { data: hcp }, game_ply }
        }
        other => return Err(PackError::InvalidStartState(other)),
    };

    let mut plies = Vec::new();
    loop {
        if input.len() < offset + 2 {
            return Err(PackError::Truncated);
        }
        let raw = u16::from_le_bytes([input[offset], input[offset + 1]]);
        offset += 2;

        if raw == PackGameResult::Draw.terminal_marker()
            || raw == PackGameResult::BlackWin.terminal_marker()
            || raw == PackGameResult::WhiteWin.terminal_marker()
        {
            if input.len() <= offset {
                return Err(PackError::Truncated);
            }
            let result = PackGameResult::try_from((raw & 0x7f) as u8)?;
            let end_reason = PackEndReason::from_byte(input[offset]);
            offset += 1;
            return Ok((PackGame { start_position, plies, result, end_reason }, offset));
        }

        let to = raw & 0x7f;
        let from = (raw >> 7) & 0x7f;
        if to == from {
            return Err(PackError::InvalidTerminalMarker(raw));
        }

        if input.len() < offset + 2 {
            return Err(PackError::Truncated);
        }
        let eval = i16::from_le_bytes([input[offset], input[offset + 1]]);
        offset += 2;
        plies.push(PackPly { mv: AperyMove::from_raw(raw), eval });
    }
}

pub fn decode_games<R: Read>(reader: &mut R) -> Result<Vec<PackGame>, PackError> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    let mut games = Vec::new();
    let mut offset = 0usize;
    while offset < bytes.len() {
        let (game, consumed) = decode_game(&bytes[offset..])?;
        games.push(game);
        offset += consumed;
    }
    Ok(games)
}

pub fn write_games<W: Write>(writer: &mut W, games: &[PackGame]) -> Result<(), PackError> {
    for game in games {
        writer.write_all(&encode_game(game))?;
    }
    Ok(())
}

pub fn records_from_bytes(input: &[u8]) -> Result<Vec<Record>, PackError> {
    let mut records = Vec::new();
    let mut offset = 0usize;
    while offset < input.len() {
        let (game, consumed) = decode_game(&input[offset..])?;
        records.push(record_from_game(&game)?);
        offset += consumed;
    }
    Ok(records)
}

pub fn games_from_records(records: &[Record]) -> Result<Vec<PackGame>, PackError> {
    records.iter().map(game_from_record).collect()
}

pub fn game_from_record(record: &Record) -> Result<PackGame, PackError> {
    let terminal = record.main_terminal().ok_or(PackError::MissingTerminal)?;
    let mut position = position_from_sfen(record.init_position_sfen())?;

    let start_position =
        if position.to_sfen(None) == hirate_position().to_sfen(None) && position.game_ply() == 1 {
            PackStartPosition::Startpos
        } else {
            PackStartPosition::Hcp {
                hcp: position.to_huffman_coded_pos(),
                game_ply: position.game_ply(),
            }
        };

    let mut plies = Vec::with_capacity(record.move_count());
    for (index, node_id) in record.main_line_ids().into_iter().enumerate() {
        let node = record.node(node_id);
        let mv_record = node.mv().ok_or(PackError::InvalidMove { index, raw: 0 })?;
        let eval = clamp_pack_eval(node.eval().ok_or(PackError::MissingEval(index))?.raw());
        let mv = mv_record.mv();
        if !position.is_legal_move(mv) {
            return Err(PackError::IllegalMove {
                index,
                raw: mv.to_apery().raw(),
                sfen: position.to_sfen(None),
            });
        }
        plies.push(PackPly { mv: AperyMove::from_move(mv), eval });
        position.apply_move(mv);
    }

    let result = pack_game_result_from_record(terminal.result(), terminal.kind())?;
    let end_reason = pack_end_reason_from_record(record, terminal);
    Ok(PackGame { start_position, plies, result, end_reason })
}

pub fn record_from_game(game: &PackGame) -> Result<Record, PackError> {
    let mut position = match game.start_position {
        PackStartPosition::Startpos => hirate_position(),
        PackStartPosition::Hcp { hcp, game_ply } => {
            let mut pos = Position::empty();
            pos.set_huffman_coded_pos(&hcp, game_ply)?;
            pos
        }
    };

    let init_position_sfen = position.to_sfen(None);
    let mut moves = Vec::with_capacity(game.plies.len());
    for (index, ply) in game.plies.iter().enumerate() {
        let mv = ply.mv.to_move();
        if !mv.is_normal() {
            return Err(PackError::InvalidMove { index, raw: ply.mv.raw() });
        }
        if !position.is_legal_move(mv) {
            return Err(PackError::IllegalMove {
                index,
                raw: ply.mv.raw(),
                sfen: position.to_sfen(None),
            });
        }
        moves.push(AnnotatedMoveEntry::new(mv).with_eval(Some(Eval::from_raw(ply.eval))));
        position.apply_move(mv);
    }

    let (special, result) = terminal_from_pack_outcome(game.result, game.end_reason)?;
    let terminal =
        SpecialMoveEntry::new(special, result).with_raw(Some(pack_end_reason_raw(game.end_reason)));
    let mut record = Record::from_annotated_main_line(
        init_position_sfen,
        moves,
        Some((terminal, RecordAnnotation::new())),
    )?;
    match game.end_reason {
        PackEndReason::WinCsa24 | PackEndReason::DrawCsa24 => {
            let mut builder = RecordMetadata::builder();
            builder.impasse_rule(Some("CSARule24".to_string()));
            record.set_metadata(builder.build());
        }
        PackEndReason::WinCsa27 => {
            let mut builder = RecordMetadata::builder();
            builder.impasse_rule(Some("CSARule27".to_string()));
            record.set_metadata(builder.build());
        }
        PackEndReason::WinTryRule => {
            let mut builder = RecordMetadata::builder();
            builder.impasse_rule(Some("TryRule".to_string()));
            record.set_metadata(builder.build());
        }
        _ => {}
    }
    Ok(record)
}

fn pack_game_result_from_record(
    result: GameResult,
    special: &SpecialMove,
) -> Result<PackGameResult, PackError> {
    match result {
        GameResult::DrawByRepetition | GameResult::DrawByMaxPlies | GameResult::DrawByImpasse => {
            Ok(PackGameResult::Draw)
        }
        GameResult::BlackWin
        | GameResult::BlackWinByDeclaration
        | GameResult::BlackWinByForfeit
        | GameResult::BlackWinByIllegalMove
        | GameResult::BlackWinByTimeout
        | GameResult::BlackWinByTryRule => Ok(PackGameResult::BlackWin),
        GameResult::WhiteWin
        | GameResult::WhiteWinByDeclaration
        | GameResult::WhiteWinByForfeit
        | GameResult::WhiteWinByIllegalMove
        | GameResult::WhiteWinByTimeout
        | GameResult::WhiteWinByTryRule => Ok(PackGameResult::WhiteWin),
        GameResult::Paused if matches!(special, SpecialMove::Interrupt) => Ok(PackGameResult::Draw),
        _ => Err(PackError::MissingTerminal),
    }
}

fn pack_end_reason_from_record(record: &Record, terminal: &SpecialMoveEntry) -> PackEndReason {
    if let Some(raw) = terminal.raw().and_then(parse_pack_end_reason_raw) {
        return raw;
    }

    match terminal.result() {
        GameResult::DrawByRepetition => PackEndReason::RepetitionDraw,
        GameResult::DrawByMaxPlies => PackEndReason::MaxMoves,
        GameResult::DrawByImpasse => PackEndReason::DrawCsa24,
        GameResult::BlackWinByTimeout | GameResult::WhiteWinByTimeout => PackEndReason::TimeUp,
        GameResult::BlackWinByIllegalMove | GameResult::WhiteWinByIllegalMove => {
            PackEndReason::IllegalMove
        }
        GameResult::BlackWinByTryRule | GameResult::WhiteWinByTryRule => PackEndReason::WinTryRule,
        GameResult::BlackWinByDeclaration | GameResult::WhiteWinByDeclaration => {
            if matches!(terminal.kind(), SpecialMove::Try) {
                return PackEndReason::WinTryRule;
            }
            if let Some(rule) = record.metadata().impasse_rule() {
                if rule.starts_with("CSARule24") {
                    return PackEndReason::WinCsa24;
                }
                if rule == "TryRule" {
                    return PackEndReason::WinTryRule;
                }
            }
            PackEndReason::WinCsa27
        }
        GameResult::Paused => match terminal.kind() {
            SpecialMove::Try => PackEndReason::WinTryRule,
            _ => PackEndReason::Interrupt,
        },
        _ => match terminal.kind() {
            SpecialMove::Resign => PackEndReason::Resign,
            SpecialMove::Interrupt => PackEndReason::Interrupt,
            SpecialMove::MaxMoves => PackEndReason::MaxMoves,
            SpecialMove::Impasse => PackEndReason::DrawCsa24,
            SpecialMove::RepetitionDraw => PackEndReason::RepetitionDraw,
            SpecialMove::Timeout => PackEndReason::TimeUp,
            SpecialMove::LoseByIllegalMove | SpecialMove::WinByIllegalMove => {
                PackEndReason::IllegalMove
            }
            SpecialMove::WinByDeclaration => PackEndReason::WinCsa27,
            SpecialMove::Try => PackEndReason::WinTryRule,
            _ => PackEndReason::Resign,
        },
    }
}

fn terminal_from_pack_outcome(
    result: PackGameResult,
    reason: PackEndReason,
) -> Result<(SpecialMove, GameResult), PackError> {
    match reason {
        PackEndReason::Resign => Ok((
            SpecialMove::Resign,
            match result {
                PackGameResult::BlackWin => GameResult::BlackWin,
                PackGameResult::WhiteWin => GameResult::WhiteWin,
                PackGameResult::Draw => {
                    return Err(PackError::InconsistentOutcome { result, reason });
                }
            },
        )),
        PackEndReason::RepetitionDraw => {
            Ok((SpecialMove::RepetitionDraw, GameResult::DrawByRepetition))
        }
        PackEndReason::MaxMoves => Ok((SpecialMove::MaxMoves, GameResult::DrawByMaxPlies)),
        PackEndReason::Interrupt => Ok((SpecialMove::Interrupt, GameResult::Paused)),
        PackEndReason::TimeUp => Ok((
            SpecialMove::Timeout,
            match result {
                PackGameResult::BlackWin => GameResult::BlackWinByTimeout,
                PackGameResult::WhiteWin => GameResult::WhiteWinByTimeout,
                PackGameResult::Draw => {
                    return Err(PackError::InconsistentOutcome { result, reason });
                }
            },
        )),
        PackEndReason::IllegalMove | PackEndReason::RepetitionCheck => Ok((
            SpecialMove::LoseByIllegalMove,
            match result {
                PackGameResult::BlackWin => GameResult::BlackWinByIllegalMove,
                PackGameResult::WhiteWin => GameResult::WhiteWinByIllegalMove,
                PackGameResult::Draw => {
                    return Err(PackError::InconsistentOutcome { result, reason });
                }
            },
        )),
        PackEndReason::WinCsa24 | PackEndReason::WinCsa27 => Ok((
            SpecialMove::WinByDeclaration,
            match result {
                PackGameResult::BlackWin => GameResult::BlackWinByDeclaration,
                PackGameResult::WhiteWin => GameResult::WhiteWinByDeclaration,
                PackGameResult::Draw => {
                    return Err(PackError::InconsistentOutcome { result, reason });
                }
            },
        )),
        PackEndReason::DrawCsa24 => Ok((SpecialMove::Impasse, GameResult::DrawByImpasse)),
        PackEndReason::WinTryRule => Ok((
            SpecialMove::Try,
            match result {
                PackGameResult::BlackWin => GameResult::BlackWinByTryRule,
                PackGameResult::WhiteWin => GameResult::WhiteWinByTryRule,
                PackGameResult::Draw => {
                    return Err(PackError::InconsistentOutcome { result, reason });
                }
            },
        )),
        PackEndReason::Reserved(value) => Ok((
            SpecialMove::Unknown(format!("PACK_RESERVED_{value}")),
            match result {
                PackGameResult::BlackWin => GameResult::BlackWin,
                PackGameResult::WhiteWin => GameResult::WhiteWin,
                PackGameResult::Draw => GameResult::Paused,
            },
        )),
    }
}

fn pack_end_reason_raw(reason: PackEndReason) -> String {
    match reason {
        PackEndReason::Reserved(value) => format!("{PACK_RAW_PREFIX}{value}"),
        _ => format!("{PACK_RAW_PREFIX}{}", reason.raw_label()),
    }
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

fn parse_pack_end_reason_raw(raw: &str) -> Option<PackEndReason> {
    let value = raw.strip_prefix(PACK_RAW_PREFIX)?;
    let reason = match value {
        "resign" => PackEndReason::Resign,
        "draw" => PackEndReason::RepetitionDraw,
        "max_moves" => PackEndReason::MaxMoves,
        "interrupt" => PackEndReason::Interrupt,
        "time_up" => PackEndReason::TimeUp,
        "illegal_move" => PackEndReason::IllegalMove,
        "repetition_check" => PackEndReason::RepetitionCheck,
        "win_csa24" => PackEndReason::WinCsa24,
        "draw_csa24" => PackEndReason::DrawCsa24,
        "win_csa27" => PackEndReason::WinCsa27,
        "win_try_rule" => PackEndReason::WinTryRule,
        other => {
            let byte = other.parse::<u8>().ok()?;
            PackEndReason::Reserved(byte)
        }
    };
    Some(reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board;
    use crate::records::record::{EngineInfo, MoveEntry, RecordAnnotation, RecordMetadataBuilder};
    use crate::types::Move;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_pack_encode_decode_roundtrip_startpos() {
        let game = PackGame {
            start_position: PackStartPosition::Startpos,
            plies: vec![
                PackPly { mv: Move::from_usi("7g7f").unwrap().to_apery(), eval: 120 },
                PackPly { mv: Move::from_usi("3c3d").unwrap().to_apery(), eval: -80 },
            ],
            result: PackGameResult::WhiteWin,
            end_reason: PackEndReason::Resign,
        };

        let encoded = encode_game(&game);
        let (decoded, consumed) = decode_game(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, game);
    }

    #[test]
    fn test_pack_encode_decode_roundtrip_hcp() {
        board::init();
        let pos = board::position_from_sfen(
            "lnsgkgsnl/1r5b1/ppppppppp/9/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL w - 2",
        )
        .unwrap();
        let game = PackGame {
            start_position: PackStartPosition::Hcp {
                hcp: pos.to_huffman_coded_pos(),
                game_ply: pos.game_ply(),
            },
            plies: vec![PackPly { mv: Move::from_usi("3c3d").unwrap().to_apery(), eval: 42 }],
            result: PackGameResult::Draw,
            end_reason: PackEndReason::RepetitionDraw,
        };

        let encoded = encode_game(&game);
        let (decoded, consumed) = decode_game(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded, game);
    }

    #[test]
    fn test_game_from_record_clamps_eval_like_reference_writer() {
        board::init();
        let mut record = Record::new(board::hirate_position().to_sfen(None)).unwrap();
        record
            .append_move_with_annotation(
                record.root_id(),
                MoveEntry::new(Move::from_usi("7g7f").unwrap()),
                RecordAnnotation::new().with_engine_info(Some(
                    EngineInfo::new().with_eval(Some(Eval::from_raw(32_767))),
                )),
            )
            .unwrap();
        record
            .set_main_terminal(SpecialMoveEntry::new(SpecialMove::Resign, GameResult::BlackWin))
            .unwrap();

        let game = game_from_record(&record).unwrap();
        assert_eq!(game.plies[0].eval, Eval::CP_MAX);
    }

    #[test]
    fn test_record_roundtrip_preserves_try_rule_reason() {
        board::init();
        let mut record = Record::new(board::hirate_position().to_sfen(None)).unwrap();
        let first = record
            .append_move_with_annotation(
                record.root_id(),
                MoveEntry::new(Move::from_usi("7g7f").unwrap()),
                RecordAnnotation::new()
                    .with_engine_info(Some(EngineInfo::new().with_eval(Some(Eval::from_raw(10))))),
            )
            .unwrap();
        record
            .append_move_with_annotation(
                first,
                MoveEntry::new(Move::from_usi("3c3d").unwrap()),
                RecordAnnotation::new()
                    .with_engine_info(Some(EngineInfo::new().with_eval(Some(Eval::from_raw(-12))))),
            )
            .unwrap();
        record
            .set_main_terminal(
                SpecialMoveEntry::new(SpecialMove::Try, GameResult::BlackWinByTryRule)
                    .with_raw(Some(pack_end_reason_raw(PackEndReason::WinTryRule))),
            )
            .unwrap();
        let mut builder = RecordMetadataBuilder::default();
        builder.impasse_rule(Some("TryRule".to_string()));
        record.set_metadata(builder.build());

        let game = game_from_record(&record).unwrap();
        assert_eq!(game.end_reason, PackEndReason::WinTryRule);
        let rebuilt = record_from_game(&game).unwrap();
        assert_eq!(rebuilt.result(), GameResult::BlackWinByTryRule);
        assert_eq!(rebuilt.main_terminal().unwrap().raw(), Some("pack:end_reason=win_try_rule"));
    }

    #[test]
    fn test_record_from_game_rejects_illegal_move() {
        board::init();
        let game = PackGame {
            start_position: PackStartPosition::Startpos,
            plies: vec![PackPly { mv: AperyMove::from_raw(0x0001), eval: 0 }],
            result: PackGameResult::BlackWin,
            end_reason: PackEndReason::Resign,
        };
        assert!(matches!(record_from_game(&game), Err(PackError::IllegalMove { .. })));
    }

    #[test]
    fn test_decode_game_rejects_invalid_terminal_marker() {
        let bytes = vec![START_POSITION_STARTPOS, 0x83, 0x01, 0];
        assert!(matches!(decode_game(&bytes), Err(PackError::InvalidTerminalMarker(0x0183))));
    }

    #[test]
    fn test_terminal_reason_mapping_cases() {
        let cases = [
            (
                PackGameResult::BlackWin,
                PackEndReason::Resign,
                SpecialMove::Resign,
                GameResult::BlackWin,
            ),
            (
                PackGameResult::Draw,
                PackEndReason::RepetitionDraw,
                SpecialMove::RepetitionDraw,
                GameResult::DrawByRepetition,
            ),
            (
                PackGameResult::Draw,
                PackEndReason::MaxMoves,
                SpecialMove::MaxMoves,
                GameResult::DrawByMaxPlies,
            ),
            (
                PackGameResult::Draw,
                PackEndReason::Interrupt,
                SpecialMove::Interrupt,
                GameResult::Paused,
            ),
            (
                PackGameResult::WhiteWin,
                PackEndReason::TimeUp,
                SpecialMove::Timeout,
                GameResult::WhiteWinByTimeout,
            ),
            (
                PackGameResult::BlackWin,
                PackEndReason::IllegalMove,
                SpecialMove::LoseByIllegalMove,
                GameResult::BlackWinByIllegalMove,
            ),
            (
                PackGameResult::WhiteWin,
                PackEndReason::RepetitionCheck,
                SpecialMove::LoseByIllegalMove,
                GameResult::WhiteWinByIllegalMove,
            ),
            (
                PackGameResult::BlackWin,
                PackEndReason::WinCsa24,
                SpecialMove::WinByDeclaration,
                GameResult::BlackWinByDeclaration,
            ),
            (
                PackGameResult::Draw,
                PackEndReason::DrawCsa24,
                SpecialMove::Impasse,
                GameResult::DrawByImpasse,
            ),
            (
                PackGameResult::WhiteWin,
                PackEndReason::WinCsa27,
                SpecialMove::WinByDeclaration,
                GameResult::WhiteWinByDeclaration,
            ),
            (
                PackGameResult::BlackWin,
                PackEndReason::WinTryRule,
                SpecialMove::Try,
                GameResult::BlackWinByTryRule,
            ),
        ];

        for (result, reason, expected_special, expected_result) in cases {
            let (special, converted) = terminal_from_pack_outcome(result, reason).unwrap();
            assert_eq!(special, expected_special);
            assert_eq!(converted, expected_result);
        }
    }

    #[test]
    fn test_pack_fixture_decodes() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/test_data/pack/sample.pack");
        let bytes = fs::read(path).unwrap();
        let records = records_from_bytes(&bytes).unwrap();
        assert_eq!(records.len(), 5);
        assert!(records.iter().all(|record| record.move_count() > 0));
    }

    #[test]
    #[ignore = "private reference fixture is large and optional"]
    fn test_private_pack_fixture_smoke() {
        let Some(path) = std::env::var_os("RSSHOGI_PACK_FIXTURE").map(PathBuf::from) else {
            return;
        };
        if let Ok(bytes) = fs::read(path) {
            let records = records_from_bytes(&bytes).unwrap();
            assert!(!records.is_empty());
        }
    }
}
