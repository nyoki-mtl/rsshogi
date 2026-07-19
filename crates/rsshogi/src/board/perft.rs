//! perft の参照データと計算ロジックを提供する。

use super::{move_list::Move32List, movegen::generate_legal_all_move32, position::Position};
use crate::types::Move;
use std::convert::TryFrom;
use std::env;
use std::time::{Duration, Instant};
use thiserror::Error;

const STARTPOS_SFEN: &str = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
const BENCH_OPENING_SFEN: &str =
    "lnsgkgsnl/1r7/p1ppp1bpp/1p3pp2/7P1/2P6/PP1PPPP1P/1B3S1R1/LNSGKG1NL b - 9";
const BENCH_TACTICAL_SFEN: &str =
    "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w RGgsn5p 1";

const STARTPOS_PERFT: &[(u8, u64)] =
    &[(1, 30), (2, 900), (3, 25_470), (4, 719_731), (5, 19_861_490), (6, 547_581_517)];

const BENCH_OPENING_PERFT: &[(u8, u64)] = &[(4, 1_307_221)];

const BENCH_TACTICAL_PERFT: &[(u8, u64)] = &[(3, 4_809_015)];

const REFERENCE_POSITIONS: &[(&str, &str)] = &[
    ("startpos", STARTPOS_SFEN),
    ("bench_opening", BENCH_OPENING_SFEN),
    ("bench_tactical", BENCH_TACTICAL_SFEN),
];

const PERFT_RESULTS: &[(&str, &[(u8, u64)])] = &[
    ("startpos", STARTPOS_PERFT),
    ("bench_opening", BENCH_OPENING_PERFT),
    ("bench_tactical", BENCH_TACTICAL_PERFT),
];

const CUSTOM_POSITION_NAME: &str = "custom";

/// perft クエリの結果を保持する構造体。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerftResult {
    pub name: &'static str,
    pub depth: u8,
    pub nodes: u64,
}

/// `perft_div` で得られる初手ごとのノード情報。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerftBranch {
    pub mv: Move,
    pub nodes: u64,
}

/// perft 計測時の統計情報。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PerftStats {
    pub name: &'static str,
    pub depth: u8,
    pub nodes: u64,
    pub elapsed: Duration,
    pub nps: u64,
}

/// 参照 perft データの解決時に発生するエラー。
#[derive(Debug, Error)]
pub enum PerftError {
    #[error("perft reference not found for sfen: {sfen}")]
    UnknownPosition { sfen: String },
    #[error("perft reference not found for {name} at depth {depth}")]
    UnknownDepth { name: &'static str, depth: u8 },
    #[error("perft_div depth must be at least 1 (got {depth})")]
    InvalidDivisionDepth { depth: u8 },
}

/// SFEN が参照テーブルに含まれる場合、対応する正規名を返す。
fn name_for_sfen(sfen: &str) -> Option<&'static str> {
    REFERENCE_POSITIONS
        .iter()
        .find_map(|(name, reference_sfen)| (*reference_sfen == sfen).then_some(*name))
}

/// 指定した局面名と深さに対応する期待ノード数を返す。
fn expected_nodes_for(name: &str, depth: u8) -> Option<u64> {
    PERFT_RESULTS
        .iter()
        .find(|(key, _)| *key == name)
        .and_then(|(_, entries)| entries.iter().find(|(d, _)| *d == depth).map(|(_, n)| *n))
}

/// 指定した [`Position`] が参照セットに含まれる場合、対応する正規名を返す。
#[must_use]
pub fn position_name(position: &Position) -> Option<&'static str> {
    name_for_sfen(&position.to_sfen(None))
}

/// 指定した [`Position`] を起点に perft 探索を実行する。
/// 参照データが存在する場合は debug assert でノード数の一致を確認する。
pub fn perft(position: &Position, depth: u8) -> Result<PerftResult, PerftError> {
    let mut working = position.clone();
    initialize_stack(&mut working);

    let mut ctx = PerftContext::new(depth);

    let nodes = compute_perft_with_context(&mut working, depth, &mut ctx);

    let name = position_name(position).unwrap_or(CUSTOM_POSITION_NAME);

    if let Some(expected) = expected_nodes_for(name, depth) {
        debug_assert!(
            expected == nodes,
            "perft mismatch for {name} depth {depth}: expected {expected}, got {nodes}"
        );
    }

    Ok(PerftResult { name, depth, nodes })
}

/// perft 実行の所要時間と NPS を計測する。
pub fn perft_bench(position: &Position, depth: u8) -> Result<PerftStats, PerftError> {
    let mut working = position.clone();
    initialize_stack(&mut working);

    let mut ctx = PerftContext::new(depth);

    let start = Instant::now();
    let nodes = compute_perft_with_context(&mut working, depth, &mut ctx);
    let elapsed = start.elapsed();

    let nps = if elapsed.as_nanos() == 0 {
        nodes
    } else {
        let scaled = u128::from(nodes) * 1_000_000_000u128;
        let per_sec = scaled / elapsed.as_nanos();
        u64::try_from(per_sec).unwrap_or(u64::MAX)
    };

    let name = position_name(position).unwrap_or(CUSTOM_POSITION_NAME);

    log::info!("perft {name} depth {depth}: {nodes} nodes in {elapsed:.3?} ({nps} nps)");

    Ok(PerftStats { name, depth, nodes, elapsed, nps })
}

/// 各初手に対するノード数を返す `perft_div`。
pub fn perft_div(position: &Position, depth: u8) -> Result<Vec<PerftBranch>, PerftError> {
    if depth == 0 {
        return Err(PerftError::InvalidDivisionDepth { depth });
    }

    let mut working = position.clone();
    initialize_stack(&mut working);

    let mut moves = Move32List::new();
    generate_legal_all_move32(&working, &mut moves);

    let mut branches = Vec::with_capacity(moves.len());
    let mut ctx = PerftContext::new(depth - 1);

    for &mv32 in moves.iter() {
        working.apply_move32(mv32);

        let nodes = compute_perft_with_context(&mut working, depth - 1, &mut ctx);
        // SAFETY: perft_div 内では apply_move32 の直後に必ず undo する。
        unsafe { working.undo_move32_unchecked(mv32) };

        branches.push(PerftBranch { mv: mv32.to_move(), nodes });
    }

    Ok(branches)
}

/// 再帰的に合法手を辿りノード数を数える perft 本体。
#[must_use]
pub fn compute_perft(position: &mut Position, depth: u8) -> u64 {
    let mut ctx = PerftContext::new(depth);
    compute_perft_with_context(position, depth, &mut ctx)
}

fn initialize_stack(position: &mut Position) {
    position.init_stack();
}

struct PerftContext {
    buffers: Vec<Move32List>,
    fast_depth2: bool,
}

impl PerftContext {
    fn new(max_depth: u8) -> Self {
        let count = usize::from(max_depth);
        let mut buffers = Vec::with_capacity(count);
        for _ in 0..count {
            buffers.push(Move32List::new());
        }
        let fast_depth2 = env::var("RSSHOGI_PERFT_FAST_DEPTH2").map_or(true, |value| value != "0");
        Self { buffers, fast_depth2 }
    }

    fn buffers_for(&mut self, depth: u8) -> &mut [Move32List] {
        let len = usize::from(depth);
        debug_assert!(len <= self.buffers.len());
        &mut self.buffers[..len]
    }
}

fn compute_perft_with_context(position: &mut Position, depth: u8, ctx: &mut PerftContext) -> u64 {
    let fast_depth2 = ctx.fast_depth2;
    let buffers = ctx.buffers_for(depth);
    compute_perft_recursive(position, depth, buffers, fast_depth2)
}

fn compute_perft_recursive(
    position: &mut Position,
    depth: u8,
    buffers: &mut [Move32List],
    fast_depth2: bool,
) -> u64 {
    if depth == 0 {
        return 1;
    }

    let (current, rest) =
        buffers.split_last_mut().expect("buffers length must match recursion depth");

    current.clear();
    generate_legal_all_move32(position, current);

    if depth == 1 {
        return current.len() as u64;
    }

    let mut nodes = 0_u64;

    if depth == 2 && fast_depth2 {
        let child = rest.last_mut().expect("buffers length must match recursion depth");
        for &mv32 in current.iter() {
            position.apply_move32(mv32);

            child.clear();
            generate_legal_all_move32(position, child);
            nodes += child.len() as u64;

            // SAFETY: perft 内では apply_move32 の直後に必ず undo する。
            unsafe { position.undo_move32_unchecked(mv32) };
        }

        return nodes;
    }

    for &mv32 in current.iter() {
        position.apply_move32(mv32);

        nodes += compute_perft_recursive(position, depth - 1, rest, fast_depth2);
        // SAFETY: perft 内では apply_move32 の直後に必ず undo する。
        unsafe { position.undo_move32_unchecked(mv32) };
    }

    nodes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_lookup_succeeds() {
        let pos = crate::board::position_from_sfen(STARTPOS_SFEN).expect("parse startpos");
        assert_eq!(position_name(&pos), Some("startpos"));
    }

    #[test]
    fn test_expected_nodes_are_available() {
        assert_eq!(expected_nodes_for("startpos", 3), Some(25_470));
        assert_eq!(expected_nodes_for("bench_tactical", 3), Some(4_809_015));
        assert!(expected_nodes_for("startpos", 7).is_none());
    }
}
