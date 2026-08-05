//! Zobrist キーの代数的な契約を、実際の局面走査で固定する。
//!
//! 対象は 3 つ。
//!
//! - `key_after` / `board_key_after` が、実際に手を適用した後のキーと一致すること
//! - 合成キーが「盤面キー ^ 持ち駒寄与の全再計算」に分解できること
//! - apply / undo でキーが往復すること

use rsshogi::board::zobrist::{Zobrist, ZobristKey};
use rsshogi::board::{self, LegalAll, MoveList, Position, generate_moves, position_from_sfen};
use rsshogi::types::{Color, Hand, HandPiece, Move32};

/// 駒打ち・捕獲・成り・王手回避を含むよう選んだ検証用局面。
const SCENARIOS: [&str; 8] = [
    // 平手初期局面。
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
    // 捕獲が生じる中盤。
    "lnsgk1snl/1r4gb1/p1pppp2p/6pp1/1p7/2P6/PP1PPPP1P/1BG4R1/LNS1KGSNL b p 11",
    // 双方が多様な持ち駒を保有し、打ちが大量に生成される。
    "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2S2/1PPb2P1P/P5GS1/R8/LN4bKL w RGSNLPbsnl3p 1",
    // 成りを含む局面。
    "lnsgk1snl/1p4g2/pR1ppp2p/2p6/9/9/P1SPPPP1P/2G6/LN2KGSNL b B3Prb2p 25",
    // 王手がかかっており evasion のみが合法。
    "l4S2l/4g1gs1/5p1p1/pr2N1pkp/4Gn3/PP3PPPP/2GPP4/1K2r4/L4+s2L b BS2N5Pb 2",
    // 双方が持ち駒を持ち、後手番で捕獲と打ちが両方出る終盤。
    "8l/1l+R2P3/p2pBG1pp/kps1p4/Nn1P2G2/P1P1P2PP/1PS6/1KSG3+r1/LN2+p3L w Sbgn3p 1",
    // 後手番かつ持ち駒が偏っている局面。
    "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPP1PPPP/1B5R1/LNSGKGSNL w P 2",
    // 歩を大量に保有する局面。持ち駒キーの枚数添字の上の方を踏ませる。
    "4k4/9/9/9/9/9/9/9/4K4 b 15P2p 1",
];

/// 持ち駒の寄与を全再計算する。合成キーの分解を検証する参照実装。
fn recompute_hand_contribution(pos: &Position) -> ZobristKey {
    let mut key = ZobristKey::default();
    for color in [Color::BLACK, Color::WHITE] {
        let hand = pos.hand(color);
        for hp in HandPiece::iter() {
            key ^= Zobrist::hand(color, hp.to_piece_type(), Hand::count_of(hand, hp));
        }
    }
    key
}

fn legal_moves(pos: &Position) -> Vec<Move32> {
    let mut moves = MoveList::new();
    generate_moves::<LegalAll>(pos, &mut moves);
    moves.iter().map(|&mv| pos.move32_from_move(mv)).filter(|&mv| pos.is_legal_move32(mv)).collect()
}

#[test]
fn key_after_matches_the_key_produced_by_applying_the_move() {
    board::init();

    for sfen in SCENARIOS {
        let base = position_from_sfen(sfen).expect("valid sfen");
        let moves = legal_moves(&base);
        assert!(!moves.is_empty(), "scenario must have legal moves: {sfen}");

        for mv in moves {
            let predicted_key = base.key_after(mv);
            let predicted_board_key = base.board_key_after(mv);
            let predicted_board_key_from_move = base.board_key_after_move(mv.to_move());

            let mut pos = base.clone();
            pos.apply_move32(mv);

            assert_eq!(predicted_key, pos.key(), "key_after mismatch for {mv:?} in {sfen}");
            assert_eq!(
                predicted_board_key,
                pos.board_key(),
                "board_key_after mismatch for {mv:?} in {sfen}"
            );
            assert_eq!(
                predicted_board_key_from_move,
                pos.board_key(),
                "board_key_after_move mismatch for {mv:?} in {sfen}"
            );
        }
    }
}

#[test]
fn key_after_null_matches_the_key_produced_by_applying_a_null_move() {
    board::init();

    for sfen in SCENARIOS {
        let mut pos = position_from_sfen(sfen).expect("valid sfen");
        if pos.is_in_check() {
            // null move は王手中には適用できない。
            continue;
        }

        let predicted = pos.key_after_null();
        pos.apply_null_move().expect("null move");

        assert_eq!(predicted, pos.key(), "key_after_null mismatch in {sfen}");
    }
}

#[test]
fn composite_key_decomposes_into_board_key_and_hand_contribution() {
    board::init();

    for sfen in SCENARIOS {
        let base = position_from_sfen(sfen).expect("valid sfen");
        assert_eq!(
            base.key(),
            base.board_key() ^ recompute_hand_contribution(&base),
            "key must decompose at the root of {sfen}"
        );

        // 1 手進めた各局面でも分解が保たれること（差分更新の検証）。
        for mv in legal_moves(&base) {
            let mut pos = base.clone();
            pos.apply_move32(mv);
            assert_eq!(
                pos.key(),
                pos.board_key() ^ recompute_hand_contribution(&pos),
                "key must decompose after {mv:?} in {sfen}"
            );
        }
    }
}

#[test]
fn apply_and_undo_restore_both_keys() {
    board::init();

    for sfen in SCENARIOS {
        let mut pos = position_from_sfen(sfen).expect("valid sfen");
        let key_before = pos.key();
        let board_key_before = pos.board_key();

        for mv in legal_moves(&pos) {
            pos.apply_move32(mv);
            pos.undo_move32(mv).expect("undo must succeed");

            assert_eq!(pos.key(), key_before, "key must restore after {mv:?} in {sfen}");
            assert_eq!(
                pos.board_key(),
                board_key_before,
                "board_key must restore after {mv:?} in {sfen}"
            );
        }
    }
}

/// 持ち駒テーブルが PRNG 消費順の最後に置かれていることを、既知の値で固定する。
///
/// これにより `side` / `no_pawns` / psq テーブルは持ち駒テーブルの形状変更に影響されない。
/// `board_key` と partial keys は 1.0.2 とビット単位で一致しており、
/// 持ち駒方式の変更で値が動いたのは合成キーだけである、という CHANGELOG の主張を支える。
/// 生成順を変えるとここが落ちるので、落ちたら CHANGELOG の互換性記述も見直すこと。
#[test]
fn board_key_and_partial_keys_are_unaffected_by_the_hand_table() {
    board::init();

    let hirate = board::hirate_position();
    assert_eq!(hirate.board_key().low_u64(), 0x5e36_b307_5c74_b019);
    assert_eq!(hirate.partial_keys().pawn.low_u64(), 0x9532_62ce_d16c_9c68);
    assert_eq!(hirate.partial_keys().minor.low_u64(), 0x7452_3185_8bb6_159e);
    assert_eq!(hirate.partial_keys().non_pawn[0].low_u64(), 0x7c69_04c8_72c4_f940);

    // 持ち駒が空なので合成キーは盤面キーに一致する。
    assert_eq!(hirate.key(), hirate.board_key());

    // 持ち駒がある局面でも board_key は持ち駒に依存しない。
    let with_hands = position_from_sfen(
        "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2S2/1PPb2P1P/P5GS1/R8/LN4bKL w RGSNLPbsnl3p 1",
    )
    .expect("valid sfen");
    assert_eq!(with_hands.board_key().low_u64(), 0x0cc9_3493_2238_9f77);
    assert_ne!(with_hands.key(), with_hands.board_key());
}

/// 同一盤面で持ち駒だけが違う局面が、異なる合成キーかつ同一の盤面キーを持つこと。
///
/// 千日手判定は `board_key` を filter に使い、持ち駒は生の `Hand` で比較する。
/// この分業が成立するには両者の被覆範囲が食い違っていないことが要る。
#[test]
fn board_key_ignores_hands_while_composite_key_does_not() {
    board::init();

    let without = position_from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 1").expect("valid sfen");
    let with_pawn = position_from_sfen("4k4/9/9/9/9/9/9/9/4K4 b P 1").expect("valid sfen");
    let with_two_pawns = position_from_sfen("4k4/9/9/9/9/9/9/9/4K4 b 2P 1").expect("valid sfen");

    assert_eq!(without.board_key(), with_pawn.board_key());
    assert_eq!(without.board_key(), with_two_pawns.board_key());

    assert_ne!(without.key(), with_pawn.key());
    assert_ne!(without.key(), with_two_pawns.key());
    assert_ne!(with_pawn.key(), with_two_pawns.key());
}
