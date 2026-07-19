//! 棋譜フォーマットの読み書き
//!
//! このモジュールは将棋の棋譜を様々なフォーマットで
//! 読み込みと書き出しを行う機能を提供する。
//!
//! # 対応フォーマット
//!
//! | フォーマット | パース | 出力 | 説明 |
//! |-------------|:------:|:----:|------|
//! | KIF | ○ | ○ | 柿木形式（人間が読みやすい標準形式） |
//! | KI2 | ○ | ○ | KIF の簡略版（移動元省略） |
//! | CSA | ○ | ○ | コンピュータ将棋協会標準形式 |
//! | JKF | ○ | ○ | JSON 形式の棋譜 |
//! | SFEN | ○ | ○ | 局面表記（USI プロトコル） |
//! | USI position | ○ | ○ | USI position command |
//! | SBINPACK | ○ | ○ | 訓練データ用バイナリ形式 |
//! | PACK | ○ | ○ | 可変長学習バイナリ |
//!
//! # 主要な型
//!
//! - [`Record`] - 棋譜データ構造（本手順 + 分岐を木構造で保持）
//! - [`MoveEntry`] - 指し手 payload（コメントや消費時間は [`RecordNode`] の annotation）
//! - [`RecordMetadata`] - 対局メタデータ（対局者名、棋戦名など）
//! - [`SpecialMoveEntry`] - 終局特殊手（投了/千日手/最大手数など）
//! - [`GameResult`](crate::types::GameResult) - 対局結果
//!
//! `Record::result()` は [`GameResult`](crate::types::GameResult) を直接返す。
//! 終局特殊手の詳細は `Record::main_terminal()` で取得できる。
//!
//! # 棋譜の読み込み
//!
//! ```
//! use rsshogi::records::formats::kif;
//!
//! let kif_text = r#"
//! 手合割：平手
//! 先手：先手
//! 後手：後手
//! 手数----指手---------消費時間--
//!    1 ７六歩(77)
//!    2 ３四歩(33)
//! まで2手で中断
//! "#;
//!
//! let record = kif::parse_kif_str(kif_text).unwrap();
//!
//! // メタデータにアクセス
//! println!("先手: {:?}", record.metadata().black_player());
//! println!("後手: {:?}", record.metadata().white_player());
//!
//! // 本手順の指し手にアクセス
//! for move_record in record.main_moves() {
//!     println!("{:?}", move_record.mv());
//! }
//! ```
//!
//! # 棋譜の書き出し
//!
//! ```ignore
//! use rsshogi::records::formats::{kif, csa, jkf};
//!
//! // KIF 形式で出力
//! let kif_output = kif::export_kif(&record).unwrap();
//!
//! // CSA 形式で出力
//! let csa_output = csa::export_csa(&record).unwrap();
//!
//! // JKF 形式で出力
//! let jkf_output = jkf::export_jkf(&record).unwrap();
//! ```
//!
//! # フォーマット間の変換
//!
//! `Record` を介して、異なるフォーマット間の変換が可能。
//!
//! ```ignore
//! use rsshogi::records::formats::{kif, csa};
//!
//! // KIF → CSA 変換
//! let record = kif::parse_kif_str(kif_text)?;
//! let csa_text = csa::export_csa(&record)?;
//! ```
//!
//! # サブモジュール
//!
//! - [`formats`] - 各フォーマットの実装
//! - [`record`] - Record 関連の型定義
//! - [`time_control`] - 持ち時間設定
//! - [`error`] - エラー型

pub mod error;
pub mod formats;
pub mod record;
pub mod time_control;

pub use error::*;
pub use formats::*;
pub use record::*;
pub use time_control::*;
