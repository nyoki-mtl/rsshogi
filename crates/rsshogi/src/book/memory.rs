use std::collections::HashMap;

use super::{Book, BookEntry, BookKey, BookMove};

/// メモリ上の小規模な定跡DB。
#[derive(Debug, Default)]
pub struct MemoryBook {
    entries: HashMap<BookKey, Vec<BookMove>>,
}

impl MemoryBook {
    #[must_use]
    pub fn new() -> Self {
        Self { entries: HashMap::new() }
    }

    /// 局面数を返す。
    #[must_use]
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// 指定キーに1手追加する。
    pub fn insert_move(&mut self, key: BookKey, book_move: BookMove) {
        self.entries.entry(key).or_default().push(book_move);
    }

    /// 指定キーに複数の手を追加する。
    pub fn extend_moves<I>(&mut self, key: BookKey, moves: I)
    where
        I: IntoIterator<Item = BookMove>,
    {
        self.entries.entry(key).or_default().extend(moves);
    }

    /// 指定キーの手一覧を取得する。
    #[must_use]
    pub fn moves(&self, key: BookKey) -> Option<&[BookMove]> {
        self.entries.get(&key).map(|moves| moves.as_slice())
    }

    pub(crate) fn entries(&self) -> &HashMap<BookKey, Vec<BookMove>> {
        &self.entries
    }

    // builder（records 橋渡し）専用の mutation hook。records off では caller が無いため gate する。
    #[cfg(feature = "records")]
    pub(crate) fn entries_mut(&mut self) -> &mut HashMap<BookKey, Vec<BookMove>> {
        &mut self.entries
    }
}

impl Book for MemoryBook {
    fn get(&self, key: BookKey) -> Option<BookEntry<'_>> {
        let moves = self.entries.get(&key)?;
        Some(BookEntry::new(key, moves))
    }

    fn len(&self) -> usize {
        self.entries.len()
    }
}
