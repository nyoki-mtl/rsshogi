/// 長時間実行される book コールバックから返す協調制御値。
///
/// `BookControl` は `core::ops::ControlFlow` ではなくドメイン固有の型として定義する。
/// GUI 側が進捗点で `Cancel` を返せるようにするためであり、
/// 公開コールバックの概念を変えずに将来のキャンセル詳細を追加できる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookControl {
    /// 現在の操作を継続する。
    Continue,
    /// 現在の操作を停止し、[`BookError::Cancelled`](super::BookError::Cancelled) を返す。
    Cancel,
}
