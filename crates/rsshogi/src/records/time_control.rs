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
    let base_seconds = parse_kif_time_minutes(value).map(|minutes| minutes * 60);
    let byoyomi_seconds = parse_kif_byoyomi_seconds(value).or_else(|| {
        let (_, bonus) = parse_kif_time_limit_line(value);
        bonus
    });

    if base_seconds.is_none() && byoyomi_seconds.is_none() {
        return None;
    }

    Some(TimeControl::new(base_seconds.unwrap_or(0), byoyomi_seconds.unwrap_or(0), 0))
}

/// KIF の「持ち時間」から分単位を取得する。
#[must_use]
fn parse_kif_time_minutes(value: &str) -> Option<u32> {
    let hours = parse_number_before_marker(value, "時間").unwrap_or(0);
    let minutes = parse_number_before_marker(value, "分").unwrap_or(0);
    if hours == 0 && minutes == 0 { None } else { Some(hours * 60 + minutes) }
}

/// KIF の「持ち時間」から秒単位の秒読みを取得する。
#[must_use]
fn parse_kif_byoyomi_seconds(value: &str) -> Option<u32> {
    parse_number_before_marker(value, "秒")
}

/// KIF の「持ち時間」行を解析して (base_seconds, byoyomi_seconds) を返す。
#[must_use]
fn parse_kif_time_limit_line(value: &str) -> (Option<u32>, Option<u32>) {
    let base_seconds = parse_kif_time_minutes(value).map(|minutes| minutes * 60);
    let bonus_seconds = parse_number_after_plus(value, "秒");
    (base_seconds, bonus_seconds)
}

/// CSA v2.2 の TIME_LIMIT を解析する（HH:MM+SS）。
#[must_use]
fn parse_csa_time_limit_v22(value: &str) -> Option<TimeControl> {
    let (base_part, byoyomi_part) = value.split_once('+')?;
    let (hour_str, minute_str) = base_part.split_once(':')?;
    let hours = parse_decimal_to_u32(hour_str)?;
    let minutes = parse_decimal_to_u32(minute_str)?;
    let byoyomi = parse_decimal_to_u32(byoyomi_part)?;
    Some(TimeControl::new(hours * 3600 + minutes * 60, byoyomi, 0))
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
    let cleaned = text.trim();
    if cleaned.is_empty() {
        return None;
    }
    if let Ok(value) = cleaned.parse::<u32>() {
        return Some(value);
    }
    let value = cleaned.parse::<f64>().ok()?;
    if value.fract() == 0.0 { u32::try_from(value as i64).ok() } else { None }
}

fn parse_number_before_marker(text: &str, marker: &str) -> Option<u32> {
    let idx = text.find(marker)?;
    let before = &text[..idx];
    let digits: String = before.chars().rev().take_while(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    let value: String = digits.chars().rev().collect();
    value.parse::<u32>().ok()
}

fn parse_number_after_plus(text: &str, marker: &str) -> Option<u32> {
    let idx = text.find('+')?;
    let after = &text[idx + 1..];
    let idx_marker = after.find(marker)?;
    let digits: String = after[..idx_marker].chars().filter(|ch| ch.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u32>().ok()
}
