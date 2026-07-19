use crate::board::movegen::{LegalAll, MoveListGen};
use crate::board::{PackedSfen, PackedSfenError, Position, SfenError};
use crate::records::record::{Record, RecordNodeId};
use crate::types::square::flip;
use crate::types::{Color, GameResult, Move, Move32, Piece, PieceType, Square};
use std::convert::TryFrom;

/// sbinpack の合法手オーダリングに使用するソートキー。
///
/// `(is_drop, is_capture, is_promotion, piece_priority, to_rank, to_file, from_rank, from_file, raw_move)`
pub(crate) type MoveOrderKey = (u8, u8, u8, u8, u8, u8, u8, u8, u16);

const FILE_PRIORITY: [u8; 9] = [7, 5, 3, 1, 0, 2, 4, 6, 8];

/// sbinpack v2 のバージョン番号。
pub const SBINPACK_VERSION: u8 = 2;
/// sbinpack v2 チャンクヘッダーのマジックバイト (`b"SBN2"`)。
pub const SBINPACK_MAGIC: [u8; 4] = *b"SBN2";

pub const SBINPACK_MAX_METADATA_BYTES: usize = 127;

/// ZigZagエンコード（i32 -> u32）。
#[must_use]
pub(crate) const fn zigzag_i32(value: i32) -> u32 {
    ((value << 1) ^ (value >> 31)) as u32
}

/// ZigZagデコード（u32 -> i32）。
#[must_use]
pub(crate) const fn unzigzag_u32(value: u32) -> i32 {
    ((value >> 1) as i32) ^ (-((value & 1) as i32))
}

/// ULEB128でu32をエンコードする。
pub(crate) fn encode_uleb128_u32(mut value: u32, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// `end` を上限として ULEB128 から `u32` をデコードする。
///
/// chunk 境界を越えて次のデータを読み込ませたくない streaming decoder 向け。
#[must_use]
pub fn decode_uleb128_u32_bounded(input: &[u8], offset: &mut usize, end: usize) -> Option<u32> {
    let end = end.min(input.len());
    let mut value = 0u32;
    let mut shift = 0u32;
    while *offset < end && shift < 32 {
        let byte = input[*offset];
        *offset += 1;
        value |= u32::from(byte & 0x7f) << shift;
        if (byte & 0x80) == 0 {
            return Some(value);
        }
        shift += 7;
    }
    None
}

/// sbinpack の評価値差分をエンコードする。
pub(crate) fn encode_score_delta(prev_eval: i32, current_eval: i32, out: &mut Vec<u8>) {
    let delta = current_eval - prev_eval;
    encode_uleb128_u32(zigzag_i32(delta), out);
}

/// `end` を上限として sbinpack の評価値差分をデコードする。
#[must_use]
pub fn decode_score_delta_bounded(input: &[u8], offset: &mut usize, end: usize) -> Option<i32> {
    decode_uleb128_u32_bounded(input, offset, end).map(unzigzag_u32)
}

/// sbinpack の読み書き時に発生するエラー。
#[derive(Debug)]
pub enum SbinpackError {
    /// マジックバイトが不正。
    InvalidMagic,
    /// データが途中で途切れている。
    Truncated,
    /// SFEN の解析に失敗。
    Sfen(SfenError),
    /// PackedSfen の解析に失敗。
    PackedSfen(PackedSfenError),
    /// 合法手リスト内のインデックスが範囲外。
    InvalidMoveIndex(u32),
    /// 指し手をオーダリングでエンコードできない。
    UnencodableMove(Move32),
    /// 指定手数目の評価値が存在しない。
    MissingEval(usize),
    /// metadata が v2 の上限を超えている。
    MetadataTooLarge { len: usize, max: usize },
}

/// sbinpack チェーンの先頭局面データ（PackedSfen + 評価値 + 最善手 + 手数/結果）。
#[derive(Debug, Clone, Copy)]
pub struct SbinpackStem {
    pub packed_sfen: PackedSfen,
    pub score: i16,
    pub best_move: Move,
    pub ply_result: u16,
}

/// sbinpack チェーン内の 1 手分のデータ（指し手 + 評価値）。
#[derive(Debug, Clone, Copy)]
pub struct SbinpackMove {
    pub mv: Move32,
    pub eval: i32,
}

/// sbinpack v2 の per-chain opaque metadata。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SbinpackMetadata {
    pub bytes: Vec<u8>,
}

impl SbinpackMetadata {
    pub fn new(bytes: Vec<u8>) -> Result<Self, SbinpackError> {
        if bytes.len() > SBINPACK_MAX_METADATA_BYTES {
            return Err(SbinpackError::MetadataTooLarge {
                len: bytes.len(),
                max: SBINPACK_MAX_METADATA_BYTES,
            });
        }
        Ok(Self { bytes })
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// sbinpack の 1 チェーン（先頭局面 + 後続手順）。
///
/// 1 つの初期局面から始まる連続した手順を表す。
#[derive(Debug, Clone)]
pub struct SbinpackChain {
    pub stem: SbinpackStem,
    pub metadata: SbinpackMetadata,
    pub moves: Vec<SbinpackMove>,
}

/// sbinpack ファイル全体（複数のチャンクとチェーンの集合）。
#[derive(Debug, Clone)]
pub struct SbinpackFile {
    pub version: u8,
    pub flags: u8,
    pub chains: Vec<SbinpackChain>,
}

/// sbinpack の逐次 decode で通知する format-native event。
///
/// `PositionBeforeMove` の `position` は callback の間だけ有効であり、decoder が
/// 次の指し手を適用する前の局面を指す。consumer 固有の学習 record への変換は
/// callback 側で行う。
pub enum SbinpackDecodeEvent<'a> {
    /// 新しい chunk の開始。
    ChunkStart,
    /// 新しい chain の開始。
    ChainStart {
        stem: SbinpackStem,
        metadata: &'a [u8],
        move_count: u16,
        result: Option<GameResult>,
    },
    /// 指し手適用前の局面と、その局面に対応する評価値。
    PositionBeforeMove {
        position: &'a Position,
        mv: Move32,
        eval: i32,
        result: Option<GameResult>,
        ply_index: u16,
    },
}

/// sbinpack の逐次 decode を consumer が制御する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbinpackDecodeControl {
    /// chain を通常どおり decode する。
    Continue,
    /// `ChainStart` で通知された chain の move payload を読み飛ばす。
    SkipChain,
}

/// sbinpack bytes を chain / position 単位で逐次 decode する。
///
/// 入力 container はdecoderが所有する。`Vec<u8>`を渡せばfile readerから受け取った
/// chunkをcopyせず保持でき、`&[u8]`を渡せばborrowed bytesをdecodeできる。move列を
/// `Vec<SbinpackMove>`へ展開せず、1つの`Position`だけをchain内で更新する。
pub struct SbinpackDecoder<B> {
    input: B,
    base_offset: usize,
    cursor: usize,
    chunk_end: usize,
    chain: Option<SbinpackDecodeChainState>,
    fused: bool,
}

struct SbinpackDecodeChainState {
    position: Position,
    moves_remaining: u16,
    previous_normalized_eval: i32,
    ply_index: u16,
    result: Option<GameResult>,
}

impl<B: AsRef<[u8]>> SbinpackDecoder<B> {
    /// 指定bytesのdecoderを作成する。
    pub fn new(input: B) -> Self {
        Self { input, base_offset: 0, cursor: 0, chunk_end: 0, chain: None, fused: false }
    }

    /// error位置の起点となるfile offsetを設定する。
    #[must_use]
    pub const fn with_base_offset(mut self, base_offset: usize) -> Self {
        self.base_offset = base_offset;
        self
    }

    /// 現在のabsolute byte offsetを返す。
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.base_offset.saturating_add(self.cursor)
    }

    /// 次のeventをdecodeし、callbackへ一時的に貸し出す。
    ///
    /// stream終端では`None`を返す。入力errorを1度返した後はfuseし、以後`None`を返す。
    /// callbackの戻り値は保持しないため、`Position`やmetadataのborrowはcallback外へ
    /// 持ち出せない。
    pub fn decode_next_with<F>(&mut self, visitor: F) -> Option<Result<(), SbinpackError>>
    where
        F: for<'a> FnOnce(SbinpackDecodeEvent<'a>),
    {
        self.decode_next_controlled_with(|event| {
            visitor(event);
            SbinpackDecodeControl::Continue
        })
    }

    /// 次の event を decode し、callback の制御値を反映する。
    ///
    /// `SkipChain` は `ChainStart` でのみ有効であり、`PackedSfen` と move index の
    /// 合法性を検証せず payload の可変長整数だけを読み飛ばす。結果種別や metadata で
    /// chain を除外する consumer 向けの高速経路である。
    pub fn decode_next_controlled_with<F>(
        &mut self,
        visitor: F,
    ) -> Option<Result<(), SbinpackError>>
    where
        F: for<'a> FnOnce(SbinpackDecodeEvent<'a>) -> SbinpackDecodeControl,
    {
        if self.fused {
            return None;
        }

        loop {
            if self.chain.is_some() {
                if self.chain.as_ref().is_some_and(|state| state.moves_remaining == 0) {
                    self.chain = None;
                    continue;
                }
                return Some(self.decode_next_move(visitor));
            }

            if self.cursor >= self.chunk_end {
                if self.cursor >= self.input.as_ref().len() {
                    return None;
                }
                return Some(self.decode_chunk_header(visitor));
            }

            return Some(self.decode_chain_header(visitor));
        }
    }

    fn decode_chunk_header<F>(&mut self, visitor: F) -> Result<(), SbinpackError>
    where
        F: for<'a> FnOnce(SbinpackDecodeEvent<'a>) -> SbinpackDecodeControl,
    {
        let input = self.input.as_ref();
        let Some(header_end) = self.cursor.checked_add(8) else {
            return self.fail(SbinpackError::Truncated);
        };
        if header_end > input.len() {
            return self.fail(SbinpackError::Truncated);
        }
        if input[self.cursor..self.cursor + 4] != SBINPACK_MAGIC {
            return self.fail(SbinpackError::InvalidMagic);
        }
        let chunk_size = u32::from_le_bytes([
            input[self.cursor + 4],
            input[self.cursor + 5],
            input[self.cursor + 6],
            input[self.cursor + 7],
        ]) as usize;
        self.cursor = header_end;
        let Some(chunk_end) = self.cursor.checked_add(chunk_size) else {
            return self.fail(SbinpackError::Truncated);
        };
        if chunk_end > input.len() {
            return self.fail(SbinpackError::Truncated);
        }
        self.chunk_end = chunk_end;
        let _ = visitor(SbinpackDecodeEvent::ChunkStart);
        Ok(())
    }

    fn decode_chain_header<F>(&mut self, visitor: F) -> Result<(), SbinpackError>
    where
        F: for<'a> FnOnce(SbinpackDecodeEvent<'a>) -> SbinpackDecodeControl,
    {
        let stem_offset = self.cursor;
        let Some(fixed_end) = stem_offset.checked_add(39) else {
            return self.fail(SbinpackError::Truncated);
        };
        if fixed_end.checked_add(2).is_none_or(|end| end > self.chunk_end) {
            return self.fail(SbinpackError::Truncated);
        }

        let input = self.input.as_ref();
        let mut packed_sfen = PackedSfen::default();
        packed_sfen.data.copy_from_slice(&input[stem_offset..stem_offset + 32]);
        let score = i16::from_le_bytes([input[stem_offset + 32], input[stem_offset + 33]]);
        let best_move =
            Move::from_raw(u16::from_le_bytes([input[stem_offset + 34], input[stem_offset + 35]]));
        let ply_result = u16::from_le_bytes([input[stem_offset + 36], input[stem_offset + 37]]);
        let metadata_len = usize::from(input[stem_offset + 38]);
        if metadata_len > SBINPACK_MAX_METADATA_BYTES {
            return self.fail(SbinpackError::MetadataTooLarge {
                len: metadata_len,
                max: SBINPACK_MAX_METADATA_BYTES,
            });
        }
        let metadata_start = stem_offset + 39;
        let Some(metadata_end) = metadata_start.checked_add(metadata_len) else {
            return self.fail(SbinpackError::Truncated);
        };
        let Some(count_end) = metadata_end.checked_add(2) else {
            return self.fail(SbinpackError::Truncated);
        };
        if count_end > self.chunk_end {
            return self.fail(SbinpackError::Truncated);
        }
        let move_count = u16::from_le_bytes([input[metadata_end], input[metadata_end + 1]]);
        self.cursor = count_end;

        let (result, stem_ply) = unpack_ply_result(ply_result);
        let stem = SbinpackStem { packed_sfen, score, best_move, ply_result };
        let metadata = &self.input.as_ref()[metadata_start..metadata_end];
        if visitor(SbinpackDecodeEvent::ChainStart { stem, metadata, move_count, result })
            == SbinpackDecodeControl::SkipChain
        {
            return self.skip_move_payload(move_count);
        }

        let mut position = Position::empty();
        if let Err(error) = position.set_packed_sfen(&packed_sfen, false, stem_ply) {
            return self.fail(SbinpackError::PackedSfen(error));
        }
        self.chain = Some(SbinpackDecodeChainState {
            position,
            moves_remaining: move_count,
            previous_normalized_eval: i32::from(score),
            ply_index: 0,
            result,
        });
        Ok(())
    }

    fn decode_next_move<F>(&mut self, visitor: F) -> Result<(), SbinpackError>
    where
        F: for<'a> FnOnce(SbinpackDecodeEvent<'a>) -> SbinpackDecodeControl,
    {
        let input = self.input.as_ref();
        let Some(move_index) = decode_uleb128_u32_bounded(input, &mut self.cursor, self.chunk_end)
        else {
            return self.fail(SbinpackError::Truncated);
        };
        let move_index = match u16::try_from(move_index) {
            Ok(index) => index,
            Err(_) => return self.fail(SbinpackError::InvalidMoveIndex(move_index)),
        };
        let Some(delta) = decode_score_delta_bounded(input, &mut self.cursor, self.chunk_end)
        else {
            return self.fail(SbinpackError::Truncated);
        };

        let state = self.chain.as_mut().expect("open sbinpack chain must have replay state");
        let Some(mv) = move_from_index(&state.position, move_index) else {
            return self.fail(SbinpackError::InvalidMoveIndex(u32::from(move_index)));
        };
        let normalized_eval = state.previous_normalized_eval + delta;
        let eval =
            if state.ply_index.is_multiple_of(2) { normalized_eval } else { -normalized_eval };
        let ply_index = state.ply_index;
        state.previous_normalized_eval = normalized_eval;
        state.ply_index += 1;
        state.moves_remaining -= 1;

        let _ = visitor(SbinpackDecodeEvent::PositionBeforeMove {
            position: &state.position,
            mv,
            eval,
            result: state.result,
            ply_index,
        });
        state.position.apply_move32(mv);
        Ok(())
    }

    fn skip_move_payload(&mut self, move_count: u16) -> Result<(), SbinpackError> {
        let input = self.input.as_ref();
        for _ in 0..move_count {
            if decode_uleb128_u32_bounded(input, &mut self.cursor, self.chunk_end).is_none() {
                return self.fail(SbinpackError::Truncated);
            }
            if decode_score_delta_bounded(input, &mut self.cursor, self.chunk_end).is_none() {
                return self.fail(SbinpackError::Truncated);
            }
        }
        Ok(())
    }

    fn fail(&mut self, error: SbinpackError) -> Result<(), SbinpackError> {
        self.fused = true;
        Err(error)
    }
}

#[derive(Debug, Clone, Copy)]
struct SbinpackInputMove {
    mv: Move,
    eval: Option<crate::types::Eval>,
}

fn chain_from_position(
    pos: &Position,
    moves: &[SbinpackInputMove],
    stem_score: i16,
    metadata: SbinpackMetadata,
    result: GameResult,
) -> Result<SbinpackChain, SbinpackError> {
    let packed_sfen = pos.to_packed_sfen();
    let mut current = pos.clone();

    let mut sbin_moves = Vec::with_capacity(moves.len());
    for (index, mv_record) in moves.iter().enumerate() {
        let eval = mv_record.eval.ok_or(SbinpackError::MissingEval(index))?;
        let mv16 = mv_record.mv;
        let mv = current.move32_from_move(mv16);
        sbin_moves.push(SbinpackMove { mv, eval: eval.to_i32() });
        current.apply_move(mv16);
    }

    let best_move = moves.first().map(|entry| entry.mv).unwrap_or(Move::MOVE_NONE);
    let ply = pos.game_ply();
    let ply_result = pack_ply_result(result, ply);

    Ok(SbinpackChain {
        stem: SbinpackStem { packed_sfen, score: stem_score, best_move, ply_result },
        metadata,
        moves: sbin_moves,
    })
}

fn position_at_node(record: &Record, node_id: RecordNodeId) -> Result<Position, SbinpackError> {
    crate::records::formats::traversal::position_at(record, node_id).map_err(|e| match e {
        crate::records::formats::traversal::TraversalError::InvalidInitPosition(sfen_err) => {
            SbinpackError::Sfen(sfen_err)
        }
        crate::records::formats::traversal::TraversalError::IllegalMove { .. } => {
            SbinpackError::Sfen(SfenError::InvalidMove(e.to_string()))
        }
    })
}

fn line_moves_from(record: &Record, start: RecordNodeId) -> Vec<SbinpackInputMove> {
    let mut moves = Vec::new();
    let mut current = Some(start);
    while let Some(id) = current {
        let node = record.node(id);
        if let Some(mv) = node.mv() {
            moves.push(SbinpackInputMove { mv: mv.mv(), eval: node.eval() });
        }
        current = record.children(id).first().copied();
    }
    moves
}

fn main_line_moves_from(record: &Record) -> Vec<SbinpackInputMove> {
    record
        .main_line_ids()
        .into_iter()
        .filter_map(|node_id| {
            let node = record.node(node_id);
            node.mv().map(|mv| SbinpackInputMove { mv: mv.mv(), eval: node.eval() })
        })
        .collect()
}

/// `Record` から `SbinpackChain` を作成する。
pub fn chain_from_record(record: &Record, stem_score: i16) -> Result<SbinpackChain, SbinpackError> {
    chain_from_record_with_metadata(record, stem_score, SbinpackMetadata::default())
}

/// `Record` と metadata から `SbinpackChain` を作成する。
pub fn chain_from_record_with_metadata(
    record: &Record,
    stem_score: i16,
    metadata: SbinpackMetadata,
) -> Result<SbinpackChain, SbinpackError> {
    let mut pos = Position::empty();
    pos.set_sfen(record.init_position_sfen()).map_err(SbinpackError::Sfen)?;
    let moves = main_line_moves_from(record);
    chain_from_position(&pos, &moves, stem_score, metadata, record.result())
}

/// `Record` から `SbinpackChain` を複数作成する。
pub fn chains_from_record(
    record: &Record,
    stem_score: i16,
    include_main: bool,
    include_variations: bool,
) -> Result<Vec<SbinpackChain>, SbinpackError> {
    chains_from_record_with_metadata(
        record,
        stem_score,
        SbinpackMetadata::default(),
        include_main,
        include_variations,
    )
}

/// `Record` と metadata から `SbinpackChain` を複数作成する。
pub fn chains_from_record_with_metadata(
    record: &Record,
    stem_score: i16,
    metadata: SbinpackMetadata,
    include_main: bool,
    include_variations: bool,
) -> Result<Vec<SbinpackChain>, SbinpackError> {
    let mut chains = Vec::new();

    if include_main {
        let mut pos = Position::empty();
        pos.set_sfen(record.init_position_sfen()).map_err(SbinpackError::Sfen)?;
        let moves = main_line_moves_from(record);
        chains.push(chain_from_position(
            &pos,
            &moves,
            stem_score,
            metadata.clone(),
            record.result(),
        )?);
    }

    if include_variations {
        let mut stack = vec![record.root_id()];
        while let Some(node_id) = stack.pop() {
            let children = record.children(node_id);
            if children.len() > 1 {
                for &child in &children[1..] {
                    let parent_pos = position_at_node(record, node_id)?;
                    let moves = line_moves_from(record, child);
                    chains.push(chain_from_position(
                        &parent_pos,
                        &moves,
                        stem_score,
                        metadata.clone(),
                        record.result(),
                    )?);
                }
            }
            for &child in children {
                stack.push(child);
            }
        }
    }

    Ok(chains)
}

/// sbinpack v2 の `ChunkHeader + Chain*` を構築する（単一チャンク）。
pub fn serialize_file(chains: &[SbinpackChain]) -> Result<Vec<u8>, SbinpackError> {
    let mut payload = Vec::new();
    for chain in chains {
        serialize_chain(chain, &mut payload)?;
    }

    let mut out = Vec::new();
    out.extend_from_slice(&SBINPACK_MAGIC);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

/// sbinpack v2 の `ChunkHeader + Chain*` を読み込む。
pub fn deserialize_file(input: &[u8]) -> Result<SbinpackFile, SbinpackError> {
    let mut offset = 0;
    let mut chains = Vec::new();
    while offset < input.len() {
        if offset + 8 > input.len() {
            return Err(SbinpackError::Truncated);
        }
        if input[offset..offset + 4] != SBINPACK_MAGIC {
            return Err(SbinpackError::InvalidMagic);
        }
        let chunk_size = u32::from_le_bytes([
            input[offset + 4],
            input[offset + 5],
            input[offset + 6],
            input[offset + 7],
        ]) as usize;
        offset += 8;
        if offset + chunk_size > input.len() {
            return Err(SbinpackError::Truncated);
        }
        let chunk_end = offset + chunk_size;
        while offset < chunk_end {
            let (chain, next) = deserialize_chain(input, offset, chunk_end)?;
            chains.push(chain);
            offset = next;
        }
    }

    Ok(SbinpackFile { version: SBINPACK_VERSION, flags: 0, chains })
}

/// result (6bit) + ply (10bit) をパックする。
#[must_use]
pub const fn pack_ply_result(result: GameResult, ply: u16) -> u16 {
    let result_bits = (result as u16) & 0x3f;
    let ply_bits = if ply > 1023 { 1023 } else { ply };
    (result_bits << 10) | ply_bits
}

/// result (6bit) + ply (10bit) をアンパックする。
#[must_use]
pub fn unpack_ply_result(value: u16) -> (Option<GameResult>, u16) {
    let result = game_result_from_u8(((value >> 10) & 0x3f) as u8);
    let ply = value & 0x03ff;
    (result, ply)
}

#[must_use]
pub fn game_result_from_u8(value: u8) -> Option<GameResult> {
    match value {
        0 => Some(GameResult::BlackWin),
        1 => Some(GameResult::WhiteWin),
        2 => Some(GameResult::DrawByRepetition),
        3 => Some(GameResult::Error),
        4 => Some(GameResult::BlackWinByDeclaration),
        5 => Some(GameResult::WhiteWinByDeclaration),
        6 => Some(GameResult::DrawByMaxPlies),
        7 => Some(GameResult::Invalid),
        8 => Some(GameResult::BlackWinByForfeit),
        9 => Some(GameResult::WhiteWinByForfeit),
        10 => Some(GameResult::DrawByImpasse),
        11 => Some(GameResult::Paused),
        12 => Some(GameResult::BlackWinByIllegalMove),
        13 => Some(GameResult::WhiteWinByIllegalMove),
        16 => Some(GameResult::BlackWinByTimeout),
        17 => Some(GameResult::WhiteWinByTimeout),
        _ => None,
    }
}

fn normalize_eval_for_delta(eval: i32, ply_index: usize) -> i32 {
    if ply_index.is_multiple_of(2) { eval } else { -eval }
}

fn denormalize_eval_from_delta(eval: i32, ply_index: usize) -> i32 {
    // The ply-parity normalization is an involution, so the same transform restores evals.
    normalize_eval_for_delta(eval, ply_index)
}

fn encode_metadata(metadata: &SbinpackMetadata, out: &mut Vec<u8>) -> Result<(), SbinpackError> {
    if metadata.bytes.len() > SBINPACK_MAX_METADATA_BYTES {
        return Err(SbinpackError::MetadataTooLarge {
            len: metadata.bytes.len(),
            max: SBINPACK_MAX_METADATA_BYTES,
        });
    }
    out.push(metadata.bytes.len() as u8);
    out.extend_from_slice(&metadata.bytes);
    Ok(())
}

fn decode_metadata(
    input: &[u8],
    offset: &mut usize,
    chunk_end: usize,
) -> Result<SbinpackMetadata, SbinpackError> {
    if *offset >= chunk_end {
        return Err(SbinpackError::Truncated);
    }
    let metadata_len = usize::from(input[*offset]);
    *offset += 1;
    if metadata_len > SBINPACK_MAX_METADATA_BYTES {
        return Err(SbinpackError::MetadataTooLarge {
            len: metadata_len,
            max: SBINPACK_MAX_METADATA_BYTES,
        });
    }
    let metadata_end = (*offset).checked_add(metadata_len).ok_or(SbinpackError::Truncated)?;
    if metadata_end.checked_add(2).ok_or(SbinpackError::Truncated)? > chunk_end {
        return Err(SbinpackError::Truncated);
    }

    let metadata = SbinpackMetadata { bytes: input[*offset..metadata_end].to_vec() };
    *offset = metadata_end;
    Ok(metadata)
}

fn serialize_chain(chain: &SbinpackChain, out: &mut Vec<u8>) -> Result<(), SbinpackError> {
    out.extend_from_slice(&chain.stem.packed_sfen.data);
    out.extend_from_slice(&chain.stem.score.to_le_bytes());
    out.extend_from_slice(&chain.stem.best_move.raw().to_le_bytes());
    out.extend_from_slice(&chain.stem.ply_result.to_le_bytes());
    encode_metadata(&chain.metadata, out)?;

    debug_assert!(
        u16::try_from(chain.moves.len()).is_ok(),
        "sbinpack chain move count exceeds u16::MAX; extra moves are not encoded"
    );
    let count = u16::try_from(chain.moves.len()).unwrap_or(u16::MAX);
    out.extend_from_slice(&count.to_le_bytes());

    let mut pos = Position::empty();
    let _ = pos.set_packed_sfen(&chain.stem.packed_sfen, false, 0);
    let mut prev_norm = i32::from(chain.stem.score);
    for (ply_index, entry) in chain.moves.iter().take(count as usize).enumerate() {
        let move_index =
            encoded_move_index(&pos, entry.mv).ok_or(SbinpackError::UnencodableMove(entry.mv))?;
        encode_uleb128_u32(u32::from(move_index), out);
        let current_norm = normalize_eval_for_delta(entry.eval, ply_index);
        encode_score_delta(prev_norm, current_norm, out);
        prev_norm = current_norm;
        pos.apply_move32(entry.mv);
    }
    Ok(())
}

fn deserialize_chain(
    input: &[u8],
    offset: usize,
    chunk_end: usize,
) -> Result<(SbinpackChain, usize), SbinpackError> {
    let mut cursor = offset;
    if cursor + 38 > chunk_end {
        return Err(SbinpackError::Truncated);
    }
    let mut packed = PackedSfen::default();
    packed.data.copy_from_slice(&input[cursor..cursor + 32]);
    cursor += 32;
    let score = i16::from_le_bytes([input[cursor], input[cursor + 1]]);
    cursor += 2;
    let best_move = Move::from_raw(u16::from_le_bytes([input[cursor], input[cursor + 1]]));
    cursor += 2;
    let ply_result = u16::from_le_bytes([input[cursor], input[cursor + 1]]);
    cursor += 2;

    let metadata = decode_metadata(input, &mut cursor, chunk_end)?;
    if cursor + 2 > chunk_end {
        return Err(SbinpackError::Truncated);
    }

    let count = u16::from_le_bytes([input[cursor], input[cursor + 1]]) as usize;
    cursor += 2;

    let mut pos = Position::empty();
    pos.set_packed_sfen(&packed, false, 0).map_err(SbinpackError::PackedSfen)?;

    let mut moves = Vec::with_capacity(count);
    let mut prev_norm = i32::from(score);
    for ply_index in 0..count {
        let Some(move_index) = decode_uleb128_u32_bounded(input, &mut cursor, chunk_end) else {
            return Err(SbinpackError::Truncated);
        };
        let Some(mv) = move_from_index(
            &pos,
            u16::try_from(move_index).map_err(|_| SbinpackError::InvalidMoveIndex(move_index))?,
        ) else {
            return Err(SbinpackError::InvalidMoveIndex(move_index));
        };
        let Some(delta) = decode_score_delta_bounded(input, &mut cursor, chunk_end) else {
            return Err(SbinpackError::Truncated);
        };
        let current_norm = prev_norm + delta;
        let eval = denormalize_eval_from_delta(current_norm, ply_index);
        moves.push(SbinpackMove { mv, eval });
        prev_norm = current_norm;
        pos.apply_move32(mv);
    }

    Ok((
        SbinpackChain {
            stem: SbinpackStem { packed_sfen: packed, score, best_move, ply_result },
            metadata,
            moves,
        },
        cursor,
    ))
}

/// sbinpack v1.0.0 の合法手オーダリングで並べた手一覧を返す。
#[must_use]
pub fn ordered_legal_moves(pos: &Position) -> Vec<Move32> {
    let legal = MoveListGen::<LegalAll>::new(pos);
    let mut moves: Vec<Move32> = legal
        .iter()
        .copied()
        .filter_map(|mv| {
            let full = pos.move32_from_move(mv);
            full.is_normal().then_some(full)
        })
        .collect();
    moves.sort_by_key(|candidate| move_order_key(pos, *candidate));
    moves
}

/// sbinpack v1.0.0 の合法手オーダリングに従ったソートキーを返す。
#[must_use]
pub(crate) fn move_order_key(pos: &Position, mv: Move32) -> MoveOrderKey {
    let side = pos.turn();
    let is_drop = mv.is_drop();
    let is_capture = if is_drop {
        false
    } else {
        let to_piece = pos.piece_on(mv.to_sq());
        to_piece != Piece::NONE && to_piece.color() != side
    };
    let is_promotion = mv.is_promotion();

    let piece_type = if is_drop {
        mv.dropped_piece().unwrap_or(PieceType::NONE)
    } else {
        pos.moved_piece_before(mv).piece_type()
    };

    let to_sq = perspective_square(mv.to_sq(), side);
    let (to_file, to_rank) = square_to_priority(to_sq);
    let (from_file, from_rank) = if is_drop {
        (9, 9)
    } else {
        let from_sq = perspective_square(mv.from_sq(), side);
        square_to_priority(from_sq)
    };

    (
        u8::from(is_drop),
        u8::from(is_capture),
        u8::from(is_promotion),
        piece_priority(piece_type),
        to_rank,
        to_file,
        from_rank,
        from_file,
        mv.to_move().raw(),
    )
}

/// sbinpack v1.0.0 のオーダリング上のインデックスを返す。
#[must_use]
pub fn encoded_move_index(pos: &Position, mv: Move32) -> Option<u16> {
    let moves = ordered_legal_moves(pos);
    moves.iter().position(|candidate| *candidate == mv).and_then(|idx| u16::try_from(idx).ok())
}

/// sbinpack v1.0.0 のオーダリング上のインデックスから指し手を復元する。
#[must_use]
pub fn move_from_index(pos: &Position, index: u16) -> Option<Move32> {
    let moves = ordered_legal_moves(pos);
    moves.get(index as usize).copied()
}

fn perspective_square(sq: Square, side: Color) -> Square {
    if side == Color::BLACK { sq } else { flip(sq) }
}

fn square_to_priority(sq: Square) -> (u8, u8) {
    let file = sq.file().raw();
    let rank = sq.rank().raw();
    let file_priority = if (0..=8).contains(&file) { FILE_PRIORITY[file as usize] } else { 9 };
    let rank_priority = if (0..=8).contains(&rank) { rank as u8 } else { 9 };
    (file_priority, rank_priority)
}

fn piece_priority(piece_type: PieceType) -> u8 {
    match piece_type {
        PieceType::PAWN => 0,
        PieceType::LANCE => 1,
        PieceType::KNIGHT => 2,
        PieceType::SILVER => 3,
        PieceType::GOLD => 4,
        PieceType::BISHOP => 5,
        PieceType::ROOK => 6,
        PieceType::KING => 7,
        PieceType::PRO_PAWN => 8,
        PieceType::PRO_LANCE => 9,
        PieceType::PRO_KNIGHT => 10,
        PieceType::PRO_SILVER => 11,
        PieceType::HORSE => 12,
        PieceType::DRAGON => 13,
        _ => 14,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::hirate_position;
    use crate::records::record::{EngineInfo, MoveEntry, RecordAnnotation};
    use crate::types::{Eval, Move};

    fn annotation_with_eval(eval: i32) -> RecordAnnotation {
        RecordAnnotation::new()
            .with_engine_info(Some(EngineInfo::new().with_eval(Some(Eval::from_i32(eval)))))
    }

    fn sample_chain(metadata: SbinpackMetadata) -> SbinpackChain {
        let mut pos = hirate_position();
        let packed_sfen = pos.to_packed_sfen();
        let mv1_16 = Move::from_usi("7g7f").expect("valid move");
        let mv1 = pos.move32_from_move(mv1_16);
        pos.apply_move(mv1_16);
        let mv2_16 = Move::from_usi("3c3d").expect("valid move");
        let mv2 = pos.move32_from_move(mv2_16);
        pos.apply_move(mv2_16);
        let mv3_16 = Move::from_usi("2g2f").expect("valid move");
        let mv3 = pos.move32_from_move(mv3_16);

        SbinpackChain {
            stem: SbinpackStem {
                packed_sfen,
                score: 100,
                best_move: mv1_16,
                ply_result: pack_ply_result(GameResult::BlackWin, 1),
            },
            metadata,
            moves: vec![
                SbinpackMove { mv: mv1, eval: 110 },
                SbinpackMove { mv: mv2, eval: -115 },
                SbinpackMove { mv: mv3, eval: 125 },
            ],
        }
    }

    #[test]
    fn public_delta_decoders_respect_input_bounds() {
        let mut offset = 0;
        assert_eq!(decode_uleb128_u32_bounded(&[0xac, 0x02], &mut offset, usize::MAX), Some(300));
        assert_eq!(offset, 2);

        let mut offset = 0;
        assert_eq!(decode_score_delta_bounded(&[9], &mut offset, 1), Some(-5));
        assert_eq!(offset, 1);

        let mut offset = 0;
        assert_eq!(decode_uleb128_u32_bounded(&[0x80], &mut offset, 1), None);
        assert_eq!(offset, 1);
    }

    #[test]
    fn score_delta_normalizes_side_to_move_values() {
        let chain = sample_chain(SbinpackMetadata::default());
        let mut bytes = Vec::new();
        serialize_chain(&chain, &mut bytes).expect("serialize");

        let mut offset = 38;
        assert_eq!(bytes[offset], 0, "empty metadata ext");
        offset += 1;
        assert_eq!(u16::from_le_bytes([bytes[offset], bytes[offset + 1]]), 3);
        offset += 2;

        let mut score_bytes = Vec::new();
        for _ in 0..3 {
            let _ =
                decode_uleb128_u32_bounded(&bytes, &mut offset, bytes.len()).expect("move index");
            score_bytes.push(bytes[offset]);
            let _ =
                decode_score_delta_bounded(&bytes, &mut offset, bytes.len()).expect("score delta");
        }

        assert_eq!(score_bytes, vec![20, 10, 20]);
    }

    #[test]
    fn sbinpack_v2_roundtrip_preserves_metadata_and_scores() {
        let metadata = SbinpackMetadata::new(b"user-defined-metadata".to_vec()).expect("metadata");
        let chain = sample_chain(metadata.clone());

        let bytes = serialize_file(std::slice::from_ref(&chain)).expect("serialize file");
        assert_eq!(&bytes[..4], b"SBN2");

        let file = deserialize_file(&bytes).expect("deserialize file");
        assert_eq!(file.version, 2);
        assert_eq!(file.chains.len(), 1);
        let decoded = &file.chains[0];
        assert_eq!(decoded.metadata, metadata);
        assert_eq!(
            decoded.moves.iter().map(|entry| entry.eval).collect::<Vec<_>>(),
            vec![110, -115, 125]
        );
    }

    #[test]
    fn streaming_decoder_yields_chain_and_positions_without_move_vec() {
        let metadata = SbinpackMetadata::new(b"stream".to_vec()).expect("metadata");
        let chain = sample_chain(metadata.clone());
        let bytes = serialize_file(std::slice::from_ref(&chain)).expect("serialize file");
        let mut decoder = SbinpackDecoder::new(bytes.as_slice());
        let mut chunk_count = 0;
        let mut chain_headers = Vec::new();
        let mut positions = Vec::new();

        while let Some(result) = decoder.decode_next_with(|event| match event {
            SbinpackDecodeEvent::ChunkStart => chunk_count += 1,
            SbinpackDecodeEvent::ChainStart { stem, metadata, move_count, result } => {
                chain_headers.push((stem.score, metadata.to_vec(), move_count, result))
            }
            SbinpackDecodeEvent::PositionBeforeMove { position, mv, eval, result, ply_index } => {
                positions.push((
                    position.to_packed_sfen(),
                    position.game_ply(),
                    mv,
                    eval,
                    result,
                    ply_index,
                ))
            }
        }) {
            result.expect("streaming decode");
        }

        assert_eq!(chunk_count, 1);
        assert_eq!(chain_headers, vec![(100, metadata.bytes, 3, Some(GameResult::BlackWin))]);
        assert_eq!(positions.len(), 3);
        assert_eq!(positions.iter().map(|entry| entry.3).collect::<Vec<_>>(), vec![110, -115, 125]);
        assert_eq!(positions.iter().map(|entry| entry.1).collect::<Vec<_>>(), vec![1, 2, 3]);
        assert_eq!(positions.iter().map(|entry| entry.5).collect::<Vec<_>>(), vec![0, 1, 2]);

        let mut expected = hirate_position();
        for (packed, _, mv, _, _, _) in positions {
            assert_eq!(packed, expected.to_packed_sfen());
            expected.apply_move32(mv);
        }
    }

    #[test]
    fn streaming_decoder_owns_input_and_reports_absolute_error_offset() {
        let chain = sample_chain(SbinpackMetadata::default());
        let mut bytes = serialize_file(std::slice::from_ref(&chain)).expect("serialize file");
        bytes.pop();
        let expected_offset = 4_096 + 8;
        let mut decoder = SbinpackDecoder::new(bytes).with_base_offset(4_096);

        let error = loop {
            match decoder.decode_next_with(|_| {}) {
                Some(Ok(())) => {}
                Some(Err(error)) => break error,
                None => panic!("truncated input must fail"),
            }
        };

        assert!(matches!(error, SbinpackError::Truncated));
        assert_eq!(decoder.offset(), expected_offset);
        assert!(decoder.decode_next_with(|_| {}).is_none(), "decoder must fuse after error");
    }

    #[test]
    fn streaming_decoder_can_skip_chain_payload() {
        let chain = sample_chain(SbinpackMetadata::default());
        let bytes = serialize_file(std::slice::from_ref(&chain)).expect("serialize file");
        let mut decoder = SbinpackDecoder::new(bytes);
        let mut chain_count = 0;
        let mut position_count = 0;

        while let Some(result) = decoder.decode_next_controlled_with(|event| match event {
            SbinpackDecodeEvent::ChainStart { .. } => {
                chain_count += 1;
                SbinpackDecodeControl::SkipChain
            }
            SbinpackDecodeEvent::PositionBeforeMove { .. } => {
                position_count += 1;
                SbinpackDecodeControl::Continue
            }
            SbinpackDecodeEvent::ChunkStart => SbinpackDecodeControl::Continue,
        }) {
            result.expect("streaming skip");
        }

        assert_eq!(chain_count, 1);
        assert_eq!(position_count, 0);
    }

    #[test]
    fn chains_from_record_copies_metadata_to_variations() {
        let metadata = SbinpackMetadata::new(b"copied".to_vec()).expect("metadata");
        let mut record = Record::new(hirate_position().to_sfen(None)).expect("record");
        let first = record
            .append_move_with_annotation(
                record.root_id(),
                MoveEntry::new(Move::from_usi("7g7f").expect("valid move")),
                annotation_with_eval(10),
            )
            .unwrap();
        record
            .append_move_with_annotation(
                first,
                MoveEntry::new(Move::from_usi("3c3d").expect("valid move")),
                annotation_with_eval(-20),
            )
            .unwrap();
        record
            .append_move_with_annotation(
                first,
                MoveEntry::new(Move::from_usi("8c8d").expect("valid move")),
                annotation_with_eval(-30),
            )
            .unwrap();

        let chains =
            chains_from_record_with_metadata(&record, 0, metadata.clone(), true, true).unwrap();

        assert_eq!(chains.len(), 2);
        assert!(chains.iter().all(|chain| chain.metadata == metadata));
    }

    #[test]
    fn deserialize_rejects_move_text_that_crosses_chunk_end() {
        let chain = sample_chain(SbinpackMetadata::default());
        let mut bytes = serialize_file(std::slice::from_ref(&chain)).expect("serialize file");
        let chunk_size = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        bytes[4..8].copy_from_slice(&(chunk_size - 1).to_le_bytes());

        let err = deserialize_file(&bytes).expect_err("truncated move text");
        assert!(matches!(err, SbinpackError::Truncated));
    }

    #[test]
    fn metadata_rejects_too_large_payload() {
        let err = SbinpackMetadata::new(vec![0; SBINPACK_MAX_METADATA_BYTES + 1])
            .expect_err("too large metadata");
        assert!(matches!(
            err,
            SbinpackError::MetadataTooLarge {
                len,
                max: SBINPACK_MAX_METADATA_BYTES
            } if len == SBINPACK_MAX_METADATA_BYTES + 1
        ));
    }

    #[test]
    fn deserialize_rejects_too_large_metadata_payload() {
        let chain = sample_chain(SbinpackMetadata::default());
        let mut bytes = serialize_file(std::slice::from_ref(&chain)).expect("serialize file");
        let metadata_offset = 8 + 38;
        bytes[metadata_offset] = 0x80;

        let err = deserialize_file(&bytes).expect_err("too large metadata");
        assert!(matches!(
            err,
            SbinpackError::MetadataTooLarge {
                len,
                max: SBINPACK_MAX_METADATA_BYTES
            } if len == SBINPACK_MAX_METADATA_BYTES + 1
        ));
    }
}
