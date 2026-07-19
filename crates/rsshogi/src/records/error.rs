use crate::board::SfenError;
use thiserror::Error;

/// 棋譜の構築と編集で発生するエラー。
#[derive(Debug, Error)]
pub enum RecordError {
    /// 初期局面の SFEN が空。
    #[error("initial position SFEN cannot be empty")]
    EmptyInitPosition,

    /// 初期局面の SFEN を解析できない。
    #[error("invalid initial position SFEN: {0}")]
    InvalidInitialPosition(#[from] SfenError),

    /// ノード ID がレコードのスロットテーブルの範囲外。
    #[error("invalid record node id {0}")]
    InvalidNodeId(usize),

    /// ノード ID が削除済みスロットを指している。
    #[error("record node {0} has been removed")]
    RemovedNode(usize),

    /// 終局の特殊ノードは子を持てない。
    #[error("cannot append a child under terminal node {0}")]
    TerminalNode(usize),

    /// 指定したノードが指定した親の子ではない。
    #[error("node {child} is not a child of parent {parent}")]
    NotChild { parent: usize, child: usize },

    /// 子インデックスが親の子配列の範囲外。
    #[error("child index {index} is out of range for parent {parent}")]
    InvalidChildIndex { parent: usize, index: usize },

    /// エディタの現在局面でその手が非合法。
    #[error("illegal move at current editor position")]
    IllegalMove,
}
