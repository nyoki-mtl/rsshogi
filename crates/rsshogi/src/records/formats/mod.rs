//! 各棋譜フォーマットの入出力実装。
//!
//! - [`kif`] - KIF / KI2 形式（柿木形式）
//! - [`csa`] - CSA 形式（コンピュータ将棋協会標準）
//! - [`jkf`] - JKF 形式（JSON Kifu Format）
//! - [`sfen`] - SFEN 形式（USI プロトコル）
//! - [`usi`] - USI position command 形式
//! - [`sbinpack`] - sbinpack 形式（NNUE 系訓練データ用バイナリ）
//! - [`sazpack`] - sazpack 形式（AlphaZero 系訓練データ用バイナリ、sparse policy）
//! - [`hcpe`] - cshogi / Apery 互換の HCPE 学習データ
//! - [`pack`] - YaneuraOu ScriptCollection 互換の可変長 pack 学習データ
//! - [`common`] - テキストエンコーディング（[`TextEncoding`] / [`ExportOptions`] / [`EncodedText`]）
//! - [`traversal`] - 棋譜ツリーの局面付き走査（[`traverse_with_position`]）

pub mod common;
pub mod csa;
pub mod hcpe;
pub mod jkf;
pub mod kif;
pub mod pack;
pub mod sazpack;
mod sazpack_selfplay;
pub mod sbinpack;
pub mod sfen;
pub mod traversal;
pub mod usi;

pub use common::{EncodedText, ExportOptions, TextEncoding};
pub use csa::*;
pub use hcpe::*;
pub use jkf::*;
pub use kif::*;
pub use pack::*;
pub use sazpack::*;
pub use sbinpack::*;
pub use sfen::*;
pub use traversal::*;
pub use usi::*;
