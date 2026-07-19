//! 指し手生成の型パラメータ定義
//!
//! [`MoveGenType`] トレイトを実装した各構造体を
//! [`generate_moves`](super::generate_moves) の型パラメータとして渡すことで、
//! 生成する手の種類を指定する。
//!
//! 通常は [`Legal`]（全合法手）を使い、探索では [`Captures`]（取る手）や
//! [`Evasions`]（王手回避の pseudo-legal 手）などを目的に応じて使い分ける。
//!
//! 各型に対応する `*All` 変種（例: [`LegalAll`]）は、
//! 通常省略される歩、香、大駒の不成を含めて生成する。

// ANCHOR: movegen_generate_type
/// 合法手生成の種類を定義するトレイト。
///
/// 定数フラグの組み合わせで生成する手のフィルタリング条件を表現する。
pub trait MoveGenType {
    const CAPTURES: bool;
    const QUIETS: bool;
    const EVASIONS: bool;
    const QUIET_CHECKS: bool;
    const GENERATE_ALL_LEGAL: bool;
    const IS_CHECKS: bool;
    const IS_QUIETS_PRO_MINUS: bool;
    const IS_LEGAL: bool;
    const IS_CAPTURE_PLUS_PRO: bool;
    const IS_RECAPTURES: bool;

    #[inline]
    #[must_use]
    fn is_generate_all_legal() -> bool {
        Self::GENERATE_ALL_LEGAL
    }
}

/// 駒を取る手のみ生成する。成り可能な場合は成りのみ生成する。
pub struct Captures;
/// 取る手のみ生成（歩と大駒の不成も含む）
pub struct CapturesAll;
/// 取る手と歩の成る手。
pub struct CapturePlusPro;
/// 取る手 + 歩成り（歩/大駒の不成も含む）
pub struct CapturePlusProAll;
/// 駒を取らない手（静かな手）のみ生成する。成り可能な場合は成りのみ生成する。
pub struct Quiets;
/// 静かな手のみ生成（歩と大駒の不成も含む）
pub struct QuietsAll;
/// 静かな手のみ生成（歩の成る手を除外）
pub struct QuietsProMinus;
/// 静かな手のみ生成（歩の成る手を除外、歩と大駒の不成を含む）
pub struct QuietsProMinusAll;
/// 王手回避の pseudo-legal 手のみ生成する。王手されている局面でのみ使用する。
///
/// ピン外しや玉の移動先が利かれている手など、split legality 後段で落ちる手を含みうる。
pub struct Evasions;
/// 王手回避の pseudo-legal 手のみ生成（歩と大駒の不成も含む）
///
/// [`Evasions`] と同様に legal-only ではない。
pub struct EvasionsAll;
/// 王手されていない局面での全手生成（取る手 + 静かな手）。
pub struct NonEvasions;
/// 非王手回避手のみ生成（歩と大駒の不成を含む）
pub struct NonEvasionsAll;
/// 全合法手を生成する。王手されていれば回避手、そうでなければ全手。
pub struct Legal;
/// 合法手すべて（歩と大駒の不成を含む）
pub struct LegalAll;
/// 相手玉に王手をかける手を生成する（取る手と静かな手の両方を含む）。
pub struct Checks;
/// 王手となる指し手を生成（歩と大駒の不成を含む）
pub struct ChecksAll;
/// 王手となる静かな手のみ生成
pub struct QuietChecks;
/// 王手となる静かな手のみ生成（歩と大駒の不成も含む）
pub struct QuietChecksAll;
/// 指定マスへの移動手のみ生成する。取り返し手の生成に使用する。
pub struct Recaptures;
/// 指定マスへの移動手（Recaptures, 歩の不成なども含む）
pub struct RecapturesAll;
// ANCHOR_END: movegen_generate_type

impl MoveGenType for Captures {
    const CAPTURES: bool = true;
    const QUIETS: bool = false;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = false;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

impl MoveGenType for CapturesAll {
    const CAPTURES: bool = true;
    const QUIETS: bool = false;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = true;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

/// qsearch（静止探索）で使用するモード。
/// - 捕獲手（相手の駒を取る手）
/// - 歩が敵陣に成る手（捕獲でなくてもよい）
impl MoveGenType for CapturePlusPro {
    const CAPTURES: bool = true;
    const QUIETS: bool = false;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = false;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = true;
    const IS_RECAPTURES: bool = false;
}

impl MoveGenType for CapturePlusProAll {
    const CAPTURES: bool = true;
    const QUIETS: bool = false;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = true;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = true;
    const IS_RECAPTURES: bool = false;
}

impl MoveGenType for Quiets {
    const CAPTURES: bool = false;
    const QUIETS: bool = true;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = false;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

impl MoveGenType for QuietsAll {
    const CAPTURES: bool = false;
    const QUIETS: bool = true;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = true;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

impl MoveGenType for QuietsProMinus {
    const CAPTURES: bool = false;
    const QUIETS: bool = true;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = false;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = true;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

impl MoveGenType for QuietsProMinusAll {
    const CAPTURES: bool = false;
    const QUIETS: bool = true;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = true;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = true;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

/// 玉の移動、合駒、王手駒の捕獲を含む pseudo-legal な回避手を生成する。
impl MoveGenType for Evasions {
    const CAPTURES: bool = true;
    const QUIETS: bool = true;
    const EVASIONS: bool = true;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = false;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

impl MoveGenType for EvasionsAll {
    const CAPTURES: bool = true;
    const QUIETS: bool = true;
    const EVASIONS: bool = true;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = true;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

/// 合法性チェックは含まれない（ピン等の除外は呼び出し側で行う）。
impl MoveGenType for NonEvasions {
    const CAPTURES: bool = true;
    const QUIETS: bool = true;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = false;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

impl MoveGenType for NonEvasionsAll {
    const CAPTURES: bool = true;
    const QUIETS: bool = true;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = true;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

/// 歩、香、大駒の不成は生成しない（通常のゲームプレイ用）。
impl MoveGenType for Legal {
    const CAPTURES: bool = true;
    const QUIETS: bool = true;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = false;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = true;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

impl MoveGenType for LegalAll {
    const CAPTURES: bool = true;
    const QUIETS: bool = true;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = true;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = true;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

impl MoveGenType for Checks {
    const CAPTURES: bool = true;
    const QUIETS: bool = true;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = false;
    const IS_CHECKS: bool = true;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

impl MoveGenType for ChecksAll {
    const CAPTURES: bool = true;
    const QUIETS: bool = true;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = true;
    const IS_CHECKS: bool = true;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

impl MoveGenType for QuietChecks {
    const CAPTURES: bool = false;
    const QUIETS: bool = false;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = true;
    const GENERATE_ALL_LEGAL: bool = false;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

impl MoveGenType for QuietChecksAll {
    const CAPTURES: bool = false;
    const QUIETS: bool = false;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = true;
    const GENERATE_ALL_LEGAL: bool = true;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = false;
}

impl MoveGenType for Recaptures {
    const CAPTURES: bool = true;
    const QUIETS: bool = true;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = false;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = true;
}

impl MoveGenType for RecapturesAll {
    const CAPTURES: bool = true;
    const QUIETS: bool = true;
    const EVASIONS: bool = false;
    const QUIET_CHECKS: bool = false;
    const GENERATE_ALL_LEGAL: bool = true;
    const IS_CHECKS: bool = false;
    const IS_QUIETS_PRO_MINUS: bool = false;
    const IS_LEGAL: bool = false;
    const IS_CAPTURE_PLUS_PRO: bool = false;
    const IS_RECAPTURES: bool = true;
}
