use crate::board::zobrist::ZobristKey;
use crate::types::Move;

/// 定跡検索用の局面キー。
///
/// 通常は 64bit、`hash-128` feature 有効時は 128bit の Zobrist ハッシュ。
pub type BookKey = ZobristKey;

/// 定跡の 1 手分のデータ。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BookMove {
    mv: Move,
    score: i16,
    depth: u16,
}

impl BookMove {
    /// 新しい `BookMove` を生成する。
    #[must_use]
    pub const fn new(mv: Move, score: i16, depth: u16) -> Self {
        Self { mv, score, depth }
    }

    /// 16bit 指し手を取得する。
    #[must_use]
    pub const fn mv(self) -> Move {
        self.mv
    }

    /// 評価スコアを取得する。
    #[must_use]
    pub const fn score(self) -> i16 {
        self.score
    }

    /// 探索深さを取得する。
    #[must_use]
    pub const fn depth(self) -> u16 {
        self.depth
    }
}

/// 参照専用の定跡エントリー（1 局面に対する候補手のリスト）。
#[derive(Clone, Copy, Debug)]
pub struct BookEntry<'a> {
    key: BookKey,
    moves: &'a [BookMove],
}

impl<'a> BookEntry<'a> {
    /// 新しい `BookEntry` を生成する。
    #[must_use]
    pub const fn new(key: BookKey, moves: &'a [BookMove]) -> Self {
        Self { key, moves }
    }

    /// この局面のキーを取得する。
    #[must_use]
    pub const fn key(self) -> BookKey {
        self.key
    }

    /// この局面の候補手一覧を取得する。
    #[must_use]
    pub const fn moves(self) -> &'a [BookMove] {
        self.moves
    }
}
