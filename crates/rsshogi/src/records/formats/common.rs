use crate::board::{Position, SfenError};
use encoding_rs::SHIFT_JIS;
use std::collections::HashMap;

pub(crate) type BoardMap = HashMap<(u8, u8), (char, String)>;
pub(crate) type HandCounts = HashMap<char, HashMap<String, u8>>;

/// テキストエンコーディング。
///
/// 棋譜のバイト出力時に使用するエンコーディングを指定する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextEncoding {
    /// UTF-8
    Utf8,
    /// Shift_JIS（Windows-31J）
    ShiftJis,
}

/// 棋譜テキスト export のオプション。
///
/// v1 ではエンコーディング指定のみを持つが、将来的な改行/BOM/canonical policy
/// などの追加先として使う。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportOptions {
    encoding: TextEncoding,
}

impl ExportOptions {
    /// 指定エンコーディングでオプションを構築する。
    #[must_use]
    pub const fn new(encoding: TextEncoding) -> Self {
        Self { encoding }
    }

    /// 出力エンコーディングを取得する。
    #[must_use]
    pub const fn encoding(self) -> TextEncoding {
        self.encoding
    }
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self::new(TextEncoding::Utf8)
    }
}

/// エンコード済みテキスト。
///
/// バイト出力結果とエンコード時の診断情報を保持する。
#[derive(Clone, Debug)]
pub struct EncodedText {
    bytes: Vec<u8>,
    has_unmappable_chars: bool,
}

impl EncodedText {
    /// エンコード済みバイト列を取得する。
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// エンコード済みバイト列を消費して取得する。
    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// エンコード時にマッピングできない文字があったかどうかを返す。
    ///
    /// `true` の場合、Shift_JIS に変換できない文字が含まれていたことを示す。
    /// UTF-8 の場合は常に `false` を返す。
    #[must_use]
    pub const fn has_unmappable_chars(&self) -> bool {
        self.has_unmappable_chars
    }
}

/// 文字列を指定エンコーディングでバイト列に変換する。
pub(crate) fn encode_text(text: &str, encoding: TextEncoding) -> EncodedText {
    match encoding {
        TextEncoding::Utf8 => {
            EncodedText { bytes: text.as_bytes().to_vec(), has_unmappable_chars: false }
        }
        TextEncoding::ShiftJis => {
            let (encoded, _, had_unmappable) = SHIFT_JIS.encode(text);
            EncodedText { bytes: encoded.into_owned(), has_unmappable_chars: had_unmappable }
        }
    }
}

/// 文字列を export options に従ってバイト列に変換する。
pub(crate) fn encode_text_with_options(text: &str, options: ExportOptions) -> EncodedText {
    encode_text(text, options.encoding())
}

const HAND_ORDER: [&str; 7] = ["FU", "KY", "KE", "GI", "KI", "KA", "HI"];

fn csa_piece_to_sfen(piece_code: &str, color: char) -> Option<String> {
    let token = match piece_code {
        "FU" => "P",
        "KY" => "L",
        "KE" => "N",
        "GI" => "S",
        "KI" => "G",
        "KA" => "B",
        "HI" => "R",
        "OU" => "K",
        "TO" => "+P",
        "NY" => "+L",
        "NK" => "+N",
        "NG" => "+S",
        "UM" => "+B",
        "RY" => "+R",
        _ => return None,
    };

    if token.starts_with('+') {
        let piece = token.chars().nth(1)?;
        let normalized =
            if color == '-' { piece.to_ascii_lowercase().to_string() } else { piece.to_string() };
        return Some(format!("+{normalized}"));
    }

    let normalized = if color == '-' { token.to_ascii_lowercase() } else { token.to_string() };
    Some(normalized)
}

pub(crate) fn board_map_to_sfen(board_map: &BoardMap) -> Result<String, String> {
    let mut rows = Vec::with_capacity(9);
    for rank in 1..=9 {
        let mut empties = 0;
        let mut row = String::new();
        for file in (1..=9).rev() {
            if let Some((color, piece_code)) = board_map.get(&(file, rank)) {
                if empties > 0 {
                    row.push_str(&empties.to_string());
                    empties = 0;
                }
                let token = csa_piece_to_sfen(piece_code, *color)
                    .ok_or_else(|| format!("unknown piece code: {piece_code}"))?;
                row.push_str(&token);
            } else {
                empties += 1;
            }
        }
        if empties > 0 {
            row.push_str(&empties.to_string());
        }
        if row.is_empty() {
            row.push('9');
        }
        rows.push(row);
    }
    Ok(rows.join("/"))
}

pub(crate) fn hand_counts_to_sfen(hand_counts: &HandCounts) -> Result<String, String> {
    let mut parts: Vec<String> = Vec::new();
    for (color, lower) in [('+', false), ('-', true)] {
        let counts =
            hand_counts.get(&color).ok_or_else(|| "hand counts missing side".to_string())?;
        for piece_code in HAND_ORDER {
            let count = counts.get(piece_code).copied().unwrap_or(0);
            if count == 0 {
                continue;
            }
            let letter = csa_piece_to_sfen(piece_code, '+')
                .ok_or_else(|| format!("unknown hand piece code: {piece_code}"))?;
            let normalized = if lower { letter.to_ascii_lowercase() } else { letter };
            let token = if count > 1 { format!("{count}{normalized}") } else { normalized };
            parts.push(token);
        }
    }
    if parts.is_empty() { Ok("-".to_string()) } else { Ok(parts.join("")) }
}

pub(crate) fn ensure_hand_sides(hand_counts: &mut HandCounts) {
    hand_counts.entry('+').or_default();
    hand_counts.entry('-').or_default();
}

pub(crate) fn refresh_position_if_needed(
    pos: &mut Position,
    since_refresh: &mut usize,
) -> Result<(), SfenError> {
    const REFRESH_INTERVAL: usize = 200;
    *since_refresh += 1;
    if *since_refresh >= REFRESH_INTERVAL {
        let sfen = pos.to_sfen(None);
        pos.set_sfen(&sfen)?;
        *since_refresh = 0;
    }
    Ok(())
}
