use std::fmt;

/// 定跡の読み込みと変換処理が返すエラー型。
#[derive(Debug)]
#[non_exhaustive]
pub enum BookError {
    /// 別のプロセスが定跡の編集ロックを保持している。
    Busy,
    /// ログの検証済み先頭部分より後ろに不正または未完了のデータがある。
    NativeLog {
        /// 最初の異常位置。
        offset: u64,
        /// 検証済みの最終位置。
        valid_end: u64,
        /// 検証済みの最終連番。
        sequence: u64,
        /// 診断内容。
        reason: &'static str,
    },
    /// 保存結果を確定できないため、再オープンが必要である。
    SaveUncertain(String),
    /// 下位の I/O 操作が失敗した。
    Io(std::io::Error),
    /// 定跡ファイルのコンテナ形式またはレコード構造がサポートされていない。
    InvalidFormat(&'static str),
    /// 定跡データに意味的な不正が含まれている。
    InvalidData(String),
    /// 要求された操作は意図的にサポートされていない。
    Unsupported(&'static str),
    /// 長時間の定跡処理が呼び出し元によってキャンセルされた。
    Cancelled,
}

impl fmt::Display for BookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy => f.write_str("opening book is busy"),
            Self::NativeLog { offset, reason, .. } => {
                write!(f, "invalid native book log at {offset}: {reason}")
            }
            Self::SaveUncertain(message) => write!(f, "book save outcome is uncertain: {message}"),
            Self::Io(err) => write!(f, "io error: {err}"),
            Self::InvalidFormat(msg) => write!(f, "invalid book format: {msg}"),
            Self::InvalidData(msg) => write!(f, "invalid data: {msg}"),
            Self::Unsupported(msg) => write!(f, "unsupported feature: {msg}"),
            Self::Cancelled => write!(f, "book operation cancelled"),
        }
    }
}

impl std::error::Error for BookError {}

impl From<std::io::Error> for BookError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
