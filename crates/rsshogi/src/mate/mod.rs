//! 詰み判定
//!
//! このモジュールはテーブル駆動の 1 手詰め判定ルーチンを提供する。
//!
//! # 提供する機能
//!
//! | 関数 | 説明 |
//! |------|------|
//! | [`solve_mate_in_one`] | テーブル駆動版 1 手詰め判定 |
//!
//! # 使用例
//!
//! ```
//! use rsshogi::board::{self, position_from_sfen};
//! use rsshogi::mate::solve_mate_in_one;
//!
//! board::init();
//!
//! // 1手詰めの局面
//! let pos = position_from_sfen(
//!     "4k4/9/4P4/9/9/9/9/9/9 b G2r2b4g4s4n4l17p 1"
//! ).unwrap();
//!
//! if let Some(mate_move) = solve_mate_in_one(&pos) {
//!     println!("詰み手: {}", mate_move.to_usi());
//! }
//! ```
//!
//! # アルゴリズム
//!
//! 内部の事前計算テーブルを使用し、
//! 玉の周囲 8 方向の利き状況を 16 ビットのインデックスにエンコードして
//! テーブル参照で「どの持ち駒で詰むか」「どの方向から詰むか」を判定する。

mod table;

use crate::board::Position;
use crate::types::Move32;

/// 現局面で先手側が一手で詰ませられる指し手を探索する。
#[must_use]
pub fn solve_mate_in_one(pos: &Position) -> Option<Move32> {
    table::solve_mate_in_one_table(pos)
}
