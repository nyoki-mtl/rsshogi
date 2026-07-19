//! 自己対局教師データ用の sazpack codec。
//!
//! 旧 sazpack schema を置換するための `SAZ2` contract を実装する。

use crate::{
    board::{PackedSfen, PackedSfenError},
    records::formats::sbinpack::{
        decode_uleb128_u32_bounded, encode_uleb128_u32, game_result_from_u8,
    },
    types::{EnteringKingRule, GameResult, Move},
};
use std::ops::Range;

pub const SAZPACK_SELFPLAY_MAGIC: [u8; 4] = *b"SAZ2";
pub const SAZPACK_SELFPLAY_VERSION: u8 = 1;
const HEADER_LEN: usize = 10;
const MAX_PREALLOC: usize = 1024;
const POSITION_FIXED_LEN: usize = 24;
const POLICY_ENTRY_MIN_LEN: usize = 8;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SazSelfplayError {
    #[error("invalid sazpack selfplay magic")]
    InvalidMagic,
    #[error("unsupported sazpack selfplay version: {0}")]
    UnsupportedVersion(u8),
    #[error("unsupported sazpack selfplay flags: {0}")]
    UnsupportedFlags(u8),
    #[error("truncated sazpack selfplay data")]
    Truncated,
    #[error("trailing bytes after sazpack selfplay chunk")]
    TrailingBytes,
    #[error("invalid game result: {0}")]
    InvalidGameResult(u8),
    #[error("invalid termination reason: {0}")]
    InvalidTerminationReason(u8),
    #[error("invalid entering king rule: {0}")]
    InvalidEnteringKingRule(u8),
    #[error("invalid outcome bound: {0}")]
    InvalidOutcomeBound(u8),
    #[error("invalid optional mate tag: {0}")]
    InvalidMateTag(u8),
    #[error("count exceeds u32")]
    CountOverflow,
    #[error("policy visits decreased")]
    DecreasingVisits,
    #[error("quantized distribution must sum to 65535")]
    InvalidDistributionSum,
    #[error("packed sfen decode failed: {0:?}")]
    PackedSfen(PackedSfenError),
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SazTerminationReason {
    NoLegalMoves = 0,
    RepetitionDraw = 1,
    PerpetualCheckWin = 2,
    PerpetualCheckLoss = 3,
    RepetitionSuperior = 4,
    RepetitionInferior = 5,
    EnteringKingDeclaration = 6,
    MaxGamePlies = 7,
}

impl TryFrom<u8> for SazTerminationReason {
    type Error = SazSelfplayError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::NoLegalMoves),
            1 => Ok(Self::RepetitionDraw),
            2 => Ok(Self::PerpetualCheckWin),
            3 => Ok(Self::PerpetualCheckLoss),
            4 => Ok(Self::RepetitionSuperior),
            5 => Ok(Self::RepetitionInferior),
            6 => Ok(Self::EnteringKingDeclaration),
            7 => Ok(Self::MaxGamePlies),
            _ => Err(SazSelfplayError::InvalidTerminationReason(value)),
        }
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SazOutcomeBound {
    Loss = 0,
    Draw = 1,
    Win = 2,
}

impl TryFrom<u8> for SazOutcomeBound {
    type Error = SazSelfplayError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Loss),
            1 => Ok(Self::Draw),
            2 => Ok(Self::Win),
            _ => Err(SazSelfplayError::InvalidOutcomeBound(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SazWdl {
    pub win: u16,
    pub draw: u16,
    pub loss: u16,
}

impl SazWdl {
    pub const TOTAL: u32 = u16::MAX as u32;

    fn validate(self) -> Result<(), SazSelfplayError> {
        if u32::from(self.win) + u32::from(self.draw) + u32::from(self.loss) == Self::TOTAL {
            Ok(())
        } else {
            Err(SazSelfplayError::InvalidDistributionSum)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SazSelfplayPolicyEntry {
    pub mv: Move,
    pub prior: u16,
    pub visits_before: u32,
    pub visits_after: u32,
    pub lower: SazOutcomeBound,
    pub upper: SazOutcomeBound,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SazSelfplayPosition {
    pub played: Move,
    pub root_wdl: SazWdl,
    pub outcome_wdl: SazWdl,
    pub plies_left: u16,
    pub requested_visits: u32,
    pub target_weight_milli: u16,
    pub exploration_flags: u8,
    pub mate: Option<i16>,
    pub policy: Vec<SazSelfplayPolicyEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SazSelfplayGame {
    pub stem_packed_sfen: PackedSfen,
    pub game_result: GameResult,
    pub termination_reason: SazTerminationReason,
    pub entering_king_rule: EnteringKingRule,
    pub positions: Vec<SazSelfplayPosition>,
}

/// A decoded game together with its exact payload byte range in the source chunk.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedSazSelfplayGame {
    pub game: SazSelfplayGame,
    pub payload_range: Range<usize>,
}

pub fn serialize_selfplay_chunk(games: &[SazSelfplayGame]) -> Result<Vec<u8>, SazSelfplayError> {
    let mut payload = Vec::new();
    for game in games {
        serialize_game(game, &mut payload)?;
    }
    let payload_len = u32::try_from(payload.len()).map_err(|_| SazSelfplayError::CountOverflow)?;
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(&SAZPACK_SELFPLAY_MAGIC);
    out.push(SAZPACK_SELFPLAY_VERSION);
    out.push(0);
    out.extend_from_slice(&payload_len.to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

fn serialize_game(game: &SazSelfplayGame, out: &mut Vec<u8>) -> Result<(), SazSelfplayError> {
    out.extend_from_slice(&game.stem_packed_sfen.data);
    out.push(game.game_result as u8);
    out.push(game.termination_reason as u8);
    out.push(entering_king_rule_to_u8(game.entering_king_rule));
    encode_uleb128_u32(
        u32::try_from(game.positions.len()).map_err(|_| SazSelfplayError::CountOverflow)?,
        out,
    );
    for position in &game.positions {
        serialize_position(position, out)?;
    }
    Ok(())
}

fn serialize_position(
    position: &SazSelfplayPosition,
    out: &mut Vec<u8>,
) -> Result<(), SazSelfplayError> {
    position.root_wdl.validate()?;
    position.outcome_wdl.validate()?;
    if position.policy.iter().any(|entry| entry.visits_after < entry.visits_before) {
        return Err(SazSelfplayError::DecreasingVisits);
    }
    let prior_sum: u32 = position.policy.iter().map(|entry| u32::from(entry.prior)).sum();
    if prior_sum != SazWdl::TOTAL {
        return Err(SazSelfplayError::InvalidDistributionSum);
    }
    out.extend_from_slice(&position.played.raw().to_le_bytes());
    for wdl in [position.root_wdl, position.outcome_wdl] {
        out.extend_from_slice(&wdl.win.to_le_bytes());
        out.extend_from_slice(&wdl.draw.to_le_bytes());
        out.extend_from_slice(&wdl.loss.to_le_bytes());
    }
    out.extend_from_slice(&position.plies_left.to_le_bytes());
    out.extend_from_slice(&position.requested_visits.to_le_bytes());
    out.extend_from_slice(&position.target_weight_milli.to_le_bytes());
    out.push(position.exploration_flags);
    match position.mate {
        Some(value) => {
            out.push(1);
            out.extend_from_slice(&value.to_le_bytes());
        }
        None => out.push(0),
    }
    encode_uleb128_u32(
        u32::try_from(position.policy.len()).map_err(|_| SazSelfplayError::CountOverflow)?,
        out,
    );
    for entry in &position.policy {
        out.extend_from_slice(&entry.mv.raw().to_le_bytes());
        out.extend_from_slice(&entry.prior.to_le_bytes());
        encode_uleb128_u32(entry.visits_before, out);
        encode_uleb128_u32(entry.visits_after, out);
        out.push(entry.lower as u8);
        out.push(entry.upper as u8);
    }
    Ok(())
}

pub fn deserialize_selfplay_chunk(input: &[u8]) -> Result<Vec<SazSelfplayGame>, SazSelfplayError> {
    deserialize_selfplay_chunk_indexed(input)
        .map(|games| games.into_iter().map(|entry| entry.game).collect())
}

/// Decode a chunk while retaining each game's exact source payload range.
pub fn deserialize_selfplay_chunk_indexed(
    input: &[u8],
) -> Result<Vec<IndexedSazSelfplayGame>, SazSelfplayError> {
    if input.len() < HEADER_LEN {
        return Err(SazSelfplayError::Truncated);
    }
    if input[..4] != SAZPACK_SELFPLAY_MAGIC {
        return Err(SazSelfplayError::InvalidMagic);
    }
    if input[4] != SAZPACK_SELFPLAY_VERSION {
        return Err(SazSelfplayError::UnsupportedVersion(input[4]));
    }
    if input[5] != 0 {
        return Err(SazSelfplayError::UnsupportedFlags(input[5]));
    }
    let payload_len = u32::from_le_bytes(input[6..10].try_into().unwrap()) as usize;
    let end = HEADER_LEN.checked_add(payload_len).ok_or(SazSelfplayError::Truncated)?;
    if end > input.len() {
        return Err(SazSelfplayError::Truncated);
    }
    if end != input.len() {
        return Err(SazSelfplayError::TrailingBytes);
    }
    let mut cursor = HEADER_LEN;
    let mut games = Vec::new();
    while cursor < end {
        let start = cursor;
        let game = deserialize_game(input, &mut cursor, end)?;
        games.push(IndexedSazSelfplayGame { game, payload_range: start..cursor });
    }
    Ok(games)
}

fn deserialize_game(
    input: &[u8],
    cursor: &mut usize,
    end: usize,
) -> Result<SazSelfplayGame, SazSelfplayError> {
    if *cursor + 35 > end {
        return Err(SazSelfplayError::Truncated);
    }
    let mut stem = PackedSfen::default();
    stem.data.copy_from_slice(&input[*cursor..*cursor + 32]);
    *cursor += 32;
    let game_result = game_result_from_u8(input[*cursor])
        .ok_or(SazSelfplayError::InvalidGameResult(input[*cursor]))?;
    *cursor += 1;
    let termination_reason = SazTerminationReason::try_from(input[*cursor])?;
    *cursor += 1;
    let entering_king_rule = entering_king_rule_from_u8(input[*cursor])?;
    *cursor += 1;
    let count = read_uleb(input, cursor, end)?;
    let mut positions = Vec::with_capacity((count as usize).min(MAX_PREALLOC));
    for _ in 0..count {
        positions.push(deserialize_position(input, cursor, end)?);
    }
    Ok(SazSelfplayGame {
        stem_packed_sfen: stem,
        game_result,
        termination_reason,
        entering_king_rule,
        positions,
    })
}

fn deserialize_position(
    input: &[u8],
    cursor: &mut usize,
    end: usize,
) -> Result<SazSelfplayPosition, SazSelfplayError> {
    if *cursor + POSITION_FIXED_LEN > end {
        return Err(SazSelfplayError::Truncated);
    }
    let played = Move::from_raw(read_u16(input, cursor));
    let root_wdl = read_wdl(input, cursor)?;
    let outcome_wdl = read_wdl(input, cursor)?;
    let plies_left = read_u16(input, cursor);
    let requested_visits = read_u32(input, cursor);
    let target_weight_milli = read_u16(input, cursor);
    let exploration_flags = input[*cursor];
    *cursor += 1;
    let mate = match input[*cursor] {
        0 => {
            *cursor += 1;
            None
        }
        1 => {
            *cursor += 1;
            if *cursor + 2 > end {
                return Err(SazSelfplayError::Truncated);
            }
            Some(read_i16(input, cursor))
        }
        tag => return Err(SazSelfplayError::InvalidMateTag(tag)),
    };
    let count = read_uleb(input, cursor, end)?;
    let mut policy = Vec::with_capacity((count as usize).min(MAX_PREALLOC));
    let mut prior_sum = 0u32;
    for _ in 0..count {
        if *cursor + POLICY_ENTRY_MIN_LEN > end {
            return Err(SazSelfplayError::Truncated);
        }
        let mv = Move::from_raw(read_u16(input, cursor));
        let prior = read_u16(input, cursor);
        prior_sum += u32::from(prior);
        let visits_before = read_uleb(input, cursor, end)?;
        let visits_after = read_uleb(input, cursor, end)?;
        if visits_after < visits_before {
            return Err(SazSelfplayError::DecreasingVisits);
        }
        if *cursor + 2 > end {
            return Err(SazSelfplayError::Truncated);
        }
        let lower = SazOutcomeBound::try_from(input[*cursor])?;
        *cursor += 1;
        let upper = SazOutcomeBound::try_from(input[*cursor])?;
        *cursor += 1;
        policy.push(SazSelfplayPolicyEntry {
            mv,
            prior,
            visits_before,
            visits_after,
            lower,
            upper,
        });
    }
    if prior_sum != SazWdl::TOTAL {
        return Err(SazSelfplayError::InvalidDistributionSum);
    }
    Ok(SazSelfplayPosition {
        played,
        root_wdl,
        outcome_wdl,
        plies_left,
        requested_visits,
        target_weight_milli,
        exploration_flags,
        mate,
        policy,
    })
}

fn read_wdl(input: &[u8], cursor: &mut usize) -> Result<SazWdl, SazSelfplayError> {
    let wdl = SazWdl {
        win: read_u16(input, cursor),
        draw: read_u16(input, cursor),
        loss: read_u16(input, cursor),
    };
    wdl.validate()?;
    Ok(wdl)
}

fn read_u16(input: &[u8], cursor: &mut usize) -> u16 {
    let value = u16::from_le_bytes([input[*cursor], input[*cursor + 1]]);
    *cursor += 2;
    value
}

fn read_i16(input: &[u8], cursor: &mut usize) -> i16 {
    let value = i16::from_le_bytes([input[*cursor], input[*cursor + 1]]);
    *cursor += 2;
    value
}

fn read_u32(input: &[u8], cursor: &mut usize) -> u32 {
    let value = u32::from_le_bytes(input[*cursor..*cursor + 4].try_into().unwrap());
    *cursor += 4;
    value
}

fn read_uleb(input: &[u8], cursor: &mut usize, end: usize) -> Result<u32, SazSelfplayError> {
    decode_uleb128_u32_bounded(input, cursor, end).ok_or(SazSelfplayError::Truncated)
}

const fn entering_king_rule_to_u8(rule: EnteringKingRule) -> u8 {
    match rule {
        EnteringKingRule::None => 0,
        EnteringKingRule::Point24 => 1,
        EnteringKingRule::Point24Handicap => 2,
        EnteringKingRule::Point27 => 3,
        EnteringKingRule::Point27Handicap => 4,
        EnteringKingRule::TryRule => 5,
        EnteringKingRule::Unset => 6,
    }
}

fn entering_king_rule_from_u8(value: u8) -> Result<EnteringKingRule, SazSelfplayError> {
    match value {
        0 => Ok(EnteringKingRule::None),
        1 => Ok(EnteringKingRule::Point24),
        2 => Ok(EnteringKingRule::Point24Handicap),
        3 => Ok(EnteringKingRule::Point27),
        4 => Ok(EnteringKingRule::Point27Handicap),
        5 => Ok(EnteringKingRule::TryRule),
        6 => Ok(EnteringKingRule::Unset),
        _ => Err(SazSelfplayError::InvalidEnteringKingRule(value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::hirate_position;

    fn fixture() -> SazSelfplayGame {
        SazSelfplayGame {
            stem_packed_sfen: hirate_position().to_packed_sfen(),
            game_result: GameResult::DrawByMaxPlies,
            termination_reason: SazTerminationReason::MaxGamePlies,
            entering_king_rule: EnteringKingRule::Point27,
            positions: vec![SazSelfplayPosition {
                played: Move::from_usi("7g7f").unwrap(),
                root_wdl: SazWdl { win: 20_000, draw: 25_535, loss: 20_000 },
                outcome_wdl: SazWdl { win: 0, draw: u16::MAX, loss: 0 },
                plies_left: 1,
                requested_visits: 800,
                target_weight_milli: 1000,
                exploration_flags: 3,
                mate: None,
                policy: vec![
                    SazSelfplayPolicyEntry {
                        mv: Move::from_usi("7g7f").unwrap(),
                        prior: 50_000,
                        visits_before: 2,
                        visits_after: 500,
                        lower: SazOutcomeBound::Loss,
                        upper: SazOutcomeBound::Win,
                    },
                    SazSelfplayPolicyEntry {
                        mv: Move::from_usi("2g2f").unwrap(),
                        prior: 15_535,
                        visits_before: 0,
                        visits_after: 300,
                        lower: SazOutcomeBound::Draw,
                        upper: SazOutcomeBound::Win,
                    },
                ],
            }],
        }
    }

    #[test]
    fn selfplay_roundtrip_preserves_typed_fields() {
        let game = fixture();
        let bytes = serialize_selfplay_chunk(std::slice::from_ref(&game)).unwrap();
        assert_eq!(&bytes[..4], b"SAZ2");
        assert_eq!(deserialize_selfplay_chunk(&bytes).unwrap(), vec![game]);
    }

    #[test]
    fn indexed_decode_preserves_exact_game_payload_ranges() {
        let games = [fixture(), fixture()];
        let bytes = serialize_selfplay_chunk(&games).unwrap();
        let indexed = deserialize_selfplay_chunk_indexed(&bytes).unwrap();
        assert_eq!(indexed.len(), 2);
        assert_eq!(indexed[0].game, games[0]);
        assert_eq!(indexed[1].game, games[1]);
        assert_eq!(indexed[0].payload_range.start, HEADER_LEN);
        assert_eq!(indexed[0].payload_range.end, indexed[1].payload_range.start);
        assert_eq!(indexed[1].payload_range.end, bytes.len());
        assert!(!bytes[indexed[0].payload_range.clone()].is_empty());
    }

    #[test]
    fn selfplay_rejects_decreasing_visits() {
        let mut game = fixture();
        game.positions[0].policy[0].visits_after = 1;
        assert_eq!(serialize_selfplay_chunk(&[game]), Err(SazSelfplayError::DecreasingVisits));
    }

    #[test]
    fn selfplay_rejects_every_truncated_prefix_without_panicking() {
        let bytes = serialize_selfplay_chunk(&[fixture()]).unwrap();
        for end in 0..bytes.len() {
            assert!(deserialize_selfplay_chunk(&bytes[..end]).is_err(), "accepted prefix {end}");
        }
    }

    #[test]
    fn selfplay_rejects_unknown_header_flags_and_mate_tag() {
        let mut bytes = serialize_selfplay_chunk(&[fixture()]).unwrap();
        bytes[5] = 1;
        assert_eq!(deserialize_selfplay_chunk(&bytes), Err(SazSelfplayError::UnsupportedFlags(1)));

        bytes[5] = 0;
        let position_start = HEADER_LEN + 32 + 3 + 1;
        let mate_tag = position_start + POSITION_FIXED_LEN - 1;
        bytes[mate_tag] = 2;
        assert_eq!(deserialize_selfplay_chunk(&bytes), Err(SazSelfplayError::InvalidMateTag(2)));
    }
}
