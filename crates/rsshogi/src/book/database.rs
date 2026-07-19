use std::collections::HashSet;

use crate::board::Position;
use crate::board::position::PackedSfen;
use crate::book::{
    BookError, BookKey, BookMove, MemoryBook, SbkEntry, StaticBook, YaneuraOuBookEntry,
    book_key_from_position,
};
use crate::types::Move;

/// 変換と編集のワークフロー向けの、所有権付き定跡データベース。
///
/// [`BookMove`] より豊富な局面と指し手のメタデータを意図的に保持する型。
/// [`MemoryBook`] や [`StaticBook`] への変換は明示的かつ不可逆であり、
/// コメント、ponder 手、ソース ID、SBK グラフリンクなどの
/// 外部フォーマット固有メタデータはネイティブ検索書籍に含まれない。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookDatabase {
    entries: Vec<BookDatabaseEntry>,
}

/// [`BookDatabase`] 内の1局面行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookDatabaseEntry {
    position: BookPosition,
    candidates: Vec<BookCandidate>,
    metadata: BookEntryMetadata,
}

/// リッチ定跡データベースが保持する局面識別情報。
///
/// [`BookKey`] は派生したネイティブ検索キーとして保持する。
/// 正規データは正規化 SFEN であり、SBK 相互運用のために Packed SFEN も
/// 利用可能な場合は合わせて保持する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookPosition {
    sfen: String,
    key: BookKey,
    packed_sfen: Option<PackedSfen>,
    original_ply: Option<u32>,
}

/// リッチ定跡エントリ内の1候補手。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookCandidate {
    mv: Move,
    ponder: Option<Move>,
    score: Option<i32>,
    depth: Option<u32>,
    weight: Option<i32>,
    count: Option<u64>,
    metadata: BookMoveMetadata,
}

/// リッチ局面エントリに付属するフラットなメタデータ。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookEntryMetadata {
    yaneuraou: Option<YaneuraOuEntryMetadata>,
    sbk: Option<SbkEntryMetadata>,
}

/// リッチ候補手に付属するフラットなメタデータ。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BookMoveMetadata {
    yaneuraou: Option<YaneuraOuMoveMetadata>,
    sbk: Option<SbkMoveMetadata>,
}

/// 局面行に付属する DB2016 メタデータ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YaneuraOuEntryMetadata {
    min_ply: u32,
    comment: String,
}

/// 候補手に付属する DB2016 メタデータ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YaneuraOuMoveMetadata {
    comment: String,
}

/// 状態に付属する SBK メタデータ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbkEntryMetadata {
    state_id: Option<i32>,
    state_index: usize,
    games: Option<i32>,
    won_black: Option<i32>,
    won_white: Option<i32>,
    comment: Option<String>,
    evals: Vec<SbkEvalMetadata>,
}

/// 候補手に付属する SBK メタデータ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbkMoveMetadata {
    raw_move: i32,
    evaluation: Option<i32>,
    weight: Option<i32>,
    next_state_id: Option<i32>,
}

/// 状態に付属する SBK エンジン評価メタデータ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbkEvalMetadata {
    evaluation_value: Option<i32>,
    depth: Option<i32>,
    sel_depth: Option<i32>,
    nodes: Option<i64>,
    variation: Option<String>,
    engine_name: Option<String>,
}

/// [`BookDatabase`] からネイティブ検索書籍への不可逆変換オプション。
///
/// 変換はデータベースのエントリ順と候補手順を保持する。
/// 異なるエントリが同じネイティブキーに解決した場合は、
/// [`MemoryBook`] の既存のマルチマップ動作に合わせ、データベース順に手を追記する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeBookConvertOptions {
    missing_score: i16,
    missing_depth: u16,
    score_policy: NativeScorePolicy,
    score_narrowing: I16NarrowingPolicy,
    depth_narrowing: U16NarrowingPolicy,
}

/// リッチ候補値のどれをネイティブ [`BookMove::score`] に使うかを選択する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeScorePolicy {
    /// `BookCandidate::score` を使い、存在しない場合は `missing_score` を使う。
    Score,
    /// `BookCandidate::weight` を使い、存在しない場合は `missing_score` を使う。
    Weight,
}

/// 符号付き 32 ビット値を符号付き 16 ビットのネイティブスコアへ変換するポリシー。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum I16NarrowingPolicy {
    /// 範囲外の値を `i16::MIN..=i16::MAX` にクランプする。
    Clamp,
}

/// 32 ビット深度を 16 ビットのネイティブ深度へ変換するポリシー。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum U16NarrowingPolicy {
    /// 範囲外の値を `u16::MAX` にクランプする。
    Clamp,
}

impl BookDatabase {
    /// 空のリッチ定跡データベースを生成する。
    #[must_use]
    pub const fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// 編集不変条件を検証した上で、所有エントリからデータベースを生成する。
    pub fn try_from_entries(entries: Vec<BookDatabaseEntry>) -> Result<Self, BookError> {
        let database = Self { entries };
        database.validate()?;
        Ok(database)
    }

    /// 一意性を検証せずに所有エントリからデータベースを生成する。
    ///
    /// 内部 book ソルバおよびライターテストで使用する。
    /// 不変条件が構築時に保証されているか、カバレッジのために意図的に破られる場合に用いる。
    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn from_entries_unchecked(entries: Vec<BookDatabaseEntry>) -> Self {
        Self { entries }
    }

    /// デコード済みの DB2016 エントリを1件取り込む。
    pub fn entry_from_yaneuraou(
        entry: &YaneuraOuBookEntry,
    ) -> Result<BookDatabaseEntry, BookError> {
        BookDatabaseEntry::from_yaneuraou(entry)
    }

    /// デコード済みの SBK 状態エントリを1件取り込む。
    pub fn entry_from_sbk(entry: &SbkEntry) -> Result<BookDatabaseEntry, BookError> {
        BookDatabaseEntry::from_sbk(entry)
    }

    /// 正規化 SFEN でエントリを1件検索する。
    pub fn entry_by_sfen(&self, sfen: &str) -> Result<Option<&BookDatabaseEntry>, BookError> {
        let position = BookPosition::from_sfen(sfen, None)?;
        Ok(self.entry_by_normalized_sfen(position.sfen()))
    }

    /// 局面でエントリを1件検索する。
    #[must_use]
    pub fn entry_by_position(&self, position: &Position) -> Option<&BookDatabaseEntry> {
        let normalized = position.to_sfen(Some(1));
        self.entry_by_normalized_sfen(&normalized)
    }

    /// 正規化 SFEN をキーとしてエントリを1件挿入または置換する。
    pub fn upsert_entry(&mut self, entry: BookDatabaseEntry) -> Result<(), BookError> {
        validate_entry(&entry)?;
        if let Some(existing) =
            self.entries.iter_mut().find(|existing| existing.position.sfen == entry.position.sfen)
        {
            *existing = entry;
        } else {
            self.entries.push(entry);
        }
        Ok(())
    }

    /// 正規化 SFEN をキーとしてエントリを1件削除する。
    pub fn remove_entry_by_sfen(
        &mut self,
        sfen: &str,
    ) -> Result<Option<BookDatabaseEntry>, BookError> {
        let position = BookPosition::from_sfen(sfen, None)?;
        let Some(index) =
            self.entries.iter().position(|entry| entry.position.sfen == position.sfen)
        else {
            return Ok(None);
        };
        Ok(Some(self.entries.remove(index)))
    }

    /// 候補手を1件挿入または更新する。既存候補手の順序は変更しない。
    pub fn upsert_candidate(
        &mut self,
        sfen: &str,
        candidate: BookCandidate,
    ) -> Result<(), BookError> {
        let position = BookPosition::from_sfen(sfen, None)?;
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.position.sfen == position.sfen)
            .ok_or_else(|| BookError::InvalidData("book database entry not found".into()))?;
        if let Some(existing) =
            entry.candidates.iter_mut().find(|existing| existing.mv == candidate.mv)
        {
            *existing = candidate;
        } else {
            entry.candidates.push(candidate);
        }
        Ok(())
    }

    /// 指し手をキーとして候補手を1件削除する。
    pub fn remove_candidate(
        &mut self,
        sfen: &str,
        mv: Move,
    ) -> Result<Option<BookCandidate>, BookError> {
        let position = BookPosition::from_sfen(sfen, None)?;
        let Some(entry) =
            self.entries.iter_mut().find(|entry| entry.position.sfen == position.sfen)
        else {
            return Ok(None);
        };
        let Some(index) = entry.candidates.iter().position(|candidate| candidate.mv == mv) else {
            return Ok(None);
        };
        Ok(Some(entry.candidates.remove(index)))
    }

    /// 候補手を新しい表示インデックスへ移動する。
    pub fn reorder_candidate(
        &mut self,
        sfen: &str,
        mv: Move,
        new_index: usize,
    ) -> Result<(), BookError> {
        let position = BookPosition::from_sfen(sfen, None)?;
        let entry = self
            .entries
            .iter_mut()
            .find(|entry| entry.position.sfen == position.sfen)
            .ok_or_else(|| BookError::InvalidData("book database entry not found".into()))?;
        if new_index >= entry.candidates.len() {
            return Err(BookError::InvalidData(format!(
                "candidate index {new_index} is out of range"
            )));
        }
        let index =
            entry.candidates.iter().position(|candidate| candidate.mv == mv).ok_or_else(|| {
                BookError::InvalidData("book database candidate not found".into())
            })?;
        let candidate = entry.candidates.remove(index);
        entry.candidates.insert(new_index, candidate);
        Ok(())
    }

    /// 正規化局面の一意性と、エントリごとの候補手一意性を検証する。
    pub fn validate(&self) -> Result<(), BookError> {
        let mut positions = HashSet::new();
        for entry in &self.entries {
            if !positions.insert(entry.position.sfen.clone()) {
                return Err(BookError::InvalidData(format!(
                    "duplicate book database position: {}",
                    entry.position.sfen
                )));
            }
            validate_entry(entry)?;
        }
        Ok(())
    }

    /// データベース順で全エントリを返す。
    #[must_use]
    pub fn entries(&self) -> &[BookDatabaseEntry] {
        &self.entries
    }

    /// 局面エントリ数を返す。
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// データベースにエントリがない場合に `true` を返す。
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// このリッチデータベースをネイティブな編集可能検索書籍へ変換する。
    ///
    /// ネイティブ [`BookMove`] が保持できないメタデータは変換時に失われる。
    #[must_use]
    pub fn to_memory_book(&self, options: NativeBookConvertOptions) -> MemoryBook {
        let mut book = MemoryBook::new();
        for entry in &self.entries {
            let moves = entry.candidates.iter().map(|candidate| candidate.to_book_move(options));
            book.extend_moves(entry.position.key, moves);
        }
        book
    }

    /// このリッチデータベースをネイティブなコンパクト読み取り専用検索書籍へ変換する。
    ///
    /// まず [`MemoryBook`] を構築し、その後 [`StaticBook`] のネイティブキー順にソートする。
    #[must_use]
    pub fn to_static_book(&self, options: NativeBookConvertOptions) -> StaticBook {
        StaticBook::from_memory_book(&self.to_memory_book(options))
    }
}

impl BookDatabase {
    fn entry_by_normalized_sfen(&self, sfen: &str) -> Option<&BookDatabaseEntry> {
        self.entries.iter().find(|entry| entry.position.sfen == sfen)
    }
}

impl BookDatabaseEntry {
    /// リッチなデータベースエントリを1件生成する。
    #[must_use]
    pub fn new(
        position: BookPosition,
        candidates: Vec<BookCandidate>,
        metadata: BookEntryMetadata,
    ) -> Self {
        Self { position, candidates, metadata }
    }

    /// デコード済みの DB2016 エントリを1件取り込む。
    pub fn from_yaneuraou(entry: &YaneuraOuBookEntry) -> Result<Self, BookError> {
        let position = BookPosition::from_sfen(entry.sfen(), Some(entry.min_ply()))?;
        let candidates = entry
            .moves()
            .iter()
            .map(|book_move| {
                BookCandidate::new(
                    book_move.mv(),
                    ponder_to_option(book_move.ponder()),
                    book_move.score(),
                    book_move.depth(),
                    None,
                    book_move.count(),
                    BookMoveMetadata::from_yaneuraou(book_move.comment().to_string()),
                )
            })
            .collect();
        let metadata =
            BookEntryMetadata::from_yaneuraou(entry.min_ply(), entry.comment().to_string());
        Ok(Self { position, candidates, metadata })
    }

    /// デコード済みの SBK 状態エントリを1件取り込む。
    pub fn from_sbk(entry: &SbkEntry) -> Result<Self, BookError> {
        let position = BookPosition::from_sfen(entry.sfen(), None)?;
        let candidates = entry
            .moves()
            .iter()
            .map(|book_move| {
                BookCandidate::new(
                    book_move.mv(),
                    None,
                    book_move.evaluation(),
                    None,
                    book_move.weight(),
                    None,
                    BookMoveMetadata::from_sbk(SbkMoveMetadata {
                        raw_move: book_move.raw_move(),
                        evaluation: book_move.evaluation(),
                        weight: book_move.weight(),
                        next_state_id: book_move.next_state_id(),
                    }),
                )
            })
            .collect();
        let evals = entry.evals().iter().map(SbkEvalMetadata::from_sbk_eval).collect();
        let metadata = BookEntryMetadata::from_sbk(SbkEntryMetadata {
            state_id: entry.state_id(),
            state_index: entry.state_index(),
            games: entry.games(),
            won_black: entry.won_black(),
            won_white: entry.won_white(),
            comment: entry.comment().map(ToOwned::to_owned),
            evals,
        });
        Ok(Self { position, candidates, metadata })
    }

    /// このエントリの局面を返す。
    #[must_use]
    pub const fn position(&self) -> &BookPosition {
        &self.position
    }

    /// データベース順で候補手を返す。
    #[must_use]
    pub fn candidates(&self) -> &[BookCandidate] {
        &self.candidates
    }

    /// エントリのメタデータを返す。
    #[must_use]
    pub const fn metadata(&self) -> &BookEntryMetadata {
        &self.metadata
    }
}

impl BookPosition {
    /// 局面からリッチな局面識別情報を構築する。
    #[must_use]
    pub fn from_position(position: &Position, original_ply: Option<u32>) -> Self {
        Self {
            sfen: position.to_sfen(Some(1)),
            key: book_key_from_position(position),
            packed_sfen: Some(position.to_packed_sfen()),
            original_ply,
        }
    }

    /// SFEN を解析して派生検索キーを構築する。
    pub fn from_sfen(sfen: &str, original_ply: Option<u32>) -> Result<Self, BookError> {
        let position =
            Position::from_sfen(sfen).map_err(|err| BookError::InvalidData(err.to_string()))?;
        Ok(Self::from_position(&position, original_ply))
    }

    /// 正規化 SFEN を返す。
    #[must_use]
    pub fn sfen(&self) -> &str {
        &self.sfen
    }

    /// 派生したネイティブ検索キーを返す。
    #[must_use]
    pub const fn key(&self) -> BookKey {
        self.key
    }

    /// 利用可能な場合に Packed SFEN 表現を返す。
    #[must_use]
    pub const fn packed_sfen(&self) -> Option<PackedSfen> {
        self.packed_sfen
    }

    /// ソース手数メタデータが存在する場合に返す。
    ///
    /// DB2016 行は現在、省略された手数を外部リーダーで `0` として公開するため、
    /// そのリーダーからのインポートでは `Some(0)` として保持される。
    #[must_use]
    pub const fn original_ply(&self) -> Option<u32> {
        self.original_ply
    }
}

impl BookCandidate {
    /// リッチな候補手を生成する。
    #[must_use]
    pub const fn new(
        mv: Move,
        ponder: Option<Move>,
        score: Option<i32>,
        depth: Option<u32>,
        weight: Option<i32>,
        count: Option<u64>,
        metadata: BookMoveMetadata,
    ) -> Self {
        Self { mv, ponder, score, depth, weight, count, metadata }
    }

    /// 候補手を返す。
    #[must_use]
    pub const fn mv(&self) -> Move {
        self.mv
    }

    /// ponder 手があれば返す。
    #[must_use]
    pub const fn ponder(&self) -> Option<Move> {
        self.ponder
    }

    /// 評価スコアがあれば返す。
    #[must_use]
    pub const fn score(&self) -> Option<i32> {
        self.score
    }

    /// 探索深度があれば返す。
    #[must_use]
    pub const fn depth(&self) -> Option<u32> {
        self.depth
    }

    /// 符号付き候補手ウェイトがあれば返す。
    #[must_use]
    pub const fn weight(&self) -> Option<i32> {
        self.weight
    }

    /// ソース出現回数があれば返す。
    #[must_use]
    pub const fn count(&self) -> Option<u64> {
        self.count
    }

    /// 指し手のメタデータを返す。
    #[must_use]
    pub const fn metadata(&self) -> &BookMoveMetadata {
        &self.metadata
    }

    fn to_book_move(&self, options: NativeBookConvertOptions) -> BookMove {
        let score = match options.score_policy {
            NativeScorePolicy::Score => self.score,
            NativeScorePolicy::Weight => self.weight,
        }
        .map(|score| narrow_i16(score, options.score_narrowing))
        .unwrap_or(options.missing_score);
        let depth = self
            .depth
            .map(|depth| narrow_u16(depth, options.depth_narrowing))
            .unwrap_or(options.missing_depth);
        BookMove::new(self.mv, score, depth)
    }
}

impl BookEntryMetadata {
    /// 空のエントリメタデータを生成する。
    #[must_use]
    pub const fn new() -> Self {
        Self { yaneuraou: None, sbk: None }
    }

    /// DB2016 エントリメタデータを生成する。
    #[must_use]
    pub fn from_yaneuraou(min_ply: u32, comment: String) -> Self {
        Self { yaneuraou: Some(YaneuraOuEntryMetadata { min_ply, comment }), sbk: None }
    }

    /// SBK エントリメタデータを生成する。
    #[must_use]
    pub fn from_sbk(metadata: SbkEntryMetadata) -> Self {
        Self { yaneuraou: None, sbk: Some(metadata) }
    }

    /// DB2016 メタデータが存在する場合に返す。
    #[must_use]
    pub const fn yaneuraou(&self) -> Option<&YaneuraOuEntryMetadata> {
        self.yaneuraou.as_ref()
    }

    /// SBK メタデータが存在する場合に返す。
    #[must_use]
    pub const fn sbk(&self) -> Option<&SbkEntryMetadata> {
        self.sbk.as_ref()
    }
}

impl BookMoveMetadata {
    /// 空の指し手メタデータを生成する。
    #[must_use]
    pub const fn new() -> Self {
        Self { yaneuraou: None, sbk: None }
    }

    /// DB2016 指し手メタデータを生成する。
    #[must_use]
    pub fn from_yaneuraou(comment: String) -> Self {
        Self { yaneuraou: Some(YaneuraOuMoveMetadata { comment }), sbk: None }
    }

    /// SBK 指し手メタデータを生成する。
    #[must_use]
    pub fn from_sbk(metadata: SbkMoveMetadata) -> Self {
        Self { yaneuraou: None, sbk: Some(metadata) }
    }

    /// DB2016 メタデータが存在する場合に返す。
    #[must_use]
    pub const fn yaneuraou(&self) -> Option<&YaneuraOuMoveMetadata> {
        self.yaneuraou.as_ref()
    }

    /// SBK メタデータが存在する場合に返す。
    #[must_use]
    pub const fn sbk(&self) -> Option<&SbkMoveMetadata> {
        self.sbk.as_ref()
    }
}

impl YaneuraOuEntryMetadata {
    /// ソース行の最小手数を返す。
    #[must_use]
    pub const fn min_ply(&self) -> u32 {
        self.min_ply
    }

    /// ソース局面行に付属するコメントを返す。
    #[must_use]
    pub fn comment(&self) -> &str {
        &self.comment
    }
}

impl YaneuraOuMoveMetadata {
    /// ソース指し手行に付属するコメントを返す。
    #[must_use]
    pub fn comment(&self) -> &str {
        &self.comment
    }
}

impl SbkEntryMetadata {
    /// SBK 状態メタデータを生成する。
    #[must_use]
    pub fn new(
        state_id: Option<i32>,
        state_index: usize,
        games: Option<i32>,
        won_black: Option<i32>,
        won_white: Option<i32>,
        comment: Option<String>,
        evals: Vec<SbkEvalMetadata>,
    ) -> Self {
        Self { state_id, state_index, games, won_black, won_white, comment, evals }
    }

    /// SBK 状態 ID を返す。
    #[must_use]
    pub const fn state_id(&self) -> Option<i32> {
        self.state_id
    }

    /// SBK 状態インデックスを返す。
    #[must_use]
    pub const fn state_index(&self) -> usize {
        self.state_index
    }

    /// SBK 状態に記録されている対局数を返す。
    #[must_use]
    pub const fn games(&self) -> Option<i32> {
        self.games
    }

    /// 先手勝利数メタデータを返す。
    #[must_use]
    pub const fn won_black(&self) -> Option<i32> {
        self.won_black
    }

    /// 後手勝利数メタデータを返す。
    #[must_use]
    pub const fn won_white(&self) -> Option<i32> {
        self.won_white
    }

    /// SBK 状態コメントを返す。
    #[must_use]
    pub fn comment(&self) -> Option<&str> {
        self.comment.as_deref()
    }

    /// SBK 状態の評価レコード一覧を返す。
    #[must_use]
    pub fn evals(&self) -> &[SbkEvalMetadata] {
        &self.evals
    }
}

impl SbkMoveMetadata {
    /// SBK 指し手メタデータを生成する。
    #[must_use]
    pub const fn new(
        raw_move: i32,
        evaluation: Option<i32>,
        weight: Option<i32>,
        next_state_id: Option<i32>,
    ) -> Self {
        Self { raw_move, evaluation, weight, next_state_id }
    }

    /// SBK の生の指し手ワードを返す。
    #[must_use]
    pub const fn raw_move(&self) -> i32 {
        self.raw_move
    }

    /// SBK 指し手評価メタデータを返す。
    #[must_use]
    pub const fn evaluation(&self) -> Option<i32> {
        self.evaluation
    }

    /// SBK 符号付きウェイトメタデータを返す。
    #[must_use]
    pub const fn weight(&self) -> Option<i32> {
        self.weight
    }

    /// SBK の次状態 ID メタデータを返す。
    #[must_use]
    pub const fn next_state_id(&self) -> Option<i32> {
        self.next_state_id
    }
}

impl SbkEvalMetadata {
    /// SBK 評価メタデータを生成する。
    #[must_use]
    pub fn new(
        evaluation_value: Option<i32>,
        depth: Option<i32>,
        sel_depth: Option<i32>,
        nodes: Option<i64>,
        variation: Option<String>,
        engine_name: Option<String>,
    ) -> Self {
        Self { evaluation_value, depth, sel_depth, nodes, variation, engine_name }
    }

    fn from_sbk_eval(eval: &crate::book::SbkEval) -> Self {
        Self {
            evaluation_value: eval.evaluation_value(),
            depth: eval.depth(),
            sel_depth: eval.sel_depth(),
            nodes: eval.nodes(),
            variation: eval.variation().map(ToOwned::to_owned),
            engine_name: eval.engine_name().map(ToOwned::to_owned),
        }
    }

    /// SBK 評価値を返す。
    #[must_use]
    pub const fn evaluation_value(&self) -> Option<i32> {
        self.evaluation_value
    }

    /// SBK 深度を返す。
    #[must_use]
    pub const fn depth(&self) -> Option<i32> {
        self.depth
    }

    /// SBK 選択深度を返す。
    #[must_use]
    pub const fn sel_depth(&self) -> Option<i32> {
        self.sel_depth
    }

    /// SBK の探索ノード数を返す。
    #[must_use]
    pub const fn nodes(&self) -> Option<i64> {
        self.nodes
    }

    /// SBK の PV 文字列を返す。
    #[must_use]
    pub fn variation(&self) -> Option<&str> {
        self.variation.as_deref()
    }

    /// SBK エンジン名を返す。
    #[must_use]
    pub fn engine_name(&self) -> Option<&str> {
        self.engine_name.as_deref()
    }
}

impl Default for NativeBookConvertOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl NativeBookConvertOptions {
    /// デフォルトの変換オプションを生成する。
    #[must_use]
    pub const fn new() -> Self {
        Self {
            missing_score: 0,
            missing_depth: 0,
            score_policy: NativeScorePolicy::Score,
            score_narrowing: I16NarrowingPolicy::Clamp,
            depth_narrowing: U16NarrowingPolicy::Clamp,
        }
    }

    /// リッチ候補手に選択スコアフィールドがない場合に使うネイティブスコアを設定する。
    #[must_use]
    pub const fn with_missing_score(mut self, missing_score: i16) -> Self {
        self.missing_score = missing_score;
        self
    }

    /// リッチ候補手に深度がない場合に使うネイティブ深度を設定する。
    #[must_use]
    pub const fn with_missing_depth(mut self, missing_depth: u16) -> Self {
        self.missing_depth = missing_depth;
        self
    }

    /// ネイティブスコアとして使うリッチ値のポリシーを設定する。
    #[must_use]
    pub const fn with_score_policy(mut self, score_policy: NativeScorePolicy) -> Self {
        self.score_policy = score_policy;
        self
    }
}

fn ponder_to_option(ponder: Move) -> Option<Move> {
    if ponder == Move::MOVE_NONE { None } else { Some(ponder) }
}

fn validate_entry(entry: &BookDatabaseEntry) -> Result<(), BookError> {
    let normalized = BookPosition::from_sfen(entry.position.sfen(), entry.position.original_ply)?;
    if normalized.sfen != entry.position.sfen {
        return Err(BookError::InvalidData(format!(
            "book database position is not normalized: {}",
            entry.position.sfen
        )));
    }
    let mut moves = HashSet::new();
    for candidate in &entry.candidates {
        if !moves.insert(candidate.mv) {
            return Err(BookError::InvalidData(format!(
                "duplicate book database candidate: {}",
                candidate.mv.to_usi()
            )));
        }
    }
    Ok(())
}

fn narrow_i16(value: i32, policy: I16NarrowingPolicy) -> i16 {
    match policy {
        I16NarrowingPolicy::Clamp => value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16,
    }
}

fn narrow_u16(value: u32, policy: U16NarrowingPolicy) -> u16 {
    match policy {
        U16NarrowingPolicy::Clamp => value.min(u32::from(u16::MAX)) as u16,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::{self, InitialPosition};
    use crate::book::{Book, SbkBook, YaneuraOuBook};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    const YANEURAOU_HEADER: &str = "#YANEURAOU-DB2016 1.00";

    const FIELD_SBOOK_STATE: u32 = 3;
    const FIELD_STATE_ID: u32 = 1;
    const FIELD_STATE_GAMES: u32 = 4;
    const FIELD_STATE_WON_BLACK: u32 = 5;
    const FIELD_STATE_WON_WHITE: u32 = 6;
    const FIELD_STATE_POSITION: u32 = 7;
    const FIELD_STATE_COMMENT: u32 = 8;
    const FIELD_STATE_MOVES: u32 = 9;
    const FIELD_STATE_EVALS: u32 = 10;
    const FIELD_MOVE_MOVE: u32 = 1;
    const FIELD_MOVE_EVALUATION: u32 = 2;
    const FIELD_MOVE_WEIGHT: u32 = 3;
    const FIELD_MOVE_NEXT_STATE_ID: u32 = 4;
    const FIELD_EVAL_VALUE: u32 = 1;
    const FIELD_EVAL_DEPTH: u32 = 2;
    const FIELD_EVAL_SEL_DEPTH: u32 = 3;
    const FIELD_EVAL_NODES: u32 = 4;
    const FIELD_EVAL_VARIATION: u32 = 5;
    const FIELD_EVAL_ENGINE_NAME: u32 = 6;

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn test_book_database_preserves_synthetic_entry_metadata() {
        board::init();
        let position = BookPosition::from_sfen(InitialPosition::Standard.to_sfen(), Some(1))
            .expect("position");
        let mv = Move::from_usi("7g7f").expect("move");
        let ponder = Move::from_usi("3c3d");
        let candidate = BookCandidate::new(
            mv,
            ponder,
            Some(120),
            Some(40),
            Some(7),
            Some(3),
            BookMoveMetadata::from_yaneuraou("move note".to_string()),
        );
        let entry = BookDatabaseEntry::new(
            position,
            vec![candidate],
            BookEntryMetadata::from_yaneuraou(1, "entry note".to_string()),
        );
        let database = BookDatabase::try_from_entries(vec![entry]).unwrap();

        let entry = &database.entries()[0];
        assert_eq!(entry.position().original_ply(), Some(1));
        assert_eq!(entry.metadata().yaneuraou().unwrap().comment(), "entry note");
        assert_eq!(entry.candidates()[0].ponder().unwrap().to_usi(), "3c3d");
        assert_eq!(entry.candidates()[0].metadata().yaneuraou().unwrap().comment(), "move note");
    }

    #[test]
    fn test_book_database_rejects_duplicate_positions_and_candidates() {
        board::init();
        let position =
            BookPosition::from_sfen(InitialPosition::Standard.to_sfen(), None).expect("position");
        let candidate = BookCandidate::new(
            Move::from_usi("7g7f").expect("move"),
            None,
            None,
            None,
            None,
            None,
            BookMoveMetadata::new(),
        );
        let duplicate_candidate_entry = BookDatabaseEntry::new(
            position.clone(),
            vec![candidate.clone(), candidate],
            BookEntryMetadata::new(),
        );

        assert!(matches!(
            BookDatabase::try_from_entries(vec![duplicate_candidate_entry]),
            Err(BookError::InvalidData(message)) if message.contains("duplicate book database candidate")
        ));

        let entry_a = BookDatabaseEntry::new(position.clone(), vec![], BookEntryMetadata::new());
        let entry_b = BookDatabaseEntry::new(position, vec![], BookEntryMetadata::new());
        assert!(matches!(
            BookDatabase::try_from_entries(vec![entry_a, entry_b]),
            Err(BookError::InvalidData(message)) if message.contains("duplicate book database position")
        ));
    }

    #[test]
    fn test_book_database_position_identity_ignores_sfen_ply() {
        board::init();
        let position = Position::from_sfen(InitialPosition::Standard.to_sfen()).expect("position");
        let sfen_ply_1 = position.to_sfen(Some(1));
        let sfen_ply_9 = position.to_sfen(Some(9));
        let position_a = BookPosition::from_sfen(&sfen_ply_1, Some(1)).expect("position a");
        let position_b = BookPosition::from_sfen(&sfen_ply_9, Some(9)).expect("position b");

        assert_eq!(position_a.sfen(), sfen_ply_1);
        assert_eq!(position_b.sfen(), sfen_ply_1);
        assert_eq!(position_b.original_ply(), Some(9));

        let entry_a = BookDatabaseEntry::new(position_a, vec![], BookEntryMetadata::new());
        let entry_b = BookDatabaseEntry::new(position_b, vec![], BookEntryMetadata::new());
        assert!(matches!(
            BookDatabase::try_from_entries(vec![entry_a, entry_b]),
            Err(BookError::InvalidData(message)) if message.contains("duplicate book database position")
        ));
    }

    #[test]
    fn test_book_database_lookup_ignores_sfen_ply() {
        board::init();
        let position = Position::from_sfen(InitialPosition::Standard.to_sfen()).expect("position");
        let sfen_ply_1 = position.to_sfen(Some(1));
        let sfen_ply_9 = position.to_sfen(Some(9));
        let entry = BookDatabaseEntry::new(
            BookPosition::from_sfen(&sfen_ply_1, Some(1)).expect("position"),
            vec![],
            BookEntryMetadata::new(),
        );
        let database = BookDatabase::try_from_entries(vec![entry]).unwrap();
        let query_position = Position::from_sfen(&sfen_ply_9).expect("query position");

        assert!(database.entry_by_sfen(&sfen_ply_9).unwrap().is_some());
        assert!(database.entry_by_position(&query_position).is_some());
    }

    #[test]
    fn test_book_database_editing_preserves_candidate_order_until_reorder() {
        board::init();
        let sfen = InitialPosition::Standard.to_sfen();
        let position = BookPosition::from_sfen(sfen, None).expect("position");
        let mv_a = Move::from_usi("7g7f").expect("move a");
        let mv_b = Move::from_usi("2g2f").expect("move b");
        let entry = BookDatabaseEntry::new(position, vec![], BookEntryMetadata::new());
        let mut database = BookDatabase::try_from_entries(vec![entry]).unwrap();

        database
            .upsert_candidate(
                sfen,
                BookCandidate::new(mv_a, None, Some(1), None, None, None, BookMoveMetadata::new()),
            )
            .unwrap();
        database
            .upsert_candidate(
                sfen,
                BookCandidate::new(mv_b, None, Some(2), None, None, None, BookMoveMetadata::new()),
            )
            .unwrap();
        database
            .upsert_candidate(
                sfen,
                BookCandidate::new(mv_a, None, Some(3), None, None, None, BookMoveMetadata::new()),
            )
            .unwrap();

        let entry = database.entry_by_sfen(sfen).unwrap().unwrap();
        assert_eq!(entry.candidates()[0].mv(), mv_a);
        assert_eq!(entry.candidates()[0].score(), Some(3));
        assert_eq!(entry.candidates()[1].mv(), mv_b);

        database.reorder_candidate(sfen, mv_b, 0).unwrap();
        let entry = database.entry_by_sfen(sfen).unwrap().unwrap();
        assert_eq!(entry.candidates()[0].mv(), mv_b);
        assert_eq!(entry.candidates()[1].mv(), mv_a);
    }

    #[test]
    fn test_book_database_exposes_sbk_metadata_constructors() {
        let eval = SbkEvalMetadata::new(
            Some(120),
            Some(18),
            Some(24),
            Some(1234),
            Some("7g7f".to_string()),
            Some("engine".to_string()),
        );
        let metadata = SbkEntryMetadata::new(
            Some(7),
            3,
            Some(11),
            Some(6),
            Some(5),
            Some("root".to_string()),
            vec![eval],
        );

        assert_eq!(metadata.state_id(), Some(7));
        assert_eq!(metadata.state_index(), 3);
        assert_eq!(metadata.games(), Some(11));
        assert_eq!(metadata.won_black(), Some(6));
        assert_eq!(metadata.won_white(), Some(5));
        assert_eq!(metadata.comment(), Some("root"));
        assert_eq!(metadata.evals()[0].evaluation_value(), Some(120));
        assert_eq!(metadata.evals()[0].depth(), Some(18));
        assert_eq!(metadata.evals()[0].sel_depth(), Some(24));
        assert_eq!(metadata.evals()[0].nodes(), Some(1234));
        assert_eq!(metadata.evals()[0].variation(), Some("7g7f"));
        assert_eq!(metadata.evals()[0].engine_name(), Some("engine"));
    }

    #[test]
    fn test_book_database_imports_yaneuraou_entry_from_reader() {
        board::init();
        let path = temp_path("db");
        let sfen = InitialPosition::Standard.to_sfen();
        fs::write(
            &path,
            format!(
                "{YANEURAOU_HEADER}\n\
                 sfen {sfen}\n\
                 #entry note\n\
                 7g7f none 120 40 3 #main move\n\
                 #move note\n"
            ),
        )
        .expect("write yaneuraou fixture");

        let book = YaneuraOuBook::open(&path).expect("open yaneuraou book");
        let source = book.lookup_sfen(sfen).expect("lookup").expect("entry");
        let entry = BookDatabaseEntry::from_yaneuraou(&source).expect("import entry");

        assert_eq!(entry.position().sfen(), sfen);
        assert_eq!(entry.position().original_ply(), Some(1));
        assert_eq!(entry.metadata().yaneuraou().unwrap().min_ply(), 1);
        assert_eq!(entry.metadata().yaneuraou().unwrap().comment(), "entry note");
        assert_eq!(entry.candidates().len(), 1);
        assert_eq!(entry.candidates()[0].mv().to_usi(), "7g7f");
        assert_eq!(entry.candidates()[0].ponder(), None);
        assert_eq!(entry.candidates()[0].score(), Some(120));
        assert_eq!(entry.candidates()[0].depth(), Some(40));
        assert_eq!(entry.candidates()[0].count(), Some(3));
        assert_eq!(
            entry.candidates()[0].metadata().yaneuraou().unwrap().comment(),
            "main move\nmove note"
        );

        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn test_book_database_imports_sbk_entry_from_reader() {
        board::init();
        let path = temp_path("sbk");
        let sfen = InitialPosition::Standard.to_sfen();
        let bytes = encode_sbook(vec![encode_state(vec![
            field_varint(FIELD_STATE_ID, 7),
            field_string(FIELD_STATE_POSITION, sfen),
            field_varint(FIELD_STATE_GAMES, 11),
            field_varint(FIELD_STATE_WON_BLACK, 6),
            field_varint(FIELD_STATE_WON_WHITE, 5),
            field_string(FIELD_STATE_COMMENT, "sbk root"),
            field_message(
                FIELD_STATE_MOVES,
                encode_move(0x0100_7677, Some(-32), Some(42), Some(9)),
            ),
            field_message(
                FIELD_STATE_EVALS,
                encode_eval(
                    Some(120),
                    Some(18),
                    Some(24),
                    Some(1234),
                    Some("7g7f 3c3d"),
                    Some("engine"),
                ),
            ),
        ])]);
        fs::write(&path, bytes).expect("write sbk fixture");

        let book = SbkBook::open(&path).expect("open sbk book");
        let source = book.lookup_sfen(sfen).expect("lookup").expect("entry");
        let entry = BookDatabaseEntry::from_sbk(&source).expect("import entry");

        let metadata = entry.metadata().sbk().expect("sbk metadata");
        assert_eq!(entry.position().sfen(), sfen);
        assert_eq!(entry.position().original_ply(), None);
        assert_eq!(metadata.state_id(), Some(7));
        assert_eq!(metadata.state_index(), 0);
        assert_eq!(metadata.games(), Some(11));
        assert_eq!(metadata.won_black(), Some(6));
        assert_eq!(metadata.won_white(), Some(5));
        assert_eq!(metadata.comment(), Some("sbk root"));
        assert_eq!(metadata.evals()[0].evaluation_value(), Some(120));
        assert_eq!(metadata.evals()[0].depth(), Some(18));
        assert_eq!(metadata.evals()[0].sel_depth(), Some(24));
        assert_eq!(metadata.evals()[0].nodes(), Some(1234));
        assert_eq!(metadata.evals()[0].variation(), Some("7g7f 3c3d"));
        assert_eq!(metadata.evals()[0].engine_name(), Some("engine"));

        assert_eq!(entry.candidates().len(), 1);
        assert_eq!(entry.candidates()[0].mv().to_usi(), "7g7f");
        assert_eq!(entry.candidates()[0].score(), Some(-32));
        assert_eq!(entry.candidates()[0].weight(), Some(42));
        let move_metadata = entry.candidates()[0].metadata().sbk().expect("sbk move metadata");
        assert_eq!(move_metadata.raw_move(), 0x0100_7677);
        assert_eq!(move_metadata.evaluation(), Some(-32));
        assert_eq!(move_metadata.weight(), Some(42));
        assert_eq!(move_metadata.next_state_id(), Some(9));

        fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn test_book_database_to_memory_book_clamps_native_values() {
        board::init();
        let position =
            BookPosition::from_sfen(InitialPosition::Standard.to_sfen(), None).expect("position");
        let key = position.key();
        let candidate = BookCandidate::new(
            Move::from_usi("7g7f").expect("move"),
            None,
            Some(i32::from(i16::MAX) + 1),
            Some(u32::from(u16::MAX) + 1),
            None,
            None,
            BookMoveMetadata::new(),
        );
        let database = BookDatabase::try_from_entries(vec![BookDatabaseEntry::new(
            position,
            vec![candidate],
            BookEntryMetadata::new(),
        )])
        .unwrap();

        let book = database.to_memory_book(NativeBookConvertOptions::default());
        let entry = book.get(key).expect("native entry");

        assert_eq!(entry.moves()[0].score(), i16::MAX);
        assert_eq!(entry.moves()[0].depth(), u16::MAX);
    }

    #[test]
    fn test_book_database_to_static_book_uses_native_lookup_key() {
        board::init();
        let position =
            BookPosition::from_sfen(InitialPosition::Standard.to_sfen(), None).expect("position");
        let key = position.key();
        let database = BookDatabase::try_from_entries(vec![BookDatabaseEntry::new(
            position,
            vec![BookCandidate::new(
                Move::from_usi("7g7f").expect("move"),
                None,
                None,
                None,
                Some(42),
                None,
                BookMoveMetadata::new(),
            )],
            BookEntryMetadata::new(),
        )])
        .unwrap();
        let options = NativeBookConvertOptions::new().with_score_policy(NativeScorePolicy::Weight);

        let book = database.to_static_book(options);
        let entry = book.get(key).expect("static entry");

        assert_eq!(entry.moves()[0].score(), 42);
        assert_eq!(entry.moves()[0].depth(), 0);
    }

    fn temp_path(extension: &str) -> PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let seq = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir()
            .join(format!("rsshogi-book-database-{}-{stamp}-{seq}.{extension}", std::process::id()))
    }

    fn encode_sbook(states: Vec<Vec<u8>>) -> Vec<u8> {
        let mut bytes = Vec::new();
        for state in states {
            bytes.extend(field_message(FIELD_SBOOK_STATE, state));
        }
        bytes
    }

    fn encode_state(fields: Vec<Vec<u8>>) -> Vec<u8> {
        fields.into_iter().flatten().collect()
    }

    fn encode_move(
        raw_move: i32,
        evaluation: Option<i32>,
        weight: Option<i32>,
        next_state_id: Option<i32>,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(field_varint(FIELD_MOVE_MOVE, raw_move as u64));
        if let Some(evaluation) = evaluation {
            bytes.extend(field_varint(FIELD_MOVE_EVALUATION, evaluation as u64));
        }
        if let Some(weight) = weight {
            bytes.extend(field_varint(FIELD_MOVE_WEIGHT, weight as u64));
        }
        if let Some(next_state_id) = next_state_id {
            bytes.extend(field_varint(FIELD_MOVE_NEXT_STATE_ID, next_state_id as u64));
        }
        bytes
    }

    fn encode_eval(
        value: Option<i32>,
        depth: Option<i32>,
        sel_depth: Option<i32>,
        nodes: Option<i64>,
        variation: Option<&str>,
        engine_name: Option<&str>,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        if let Some(value) = value {
            bytes.extend(field_varint(FIELD_EVAL_VALUE, value as u64));
        }
        if let Some(depth) = depth {
            bytes.extend(field_varint(FIELD_EVAL_DEPTH, depth as u64));
        }
        if let Some(sel_depth) = sel_depth {
            bytes.extend(field_varint(FIELD_EVAL_SEL_DEPTH, sel_depth as u64));
        }
        if let Some(nodes) = nodes {
            bytes.extend(field_varint(FIELD_EVAL_NODES, nodes as u64));
        }
        if let Some(variation) = variation {
            bytes.extend(field_string(FIELD_EVAL_VARIATION, variation));
        }
        if let Some(engine_name) = engine_name {
            bytes.extend(field_string(FIELD_EVAL_ENGINE_NAME, engine_name));
        }
        bytes
    }

    fn field_varint(field: u32, value: u64) -> Vec<u8> {
        let mut bytes = varint(u64::from(field << 3));
        bytes.extend(varint(value));
        bytes
    }

    fn field_string(field: u32, value: &str) -> Vec<u8> {
        field_message(field, value.as_bytes().to_vec())
    }

    fn field_message(field: u32, value: Vec<u8>) -> Vec<u8> {
        let mut bytes = varint(u64::from((field << 3) | 2));
        bytes.extend(varint(value.len() as u64));
        bytes.extend(value);
        bytes
    }

    fn varint(mut value: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        loop {
            let mut byte = (value & 0x7f) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            bytes.push(byte);
            if value == 0 {
                return bytes;
            }
        }
    }
}
