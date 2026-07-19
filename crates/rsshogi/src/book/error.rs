use std::fmt;

/// 定跡の読み込みと変換処理が返すエラー型。
#[derive(Debug)]
pub enum BookError {
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
