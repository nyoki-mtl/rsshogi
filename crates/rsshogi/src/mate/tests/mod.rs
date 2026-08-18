use super::{
    CheckClass, MateContext, MoveDelta, attackers_with_delta, classify_check, mate_context,
    solve_mate_in_one, solve_mate_in_one_in_place,
};
use crate::board::attack_tables::KING_ATTACKS;
use crate::board::movegen::generate_checks_all_move32;
use crate::board::{Move32List, Position, generate_legal_all_move32};
use crate::types::{Bitboard, Color, Move32};

/// 総当たり参照実装。候補列の順序を保ったまま各候補を適用して確定する。
///
/// 静的フィルタを一切持たないため、フィルタ健全性の exact oracle として使う。
fn naive_mate_in_one(position: &Position) -> Option<Move32> {
    let mut candidates = Move32List::new();
    let in_check = position.is_in_check();
    if in_check {
        generate_legal_all_move32(position, &mut candidates);
    } else {
        generate_checks_all_move32(position, &mut candidates);
        candidates.retain_unordered(|mv| position.is_legal_move32(mv));
    }

    let mut next = position.clone();
    next.init_stack();
    for &mv in candidates.iter() {
        if in_check && !position.gives_check_move32(mv) {
            continue;
        }
        next.apply_move32_with_gives_check(mv, true);
        if next.is_mated() {
            return Some(mv);
        }
        next.undo_move32(mv).expect("oracle must undo the move it just applied");
    }
    None
}

/// 候補 1 つ分の `attackers_with_delta` を、実際に適用した局面の利きと突き合わせる。
///
/// 検証対象の升は玉隣接 8 升に王手駒の升と王手線上の升を加えたもの。stale bitboard 由来の
/// 「存在しない駒を攻撃駒として数える」誤りはこの層で検出する。
fn verify_attackers_with_delta(position: &Position, ctx: &MateContext, mv: Move32) {
    let delta = MoveDelta::from_move32(mv, ctx.occupied);
    let mut next = position.clone();
    next.init_stack();
    next.apply_move32_with_gives_check(mv, position.gives_check_move32(mv));
    assert_eq!(
        next.bitboards().occupied(),
        delta.occupied_after,
        "occupied_after mismatch: sfen={} usi={}",
        position.to_sfen(None),
        mv.to_usi(),
    );

    let mut targets = KING_ATTACKS[ctx.their_king];
    targets.set(ctx.their_king);
    match classify_check(position, ctx, mv) {
        Some(CheckClass::Direct(checker) | CheckClass::Discovered(checker)) => {
            targets.set(checker);
            targets |= Bitboard::between(checker, ctx.their_king);
        }
        Some(CheckClass::Double) | None => {}
    }

    // 逃げ道の再計算（S3）は受け方玉を除いた占有で問い合わせるため、その variant も
    // 同じ突き合わせで検証する。
    let delta_without_king = MoveDelta {
        occupied_after: delta.occupied_after.and_not(Bitboard::from_square(ctx.their_king)),
        ..delta
    };
    while let Some(sq) = targets.pop_lsb() {
        for color in [Color::BLACK, Color::WHITE] {
            for probe in [&delta, &delta_without_king] {
                let expected = next.attackers_to_color(color, sq, probe.occupied_after);
                let actual = attackers_with_delta(position, color, sq, probe);
                assert_eq!(
                    actual,
                    expected,
                    "attackers_with_delta mismatch: sfen={} usi={} sq={} color={}",
                    position.to_sfen(None),
                    mv.to_usi(),
                    sq.to_index(),
                    color.to_index(),
                );
            }
        }
    }
}

/// `solve_mate_in_one_in_place` の等価性と局面復元を 1 局面分検査する。
///
/// 詰みが見つかる場合・見つからない場合の両方が corpus 経由でこの検査を通る。
fn verify_in_place_solver(position: &Position, expected: Option<Move32>, sfen: &str) {
    let mut work = position.clone();
    work.init_stack();
    let entry_sfen = work.to_sfen(None);
    let entry_board_key = work.board_key();
    let entry_hands = [work.hand(Color::BLACK), work.hand(Color::WHITE)];
    let entry_turn = work.turn();
    let entry_depth = work.state_stack_depth();

    let found = solve_mate_in_one_in_place(&mut work);
    assert_eq!(found, expected, "in-place solver diverged from wrapper: sfen={sfen}");

    assert_eq!(work.to_sfen(None), entry_sfen, "sfen must be restored: sfen={sfen}");
    assert_eq!(work.board_key(), entry_board_key, "board key must be restored: sfen={sfen}");
    assert_eq!(
        [work.hand(Color::BLACK), work.hand(Color::WHITE)],
        entry_hands,
        "hands must be restored: sfen={sfen}",
    );
    assert_eq!(work.turn(), entry_turn, "turn must be restored: sfen={sfen}");
    assert_eq!(
        work.state_stack_depth(),
        entry_depth,
        "state stack depth must be restored: sfen={sfen}",
    );
}

/// 1 局面に対して全オラクルを検査する。
///
/// - `solve_mate_in_one` と総当たり参照実装の一致（debug ビルドでは棄却候補の
///   非詰み assert も同時に検査される）。
/// - `solve_mate_in_one_in_place` の wrapper との一致と、呼び出し前後の局面復元。
/// - 非王手局面では全候補について `attackers_with_delta` の apply 突き合わせ。
fn verify_position_oracles(sfen: &str) {
    let position = Position::from_sfen(sfen).expect("oracle corpus line must be a valid sfen");
    let fast = solve_mate_in_one(&position);
    let naive = naive_mate_in_one(&position);
    assert_eq!(
        fast.is_some(),
        naive.is_some(),
        "solve_mate_in_one disagreed on mate existence: sfen={sfen}"
    );
    // in-place は board-first 候補順を使うため、naive の選択手と一致する。
    verify_in_place_solver(&position, naive, sfen);

    if position.is_in_check() {
        return;
    }
    let Some(ctx) = mate_context(&position) else {
        return;
    };
    let mut candidates = Move32List::new();
    generate_checks_all_move32(&position, &mut candidates);
    candidates.retain_unordered(|mv| position.is_legal_move32(mv));
    for &mv in candidates.iter() {
        verify_attackers_with_delta(&position, &ctx, mv);
    }
}

/// 手元検証用の局面集。詰将棋 corpus の先頭局面（一手詰め陽性・陰性）を含む。
const ORACLE_SFENS: &[&str] = &[
    // mate1 corpus 先頭（一手詰めが存在する陽性局面）。
    "ln1gkg1nl/6+P2/2sppps1p/2p3p2/p8/P1P1P3P/2NPbPP2/3sK1SR1/L1+b2G1NL w R2Pgp 44",
    "l3kgsnl/9/p1pS+Bp3/7pp/6PP1/9/PPPPPPn1P/1B1GG4/LNS1KG+r1L w R3Psnp 56",
    "l3k2nl/4g1gb1/1+S1pspp+P1/p1p6/3n4p/2PPR1P2/P2bPP2P/2g2GS2/LN2K3L w R2Psn2p 52",
    "lns+R4l/1p1pS4/p1p1ppB1p/4k1p2/1R7/6P1P/P1PPnPS2/2+b1G1g2/L3K1sNL b 2G3Pnp 53",
    "1+P1gkg2l/2s3s+P1/3ppp2p/P1p2npp1/l1gN1+b3/4P1P2/N2PKPS1P/2+p1G2R1/L1+r3sNL w Pbp 60",
    "lnsG5/4g4/prpp1p1pp/1p4p1k/4+B4/2P1P3S/P+b1PSP1LP/4K2SL/2G2G1r1 b P3nl3p 73",
    // 玉に隣接する相手駒を捕獲して詰ます局面（S3: 捕獲後の升への取り返し判定）。
    "lnsg2g1l/3k3p1/p1ppp+P+S2/8p/9/PPP2Sp1P/3PP4/1BGSKP+r2/LN3r2L w BG2NP2p 76",
    "lnk1p1+R1l/1rsg3+P1/p1ppG2p1/4N3p/3S5/P7P/2+lPP4/2G1KP3/L1S4+b1 b N2Pbgsn4p 85",
    // 移動元退去で開く利きの再計算（S3: fragile な封鎖）を通る詰み局面。
    "l7l/3+R5/k1+B3n2/4p1p2/P1p2p2p/2SbP1P2/1G1G1PN1P/9/LNK5L b R2GSN6P2s2p 107",
    // mate3 corpus 先頭（一手詰めが存在しない陰性局面）。
    "ln1gkg1nl/6+P2/2sppps1p/2p3p2/p8/P1P1P3P/2NP1PP2/3s1KSR1/L1+b2G1NL w R2Pbgp 42",
    "l3kgsnl/9/p1pS+Bp3/7pp/6PP1/9/PPPPPPn1P/1B1GG2+r1/LNS1K3L w RG3Psnp 54",
    "l3k2nl/4g1gb1/1+S1pspp+P1/p1p6/3n4p/2PPR1P2/P2bPP2P/5GS2/LN1K4L w R2Pgsn2p 50",
    "lns+R4l/1p1p5/p1pkppB1p/6p2/1R7/6P1P/P1PPnPS2/2+b1G1g2/L3K1sNL b 2GS3Pnp 51",
    "1+P1gkg2l/2s3s+P1/3ppp2p/P1p2npp1/l2N1+b3/3KP1P2/N2P1PS1P/2+p1G2R1/L1+r3sNL w Pbgp 58",
    "lnsG5/4g4/prpp1p1pp/1p4p2/4+B3k/2P1P4/P+b1PSP1LP/4K2SL/2G2G1r1 b SP3nl3p 71",
    // 玉に隣接する相手駒を捕獲する王手候補があるが詰まない局面（S3 の棄却経路の陰性側）。
    "1n5nl/lS5k1/1r2pg3/p1K3ppp/2P6/P2nSp3/1P+r1P3P/1s3s3/L7L w P2b3gn6p 136",
    "l5knl/4+Rg2g/pp1p+Ng2p/7P1/1SP1B1p2/PKp1s3P/1P2P4/3B5/LN2G3+p b SNLPrs4p 145",
];

/// 手番側が王手を受けている局面。静的反証の高速路には入らず、従来経路で処理される。
const IN_CHECK_SFENS: &[&str] = &[
    "l1r3bn1/3k1g1sl/2G1pp3/p2p1Pp1p/4P4/PP1P2PRP/1g1S5/4+b4/LNK3sNL w Pgsn4p 104",
    "ln2k3l/1r2g+B3/2p2g+b2/ppl2P2p/1N1pS2p1/1K2pS2P/PP1+r2+p2/L1g2+n3/9 w g2sn6p 126",
    "2lk3n1/1+P3Gs2/p1Kppp1p1/4n4/2pP2p2/7P1/P3PPPR1/3+n1G3/7S1 b R2B2G2SN3L2P2p 117",
    "l8/2+NG1P+N2/pg1pp4/5p3/Psp1bl3/s1PP5/1p2k3P/K3P2P1/LN2+r3L w Prb2g2sn4p 150",
    "l6nl/4s2b1/p3krp2/2GPpS1Np/3pP1Pp1/4sK2P/PPp2P3/1p+b1G1r+n1/L6NL w S2P2gp 108",
    "l6n1/2p1gsg2/p1n1pkpP1/5N2l/1+r6P/6P2/PB+b1gPN2/6S2/+p1PK1G2L w LPr2s6p 84",
];

#[test]
fn mate_search_preserves_input_and_handles_checked_positions() {
    let position = Position::from_sfen(
        "lnsG5/4g4/prpp1p1pp/1p4p1k/4+B4/2P1P3P/P+b1PSP1L1/4K2SL/2G2G1r1 b SP3nl3p 73",
    )
    .expect("valid mating position");
    let original = position.to_sfen(None);

    assert!(solve_mate_in_one(&position).is_some());
    assert_eq!(position.to_sfen(None), original);

    let checked =
        Position::from_sfen("4k4/9/4R4/9/9/9/9/9/4K4 w - 1").expect("valid checked position");
    assert!(checked.is_in_check());
    assert_eq!(solve_mate_in_one(&checked), None);
}

#[test]
fn refutation_oracles_hold_on_builtin_positions() {
    for sfen in ORACLE_SFENS {
        verify_position_oracles(sfen);
    }
}

#[test]
fn in_check_positions_stay_on_the_legacy_path() {
    for sfen in IN_CHECK_SFENS {
        let position = Position::from_sfen(sfen).expect("valid in-check position");
        assert!(position.is_in_check(), "sfen must be an in-check position: {sfen}");
        assert_eq!(
            solve_mate_in_one(&position),
            naive_mate_in_one(&position),
            "in-check immutable path must retain the board-first candidate order: {sfen}"
        );
        verify_position_oracles(sfen);
    }
}

fn assert_production_immutable_result(position: &Position) {
    let entry = position.to_sfen(None);
    let fast = solve_mate_in_one(position);
    let naive = naive_mate_in_one(position);
    assert_eq!(fast.is_some(), naive.is_some(), "mate existence mismatch: {entry}");
    assert_eq!(position.to_sfen(None), entry, "immutable solver mutated input: {entry}");
    if let Some(mv) = fast {
        assert!(position.is_legal_move32(mv), "illegal candidate: {entry}");
        assert!(position.gives_check_move32(mv), "non-check candidate: {entry}");
        let mut next = position.clone_for_search();
        next.apply_move32_with_gives_check(mv, true);
        assert!(next.is_mated(), "non-mating candidate: {entry}");
        next.undo_move32(mv).expect("candidate validation must undo the move it just applied");
    }
}

#[test]
fn production_immutable_streaming_matches_naive_existence_and_preserves_input() {
    let drop = Position::from_sfen("3pkp3/9/3G5/9/9/9/9/9/4K4 b R 1").expect("valid drop fixture");
    let board =
        Position::from_sfen("3pkp3/9/3G1S3/9/9/9/9/9/4K4 b - 1").expect("valid board fixture");
    let nonmate =
        Position::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b P 1").expect("valid nonmate fixture");
    let capture = Position::from_sfen(
        "6+S2/ln5gP/n1sg+R1n1+N/1S2ppp2/P2gkP3/3l3P1/1+bPpKB3/s3G1Pp+l/L4r3 w 2P5p 178",
    )
    .expect("valid capture fixture");
    let promotion = Position::from_sfen(
        "6+S2/ln5gP/n1sg+R1n1+N/1S2ppp2/P3kP3/3lB2P1/1+bPpK4/s3G1Pp+l/L4r3 w G2P5p 180",
    )
    .expect("valid promotion fixture");
    let in_check = Position::from_sfen(IN_CHECK_SFENS[0]).expect("valid in-check fixture");

    for position in [&drop, &board, &nonmate, &capture, &promotion, &in_check] {
        assert_production_immutable_result(position);
    }
    assert_production_immutable_result(
        &Position::from_sfen(ORACLE_SFENS[0]).expect("valid white fixture"),
    );

    let mut history = crate::board::hirate_position();
    for usi in ["7g7f", "3c3d", "2g2f", "8c8d"] {
        let mv = crate::board::move_from_usi(&history, usi).expect("valid history move");
        history.apply_move32(mv);
    }
    assert!(history.state_stack_depth() > 1, "fixture must carry history");
    assert_production_immutable_result(&history);
}

/// 環境変数 `RSSHOGI_MATE_ORACLE_CORPUS` で指定した corpus 全局面に同じ検証を回す。
///
/// 値は SFEN を 1 行 1 局面で並べたファイルのパス。未設定なら何もしない。
/// 例: `RSSHOGI_MATE_ORACLE_CORPUS=~/mate1-50k.sfen cargo test -p rsshogi corpus`
#[test]
fn refutation_oracles_hold_on_corpus_if_configured() {
    let Ok(path) = std::env::var("RSSHOGI_MATE_ORACLE_CORPUS") else {
        return;
    };
    let content = std::fs::read_to_string(&path).expect("corpus file must be readable");
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        verify_position_oracles(line);
    }
}
