//! 内部 book solver（peta_shock 相当の eval-free 後退解析）。
//!
//! `makebook peta_shock` 相当の処理を移植したもの。
//! **評価と探索は一切行わない**。入力 [`BookDatabase`] に既に書かれている leaf 評価値を
//! 定跡グラフ上で min-max（後退解析）し、千日手と連続王手の千日手を処理して、整形済みの
//! [`BookDatabase`] を返す。
//!
//! 公開面は [`solve_peta_shock_book`] 関数と [`PetaShockOptions`] の 2 つだけ。`BookGraph` /
//! `ValueDepth` / sentinel などの作業表現はすべて `pub(crate)` に閉じる（task 0001 D3）。
//!
//! # パイプライン
//!
//! 1. [`BookGraph::build`]：`BookDatabase` → 手番正準化された node 集合 + 合流エッジ（0003）。
//! 2. [`BookGraph::remove_const_nodes`]：DAG 部分の後退解析（const node 凍結, 0004）。
//! 3. [`BookGraph::extract_check_loop`]：連続王手の千日手ループ抽出（0005）。
//! 4. [`BookGraph::init_cycle_nodes`]：残るサイクル node を千日手スコアで初期化（0005）。
//! 5. [`BookGraph::propagate_all_nodes`]：不動点まで評価値を親へ伝播（0005）。
//! 6. [`BookGraph::project`]：flip 戻し + 出力契約適用 → `BookDatabase`（0006）。
//!
//! # 手番正準化（重要, task 0003 Decision 2）
//!
//! peta_shock 互換: hashkey は「後手番化した局面」で計算し、node の moves は「先手番化した
//! 局面」での指し手で保持する。元の手番 `color` は出力再現用に保持し、[`BookGraph::project`]
//! で逆変換する（flip→solve→unflip で局面と指し手が一致する不変条件）。

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::board::Position;
use crate::board::movegen::{LegalAll, MoveListGen};
use crate::book::{
    BookCandidate, BookDatabase, BookDatabaseEntry, BookEntryMetadata, BookError, BookKey,
    BookMoveMetadata, BookPosition, book_key_from_position,
};
use crate::types::{Color, Move};

// === 定数（makebook2025.cpp L226-248, peta-shock-analysis §10）===

/// ∞ を表す評価値（参照 §10 の定数。`bestvd_for_parent` の seed には**使わない**。
/// `BOOK_VALUE_NONE` より大きく、未評価/空 node で `+INF` が漏れるため。詳細は当該関数 doc）。
#[allow(dead_code)]
const BOOK_VALUE_INF: i16 = i16::MAX;
/// 「指し手は在るが評価値は不明」を表す番兵（= `i16::MIN`）。
const BOOK_VALUE_NONE: i16 = i16::MIN;
/// 詰みスコアの上限相当。
const BOOK_VALUE_MATE: i16 = 32000;
/// 詰みスコア上限。
const BOOK_VALUE_MAX: i16 = BOOK_VALUE_MATE;
/// 詰みスコア下限。
const BOOK_VALUE_MIN: i16 = -BOOK_VALUE_MATE;

/// 千日手等の「∞ depth」を表す特徴的な depth。
const BOOK_DEPTH_MAX: u16 = 9999;
/// 連続王手の千日手で王手されている側を表す sentinel depth。
const BOOK_DEPTH_PERPETUAL_CHECKED: u16 = BOOK_DEPTH_MAX - 1; // 9998
/// 連続王手の千日手で王手している側を表す sentinel depth。
const BOOK_DEPTH_PERPETUAL_CHECK: u16 = BOOK_DEPTH_MAX - 2; // 9997

/// 後退解析の反復上限（定跡の最大長さ）。
const BOOK_MAX_PLY: u16 = 256;

// === ValueDepth（task 0004 Decision 1, makebook2025.cpp L251-288）===

/// 定跡の評価値とその depth を一まとめにした値。
///
/// 比較（[`Ord`]）は solve の best 選択と出力 sort（0006）の両方で使う。`Greater` =
/// 「より優れた手」。手実装の根拠は [`Self::cmp`] のコメント参照。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ValueDepth {
    pub(crate) value: i16,
    pub(crate) depth: u16,
}

impl ValueDepth {
    pub(crate) const fn new(value: i16, depth: u16) -> Self {
        Self { value, depth }
    }
}

impl Ord for ValueDepth {
    /// makebook2025.cpp L264-283 の `operator>` を全順序へ移植する。
    ///
    /// - まず value 降順（高い評価値が優れる）。
    /// - value 同値で `BOOK_DEPTH_PERPETUAL_CHECK` を含む場合は特別扱い（ループ回避）:
    ///   `PERPETUAL_CHECK` 側を常に劣る扱いにする。
    /// - それ以外は value の符号で depth の向きが変わる: 勝勢（value≥0）は短 depth 優先、
    ///   劣勢（value<0）は長 depth 優先（「負けている側は手数を伸ばしたい」）。
    fn cmp(&self, other: &Self) -> Ordering {
        if self.value != other.value {
            return self.value.cmp(&other.value);
        }
        // value 同値。PERPETUAL_CHECK の特別扱い。
        let self_pc = self.depth == BOOK_DEPTH_PERPETUAL_CHECK;
        let other_pc = other.depth == BOOK_DEPTH_PERPETUAL_CHECK;
        match (self_pc, other_pc) {
            (true, true) => return Ordering::Equal,
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            (false, false) => {}
        }
        // depth ベースの比較。
        if self.value >= 0 {
            // 勝勢: 短 depth が優れる → depth 昇順を反転。
            other.depth.cmp(&self.depth)
        } else {
            // 劣勢: 長 depth が優れる。
            self.depth.cmp(&other.depth)
        }
    }
}

impl PartialOrd for ValueDepth {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// 親に伝播する `ValueDepth` を作る（value 反転 + depth+1, `BOOK_DEPTH_MAX` 飽和）。
fn make_vd_for_parent(vd: ValueDepth) -> ValueDepth {
    ValueDepth::new(vd.value.wrapping_neg(), (vd.depth + 1).min(BOOK_DEPTH_MAX))
}

/// leaf 評価値（`BookCandidate::score`）を `ValueDepth` に落とす（task 0003 Decision 4）。
///
/// `None`（評価値不明）→ `BOOK_VALUE_NONE`、`Some(v)` → `[MIN, MAX]` に clamp。depth は 0。
fn leaf_value_depth(score: Option<i32>) -> ValueDepth {
    let value = match score {
        None => BOOK_VALUE_NONE,
        Some(v) => v.clamp(i32::from(BOOK_VALUE_MIN), i32::from(BOOK_VALUE_MAX)) as i16,
    };
    ValueDepth::new(value, 0)
}

// === グラフ構造（task 0003）===

/// グラフの 1 指し手。leaf は確定 `ValueDepth`、非 leaf は子 node への index を持つ。
///
/// `mv` は **先手番化（black-to-move）局面での指し手**。
#[derive(Debug, Clone, Copy)]
pub(crate) enum GraphMove {
    /// 子局面を持たない手（DB の評価値をそのまま使う）。
    Leaf { mv: Move, vd: ValueDepth },
    /// 既知局面へ合流する手（子 node の index）。
    Child { mv: Move, next: u32 },
}

impl GraphMove {
    fn mv(&self) -> Move {
        match self {
            Self::Leaf { mv, .. } | Self::Child { mv, .. } => *mv,
        }
    }
}

/// グラフの 1 局面。
#[derive(Debug, Clone)]
pub(crate) struct GraphNode {
    /// この局面での指し手（先手番化した局面の指し手）。
    moves: Vec<GraphMove>,
    /// 親に伝播するための `ValueDepth`（value = -best, depth = best.depth+1）。
    vd: ValueDepth,
    /// 元の（棋譜に出現した）手番。出力時にこれを再現する。
    color: Color,
    /// 出力用に保持する元の normalized SFEN（手番込み）。
    sfen: String,
    /// 出力用に保持する元の ply 情報。
    original_ply: Option<u32>,
    /// 子がすべて leaf もしくは const node か（凍結フラグ）。
    const_node: bool,
    /// この局面で（手番側が）王手されているか。
    checked: bool,
    /// 連続王手の千日手ループ上の局面か。
    check_loop: bool,
}

/// solver の作業表現。`BookDatabase` から構築し、solve 後 `BookDatabase` へ射出する。
pub(crate) struct BookGraph {
    nodes: Vec<GraphNode>,
    stats: BookGraphStats,
}

/// solve の統計値。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct BookGraphStats {
    /// 合流チェックで（DB に無い手から）既知局面へ合流した指し手の数。
    pub(crate) converged_moves: u64,
    /// const node 化した（ループを含まなかった）node の数。
    pub(crate) const_nodes: u64,
    /// 王手されていた局面の数。
    pub(crate) in_check: u64,
    /// check loop 上の（王手されていた）局面の数。
    pub(crate) check_loop_nodes: u64,
}

impl BookGraph {
    /// `BookDatabase` から `BookGraph` を構築する（手番正準化 + エッジ構築）。
    ///
    /// node identity は 128bit Zobrist hash に全面依存するため、`hash-128` feature が
    /// 無効な場合は明確にエラーを返す（task 0003 Decision 1, 衝突非許容）。
    pub(crate) fn build(database: &BookDatabase) -> Result<Self, BookError> {
        if !cfg!(feature = "hash-128") {
            return Err(BookError::Unsupported(
                "book solver requires the `hash-128` feature (128bit position hash); \
                 64bit hashing would merge distinct positions",
            ));
        }

        let mut nodes: Vec<GraphNode> = Vec::with_capacity(database.len());
        let mut key_to_index: HashMap<BookKey, u32> = HashMap::with_capacity(database.len());
        let mut stats = BookGraphStats::default();

        // --- Pass 1: node 作成・手番正準化・hashkey 登録・leaf 手の取り込み（read_book 相当）。
        for entry in database.entries() {
            let sfen = entry.position().sfen();
            let (black_pos, white_pos, color) = orientations(sfen)?;

            // hashkey は後手番化（white-to-move）局面で計算して登録する。
            let key = book_key_from_position(&white_pos);
            // 王手判定は flip 不変なので先手番化局面で取る。
            let checked = !black_pos.checkers().is_empty();
            if checked {
                stats.in_check += 1;
            }

            let index = u32::try_from(nodes.len())
                .map_err(|_| BookError::Unsupported("book solver node count exceeds u32"))?;
            // 元の定跡に flip 重複があれば、あとから出現した局面を優先する（参照 L599-603）。
            key_to_index.insert(key, index);

            // DB の candidate を先手番化局面の leaf 手として取り込む。
            let mut moves = Vec::with_capacity(entry.candidates().len());
            for candidate in entry.candidates() {
                let mut mv = candidate.mv();
                if mv == Move::MOVE_NONE {
                    continue;
                }
                if color == Color::WHITE {
                    mv = flip_move(mv)?;
                }
                moves.push(GraphMove::Leaf { mv, vd: leaf_value_depth(candidate.score()) });
            }

            nodes.push(GraphNode {
                moves,
                vd: ValueDepth::new(BOOK_VALUE_NONE, 0),
                color,
                sfen: sfen.to_string(),
                original_ply: entry.position().original_ply(),
                const_node: false,
                checked,
                check_loop: checked,
            });
        }

        // --- Pass 2: 合流チェック（convergence_check 相当）。全合法手展開 + hash 照合。
        // 各 node は自分の sfen と moves しか触らない（子 node には触れない）ので iter_mut で良い。
        for node in &mut nodes {
            let (black_pos, _white, _color) = orientations(&node.sfen)?;

            // 元 DB の leaf 手を退避し、いったん空にする（参照 L688-689）。
            let book_moves = std::mem::take(&mut node.moves);

            let mut moves: Vec<GraphMove> = Vec::new();
            for mv in MoveListGen::<LegalAll>::new(&black_pos).iter() {
                let next_key = black_pos.key_after_move(*mv);
                if let Some(&child) = key_to_index.get(&next_key) {
                    // DB に無かった手で既知局面へ合流したらカウント。
                    if !book_moves.iter().any(|bm| bm.mv() == *mv) {
                        stats.converged_moves += 1;
                    }
                    moves.push(GraphMove::Child { mv: *mv, next: child });
                }
            }

            // どこにも合流しなければ leaf のみの node（const 候補）。
            node.const_node = moves.is_empty();

            // 合流先が未エッジの元 DB leaf 手も登録する（合流済みは子の値を使うので除外）。
            for book_move in &book_moves {
                let mv = book_move.mv();
                if !moves.iter().any(|m| m.mv() == mv) {
                    moves.push(*book_move);
                }
            }

            node.moves = moves;
        }

        Ok(Self { nodes, stats })
    }

    /// 統計値を返す（solve の検証用インストルメンテーション。テストでのみ参照される）。
    #[allow(dead_code)]
    pub(crate) const fn stats(&self) -> BookGraphStats {
        self.stats
    }

    // === 後退解析 I: const node 凍結（task 0004, makebook2025.cpp L770-851）===

    /// 出次数 0（DAG 部分）の node を後退解析で確定・凍結する。
    pub(crate) fn remove_const_nodes(&mut self) {
        for node in &mut self.nodes {
            node.const_node = false;
        }
        for _ in 0..BOOK_MAX_PLY {
            let count = self.remove_const_nodes_once();
            self.stats.const_nodes += count;
            if count == 0 {
                break;
            }
        }
    }

    fn remove_const_nodes_once(&mut self) -> u64 {
        let mut count = 0;
        for i in 0..self.nodes.len() {
            if self.nodes[i].const_node {
                continue;
            }
            // すべての手が leaf もしくは const 子か？
            let all_const = self.nodes[i].moves.iter().all(|m| match m {
                GraphMove::Leaf { .. } => true,
                GraphMove::Child { next, .. } => self.nodes[*next as usize].const_node,
            });
            if !all_const {
                continue;
            }
            let vd = self.bestvd_for_parent(i);
            self.nodes[i].vd = vd;
            self.nodes[i].const_node = true;
            count += 1;
        }
        count
    }

    /// node の全手の `ValueDepth` の max を取り、親伝播用に反転 + depth+1 する。
    ///
    /// ⚠ seed には実手より優れた値（`-BOOK_VALUE_INF` 等）を使わない。`BOOK_VALUE_NONE`
    /// （= `i16::MIN`）は `-BOOK_VALUE_INF`（= -32767）より小さいため、seed を後者にすると
    /// 全手が `None`（評価値不明）の node や空 node で seed が選ばれ続け、その反転
    /// `+BOOK_VALUE_INF`（= +32767, 事実上の勝勢）が親へ漏れて未評価/行き止まりの手を
    /// 「ほぼ勝ち」に見せてしまう。実手から畳み込み、手が無ければ `NONE`（最弱）を伝える。
    fn bestvd_for_parent(&self, node_index: usize) -> ValueDepth {
        let mut best: Option<ValueDepth> = None;
        for m in &self.nodes[node_index].moves {
            let vd = match m {
                GraphMove::Leaf { vd, .. } => *vd,
                GraphMove::Child { next, .. } => self.nodes[*next as usize].vd,
            };
            if best.is_none_or(|b| vd > b) {
                best = Some(vd);
            }
        }
        // 継続手が無い（空 node = 行き止まり）なら評価不能。最弱の NONE として親へ伝える。
        let best = best.unwrap_or(ValueDepth::new(BOOK_VALUE_NONE, BOOK_DEPTH_MAX));
        make_vd_for_parent(best)
    }

    // === 後退解析 II: 連続王手の千日手ループ抽出（task 0005, L854-1005）===

    /// 連続王手の千日手ループ（check loop）を抽出する。
    pub(crate) fn extract_check_loop(&mut self) {
        // 「2 手先に check_loop が無ければ自分も check_loop でない」を収束させる。
        for _ in 0..BOOK_MAX_PLY {
            let mut updated = 0u64;
            for i in 0..self.nodes.len() {
                if !self.nodes[i].check_loop {
                    continue;
                }
                if !self.two_ply_has_check_loop(i) {
                    self.nodes[i].check_loop = false;
                    updated += 1;
                }
            }
            if updated == 0 {
                break;
            }
        }

        // 残った check loop 局面（王手されている側）の数。
        self.stats.check_loop_nodes =
            self.nodes.iter().filter(|node| node.check_loop).count() as u64;

        // 数珠つなぎ: check loop 局面の 2 手先が check loop なら、間の局面も check loop。
        let seeds: Vec<usize> =
            (0..self.nodes.len()).filter(|&i| self.nodes[i].check_loop).collect();
        for i in seeds {
            let next_indices: Vec<u32> = self.nodes[i]
                .moves
                .iter()
                .filter_map(|m| match m {
                    GraphMove::Child { next, .. } => Some(*next),
                    GraphMove::Leaf { .. } => None,
                })
                .collect();
            for next in next_indices {
                let has_check_loop_grandchild =
                    self.nodes[next as usize].moves.iter().any(|m| match m {
                        GraphMove::Child { next: nn, .. } => self.nodes[*nn as usize].check_loop,
                        GraphMove::Leaf { .. } => false,
                    });
                if has_check_loop_grandchild {
                    self.nodes[next as usize].check_loop = true;
                }
            }
        }
    }

    /// node の 2 手先に check_loop 局面があるか。
    fn two_ply_has_check_loop(&self, node_index: usize) -> bool {
        for m in &self.nodes[node_index].moves {
            let GraphMove::Child { next, .. } = m else {
                continue;
            };
            for m2 in &self.nodes[*next as usize].moves {
                let GraphMove::Child { next: nn, .. } = m2 else {
                    continue;
                };
                if self.nodes[*nn as usize].check_loop {
                    return true;
                }
            }
        }
        false
    }

    // === 千日手スコアでサイクル node を初期化（task 0005, L1008-1031）===

    /// 残るサイクル node を千日手スコアで初期化する。
    pub(crate) fn init_cycle_nodes(&mut self) {
        for node in &mut self.nodes {
            if node.const_node {
                continue;
            }
            node.vd = if node.check_loop {
                // 連続王手の千日手: 王手されている側は -MATE、している側は +MATE。
                let value = if node.checked { BOOK_VALUE_MIN } else { BOOK_VALUE_MAX };
                ValueDepth::new(value, BOOK_DEPTH_MAX)
            } else {
                // 通常の千日手は引き分け 0。
                ValueDepth::new(0, BOOK_DEPTH_MAX)
            };
        }
    }

    // === 後退解析 III: 伝播（task 0005, L1060-1173）===

    /// 評価値を親へ伝播する。`BOOK_MAX_PLY + 100` 回、不動点で早期終了。
    pub(crate) fn propagate_all_nodes(&mut self) -> Result<(), BookError> {
        let check_loop_nodes: Vec<u32> =
            (0..self.nodes.len()).filter(|&i| self.nodes[i].check_loop).map(|i| i as u32).collect();

        for _ in 0..(BOOK_MAX_PLY + 100) {
            let updating = self.propagate_all_nodes_once();
            self.dfs_for_check_loop_nodes(&check_loop_nodes)?;
            if updating == 0 {
                break;
            }
        }
        Ok(())
    }

    /// 通常 node の vd を 1 回伝播する（const / check_loop は除外）。更新数を返す。
    fn propagate_all_nodes_once(&mut self) -> u64 {
        let mut count = 0;
        for i in 0..self.nodes.len() {
            if self.nodes[i].const_node || self.nodes[i].check_loop {
                continue;
            }
            let mut best = self.bestvd_for_parent(i);
            // 非循環が絡んで BOOK_DEPTH_MAX になっていないだけで実際は循環とみなす。
            if best.depth > BOOK_MAX_PLY {
                best.depth = BOOK_DEPTH_MAX;
            }
            if self.nodes[i].vd != best {
                count += 1;
            }
            self.nodes[i].vd = best;
        }
        count
    }

    /// check loop 上の node だけ trajectory 付き DFS で更新する。
    fn dfs_for_check_loop_nodes(&mut self, check_loop_nodes: &[u32]) -> Result<(), BookError> {
        let mut trajectory: Vec<u32> = Vec::new();
        for &node_index in check_loop_nodes {
            let vd = self.dfs_for_check_loop_node(node_index, &mut trajectory, 0)?;
            self.nodes[node_index as usize].vd = vd;
        }
        Ok(())
    }

    /// check loop 上の node から DFS して親 node 用の `ValueDepth` を返す。
    ///
    /// `trajectory`（現在の訪問経路）で連続王手の千日手成立を検出する。再帰深さは
    /// check loop node 数で上限が付くが、念のため明示ガードを設ける（task 0005 Decision 2）。
    fn dfs_for_check_loop_node(
        &self,
        node_index: u32,
        trajectory: &mut Vec<u32>,
        depth: u32,
    ) -> Result<ValueDepth, BookError> {
        let node = &self.nodes[node_index as usize];

        // check loop 上ではないのでこれ以上探索しない。
        if !node.check_loop {
            return Ok(node.vd);
        }

        // 連続王手の千日手が成立。
        if trajectory.contains(&node_index) {
            return Ok(if node.checked {
                ValueDepth::new(-BOOK_VALUE_MAX, BOOK_DEPTH_PERPETUAL_CHECK)
            } else {
                ValueDepth::new(BOOK_VALUE_MAX, BOOK_DEPTH_PERPETUAL_CHECKED)
            });
        }

        // 深さガード（trajectory が node 総数を超えることは正常系では無い）。
        if depth as usize > self.nodes.len() + 16 {
            return Err(BookError::InvalidData(
                "book solver check-loop DFS exceeded depth guard".into(),
            ));
        }

        trajectory.push(node_index);

        // `bestvd_for_parent` と同じく seed には実手より優れた値を使わない（全 None / 空で
        // `+BOOK_VALUE_INF` が漏れるのを防ぐ）。実手から畳み込み、無ければ NONE（最弱）。
        let mut best: Option<ValueDepth> = None;
        // node.moves をインデックスで走査して借用を避ける。
        for move_index in 0..node.moves.len() {
            let vd = match self.nodes[node_index as usize].moves[move_index] {
                GraphMove::Leaf { vd, .. } => vd,
                GraphMove::Child { next, .. } => {
                    self.dfs_for_check_loop_node(next, trajectory, depth + 1)?
                }
            };
            if best.is_none_or(|b| vd > b) {
                best = Some(vd);
            }
        }

        // push したものを取り除く（参照の trajectory.erase 相当）。
        if let Some(pos) = trajectory.iter().rposition(|&x| x == node_index) {
            trajectory.remove(pos);
        }

        let best = best.unwrap_or(ValueDepth::new(BOOK_VALUE_NONE, BOOK_DEPTH_MAX));
        Ok(make_vd_for_parent(best))
    }

    // === 出力射出（task 0006, write_peta_shock_book 相当, L1177-1280）===

    /// solve 済みグラフを `BookDatabase` へ射出する。
    ///
    /// 各 node の手を {leaf vd / 子 node vd} に展開し評価値降順 sort、連続王手 depth 調整、
    /// 同 value・depth 違いの value -1 調整、shrink を適用し、元手番へ flip 戻しする。
    pub(crate) fn project(&self, shrink: bool) -> Result<BookDatabase, BookError> {
        let mut entries: Vec<BookDatabaseEntry> = Vec::with_capacity(self.nodes.len());

        for node in &self.nodes {
            // 手を (move, vd) に展開。
            let mut moves: Vec<(Move, ValueDepth)> = Vec::with_capacity(node.moves.len());
            for m in &node.moves {
                match m {
                    GraphMove::Leaf { mv, vd } => moves.push((*mv, *vd)),
                    GraphMove::Child { mv, next } => {
                        let child = &self.nodes[*next as usize];
                        let mut vd = child.vd;
                        // この手を選ぶと連続王手の千日手（王手される側）に入る。同値なら
                        // 選ばれないよう depth を PERPETUAL_CHECK にする（参照 L1233-1234）。
                        if child.check_loop && child.checked {
                            vd.depth = BOOK_DEPTH_PERPETUAL_CHECK;
                        }
                        moves.push((*mv, vd));
                    }
                }
            }

            // 評価値降順 sort（Ord: Greater = 優れる）。`Reverse` で降順・安定 sort。
            moves.sort_by_key(|(_, vd)| core::cmp::Reverse(*vd));

            let best_value = moves.first().map(|(_, vd)| vd.value);
            let best_depth = moves.first().map(|(_, vd)| vd.depth);

            let mut candidates: Vec<BookCandidate> = Vec::with_capacity(moves.len());
            for (i, (mv, vd)) in moves.iter().enumerate() {
                // shrink: 最善 value と異なる手は出力しない（参照 L1250-1251）。
                if shrink && best_value != Some(vd.value) {
                    continue;
                }

                let mut out_vd = *vd;
                // 同 value で depth が異なるなら value を -1 して区別（参照 L1265-1266）。
                if i > 0 && best_value == Some(vd.value) && best_depth != Some(vd.depth) {
                    out_vd.value = out_vd.value.saturating_sub(1);
                }

                // 元手番へ flip 戻し（0003 の正準化と対称）。
                let out_mv = if node.color == Color::WHITE { flip_move(*mv)? } else { *mv };

                candidates.push(BookCandidate::new(
                    out_mv,
                    None,
                    Some(i32::from(out_vd.value)),
                    Some(u32::from(out_vd.depth)),
                    None,
                    None,
                    BookMoveMetadata::new(),
                ));
            }

            let position = BookPosition::from_sfen(&node.sfen, node.original_ply)?;
            entries.push(BookDatabaseEntry::new(position, candidates, BookEntryMetadata::new()));
        }

        // 入力 DB が normalized SFEN 一意なので node.sfen も一意。検証コストを避けて構築する。
        Ok(BookDatabase::from_entries_unchecked(entries))
    }
}

/// SFEN を先手番化（black-to-move）と後手番化（white-to-move）の 2 局面 + 元手番に分解する。
fn orientations(sfen: &str) -> Result<(Position, Position, Color), BookError> {
    let original =
        Position::from_sfen(sfen).map_err(|err| BookError::InvalidData(err.to_string()))?;
    let color = original.turn();
    let flipped = Position::from_sfen(&original.to_sfen_flipped(None))
        .map_err(|err| BookError::InvalidData(err.to_string()))?;
    let (black_pos, white_pos) = match color {
        Color::BLACK => (original, flipped),
        Color::WHITE => (flipped, original),
    };
    Ok((black_pos, white_pos, color))
}

/// 指し手を先後反転する（盤面 180 度回転に対応する升の反転）。
///
/// 特殊手（`MOVE_NONE` 等）はそのまま返す。0003 の正準化と 0006 の出力で対称に使う。
fn flip_move(mv: Move) -> Result<Move, BookError> {
    if !mv.is_normal() {
        return Ok(mv);
    }
    let to = mv.to_sq().flip();
    if mv.is_drop() {
        let piece_type = mv.dropped_piece().ok_or_else(|| {
            BookError::InvalidData(format!("invalid book solver drop move: 0x{:04x}", mv.raw()))
        })?;
        Ok(Move::drop(piece_type, to))
    } else {
        let from = mv.from_sq().flip();
        Ok(if mv.is_promotion() { Move::promotion(from, to) } else { Move::normal(from, to) })
    }
}

// === 公開入口（task 0006 Decision 1）===

/// [`solve_peta_shock_book`] の挙動を制御するオプション。
///
/// peta_shock の `fast` / Threads 並列は当面非対応（将来タスク）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PetaShockOptions {
    shrink: bool,
}

impl Default for PetaShockOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl PetaShockOptions {
    /// 既定オプション（`shrink` 無効）。
    #[must_use]
    pub const fn new() -> Self {
        Self { shrink: false }
    }

    /// 各局面で最善 value の手だけを残す（shrink）かどうかを設定する。
    #[must_use]
    pub const fn with_shrink(mut self, shrink: bool) -> Self {
        self.shrink = shrink;
        self
    }

    /// `shrink` が有効かどうかを返す。
    #[must_use]
    pub const fn shrink(self) -> bool {
        self.shrink
    }
}

/// 既存スコア付き定跡 [`BookDatabase`] を peta_shock 相当の後退解析で整形する。
///
/// 入力に既に書かれている leaf 評価値を定跡グラフ上で min-max 伝播し、千日手と連続王手の
/// 千日手を処理して、評価値降順に整形した [`BookDatabase`] を返す。**評価関数と探索は使わない**
/// （eval-free。`think` は engine 層であり scope 外, task 0001 D2）。出力の DB2016 書き出しは
/// [`BookDatabase::write_yaneuraou_db2016`] に委ねる。
///
/// 入力は **FlippedBook 前提**（片側手番のみ）を想定する。手番正準化により、ある局面と
/// その先後反転は同一視されて合流する。
///
/// # Errors
///
/// `hash-128` feature が無効な場合、または局面/指し手の整合性が壊れている場合にエラーを返す。
pub fn solve_peta_shock_book(
    database: &BookDatabase,
    options: &PetaShockOptions,
) -> Result<BookDatabase, BookError> {
    let mut graph = BookGraph::build(database)?;
    graph.remove_const_nodes();
    graph.extract_check_loop();
    graph.init_cycle_nodes();
    graph.propagate_all_nodes()?;
    graph.project(options.shrink())
}

#[cfg(all(test, feature = "hash-128"))]
mod tests {
    use super::*;
    use crate::board::{self, InitialPosition};
    use crate::book::{YaneuraOuBook, YaneuraOuBookDiagnostics, YaneuraOuDb2016WriteOptions};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// SFEN を normalized 形（ply=1）で返す。
    fn norm(sfen: &str) -> String {
        Position::from_sfen(sfen).unwrap().to_sfen(None)
    }

    /// `parent` 局面に USI 手を適用した子局面の normalized SFEN を返す。
    fn child(parent_sfen: &str, usi: &str) -> String {
        let mut pos = Position::from_sfen(parent_sfen).unwrap();
        pos.apply_move(Move::from_usi(usi).unwrap());
        pos.to_sfen(None)
    }

    fn cand(usi: &str, score: Option<i32>) -> BookCandidate {
        BookCandidate::new(
            Move::from_usi(usi).unwrap(),
            None,
            score,
            None,
            None,
            None,
            BookMoveMetadata::new(),
        )
    }

    /// テスト用の book 仕様: `(sfen, &[(move_usi, score)])`。
    type EntrySpec<'a> = (&'a str, &'a [(&'a str, Option<i32>)]);

    fn make_db(entries: &[EntrySpec]) -> BookDatabase {
        let es = entries
            .iter()
            .map(|(sfen, moves)| {
                let position = BookPosition::from_sfen(sfen, Some(1)).unwrap();
                let candidates = moves.iter().map(|(usi, score)| cand(usi, *score)).collect();
                BookDatabaseEntry::new(position, candidates, BookEntryMetadata::new())
            })
            .collect();
        BookDatabase::try_from_entries(es).unwrap()
    }

    /// solve 結果から指定 SFEN の候補手 (usi, score, depth) を取り出す。
    fn candidates_of(db: &BookDatabase, sfen: &str) -> Vec<(String, Option<i32>, Option<u32>)> {
        let entry = db.entry_by_sfen(sfen).unwrap().expect("entry");
        entry.candidates().iter().map(|c| (c.mv().to_usi(), c.score(), c.depth())).collect()
    }

    fn temp_path() -> PathBuf {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let seq = TEMP_COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
        std::env::temp_dir()
            .join(format!("rsshogi-book-solver-{}-{stamp}-{seq}.db", std::process::id()))
    }

    // === 0003: build / convergence / const / checked ===

    #[test]
    fn test_build_discovers_convergence_edge_for_unrecorded_move() {
        board::init();
        let r = InitialPosition::Standard.to_sfen().to_string();
        let a = child(&r, "7g7f");
        // R は手を記録しない（空）。A は leaf 手 3c3d を持つ。
        let db = make_db(&[(&r, &[]), (&a, &[("3c3d", Some(100))])]);

        let graph = BookGraph::build(&db).unwrap();
        let stats = graph.stats();
        // movegen が R→A(7g7f) を発見し、記録されていないので converged。
        assert_eq!(stats.converged_moves, 1);
        assert_eq!(stats.in_check, 0);
    }

    #[test]
    fn test_build_marks_checked_node() {
        board::init();
        // 後手が王手している局面（先手番で王手されている）を作る。
        // 先手玉 5i に後手飛車 5e が利く形。
        let checked_sfen = norm("4k4/9/9/9/4r4/9/9/9/4K4 b - 1");
        let db = make_db(&[(&checked_sfen, &[("5i6i", Some(0))])]);
        let graph = BookGraph::build(&db).unwrap();
        assert_eq!(graph.stats().in_check, 1);
    }

    // === 0004: acyclic min-max ===

    #[test]
    fn test_acyclic_minimax_propagates_negamax_values() {
        board::init();
        let r = InitialPosition::Standard.to_sfen().to_string();
        let a = child(&r, "7g7f");
        // R --7g7f--> A(leaf 3c3d, score +100)。
        let db = make_db(&[(&r, &[("7g7f", None)]), (&a, &[("3c3d", Some(100))])]);

        let solved = solve_peta_shock_book(&db, &PetaShockOptions::new()).unwrap();

        // A: leaf 3c3d (100, depth 0) がそのまま出る（白番→flip 戻しで 3c3d 復元）。
        let a_norm = norm(&a);
        assert_eq!(candidates_of(&solved, &a_norm), vec![("3c3d".to_string(), Some(100), Some(0))]);

        // R: 7g7f は子 A.vd = make_vd_for_parent(100,0) = (-100, 1)。
        let r_norm = norm(&r);
        assert_eq!(
            candidates_of(&solved, &r_norm),
            vec![("7g7f".to_string(), Some(-100), Some(1))]
        );
    }

    #[test]
    fn test_minimax_chooses_best_child_with_depth_tiebreak() {
        board::init();
        let r = InitialPosition::Standard.to_sfen().to_string();
        let a = child(&r, "7g7f"); // 白番
        let b = child(&r, "2g2f"); // 白番
        // R から 2 手 (7g7f→A, 2g2f→B)。A は leaf +50、B は leaf +200。
        // R にとっては子の値が反転するので、-(-50)=... 実際は A.vd=(-50,1), B.vd=(-200,1)。
        // R の best は max(A.vd, B.vd) = (-50,1)（value 大きい方）→ 7g7f が選ばれる。
        let db = make_db(&[
            (&r, &[("7g7f", None), ("2g2f", None)]),
            (&a, &[("3c3d", Some(50))]),
            (&b, &[("3c3d", Some(200))]),
        ]);
        let solved = solve_peta_shock_book(&db, &PetaShockOptions::new()).unwrap();
        let r_norm = norm(&r);
        let cands = candidates_of(&solved, &r_norm);
        // 評価値降順: 7g7f(-50,1) が先頭、2g2f(-200,1) が後。
        assert_eq!(cands[0].0, "7g7f");
        assert_eq!(cands[0].1, Some(-50));
        assert_eq!(cands[1].0, "2g2f");
        assert_eq!(cands[1].1, Some(-200));
    }

    // === 0005: cycle / draw ===

    #[test]
    fn test_simple_repetition_converges_to_draw_zero() {
        board::init();
        // 4 手のループ（玉の往復）で千日手。S0→S1→S2→S3→S0。
        let s0 = norm("4k4/9/9/9/9/9/9/9/4K4 b - 1");
        let s1 = child(&s0, "5i4i");
        let s2 = child(&s1, "5a4a");
        let s3 = child(&s2, "4i5i");
        let back = child(&s3, "4a5a");
        // 盤面・手番・持駒は一致する（ply のみ異なる。グラフ identity は ply を含まない）。
        let strip_ply = |s: &str| s.rsplit_once(' ').unwrap().0.to_string();
        assert_eq!(strip_ply(&back), strip_ply(&s0), "4 手で元局面へ戻る repetition のはず");

        let db = make_db(&[
            (&s0, &[("5i4i", None)]),
            (&s1, &[("5a4a", None)]),
            (&s2, &[("4i5i", None)]),
            (&s3, &[("4a5a", None)]),
        ]);
        let solved = solve_peta_shock_book(&db, &PetaShockOptions::new()).unwrap();

        // すべての cycle node の手が引き分け 0 に収束する。
        for s in [&s0, &s1, &s2, &s3] {
            let cands = candidates_of(&solved, &norm(s));
            assert!(!cands.is_empty(), "cycle node に手が無い: {s}");
            for (_, score, _) in &cands {
                assert_eq!(*score, Some(0), "千日手なら value 0 のはず: {s}");
            }
        }
    }

    #[test]
    fn test_perpetual_check_marks_sentinel_depth_and_terminates() {
        board::init();
        // 連続王手の千日手ループ。後手玉(=ここでは "k" 側)を黒飛車が追い回す 4 手ループ。
        // P0(白番・王手されている) -5a4a-> P1(黒番) -5e4e-> P2(白番・王手) -4a5a-> P3(黒番)
        //   -4e5e-> P0。
        let p0 = norm("4k4/9/9/9/4R4/9/9/9/4K4 w - 1");
        let p1 = child(&p0, "5a4a");
        let p2 = child(&p1, "5e4e");
        let p3 = child(&p2, "4a5a");
        let back = child(&p3, "4e5e");
        let strip_ply = |s: &str| s.rsplit_once(' ').unwrap().0.to_string();
        assert_eq!(strip_ply(&back), strip_ply(&p0), "連続王手 4 手で元局面へ戻るはず");

        let db = make_db(&[
            (&p0, &[("5a4a", None)]),
            (&p1, &[("5e4e", None)]),
            (&p2, &[("4a5a", None)]),
            (&p3, &[("4e5e", None)]),
        ]);

        // 王手されている局面（P0, P2）が検出される。
        let graph = BookGraph::build(&db).unwrap();
        assert_eq!(graph.stats().in_check, 2);

        // solve は無限ループせず終了する（連続王手の DFS が成立を検出する）。
        let solved = solve_peta_shock_book(&db, &PetaShockOptions::new()).unwrap();

        // 王手している側の手（P1 の 5e4e → 王手される子 P2 へ入る）は depth が
        // BOOK_DEPTH_PERPETUAL_CHECK(9997) に調整され、選ばれにくくなる。
        let p1_cands = candidates_of(&solved, &norm(&p1));
        assert!(
            p1_cands.iter().any(|(_, _, depth)| *depth == Some(9997)),
            "連続王手の千日手へ入る手は depth 9997 で印付けされるはず: {p1_cands:?}"
        );
    }

    #[test]
    fn test_unevaluated_child_does_not_propagate_phantom_win() {
        board::init();
        // R --7g7f--> A(leaf 3c3d, score None) / R --2g2f--> B(leaf 3c3d, score +50)。
        // A は全 None。バグ版だと A.vd=+INF になり 7g7f が「ほぼ勝ち」として 1 手目に来る。
        // 正しくは A.vd=NONE(最弱)で、評価済みの 2g2f が優先される。
        let r = InitialPosition::Standard.to_sfen().to_string();
        let a = child(&r, "7g7f");
        let b = child(&r, "2g2f");
        let db = make_db(&[
            (&r, &[("7g7f", None), ("2g2f", None)]),
            (&a, &[("3c3d", None)]),
            (&b, &[("3c3d", Some(50))]),
        ]);

        let solved = solve_peta_shock_book(&db, &PetaShockOptions::new()).unwrap();
        let cands = candidates_of(&solved, &norm(&r));

        // 評価済みの 2g2f(-50) が先頭、未評価経由の 7g7f は最弱(i16::MIN)で末尾。
        assert_eq!(cands[0].0, "2g2f");
        assert_eq!(cands[0].1, Some(-50));
        assert_eq!(cands[1].0, "7g7f");
        assert_eq!(cands[1].1, Some(i32::from(i16::MIN)));
        // 旧バグの兆候（+32767 = BOOK_VALUE_INF の漏れ）が無いこと。
        assert!(cands.iter().all(|(_, score, _)| *score != Some(i32::from(i16::MAX))));
    }

    #[test]
    fn test_empty_child_node_does_not_propagate_phantom_win() {
        board::init();
        // R --7g7f--> A（候補 0 の空 node = 行き止まり）。
        // バグ版だと空 node が +INF を伝播し 7g7f が勝勢に見える。正しくは NONE(最弱)。
        let r = InitialPosition::Standard.to_sfen().to_string();
        let a = child(&r, "7g7f");
        let db = make_db(&[(&r, &[("7g7f", None)]), (&a, &[])]);

        let solved = solve_peta_shock_book(&db, &PetaShockOptions::new()).unwrap();
        let cands = candidates_of(&solved, &norm(&r));
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].0, "7g7f");
        // 行き止まりは最弱 NONE(i16::MIN)。+32767 の漏れではない。
        assert_eq!(cands[0].1, Some(i32::from(i16::MIN)));
    }

    // === 0006: end-to-end / flip / shrink ===

    #[test]
    fn test_end_to_end_writer_peta_shock_profile() {
        board::init();
        let r = InitialPosition::Standard.to_sfen().to_string();
        let a = child(&r, "7g7f");
        let db = make_db(&[(&r, &[("7g7f", None)]), (&a, &[("3c3d", Some(100))])]);

        let solved = solve_peta_shock_book(&db, &PetaShockOptions::new()).unwrap();
        let text =
            solved.to_yaneuraou_db2016_string(&YaneuraOuDb2016WriteOptions::peta_shock()).unwrap();

        // peta_shock profile は `move none value depth` の 4 列。
        for line in text.lines().filter(|l| !l.starts_with('#') && !l.starts_with("sfen")) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 4, "4 列のはず: {line}");
            assert_eq!(fields[1], "none", "ponder は常に none: {line}");
        }

        // 出力を読み戻すと Sorted { complete: true }。
        let path = temp_path();
        fs::write(&path, &text).unwrap();
        let book = YaneuraOuBook::open(&path).unwrap();
        assert!(matches!(
            book.diagnostics(),
            YaneuraOuBookDiagnostics::Sorted { complete: true, .. }
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn test_shrink_keeps_only_best_value_moves() {
        board::init();
        let r = InitialPosition::Standard.to_sfen().to_string();
        let a = child(&r, "7g7f");
        let b = child(&r, "2g2f");
        let db = make_db(&[
            (&r, &[("7g7f", None), ("2g2f", None)]),
            (&a, &[("3c3d", Some(50))]),
            (&b, &[("3c3d", Some(200))]),
        ]);

        // shrink なし: R に 2 手。
        let full = solve_peta_shock_book(&db, &PetaShockOptions::new()).unwrap();
        assert_eq!(candidates_of(&full, &norm(&r)).len(), 2);

        // shrink あり: R は best value の手 (7g7f, -50) のみ。
        let shrunk =
            solve_peta_shock_book(&db, &PetaShockOptions::new().with_shrink(true)).unwrap();
        let cands = candidates_of(&shrunk, &norm(&r));
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].0, "7g7f");
    }
}
