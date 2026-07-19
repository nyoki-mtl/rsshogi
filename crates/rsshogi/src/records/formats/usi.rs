use crate::board::{Position, SfenError};
use crate::records::error::RecordError;
use crate::records::record::{
    MoveEntry, Record, RecordEditor, RecordInitialPosition, SpecialMove, SpecialMoveEntry,
};
use crate::types::{Color, GameResult, Move};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UsiPositionError {
    #[error("USI position command must not be empty")]
    Empty,
    #[error("invalid USI position token sequence: {0}")]
    InvalidTokenSequence(String),
    #[error("invalid USI position SFEN: {0}")]
    Sfen(#[from] SfenError),
    #[error("invalid move token at index {index}: {token}")]
    InvalidMove { index: usize, token: String },
    #[error("special token is not allowed in strict USI position mode at index {index}: {token}")]
    SpecialTokenNotAllowed { index: usize, token: String },
    #[error("move appears after terminal special token at index {index}: {token}")]
    MoveAfterSpecial { index: usize, token: String },
    #[error("record construction failed: {0}")]
    Record(#[from] RecordError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct UsiPositionParseOptions {
    pub allow_special_tokens: bool,
}

impl UsiPositionParseOptions {
    #[must_use]
    pub const fn strict() -> Self {
        Self { allow_special_tokens: false }
    }

    #[must_use]
    pub const fn extended() -> Self {
        Self { allow_special_tokens: true }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct UsiPositionExportOptions {
    pub include_special_tokens: bool,
}

impl UsiPositionExportOptions {
    #[must_use]
    pub const fn strict() -> Self {
        Self { include_special_tokens: false }
    }

    #[must_use]
    pub const fn extended() -> Self {
        Self { include_special_tokens: true }
    }
}

pub fn export_usi_position(record: &Record) -> Result<String, UsiPositionError> {
    export_usi_position_with_options(record, UsiPositionExportOptions::strict())
}

pub fn export_usi_position_with_options(
    record: &Record,
    options: UsiPositionExportOptions,
) -> Result<String, UsiPositionError> {
    let mut position = Position::empty();
    let mut parts = Vec::new();
    parts.push("position".to_string());
    match record.initial_position() {
        RecordInitialPosition::Startpos => {
            position.set_hirate();
            parts.push("startpos".to_string());
        }
        RecordInitialPosition::Sfen(sfen) => {
            position.set_sfen(sfen)?;
            parts.push("sfen".to_string());
            parts.extend(sfen.split_whitespace().map(ToOwned::to_owned));
        }
    }

    let mut move_tokens = Vec::new();
    for mv in record.main_moves() {
        if !position.is_legal_move(mv.mv()) {
            return Err(RecordError::IllegalMove.into());
        }
        move_tokens.push(mv.mv().to_usi());
        position.apply_move(mv.mv());
    }

    if options.include_special_tokens
        && let Some(terminal) = record.main_terminal()
        && let Some(token) = special_to_usi_token(terminal, position.turn())
    {
        move_tokens.push(token);
    }

    if !move_tokens.is_empty() {
        parts.push("moves".to_string());
        parts.extend(move_tokens);
    }

    Ok(parts.join(" "))
}

pub fn parse_usi_position(text: &str) -> Result<Record, UsiPositionError> {
    parse_usi_position_with_options(text, UsiPositionParseOptions::strict())
}

pub fn parse_usi_position_with_options(
    text: &str,
    options: UsiPositionParseOptions,
) -> Result<Record, UsiPositionError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(UsiPositionError::Empty);
    }
    let text = text.strip_prefix("position ").map_or(text, str::trim);
    let tokens: Vec<&str> = text.split_whitespace().collect();
    if tokens.is_empty() {
        return Err(UsiPositionError::Empty);
    }

    let (initial_position, move_start) = match tokens[0] {
        "startpos" => (RecordInitialPosition::Startpos, 1),
        "sfen" => {
            if tokens.len() < 5 {
                return Err(UsiPositionError::InvalidTokenSequence(text.to_string()));
            }
            let sfen = tokens[1..5].join(" ");
            validate_sfen(&sfen)?;
            (RecordInitialPosition::Sfen(sfen), 5)
        }
        _ => {
            if tokens.len() < 4 {
                return Err(UsiPositionError::InvalidTokenSequence(text.to_string()));
            }
            let sfen = tokens[..4].join(" ");
            validate_sfen(&sfen)?;
            (RecordInitialPosition::Sfen(sfen), 4)
        }
    };

    let mut editor = RecordEditor::new(initial_position)?;
    if move_start == tokens.len() {
        return Ok(editor.into_record());
    }
    if tokens[move_start] != "moves" {
        return Err(UsiPositionError::InvalidTokenSequence(text.to_string()));
    }

    let mut terminal_seen = false;
    for (index, token) in tokens[(move_start + 1)..].iter().enumerate() {
        if terminal_seen {
            return Err(UsiPositionError::MoveAfterSpecial { index, token: (*token).to_string() });
        }
        if let Some(special) = special_from_usi_token(token, editor.position().turn()) {
            if !options.allow_special_tokens {
                return Err(UsiPositionError::SpecialTokenNotAllowed {
                    index,
                    token: (*token).to_string(),
                });
            }
            editor.append_special(special)?;
            terminal_seen = true;
            continue;
        }
        let mv = Move::from_usi(token)
            .filter(|mv| mv.is_normal())
            .ok_or_else(|| UsiPositionError::InvalidMove { index, token: (*token).to_string() })?;
        editor.append_move(MoveEntry::new(mv))?;
    }

    Ok(editor.into_record())
}

fn validate_sfen(sfen: &str) -> Result<(), SfenError> {
    let mut position = Position::empty();
    position.set_sfen(sfen)
}

fn special_from_usi_token(token: &str, side_to_move: Color) -> Option<SpecialMoveEntry> {
    let lower = token.to_ascii_lowercase();
    let winner = side_to_move.flip();
    let entry = match lower.as_str() {
        "resign" => SpecialMoveEntry::new(SpecialMove::Resign, GameResult::win_from_color(winner)),
        "win" => SpecialMoveEntry::new(
            SpecialMove::WinByDeclaration,
            GameResult::win_by_declaration_from_color(side_to_move),
        ),
        "draw" | "sennichite" | "repetition" => {
            SpecialMoveEntry::new(SpecialMove::RepetitionDraw, GameResult::DrawByRepetition)
        }
        "max_moves" | "maxmoves" => {
            SpecialMoveEntry::new(SpecialMove::MaxMoves, GameResult::DrawByMaxPlies)
        }
        "jishogi" | "impasse" => {
            SpecialMoveEntry::new(SpecialMove::Impasse, GameResult::DrawByImpasse)
        }
        "time_up" | "timeout" => SpecialMoveEntry::new(
            SpecialMove::Timeout,
            GameResult::win_by_timeout_from_color(winner),
        ),
        "interrupt" | "chudan" => SpecialMoveEntry::new(SpecialMove::Interrupt, GameResult::Paused),
        "mate" => SpecialMoveEntry::new(SpecialMove::Mate, GameResult::win_from_color(winner)),
        "nomate" | "no_mate" => {
            SpecialMoveEntry::new(SpecialMove::NoMate, GameResult::DrawByMaxPlies)
        }
        _ => return None,
    };
    Some(entry.with_raw(Some(token.to_string())))
}

fn special_to_usi_token(special: &SpecialMoveEntry, side_to_move: Color) -> Option<String> {
    if let Some(raw) = special.raw()
        && special_from_usi_token(raw, side_to_move).is_some()
    {
        return Some(raw.to_string());
    }
    match special.kind() {
        SpecialMove::Resign => Some("resign".to_string()),
        SpecialMove::WinByDeclaration | SpecialMove::Try => Some("win".to_string()),
        SpecialMove::Draw => Some("draw".to_string()),
        SpecialMove::RepetitionDraw => Some("sennichite".to_string()),
        SpecialMove::MaxMoves => Some("max_moves".to_string()),
        SpecialMove::Impasse => Some("jishogi".to_string()),
        SpecialMove::Timeout => Some("time_up".to_string()),
        SpecialMove::Interrupt => Some("interrupt".to_string()),
        SpecialMove::Mate => Some("mate".to_string()),
        SpecialMove::NoMate => Some("nomate".to_string()),
        SpecialMove::Unknown(name) => Some(name.to_ascii_lowercase()),
        SpecialMove::WinByIllegalMove
        | SpecialMove::LoseByIllegalMove
        | SpecialMove::WinByDefault
        | SpecialMove::LoseByDefault => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitialPosition as BoardInitialPosition;

    #[test]
    fn parse_startpos_preserves_startpos_token() {
        let record = parse_usi_position("position startpos moves 7g7f 3c3d").expect("parse");
        assert!(record.initial_position().is_startpos());
        assert_eq!(
            record.moves().iter().map(|mv| mv.mv().to_usi()).collect::<Vec<_>>(),
            ["7g7f", "3c3d"]
        );
        assert_eq!(
            export_usi_position(&record).expect("export"),
            "position startpos moves 7g7f 3c3d"
        );
    }

    #[test]
    fn parse_sfen_preserves_sfen_token() {
        let text = "position sfen lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
        let record = parse_usi_position(text).expect("parse");
        assert!(!record.initial_position().is_startpos());
        assert_eq!(export_usi_position(&record).expect("export"), text);
    }

    #[test]
    fn strict_parse_rejects_special_tokens() {
        let err = parse_usi_position("position startpos moves resign").expect_err("strict error");
        assert!(matches!(err, UsiPositionError::SpecialTokenNotAllowed { .. }));
    }

    #[test]
    fn extended_parse_and_export_special_tokens() {
        let record = parse_usi_position_with_options(
            "position startpos moves 7g7f resign",
            UsiPositionParseOptions::extended(),
        )
        .expect("parse");
        assert_eq!(record.result(), GameResult::BlackWin);
        assert_eq!(
            export_usi_position_with_options(&record, UsiPositionExportOptions::extended())
                .expect("export"),
            "position startpos moves 7g7f resign"
        );
    }

    #[test]
    fn bare_sfen_is_accepted() {
        let sfen = BoardInitialPosition::Standard.to_sfen();
        let record = parse_usi_position(sfen).expect("parse");
        assert_eq!(record.init_position_sfen(), sfen);
    }
}
