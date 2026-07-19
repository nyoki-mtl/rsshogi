use std::{collections::BTreeMap, convert::TryFrom, fmt::Write, time::Duration};

use serde::Deserialize;

use rsshogi::board::{self, MoveList, NonEvasionsAll, generate_moves, perft, position::Position};
use rsshogi::types::{Move, PieceType};

const MAX_BRANCHES: usize = 12;
const PERFT_DATA_JSON: &str = include_str!("test_data/perft_expectations.json");

#[test]
fn test_perft_depth_zero_returns_one() {
    let pos = board::hirate_position();

    let result = perft::perft(&pos, 0).expect("perft depth 0 succeeds");
    assert_eq!(result.nodes, 1, "depth 0 must return a single node");
}

#[test]
fn test_perft_startpos_depth_one_matches_reference() {
    let pos = board::hirate_position();

    assert_perft_matches_reference(&pos, 1, 30);
}

#[test]
fn test_perft_div_startpos_depth_two_matches_reference() {
    let pos = board::hirate_position();

    let branches = perft::perft_div(&pos, 2).expect("perft_div depth 2 succeeds");

    // 深さ1のノード数と一致するはず（初手の数）
    let depth_one_nodes = perft::perft(&pos, 1).expect("perft depth 1");
    let expected_len = usize::try_from(depth_one_nodes.nodes).expect("depth 1 count fits usize");
    assert_eq!(branches.len(), expected_len);

    let total: u64 = branches.iter().map(|b| b.nodes).sum();
    assert_eq!(total, 900, "startpos depth 2 should have 900 leaf nodes");

    // 既存の perft 実装で期待値を計算し、個別ノード数が一致することも検証
    let expected = expected_branches(&pos, 2);
    let mut actual = BTreeMap::new();
    for branch in branches {
        actual.insert(branch.mv.to_usi(), branch.nodes);
    }

    assert_eq!(actual, expected, "perft_div results must match computed expectations");
}

#[test]
fn test_perft_bench_reports_elapsed_and_nps() {
    let pos = board::hirate_position();

    let stats = perft::perft_bench(&pos, 1).expect("perft_bench depth 1 succeeds");
    assert_eq!(stats.name, "startpos");
    assert_eq!(stats.depth, 1);
    assert_eq!(stats.nodes, 30);
    assert!(stats.elapsed >= Duration::ZERO);
    assert!(stats.nps > 0);
}

#[test]
fn test_perft_startpos_depths_three_four_match_reference() {
    let pos = board::hirate_position();

    // startpos の参照 perft（depth, nodes）。
    for (depth, expected) in [(3_u8, 25_470_u64), (4, 719_731)] {
        assert_perft_matches_reference(&pos, depth, expected);
    }
}

#[test]
#[ignore = "slow perft depth 5"]
fn test_perft_startpos_depth_five_matches_reference() {
    let pos = board::hirate_position();

    let expected = expected_nodes_from_dataset_sfen("startpos", 5)
        .expect("startpos depth 5 expectation available");
    assert_perft_matches_reference(&pos, 5, expected);
}

#[test]
#[ignore = "slow perft depth 6"]
fn test_perft_startpos_depth_six_matches_reference() {
    let pos = board::hirate_position();

    let expected = expected_nodes_from_dataset_sfen("startpos", 6)
        .expect("startpos depth 6 expectation available");
    assert_perft_matches_reference(&pos, 6, expected);
}

#[test]
fn test_perft_branch_7g7f_5a4b_matches_reference() {
    let mut pos = board::hirate_position();
    apply_usi_sequence(&mut pos, &["7g7f", "5a4b"]);

    let result = perft::perft(&pos, 2).expect("perft depth 2 succeeds");
    assert_eq!(result.nodes, 989, "branch 7g7f 5a4b should match reference count");
}

#[test]
fn test_perft_reference_dataset_matches_expectations() {
    let cases = load_perft_expectations();
    let target_case = std::env::var("PERFT_CASE").ok();

    for (name, case) in cases {
        if let Some(ref target) = target_case
            && name != *target
        {
            continue;
        }
        let max_depth = case.depths.keys().copied().max().unwrap_or(0);
        if name == "startpos" || max_depth > 4 {
            // 重いエントリは専用の ignore テストでカバーされる。
            continue;
        }

        let pos = board::position_from_sfen(&case.sfen)
            .unwrap_or_else(|e| panic!("Failed to parse {name}: {e}"));

        for (&depth, &expected) in &case.depths {
            assert_perft_matches_reference(&pos, depth, expected);
        }
    }
}

fn expected_branches(pos: &Position, depth: u8) -> BTreeMap<String, u64> {
    assert!(depth > 0, "perft_div depth must be at least 1");

    let mut moves = MoveList::new();
    generate_moves::<NonEvasionsAll>(pos, &mut moves);

    let mut result = BTreeMap::new();
    for mv in moves.iter().copied() {
        if !pos.is_legal_move(mv) {
            continue;
        }
        let mut branch_pos = pos.clone();
        branch_pos.init_stack();
        branch_pos.apply_move32(pos.move32_from_move(mv));
        let nodes = perft::compute_perft(&mut branch_pos, depth - 1);
        result.insert(mv.to_usi(), nodes);
    }

    result
}

fn assert_perft_matches_reference(pos: &Position, depth: u8, expected: u64) {
    let result = perft::perft(pos, depth).expect("perft computation succeeded");
    if result.nodes != expected {
        let strict = strict_perft(pos, depth);
        report_perft_mismatch(pos, depth, expected, result.nodes, strict);
    }
    assert_eq!(result.nodes, expected, "perft mismatch at depth {depth}");
}

fn report_perft_mismatch(pos: &Position, depth: u8, expected: u64, actual: u64, strict: u64) -> ! {
    let mut detail = String::new();
    if depth > 0 {
        match perft::perft_div(pos, depth) {
            Ok(branches) => {
                let mut divergence: Option<(Move, String, u64, u64)> = None;
                for branch in &branches {
                    if let Some(next) = do_move_if_legal(pos, branch.mv) {
                        if depth > 1 {
                            let strict_child = strict_perft(&next, depth - 1);
                            if strict_child != branch.nodes {
                                divergence = Some((
                                    branch.mv,
                                    branch.mv.to_usi(),
                                    strict_child,
                                    branch.nodes,
                                ));
                                break;
                            }
                        }
                    } else {
                        divergence = Some((branch.mv, branch.mv.to_usi(), 0, branch.nodes));
                        break;
                    }
                }

                for (idx, branch) in branches.iter().enumerate() {
                    if idx >= MAX_BRANCHES {
                        break;
                    }
                    let _ = writeln!(detail, "{}: {}", branch.mv.to_usi(), branch.nodes);
                }
                if branches.len() > MAX_BRANCHES {
                    let remaining = branches.len() - MAX_BRANCHES;
                    let _ = writeln!(detail, "... ({remaining} more branches)");
                }

                if let Some((mv, mv_str, strict_child, actual_nodes)) = divergence {
                    let _ = writeln!(
                        detail,
                        "↳ divergence via {mv_str}: strict {strict_child} vs actual {actual_nodes}"
                    );

                    if let Some(next_pos) = do_move_if_legal(pos, mv)
                        && depth > 1
                        && let Ok(children) = perft::perft_div(&next_pos, depth - 1)
                    {
                        let _ = writeln!(detail, "  child nodes after {mv_str}:");
                        for child in children.iter().take(MAX_BRANCHES) {
                            let _ = writeln!(detail, "    {}: {}", child.mv.to_usi(), child.nodes);
                        }
                    }
                }
            }
            Err(err) => {
                let _ = writeln!(detail, "perft_div failed: {err}");
            }
        }
    }

    if let Some(path) = find_pawn_drop_mate(pos, depth) {
        let sequence: Vec<String> = path.iter().map(|mv| mv.to_usi()).collect();
        let _ = writeln!(detail, "↳ pawn drop mate sequence: {}", sequence.join(" "));
    }

    if let Some(path) = find_illegal_move(pos, depth) {
        let sequence: Vec<String> = path.iter().map(|mv| mv.to_usi()).collect();
        let _ = writeln!(detail, "↳ illegal move sequence: {}", sequence.join(" "));
    }

    if let Some(path) = find_allowed_drop_mate(pos, depth) {
        let sequence: Vec<String> = path.iter().map(|mv| mv.to_usi()).collect();
        let _ = writeln!(detail, "↳ allowed drop mate sequence: {}", sequence.join(" "));
    }

    panic!(
        "perft mismatch at depth {depth}: expected {expected}, actual {actual}, strict {strict}
Branch sample:
{detail}"
    );
}

#[derive(Debug, Deserialize)]
struct PerftCase {
    sfen: String,
    depths: BTreeMap<u8, u64>,
}

fn load_perft_expectations() -> BTreeMap<String, PerftCase> {
    serde_json::from_str(PERFT_DATA_JSON).expect("valid perft expectations json")
}

fn expected_nodes_from_dataset_sfen(name: &str, depth: u8) -> Option<u64> {
    let cases = load_perft_expectations();
    cases.get(name)?.depths.get(&depth).copied()
}

fn do_move_if_legal(pos: &Position, mv: Move) -> Option<Position> {
    let mut scratch = pos.clone();
    scratch.init_stack();

    let full = scratch.move32_from_move(mv);
    if !full.is_normal() || !scratch.is_legal_move32(full) {
        return None;
    }
    scratch.apply_move32(full);

    let us = pos.turn();
    let king_bb = scratch.bitboards().pieces_for(PieceType::KING, us);
    let king_sq = king_bb.lsb()?;

    let occupied = scratch.bitboards().occupied();
    let attackers =
        scratch.attackers_to(king_sq, occupied) & scratch.bitboards().color_pieces(us.flip());

    if attackers.is_empty() { Some(scratch) } else { None }
}

fn strict_perft(pos: &Position, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }

    let mut moves = MoveList::new();
    generate_moves::<NonEvasionsAll>(pos, &mut moves);

    let mut nodes = 0_u64;
    for mv in moves.iter().copied() {
        if let Some(next) = do_move_if_legal(pos, mv) {
            nodes += strict_perft(&next, depth - 1);
        }
    }

    nodes
}

fn find_pawn_drop_mate(pos: &Position, depth: u8) -> Option<Vec<Move>> {
    fn dfs(pos: &Position, depth: u8, path: &mut Vec<Move>) -> Option<Vec<Move>> {
        if depth == 0 {
            return None;
        }

        let mut moves = MoveList::new();
        generate_moves::<NonEvasionsAll>(pos, &mut moves);

        for mv in moves.iter().copied() {
            if let Some(next) = do_move_if_legal(pos, mv) {
                path.push(mv);

                if mv.is_drop() && mv.dropped_piece() == Some(PieceType::PAWN) {
                    let gives_check = !next.checkers().is_empty();
                    if gives_check {
                        let mut responses = MoveList::new();
                        generate_moves::<NonEvasionsAll>(&next, &mut responses);
                        let opponent_has_escape = responses
                            .iter()
                            .any(|response| do_move_if_legal(&next, *response).is_some());
                        if !opponent_has_escape {
                            return Some(path.clone());
                        }
                    }
                }

                if depth > 1
                    && let Some(found) = dfs(&next, depth - 1, path)
                {
                    return Some(found);
                }

                path.pop();
            }
        }

        None
    }

    dfs(pos, depth, &mut Vec::new())
}

fn find_illegal_move(pos: &Position, depth: u8) -> Option<Vec<Move>> {
    fn dfs(pos: &Position, depth: u8, path: &mut Vec<Move>) -> Option<Vec<Move>> {
        if depth == 0 {
            return None;
        }

        let mut moves = MoveList::new();
        generate_moves::<NonEvasionsAll>(pos, &mut moves);

        for mv in moves.iter().copied() {
            path.push(mv);
            if let Some(next) = do_move_if_legal(pos, mv) {
                if depth > 1
                    && let Some(found) = dfs(&next, depth - 1, path)
                {
                    path.pop();
                    return Some(found);
                }
                path.pop();
            } else {
                let result = Some(path.clone());
                path.pop();
                return result;
            }
        }

        None
    }

    dfs(pos, depth, &mut Vec::new())
}

fn find_allowed_drop_mate(pos: &Position, depth: u8) -> Option<Vec<Move>> {
    fn dfs(pos: &Position, depth: u8, path: &mut Vec<Move>) -> Option<Vec<Move>> {
        if depth == 0 {
            return None;
        }

        let mut moves = MoveList::new();
        generate_moves::<NonEvasionsAll>(pos, &mut moves);

        for mv in moves.iter().copied() {
            path.push(mv);
            let mut found = None;
            if let Some(next) = do_move_if_legal(pos, mv) {
                if mv.is_drop() && mv.dropped_piece() == Some(PieceType::PAWN) {
                    let mut responses = MoveList::new();
                    generate_moves::<NonEvasionsAll>(&next, &mut responses);
                    if responses.is_empty() {
                        found = Some(path.clone());
                    }
                }

                if found.is_none()
                    && depth > 1
                    && let Some(sub) = dfs(&next, depth - 1, path)
                {
                    found = Some(sub);
                }
            }

            if let Some(result) = found {
                path.pop();
                return Some(result);
            }
            path.pop();
        }

        None
    }

    dfs(pos, depth, &mut Vec::new())
}

fn apply_usi_sequence(pos: &mut Position, moves: &[&str]) {
    for mv_str in moves {
        let mut legal = MoveList::new();
        generate_moves::<NonEvasionsAll>(pos, &mut legal);
        let mv = legal
            .iter()
            .copied()
            .find(|m| m.to_usi() == *mv_str)
            .unwrap_or_else(|| panic!("move {mv_str} not found"));
        pos.apply_move32(pos.move32_from_move(mv));
    }
}
