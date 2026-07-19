//! 定跡（Book）モジュール
//!
//! このモジュールは Zobrist ハッシュによる高速な定跡検索機能を提供する。
//!
//! # 主要な型
//!
//! - [`Book`] - 定跡を参照するための共通トレイト
//! - [`MemoryBook`] - メモリ内で定跡を管理する実装
//! - [`StaticBook`] - 静的にコンパイルされた定跡
//! - [`BookBuilder`] - [`Record`](crate::records::Record) から定跡を構築
//! - [`BookKey`] - 局面を一意に識別する Zobrist キー
//! - [`BookEntry`] - 定跡エントリー（指し手と出現回数のリスト）
//!
//! # 定跡の構築
//!
//! ```ignore
//! use rsshogi::book::{BookBuilder, MemoryBook};
//! use rsshogi::records::formats::kif;
//!
//! // 棋譜から定跡を構築
//! let mut builder = BookBuilder::default();
//!
//! // 複数の棋譜を追加
//! for record in records {
//!     builder.add_record(&record);
//! }
//!
//! // MemoryBook として取得
//! let book: MemoryBook = builder.build();
//! ```
//!
//! # 定跡の検索
//!
//! ```ignore
//! use rsshogi::board::{self, Position};
//! use rsshogi::book::{Book, book_key_from_position};
//!
//! board::init();
//! let pos = board::hirate_position();
//!
//! // 現在の局面のキーを取得
//! let key = book_key_from_position(&pos);
//!
//! // 定跡を検索
//! if let Some(entry) = book.get(key) {
//!     for book_move in entry.moves() {
//!         println!("{}: {} 回", book_move.mov.to_usi(), book_move.count);
//!     }
//! }
//! ```
//!
//! # BookKey について
//!
//! `BookKey` は局面の Zobrist キーである。通常は 64bit、`hash-128` feature 有効時は
//! 128bit で、以下の情報を含む：
//!
//! - 盤面の駒配置
//! - 持ち駒
//! - 手番
//!
//! これにより、同一局面を高速に検索できる。

// BookBuilder は records（Record）から定跡を構築する橋渡し。records feature 有効時のみ。
#[cfg(feature = "records")]
mod builder;
mod control;
mod database;
mod error;
mod memory;
mod sbk;
mod solver;
mod static_book;
mod types;
mod writer;
mod ybb;
mod yaneuraou;

use crate::board::Position;
use crate::types::Move;
#[cfg(feature = "records")]
pub use builder::BookBuilder;
pub use control::BookControl;
pub use database::{
    BookCandidate, BookDatabase, BookDatabaseEntry, BookEntryMetadata, BookMoveMetadata,
    BookPosition, NativeBookConvertOptions, NativeScorePolicy, SbkEntryMetadata, SbkEvalMetadata,
    SbkMoveMetadata, YaneuraOuEntryMetadata, YaneuraOuMoveMetadata,
};
pub use error::BookError;
pub use memory::MemoryBook;
pub use sbk::{
    SbkBook, SbkDiagnostics, SbkEntries, SbkEntry, SbkEval, SbkMove, SbkOpenProgress,
    SbkWriteOptions,
};
pub use solver::{PetaShockOptions, solve_peta_shock_book};
pub use static_book::StaticBook;
pub use types::{BookEntry, BookKey, BookMove};
pub use writer::YaneuraOuDb2016WriteOptions;
pub use yaneuraou::{
    YaneuraOuAccessMode, YaneuraOuBook, YaneuraOuBookDiagnostics, YaneuraOuBookEntries,
    YaneuraOuBookEntry, YaneuraOuBookMove, YaneuraOuBookOpenOptions, YaneuraOuValidationProgress,
};
pub use ybb::{YbbBook, YbbBookOpenOptions, YbbEntry, YbbMove};

/// Bookを参照するための共通インターフェース。
pub trait Book {
    /// 指定キーの定跡エントリーを取得する。
    fn get(&self, key: BookKey) -> Option<BookEntry<'_>>;

    /// 指定キーの局面が存在するか確認する。
    fn contains(&self, key: BookKey) -> bool {
        self.get(key).is_some()
    }

    /// 局面数を取得する。
    fn len(&self) -> usize;

    /// 空かどうかを判定する。
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Book用のZobristキー（盤面+持ち駒+手番）。
#[must_use]
pub fn book_key_from_position(pos: &Position) -> BookKey {
    let mut key = pos.board_key();
    key.xor(pos.hand_key());
    key
}

/// 1手進めた局面のBook用キーを計算する。
#[must_use]
pub fn book_key_after(pos: &Position, mv: Move) -> BookKey {
    pos.key_after_move(mv)
}
