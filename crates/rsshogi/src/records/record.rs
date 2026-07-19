//! 棋譜のデータ構造。
//!
//! [`Record`] は KIF、CSA、JKF などの対局棋譜を表す。sazpack や HCPE などの
//! 学習データ用レコードとは区別する。

use crate::board::{InitialPosition as BoardInitialPosition, Position};
use crate::records::error::RecordError;
use crate::records::time_control::TimeControl;
use crate::types::{Eval, GameResult, Move};
use ordered_float::OrderedFloat;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::num::NonZeroU32;
use std::time::Duration;

/// レコード単位で保持する初期局面。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordInitialPosition {
    /// USI の `startpos`。展開した SFEN ではなく `startpos` として出力する。
    Startpos,
    /// 明示的な SFEN 初期局面。
    Sfen(String),
}

impl RecordInitialPosition {
    /// [`Position`] の初期化に使う SFEN を返す。
    #[must_use]
    pub fn to_sfen(&self) -> &str {
        match self {
            Self::Startpos => BoardInitialPosition::Standard.to_sfen(),
            Self::Sfen(sfen) => sfen,
        }
    }

    /// USI の `startpos` を保持しているとき `true` を返す。
    #[must_use]
    pub const fn is_startpos(&self) -> bool {
        matches!(self, Self::Startpos)
    }
}

impl From<String> for RecordInitialPosition {
    fn from(value: String) -> Self {
        Self::Sfen(value)
    }
}

impl From<&str> for RecordInitialPosition {
    fn from(value: &str) -> Self {
        Self::Sfen(value.to_string())
    }
}

/// エンジン固有の追加値。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EngineExtraValue {
    String(String),
    Int(i64),
    Float(OrderedFloat<f64>),
    Bool(bool),
}

impl From<String> for EngineExtraValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for EngineExtraValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl From<i64> for EngineExtraValue {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<i32> for EngineExtraValue {
    fn from(value: i32) -> Self {
        Self::Int(i64::from(value))
    }
}

impl From<u32> for EngineExtraValue {
    fn from(value: u32) -> Self {
        Self::Int(i64::from(value))
    }
}

impl From<f64> for EngineExtraValue {
    fn from(value: f64) -> Self {
        Self::Float(OrderedFloat(value))
    }
}

impl From<f32> for EngineExtraValue {
    fn from(value: f32) -> Self {
        Self::Float(OrderedFloat(f64::from(value)))
    }
}

impl From<bool> for EngineExtraValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

/// 1 つのレコードノードに付随するエンジン情報。
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct EngineInfo {
    eval: Option<Eval>,
    depth: Option<u16>,
    nodes: Option<u64>,
    seldepth: Option<u16>,
    extras: HashMap<String, EngineExtraValue>,
}

impl EngineInfo {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_eval(mut self, eval: Option<Eval>) -> Self {
        self.eval = eval;
        self
    }

    #[must_use]
    pub fn with_depth(mut self, depth: Option<u16>) -> Self {
        self.depth = depth;
        self
    }

    #[must_use]
    pub fn with_nodes(mut self, nodes: Option<u64>) -> Self {
        self.nodes = nodes;
        self
    }

    #[must_use]
    pub fn with_seldepth(mut self, seldepth: Option<u16>) -> Self {
        self.seldepth = seldepth;
        self
    }

    #[must_use]
    pub fn with_extra(mut self, key: String, value: EngineExtraValue) -> Self {
        self.extras.insert(key, value);
        self
    }

    #[must_use]
    pub const fn eval(&self) -> Option<Eval> {
        self.eval
    }

    #[must_use]
    pub const fn depth(&self) -> Option<u16> {
        self.depth
    }

    #[must_use]
    pub const fn nodes(&self) -> Option<u64> {
        self.nodes
    }

    #[must_use]
    pub const fn seldepth(&self) -> Option<u16> {
        self.seldepth
    }

    #[must_use]
    pub const fn extras(&self) -> &HashMap<String, EngineExtraValue> {
        &self.extras
    }

    pub fn set_eval(&mut self, eval: Option<Eval>) {
        self.eval = eval;
    }

    pub fn set_depth(&mut self, depth: Option<u16>) {
        self.depth = depth;
    }

    pub fn set_nodes(&mut self, nodes: Option<u64>) {
        self.nodes = nodes;
    }

    pub fn set_seldepth(&mut self, seldepth: Option<u16>) {
        self.seldepth = seldepth;
    }

    pub fn set_extra(&mut self, key: String, value: EngineExtraValue) {
        self.extras.insert(key, value);
    }

    #[must_use]
    pub fn remove_extra(&mut self, key: &str) -> Option<EngineExtraValue> {
        self.extras.remove(key)
    }

    pub fn clear_extras(&mut self) {
        self.extras.clear();
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.eval.is_none()
            && self.depth.is_none()
            && self.nodes.is_none()
            && self.seldepth.is_none()
            && self.extras.is_empty()
    }
}

/// [`RecordNode`] に直接保持する注釈。
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RecordAnnotation {
    comment: Option<String>,
    elapsed: Option<Duration>,
    engine_info: Option<EngineInfo>,
    tags: BTreeSet<String>,
    attributes: BTreeMap<String, String>,
}

impl RecordAnnotation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_comment(mut self, comment: Option<String>) -> Self {
        self.comment = comment;
        self
    }

    #[must_use]
    pub fn with_elapsed(mut self, elapsed: Option<Duration>) -> Self {
        self.elapsed = elapsed;
        self
    }

    #[must_use]
    pub fn with_elapsed_ms(self, elapsed_ms: Option<u32>) -> Self {
        self.with_elapsed(elapsed_ms.map(|ms| Duration::from_millis(u64::from(ms))))
    }

    #[must_use]
    pub fn with_engine_info(mut self, engine_info: Option<EngineInfo>) -> Self {
        self.engine_info = engine_info.filter(|info| !info.is_empty());
        self
    }

    #[must_use]
    pub fn comment(&self) -> Option<&str> {
        self.comment.as_deref()
    }

    #[must_use]
    pub const fn elapsed(&self) -> Option<Duration> {
        self.elapsed
    }

    #[must_use]
    pub fn elapsed_ms(&self) -> Option<u32> {
        self.elapsed.and_then(|elapsed| u32::try_from(elapsed.as_millis()).ok())
    }

    #[must_use]
    pub const fn engine_info(&self) -> Option<&EngineInfo> {
        self.engine_info.as_ref()
    }

    #[must_use]
    pub fn eval(&self) -> Option<Eval> {
        self.engine_info().and_then(EngineInfo::eval)
    }

    #[must_use]
    pub fn nodes(&self) -> Option<u64> {
        self.engine_info().and_then(EngineInfo::nodes)
    }

    #[must_use]
    pub fn depth(&self) -> Option<u16> {
        self.engine_info().and_then(EngineInfo::depth)
    }

    #[must_use]
    pub fn seldepth(&self) -> Option<u16> {
        self.engine_info().and_then(EngineInfo::seldepth)
    }

    #[must_use]
    pub const fn tags(&self) -> &BTreeSet<String> {
        &self.tags
    }

    #[must_use]
    pub const fn attributes(&self) -> &BTreeMap<String, String> {
        &self.attributes
    }

    pub fn set_comment(&mut self, comment: Option<String>) {
        self.comment = comment;
    }

    pub fn append_comment_line(&mut self, comment: &str) {
        if comment.is_empty() {
            return;
        }
        if let Some(existing) = &mut self.comment {
            if !existing.is_empty() {
                existing.push('\n');
            }
            existing.push_str(comment);
        } else {
            self.comment = Some(comment.to_string());
        }
    }

    pub fn set_elapsed(&mut self, elapsed: Option<Duration>) {
        self.elapsed = elapsed;
    }

    pub fn set_elapsed_ms(&mut self, elapsed_ms: Option<u32>) {
        self.elapsed = elapsed_ms.map(|ms| Duration::from_millis(u64::from(ms)));
    }

    pub fn set_engine_info(&mut self, engine_info: Option<EngineInfo>) {
        self.engine_info = engine_info.filter(|info| !info.is_empty());
    }

    pub fn set_eval(&mut self, eval: Option<Eval>) {
        self.ensure_engine_info().set_eval(eval);
        self.cleanup_engine_info_if_empty();
    }

    pub fn set_depth(&mut self, depth: Option<u16>) {
        self.ensure_engine_info().set_depth(depth);
        self.cleanup_engine_info_if_empty();
    }

    pub fn set_nodes(&mut self, nodes: Option<u64>) {
        self.ensure_engine_info().set_nodes(nodes);
        self.cleanup_engine_info_if_empty();
    }

    pub fn set_seldepth(&mut self, seldepth: Option<u16>) {
        self.ensure_engine_info().set_seldepth(seldepth);
        self.cleanup_engine_info_if_empty();
    }

    pub fn insert_tag(&mut self, tag: String) -> bool {
        self.tags.insert(tag)
    }

    pub fn remove_tag(&mut self, tag: &str) -> bool {
        self.tags.remove(tag)
    }

    pub fn set_attribute(&mut self, key: String, value: String) {
        self.attributes.insert(key, value);
    }

    #[must_use]
    pub fn remove_attribute(&mut self, key: &str) -> Option<String> {
        self.attributes.remove(key)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.comment.is_none()
            && self.elapsed.is_none()
            && self.engine_info.is_none()
            && self.tags.is_empty()
            && self.attributes.is_empty()
    }

    fn ensure_engine_info(&mut self) -> &mut EngineInfo {
        self.engine_info.get_or_insert_with(EngineInfo::new)
    }

    fn cleanup_engine_info_if_empty(&mut self) {
        if let Some(info) = self.engine_info.as_ref()
            && info.is_empty()
        {
            self.engine_info = None;
        }
    }
}

/// 通常手 1 つ分のエントリ。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveEntry {
    mv: Move,
}

impl MoveEntry {
    #[must_use]
    pub const fn new(mv: Move) -> Self {
        Self { mv }
    }

    #[must_use]
    pub const fn mv(&self) -> Move {
        self.mv
    }
}

/// 手とノード注釈をまとめた構築用の一時ペイロード。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AnnotatedMoveEntry {
    entry: MoveEntry,
    annotation: RecordAnnotation,
}

impl AnnotatedMoveEntry {
    #[must_use]
    pub(crate) fn new(mv: Move) -> Self {
        Self { entry: MoveEntry::new(mv), annotation: RecordAnnotation::new() }
    }

    #[must_use]
    pub(crate) fn with_comment(mut self, comment: Option<String>) -> Self {
        self.annotation.set_comment(comment);
        self
    }

    #[must_use]
    pub(crate) fn with_time_ms(mut self, time_ms: Option<u32>) -> Self {
        self.annotation.set_elapsed_ms(time_ms);
        self
    }

    #[must_use]
    pub(crate) fn with_eval(mut self, eval: Option<Eval>) -> Self {
        self.annotation.set_eval(eval);
        self
    }

    #[must_use]
    pub(crate) const fn mv(&self) -> Move {
        self.entry.mv()
    }

    #[must_use]
    pub(crate) fn comment(&self) -> Option<&str> {
        self.annotation.comment()
    }

    pub(crate) fn set_comment(&mut self, comment: Option<String>) {
        self.annotation.set_comment(comment);
    }

    pub(crate) fn set_time_ms(&mut self, time_ms: Option<u32>) {
        self.annotation.set_elapsed_ms(time_ms);
    }

    pub(crate) fn set_eval(&mut self, eval: Option<Eval>) {
        self.annotation.set_eval(eval);
    }

    #[must_use]
    pub(crate) fn into_parts(self) -> (MoveEntry, RecordAnnotation) {
        (self.entry, self.annotation)
    }
}

/// 各棋譜フォーマットが保持する標準メタデータキー。
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RecordMetadataKey {
    Event,
    Site,
    BlackPlayer,
    WhitePlayer,
    GameName,
    GameType,
    StartDate,
    EndDate,
    UpdatedDate,
    Comment,
    Other(String),
}

/// 対局単位のメタデータ。
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct RecordMetadata {
    standard: BTreeMap<RecordMetadataKey, String>,
    custom: BTreeMap<String, String>,
    time_control: Option<TimeControl>,
    black_time_control: Option<TimeControl>,
    white_time_control: Option<TimeControl>,
    max_moves: Option<u32>,
    impasse_rule: Option<String>,
}

impl RecordMetadata {
    #[must_use]
    pub fn get(&self, key: &RecordMetadataKey) -> Option<&str> {
        self.standard.get(key).map(String::as_str)
    }

    #[must_use]
    pub fn custom(&self) -> &BTreeMap<String, String> {
        &self.custom
    }

    #[must_use]
    pub fn event(&self) -> Option<&str> {
        self.get(&RecordMetadataKey::Event)
    }

    #[must_use]
    pub fn site(&self) -> Option<&str> {
        self.get(&RecordMetadataKey::Site)
    }

    #[must_use]
    pub fn black_player(&self) -> Option<&str> {
        self.get(&RecordMetadataKey::BlackPlayer)
    }

    #[must_use]
    pub fn white_player(&self) -> Option<&str> {
        self.get(&RecordMetadataKey::WhitePlayer)
    }

    #[must_use]
    pub fn game_name(&self) -> Option<&str> {
        self.get(&RecordMetadataKey::GameName)
    }

    #[must_use]
    pub fn game_type(&self) -> Option<&str> {
        self.get(&RecordMetadataKey::GameType)
    }

    #[must_use]
    pub const fn time_control(&self) -> Option<&TimeControl> {
        self.time_control.as_ref()
    }

    #[must_use]
    pub const fn black_time_control(&self) -> Option<&TimeControl> {
        self.black_time_control.as_ref()
    }

    #[must_use]
    pub const fn white_time_control(&self) -> Option<&TimeControl> {
        self.white_time_control.as_ref()
    }

    #[must_use]
    pub const fn max_moves(&self) -> Option<u32> {
        self.max_moves
    }

    #[must_use]
    pub fn impasse_rule(&self) -> Option<&str> {
        self.impasse_rule.as_deref()
    }

    #[must_use]
    pub fn start_date(&self) -> Option<&str> {
        self.get(&RecordMetadataKey::StartDate)
    }

    #[must_use]
    pub fn end_date(&self) -> Option<&str> {
        self.get(&RecordMetadataKey::EndDate)
    }

    #[must_use]
    pub fn updated_date(&self) -> Option<&str> {
        self.get(&RecordMetadataKey::UpdatedDate)
    }

    #[must_use]
    pub fn comment(&self) -> Option<&str> {
        self.get(&RecordMetadataKey::Comment)
    }

    #[must_use]
    pub fn attributes(&self) -> &BTreeMap<String, String> {
        &self.custom
    }

    #[must_use]
    pub fn builder() -> RecordMetadataBuilder {
        RecordMetadataBuilder::default()
    }
}

/// [`RecordMetadata`] のビルダー。
#[derive(Default)]
pub struct RecordMetadataBuilder {
    metadata: RecordMetadata,
}

impl RecordMetadataBuilder {
    pub fn set_standard(&mut self, key: RecordMetadataKey, value: Option<String>) -> &mut Self {
        if let Some(value) = value {
            self.metadata.standard.insert(key, value);
        } else {
            self.metadata.standard.remove(&key);
        }
        self
    }

    pub fn event(&mut self, value: Option<String>) -> &mut Self {
        self.set_standard(RecordMetadataKey::Event, value)
    }

    pub fn site(&mut self, value: Option<String>) -> &mut Self {
        self.set_standard(RecordMetadataKey::Site, value)
    }

    pub fn black_player(&mut self, value: Option<String>) -> &mut Self {
        self.set_standard(RecordMetadataKey::BlackPlayer, value)
    }

    pub fn white_player(&mut self, value: Option<String>) -> &mut Self {
        self.set_standard(RecordMetadataKey::WhitePlayer, value)
    }

    pub fn game_name(&mut self, value: Option<String>) -> &mut Self {
        self.set_standard(RecordMetadataKey::GameName, value)
    }

    pub fn game_type(&mut self, value: Option<String>) -> &mut Self {
        self.set_standard(RecordMetadataKey::GameType, value)
    }

    pub fn time_control(&mut self, value: Option<TimeControl>) -> &mut Self {
        self.metadata.time_control = value;
        self
    }

    pub fn black_time_control(&mut self, value: Option<TimeControl>) -> &mut Self {
        self.metadata.black_time_control = value;
        self
    }

    pub fn white_time_control(&mut self, value: Option<TimeControl>) -> &mut Self {
        self.metadata.white_time_control = value;
        self
    }

    pub fn max_moves(&mut self, value: Option<u32>) -> &mut Self {
        self.metadata.max_moves = value;
        self
    }

    pub fn impasse_rule(&mut self, value: Option<String>) -> &mut Self {
        self.metadata.impasse_rule = value;
        self
    }

    pub fn start_date(&mut self, value: Option<String>) -> &mut Self {
        self.set_standard(RecordMetadataKey::StartDate, value)
    }

    pub fn end_date(&mut self, value: Option<String>) -> &mut Self {
        self.set_standard(RecordMetadataKey::EndDate, value)
    }

    pub fn updated_date(&mut self, value: Option<String>) -> &mut Self {
        self.set_standard(RecordMetadataKey::UpdatedDate, value)
    }

    pub fn comment(&mut self, value: Option<String>) -> &mut Self {
        self.set_standard(RecordMetadataKey::Comment, value)
    }

    pub fn append_comment_line(&mut self, value: &str) -> &mut Self {
        if value.is_empty() {
            return self;
        }
        let key = RecordMetadataKey::Comment;
        self.metadata
            .standard
            .entry(key)
            .and_modify(|comment| {
                if !comment.is_empty() {
                    comment.push('\n');
                }
                comment.push_str(value);
            })
            .or_insert_with(|| value.to_string());
        self
    }

    pub fn add_attribute(&mut self, key: String, value: String) -> &mut Self {
        self.metadata.custom.insert(key, value);
        self
    }

    #[must_use]
    pub fn build(self) -> RecordMetadata {
        self.metadata
    }
}

/// 終局を表す特殊手の種別。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecialMove {
    Interrupt,
    Resign,
    MaxMoves,
    Impasse,
    Draw,
    RepetitionDraw,
    Mate,
    NoMate,
    Timeout,
    WinByIllegalMove,
    LoseByIllegalMove,
    WinByDeclaration,
    WinByDefault,
    LoseByDefault,
    Try,
    Unknown(String),
}

/// 終局の特殊エントリ。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecialMoveEntry {
    kind: SpecialMove,
    result: GameResult,
    raw: Option<String>,
}

impl SpecialMoveEntry {
    #[must_use]
    pub const fn new(kind: SpecialMove, result: GameResult) -> Self {
        Self { kind, result, raw: None }
    }

    #[must_use]
    pub fn with_raw(mut self, raw: Option<String>) -> Self {
        self.raw = raw;
        self
    }

    #[must_use]
    pub const fn kind(&self) -> &SpecialMove {
        &self.kind
    }

    #[must_use]
    pub const fn result(&self) -> GameResult {
        self.result
    }

    #[must_use]
    pub fn raw(&self) -> Option<&str> {
        self.raw.as_deref()
    }

    pub fn set_raw(&mut self, raw: Option<String>) {
        self.raw = raw;
    }
}

/// ノードのペイロード。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordEntry {
    Move(MoveEntry),
    Special(SpecialMoveEntry),
}

impl RecordEntry {
    #[must_use]
    pub fn as_move(&self) -> Option<&MoveEntry> {
        match self {
            Self::Move(mv) => Some(mv),
            Self::Special(_) => None,
        }
    }

    #[must_use]
    pub fn as_special(&self) -> Option<&SpecialMoveEntry> {
        match self {
            Self::Move(_) => None,
            Self::Special(special) => Some(special),
        }
    }
}

/// 不透明なノード ID。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordNodeId(NonZeroU32);

impl RecordNodeId {
    /// 現在の raw なスロットインデックスを返す。
    ///
    /// この値は生成元の [`Record`] に対してのみ意味を持ち、安定した
    /// シリアライズ形式ではない。
    #[must_use]
    pub fn raw(self) -> usize {
        self.slot_index()
    }

    /// raw なスロットインデックスからノード ID を構築する。
    ///
    /// 信頼できない入力や外部入力には [`try_from_raw`](Self::try_from_raw) を優先する。
    /// このコンストラクタは、値が内部の ID 表現に収まらない場合は panic する。
    #[must_use]
    pub fn from_raw(raw: usize) -> Self {
        Self::from_slot_index(raw).expect("record node id must fit in u32")
    }

    /// raw なスロットインデックスからノード ID の構築を試みる。
    ///
    /// 表現範囲だけを検査する。呼び出し側は、対象の [`Record`] に対して ID を
    /// 別途検証する必要がある。
    #[must_use]
    pub fn try_from_raw(raw: usize) -> Option<Self> {
        Self::from_slot_index(raw)
    }

    #[must_use]
    pub(crate) fn slot_index(self) -> usize {
        usize::try_from(self.0.get()).expect("u32 fits usize") - 1
    }

    fn from_slot_index(index: usize) -> Option<Self> {
        let slot = u32::try_from(index.checked_add(1)?).ok()?;
        NonZeroU32::new(slot).map(Self)
    }
}

/// 棋譜ツリー内の 1 ノード。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordNode {
    parent: Option<RecordNodeId>,
    children: Vec<RecordNodeId>,
    entry: Option<RecordEntry>,
    annotation: RecordAnnotation,
}

impl RecordNode {
    fn root() -> Self {
        Self {
            parent: None,
            children: Vec::new(),
            entry: None,
            annotation: RecordAnnotation::new(),
        }
    }

    fn with_entry_and_annotation(
        parent: RecordNodeId,
        entry: RecordEntry,
        annotation: RecordAnnotation,
    ) -> Self {
        Self { parent: Some(parent), children: Vec::new(), entry: Some(entry), annotation }
    }

    #[must_use]
    pub fn parent(&self) -> Option<RecordNodeId> {
        self.parent
    }

    #[must_use]
    pub fn children(&self) -> &[RecordNodeId] {
        &self.children
    }

    #[must_use]
    pub fn variations(&self) -> &[RecordNodeId] {
        &self.children
    }

    #[must_use]
    pub fn entry(&self) -> Option<&RecordEntry> {
        self.entry.as_ref()
    }

    #[must_use]
    pub fn annotation(&self) -> &RecordAnnotation {
        &self.annotation
    }

    #[must_use]
    pub fn annotation_mut(&mut self) -> &mut RecordAnnotation {
        &mut self.annotation
    }

    #[must_use]
    pub fn mv(&self) -> Option<&MoveEntry> {
        self.entry().and_then(RecordEntry::as_move)
    }

    #[must_use]
    pub fn special(&self) -> Option<&SpecialMoveEntry> {
        self.entry().and_then(RecordEntry::as_special)
    }

    fn set_special(&mut self, special: SpecialMoveEntry) {
        self.entry = Some(RecordEntry::Special(special));
    }

    #[must_use]
    pub fn comment(&self) -> Option<&str> {
        self.annotation.comment()
    }

    #[must_use]
    pub fn time_ms(&self) -> Option<u32> {
        self.annotation.elapsed_ms()
    }

    #[must_use]
    pub const fn engine_info(&self) -> Option<&EngineInfo> {
        self.annotation.engine_info()
    }

    #[must_use]
    pub fn eval(&self) -> Option<Eval> {
        self.annotation.eval()
    }

    #[must_use]
    pub fn nodes(&self) -> Option<u64> {
        self.annotation.nodes()
    }

    #[must_use]
    pub fn depth(&self) -> Option<u16> {
        self.annotation.depth()
    }

    #[must_use]
    pub fn seldepth(&self) -> Option<u16> {
        self.annotation.seldepth()
    }

    pub fn set_comment(&mut self, comment: Option<String>) {
        self.annotation.set_comment(comment);
    }

    pub fn append_comment_line(&mut self, comment: &str) {
        self.annotation.append_comment_line(comment);
    }

    pub fn set_time_ms(&mut self, time_ms: Option<u32>) {
        self.annotation.set_elapsed_ms(time_ms);
    }

    pub fn set_engine_info(&mut self, engine_info: Option<EngineInfo>) {
        self.annotation.set_engine_info(engine_info);
    }

    pub fn set_eval(&mut self, eval: Option<Eval>) {
        self.annotation.set_eval(eval);
    }

    pub fn set_depth(&mut self, depth: Option<u16>) {
        self.annotation.set_depth(depth);
    }

    pub fn set_nodes(&mut self, nodes: Option<u64>) {
        self.annotation.set_nodes(nodes);
    }

    pub fn set_seldepth(&mut self, seldepth: Option<u16>) {
        self.annotation.set_seldepth(seldepth);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum NodeSlot {
    Occupied(Box<RecordNode>),
    Removed,
}

/// 棋譜ツリー。
#[derive(Clone, Debug)]
pub struct Record {
    initial_position: RecordInitialPosition,
    metadata: RecordMetadata,
    root: RecordNodeId,
    nodes: Vec<NodeSlot>,
}

impl PartialEq for Record {
    fn eq(&self, other: &Self) -> bool {
        self.initial_position == other.initial_position
            && self.metadata == other.metadata
            && nodes_logically_equal(self, self.root, other, other.root)
    }
}

impl Eq for Record {}

impl Record {
    /// 初期局面から空のレコードを生成する。
    pub fn new(initial_position: impl Into<RecordInitialPosition>) -> Result<Self, RecordError> {
        let initial_position = initial_position.into();
        if let RecordInitialPosition::Sfen(sfen) = &initial_position
            && sfen.is_empty()
        {
            return Err(RecordError::EmptyInitPosition);
        }

        let root = RecordNodeId::from_raw(0);
        Ok(Self {
            initial_position,
            metadata: RecordMetadata::default(),
            root,
            nodes: vec![NodeSlot::Occupied(Box::new(RecordNode::root()))],
        })
    }

    /// 本譜と終局エントリからレコードを生成する。
    pub fn from_main_line(
        initial_position: impl Into<RecordInitialPosition>,
        moves: Vec<MoveEntry>,
        terminal: Option<SpecialMoveEntry>,
    ) -> Result<Self, RecordError> {
        let mut record = Self::new(initial_position)?;
        record.extend_main_line(moves)?;
        if let Some(terminal) = terminal {
            record.set_main_terminal(terminal)?;
        }
        Ok(record)
    }

    pub(crate) fn from_annotated_main_line(
        initial_position: impl Into<RecordInitialPosition>,
        moves: Vec<AnnotatedMoveEntry>,
        terminal: Option<(SpecialMoveEntry, RecordAnnotation)>,
    ) -> Result<Self, RecordError> {
        let mut record = Self::new(initial_position)?;
        record.extend_main_line_with_annotations(moves)?;
        if let Some((terminal, annotation)) = terminal {
            record.set_main_terminal_with_annotation(terminal, annotation)?;
        }
        Ok(record)
    }

    /// 本譜と対局結果からレコードを生成する。
    pub fn from_main_line_with_result(
        initial_position: impl Into<RecordInitialPosition>,
        moves: Vec<MoveEntry>,
        result: GameResult,
    ) -> Result<Self, RecordError> {
        let terminal = SpecialMoveEntry::new(special_from_game_result(result), result);
        Self::from_main_line(initial_position, moves, Some(terminal))
    }

    /// このレコードを所有権を持つエディタへ変換する。
    pub fn into_editor(self) -> Result<RecordEditor, RecordError> {
        RecordEditor::from_record(self)
    }

    #[must_use]
    pub fn initial_position(&self) -> &RecordInitialPosition {
        &self.initial_position
    }

    #[must_use]
    pub fn init_position_sfen(&self) -> &str {
        self.initial_position.to_sfen()
    }

    #[must_use]
    pub fn initial_comment(&self) -> Option<&str> {
        self.root().annotation().comment()
    }

    pub fn set_initial_comment(&mut self, comment: Option<String>) {
        self.root_mut().annotation_mut().set_comment(comment);
    }

    #[must_use]
    pub const fn metadata(&self) -> &RecordMetadata {
        &self.metadata
    }

    pub fn set_metadata(&mut self, metadata: RecordMetadata) {
        self.metadata = metadata;
    }

    #[must_use]
    pub fn result(&self) -> GameResult {
        self.main_terminal().map_or(GameResult::Invalid, SpecialMoveEntry::result)
    }

    #[must_use]
    pub fn main_terminal(&self) -> Option<&SpecialMoveEntry> {
        self.main_terminal_node().and_then(|node_id| self.node(node_id).special())
    }

    #[must_use]
    pub fn main_terminal_node(&self) -> Option<RecordNodeId> {
        let tail = self.main_line_tail();
        if tail == self.root || self.node(tail).special().is_none() {
            return None;
        }
        Some(tail)
    }

    pub fn set_main_terminal(
        &mut self,
        special: SpecialMoveEntry,
    ) -> Result<RecordNodeId, RecordError> {
        self.set_main_terminal_with_annotation(special, RecordAnnotation::new())
    }

    pub fn set_main_terminal_with_annotation(
        &mut self,
        special: SpecialMoveEntry,
        annotation: RecordAnnotation,
    ) -> Result<RecordNodeId, RecordError> {
        let tail = self.main_line_tail();
        if tail != self.root && self.node(tail).special().is_some() {
            let node = self.node_mut(tail)?;
            node.set_special(special);
            node.annotation = annotation;
            return Ok(tail);
        }
        self.append_child_with_annotation(tail, RecordEntry::Special(special), annotation)
    }

    #[must_use]
    pub const fn root_id(&self) -> RecordNodeId {
        self.root
    }

    #[must_use]
    pub fn root(&self) -> &RecordNode {
        self.node(self.root)
    }

    #[must_use]
    pub fn root_mut(&mut self) -> &mut RecordNode {
        self.node_mut(self.root).expect("root exists")
    }

    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.iter().filter(|slot| matches!(slot, NodeSlot::Occupied(_))).count()
    }

    #[must_use]
    pub fn slot_count(&self) -> usize {
        self.nodes.len()
    }

    #[must_use]
    pub fn node(&self, node_id: RecordNodeId) -> &RecordNode {
        self.try_node(node_id).expect("record node id must be valid")
    }

    pub fn try_node(&self, node_id: RecordNodeId) -> Result<&RecordNode, RecordError> {
        match self.nodes.get(node_id.slot_index()) {
            Some(NodeSlot::Occupied(node)) => Ok(node),
            Some(NodeSlot::Removed) => Err(RecordError::RemovedNode(node_id.raw())),
            None => Err(RecordError::InvalidNodeId(node_id.raw())),
        }
    }

    pub fn node_mut(&mut self, node_id: RecordNodeId) -> Result<&mut RecordNode, RecordError> {
        match self.nodes.get_mut(node_id.slot_index()) {
            Some(NodeSlot::Occupied(node)) => Ok(node),
            Some(NodeSlot::Removed) => Err(RecordError::RemovedNode(node_id.raw())),
            None => Err(RecordError::InvalidNodeId(node_id.raw())),
        }
    }

    #[must_use]
    pub fn children(&self, node_id: RecordNodeId) -> &[RecordNodeId] {
        self.try_children(node_id).expect("record node id must be valid")
    }

    pub fn try_children(&self, node_id: RecordNodeId) -> Result<&[RecordNodeId], RecordError> {
        Ok(self.try_node(node_id)?.children())
    }

    #[must_use]
    pub fn main_child(&self, node_id: RecordNodeId) -> Option<RecordNodeId> {
        self.node(node_id).children.first().copied()
    }

    #[must_use]
    pub fn variation_children(&self, node_id: RecordNodeId) -> &[RecordNodeId] {
        self.children(node_id).get(1..).unwrap_or(&[])
    }

    #[must_use]
    pub fn main_moves(&self) -> MainLineMoves<'_> {
        MainLineMoves { record: self, current: self.main_child(self.root) }
    }

    #[must_use]
    pub fn moves(&self) -> Vec<MoveEntry> {
        self.main_moves().cloned().collect()
    }

    #[must_use]
    pub fn move_count(&self) -> usize {
        self.main_moves().count()
    }

    #[must_use]
    pub fn main_line_ids(&self) -> Vec<RecordNodeId> {
        let mut ids = Vec::new();
        let mut current = self.main_child(self.root);
        while let Some(node_id) = current {
            if self.node(node_id).mv().is_some() {
                ids.push(node_id);
            }
            current = self.main_child(node_id);
        }
        ids
    }

    #[must_use]
    pub fn main_line_tail(&self) -> RecordNodeId {
        let mut current = self.root;
        while let Some(next) = self.main_child(current) {
            current = next;
        }
        current
    }

    pub fn line_to(&self, node_id: RecordNodeId) -> Result<Vec<RecordNodeId>, RecordError> {
        let mut line = Vec::new();
        let mut current = Some(node_id);
        while let Some(id) = current {
            let node = self.try_node(id)?;
            line.push(id);
            current = node.parent();
        }
        line.reverse();
        Ok(line)
    }

    pub fn append_child(
        &mut self,
        parent: RecordNodeId,
        entry: RecordEntry,
    ) -> Result<RecordNodeId, RecordError> {
        self.append_child_with_annotation(parent, entry, RecordAnnotation::new())
    }

    pub fn append_child_with_annotation(
        &mut self,
        parent: RecordNodeId,
        entry: RecordEntry,
        annotation: RecordAnnotation,
    ) -> Result<RecordNodeId, RecordError> {
        if self.try_node(parent)?.special().is_some() {
            return Err(RecordError::TerminalNode(parent.raw()));
        }

        let child =
            self.push_node(RecordNode::with_entry_and_annotation(parent, entry, annotation));
        self.node_mut(parent)?.children.push(child);
        Ok(child)
    }

    pub fn append_move(
        &mut self,
        parent: RecordNodeId,
        mv: MoveEntry,
    ) -> Result<RecordNodeId, RecordError> {
        self.append_child(parent, RecordEntry::Move(mv))
    }

    pub fn append_move_with_annotation(
        &mut self,
        parent: RecordNodeId,
        mv: MoveEntry,
        annotation: RecordAnnotation,
    ) -> Result<RecordNodeId, RecordError> {
        self.append_child_with_annotation(parent, RecordEntry::Move(mv), annotation)
    }

    pub fn append_special(
        &mut self,
        parent: RecordNodeId,
        special: SpecialMoveEntry,
    ) -> Result<RecordNodeId, RecordError> {
        self.append_child(parent, RecordEntry::Special(special))
    }

    pub fn append_special_with_annotation(
        &mut self,
        parent: RecordNodeId,
        special: SpecialMoveEntry,
        annotation: RecordAnnotation,
    ) -> Result<RecordNodeId, RecordError> {
        self.append_child_with_annotation(parent, RecordEntry::Special(special), annotation)
    }

    pub fn extend_main_line(
        &mut self,
        moves: Vec<MoveEntry>,
    ) -> Result<Vec<RecordNodeId>, RecordError> {
        let mut created = Vec::new();
        let mut current = self.main_line_tail();
        for mv in moves {
            let child = self.append_move(current, mv)?;
            created.push(child);
            current = child;
        }
        Ok(created)
    }

    pub(crate) fn extend_main_line_with_annotations(
        &mut self,
        moves: Vec<AnnotatedMoveEntry>,
    ) -> Result<Vec<RecordNodeId>, RecordError> {
        let mut created = Vec::new();
        let mut current = self.main_line_tail();
        for mv in moves {
            let (entry, annotation) = mv.into_parts();
            let child = self.append_move_with_annotation(current, entry, annotation)?;
            created.push(child);
            current = child;
        }
        Ok(created)
    }

    pub fn add_variation_line(
        &mut self,
        parent: RecordNodeId,
        moves: Vec<MoveEntry>,
    ) -> Result<Vec<RecordNodeId>, RecordError> {
        if moves.is_empty() {
            return Ok(Vec::new());
        }

        let mut created = Vec::new();
        let mut current = parent;
        for mv in moves {
            let child = self.append_move(current, mv)?;
            created.push(child);
            current = child;
        }
        Ok(created)
    }

    pub(crate) fn add_variation_line_with_annotations(
        &mut self,
        parent: RecordNodeId,
        moves: Vec<AnnotatedMoveEntry>,
    ) -> Result<Vec<RecordNodeId>, RecordError> {
        if moves.is_empty() {
            return Ok(Vec::new());
        }

        let mut created = Vec::new();
        let mut current = parent;
        for mv in moves {
            let (entry, annotation) = mv.into_parts();
            let child = self.append_move_with_annotation(current, entry, annotation)?;
            created.push(child);
            current = child;
        }
        Ok(created)
    }

    pub fn promote_child_to_main(
        &mut self,
        parent: RecordNodeId,
        child: RecordNodeId,
    ) -> Result<(), RecordError> {
        let parent_node = self.node_mut(parent)?;
        let Some(index) = parent_node.children.iter().position(|&id| id == child) else {
            return Err(RecordError::NotChild { parent: parent.raw(), child: child.raw() });
        };
        parent_node.children.remove(index);
        parent_node.children.insert(0, child);
        Ok(())
    }

    pub fn swap_children(
        &mut self,
        parent: RecordNodeId,
        a: usize,
        b: usize,
    ) -> Result<(), RecordError> {
        let parent_node = self.node_mut(parent)?;
        if a >= parent_node.children.len() || b >= parent_node.children.len() {
            return Err(RecordError::InvalidChildIndex {
                parent: parent.raw(),
                index: if a >= parent_node.children.len() { a } else { b },
            });
        }
        parent_node.children.swap(a, b);
        Ok(())
    }

    pub fn detach_subtree(&mut self, node_id: RecordNodeId) -> Result<(), RecordError> {
        if node_id == self.root {
            return Err(RecordError::InvalidNodeId(node_id.raw()));
        }
        let parent = self.try_node(node_id)?.parent();
        if let Some(parent) = parent {
            let parent_node = self.node_mut(parent)?;
            parent_node.children.retain(|&child| child != node_id);
        }
        self.remove_subtree_slots(node_id)
    }

    pub fn subtree(
        &self,
        node_id: RecordNodeId,
        options: SubtreeOptions,
    ) -> Result<Record, RecordError> {
        let mut out = Record::new(self.subtree_initial_position(node_id)?)?;
        if options.preserve_root_annotation {
            out.root_mut().annotation = self.try_node(node_id)?.annotation().clone();
        }
        let out_root = out.root_id();
        for &child in self.try_node(node_id)?.children() {
            copy_subtree(self, child, &mut out, out_root)?;
        }
        Ok(out)
    }

    fn subtree_initial_position(
        &self,
        node_id: RecordNodeId,
    ) -> Result<RecordInitialPosition, RecordError> {
        self.try_node(node_id)?;
        if node_id == self.root {
            return Ok(self.initial_position.clone());
        }
        let mut position = Position::empty();
        position.set_sfen(self.init_position_sfen())?;
        for id in self.line_to(node_id)?.into_iter().skip(1) {
            if let Some(mv) = self.node(id).mv() {
                if !position.is_legal_move(mv.mv()) {
                    return Err(RecordError::IllegalMove);
                }
                position.apply_move(mv.mv());
            }
        }
        Ok(RecordInitialPosition::Sfen(position.to_sfen(None)))
    }

    fn push_node(&mut self, node: RecordNode) -> RecordNodeId {
        let id = RecordNodeId::from_slot_index(self.nodes.len()).expect("record node id overflow");
        self.nodes.push(NodeSlot::Occupied(Box::new(node)));
        id
    }

    fn remove_subtree_slots(&mut self, node_id: RecordNodeId) -> Result<(), RecordError> {
        let children = self.try_node(node_id)?.children.clone();
        for child in children {
            self.remove_subtree_slots(child)?;
        }
        let slot = self
            .nodes
            .get_mut(node_id.slot_index())
            .ok_or(RecordError::InvalidNodeId(node_id.raw()))?;
        *slot = NodeSlot::Removed;
        Ok(())
    }
}

/// [`Record::subtree`] のオプション。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubtreeOptions {
    pub preserve_root_annotation: bool,
}

impl Default for SubtreeOptions {
    fn default() -> Self {
        Self { preserve_root_annotation: true }
    }
}

/// 本譜の手を辿るイテレータ。
pub struct MainLineMoves<'a> {
    record: &'a Record,
    current: Option<RecordNodeId>,
}

impl<'a> Iterator for MainLineMoves<'a> {
    type Item = &'a MoveEntry;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(current_id) = self.current {
            let node = self.record.node(current_id);
            self.current = self.record.main_child(current_id);
            if let Some(mv) = node.mv() {
                return Some(mv);
            }
        }
        None
    }
}

/// レコードのマージで使う重複の扱い。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DuplicateHandling {
    ReuseExisting,
    AlwaysCreate,
}

/// [`RecordEditor::merge_record`] のマージオプション。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MergeOptions {
    pub duplicate_handling: DuplicateHandling,
}

impl Default for MergeOptions {
    fn default() -> Self {
        Self { duplicate_handling: DuplicateHandling::ReuseExisting }
    }
}

/// 一時的なカーソルと局面を持つ所有エディタ。
pub struct RecordEditor {
    record: Record,
    current: RecordNodeId,
    position: Position,
}

impl RecordEditor {
    pub fn new(initial_position: impl Into<RecordInitialPosition>) -> Result<Self, RecordError> {
        Self::from_record(Record::new(initial_position)?)
    }

    pub fn from_record(record: Record) -> Result<Self, RecordError> {
        let mut position = Position::empty();
        position.set_sfen(record.init_position_sfen())?;
        let current = record.root_id();
        Ok(Self { record, current, position })
    }

    #[must_use]
    pub fn into_record(self) -> Record {
        self.record
    }

    #[must_use]
    pub const fn current_node(&self) -> RecordNodeId {
        self.current
    }

    #[must_use]
    pub const fn position(&self) -> &Position {
        &self.position
    }

    #[must_use]
    pub const fn record(&self) -> &Record {
        &self.record
    }

    pub fn go_to(&mut self, node_id: RecordNodeId) -> Result<(), RecordError> {
        self.record.try_node(node_id)?;
        let mut position = Position::empty();
        position.set_sfen(self.record.init_position_sfen())?;
        for id in self.record.line_to(node_id)?.into_iter().skip(1) {
            let node = self.record.node(id);
            if let Some(mv) = node.mv() {
                if !position.is_legal_move(mv.mv()) {
                    return Err(RecordError::IllegalMove);
                }
                position.apply_move(mv.mv());
            }
        }
        self.position = position;
        self.current = node_id;
        Ok(())
    }

    pub fn go_back(&mut self) -> Result<bool, RecordError> {
        let Some(parent) = self.record.try_node(self.current)?.parent() else {
            return Ok(false);
        };
        self.go_to(parent)?;
        Ok(true)
    }

    pub fn go_forward(&mut self, child_index: usize) -> Result<bool, RecordError> {
        let Some(&child) = self.record.try_node(self.current)?.children().get(child_index) else {
            return Ok(false);
        };
        self.go_to(child)?;
        Ok(true)
    }

    pub fn branch_to(&mut self, child_index: usize) -> Result<bool, RecordError> {
        let Some(&child) = self.record.try_node(self.current)?.children().get(child_index) else {
            return Ok(false);
        };
        self.record.promote_child_to_main(self.current, child)?;
        self.go_to(child)?;
        Ok(true)
    }

    pub fn append_move(&mut self, mv: MoveEntry) -> Result<RecordNodeId, RecordError> {
        if !self.position.is_legal_move(mv.mv()) {
            return Err(RecordError::IllegalMove);
        }
        let mv16 = mv.mv();
        let child = self.record.append_move(self.current, mv)?;
        self.position.apply_move(mv16);
        self.current = child;
        Ok(child)
    }

    pub fn append_move_with_annotation(
        &mut self,
        mv: MoveEntry,
        annotation: RecordAnnotation,
    ) -> Result<RecordNodeId, RecordError> {
        if !self.position.is_legal_move(mv.mv()) {
            return Err(RecordError::IllegalMove);
        }
        let mv16 = mv.mv();
        let child = self.record.append_move_with_annotation(self.current, mv, annotation)?;
        self.position.apply_move(mv16);
        self.current = child;
        Ok(child)
    }

    pub fn append_special(
        &mut self,
        special: SpecialMoveEntry,
    ) -> Result<RecordNodeId, RecordError> {
        let child = self.record.append_special(self.current, special)?;
        self.current = child;
        Ok(child)
    }

    pub fn append_special_with_annotation(
        &mut self,
        special: SpecialMoveEntry,
        annotation: RecordAnnotation,
    ) -> Result<RecordNodeId, RecordError> {
        let child =
            self.record.append_special_with_annotation(self.current, special, annotation)?;
        self.current = child;
        Ok(child)
    }

    pub fn merge_record(
        &mut self,
        other: &Record,
        options: MergeOptions,
    ) -> Result<Vec<RecordNodeId>, RecordError> {
        let mut created = Vec::new();
        let mut source = other.main_child(other.root_id());
        while let Some(source_id) = source {
            let source_node = other.try_node(source_id)?;
            let Some(entry) = source_node.entry().cloned() else {
                source = other.main_child(source_id);
                continue;
            };
            let target = match options.duplicate_handling {
                DuplicateHandling::ReuseExisting => self.find_matching_child(self.current, &entry),
                DuplicateHandling::AlwaysCreate => None,
            };
            let child = if let Some(existing) = target {
                existing
            } else {
                let child = self.record.append_child_with_annotation(
                    self.current,
                    entry.clone(),
                    source_node.annotation().clone(),
                )?;
                created.push(child);
                child
            };
            if let RecordEntry::Move(mv) = &entry {
                if !self.position.is_legal_move(mv.mv()) {
                    return Err(RecordError::IllegalMove);
                }
                self.position.apply_move(mv.mv());
            }
            self.current = child;
            source = other.main_child(source_id);
        }
        Ok(created)
    }

    fn find_matching_child(
        &self,
        parent: RecordNodeId,
        entry: &RecordEntry,
    ) -> Option<RecordNodeId> {
        self.record.children(parent).iter().copied().find(|&child| {
            self.record.node(child).entry().is_some_and(|candidate| candidate == entry)
        })
    }
}

fn copy_subtree(
    source: &Record,
    source_id: RecordNodeId,
    target: &mut Record,
    target_parent: RecordNodeId,
) -> Result<RecordNodeId, RecordError> {
    let source_node = source.try_node(source_id)?;
    let Some(entry) = source_node.entry().cloned() else {
        return Ok(target_parent);
    };
    let copied = target.append_child_with_annotation(
        target_parent,
        entry,
        source_node.annotation().clone(),
    )?;
    for &child in source_node.children() {
        copy_subtree(source, child, target, copied)?;
    }
    Ok(copied)
}

fn nodes_logically_equal(
    left_record: &Record,
    left_id: RecordNodeId,
    right_record: &Record,
    right_id: RecordNodeId,
) -> bool {
    let left = left_record.node(left_id);
    let right = right_record.node(right_id);
    left.entry == right.entry
        && left.annotation == right.annotation
        && left.children.len() == right.children.len()
        && left.children.iter().zip(&right.children).all(|(&left_child, &right_child)| {
            nodes_logically_equal(left_record, left_child, right_record, right_child)
        })
}

fn special_from_game_result(result: GameResult) -> SpecialMove {
    match result {
        GameResult::BlackWin | GameResult::WhiteWin => SpecialMove::Resign,
        GameResult::DrawByRepetition => SpecialMove::RepetitionDraw,
        GameResult::Error | GameResult::Invalid | GameResult::Paused => SpecialMove::Interrupt,
        GameResult::BlackWinByDeclaration | GameResult::WhiteWinByDeclaration => {
            SpecialMove::WinByDeclaration
        }
        GameResult::BlackWinByTryRule | GameResult::WhiteWinByTryRule => SpecialMove::Try,
        GameResult::DrawByMaxPlies => SpecialMove::MaxMoves,
        GameResult::BlackWinByForfeit | GameResult::WhiteWinByForfeit => SpecialMove::WinByDefault,
        GameResult::DrawByImpasse => SpecialMove::Impasse,
        GameResult::BlackWinByIllegalMove | GameResult::WhiteWinByIllegalMove => {
            SpecialMove::WinByIllegalMove
        }
        GameResult::BlackWinByTimeout | GameResult::WhiteWinByTimeout => SpecialMove::Timeout,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::hirate_position;

    #[test]
    fn record_rejects_append_under_special_node() {
        let pos = hirate_position();
        let mut record = Record::new(pos.to_sfen(None)).unwrap();
        let first = Move::from_usi("7g7f").unwrap();
        record.extend_main_line(vec![MoveEntry::new(first)]).unwrap();
        let terminal = record
            .set_main_terminal(SpecialMoveEntry::new(SpecialMove::Interrupt, GameResult::Invalid))
            .unwrap();

        let after_terminal = Move::from_usi("3c3d").unwrap();
        let result = record.append_move(terminal, MoveEntry::new(after_terminal));

        assert!(matches!(result, Err(RecordError::TerminalNode(_))));
        assert!(record.children(terminal).is_empty());
    }

    #[test]
    fn record_append_child_does_not_implicitly_promote_to_main() {
        let pos = hirate_position();
        let mut record = Record::new(pos.to_sfen(None)).unwrap();
        let root = record.root_id();
        let main =
            record.append_move(root, MoveEntry::new(Move::from_usi("7g7f").unwrap())).unwrap();
        let variation =
            record.append_move(root, MoveEntry::new(Move::from_usi("2g2f").unwrap())).unwrap();

        assert_eq!(record.children(root), &[main, variation]);
        assert_eq!(record.main_line_ids(), vec![main]);

        record.promote_child_to_main(root, variation).unwrap();
        assert_eq!(record.children(root), &[variation, main]);
        assert_eq!(record.main_line_ids(), vec![variation]);
    }

    #[test]
    fn record_node_id_option_is_four_bytes() {
        assert_eq!(std::mem::size_of::<Option<RecordNodeId>>(), 4);
    }

    #[test]
    fn subtree_uses_selected_node_position_as_initial_position() {
        let pos = hirate_position();
        let mut record = Record::new(pos.to_sfen(None)).unwrap();
        let first = record
            .append_move(record.root_id(), MoveEntry::new(Move::from_usi("7g7f").unwrap()))
            .unwrap();
        let main =
            record.append_move(first, MoveEntry::new(Move::from_usi("3c3d").unwrap())).unwrap();
        let variation =
            record.append_move(first, MoveEntry::new(Move::from_usi("8c8d").unwrap())).unwrap();

        let subtree = record.subtree(first, SubtreeOptions::default()).unwrap();

        let mut expected = hirate_position();
        expected.apply_move(Move::from_usi("7g7f").unwrap());
        assert_eq!(subtree.init_position_sfen(), expected.to_sfen(None));
        assert_eq!(
            subtree.moves().iter().map(|mv| mv.mv().to_usi()).collect::<Vec<_>>(),
            vec!["3c3d"]
        );
        assert_eq!(subtree.children(subtree.root_id()).len(), 2);
        assert_eq!(
            subtree.node(subtree.children(subtree.root_id())[0]).mv().unwrap().mv().to_usi(),
            record.node(main).mv().unwrap().mv().to_usi()
        );
        assert_eq!(
            subtree.node(subtree.children(subtree.root_id())[1]).mv().unwrap().mv().to_usi(),
            record.node(variation).mv().unwrap().mv().to_usi()
        );
    }
}
