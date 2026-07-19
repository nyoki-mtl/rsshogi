use super::{BookError, BookKey, BookMove, MemoryBook, StaticBook, book_key_from_position};
use crate::board::Position;
use crate::records::formats::traversal;
use crate::records::record::{Record, RecordEntry, RecordNodeId};
use crate::types::Eval;
use crate::types::Move;
use std::collections::HashMap;

/// Bookの構築を支援するビルダー。
#[derive(Debug, Default)]
pub struct BookBuilder {
    book: MemoryBook,
}

impl BookBuilder {
    /// 空の `BookBuilder` を生成する。
    #[must_use]
    pub fn new() -> Self {
        Self { book: MemoryBook::new() }
    }

    /// 局面キーを指定して 1 手を追加する。
    ///
    /// 同一キーに複数の手を追加でき、[`build_static`](Self::build_static) 時にスコア降順でソートされる。
    pub fn insert(&mut self, key: BookKey, mv: Move, score: i16, depth: u16) {
        self.book.insert_move(key, BookMove::new(mv, score, depth));
    }

    /// [`Position`] から Zobrist キーを計算して 1 手を追加する。
    pub fn insert_for_position(&mut self, position: &Position, mv: Move, score: i16, depth: u16) {
        let key = book_key_from_position(position);
        self.insert(key, mv, score, depth);
    }

    /// [`Record`] の全手順（変化手順を含む）を定跡として取り込む。
    ///
    /// 各手の評価値・探索深さは [`MoveEntry`](crate::records::record::MoveEntry) から取得し、
    /// 存在しない場合は `default_score` / `default_depth` を使用する。
    ///
    /// 内部的に [`traversal::traverse_with_position`](crate::records::formats::traversal::traverse_with_position)
    /// を使用して DFS 走査を行う。
    pub fn extend_from_game_record(
        &mut self,
        record: &Record,
        default_score: i16,
        default_depth: u16,
    ) -> Result<(), BookError> {
        // ルート局面のキーを事前に計算する。
        let mut root_pos = Position::empty();
        root_pos
            .set_sfen(record.init_position_sfen())
            .map_err(|err| BookError::InvalidData(format!("invalid init sfen: {err:?}")))?;
        let root_key = book_key_from_position(&root_pos);

        // 各ノードの局面キーを記録し、指し手適用前（= 親ノード）のキーを引く。
        let mut keys: HashMap<RecordNodeId, BookKey> = HashMap::new();
        keys.insert(record.root_id(), root_key);

        traversal::traverse_with_position(record, |node| match node.entry {
            RecordEntry::Move(mv_record) => {
                let current_key = book_key_from_position(node.position);
                keys.insert(node.node_id, current_key);
                let parent_key = keys[&node.parent_id];
                let mv = mv_record.mv();
                let annotation = record.node(node.node_id).annotation();
                let score = annotation.eval().map(Eval::raw).unwrap_or(default_score);
                let depth = annotation.depth().unwrap_or(default_depth);
                self.insert(parent_key, mv, score, depth);
                true
            }
            // 旧実装互換: special node 自体も、その配下の subtree も book へは取り込まない。
            RecordEntry::Special(_) => false,
        })
        .map_err(|err| BookError::InvalidData(format!("traversal failed: {err}")))?;

        Ok(())
    }

    /// 内部の [`MemoryBook`] を消費して返す。
    #[must_use]
    pub fn into_memory(self) -> MemoryBook {
        self.book
    }

    /// [`StaticBook`] へ変換する。
    ///
    /// 各局面の候補手はスコア降順 → 探索深さ降順 → `Move` 昇順でソートされ、
    /// 決定的な順序で固定される。
    #[must_use]
    pub fn build_static(mut self) -> StaticBook {
        for moves in self.book.entries_mut().values_mut() {
            sort_moves(moves);
        }
        StaticBook::from_memory_book(&self.book)
    }
}

impl StaticBook {
    /// [`MemoryBook`] の候補手を決定的な順序にソートしてから `StaticBook` へ変換する。
    #[must_use]
    pub fn from_memory_book_sorted(book: &MemoryBook) -> Self {
        let mut builder = BookBuilder { book: MemoryBook::new() };
        for (key, moves) in book.entries() {
            builder.book.extend_moves(*key, moves.iter().copied());
        }
        builder.build_static()
    }
}

fn sort_moves(moves: &mut [BookMove]) {
    moves.sort_by(|lhs, rhs| {
        rhs.score()
            .cmp(&lhs.score())
            .then_with(|| rhs.depth().cmp(&lhs.depth()))
            .then_with(|| lhs.mv().raw().cmp(&rhs.mv().raw()))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{self, Position};
    use crate::records::record::{
        EngineInfo, MoveEntry, RecordAnnotation, SpecialMove, SpecialMoveEntry,
    };
    use crate::types::GameResult;

    #[test]
    fn test_extend_from_game_record_collects_main_and_variation_moves() {
        board::init();

        let root = board::hirate_position();
        let mv1 = Move::from_usi("7g7f").unwrap();
        let mv2 = Move::from_usi("3c3d").unwrap();
        let mv_var = Move::from_usi("8c8d").unwrap();

        let mut record = Record::new(root.to_sfen(None)).unwrap();
        let first = record
            .append_move_with_annotation(
                record.root_id(),
                MoveEntry::new(mv1),
                RecordAnnotation::new().with_engine_info(Some(
                    EngineInfo::new().with_eval(Some(Eval::from_i32(120))).with_depth(Some(18)),
                )),
            )
            .unwrap();
        record.append_move(first, MoveEntry::new(mv2)).unwrap();
        record
            .append_move_with_annotation(
                first,
                MoveEntry::new(mv_var),
                RecordAnnotation::new().with_engine_info(Some(
                    EngineInfo::new().with_eval(Some(Eval::from_i32(-40))).with_depth(Some(9)),
                )),
            )
            .unwrap();

        let mut builder = BookBuilder::default();
        builder.extend_from_game_record(&record, 7, 5).unwrap();
        let book = builder.into_memory();

        let root_key = book_key_from_position(&root);
        assert_eq!(book.moves(root_key).unwrap(), &[BookMove::new(mv1, 120, 18)]);

        let mut after_mv1 = Position::empty();
        after_mv1.set_sfen(record.init_position_sfen()).unwrap();
        after_mv1.apply_move(mv1);
        let after_mv1_key = book_key_from_position(&after_mv1);

        assert_eq!(
            book.moves(after_mv1_key).unwrap(),
            &[BookMove::new(mv2, 7, 5), BookMove::new(mv_var, -40, 9)]
        );
    }

    #[test]
    fn test_extend_from_game_record_skips_special_subtrees() {
        board::init();

        let root = board::hirate_position();
        let mv1 = Move::from_usi("7g7f").unwrap();
        let mv2 = Move::from_usi("3c3d").unwrap();

        let mut record = Record::new(root.to_sfen(None)).unwrap();
        record.extend_main_line(vec![MoveEntry::new(mv1)]).unwrap();
        record
            .set_main_terminal(SpecialMoveEntry::new(SpecialMove::Interrupt, GameResult::Invalid))
            .unwrap();
        let result = record.extend_main_line(vec![MoveEntry::new(mv2)]);
        assert!(result.is_err());

        let mut builder = BookBuilder::default();
        builder.extend_from_game_record(&record, 0, 0).unwrap();
        let book = builder.into_memory();

        let root_key = book_key_from_position(&root);
        assert_eq!(book.moves(root_key).unwrap(), &[BookMove::new(mv1, 0, 0)]);

        let mut after_mv1 = Position::empty();
        after_mv1.set_sfen(record.init_position_sfen()).unwrap();
        after_mv1.apply_move(mv1);
        let after_mv1_key = book_key_from_position(&after_mv1);
        assert!(book.moves(after_mv1_key).is_none());
    }
}
