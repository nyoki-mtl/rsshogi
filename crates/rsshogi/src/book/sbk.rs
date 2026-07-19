//! SBK 定跡書庫の読み込みと書き出しサポート。

use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::board::position::{PackedSfen, Ply};
use crate::board::{InitialPosition, Position};
use crate::book::{
    BookCandidate, BookControl, BookDatabase, BookDatabaseEntry, BookError, SbkEntryMetadata,
    SbkEvalMetadata,
};
use crate::types::{Color, Move, Move32, PieceType, Square};

const FIELD_SBOOK_AUTHOR: u32 = 1;
const FIELD_SBOOK_DESCRIPTION: u32 = 2;
const FIELD_SBOOK_STATE: u32 = 3;

const FIELD_STATE_ID: u32 = 1;
const FIELD_STATE_BOARD_KEY: u32 = 2;
const FIELD_STATE_HAND_KEY: u32 = 3;
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

#[derive(Debug, Clone)]
struct SbkStateOffset {
    payload_offset: u64,
    payload_bytes: usize,
    state_id: Option<i32>,
}

#[derive(Debug, Clone)]
struct SbkLookupRow {
    key: PackedSfen,
    ply: Ply,
    state_index: usize,
}

#[derive(Debug, Clone)]
struct SbkScan {
    author: Option<String>,
    description: Option<String>,
    states: Vec<SbkStateOffset>,
}

/// コンパクトな Packed SFEN インデックスを持つ読み取り専用 SBK 定跡書庫。
#[derive(Debug, Clone)]
pub struct SbkBook {
    path: PathBuf,
    states: Vec<SbkStateOffset>,
    index: Vec<SbkLookupRow>,
    state_lookup: Vec<Option<SbkLookupRow>>,
    state_id_to_index: HashMap<i32, usize>,
    author: Option<String>,
    description: Option<String>,
    diagnostics: SbkDiagnostics,
}

/// SBK インデックス構築中に発行される進捗通知。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SbkOpenProgress {
    indexed_states: usize,
    total_states: usize,
}

/// SBK インデックス構築中に収集される診断情報。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SbkDiagnostics {
    duplicate_positions: usize,
    unresolved_next_states: usize,
}

/// ルックアップで返されるデコード済み SBK ステート。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbkEntry {
    state_id: Option<i32>,
    state_index: usize,
    sfen: String,
    games: Option<i32>,
    won_black: Option<i32>,
    won_white: Option<i32>,
    comment: Option<String>,
    moves: Vec<SbkMove>,
    evals: Vec<SbkEval>,
}

/// SBK ステートに格納される指し手ごとのメタデータ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbkMove {
    mv: Move,
    raw_move: i32,
    evaluation: Option<i32>,
    weight: Option<i32>,
    next_state_id: Option<i32>,
}

/// SBK に格納されるステートごとの評価メタデータ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SbkEval {
    evaluation_value: Option<i32>,
    depth: Option<i32>,
    sel_depth: Option<i32>,
    nodes: Option<i64>,
    variation: Option<String>,
    engine_name: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct RawSbkState {
    id: Option<i32>,
    games: Option<i32>,
    won_black: Option<i32>,
    won_white: Option<i32>,
    position: Option<String>,
    comment: Option<String>,
    moves: Vec<RawSbkMove>,
    evals: Vec<SbkEval>,
}

#[derive(Debug, Clone, Default)]
struct RawSbkMove {
    raw_move: Option<i32>,
    evaluation: Option<i32>,
    weight: Option<i32>,
    next_state_id: Option<i32>,
}

impl SbkBook {
    /// SBK 定跡書庫を開き、状態グラフ全体をデコードせずに Packed SFEN インデックスを構築する。
    ///
    /// 状態ペイロードはオンデマンドでデコードされる。インデックスは解決済みステートごとに 1 行を保持する。
    pub fn open(path: impl AsRef<Path>) -> Result<Self, BookError> {
        Self::open_with_control(path, |_| BookControl::Continue)
    }

    /// SBK 定跡書庫を開き、インデックス登録したステートごとに進捗を報告する。
    ///
    /// [`BookControl::Cancel`] を返すとインデックス構築を中断し、
    /// [`BookError::Cancelled`] を返す。キャンセルは協調的で、
    /// 進捗コールバック呼び出し時点でのみ検出される。
    pub fn open_with_control(
        path: impl AsRef<Path>,
        mut on_progress: impl FnMut(SbkOpenProgress) -> BookControl,
    ) -> Result<Self, BookError> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;

        let scan = scan_sbook_file(&mut file)?;
        if scan.states.is_empty() {
            return Err(BookError::InvalidFormat("sbk contains no states"));
        }

        let mut book = Self {
            path,
            states: scan.states,
            index: Vec::new(),
            state_lookup: Vec::new(),
            state_id_to_index: HashMap::new(),
            author: scan.author,
            description: scan.description,
            diagnostics: SbkDiagnostics::default(),
        };
        book.state_lookup = vec![None; book.states.len()];
        book.build_state_id_index();
        book.build_index(&mut file, &mut on_progress)?;
        Ok(book)
    }

    /// SBK の著者メタデータ（存在する場合）を返す。
    #[must_use]
    pub fn author(&self) -> Option<&str> {
        self.author.as_deref()
    }

    /// SBK の説明メタデータ（存在する場合）を返す。
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    /// インデックス済みステートの数を返す。
    #[must_use]
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// インデックス済みステートが存在しない場合に `true` を返す。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    /// SBK グラフのインデックス構築中に収集された診断情報を返す。
    #[must_use]
    pub const fn diagnostics(&self) -> &SbkDiagnostics {
        &self.diagnostics
    }

    /// SFEN 文字列で 1 局面を検索する。
    pub fn lookup_sfen(&self, sfen: &str) -> Result<Option<SbkEntry>, BookError> {
        let pos =
            Position::from_sfen(sfen).map_err(|err| BookError::InvalidData(err.to_string()))?;
        self.lookup_position(&pos)
    }

    /// 局面オブジェクトで 1 局面を検索する。
    pub fn lookup_position(&self, pos: &Position) -> Result<Option<SbkEntry>, BookError> {
        let key = pos.to_packed_sfen();
        let Ok(idx) = self.index.binary_search_by(|row| row.key.cmp_sbk_words(&key)) else {
            return Ok(None);
        };
        Ok(Some(self.entry_from_row(&self.index[idx])?))
    }

    /// デコードされたステート ID でインデックス済み SBK ステートを 1 件検索する。
    ///
    /// 負値・未知・未解決のステート ID は `Ok(None)` を返す。
    pub fn lookup_state_id(&self, state_id: i32) -> Result<Option<SbkEntry>, BookError> {
        if state_id < 0 {
            return Ok(None);
        }
        let Some(state_index) = self.state_id_to_index.get(&state_id).copied() else {
            return Ok(None);
        };
        self.lookup_state_index(state_index)
    }

    /// 物理的なステートペイロードインデックスでインデックス済み SBK ステートを 1 件検索する。
    ///
    /// 範囲外または未解決のステートインデックスは `Ok(None)` を返す。
    pub fn lookup_state_index(&self, state_index: usize) -> Result<Option<SbkEntry>, BookError> {
        let Some(row) = self.state_lookup.get(state_index).and_then(Option::as_ref) else {
            return Ok(None);
        };
        Ok(Some(self.entry_from_row(row)?))
    }

    /// `parent.moves()[move_index]` が参照する子 SBK エントリを検索する。
    ///
    /// 範囲外の指し手インデックス、および `next_state_id` を持たない指し手は
    /// `Ok(None)` を返す。
    pub fn child_entry(
        &self,
        parent: &SbkEntry,
        move_index: usize,
    ) -> Result<Option<SbkEntry>, BookError> {
        let Some(book_move) = parent.moves().get(move_index) else {
            return Ok(None);
        };
        let Some(next_state_id) = book_move.next_state_id() else {
            return Ok(None);
        };
        self.lookup_state_id(next_state_id)
    }

    /// インデックス済み SBK エントリを順に走査し、各ステートをオンデマンドでデコードする。
    pub fn iter_entries(&self) -> SbkEntries<'_> {
        SbkEntries { book: self, index: 0 }
    }

    fn build_state_id_index(&mut self) {
        for (state_index, state) in self.states.iter().enumerate() {
            let Some(state_id) = state.state_id else {
                continue;
            };
            if state_id >= 0 {
                self.state_id_to_index.entry(state_id).or_insert(state_index);
            }
        }
    }

    fn build_index(
        &mut self,
        file: &mut File,
        on_progress: &mut impl FnMut(SbkOpenProgress) -> BookControl,
    ) -> Result<(), BookError> {
        let standard = Position::from_sfen(InitialPosition::Standard.to_sfen())
            .map_err(|err| BookError::InvalidData(err.to_string()))?;

        let mut visited = HashSet::new();
        for root_index in 0..self.states.len() {
            if visited.contains(&root_index) {
                continue;
            }

            let raw = self.read_state_from_file(file, root_index)?;
            let root_pos = match raw.position.as_deref() {
                Some(position) => Position::from_sfen(position).map_err(|err| {
                    BookError::InvalidData(format!("invalid sbk state SFEN: {err}"))
                })?,
                None if root_index == 0 => standard.clone(),
                None => continue,
            };

            let mut queue = VecDeque::new();
            queue.push_back((root_index, root_pos));

            while let Some((state_index, fallback_pos)) = queue.pop_front() {
                if state_index >= self.states.len() || !visited.insert(state_index) {
                    continue;
                };

                let raw = self.read_state_from_file(file, state_index)?;
                let pos = match raw.position.as_deref() {
                    Some(position) => Position::from_sfen(position).map_err(|err| {
                        BookError::InvalidData(format!("invalid sbk state SFEN: {err}"))
                    })?,
                    None => fallback_pos,
                };

                let row =
                    SbkLookupRow { key: pos.to_packed_sfen(), ply: pos.game_ply(), state_index };
                if let Some(slot) = self.state_lookup.get_mut(state_index) {
                    *slot = Some(row.clone());
                }
                self.index.push(row);

                for raw_move in &raw.moves {
                    let Some(next_state_id) = raw_move.next_state_id else {
                        continue;
                    };
                    let Some(next_state_index) =
                        self.state_id_to_index.get(&next_state_id).copied()
                    else {
                        continue;
                    };
                    if next_state_index >= self.states.len() || visited.contains(&next_state_index)
                    {
                        continue;
                    }
                    let Some(raw_move_word) = raw_move.raw_move else {
                        continue;
                    };
                    let Ok(mv) = sbk_move_word_to_move(raw_move_word) else {
                        continue;
                    };
                    let mut next_pos = pos.clone();
                    let mv32 = next_pos.move32_from_move(mv);
                    if !next_pos.is_legal_move32(mv32) {
                        continue;
                    }
                    next_pos.apply_move32(mv32);
                    queue.push_back((next_state_index, next_pos));
                }

                match on_progress(SbkOpenProgress {
                    indexed_states: self.index.len(),
                    total_states: self.states.len(),
                }) {
                    BookControl::Continue => {}
                    BookControl::Cancel => return Err(BookError::Cancelled),
                }
            }
        }

        self.index.sort_by(|lhs, rhs| lhs.key.cmp_sbk_words(&rhs.key));
        let before_dedup = self.index.len();
        self.index.dedup_by(|lhs, rhs| lhs.key == rhs.key);
        self.diagnostics.duplicate_positions = before_dedup - self.index.len();
        self.diagnostics.unresolved_next_states = self.states.len().saturating_sub(visited.len());
        Ok(())
    }

    fn read_state_from_file(
        &self,
        file: &mut File,
        state_index: usize,
    ) -> Result<RawSbkState, BookError> {
        let offset = self
            .states
            .get(state_index)
            .ok_or_else(|| BookError::InvalidData(format!("missing sbk state {state_index}")))?;
        file.seek(SeekFrom::Start(offset.payload_offset))?;
        let mut payload = vec![0u8; offset.payload_bytes];
        file.read_exact(&mut payload)?;
        decode_state_payload(&payload)
    }

    fn read_state(&self, state_index: usize) -> Result<RawSbkState, BookError> {
        let mut file = File::open(&self.path)?;
        self.read_state_from_file(&mut file, state_index)
    }

    fn entry_from_row(&self, row: &SbkLookupRow) -> Result<SbkEntry, BookError> {
        let raw = self.read_state(row.state_index)?;
        let mut pos = Position::empty();
        pos.set_packed_sfen(&row.key, false, row.ply).map_err(|err| {
            BookError::InvalidData(format!("invalid SBK Packed SFEN index row: {err:?}"))
        })?;
        let sfen = pos.to_sfen(None);
        Ok(SbkEntry::from_raw(row.state_index, sfen, raw))
    }
}

/// インデックス済み SBK エントリのイテレータ。
#[derive(Debug)]
pub struct SbkEntries<'a> {
    book: &'a SbkBook,
    index: usize,
}

impl Iterator for SbkEntries<'_> {
    type Item = Result<SbkEntry, BookError>;

    fn next(&mut self) -> Option<Self::Item> {
        let row = self.book.index.get(self.index)?;
        self.index += 1;
        Some(self.book.entry_from_row(row))
    }
}

impl SbkOpenProgress {
    /// これまでにインデックス登録したステート数を返す。
    #[must_use]
    pub const fn indexed_states(self) -> usize {
        self.indexed_states
    }

    /// SBK ファイル内のステートペイロードの総数を返す。
    #[must_use]
    pub const fn total_states(self) -> usize {
        self.total_states
    }
}

impl SbkDiagnostics {
    /// ルックアップインデックスから除去された Packed SFEN 重複行の数を返す。
    #[must_use]
    pub const fn duplicate_positions(&self) -> usize {
        self.duplicate_positions
    }

    /// `NextStateId` リンクを辿っても到達できなかったステートペイロードの数を返す。
    #[must_use]
    pub const fn unresolved_next_states(&self) -> usize {
        self.unresolved_next_states
    }
}

impl SbkEntry {
    fn from_raw(state_index: usize, sfen: String, raw: RawSbkState) -> Self {
        let moves = raw
            .moves
            .into_iter()
            .filter_map(|raw_move| raw_move.raw_move.map(|word| (word, raw_move)))
            .filter_map(|(word, raw_move)| {
                let Ok(mv) = sbk_move_word_to_move(word) else {
                    return None;
                };
                Some(SbkMove {
                    mv,
                    raw_move: word,
                    evaluation: raw_move.evaluation,
                    weight: raw_move.weight,
                    next_state_id: raw_move.next_state_id,
                })
            })
            .collect();

        Self {
            state_id: raw.id,
            state_index,
            sfen,
            games: raw.games,
            won_black: raw.won_black,
            won_white: raw.won_white,
            comment: raw.comment,
            moves,
            evals: raw.evals,
        }
    }

    #[must_use]
    pub const fn state_id(&self) -> Option<i32> {
        self.state_id
    }

    #[must_use]
    pub const fn state_index(&self) -> usize {
        self.state_index
    }

    #[must_use]
    pub fn sfen(&self) -> &str {
        &self.sfen
    }

    #[must_use]
    pub const fn games(&self) -> Option<i32> {
        self.games
    }

    #[must_use]
    pub const fn won_black(&self) -> Option<i32> {
        self.won_black
    }

    #[must_use]
    pub const fn won_white(&self) -> Option<i32> {
        self.won_white
    }

    #[must_use]
    pub fn comment(&self) -> Option<&str> {
        self.comment.as_deref()
    }

    #[must_use]
    pub fn moves(&self) -> &[SbkMove] {
        &self.moves
    }

    #[must_use]
    pub fn evals(&self) -> &[SbkEval] {
        &self.evals
    }
}

impl SbkMove {
    #[must_use]
    pub const fn mv(&self) -> Move {
        self.mv
    }

    #[must_use]
    pub const fn raw_move(&self) -> i32 {
        self.raw_move
    }

    #[must_use]
    pub const fn evaluation(&self) -> Option<i32> {
        self.evaluation
    }

    #[must_use]
    pub const fn weight(&self) -> Option<i32> {
        self.weight
    }

    #[must_use]
    pub const fn next_state_id(&self) -> Option<i32> {
        self.next_state_id
    }
}

impl SbkEval {
    #[must_use]
    pub const fn evaluation_value(&self) -> Option<i32> {
        self.evaluation_value
    }

    #[must_use]
    pub const fn depth(&self) -> Option<i32> {
        self.depth
    }

    #[must_use]
    pub const fn sel_depth(&self) -> Option<i32> {
        self.sel_depth
    }

    #[must_use]
    pub const fn nodes(&self) -> Option<i64> {
        self.nodes
    }

    #[must_use]
    pub fn variation(&self) -> Option<&str> {
        self.variation.as_deref()
    }

    #[must_use]
    pub fn engine_name(&self) -> Option<&str> {
        self.engine_name.as_deref()
    }
}

/// [`BookDatabase`] を SBK として書き出すときのオプション。
///
/// 初期 writer は `BookDatabase` 全体を protobuf として再生成する full export のみを扱う。
/// state ID は常に `BookStates[i].Id == i` の連番で、候補手順は入力順を保持する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SbkWriteOptions {
    emit_zero_keys: bool,
}

impl Default for SbkWriteOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl SbkWriteOptions {
    /// 既定の writer オプションを生成する。
    #[must_use]
    pub const fn new() -> Self {
        Self { emit_zero_keys: true }
    }

    /// `BoardKey` / `HandKey` の 0 フィールドを省略する。
    ///
    /// 既定では初期 writer の方針をワイヤ上でも明示するため、両フィールドを 0 として出力する。
    #[must_use]
    pub const fn with_omitted_zero_keys(mut self) -> Self {
        self.emit_zero_keys = false;
        self
    }
}

impl BookDatabase {
    /// この定跡データベースを SBK protobuf として `w` へ書き出す。
    ///
    /// `NextStateId` は候補手を合法に適用して得られる子 SFEN から再構築する。子局面が
    /// `BookDatabase` に存在しない場合は `-1` を出力し、leaf state は追加しない。
    /// 出力 state は root を先に並べ、`BookStates[i].Id == i` になるよう再採番する。
    pub fn write_sbk(
        &self,
        w: &mut impl Write,
        options: &SbkWriteOptions,
    ) -> Result<(), BookError> {
        w.write_all(&self.to_sbk_bytes(options)?)?;
        Ok(())
    }

    /// 便宜的な `Vec<u8>` 版。内部で [`Self::write_sbk`] と同じ full export を行う。
    pub fn to_sbk_bytes(&self, options: &SbkWriteOptions) -> Result<Vec<u8>, BookError> {
        let (states, roots) = prepare_sbk_states(self)?;
        let (order, input_to_output) = sbk_output_order(&states, &roots)?;
        let mut bytes = Vec::new();
        for (output_index, input_index) in order.iter().copied().enumerate() {
            let state = &states[input_index];
            let payload = encode_sbk_state(
                output_index,
                state,
                roots[input_index],
                &input_to_output,
                options,
            )?;
            write_field_message(&mut bytes, FIELD_SBOOK_STATE, &payload)?;
        }
        Ok(bytes)
    }
}

struct PreparedSbkState<'a> {
    entry: &'a BookDatabaseEntry,
    position: Position,
    moves: Vec<PreparedSbkMove>,
}

#[derive(Debug)]
struct PreparedSbkMove {
    raw_move: i32,
    evaluation: i32,
    weight: Option<i32>,
    next_state_index: Option<usize>,
}

fn prepare_sbk_states(
    database: &BookDatabase,
) -> Result<(Vec<PreparedSbkState<'_>>, Vec<bool>), BookError> {
    if database.is_empty() {
        return Err(BookError::InvalidData("cannot write empty SBK database".into()));
    }

    let mut sfen_to_id = HashMap::new();
    let mut states = Vec::with_capacity(database.len());
    for (index, entry) in database.entries().iter().enumerate() {
        let sfen = entry.position().sfen();
        let position = Position::from_sfen(sfen).map_err(|err| {
            BookError::InvalidData(format!("invalid book database position for SBK: {err}"))
        })?;
        let normalized = position.to_sfen(Some(1));
        if normalized != sfen {
            return Err(BookError::InvalidData(format!(
                "book database position is not normalized: {sfen}"
            )));
        }
        if sfen_to_id.insert(normalized, index).is_some() {
            return Err(BookError::InvalidData(format!(
                "duplicate book database position: {sfen}"
            )));
        }
        states.push(PreparedSbkState { entry, position, moves: Vec::new() });
    }

    for state in &mut states {
        let mut seen_moves = HashSet::new();
        for candidate in state.entry.candidates() {
            if !seen_moves.insert(candidate.mv()) {
                return Err(BookError::InvalidData(format!(
                    "duplicate book database candidate: {}",
                    candidate.mv().to_usi()
                )));
            }

            let mv32 = checked_move32(&state.position, candidate.mv())?;
            let raw_move = sbk_move_word_from_position_move(&state.position, candidate.mv())?;
            let mut child = state.position.clone();
            child.apply_move32(mv32);
            let child_sfen = child.to_sfen(Some(1));
            state.moves.push(PreparedSbkMove {
                raw_move,
                evaluation: sbk_candidate_evaluation(candidate),
                weight: sbk_candidate_weight(candidate),
                next_state_index: sfen_to_id.get(&child_sfen).copied(),
            });
        }
    }

    let roots = sbk_root_flags(&states)?;
    Ok((states, roots))
}

fn checked_move32(position: &Position, mv: Move) -> Result<Move32, BookError> {
    if !mv.is_normal() {
        return Err(BookError::InvalidData(format!("invalid SBK move: {}", mv.to_usi())));
    }
    let mv32 = position.move32_from_move(mv);
    if !mv32.is_normal() || !position.is_legal_move32(mv32) {
        return Err(BookError::InvalidData(format!(
            "illegal SBK move {} from {}",
            mv.to_usi(),
            position.to_sfen(None)
        )));
    }
    Ok(mv32)
}

fn sbk_root_flags(states: &[PreparedSbkState<'_>]) -> Result<Vec<bool>, BookError> {
    let mut roots = vec![true; states.len()];
    for state in states {
        for mv in &state.moves {
            if let Some(child) = mv.next_state_index {
                if child >= states.len() {
                    return Err(BookError::InvalidData(format!(
                        "SBK next state id out of range: {child}"
                    )));
                }
                roots[child] = false;
            }
        }
    }

    let mut reachable = vec![false; states.len()];
    for (index, is_root) in roots.iter().copied().enumerate() {
        if is_root {
            mark_reachable_sbk_states(index, states, &mut reachable)?;
        }
    }
    let mut index = 0;
    while index < states.len() {
        if !reachable[index] {
            roots[index] = true;
            mark_reachable_sbk_states(index, states, &mut reachable)?;
        }
        index += 1;
    }
    Ok(roots)
}

fn sbk_output_order(
    states: &[PreparedSbkState<'_>],
    roots: &[bool],
) -> Result<(Vec<usize>, Vec<usize>), BookError> {
    let mut order = Vec::with_capacity(states.len());
    let mut visited = vec![false; states.len()];

    for (index, is_root) in roots.iter().copied().enumerate() {
        if is_root {
            collect_sbk_states_in_graph_order(index, states, &mut visited, &mut order)?;
        }
    }
    for index in 0..states.len() {
        if !visited[index] {
            collect_sbk_states_in_graph_order(index, states, &mut visited, &mut order)?;
        }
    }

    let mut input_to_output = vec![usize::MAX; states.len()];
    for (output, input) in order.iter().copied().enumerate() {
        input_to_output[input] = output;
    }
    debug_assert!(input_to_output.iter().all(|index| *index != usize::MAX));
    Ok((order, input_to_output))
}

fn collect_sbk_states_in_graph_order(
    root: usize,
    states: &[PreparedSbkState<'_>],
    visited: &mut [bool],
    order: &mut Vec<usize>,
) -> Result<(), BookError> {
    let mut queue = VecDeque::from([root]);
    while let Some(index) = queue.pop_front() {
        if index >= states.len() || visited[index] {
            continue;
        }
        visited[index] = true;
        order.push(index);
        for mv in &states[index].moves {
            if let Some(child) = mv.next_state_index {
                if child >= states.len() {
                    return Err(BookError::InvalidData(format!(
                        "SBK next state id out of range: {child}"
                    )));
                }
                queue.push_back(child);
            }
        }
    }
    Ok(())
}

fn mark_reachable_sbk_states(
    root: usize,
    states: &[PreparedSbkState<'_>],
    reachable: &mut [bool],
) -> Result<(), BookError> {
    let mut stack = vec![root];
    while let Some(index) = stack.pop() {
        if index >= states.len() || reachable[index] {
            continue;
        }
        reachable[index] = true;
        for mv in &states[index].moves {
            if let Some(child) = mv.next_state_index {
                stack.push(child);
            }
        }
    }
    Ok(())
}

fn encode_sbk_state(
    state_index: usize,
    state: &PreparedSbkState<'_>,
    is_root: bool,
    input_to_output: &[usize],
    options: &SbkWriteOptions,
) -> Result<Vec<u8>, BookError> {
    let metadata = state.entry.metadata().sbk();
    let mut bytes = Vec::new();
    write_field_i32(&mut bytes, FIELD_STATE_ID, usize_to_i32(state_index, "SBK state id")?);
    if options.emit_zero_keys {
        write_field_u64(&mut bytes, FIELD_STATE_BOARD_KEY, 0);
        write_field_u64(&mut bytes, FIELD_STATE_HAND_KEY, 0);
    }
    if let Some(games) = metadata.and_then(SbkEntryMetadata::games) {
        write_field_i32(&mut bytes, FIELD_STATE_GAMES, games);
    }
    if let Some(won_black) = metadata.and_then(SbkEntryMetadata::won_black) {
        write_field_i32(&mut bytes, FIELD_STATE_WON_BLACK, won_black);
    }
    if let Some(won_white) = metadata.and_then(SbkEntryMetadata::won_white) {
        write_field_i32(&mut bytes, FIELD_STATE_WON_WHITE, won_white);
    }
    if is_root {
        write_field_string(&mut bytes, FIELD_STATE_POSITION, state.entry.position().sfen())?;
    }
    if let Some(comment) = metadata.and_then(SbkEntryMetadata::comment) {
        write_field_string(&mut bytes, FIELD_STATE_COMMENT, comment)?;
    }
    for mv in &state.moves {
        let payload = encode_sbk_move(mv, input_to_output)?;
        write_field_message(&mut bytes, FIELD_STATE_MOVES, &payload)?;
    }
    if let Some(metadata) = metadata {
        for eval in metadata.evals() {
            let payload = encode_sbk_eval(eval)?;
            write_field_message(&mut bytes, FIELD_STATE_EVALS, &payload)?;
        }
    }
    Ok(bytes)
}

fn encode_sbk_move(mv: &PreparedSbkMove, input_to_output: &[usize]) -> Result<Vec<u8>, BookError> {
    let mut bytes = Vec::new();
    write_field_i32(&mut bytes, FIELD_MOVE_MOVE, mv.raw_move);
    write_field_i32(&mut bytes, FIELD_MOVE_EVALUATION, mv.evaluation);
    if let Some(weight) = mv.weight {
        write_field_i32(&mut bytes, FIELD_MOVE_WEIGHT, weight);
    }
    let next_state_id = match mv.next_state_index {
        Some(input_index) => usize_to_i32(
            *input_to_output.get(input_index).ok_or_else(|| {
                BookError::InvalidData(format!("SBK next state id out of range: {input_index}"))
            })?,
            "SBK next state id",
        )?,
        None => -1,
    };
    write_field_i32(&mut bytes, FIELD_MOVE_NEXT_STATE_ID, next_state_id);
    Ok(bytes)
}

fn encode_sbk_eval(eval: &SbkEvalMetadata) -> Result<Vec<u8>, BookError> {
    let mut bytes = Vec::new();
    if let Some(value) = eval.evaluation_value() {
        write_field_i32(&mut bytes, FIELD_EVAL_VALUE, value);
    }
    if let Some(depth) = eval.depth() {
        write_field_i32(&mut bytes, FIELD_EVAL_DEPTH, depth);
    }
    if let Some(sel_depth) = eval.sel_depth() {
        write_field_i32(&mut bytes, FIELD_EVAL_SEL_DEPTH, sel_depth);
    }
    if let Some(nodes) = eval.nodes() {
        write_field_i64(&mut bytes, FIELD_EVAL_NODES, nodes);
    }
    if let Some(variation) = eval.variation() {
        write_field_string(&mut bytes, FIELD_EVAL_VARIATION, variation)?;
    }
    if let Some(engine_name) = eval.engine_name() {
        write_field_string(&mut bytes, FIELD_EVAL_ENGINE_NAME, engine_name)?;
    }
    Ok(bytes)
}

fn sbk_candidate_evaluation(candidate: &BookCandidate) -> i32 {
    candidate.metadata().sbk().and_then(|metadata| metadata.evaluation()).unwrap_or(0)
}

fn sbk_candidate_weight(candidate: &BookCandidate) -> Option<i32> {
    candidate.weight().or_else(|| candidate.metadata().sbk().and_then(|metadata| metadata.weight()))
}

fn usize_to_i32(value: usize, label: &'static str) -> Result<i32, BookError> {
    i32::try_from(value).map_err(|_| BookError::InvalidData(format!("{label} is too large")))
}

fn write_field_i32(bytes: &mut Vec<u8>, field: u32, value: i32) {
    write_field_varint(bytes, field, value as u64);
}

fn write_field_i64(bytes: &mut Vec<u8>, field: u32, value: i64) {
    write_field_varint(bytes, field, value as u64);
}

fn write_field_u64(bytes: &mut Vec<u8>, field: u32, value: u64) {
    write_field_varint(bytes, field, value);
}

fn write_field_string(bytes: &mut Vec<u8>, field: u32, value: &str) -> Result<(), BookError> {
    write_field_message(bytes, field, value.as_bytes())
}

fn write_field_message(bytes: &mut Vec<u8>, field: u32, payload: &[u8]) -> Result<(), BookError> {
    write_field_tag(bytes, field, 2);
    write_varint(
        bytes,
        u64::try_from(payload.len())
            .map_err(|_| BookError::InvalidFormat("protobuf length is too large"))?,
    );
    bytes.extend_from_slice(payload);
    Ok(())
}

fn write_field_varint(bytes: &mut Vec<u8>, field: u32, value: u64) {
    write_field_tag(bytes, field, 0);
    write_varint(bytes, value);
}

fn write_field_tag(bytes: &mut Vec<u8>, field: u32, wire_type: u32) {
    write_varint(bytes, u64::from((field << 3) | wire_type));
}

fn write_varint(bytes: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if value == 0 {
            return;
        }
    }
}

fn scan_sbook_file(file: &mut File) -> Result<SbkScan, BookError> {
    let end = file.seek(SeekFrom::End(0))?;
    file.seek(SeekFrom::Start(0))?;
    let mut input = WireFileInput::new(BufReader::new(file.try_clone()?), end);
    let mut author = None;
    let mut description = None;
    let mut states = Vec::new();

    while !input.is_empty() {
        let (field, wire_type) = input.read_tag()?;
        match (field, wire_type) {
            (FIELD_SBOOK_AUTHOR, 2) => author = Some(input.read_string()?),
            (FIELD_SBOOK_DESCRIPTION, 2) => description = Some(input.read_string()?),
            (FIELD_SBOOK_STATE, 2) => {
                let payload_bytes = input.read_len()?;
                let payload_offset = input.position();
                let state_id = read_state_id_from_wire_payload(&mut input, payload_bytes)?;
                states.push(SbkStateOffset { payload_offset, payload_bytes, state_id });
            }
            _ => input.skip_value(wire_type)?,
        }
    }

    Ok(SbkScan { author, description, states })
}

fn read_state_id_from_wire_payload(
    input: &mut WireFileInput,
    payload_bytes: usize,
) -> Result<Option<i32>, BookError> {
    let payload_end = input
        .position()
        .checked_add(
            u64::try_from(payload_bytes)
                .map_err(|_| BookError::InvalidFormat("protobuf length is too large"))?,
        )
        .ok_or(BookError::InvalidFormat("protobuf length overflow"))?;
    if payload_end > input.end() {
        return Err(BookError::InvalidFormat("truncated length-delimited field"));
    }

    let mut state_id = None;
    while input.position() < payload_end {
        let (field, wire_type) = input.read_tag()?;
        match (field, wire_type) {
            (FIELD_STATE_ID, 0) => {
                state_id = Some(input.read_i32()?);
            }
            _ => input.skip_value(wire_type)?,
        }
        if input.position() > payload_end {
            return Err(BookError::InvalidFormat("protobuf field exceeds state payload"));
        }
        if state_id.is_some() {
            break;
        }
    }

    if input.position() < payload_end {
        let remaining = usize::try_from(payload_end - input.position())
            .map_err(|_| BookError::InvalidFormat("protobuf length is too large"))?;
        input.skip_len(remaining)?;
    }

    Ok(state_id)
}

fn decode_state_payload(payload: &[u8]) -> Result<RawSbkState, BookError> {
    let mut input = WireInput::new(payload);
    let mut state = RawSbkState::default();

    while !input.is_empty() {
        let (field, wire_type) = input.read_tag()?;
        match (field, wire_type) {
            (FIELD_STATE_ID, 0) => state.id = Some(input.read_i32()?),
            (FIELD_STATE_GAMES, 0) => state.games = Some(input.read_i32()?),
            (FIELD_STATE_WON_BLACK, 0) => state.won_black = Some(input.read_i32()?),
            (FIELD_STATE_WON_WHITE, 0) => state.won_white = Some(input.read_i32()?),
            (FIELD_STATE_POSITION, 2) => state.position = Some(input.read_string()?),
            (FIELD_STATE_COMMENT, 2) => state.comment = Some(input.read_string()?),
            (FIELD_STATE_MOVES, 2) => {
                let bytes = input.read_bytes()?;
                state.moves.push(decode_move_payload(bytes)?);
            }
            (FIELD_STATE_EVALS, 2) => {
                let bytes = input.read_bytes()?;
                state.evals.push(decode_eval_payload(bytes)?);
            }
            _ => input.skip_value(wire_type)?,
        }
    }

    Ok(state)
}

fn decode_move_payload(payload: &[u8]) -> Result<RawSbkMove, BookError> {
    let mut input = WireInput::new(payload);
    let mut mv = RawSbkMove::default();

    while !input.is_empty() {
        let (field, wire_type) = input.read_tag()?;
        match (field, wire_type) {
            (FIELD_MOVE_MOVE, 0) => mv.raw_move = Some(input.read_i32()?),
            (FIELD_MOVE_EVALUATION, 0) => mv.evaluation = Some(input.read_i32()?),
            (FIELD_MOVE_WEIGHT, 0) => mv.weight = Some(input.read_i32()?),
            (FIELD_MOVE_NEXT_STATE_ID, 0) => mv.next_state_id = Some(input.read_i32()?),
            _ => input.skip_value(wire_type)?,
        }
    }

    Ok(mv)
}

fn decode_eval_payload(payload: &[u8]) -> Result<SbkEval, BookError> {
    let mut input = WireInput::new(payload);
    let mut eval = SbkEval {
        evaluation_value: None,
        depth: None,
        sel_depth: None,
        nodes: None,
        variation: None,
        engine_name: None,
    };

    while !input.is_empty() {
        let (field, wire_type) = input.read_tag()?;
        match (field, wire_type) {
            (FIELD_EVAL_VALUE, 0) => eval.evaluation_value = Some(input.read_i32()?),
            (FIELD_EVAL_DEPTH, 0) => eval.depth = Some(input.read_i32()?),
            (FIELD_EVAL_SEL_DEPTH, 0) => eval.sel_depth = Some(input.read_i32()?),
            (FIELD_EVAL_NODES, 0) => eval.nodes = Some(input.read_i64()?),
            (FIELD_EVAL_VARIATION, 2) => eval.variation = Some(input.read_string()?),
            (FIELD_EVAL_ENGINE_NAME, 2) => eval.engine_name = Some(input.read_string()?),
            _ => input.skip_value(wire_type)?,
        }
    }

    Ok(eval)
}

fn sbk_move_word_to_move(word: i32) -> Result<Move, BookError> {
    let word = u32::from_le_bytes(word.to_le_bytes());
    let from_rank = word & 0x0f;
    let from_file = (word >> 4) & 0x0f;
    let to_rank = (word >> 8) & 0x0f;
    let to_file = (word >> 12) & 0x0f;
    let promote = (word & (1 << 19)) != 0;
    let piece = (word >> 24) & 0x1f;

    if !(1..=9).contains(&to_file) || !(1..=9).contains(&to_rank) {
        return Err(BookError::InvalidData(format!("invalid SBK move destination: 0x{word:08x}")));
    }

    let to = square_usi(to_file, to_rank)?;
    let usi = if from_file == 0 && from_rank == 0 {
        let piece = sbk_piece_to_drop_usi(piece).ok_or_else(|| {
            BookError::InvalidData(format!("invalid SBK drop piece: 0x{word:08x}"))
        })?;
        format!("{piece}*{to}")
    } else {
        if !(1..=9).contains(&from_file) || !(1..=9).contains(&from_rank) {
            return Err(BookError::InvalidData(format!("invalid SBK move source: 0x{word:08x}")));
        }
        let from = square_usi(from_file, from_rank)?;
        format!("{from}{to}{}", if promote { "+" } else { "" })
    };

    Move::from_usi(&usi)
        .ok_or_else(|| BookError::InvalidData(format!("invalid SBK move USI: {usi}")))
}

fn sbk_move_word_from_position_move(position: &Position, mv: Move) -> Result<i32, BookError> {
    if !mv.is_normal() {
        return Err(BookError::InvalidData(format!("invalid SBK move: {}", mv.to_usi())));
    }

    let (to_file, to_rank) = square_to_sbk_file_rank(mv.to_sq())?;
    let color_bit = if position.turn() == Color::WHITE { 1 << 31 } else { 0 };
    let (from_file, from_rank, piece_type, capture_bits, promote) = if mv.is_drop() {
        let piece_type = mv
            .dropped_piece()
            .ok_or_else(|| BookError::InvalidData(format!("invalid SBK drop: {}", mv.to_usi())))?;
        (0, 0, piece_type, 0, 0)
    } else {
        let from = mv.from_sq();
        let (from_file, from_rank) = square_to_sbk_file_rank(from)?;
        let moved = position.piece_on(from);
        if moved.is_empty() {
            return Err(BookError::InvalidData(format!(
                "SBK move source is empty: {}",
                mv.to_usi()
            )));
        }
        let captured = position.piece_on(mv.to_sq());
        let capture_bits = if captured.is_empty() {
            0
        } else {
            piece_type_to_sbk_index(captured.piece_type()).ok_or_else(|| {
                BookError::InvalidData(format!(
                    "unsupported SBK captured piece: {:?}",
                    captured.piece_type()
                ))
            })?
        };
        (from_file, from_rank, moved.piece_type(), capture_bits, u32::from(mv.is_promotion()))
    };

    let piece_bits = sbk_piece_value(piece_type, position.turn())?;
    let word = color_bit
        | (piece_bits << 24)
        | (capture_bits << 20)
        | (promote << 19)
        | (to_file << 12)
        | (to_rank << 8)
        | (from_file << 4)
        | from_rank;
    Ok(i32::from_ne_bytes(word.to_ne_bytes()))
}

fn square_usi(file: u32, rank: u32) -> Result<String, BookError> {
    let rank_idx = u8::try_from(rank - 1).map_err(|_| BookError::InvalidData("rank".into()))?;
    let rank = char::from(b'a' + rank_idx);
    Ok(format!("{file}{rank}"))
}

fn square_to_sbk_file_rank(square: Square) -> Result<(u32, u32), BookError> {
    if !square.is_on_board() {
        return Err(BookError::InvalidData(format!("invalid SBK square: {square:?}")));
    }
    let file = u32::try_from(square.file().raw() + 1)
        .map_err(|_| BookError::InvalidData(format!("invalid SBK square file: {square:?}")))?;
    let rank = u32::try_from(square.rank().raw() + 1)
        .map_err(|_| BookError::InvalidData(format!("invalid SBK square rank: {square:?}")))?;
    Ok((file, rank))
}

fn sbk_piece_to_drop_usi(piece: u32) -> Option<&'static str> {
    match piece & 0x0f {
        1 => Some("P"),
        2 => Some("L"),
        3 => Some("N"),
        4 => Some("S"),
        5 => Some("G"),
        6 => Some("B"),
        7 => Some("R"),
        _ => None,
    }
}

fn sbk_piece_value(piece_type: PieceType, color: Color) -> Result<u32, BookError> {
    let piece = piece_type_to_sbk_index(piece_type)
        .ok_or_else(|| BookError::InvalidData(format!("unsupported SBK piece: {piece_type:?}")))?;
    Ok(piece | if color == Color::WHITE { 0x10 } else { 0 })
}

fn piece_type_to_sbk_index(piece_type: PieceType) -> Option<u32> {
    if piece_type == PieceType::NONE {
        Some(0)
    } else if piece_type == PieceType::PAWN {
        Some(1)
    } else if piece_type == PieceType::LANCE {
        Some(2)
    } else if piece_type == PieceType::KNIGHT {
        Some(3)
    } else if piece_type == PieceType::SILVER {
        Some(4)
    } else if piece_type == PieceType::GOLD {
        Some(5)
    } else if piece_type == PieceType::BISHOP {
        Some(6)
    } else if piece_type == PieceType::ROOK {
        Some(7)
    } else if piece_type == PieceType::KING {
        Some(8)
    } else if piece_type == PieceType::PRO_PAWN {
        Some(9)
    } else if piece_type == PieceType::PRO_LANCE {
        Some(10)
    } else if piece_type == PieceType::PRO_KNIGHT {
        Some(11)
    } else if piece_type == PieceType::PRO_SILVER {
        Some(12)
    } else if piece_type == PieceType::HORSE {
        Some(14)
    } else if piece_type == PieceType::DRAGON {
        Some(15)
    } else {
        None
    }
}

#[derive(Debug)]
struct WireInput<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> WireInput<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    const fn is_empty(&self) -> bool {
        self.pos >= self.bytes.len()
    }

    fn read_tag(&mut self) -> Result<(u32, u32), BookError> {
        let tag = self.read_varint()?;
        let field =
            u32::try_from(tag >> 3).map_err(|_| BookError::InvalidFormat("field too large"))?;
        let wire_type = u32::try_from(tag & 0x07)
            .map_err(|_| BookError::InvalidFormat("wire type too large"))?;
        if field == 0 {
            return Err(BookError::InvalidFormat("protobuf field number must be non-zero"));
        }
        Ok((field, wire_type))
    }

    fn read_varint(&mut self) -> Result<u64, BookError> {
        let mut shift = 0;
        let mut value = 0u64;
        loop {
            if shift >= 64 {
                return Err(BookError::InvalidFormat("protobuf varint is too long"));
            }
            let byte =
                *self.bytes.get(self.pos).ok_or(BookError::InvalidFormat("truncated protobuf"))?;
            self.pos += 1;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
    }

    fn read_i32(&mut self) -> Result<i32, BookError> {
        Ok(self.read_varint()? as u32 as i32)
    }

    fn read_i64(&mut self) -> Result<i64, BookError> {
        Ok(self.read_varint()? as i64)
    }

    fn read_len(&mut self) -> Result<usize, BookError> {
        usize::try_from(self.read_varint()?)
            .map_err(|_| BookError::InvalidFormat("length-delimited field is too large"))
    }

    fn read_bytes(&mut self) -> Result<&'a [u8], BookError> {
        let len = self.read_len()?;
        let start = self.pos;
        self.skip_len(len)?;
        self.bytes.get(start..self.pos).ok_or(BookError::InvalidFormat("truncated bytes"))
    }

    fn read_string(&mut self) -> Result<String, BookError> {
        let bytes = self.read_bytes()?;
        String::from_utf8(bytes.to_vec())
            .map_err(|_| BookError::InvalidFormat("protobuf string is not UTF-8"))
    }

    fn skip_len(&mut self, len: usize) -> Result<(), BookError> {
        self.pos = self
            .pos
            .checked_add(len)
            .ok_or(BookError::InvalidFormat("protobuf length overflow"))?;
        if self.pos > self.bytes.len() {
            return Err(BookError::InvalidFormat("truncated length-delimited field"));
        }
        Ok(())
    }

    fn skip_value(&mut self, wire_type: u32) -> Result<(), BookError> {
        match wire_type {
            0 => {
                let _ = self.read_varint()?;
                Ok(())
            }
            1 => self.skip_len(8),
            2 => {
                let len = self.read_len()?;
                self.skip_len(len)
            }
            5 => self.skip_len(4),
            _ => Err(BookError::Unsupported("unsupported protobuf wire type")),
        }
    }
}

#[derive(Debug)]
struct WireFileInput {
    reader: BufReader<File>,
    pos: u64,
    end: u64,
}

impl WireFileInput {
    const fn new(reader: BufReader<File>, end: u64) -> Self {
        Self { reader, pos: 0, end }
    }

    const fn position(&self) -> u64 {
        self.pos
    }

    const fn end(&self) -> u64 {
        self.end
    }

    const fn is_empty(&self) -> bool {
        self.pos >= self.end
    }

    fn read_tag(&mut self) -> Result<(u32, u32), BookError> {
        let tag = self.read_varint()?;
        let field =
            u32::try_from(tag >> 3).map_err(|_| BookError::InvalidFormat("field too large"))?;
        let wire_type = u32::try_from(tag & 0x07)
            .map_err(|_| BookError::InvalidFormat("wire type too large"))?;
        if field == 0 {
            return Err(BookError::InvalidFormat("protobuf field number must be non-zero"));
        }
        Ok((field, wire_type))
    }

    fn read_varint(&mut self) -> Result<u64, BookError> {
        let mut shift = 0;
        let mut value = 0u64;
        loop {
            if shift >= 64 {
                return Err(BookError::InvalidFormat("protobuf varint is too long"));
            }
            let mut byte = [0u8; 1];
            self.reader.read_exact(&mut byte).map_err(|err| {
                if err.kind() == std::io::ErrorKind::UnexpectedEof {
                    BookError::InvalidFormat("truncated protobuf")
                } else {
                    BookError::Io(err)
                }
            })?;
            self.pos = self
                .pos
                .checked_add(1)
                .ok_or(BookError::InvalidFormat("protobuf position overflow"))?;
            value |= u64::from(byte[0] & 0x7f) << shift;
            if byte[0] & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
    }

    fn read_len(&mut self) -> Result<usize, BookError> {
        usize::try_from(self.read_varint()?)
            .map_err(|_| BookError::InvalidFormat("length-delimited field is too large"))
    }

    fn read_i32(&mut self) -> Result<i32, BookError> {
        Ok(self.read_varint()? as u32 as i32)
    }

    fn read_bytes(&mut self) -> Result<Vec<u8>, BookError> {
        let len = self.read_len()?;
        let end = self
            .position()
            .checked_add(
                u64::try_from(len)
                    .map_err(|_| BookError::InvalidFormat("protobuf length is too large"))?,
            )
            .ok_or(BookError::InvalidFormat("protobuf length overflow"))?;
        if end > self.end {
            return Err(BookError::InvalidFormat("truncated length-delimited field"));
        }
        let mut bytes = vec![0u8; len];
        self.reader.read_exact(&mut bytes)?;
        self.pos = end;
        Ok(bytes)
    }

    fn read_string(&mut self) -> Result<String, BookError> {
        String::from_utf8(self.read_bytes()?)
            .map_err(|_| BookError::InvalidFormat("protobuf string is not UTF-8"))
    }

    fn skip_len(&mut self, len: usize) -> Result<(), BookError> {
        let next = self
            .position()
            .checked_add(
                u64::try_from(len)
                    .map_err(|_| BookError::InvalidFormat("protobuf length is too large"))?,
            )
            .ok_or(BookError::InvalidFormat("protobuf length overflow"))?;
        if next > self.end {
            return Err(BookError::InvalidFormat("truncated length-delimited field"));
        }
        let buffered = self.reader.buffer().len();
        if len <= buffered {
            self.reader.consume(len);
        } else {
            let remaining = len - buffered;
            self.reader.consume(buffered);
            self.reader.seek(SeekFrom::Current(
                i64::try_from(remaining)
                    .map_err(|_| BookError::InvalidFormat("protobuf length is too large"))?,
            ))?;
        }
        self.pos = next;
        Ok(())
    }

    fn skip_value(&mut self, wire_type: u32) -> Result<(), BookError> {
        match wire_type {
            0 => {
                let _ = self.read_varint()?;
                Ok(())
            }
            1 => self.skip_len(8),
            2 => {
                let len = self.read_len()?;
                self.skip_len(len)
            }
            5 => self.skip_len(4),
            _ => Err(BookError::Unsupported("unsupported protobuf wire type")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::board;
    use crate::book::{BookEntryMetadata, BookMoveMetadata, BookPosition, SbkMoveMetadata};

    use super::*;

    #[test]
    fn test_sbk_open_lookup_explicit_position() {
        let path = temp_path("explicit");
        let bytes = encode_sbook(vec![encode_state(vec![
            field_varint(FIELD_STATE_ID, 0),
            field_string(FIELD_STATE_POSITION, InitialPosition::Standard.to_sfen()),
            field_varint(FIELD_STATE_GAMES, 10),
            field_varint(FIELD_STATE_WON_BLACK, 6),
            field_varint(FIELD_STATE_WON_WHITE, 4),
            field_string(FIELD_STATE_COMMENT, "root"),
            field_message(FIELD_STATE_MOVES, encode_move(0x0100_7677, Some(3), Some(42), None)),
            field_message(
                FIELD_STATE_EVALS,
                encode_eval(
                    Some(120),
                    Some(18),
                    Some(24),
                    Some(1234),
                    Some("7g7f"),
                    Some("engine"),
                ),
            ),
        ])]);
        fs::write(&path, bytes).expect("write fixture");

        let mut progress = Vec::new();
        let book = SbkBook::open_with_control(&path, |step| {
            progress.push((step.indexed_states(), step.total_states()));
            BookControl::Continue
        })
        .expect("open sbk");
        let entry =
            book.lookup_sfen(InitialPosition::Standard.to_sfen()).expect("lookup").expect("entry");

        assert_eq!(book.len(), 1);
        assert_eq!(progress, vec![(1, 1)]);
        assert_eq!(book.diagnostics().duplicate_positions(), 0);
        assert_eq!(book.diagnostics().unresolved_next_states(), 0);
        assert_eq!(entry.state_id(), Some(0));
        assert_eq!(entry.games(), Some(10));
        assert_eq!(entry.won_black(), Some(6));
        assert_eq!(entry.won_white(), Some(4));
        assert_eq!(entry.comment(), Some("root"));
        assert_eq!(entry.moves()[0].mv().to_usi(), "7g7f");
        assert_eq!(entry.moves()[0].evaluation(), Some(3));
        assert_eq!(entry.moves()[0].weight(), Some(42));
        assert_eq!(entry.evals()[0].evaluation_value(), Some(120));
        assert_eq!(entry.evals()[0].depth(), Some(18));
        assert_eq!(entry.evals()[0].sel_depth(), Some(24));
        assert_eq!(entry.evals()[0].nodes(), Some(1234));
        assert_eq!(entry.evals()[0].variation(), Some("7g7f"));
        assert_eq!(entry.evals()[0].engine_name(), Some("engine"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_sbk_open_with_control_cancels() {
        let path = temp_path("cancel");
        let bytes = encode_sbook(vec![encode_state(vec![
            field_varint(FIELD_STATE_ID, 0),
            field_string(FIELD_STATE_POSITION, InitialPosition::Standard.to_sfen()),
        ])]);
        fs::write(&path, bytes).expect("write fixture");

        let result = SbkBook::open_with_control(&path, |_| BookControl::Cancel);

        assert!(matches!(result, Err(BookError::Cancelled)));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_sbk_index_follows_next_state_ids() {
        let path = temp_path("next");
        let bytes = encode_sbook(vec![
            encode_state(vec![
                field_varint(FIELD_STATE_ID, 0),
                field_message(FIELD_STATE_MOVES, encode_move(0x0100_7677, None, Some(1), Some(1))),
            ]),
            encode_state(vec![
                field_varint(FIELD_STATE_ID, 1),
                field_message(FIELD_STATE_MOVES, encode_move(0x0100_3433, None, Some(2), None)),
            ]),
        ]);
        fs::write(&path, bytes).expect("write fixture");

        let book = SbkBook::open(&path).expect("open sbk");
        let mut pos = Position::from_sfen(InitialPosition::Standard.to_sfen()).unwrap();
        pos.apply_move(Move::from_usi("7g7f").unwrap());
        let entry = book.lookup_position(&pos).expect("lookup").expect("entry");

        assert_eq!(book.len(), 2);
        assert_eq!(entry.state_id(), Some(1));
        assert_eq!(entry.moves()[0].mv().to_usi(), "3c3d");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_sbk_lookup_state_id_matches_sfen_lookup() {
        let path = temp_path("state-id");
        let bytes = encode_sbook(vec![encode_state(vec![
            field_varint(FIELD_STATE_ID, 7),
            field_string(FIELD_STATE_POSITION, InitialPosition::Standard.to_sfen()),
            field_string(FIELD_STATE_COMMENT, "known"),
        ])]);
        fs::write(&path, bytes).expect("write fixture");

        let book = SbkBook::open(&path).expect("open sbk");
        let by_sfen =
            book.lookup_sfen(InitialPosition::Standard.to_sfen()).expect("lookup sfen").unwrap();
        let by_state_id = book.lookup_state_id(7).expect("lookup state id").unwrap();

        assert_eq!(by_state_id, by_sfen);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_sbk_child_entry_follows_state_id_not_physical_index() {
        let path = temp_path("child-state-id");
        let bytes = encode_sbook(vec![
            encode_state(vec![
                field_varint(FIELD_STATE_ID, 10),
                field_message(FIELD_STATE_MOVES, encode_move(0x0100_7677, None, Some(1), Some(20))),
            ]),
            encode_state(vec![
                field_varint(FIELD_STATE_ID, 20),
                field_message(FIELD_STATE_MOVES, encode_move(0x0100_3433, None, Some(2), None)),
            ]),
        ]);
        fs::write(&path, bytes).expect("write fixture");

        let book = SbkBook::open(&path).expect("open sbk");
        let parent =
            book.lookup_sfen(InitialPosition::Standard.to_sfen()).expect("lookup root").unwrap();
        let child = book.child_entry(&parent, 0).expect("lookup child").unwrap();
        let by_id = book.lookup_state_id(20).expect("lookup state id").unwrap();
        let by_index = book.lookup_state_index(1).expect("lookup state index").unwrap();
        let mut child_pos = Position::from_sfen(InitialPosition::Standard.to_sfen()).unwrap();
        child_pos.apply_move(Move::from_usi("7g7f").unwrap());

        assert_eq!(book.len(), 2);
        assert_eq!(parent.state_id(), Some(10));
        assert_eq!(child.state_id(), Some(20));
        assert_eq!(child.state_index(), 1);
        assert_eq!(child.sfen(), child_pos.to_sfen(None));
        assert_eq!(child, by_id);
        assert_eq!(child, by_index);
        assert_eq!(child.moves()[0].mv().to_usi(), "3c3d");

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_sbk_direct_state_lookup_missing_cases_return_none() {
        let path = temp_path("missing-state");
        let bytes = encode_sbook(vec![encode_state(vec![
            field_varint(FIELD_STATE_ID, 0),
            field_message(FIELD_STATE_MOVES, encode_move(0x0100_7677, None, Some(1), None)),
        ])]);
        fs::write(&path, bytes).expect("write fixture");

        let book = SbkBook::open(&path).expect("open sbk");
        let parent =
            book.lookup_sfen(InitialPosition::Standard.to_sfen()).expect("lookup root").unwrap();

        assert!(book.lookup_state_id(-1).expect("lookup negative state id").is_none());
        assert!(book.lookup_state_id(999).expect("lookup missing state id").is_none());
        assert!(book.lookup_state_index(999).expect("lookup missing state index").is_none());
        assert!(book.child_entry(&parent, 0).expect("lookup missing child").is_none());
        assert!(book.child_entry(&parent, 99).expect("lookup out-of-range child").is_none());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_sbk_iter_entries_decodes_indexed_states() {
        let path = temp_path("iter");
        let bytes = encode_sbook(vec![
            encode_state(vec![
                field_varint(FIELD_STATE_ID, 0),
                field_message(FIELD_STATE_MOVES, encode_move(0x0100_7677, None, Some(1), Some(1))),
            ]),
            encode_state(vec![field_varint(FIELD_STATE_ID, 1)]),
        ]);
        fs::write(&path, bytes).expect("write fixture");

        let book = SbkBook::open(&path).expect("open sbk");
        let entries = book.iter_entries().collect::<Result<Vec<_>, _>>().unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].state_id(), Some(0));
        assert_eq!(entries[0].sfen(), InitialPosition::Standard.to_sfen());
        assert_eq!(entries[1].state_id(), Some(1));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_sbk_index_skips_illegal_graph_moves() {
        let path = temp_path("illegal-edge");
        let bytes = encode_sbook(vec![
            encode_state(vec![
                field_varint(FIELD_STATE_ID, 0),
                field_message(FIELD_STATE_MOVES, encode_move(0x0600_3300, None, Some(1), Some(1))),
            ]),
            encode_state(vec![field_varint(FIELD_STATE_ID, 1)]),
        ]);
        fs::write(&path, bytes).expect("write fixture");

        let book = SbkBook::open(&path).expect("open sbk");

        assert_eq!(book.len(), 1);
        assert_eq!(book.diagnostics().unresolved_next_states(), 1);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_sbk_index_uses_unreachable_explicit_position_roots() {
        let path = temp_path("explicit-root");
        let mut pos = Position::from_sfen(InitialPosition::Standard.to_sfen()).unwrap();
        pos.apply_move(Move::from_usi("7g7f").unwrap());
        let sfen = pos.to_sfen(None);
        let bytes = encode_sbook(vec![
            encode_state(vec![field_varint(FIELD_STATE_ID, 0)]),
            encode_state(vec![
                field_varint(FIELD_STATE_ID, 1),
                field_string(FIELD_STATE_POSITION, &sfen),
            ]),
        ]);
        fs::write(&path, bytes).expect("write fixture");

        let book = SbkBook::open(&path).expect("open sbk");
        let entry = book.lookup_sfen(&sfen).expect("lookup").expect("entry");

        assert_eq!(book.len(), 2);
        assert_eq!(book.diagnostics().unresolved_next_states(), 0);
        assert_eq!(entry.state_id(), Some(1));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_book_database_write_sbk_minimal_roundtrip() {
        board::init();
        let database = BookDatabase::try_from_entries(vec![database_entry(
            InitialPosition::Standard.to_sfen(),
            vec![db_candidate("7g7f", Some(999), None, None, None)],
            BookEntryMetadata::new(),
        )])
        .unwrap();

        let bytes = database.to_sbk_bytes(&SbkWriteOptions::new()).expect("write sbk");
        let raw_states = decode_exported_states(&bytes);

        assert_eq!(raw_states.len(), 1);
        assert_eq!(raw_states[0].id, Some(0));
        assert_eq!(raw_states[0].position.as_deref(), Some(InitialPosition::Standard.to_sfen()));
        assert_eq!(raw_states[0].moves[0].raw_move, Some(0x0100_7677));
        assert_eq!(raw_states[0].moves[0].evaluation, Some(0));
        assert_eq!(raw_states[0].moves[0].weight, None);
        assert_eq!(raw_states[0].moves[0].next_state_id, Some(-1));

        let (book, path) = open_written_sbk("writer-minimal", &bytes);
        let entry = book.lookup_sfen(InitialPosition::Standard.to_sfen()).unwrap().expect("entry");

        assert_eq!(entry.state_id(), Some(0));
        assert_eq!(entry.moves()[0].mv().to_usi(), "7g7f");
        assert_eq!(entry.moves()[0].evaluation(), Some(0));
        assert_eq!(entry.moves()[0].weight(), None);
        assert_eq!(entry.moves()[0].next_state_id(), Some(-1));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_book_database_write_sbk_branching_next_state_ids() {
        board::init();
        let root = InitialPosition::Standard.to_sfen();
        let child_a = next_sfen(root, "7g7f");
        let child_b = next_sfen(root, "2g2f");
        let database = BookDatabase::try_from_entries(vec![
            database_entry(
                root,
                vec![
                    db_candidate("7g7f", None, None, None, None),
                    db_candidate("2g2f", None, None, None, None),
                ],
                BookEntryMetadata::new(),
            ),
            database_entry(
                &child_a,
                vec![db_candidate("3c3d", None, None, None, None)],
                BookEntryMetadata::new(),
            ),
            database_entry(&child_b, vec![], BookEntryMetadata::new()),
        ])
        .unwrap();

        let bytes = database.to_sbk_bytes(&SbkWriteOptions::new()).expect("write sbk");
        let raw_states = decode_exported_states(&bytes);

        assert_eq!(raw_states[0].position.as_deref(), Some(root));
        assert_eq!(raw_states[1].position, None);
        assert_eq!(raw_states[2].position, None);
        assert_eq!(raw_states[0].moves[0].next_state_id, Some(1));
        assert_eq!(raw_states[0].moves[1].next_state_id, Some(2));

        let (book, path) = open_written_sbk("writer-branching", &bytes);
        let parent = book.lookup_sfen(root).unwrap().expect("parent");
        let first_child = book.child_entry(&parent, 0).unwrap().expect("first child");
        let second_child = book.child_entry(&parent, 1).unwrap().expect("second child");

        assert_eq!(parent.moves()[0].mv().to_usi(), "7g7f");
        assert_eq!(parent.moves()[1].mv().to_usi(), "2g2f");
        assert_eq!(first_child.state_id(), Some(1));
        assert_eq!(first_child.sfen(), child_a);
        assert_eq!(second_child.state_id(), Some(2));
        assert_eq!(second_child.sfen(), child_b);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_book_database_write_sbk_reorders_roots_before_children() {
        board::init();
        let root = InitialPosition::Standard.to_sfen();
        let child = next_sfen(root, "7g7f");
        let database = BookDatabase::try_from_entries(vec![
            database_entry(&child, vec![], BookEntryMetadata::new()),
            database_entry(
                root,
                vec![db_candidate("7g7f", None, None, None, None)],
                BookEntryMetadata::new(),
            ),
        ])
        .unwrap();

        let bytes = database.to_sbk_bytes(&SbkWriteOptions::new()).expect("write sbk");
        let raw_states = decode_exported_states(&bytes);

        assert_eq!(raw_states[0].id, Some(0));
        assert_eq!(raw_states[0].position.as_deref(), Some(root));
        assert_eq!(raw_states[0].moves[0].next_state_id, Some(1));
        assert_eq!(raw_states[1].id, Some(1));
        assert_eq!(raw_states[1].position, None);

        let (book, path) = open_written_sbk("writer-root-first", &bytes);
        let parent = book.lookup_sfen(root).unwrap().expect("parent");
        let child_entry = book.child_entry(&parent, 0).unwrap().expect("child");

        assert_eq!(parent.state_id(), Some(0));
        assert_eq!(child_entry.state_id(), Some(1));
        assert_eq!(child_entry.sfen(), child);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_book_database_write_sbk_multiple_roots_have_positions() {
        board::init();
        let root_a = InitialPosition::Standard.to_sfen();
        let root_b = next_sfen(root_a, "7g7f");
        let database = BookDatabase::try_from_entries(vec![
            database_entry(root_a, vec![], BookEntryMetadata::new()),
            database_entry(&root_b, vec![], BookEntryMetadata::new()),
        ])
        .unwrap();

        let bytes = database.to_sbk_bytes(&SbkWriteOptions::new()).expect("write sbk");
        let raw_states = decode_exported_states(&bytes);

        assert_eq!(raw_states[0].position.as_deref(), Some(root_a));
        assert_eq!(raw_states[1].position.as_deref(), Some(normalized_sfen(&root_b).as_str()));

        let (book, path) = open_written_sbk("writer-multiple-roots", &bytes);
        assert_eq!(book.lookup_sfen(root_a).unwrap().unwrap().state_id(), Some(0));
        assert_eq!(book.lookup_sfen(&root_b).unwrap().unwrap().state_id(), Some(1));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_book_database_write_sbk_missing_leaf_next_state_id_minus_one() {
        board::init();
        let root = InitialPosition::Standard.to_sfen();
        let leaf = next_sfen(root, "7g7f");
        let database = BookDatabase::try_from_entries(vec![database_entry(
            root,
            vec![db_candidate("7g7f", None, None, None, None)],
            BookEntryMetadata::new(),
        )])
        .unwrap();

        let bytes = database.to_sbk_bytes(&SbkWriteOptions::new()).expect("write sbk");
        let (book, path) = open_written_sbk("writer-missing-leaf", &bytes);
        let entry = book.lookup_sfen(root).unwrap().expect("entry");

        assert_eq!(book.len(), 1);
        assert_eq!(entry.moves()[0].next_state_id(), Some(-1));
        assert!(book.lookup_sfen(&leaf).unwrap().is_none());

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_book_database_write_sbk_metadata_roundtrip() {
        board::init();
        let eval = SbkEvalMetadata::new(
            Some(120),
            Some(18),
            Some(24),
            Some(1234),
            Some("7g7f 3c3d".to_string()),
            Some("engine".to_string()),
        );
        let entry_metadata = BookEntryMetadata::from_sbk(SbkEntryMetadata::new(
            Some(99),
            7,
            Some(11),
            Some(6),
            Some(5),
            Some("sbk root".to_string()),
            vec![eval],
        ));
        let candidate = db_candidate("7g7f", Some(999), Some(42), Some(2), Some(9));
        let database = BookDatabase::try_from_entries(vec![database_entry(
            InitialPosition::Standard.to_sfen(),
            vec![candidate],
            entry_metadata,
        )])
        .unwrap();

        let bytes = database.to_sbk_bytes(&SbkWriteOptions::new()).expect("write sbk");
        let (book, path) = open_written_sbk("writer-metadata", &bytes);
        let entry = book.lookup_sfen(InitialPosition::Standard.to_sfen()).unwrap().expect("entry");

        assert_eq!(entry.state_id(), Some(0));
        assert_eq!(entry.games(), Some(11));
        assert_eq!(entry.won_black(), Some(6));
        assert_eq!(entry.won_white(), Some(5));
        assert_eq!(entry.comment(), Some("sbk root"));
        assert_eq!(entry.moves()[0].evaluation(), Some(2));
        assert_eq!(entry.moves()[0].weight(), Some(42));
        assert_eq!(entry.evals()[0].evaluation_value(), Some(120));
        assert_eq!(entry.evals()[0].depth(), Some(18));
        assert_eq!(entry.evals()[0].sel_depth(), Some(24));
        assert_eq!(entry.evals()[0].nodes(), Some(1234));
        assert_eq!(entry.evals()[0].variation(), Some("7g7f 3c3d"));
        assert_eq!(entry.evals()[0].engine_name(), Some("engine"));

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_book_database_write_sbk_rejects_invalid_and_illegal_moves() {
        board::init();
        let invalid = BookDatabase::try_from_entries(vec![database_entry(
            InitialPosition::Standard.to_sfen(),
            vec![BookCandidate::new(
                Move::MOVE_NONE,
                None,
                None,
                None,
                None,
                None,
                BookMoveMetadata::new(),
            )],
            BookEntryMetadata::new(),
        )])
        .unwrap();
        assert!(matches!(
            invalid.to_sbk_bytes(&SbkWriteOptions::new()),
            Err(BookError::InvalidData(message)) if message.contains("invalid SBK move")
        ));

        let illegal = BookDatabase::try_from_entries(vec![database_entry(
            InitialPosition::Standard.to_sfen(),
            vec![db_candidate("3c3d", None, None, None, None)],
            BookEntryMetadata::new(),
        )])
        .unwrap();
        assert!(matches!(
            illegal.to_sbk_bytes(&SbkWriteOptions::new()),
            Err(BookError::InvalidData(message)) if message.contains("illegal SBK move")
        ));
    }

    #[test]
    fn test_book_database_write_sbk_is_deterministic() {
        board::init();
        let root = InitialPosition::Standard.to_sfen();
        let child = next_sfen(root, "7g7f");
        let database = BookDatabase::try_from_entries(vec![
            database_entry(
                root,
                vec![
                    db_candidate("7g7f", Some(10), Some(3), Some(1), Some(2)),
                    db_candidate("2g2f", Some(20), None, None, None),
                ],
                BookEntryMetadata::new(),
            ),
            database_entry(&child, vec![], BookEntryMetadata::new()),
        ])
        .unwrap();

        let first = database.to_sbk_bytes(&SbkWriteOptions::new()).expect("first write");
        let second = database.to_sbk_bytes(&SbkWriteOptions::new()).expect("second write");

        assert_eq!(first, second);
    }

    #[test]
    fn test_sbk_move_word_writer_uses_bookconv_piece_mapping_and_capture_bits() {
        board::init();
        let sfen = "4k4/7g1/9/9/9/9/9/1B7/4K4 b - 1";
        let database = BookDatabase::try_from_entries(vec![database_entry(
            sfen,
            vec![db_candidate("8h2b+", None, None, None, None)],
            BookEntryMetadata::new(),
        )])
        .unwrap();

        let bytes = database.to_sbk_bytes(&SbkWriteOptions::new()).expect("write sbk");
        let raw_states = decode_exported_states(&bytes);

        assert_eq!(raw_states[0].moves[0].raw_move, Some(0x0658_2288));
    }

    #[test]
    #[ignore = "requires RSSHOGI_SBK_FIXTURE_DIR"]
    fn test_sbk_shogihome_fixtures_smoke() {
        let fixture_dir = std::env::var_os("RSSHOGI_SBK_FIXTURE_DIR")
            .map(PathBuf::from)
            .expect("set RSSHOGI_SBK_FIXTURE_DIR to a directory containing SBK fixtures");
        for name in ["shogigui01.sbk", "shogigui02.sbk", "sbk-merge.sbk"] {
            let path = fixture_dir.join(name);
            assert!(path.exists(), "missing local fixture: {}", path.display());
            let book = SbkBook::open(&path).expect("open fixture");
            assert!(!book.is_empty(), "{name} should index at least one state");
            let entry =
                book.lookup_sfen(InitialPosition::Standard.to_sfen()).expect("lookup hirate");
            assert!(entry.is_some(), "{name} should contain hirate");
        }
    }

    #[test]
    #[ignore = "requires RSSHOGI_LARGE_SBK_FIXTURE"]
    fn test_sbk_large_local_smoke() {
        let path = std::env::var_os("RSSHOGI_LARGE_SBK_FIXTURE")
            .map(PathBuf::from)
            .expect("set RSSHOGI_LARGE_SBK_FIXTURE to a large SBK file");
        assert!(path.exists(), "missing local fixture: {}", path.display());
        let book = SbkBook::open(&path).expect("open large sbk");
        assert!(!book.is_empty(), "large SBK should index at least one state");
        let entry = book.lookup_sfen(InitialPosition::Standard.to_sfen()).expect("lookup hirate");
        assert!(entry.is_some(), "large SBK should contain hirate");
    }

    fn temp_path(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("rsshogi-sbk-{name}-{}.sbk", std::process::id()));
        path
    }

    fn database_entry(
        sfen: &str,
        candidates: Vec<BookCandidate>,
        metadata: BookEntryMetadata,
    ) -> BookDatabaseEntry {
        BookDatabaseEntry::new(
            BookPosition::from_sfen(sfen, None).expect("position"),
            candidates,
            metadata,
        )
    }

    fn db_candidate(
        usi: &str,
        score: Option<i32>,
        weight: Option<i32>,
        sbk_evaluation: Option<i32>,
        sbk_weight: Option<i32>,
    ) -> BookCandidate {
        let metadata = if sbk_evaluation.is_some() || sbk_weight.is_some() {
            BookMoveMetadata::from_sbk(SbkMoveMetadata::new(0, sbk_evaluation, sbk_weight, None))
        } else {
            BookMoveMetadata::new()
        };
        BookCandidate::new(
            Move::from_usi(usi).expect("move"),
            None,
            score,
            None,
            weight,
            None,
            metadata,
        )
    }

    fn next_sfen(sfen: &str, usi: &str) -> String {
        let mut position = Position::from_sfen(sfen).expect("position");
        let mv = Move::from_usi(usi).expect("move");
        let mv32 = position.move32_from_move(mv);
        assert!(position.is_legal_move32(mv32), "{usi} should be legal from {sfen}");
        position.apply_move32(mv32);
        position.to_sfen(None)
    }

    fn normalized_sfen(sfen: &str) -> String {
        Position::from_sfen(sfen).expect("position").to_sfen(Some(1))
    }

    fn open_written_sbk(name: &str, bytes: &[u8]) -> (SbkBook, PathBuf) {
        let path = temp_path(name);
        fs::write(&path, bytes).expect("write sbk");
        let book = SbkBook::open(&path).expect("open written sbk");
        (book, path)
    }

    fn decode_exported_states(bytes: &[u8]) -> Vec<RawSbkState> {
        let mut input = WireInput::new(bytes);
        let mut states = Vec::new();
        while !input.is_empty() {
            let (field, wire_type) = input.read_tag().expect("tag");
            match (field, wire_type) {
                (FIELD_SBOOK_STATE, 2) => {
                    let payload = input.read_bytes().expect("state payload");
                    states.push(decode_state_payload(payload).expect("decode state"));
                }
                _ => input.skip_value(wire_type).expect("skip"),
            }
        }
        states
    }

    fn encode_sbook(states: Vec<Vec<u8>>) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(field_string(FIELD_SBOOK_AUTHOR, "author"));
        bytes.extend(field_string(FIELD_SBOOK_DESCRIPTION, "desc"));
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
