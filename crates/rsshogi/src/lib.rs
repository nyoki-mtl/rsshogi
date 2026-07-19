// パフォーマンス上の理由で、一部のホットパスに限定して `unsafe` を許可する。
// 使用箇所には SAFETY コメントを付与し、テストで妥当性を担保する。
#![allow(unsafe_code)]
// README 由来の固有名詞やプロトコル表記をそのまま rustdoc に載せるため許可する。
#![allow(clippy::doc_markdown)]
// 生成テーブルと movegen ホットパスでは、表現と実行順を維持するために一部 lint を抑制する。
#![allow(
    // 生成テーブルと盤面定数。
    clippy::unreadable_literal,
    clippy::similar_names,
    // 分岐形状は将棋ルールや参照実装との対応を保つ。
    clippy::if_not_else,
    clippy::map_unwrap_or,
    clippy::redundant_closure_for_method_calls,
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    clippy::match_same_arms,
    clippy::filter_map_next,
    clippy::bool_to_int_with_if,
    clippy::single_match_else,
    clippy::branches_sharing_code,
    // 盤面と指し手の圧縮表現で境界をテスト済みの整数変換を使う。
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::float_cmp,
    // Public API 名やエラー型の安定性を優先する。段階的に局所 allow へ移す。
    clippy::missing_const_for_fn,
    clippy::redundant_pub_crate,
    clippy::unnecessary_wraps,
    clippy::use_self,
    clippy::unnecessary_to_owned
)]
#![doc = include_str!("../README.md")]

pub mod board;
#[cfg(feature = "book")]
pub mod book;
#[cfg(feature = "policy-labels")]
pub mod labels;
pub mod mate;
pub(crate) mod simd;
pub mod types;
#[cfg(feature = "records")]
pub mod records;

// movegen の型をクレートルートから直接使えるよう再エクスポートする。
pub use board::movegen;
