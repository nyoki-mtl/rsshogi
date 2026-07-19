//! 初期局面と駒落ち局面の SFEN 定義
//!
//! [`InitialPosition`] は平手と駒落ちの各初期局面を列挙し、
//! 対応する SFEN 文字列を [`to_sfen()`](InitialPosition::to_sfen) で取得できる。
//!
//! 駒落ち局面では上手（駒を落とす側）が先に指すため、
//! 手番は後手（`w`）になる。

/// 初期局面の種類。
///
/// 平手と各種駒落ちの初期配置を SFEN 文字列として提供する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InitialPosition {
    /// 平手（通常の初期配置）
    Standard,
    /// 空の盤面（駒なし）
    Empty,
    /// 香落ち（上手の左香車を除去）
    HandicapLance,
    /// 右香落ち（上手の右香車を除去）
    HandicapRightLance,
    /// 角落ち（上手の角行を除去）
    HandicapBishop,
    /// 飛車落ち（上手の飛車を除去）
    HandicapRook,
    /// 飛香落ち（上手の飛車と左香車を除去）
    HandicapRookLance,
    /// 二枚落ち（上手の飛車と角行を除去）
    Handicap2Pieces,
    /// 三枚落ち（上手の飛車・角行・左香車を除去）
    Handicap3Pieces,
    /// 四枚落ち（上手の飛車・角行・両香車を除去）
    Handicap4Pieces,
    /// 五枚落ち（上手の飛車・角行・両香車・左桂馬を除去）
    Handicap5Pieces,
    /// 左五枚落ち（上手の飛車・角行・両香車・右桂馬を除去）
    HandicapLeft5Pieces,
    /// 六枚落ち（上手の飛車・角行・両香車・両桂馬を除去）
    Handicap6Pieces,
    /// 八枚落ち（上手の飛車・角行・両香車・両桂馬・両銀将を除去）
    Handicap8Pieces,
    /// 十枚落ち（上手の飛車・角行・両香車・両桂馬・両銀将・両金将を除去）
    Handicap10Pieces,
}

impl InitialPosition {
    /// この初期局面に対応する SFEN 文字列を返す。
    #[must_use]
    pub const fn to_sfen(self) -> &'static str {
        match self {
            Self::Standard => "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
            Self::Empty => "9/9/9/9/9/9/9/9/9 b - 1",
            Self::HandicapLance => {
                "lnsgkgsn1/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
            }
            Self::HandicapRightLance => {
                "1nsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
            }
            Self::HandicapBishop => "lnsgkgsnl/1r7/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1",
            Self::HandicapRook => "lnsgkgsnl/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1",
            Self::HandicapRookLance => {
                "lnsgkgsn1/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
            }
            Self::Handicap2Pieces => "lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1",
            Self::Handicap3Pieces => "lnsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1",
            Self::Handicap4Pieces => "1nsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1",
            Self::Handicap5Pieces => "2sgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1",
            Self::HandicapLeft5Pieces => {
                "1nsgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
            }
            Self::Handicap6Pieces => "2sgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1",
            Self::Handicap8Pieces => "3gkg3/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1",
            Self::Handicap10Pieces => "4k4/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1",
        }
    }

    /// この初期局面に対応する日本語の手合割名を返す。
    ///
    /// `Empty` は通常の手合割名を持たないため `None` を返す。
    #[must_use]
    pub const fn handicap_name_ja(self) -> Option<&'static str> {
        match self {
            Self::Standard => Some("平手"),
            Self::Empty => None,
            Self::HandicapLance => Some("香落ち"),
            Self::HandicapRightLance => Some("右香落ち"),
            Self::HandicapBishop => Some("角落ち"),
            Self::HandicapRook => Some("飛車落ち"),
            Self::HandicapRookLance => Some("飛香落ち"),
            Self::Handicap2Pieces => Some("二枚落ち"),
            Self::Handicap3Pieces => Some("三枚落ち"),
            Self::Handicap4Pieces => Some("四枚落ち"),
            Self::Handicap5Pieces => Some("五枚落ち"),
            Self::HandicapLeft5Pieces => Some("左五枚落ち"),
            Self::Handicap6Pieces => Some("六枚落ち"),
            Self::Handicap8Pieces => Some("八枚落ち"),
            Self::Handicap10Pieces => Some("十枚落ち"),
        }
    }

    /// 日本語の手合割名から初期局面を取得する。
    #[must_use]
    pub fn from_handicap_name_ja(name: &str) -> Option<Self> {
        match name {
            "平手" => Some(Self::Standard),
            "香落ち" => Some(Self::HandicapLance),
            "右香落ち" => Some(Self::HandicapRightLance),
            "角落ち" => Some(Self::HandicapBishop),
            "飛車落ち" => Some(Self::HandicapRook),
            "飛香落ち" => Some(Self::HandicapRookLance),
            "二枚落ち" => Some(Self::Handicap2Pieces),
            "三枚落ち" => Some(Self::Handicap3Pieces),
            "四枚落ち" => Some(Self::Handicap4Pieces),
            "五枚落ち" => Some(Self::Handicap5Pieces),
            "左五枚落ち" => Some(Self::HandicapLeft5Pieces),
            "六枚落ち" => Some(Self::Handicap6Pieces),
            "八枚落ち" => Some(Self::Handicap8Pieces),
            "十枚落ち" => Some(Self::Handicap10Pieces),
            _ => None,
        }
    }

    /// SFEN 文字列から対応する初期局面を取得する。
    #[must_use]
    pub fn from_sfen(sfen: &str) -> Option<Self> {
        match sfen {
            "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1" => {
                Some(Self::Standard)
            }
            "9/9/9/9/9/9/9/9/9 b - 1" => Some(Self::Empty),
            "lnsgkgsn1/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
                Some(Self::HandicapLance)
            }
            "1nsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
                Some(Self::HandicapRightLance)
            }
            "lnsgkgsnl/1r7/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
                Some(Self::HandicapBishop)
            }
            "lnsgkgsnl/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
                Some(Self::HandicapRook)
            }
            "lnsgkgsn1/7b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
                Some(Self::HandicapRookLance)
            }
            "lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
                Some(Self::Handicap2Pieces)
            }
            "lnsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
                Some(Self::Handicap3Pieces)
            }
            "1nsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
                Some(Self::Handicap4Pieces)
            }
            "2sgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
                Some(Self::Handicap5Pieces)
            }
            "1nsgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
                Some(Self::HandicapLeft5Pieces)
            }
            "2sgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
                Some(Self::Handicap6Pieces)
            }
            "3gkg3/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => {
                Some(Self::Handicap8Pieces)
            }
            "4k4/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1" => Some(Self::Handicap10Pieces),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::InitialPosition;

    #[test]
    fn test_handicap_lookup_roundtrip() {
        let cases = [
            (InitialPosition::Standard, "平手"),
            (InitialPosition::HandicapLance, "香落ち"),
            (InitialPosition::HandicapRightLance, "右香落ち"),
            (InitialPosition::HandicapBishop, "角落ち"),
            (InitialPosition::HandicapRook, "飛車落ち"),
            (InitialPosition::HandicapRookLance, "飛香落ち"),
            (InitialPosition::Handicap2Pieces, "二枚落ち"),
            (InitialPosition::Handicap3Pieces, "三枚落ち"),
            (InitialPosition::Handicap4Pieces, "四枚落ち"),
            (InitialPosition::Handicap5Pieces, "五枚落ち"),
            (InitialPosition::HandicapLeft5Pieces, "左五枚落ち"),
            (InitialPosition::Handicap6Pieces, "六枚落ち"),
            (InitialPosition::Handicap8Pieces, "八枚落ち"),
            (InitialPosition::Handicap10Pieces, "十枚落ち"),
        ];

        for (position, handicap_name) in cases {
            assert_eq!(position.handicap_name_ja(), Some(handicap_name));
            assert_eq!(InitialPosition::from_handicap_name_ja(handicap_name), Some(position));
            assert_eq!(InitialPosition::from_sfen(position.to_sfen()), Some(position));
        }
    }

    #[test]
    fn test_empty_has_no_handicap_name() {
        assert_eq!(InitialPosition::Empty.handicap_name_ja(), None);
        assert_eq!(InitialPosition::from_handicap_name_ja("その他"), None);
        assert_eq!(InitialPosition::from_sfen("invalid/invalid/invalid/invalid b - 1"), None);
    }
}
