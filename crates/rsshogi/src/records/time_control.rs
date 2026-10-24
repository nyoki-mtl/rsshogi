//! 持ち時間設定
//!
//! [`TimeControl`] は対局の持ち時間を表現する。
//! 基本持ち時間、秒読み、フィッシャー加算の 3 要素で構成され、
//! すべて秒単位で保持する。
//!
//! `base+byoyomi+increment` 形式の spec 文字列との相互変換もサポートする。

/// 持ち時間（秒）を表す構造体。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(clippy::struct_field_names)]
pub struct TimeControl {
    base_seconds: u32,
    byoyomi_seconds: u32,
    increment_seconds: u32,
}

impl TimeControl {
    /// 持ち時間を生成する。各値の単位は秒。
    #[must_use]
    pub const fn new(base_seconds: u32, byoyomi_seconds: u32, increment_seconds: u32) -> Self {
        Self { base_seconds, byoyomi_seconds, increment_seconds }
    }

    /// 持ち時間の基本時間（秒）。
    #[must_use]
    pub const fn base_seconds(&self) -> u32 {
        self.base_seconds
    }

    /// 秒読み時間（秒）。
    #[must_use]
    pub const fn byoyomi_seconds(&self) -> u32 {
        self.byoyomi_seconds
    }

    /// フィッシャー加算時間（秒）。
    #[must_use]
    pub const fn increment_seconds(&self) -> u32 {
        self.increment_seconds
    }

    /// `base+byoyomi+increment` の spec 文字列に変換する。
    #[must_use]
    pub fn to_spec(&self) -> String {
        format!("{}+{}+{}", self.base_seconds, self.byoyomi_seconds, self.increment_seconds)
    }

    /// `base+byoyomi+increment` の spec 文字列から生成する。
    #[must_use]
    pub fn from_spec(spec: &str) -> Option<Self> {
        let parts: Vec<&str> = spec.split('+').collect();
        if parts.len() != 3 {
            return None;
        }
        let base = parse_decimal_to_u32(parts[0])?;
        let byoyomi = parse_decimal_to_u32(parts[1])?;
        let increment = parse_decimal_to_u32(parts[2])?;
        Some(Self::new(base, byoyomi, increment))
    }

    /// spec 文字列を正規化する（整数のみを許可）。
    #[must_use]
    pub fn normalize_spec(spec: &str) -> Option<String> {
        Self::from_spec(spec).map(|tc| tc.to_spec())
    }
}

/// KIF の「持ち時間」行から持ち時間を推定する。
#[must_use]
pub fn parse_kif_time_control(value: &str) -> Option<TimeControl> {
    let normalized = normalize_decimal_digits(value)
        .replace('＋', "+")
        .replace(['（', '('], "+")
        .replace(['）', ')'], "");
    let value = normalized.trim();
    let (main, increment) = if let Some(prefix) = value.strip_suffix("加算") {
        if let Some((main, increment)) = prefix.rsplit_once('+') {
            (main, Some(increment))
        } else if !prefix.contains('秒') {
            (prefix, Some(""))
        } else {
            return None;
        }
    } else if let Some((main, increment)) = value.split_once("加算") {
        (main, Some(increment))
    } else {
        (value, None)
    };
    let hours = parse_number_before_marker(main, "時間").ok()?;
    let minutes = parse_number_before_marker(main, "分").ok()?;
    let base_seconds =
        u32::try_from(u64::from(hours.unwrap_or(0)) * 3600 + u64::from(minutes.unwrap_or(0)) * 60)
            .ok()?;
    let byoyomi_text = main.split_once("秒読み").map_or(main, |(_, rest)| rest);
    let byoyomi_seconds = parse_number_before_marker(byoyomi_text, "秒").ok()?;
    let increment_seconds = match increment {
        Some(text) => parse_number_before_marker(text, "秒").ok()?,
        None => None,
    };

    if base_seconds == 0 && byoyomi_seconds.is_none() && increment_seconds.is_none() {
        return None;
    }

    Some(TimeControl::new(
        base_seconds,
        byoyomi_seconds.unwrap_or(0),
        increment_seconds.unwrap_or(0),
    ))
}

/// CSA v2.2 の TIME_LIMIT を解析する（HH:MM+SS）。
#[must_use]
fn parse_csa_time_limit_v22(value: &str) -> Option<TimeControl> {
    let (base_part, byoyomi_part) = value.split_once('+')?;
    let (hour_str, minute_str) = base_part.split_once(':')?;
    let hours = parse_decimal_to_u32(hour_str)?;
    let minutes = parse_decimal_to_u32(minute_str)?;
    let byoyomi = parse_decimal_to_u32(byoyomi_part)?;
    let base = u32::try_from(u64::from(hours) * 3600 + u64::from(minutes) * 60).ok()?;
    Some(TimeControl::new(base, byoyomi, 0))
}

/// CSA v3.0 の TIME_LIMIT を解析する（base+byoyomi+increment）。
#[must_use]
fn parse_csa_time_limit_v30(value: &str) -> Option<TimeControl> {
    TimeControl::from_spec(value)
}

/// CSA の TIME_LIMIT / TIME を解析する。
#[must_use]
pub fn parse_csa_time_control(value: &str) -> Option<TimeControl> {
    parse_csa_time_limit_v30(value).or_else(|| parse_csa_time_limit_v22(value))
}

fn parse_decimal_to_u32(text: &str) -> Option<u32> {
    let normalized = normalize_decimal_digits(text);
    let cleaned = normalized.trim();
    if cleaned.is_empty() {
        return None;
    }
    if let Ok(value) = cleaned.parse::<u32>() {
        return Some(value);
    }
    let value = cleaned.parse::<f64>().ok()?;
    if value.fract() == 0.0 { u32::try_from(value as i64).ok() } else { None }
}

fn parse_number_before_marker(
    text: &str,
    marker: &str,
) -> Result<Option<u32>, std::num::ParseIntError> {
    let Some(idx) = text.find(marker) else {
        return Ok(None);
    };
    let before = &text[..idx];
    let digits: String = before.chars().rev().take_while(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        return Ok(None);
    }
    let value: String = digits.chars().rev().collect();
    value.parse::<u32>().map(Some)
}

fn normalize_decimal_digits(text: &str) -> String {
    text.chars()
        .map(|ch| match ch {
            '０'..='９' => char::from_u32('0' as u32 + (ch as u32 - '０' as u32))
                .expect("normalized decimal digit"),
            _ => ch,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::STARTPOS_SFEN;
    use crate::records::formats::{csa, jkf, kif};
    use crate::records::{Record, RecordMetadata};

    #[test]
    fn parse_kif_increment_time_control() {
        for (text, expected) in [
            ("10分加算5秒", TimeControl::new(600, 0, 5)),
            ("加算5秒", TimeControl::new(0, 0, 5)),
            ("10分秒読み30秒加算5秒", TimeControl::new(600, 30, 5)),
            ("秒読み30秒加算5秒", TimeControl::new(0, 30, 5)),
            ("１時間３０分秒読み３０秒加算５秒", TimeControl::new(5400, 30, 5)),
            ("１０分加算５秒", TimeControl::new(600, 0, 5)),
            ("10分+5秒加算", TimeControl::new(600, 0, 5)),
            ("10分+1手5秒加算", TimeControl::new(600, 0, 5)),
            ("10分秒読み30秒+5秒加算", TimeControl::new(600, 30, 5)),
            ("10分+加算5秒", TimeControl::new(600, 0, 5)),
            ("10分 加算5秒", TimeControl::new(600, 0, 5)),
            ("１０分+１手５秒加算", TimeControl::new(600, 0, 5)),
            ("10分加算4294967295秒", TimeControl::new(600, 0, u32::MAX)),
            ("10分秒読み4294967295秒", TimeControl::new(600, u32::MAX, 0)),
            ("30分+30秒", TimeControl::new(1800, 30, 0)),
            ("30分+30秒加算5秒", TimeControl::new(1800, 30, 5)),
            ("30分30秒", TimeControl::new(1800, 30, 0)),
            ("30秒", TimeControl::new(0, 30, 0)),
            ("10分（加算なし）", TimeControl::new(600, 0, 0)),
            ("15分（1手10秒加算）", TimeControl::new(900, 0, 10)),
            ("10分＋10秒加算", TimeControl::new(600, 0, 10)),
            ("10分加算", TimeControl::new(600, 0, 0)),
        ] {
            assert_eq!(parse_kif_time_control(text), Some(expected), "{text}");
        }
    }

    #[test]
    fn parse_kif_base_time_overflow() {
        for text in [
            "71582789分加算5秒",
            "4294967295時間30分秒読み30秒",
            "4294967296時間30分",
            "4294967296分加算5秒",
            "10分秒読み4294967296秒",
            "10分加算4294967296秒",
            "10分+4294967296秒加算",
        ] {
            assert_eq!(parse_kif_time_control(text), None, "{text}");
        }
    }

    #[test]
    fn invalid_time_controls_keep_raw_headers_without_partial_values() {
        for value in ["4294967296時間30分", "10分秒読み4294967296秒", "10分加算4294967296秒"]
        {
            let text = format!(
                "手合割：平手\n持ち時間：{value}\n手数----指手---------消費時間--\n1 投了\n"
            );
            let record = kif::parse_kif_str(&text).unwrap();
            assert_eq!(record.metadata().time_control(), None);
            assert_eq!(
                record.metadata().attributes().get("持ち時間").map(String::as_str),
                Some(value)
            );
            assert!(kif::export_kif(&record).unwrap().contains(value));
            let jkf_text = jkf::export_jkf(&record).unwrap();
            let restored = jkf::parse_jkf_str(&jkf_text).unwrap();
            assert_eq!(restored.metadata().time_control(), None);
            assert_eq!(
                restored.metadata().attributes().get("持ち時間").map(String::as_str),
                Some(value)
            );
        }
    }

    #[test]
    fn exported_time_controls_roundtrip() {
        for time_control in [
            TimeControl::new(600, 0, 5),
            TimeControl::new(30, 0, 0),
            TimeControl::new(90, 30, 5),
            TimeControl::new(u32::MAX, 30, 5),
            TimeControl::new(0, 0, 5),
            TimeControl::new(600, 30, 5),
            TimeControl::new(0, 30, 5),
        ] {
            let mut record = Record::new(STARTPOS_SFEN).unwrap();
            let mut metadata = RecordMetadata::builder();
            metadata.time_control(Some(time_control));
            record.set_metadata(metadata.build());

            let csa = csa::export_csa(&record).unwrap();
            let reparsed_csa = csa::parse_csa_str(&csa).unwrap();
            assert_eq!(reparsed_csa.metadata().time_control(), Some(&time_control));

            let kif = kif::export_kif(&record).unwrap();
            let reparsed_kif = kif::parse_kif_str(&kif).unwrap();
            assert_eq!(reparsed_kif.metadata().time_control(), Some(&time_control));

            let jkf = jkf::export_jkf(&record).unwrap();
            let reparsed_jkf = jkf::parse_jkf_str(&jkf).unwrap();
            assert_eq!(reparsed_jkf.metadata().time_control(), Some(&time_control));
        }
    }

    #[test]
    fn csa_time_limit_rejects_overflow_without_panicking() {
        for text in ["99999999:00+00", "1193046:29+00", "4294967295:00+00", "00:4294967295+00"] {
            assert_eq!(parse_csa_time_control(text), None, "{text}");
        }
        assert_eq!(
            parse_csa_time_control("1193046:28+00"),
            Some(TimeControl::new(4_294_967_280, 0, 0))
        );
    }

    #[test]
    fn csa_invalid_time_alias_does_not_clear_a_valid_setting() {
        for (valid, invalid) in [
            ("TIME_LIMIT", "TIME"),
            ("TIME", "TIME_LIMIT"),
            ("TIME+", "TIMET+"),
            ("TIMET+", "TIME+"),
            ("TIME-", "TIMET-"),
            ("TIMET-", "TIME-"),
        ] {
            for invalid_value in ["99999999:00+00", "unrecognized"] {
                let text =
                    format!("V2.2\n${valid}:00:10+00\n${invalid}:{invalid_value}\nPI\n+\n%TORYO\n");
                let record = csa::parse_csa_str(&text).unwrap();
                let get_time = |record: &crate::records::Record| match valid {
                    "TIME+" | "TIMET+" => record.metadata().black_time_control().cloned(),
                    "TIME-" | "TIMET-" => record.metadata().white_time_control().cloned(),
                    _ => record.metadata().time_control().cloned(),
                };
                assert_eq!(get_time(&record), Some(TimeControl::new(600, 0, 0)));
                assert_eq!(
                    record.metadata().attributes().get(invalid).map(String::as_str),
                    Some(invalid_value)
                );
                let restored = csa::parse_csa_str(&csa::export_csa(&record).unwrap()).unwrap();
                assert_eq!(get_time(&restored), get_time(&record));
                let updated = text.replace("PI\n", &format!("${invalid}:00:20+00\nPI\n"));
                assert_eq!(
                    get_time(&csa::parse_csa_str(&updated).unwrap()),
                    Some(TimeControl::new(1200, 0, 0))
                );
            }
        }
    }

    #[test]
    fn csa_valid_time_setting_overrides_an_invalid_alias() {
        for (invalid, valid) in [
            ("TIME_LIMIT", "TIME"),
            ("TIME", "TIME_LIMIT"),
            ("TIMET+", "TIME+"),
            ("TIMET-", "TIME-"),
        ] {
            let text =
                format!("V2.2\n${invalid}:99999999:00+00\n${valid}:600+0+5\nPI\n+\n%TORYO\n");
            let record = csa::parse_csa_str(&text).unwrap();
            let output = csa::export_csa(&record).unwrap();
            let restored = csa::parse_csa_str(&output).unwrap();
            assert_eq!(
                restored.metadata().time_control(),
                record.metadata().time_control(),
                "{output}"
            );
            assert_eq!(
                restored.metadata().black_time_control(),
                record.metadata().black_time_control(),
                "{output}"
            );
            assert_eq!(
                restored.metadata().white_time_control(),
                record.metadata().white_time_control(),
                "{output}"
            );
        }
    }

    #[test]
    fn parse_kif_exported_byoyomi_form() {
        let parsed = parse_kif_time_control("30分秒読み30秒").unwrap();
        assert_eq!(parsed.base_seconds(), 30 * 60);
        assert_eq!(parsed.byoyomi_seconds(), 30);
    }

    #[test]
    fn parse_kif_fullwidth_time_control() {
        let parsed = parse_kif_time_control("１時間３０分秒読み３０秒").unwrap();
        assert_eq!(parsed.base_seconds(), 90 * 60);
        assert_eq!(parsed.byoyomi_seconds(), 30);
    }
}
