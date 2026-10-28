use crate::board::{PackedSfen, Position};
use crate::book::BookError;
use crate::types::Move;

use super::wire::MAX_TEXT_BYTES;

/// 深さの意味が未指定。
pub const DEPTH_KIND_UNSPECIFIED: u32 = 0;
/// 探索深さ。
pub const DEPTH_KIND_SEARCH: u32 = 1;
/// 定跡内の後退解析距離。
pub const DEPTH_KIND_RETROGRADE: u32 = 2;

/// 文書全体の任意メタデータ。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NativeBookMetadata {
    /// 定跡の題名。
    pub title: Option<String>,
    /// 定跡の説明。
    pub description: Option<String>,
    /// 深さの意味。既知の値は [`DEPTH_KIND_UNSPECIFIED`]、[`DEPTH_KIND_SEARCH`]、
    /// [`DEPTH_KIND_RETROGRADE`]。
    ///
    /// 未知の値も保持する。探索深さ条件へ適用できるのは [`DEPTH_KIND_SEARCH`] だけ。
    pub depth_kind: u32,
}

impl NativeBookMetadata {
    pub(super) fn normalized(&self) -> Result<Self, BookError> {
        Ok(Self {
            title: normalize_text(self.title.clone())?,
            description: normalize_text(self.description.clone())?,
            depth_kind: self.depth_kind,
        })
    }
}

/// 評価値の境界の種類。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeBound {
    /// 確定値。
    Exact,
    /// 下限値。
    Lower,
    /// 上限値。
    Upper,
}

/// 候補手を指す前の手番側から見た評価値。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NativeScore {
    /// 未評価。
    #[default]
    Unknown,
    /// 符号付き centipawn 評価。
    Cp {
        /// 評価値。
        value: i32,
        /// 評価値の境界。
        bound: NativeBound,
    },
    /// 候補手を含む終局までの符号付き手数。
    Mate {
        /// 正は手番勝ち、負は手番負け。
        plies: i32,
        /// 評価値の境界。
        bound: NativeBound,
    },
    /// 距離不明の詰み。
    MateUnknown {
        /// 手番側の勝ちなら真。
        winning: bool,
    },
}

/// 保存する候補手とその属性。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeCandidate {
    /// 通常移動または駒打ち。
    pub mv: Move,
    /// 手番視点の評価値。
    pub score: NativeScore,
    /// 文書の `depth_kind` に従う任意の深さ。
    pub depth: Option<u32>,
    /// 局面内の表示順位。
    pub display_order: u16,
}

impl NativeCandidate {
    /// 未評価かつ深さが未設定の候補手を作る。
    #[must_use]
    pub const fn new(mv: Move) -> Self {
        Self { mv, score: NativeScore::Unknown, depth: None, display_order: 0 }
    }

    pub(super) fn validate(&self) -> Result<(), BookError> {
        if !self.mv.is_normal() {
            return Err(BookError::InvalidData("invalid native candidate move".into()));
        }
        if matches!(self.score, NativeScore::Mate { plies: 0 | i32::MIN, .. }) {
            return Err(BookError::InvalidData("invalid native mate distance".into()));
        }
        Ok(())
    }
}

/// 正規化した局面キーと指し手順に整列した候補手。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeBookEntry {
    /// 正規形の PackedSfen バイト列。
    pub key: [u8; 32],
    /// 指し手の整数値順の候補手。
    pub candidates: Vec<NativeCandidate>,
}

impl NativeBookEntry {
    /// 局面から正規化キーを作り、候補手を整列して検証する。
    ///
    /// # Errors
    /// 局面が表現不能、候補手が不正、表示順位が順列でない場合に返す。
    pub fn from_position(
        position: &Position,
        mut candidates: Vec<NativeCandidate>,
    ) -> Result<Self, BookError> {
        let packed = position
            .try_to_packed_sfen()
            .map_err(|error| BookError::InvalidData(format!("invalid native position: {error}")))?;
        candidates.sort_unstable_by_key(|candidate| candidate.mv.raw());
        let entry = Self { key: *packed.as_bytes(), candidates };
        entry.validate()?;
        Ok(entry)
    }

    /// キーを復号し、手数 1 の局面を返す。
    ///
    /// # Errors
    /// キーが復号不能または正規形でない場合に返す。
    pub fn position(&self) -> Result<Position, BookError> {
        let mut position = Position::empty();
        position
            .set_packed_sfen(&PackedSfen::from_bytes(self.key), false, 1)
            .map_err(|error| BookError::InvalidData(format!("invalid native key: {error}")))?;
        let packed = position
            .try_to_packed_sfen()
            .map_err(|error| BookError::InvalidData(format!("invalid native key: {error}")))?;
        if packed.as_bytes() != &self.key {
            return Err(BookError::InvalidData("noncanonical native key".into()));
        }
        Ok(position)
    }

    pub(super) fn validate(&self) -> Result<(), BookError> {
        self.validate_with_position().map(|_| ())
    }

    pub(super) fn validate_with_position(&self) -> Result<Position, BookError> {
        if self.candidates.len() > usize::from(u16::MAX) {
            return Err(BookError::InvalidData("too many native candidates".into()));
        }
        let position = self.position()?;
        let mut orders = vec![false; self.candidates.len()];
        let mut previous = None;
        for candidate in &self.candidates {
            candidate.validate()?;
            if previous.is_some_and(|raw| raw >= candidate.mv.raw()) {
                return Err(BookError::InvalidData(
                    "native candidates are not strictly sorted".into(),
                ));
            }
            previous = Some(candidate.mv.raw());
            let order = orders
                .get_mut(usize::from(candidate.display_order))
                .ok_or_else(|| BookError::InvalidData("invalid native display order".into()))?;
            if *order {
                return Err(BookError::InvalidData("duplicate native display order".into()));
            }
            *order = true;
            if !position.is_legal_move(candidate.mv) {
                return Err(BookError::InvalidData("illegal native candidate move".into()));
            }
        }
        Ok(position)
    }
}

pub(super) fn normalize_text(text: Option<String>) -> Result<Option<String>, BookError> {
    let Some(text) = text else { return Ok(None) };
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    if normalized.len() > MAX_TEXT_BYTES {
        return Err(BookError::InvalidData("native text exceeds 16 MiB".into()));
    }
    Ok((!normalized.is_empty()).then_some(normalized))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_position_and_candidate_validation() {
        let position = crate::board::hirate_position();
        let mut first = NativeCandidate::new(Move::from_usi("7g7f").unwrap());
        let mut second = NativeCandidate::new(Move::from_usi("2g2f").unwrap());
        first.display_order = 1;
        second.display_order = 0;
        let entry =
            NativeBookEntry::from_position(&position, vec![first.clone(), second.clone()]).unwrap();
        assert!(entry.candidates[0].mv.raw() < entry.candidates[1].mv.raw());
        assert!(entry.position().unwrap().to_sfen(None).ends_with(" 1"));
        assert_eq!(entry.position().unwrap().to_sfen(None), position.to_sfen(None));
        assert!(NativeBookEntry::from_position(&position, vec![]).is_ok());
        assert!(NativeBookEntry::from_position(&position, vec![first.clone(), first]).is_err());
        second.mv = Move::from_usi("7g7e").unwrap();
        assert!(NativeBookEntry::from_position(&position, vec![second]).is_err());
        assert!(NativeBookEntry { key: [0; 32], candidates: vec![] }.position().is_err());
        assert!(NativeBookEntry::from_position(&Position::empty(), vec![]).is_err());
    }

    #[test]
    fn text_empty_newlines_and_limit() {
        assert_eq!(normalize_text(Some(String::new())).unwrap(), None);
        assert_eq!(normalize_text(Some("a\r\nb\rc".into())).unwrap(), Some("a\nb\nc".into()));
        assert!(normalize_text(Some("a".repeat(MAX_TEXT_BYTES))).is_ok());
        assert!(normalize_text(Some("a".repeat(MAX_TEXT_BYTES + 1))).is_err());
    }

    #[test]
    fn metadata_normalization_preserves_depth_kind() {
        for depth_kind in [0, 1, 2, u32::MAX] {
            let metadata = NativeBookMetadata {
                title: Some("title\r\n".into()),
                description: None,
                depth_kind,
            };
            let normalized = metadata.normalized().unwrap();
            assert_eq!(normalized.depth_kind, depth_kind);
            assert_eq!(normalized.title.as_deref(), Some("title\n"));
        }
    }

    #[test]
    fn identity_ignores_ply_and_move_order_but_preserves_side_hands_and_board() {
        fn entry(sfen: &str) -> NativeBookEntry {
            NativeBookEntry::from_position(&Position::from_sfen(sfen).unwrap(), vec![]).unwrap()
        }
        let start = crate::board::hirate_position();
        assert_eq!(entry(&start.to_sfen(Some(1))), entry(&start.to_sfen(Some(135))));
        assert_ne!(entry(&start.to_sfen(None)), entry(&start.to_sfen(None).replace(" b ", " w ")));
        let hand = "lnsgkgsnl/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b R 1";
        assert_ne!(entry(hand), entry(&hand.replace(" b R ", " b r ")));
        let handicap = "lnsgkgsnl/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1";
        assert!(entry(handicap).position().is_ok());
        fn replay(moves: &[&str]) -> NativeBookEntry {
            let mut position = crate::board::hirate_position();
            for usi in moves {
                let mv = Move::from_usi(usi).unwrap();
                assert!(position.is_legal_move(mv));
                position.apply_move32(position.move32_from_move(mv));
            }
            NativeBookEntry::from_position(&position, vec![]).unwrap()
        }
        assert_eq!(
            replay(&["7g7f", "3c3d", "2g2f", "8c8d"]),
            replay(&["2g2f", "8c8d", "7g7f", "3c3d"])
        );
        assert_ne!(replay(&["7g7f"]), replay(&["3g3f"]));
    }
}
