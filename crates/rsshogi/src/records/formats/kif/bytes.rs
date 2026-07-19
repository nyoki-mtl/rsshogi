use crate::records::formats::common::{
    EncodedText, ExportOptions, TextEncoding, encode_text, encode_text_with_options,
};
use crate::records::record::Record;
use encoding_rs::SHIFT_JIS;
use std::borrow::Cow;

use super::{Ki2Error, KifError, export_ki2, export_kif, parse_ki2_str, parse_kif_str};

fn decode_kif_bytes(data: &[u8]) -> Cow<'_, str> {
    std::str::from_utf8(data).map_or_else(
        |_| {
            let (decoded, _, _) = SHIFT_JIS.decode(data);
            decoded
        },
        Cow::Borrowed,
    )
}

/// バイト列から KIF 形式の棋譜を解析する。
///
/// エンコーディングは自動検出（Shift_JIS / UTF-8）。
pub fn parse_kif_bytes(data: &[u8]) -> Result<Record, KifError> {
    let decoded = decode_kif_bytes(data);
    parse_kif_str(&decoded)
}

/// バイト列から KI2 形式の棋譜を解析する。
///
/// エンコーディングは自動検出（Shift_JIS / UTF-8）。
pub fn parse_ki2_bytes(data: &[u8]) -> Result<Record, Ki2Error> {
    let decoded = decode_kif_bytes(data);
    parse_ki2_str(&decoded)
}

/// [`Record`] を KIF 形式でエンコードしたバイト列に変換する。
///
/// # Examples
///
/// ```
/// use rsshogi::records::formats::kif;
/// use rsshogi::records::formats::common::TextEncoding;
///
/// # let kif_text = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\nまで1手で中断\n";
/// # let record = kif::parse_kif_str(kif_text).unwrap();
/// let encoded = kif::export_kif_bytes(&record, TextEncoding::Utf8).unwrap();
/// assert!(!encoded.has_unmappable_chars());
/// ```
pub fn export_kif_bytes(record: &Record, encoding: TextEncoding) -> Result<EncodedText, KifError> {
    let text = export_kif(record)?;
    Ok(encode_text(&text, encoding))
}

/// [`Record`] を KIF 形式でエンコードしたバイト列に変換する。
///
/// [`ExportOptions`] を受け取る拡張版。v1 では `encoding` のみを解釈する。
pub fn export_kif_bytes_with_options(
    record: &Record,
    options: ExportOptions,
) -> Result<EncodedText, KifError> {
    let text = export_kif(record)?;
    Ok(encode_text_with_options(&text, options))
}

/// [`Record`] を KI2 形式でエンコードしたバイト列に変換する。
///
/// # Examples
///
/// ```
/// use rsshogi::records::formats::kif;
/// use rsshogi::records::formats::common::TextEncoding;
///
/// # let kif_text = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\nまで1手で中断\n";
/// # let record = kif::parse_kif_str(kif_text).unwrap();
/// let encoded = kif::export_ki2_bytes(&record, TextEncoding::Utf8).unwrap();
/// assert!(!encoded.has_unmappable_chars());
/// ```
pub fn export_ki2_bytes(record: &Record, encoding: TextEncoding) -> Result<EncodedText, Ki2Error> {
    let text = export_ki2(record)?;
    Ok(encode_text(&text, encoding))
}

/// [`Record`] を KI2 形式でエンコードしたバイト列に変換する。
///
/// [`ExportOptions`] を受け取る拡張版。v1 では `encoding` のみを解釈する。
pub fn export_ki2_bytes_with_options(
    record: &Record,
    options: ExportOptions,
) -> Result<EncodedText, Ki2Error> {
    let text = export_ki2(record)?;
    Ok(encode_text_with_options(&text, options))
}
